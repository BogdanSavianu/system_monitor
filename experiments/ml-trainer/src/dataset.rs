use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::features::TelemetrySample;

#[derive(Debug, Clone)]
pub struct LabeledRun {
    pub run_id: String,
    pub scenario: String,
    pub samples: Vec<TelemetrySample>,
}

#[derive(Debug, Deserialize)]
struct CsvRow {
    scenario: String,
    label: u8,
    run_id: String,
    step: u64,
    elapsed_s: f64,
    leaked_kb_step: f64,
    leaked_kb_total: f64,
    workload_kb_this_step: f64,
}

pub fn csv_paths_from_manifest<P: AsRef<Path>>(manifest: P) -> Result<Vec<String>> {
    let manifest_ref = manifest.as_ref();
    let content = fs::read_to_string(manifest_ref)
        .with_context(|| format!("failed to read manifest '{}'", manifest_ref.display()))?;

    let paths = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if paths.is_empty() {
        bail!(
            "manifest '{}' contains no csv paths",
            manifest_ref.display()
        );
    }

    Ok(paths)
}

pub fn csv_paths_from_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<String>> {
    let dir_ref = dir.as_ref();
    let mut paths = Vec::new();

    for entry in fs::read_dir(dir_ref)
        .with_context(|| format!("failed to read dataset directory '{}'", dir_ref.display()))?
    {
        let entry = entry?;
        let path: PathBuf = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("csv") {
            paths.push(path.display().to_string());
        }
    }

    paths.sort();
    if paths.is_empty() {
        bail!("no csv files found in '{}'", dir_ref.display());
    }

    Ok(paths)
}

pub fn load_runs_from_csv_paths(paths: &[String]) -> Result<Vec<LabeledRun>> {
    let mut all_runs = Vec::new();
    for path in paths {
        let mut runs = load_runs_from_csv(path)?;
        all_runs.append(&mut runs);
    }
    Ok(all_runs)
}

pub fn load_runs_from_csv<P: AsRef<Path>>(path: P) -> Result<Vec<LabeledRun>> {
    let path_ref = path.as_ref();
    let source_key = path_ref.display().to_string();
    let mut reader = csv::Reader::from_path(path_ref)
        .with_context(|| format!("failed to open csv '{}'", path_ref.display()))?;

    let mut grouped: BTreeMap<String, Vec<CsvRow>> = BTreeMap::new();
    for row in reader.deserialize::<CsvRow>() {
        let parsed =
            row.with_context(|| format!("failed to parse row in '{}'", path_ref.display()))?;
        let scoped_run_id = format!("{}::{}", source_key, parsed.run_id);
        grouped.entry(scoped_run_id).or_default().push(parsed);
    }

    let mut runs = Vec::with_capacity(grouped.len());
    for (run_id, mut rows) in grouped {
        rows.sort_by_key(|r| r.step);
        let scenario_name = rows
            .first()
            .map(|r| r.scenario.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let samples = rows
            .into_iter()
            .map(|r| TelemetrySample {
                elapsed_s: r.elapsed_s,
                leaked_kb_step: r.leaked_kb_step,
                leaked_kb_total: r.leaked_kb_total,
                workload_kb_this_step: r.workload_kb_this_step,
                observed_memory_kb: synth_observed_memory_kb(
                    &scenario_name,
                    &run_id,
                    r.step,
                    r.label,
                    r.leaked_kb_step,
                    r.leaked_kb_total,
                    r.workload_kb_this_step,
                ),
                label: r.label,
            })
            .collect::<Vec<_>>();

        runs.push(LabeledRun {
            run_id,
            scenario: scenario_name,
            samples,
        });
    }

    Ok(runs)
}

fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    x
}

fn hash64(parts: &[u64]) -> u64 {
    let mut h = 0x9e3779b97f4a7c15u64;
    for p in parts {
        h = mix64(h ^ p);
    }
    h
}

fn str_hash64(s: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211u64);
    }
    h
}

fn synth_observed_memory_kb(
    scenario: &str,
    run_id: &str,
    step: u64,
    label: u8,
    leaked_kb_step: f64,
    leaked_kb_total: f64,
    workload_kb_this_step: f64,
) -> f64 {
    let run_h = str_hash64(run_id);
    let sc_h = str_hash64(scenario);
    let h = hash64(&[run_h, sc_h, step]);

    let base_kb = 1200.0 + (h % 1300) as f64;
    let jitter = ((h % 121) as f64) - 60.0;
    let workload_term = workload_kb_this_step * 0.015;

    let mut observed = base_kb + workload_term + leaked_kb_total + jitter;

    if label == 0 {
        // for negatives sometimes allow slow upward drift and occasional memory bursts.
        let drift_per_step = 0.25 + ((h >> 8) % 60) as f64 / 100.0;
        observed += drift_per_step * step as f64;
        if h % 53 == 0 {
            observed += 120.0 + ((h >> 16) % 420) as f64;
        }
    } else {
        // for positives create occasional plateaus and periodic recoveries.
        if h % 19 < 5 {
            observed -= leaked_kb_step * 0.9;
        }
        if h % 41 == 0 {
            observed -= 80.0 + ((h >> 12) % 240) as f64;
        }
    }

    observed.max(0.0)
}

pub fn split_runs_validation_scenarios(
    runs: Vec<LabeledRun>,
    train_ratio: f64,
) -> (Vec<LabeledRun>, Vec<LabeledRun>) {
    fn label_of(run: &LabeledRun) -> u8 {
        run.samples.first().map(|s| s.label).unwrap_or(0)
    }

    fn split_for_label(
        group: Vec<LabeledRun>,
        ratio: f64,
        train_out: &mut Vec<LabeledRun>,
        valid_out: &mut Vec<LabeledRun>,
    ) {
        if group.is_empty() {
            return;
        }

        let mut scenarios = group
            .iter()
            .map(|r| r.scenario.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        scenarios.sort();

        if scenarios.len() < 2 {
            train_out.extend(group);
            return;
        }

        let mut train_scenario_count = ((scenarios.len() as f64) * ratio).round() as usize;
        train_scenario_count = train_scenario_count.clamp(1, scenarios.len() - 1);

        let train_scenarios = scenarios[..train_scenario_count]
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();

        for run in group {
            if train_scenarios.contains(&run.scenario) {
                train_out.push(run);
            } else {
                valid_out.push(run);
            }
        }
    }

    let ratio = train_ratio.clamp(0.1, 0.95);
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for run in runs {
        if label_of(&run) == 1 {
            pos.push(run);
        } else {
            neg.push(run);
        }
    }

    let mut train = Vec::new();
    let mut valid = Vec::new();
    split_for_label(pos, ratio, &mut train, &mut valid);
    split_for_label(neg, ratio, &mut train, &mut valid);

    train.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    valid.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    (train, valid)
}

pub fn split_runs_by_ratio(
    runs: Vec<LabeledRun>,
    train_ratio: f64,
) -> (Vec<LabeledRun>, Vec<LabeledRun>) {
    fn split_group(
        mut group: Vec<LabeledRun>,
        ratio: f64,
        train_out: &mut Vec<LabeledRun>,
        valid_out: &mut Vec<LabeledRun>,
    ) {
        if group.is_empty() {
            return;
        }

        group.sort_by(|a, b| a.run_id.cmp(&b.run_id));
        let mut train_count = ((group.len() as f64) * ratio).round() as usize;
        if group.len() >= 2 {
            train_count = train_count.clamp(1, group.len() - 1);
        } else {
            train_count = 1;
        }

        let mut valid = if train_count < group.len() {
            group.split_off(train_count)
        } else {
            Vec::new()
        };

        train_out.append(&mut group);
        valid_out.append(&mut valid);
    }

    let ratio = train_ratio.clamp(0.1, 0.95);

    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for run in runs {
        let label = run.samples.first().map(|s| s.label).unwrap_or(0);
        if label == 1 {
            pos.push(run);
        } else {
            neg.push(run);
        }
    }

    let mut train = Vec::new();
    let mut valid = Vec::new();
    split_group(pos, ratio, &mut train, &mut valid);
    split_group(neg, ratio, &mut train, &mut valid);

    train.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    valid.sort_by(|a, b| a.run_id.cmp(&b.run_id));

    (train, valid)
}
