use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::features::TelemetrySample;

#[derive(Debug, Clone)]
pub struct LabeledRun {
    pub run_id: String,
    pub samples: Vec<TelemetrySample>,
}

#[derive(Debug, Deserialize)]
struct CsvRow {
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
        let samples = rows
            .into_iter()
            .map(|r| TelemetrySample {
                elapsed_s: r.elapsed_s,
                leaked_kb_step: r.leaked_kb_step,
                leaked_kb_total: r.leaked_kb_total,
                workload_kb_this_step: r.workload_kb_this_step,
                label: r.label,
            })
            .collect::<Vec<_>>();

        runs.push(LabeledRun { run_id, samples });
    }

    Ok(runs)
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
