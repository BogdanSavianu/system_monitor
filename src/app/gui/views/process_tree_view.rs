use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::app::gui::view_models::ProcessHierarchyNodeViewModel;
use system_monitor::util::Pid;

pub struct NodeStats {
    pub cpu_top: f64,
    pub physical_mem_mb: f64,
    pub is_anomalous: bool,
}

pub struct ProcessTreeViewProps<'a> {
    pub roots: &'a [ProcessHierarchyNodeViewModel],
    pub expanded: &'a HashSet<u32>,
    pub tree_filter_text: &'a str,
    pub tree_focus_pid: Option<u32>,
    pub stats: &'a HashMap<Pid, NodeStats>,
    pub on_filter_change: EventHandler<String>,
    pub on_toggle_node: EventHandler<u32>,
    pub on_expand_all: EventHandler<()>,
    pub on_collapse_all: EventHandler<()>,
    pub on_focus_pid: EventHandler<Option<u32>>,
    pub on_jump_to_monitor: EventHandler<HashSet<Pid>>,
}

pub fn render_process_tree_view(props: ProcessTreeViewProps) -> Element {
    let ProcessTreeViewProps {
        roots,
        expanded,
        tree_filter_text,
        tree_focus_pid,
        stats,
        on_filter_change,
        on_toggle_node,
        on_expand_all,
        on_collapse_all,
        on_focus_pid,
        on_jump_to_monitor,
    } = props;

    let filter = tree_filter_text.to_lowercase();
    let focused_roots = if let Some(pid) = tree_focus_pid {
        roots
            .iter()
            .filter_map(|root| find_subtree(root, pid))
            .collect::<Vec<_>>()
    } else {
        roots.to_vec()
    };

    let visible_roots = if filter.is_empty() {
        focused_roots
    } else {
        focused_roots
            .into_iter()
            .filter_map(|root| filter_tree_node(&root, &filter))
            .collect::<Vec<_>>()
    };

    let filter_expanded: HashSet<Pid>;
    let effective_expanded: &HashSet<Pid> = if filter.is_empty() {
        expanded
    } else {
        filter_expanded = collect_all_pids(&visible_roots);
        &filter_expanded
    };

    rsx! {
        div {
            class: "list-panel",
            h3 { "Process tree" }
            p {
                class: "detected-leaks-subtitle",
                "Click the arrow to expand/collapse child processes"
            }

            div { class: "tree-toolbar",
                input {
                    value: "{tree_filter_text}",
                    placeholder: "Filter by pid or name",
                    oninput: move |ev| on_filter_change.call(ev.value()),
                }
                button { onclick: move |_| on_expand_all.call(()), "Expand all" }
                button { onclick: move |_| on_collapse_all.call(()), "Collapse all" }
                if tree_focus_pid.is_some() {
                    button { onclick: move |_| on_focus_pid.call(None), "Clear focus" }
                }
            }

            if visible_roots.is_empty() {
                p { class: "detected-leaks-empty", "No process tree data yet" }
            } else {
                div { class: "process-tree",
                    for node in &visible_roots {
                        {render_tree_node(node, effective_expanded, stats, on_toggle_node, on_focus_pid, on_jump_to_monitor, 0)}
                    }
                }
            }
        }
    }
}

fn render_tree_node(
    node: &ProcessHierarchyNodeViewModel,
    expanded: &HashSet<u32>,
    stats: &HashMap<Pid, NodeStats>,
    on_toggle_node: EventHandler<u32>,
    on_focus_pid: EventHandler<Option<u32>>,
    on_jump_to_monitor: EventHandler<HashSet<Pid>>,
    depth: usize,
) -> Element {
    let node_pid = node.pid;
    let has_children = !node.children.is_empty();
    let is_expanded = expanded.contains(&node.pid);
    let padding_left = format!("{}px", depth * 18);
    let child_count = node.children.len();

    let node_stats = stats.get(&node.pid);
    let cpu_top = node_stats.map(|s| s.cpu_top).unwrap_or(0.0);
    let mem_mb = node_stats.map(|s| s.physical_mem_mb).unwrap_or(0.0);
    let is_anomalous = node_stats.map(|s| s.is_anomalous).unwrap_or(false);

    let subtree_pids = collect_all_pids(std::slice::from_ref(node));
    let subtree = if has_children { Some(aggregate_subtree(node, stats)) } else { None };
    let child_label = if child_count == 1 { "1 child".to_string() } else { format!("{child_count} children") };

    let mut sorted_children = node.children.clone();
    sorted_children.sort_by(|a, b| {
        let a_cpu = stats.get(&a.pid).map(|s| s.cpu_top).unwrap_or(0.0);
        let b_cpu = stats.get(&b.pid).map(|s| s.cpu_top).unwrap_or(0.0);
        b_cpu.total_cmp(&a_cpu)
    });

    rsx! {
        div {
            class: if is_anomalous { "process-tree-node tree-node-anomalous" } else { "process-tree-node" },
            style: "padding-left: {padding_left};",

            if has_children {
                button {
                    class: "tree-toggle-btn",
                    onclick: move |_| on_toggle_node.call(node_pid),
                    if is_expanded { "v" } else { ">" }
                }
            } else {
                span { class: "tree-leaf-marker", "–" }
            }

            if is_anomalous {
                span {
                    class: "hazard-indicator",
                    title: "Suspected memory leak",
                    "⚠"
                }
            }

            span { class: "tree-node-name", "{node.name}" }
            span { class: "tree-node-pid", "pid {node.pid}" }

            if cpu_top > 0.0 {
                span { class: "tree-node-cpu", "{cpu_top:.1}%" }
            }
            if mem_mb > 0.0 {
                span { class: "tree-node-mem", "{mem_mb:.0} MB" }
            }

            if has_children && !is_expanded {
                span { class: "tree-child-count", "{child_label}" }
            }

            button {
                class: "tree-focus-btn",
                onclick: move |_| on_focus_pid.call(Some(node_pid)),
                "focus"
            }

            button {
                class: "tree-jump-btn",
                onclick: move |_| on_jump_to_monitor.call(subtree_pids.clone()),
                "→ monitor"
            }
        }

        if let Some(totals) = subtree {
            div {
                class: "tree-subtree-totals",
                style: "padding-left: calc({padding_left} + 30px);",
                span { class: "tree-subtree-label", "subtree" }
                if totals.cpu > 0.0 {
                    span { class: "tree-node-cpu", "{totals.cpu:.1}%" }
                }
                if totals.mem_mb > 0.0 {
                    span { class: "tree-node-mem", "{totals.mem_mb:.0} MB" }
                }
                span { class: "tree-subtree-count", "{totals.process_count} processes" }
            }
        }

        if has_children && is_expanded {
            for child in &sorted_children {
                {render_tree_node(child, expanded, stats, on_toggle_node, on_focus_pid, on_jump_to_monitor, depth + 1)}
            }
        }
    }
}

struct SubtreeTotals {
    cpu: f64,
    mem_mb: f64,
    process_count: usize,
}

fn aggregate_subtree(node: &ProcessHierarchyNodeViewModel, stats: &HashMap<Pid, NodeStats>) -> SubtreeTotals {
    let mut cpu = 0.0_f64;
    let mut mem_mb = 0.0_f64;
    let mut process_count = 0_usize;
    fn recurse(node: &ProcessHierarchyNodeViewModel, stats: &HashMap<Pid, NodeStats>, cpu: &mut f64, mem: &mut f64, count: &mut usize) {
        if let Some(s) = stats.get(&node.pid) {
            *cpu += s.cpu_top;
            *mem += s.physical_mem_mb;
            *count += 1;
        }
        for child in &node.children {
            recurse(child, stats, cpu, mem, count);
        }
    }
    for child in &node.children {
        recurse(child, stats, &mut cpu, &mut mem_mb, &mut process_count);
    }
    SubtreeTotals { cpu, mem_mb, process_count }
}

fn collect_all_pids(nodes: &[ProcessHierarchyNodeViewModel]) -> HashSet<Pid> {
    let mut set = HashSet::new();
    fn recurse(node: &ProcessHierarchyNodeViewModel, set: &mut HashSet<Pid>) {
        set.insert(node.pid);
        for child in &node.children {
            recurse(child, set);
        }
    }
    for node in nodes {
        recurse(node, &mut set);
    }
    set
}

fn find_subtree(
    node: &ProcessHierarchyNodeViewModel,
    pid: u32,
) -> Option<ProcessHierarchyNodeViewModel> {
    if node.pid == pid {
        return Some(node.clone());
    }
    for child in &node.children {
        if let Some(found) = find_subtree(child, pid) {
            return Some(found);
        }
    }
    None
}

fn filter_tree_node(
    node: &ProcessHierarchyNodeViewModel,
    filter: &str,
) -> Option<ProcessHierarchyNodeViewModel> {
    let self_match =
        node.pid.to_string().contains(filter) || node.name.to_lowercase().contains(filter);
    let filtered_children = node
        .children
        .iter()
        .filter_map(|child| filter_tree_node(child, filter))
        .collect::<Vec<_>>();

    if self_match || !filtered_children.is_empty() {
        Some(ProcessHierarchyNodeViewModel {
            pid: node.pid,
            ppid: node.ppid,
            name: node.name.clone(),
            children: filtered_children,
        })
    } else {
        None
    }
}
