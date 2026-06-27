//! turns `sym+offset` stack frames into `file:line` using each loaded elf's
//! dwarf info. frames it can't resolve are left as they are

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use object::{Object, ObjectSymbol};

use crate::util::Pid;

pub struct Symbolizer {
    /// object basename to full path, from `/proc/<pid>/maps` and the exe link
    basename_to_path: HashMap<String, PathBuf>,
    /// per-object resolver, built lazily. a `None` slot means we tried and
    /// failed, so don't retry it every frame
    loaded: HashMap<String, Option<LoadedObject>>,
    /// the target's main executable. frames with no `[object]` annotation
    /// try this first, then fall back to scanning every known object
    default_obj: Option<String>,
    debug: bool,
    debug_budget: std::cell::Cell<u32>,
}

struct LoadedObject {
    /// symbol name to file-relative address from the elf symbol table
    syms: HashMap<String, u64>,
    /// dwarf resolver for addr to file:line
    loader: addr2line::Loader,
}

impl Symbolizer {
    pub fn from_pid(pid: Pid) -> Self {
        let mut basename_to_path: HashMap<String, PathBuf> = HashMap::new();
        let mut default_obj: Option<String> = None;

        // /proc/<pid>/exe is a symlink to the on-disk binary
        if let Ok(exe) = std::fs::read_link(format!("/proc/{pid}/exe"))
            && let Some(name) = exe.file_name().and_then(|n| n.to_str())
        {
            default_obj = Some(name.to_string());
            basename_to_path.insert(name.to_string(), exe);
        }

        // every file-backed mapping. the same path appears once per elf
        // segment, so keep the first
        if let Ok(maps) = std::fs::read_to_string(format!("/proc/{pid}/maps")) {
            for line in maps.lines() {
                if let Some(path) = line.split_whitespace().last() {
                    if !path.starts_with('/') {
                        continue;
                    }
                    if let Some(name) = Path::new(path).file_name().and_then(|n| n.to_str()) {
                        basename_to_path
                            .entry(name.to_string())
                            .or_insert_with(|| PathBuf::from(path));
                    }
                }
            }
        }

        let debug = std::env::var_os("LEAKPROBE_DEBUG").is_some();
        if debug {
            eprintln!(
                "[leakprobe] symbolizer pid={pid}: {} known objects, default={:?}",
                basename_to_path.len(),
                default_obj
            );
        }

        Self {
            basename_to_path,
            loaded: HashMap::new(),
            default_obj,
            debug,
            debug_budget: std::cell::Cell::new(8),
        }
    }

    /// resolves each frame to `file:line` where it can
    pub fn enrich_stack(&mut self, frames: &[String]) -> Vec<String> {
        frames.iter().map(|f| self.enrich_frame(f)).collect()
    }

    /// applies enrich_stack to every value in an addr-to-frames map
    pub fn enrich_alloc_stacks(
        &mut self,
        stacks: HashMap<u64, Vec<String>>,
    ) -> HashMap<u64, Vec<String>> {
        stacks
            .into_iter()
            .map(|(addr, frames)| (addr, self.enrich_stack(&frames)))
            .collect()
    }

    fn enrich_frame(&mut self, frame: &str) -> String {
        let Some((sym, offset, obj_hint)) = parse_frame(frame) else {
            if self.debug && self.debug_budget.get() > 0 {
                self.debug_budget.set(self.debug_budget.get() - 1);
                eprintln!("[leakprobe] symbolize: cannot parse frame {frame:?} (no sym+offset)");
            }
            return frame.to_string();
        };
        // candidate objects in order: explicit `[obj]`, then the main
        // executable, then everything else. first one whose symbol table has
        // `sym` wins
        let mut tried: Vec<String> = Vec::new();
        let mut candidates: Vec<String> = Vec::new();
        if let Some(o) = obj_hint.clone() {
            candidates.push(o);
        }
        if let Some(d) = self.default_obj.clone()
            && !candidates.contains(&d)
        {
            candidates.push(d);
        }
        let mut others: Vec<String> = self.basename_to_path.keys().cloned().collect();
        others.sort();
        for o in others {
            if !candidates.contains(&o) {
                candidates.push(o);
            }
        }

        for candidate in &candidates {
            tried.push(candidate.clone());
            self.ensure_loaded(candidate);
            let Some(Some(info)) = self.loaded.get(candidate) else {
                continue;
            };
            let Some(&sym_addr) = info.syms.get(&sym) else {
                continue;
            };
            let target = sym_addr.wrapping_add(offset);
            match info.loader.find_location(target) {
                Ok(Some(loc)) => {
                    let file = loc.file.unwrap_or("?");
                    let basename = Path::new(file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file);
                    let result = match loc.line {
                        Some(line) => format!("{frame}  →  {basename}:{line}"),
                        None => format!("{frame}  →  {basename}"),
                    };
                    if self.debug && self.debug_budget.get() > 0 {
                        self.debug_budget.set(self.debug_budget.get() - 1);
                        eprintln!(
                            "[leakprobe] symbolize: {sym}+{offset:#x} resolved via {candidate} → {result}"
                        );
                    }
                    return result;
                }
                Ok(None) => {
                    if self.debug && self.debug_budget.get() > 0 {
                        self.debug_budget.set(self.debug_budget.get() - 1);
                        eprintln!(
                            "[leakprobe] symbolize: {sym}+{offset:#x} found in {candidate} \
                             at {target:#x}, no .debug_line entry"
                        );
                    }
                    return frame.to_string();
                }
                Err(_) => continue,
            }
        }
        if self.debug && self.debug_budget.get() > 0 {
            self.debug_budget.set(self.debug_budget.get() - 1);
            eprintln!(
                "[leakprobe] symbolize: {sym}+{offset:#x} unresolved \
                 (hint={obj_hint:?}, tried {} objects)",
                tried.len()
            );
        }
        frame.to_string()
    }

    fn ensure_loaded(&mut self, basename: &str) {
        if self.loaded.contains_key(basename) {
            return;
        }
        let info = self
            .basename_to_path
            .get(basename)
            .cloned()
            .and_then(|path| build_object(&path));
        if self.debug {
            eprintln!(
                "[leakprobe] symbolize: load {basename} → {}",
                if info.is_some() {
                    "ok"
                } else {
                    "no debug info / unreadable"
                }
            );
        }
        self.loaded.insert(basename.to_string(), info);
    }
}

fn parse_frame(frame: &str) -> Option<(String, u64, Option<String>)> {
    let frame = frame.trim();
    // optional trailing `[object]` annotation.
    let (sym_off, obj) = if let Some((sym_off, rest)) = frame.rsplit_once(" [") {
        let obj = rest.trim_end_matches(']').trim().to_string();
        let obj = if obj.is_empty() { None } else { Some(obj) };
        (sym_off, obj)
    } else {
        (frame, None)
    };
    // demangled c++ names can contain `+`, so split at the last one
    let (sym, off_raw) = sym_off.rsplit_once('+')?;
    // a symbol can't start with a digit or `0x`
    if sym.is_empty() || sym.starts_with("0x") || sym.starts_with("0X") {
        return None;
    }
    if sym.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let off = off_raw.trim();
    let offset = if let Some(hex) = off.strip_prefix("0x").or_else(|| off.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()?
    } else {
        off.parse::<u64>().ok()?
    };
    Some((sym.trim().to_string(), offset, obj))
}

fn build_object(path: &Path) -> Option<LoadedObject> {
    let syms = read_symbols(path).ok()?;
    let loader = addr2line::Loader::new(path).ok()?;
    Some(LoadedObject { syms, loader })
}

fn read_symbols(path: &Path) -> std::io::Result<HashMap<String, u64>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let object = object::File::parse(&*mmap).map_err(|e| std::io::Error::other(format!("{e}")))?;
    let mut syms = HashMap::new();
    // symbols() visits both .dynsym and .symtab
    for sym in object.symbols() {
        if sym.address() == 0 {
            continue;
        }
        let Ok(name) = sym.name() else { continue };
        syms.entry(name.to_string()).or_insert(sym.address());
    }
    Ok(syms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_offset_frame() {
        let (sym, off, obj) = parse_frame("main+0x1dc [staircase_leak]").unwrap();
        assert_eq!(sym, "main");
        assert_eq!(off, 0x1dc);
        assert_eq!(obj.as_deref(), Some("staircase_leak"));
    }

    #[test]
    fn parses_decimal_offset_frame() {
        let (sym, off, obj) = parse_frame("__libc_start_call_main+116 [libc.so.6]").unwrap();
        assert_eq!(sym, "__libc_start_call_main");
        assert_eq!(off, 116);
        assert_eq!(obj.as_deref(), Some("libc.so.6"));
    }

    #[test]
    fn parses_frame_without_object_annotation() {
        // bpftrace's default ustack on aarch64 emits bare sym+offset with no
        // `[obj]` anchor, so the symbolizer scans every loaded object
        let (sym, off, obj) = parse_frame("main+420").unwrap();
        assert_eq!(sym, "main");
        assert_eq!(off, 420);
        assert!(obj.is_none());
    }

    #[test]
    fn rejects_bare_hex_address() {
        // bare hex addresses are unresolved frames, nothing to symbolize
        assert!(parse_frame("0x50ae44").is_none());
    }

    #[test]
    fn handles_plus_inside_symbol_name() {
        // demangled c++ names can contain `+`, so we split on the last one
        let (sym, off, _) = parse_frame("foo<int, X+Y>::bar+0x10 [a.out]").unwrap();
        assert_eq!(sym, "foo<int, X+Y>::bar");
        assert_eq!(off, 0x10);
    }
}
