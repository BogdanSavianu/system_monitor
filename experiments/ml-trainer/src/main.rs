mod dataset;
mod eval;
mod features;
mod model_rf;

use std::env;
use std::fs;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::dataset::{
    csv_paths_from_dir, csv_paths_from_manifest, load_runs_from_csv_paths, split_runs_by_ratio,
};
use crate::eval::binary_metrics;
use crate::features::build_feature_rows;
use crate::model_rf::{RandomForestConfig, RandomForestModel};

#[derive(Debug, Clone)]
struct Args {
    dataset_dir: Option<String>,
    manifest: Option<String>,
    window: usize,
    train_ratio: f64,
    out: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrainingReport {
    train_runs: usize,
    valid_runs: usize,
    train_rows: usize,
    valid_rows: usize,
    window: usize,
    train_ratio: f64,
    accuracy: f64,
    precision: f64,
    recall: f64,
    f1: f64,
}

fn parse_args() -> Result<Args> {
    let mut dataset_dir: Option<String> = None;
    let mut manifest: Option<String> = None;
    let mut window: usize = 24;
    let mut train_ratio: f64 = 0.8;
    let mut out: Option<String> = None;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--dataset-dir" {
            let value = args.get(i + 1).context("--dataset-dir expects a value")?;
            dataset_dir = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--dataset-dir=") {
            dataset_dir = Some(value.to_string());
            i += 1;
            continue;
        }

        if arg == "--manifest" {
            let value = args.get(i + 1).context("--manifest expects a value")?;
            manifest = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--manifest=") {
            manifest = Some(value.to_string());
            i += 1;
            continue;
        }

        if arg == "--window" {
            let value = args.get(i + 1).context("--window expects a value")?;
            window = value
                .parse::<usize>()
                .with_context(|| format!("invalid --window value '{}'", value))?;
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--window=") {
            window = value
                .parse::<usize>()
                .with_context(|| format!("invalid --window value '{}'", value))?;
            i += 1;
            continue;
        }

        if arg == "--train-ratio" {
            let value = args.get(i + 1).context("--train-ratio expects a value")?;
            train_ratio = value
                .parse::<f64>()
                .with_context(|| format!("invalid --train-ratio value '{}'", value))?;
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--train-ratio=") {
            train_ratio = value
                .parse::<f64>()
                .with_context(|| format!("invalid --train-ratio value '{}'", value))?;
            i += 1;
            continue;
        }

        if arg == "--out" {
            let value = args.get(i + 1).context("--out expects a value")?;
            out = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--out=") {
            out = Some(value.to_string());
            i += 1;
            continue;
        }

        bail!("unknown argument '{}'", arg);
    }

    if window < 2 {
        bail!("--window must be >= 2");
    }
    if !(0.1..=0.95).contains(&train_ratio) {
        bail!("--train-ratio must be in [0.1, 0.95]");
    }
    if dataset_dir.is_none() && manifest.is_none() {
        dataset_dir = Some("./experiments/dataset_large".to_string());
    }

    Ok(Args {
        dataset_dir,
        manifest,
        window,
        train_ratio,
        out,
    })
}

fn run() -> Result<()> {
    let args = parse_args()?;

    let csv_paths = if let Some(manifest) = &args.manifest {
        csv_paths_from_manifest(manifest)?
    } else {
        let dir = args.dataset_dir.as_deref().unwrap_or("./experiments/dataset_large");
        csv_paths_from_dir(dir)?
    };

    let runs = load_runs_from_csv_paths(&csv_paths)?;
    if runs.len() < 2 {
        bail!("need at least 2 runs for train/validation split");
    }

    let (train_runs, valid_runs) = split_runs_by_ratio(runs, args.train_ratio);

    let train_rows = train_runs
        .iter()
        .flat_map(|r| build_feature_rows(&r.samples, args.window))
        .collect::<Vec<_>>();
    let valid_rows = valid_runs
        .iter()
        .flat_map(|r| build_feature_rows(&r.samples, args.window))
        .collect::<Vec<_>>();

    if train_rows.is_empty() || valid_rows.is_empty() {
        bail!("not enough rows after feature-window transform; lower --window or add more data");
    }

    let train_has_pos = train_rows.iter().any(|r| r.label == 1);
    let train_has_neg = train_rows.iter().any(|r| r.label == 0);
    if !(train_has_pos && train_has_neg) {
        bail!("training split has only one class; add more runs per class or adjust --train-ratio");
    }

    let valid_has_pos = valid_rows.iter().any(|r| r.label == 1);
    let valid_has_neg = valid_rows.iter().any(|r| r.label == 0);
    if !(valid_has_pos && valid_has_neg) {
        bail!("validation split has only one class; add more runs per class or adjust --train-ratio");
    }

    let rf_config = RandomForestConfig::default();
    let model = RandomForestModel::train(&train_rows, &rf_config)
        .context("random forest training failed")?;

    let y_true = valid_rows.iter().map(|r| r.label).collect::<Vec<_>>();
    let y_pred = model.predict_labels(&valid_rows).context("prediction failed")?;
    let metrics = binary_metrics(&y_true, &y_pred);

    println!("ml-trainer complete");
    println!(
        "train_runs={} valid_runs={}",
        train_runs.len(),
        valid_runs.len()
    );
    println!(
        "train_rows={} valid_rows={} window={}",
        train_rows.len(),
        valid_rows.len(),
        args.window
    );
    println!(
        "accuracy={:.4} precision={:.4} recall={:.4} f1={:.4}",
        metrics.accuracy, metrics.precision, metrics.recall, metrics.f1
    );

    if let Some(path) = args.out {
        let report = TrainingReport {
            train_runs: train_runs.len(),
            valid_runs: valid_runs.len(),
            train_rows: train_rows.len(),
            valid_rows: valid_rows.len(),
            window: args.window,
            train_ratio: args.train_ratio,
            accuracy: metrics.accuracy,
            precision: metrics.precision,
            recall: metrics.recall,
            f1: metrics.f1,
        };

        let json = serde_json::to_string_pretty(&report)?;
        fs::write(&path, json).with_context(|| format!("failed to write report '{}'", path))?;
        println!("report={}", path);
    }

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
