use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::util::ParseError;

use super::features::TelemetrySample;

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

pub fn load_runs_from_csv_paths(paths: &[String]) -> Result<Vec<LabeledRun>, ParseError> {
    let mut all_runs = Vec::new();
    for path in paths {
        let mut runs = load_runs_from_csv(path)?;
        all_runs.append(&mut runs);
    }
    Ok(all_runs)
}

pub fn load_runs_from_csv<P: AsRef<Path>>(path: P) -> Result<Vec<LabeledRun>, ParseError> {
    let path_ref = path.as_ref();
    let source_key = path_ref.display().to_string();
    let mut reader = csv::Reader::from_path(path_ref).map_err(|err| {
        ParseError::ParsingError(format!(
            "failed to open csv '{}': {}",
            path_ref.display(),
            err
        ))
    })?;

    let mut grouped: BTreeMap<String, Vec<CsvRow>> = BTreeMap::new();
    for row in reader.deserialize::<CsvRow>() {
        let parsed = row.map_err(|err| {
            ParseError::ParsingError(format!(
                "failed to parse csv row in '{}': {}",
                path_ref.display(),
                err
            ))
        })?;
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
