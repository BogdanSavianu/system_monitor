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
    valid_dataset_dir: Option<String>,
    valid_manifest: Option<String>,
    model_in: Option<String>,
    model_out: Option<String>,
    window: usize,
    train_ratio: f64,
    out: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrainingReport {
    split_mode: String,
    model_in: Option<String>,
    model_out: Option<String>,
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
    let mut valid_dataset_dir: Option<String> = None;
    let mut valid_manifest: Option<String> = None;
    let mut model_in: Option<String> = None;
    let mut model_out: Option<String> = None;
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

        if arg == "--valid-dataset-dir" {
            let value = args
                .get(i + 1)
                .context("--valid-dataset-dir expects a value")?;
            valid_dataset_dir = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--valid-dataset-dir=") {
            valid_dataset_dir = Some(value.to_string());
            i += 1;
            continue;
        }

        if arg == "--valid-manifest" {
            let value = args
                .get(i + 1)
                .context("--valid-manifest expects a value")?;
            valid_manifest = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--valid-manifest=") {
            valid_manifest = Some(value.to_string());
            i += 1;
            continue;
        }

        if arg == "--model-in" {
            let value = args.get(i + 1).context("--model-in expects a value")?;
            model_in = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--model-in=") {
            model_in = Some(value.to_string());
            i += 1;
            continue;
        }

        if arg == "--model-out" {
            let value = args.get(i + 1).context("--model-out expects a value")?;
            model_out = Some(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--model-out=") {
            model_out = Some(value.to_string());
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
    if valid_dataset_dir.is_some() && valid_manifest.is_some() {
        bail!("provide only one of --valid-dataset-dir or --valid-manifest");
    }
    if model_in.is_some() && model_out.is_some() {
        bail!("provide only one of --model-in or --model-out");
    }
    if model_in.is_some() && (valid_dataset_dir.is_some() || valid_manifest.is_some()) {
        bail!(
            "--model-in mode uses only one evaluation dataset; do not provide validation dataset arguments"
        );
    }
    if dataset_dir.is_none() && manifest.is_none() {
        dataset_dir = Some("./experiments/dataset_large".to_string());
    }

    Ok(Args {
        dataset_dir,
        manifest,
        valid_dataset_dir,
        valid_manifest,
        model_in,
        model_out,
        window,
        train_ratio,
        out,
    })
}

fn csv_paths_from_args(
    dataset_dir: &Option<String>,
    manifest: &Option<String>,
    default_dir: &str,
) -> Result<Vec<String>> {
    if let Some(manifest_path) = manifest {
        csv_paths_from_manifest(manifest_path)
    } else {
        let dir = dataset_dir.as_deref().unwrap_or(default_dir);
        csv_paths_from_dir(dir)
    }
}

fn run() -> Result<()> {
    let args = parse_args()?;

    if let Some(model_path) = &args.model_in {
        let eval_csv_paths = csv_paths_from_args(
            &args.dataset_dir,
            &args.manifest,
            "./experiments/dataset_large",
        )?;
        let eval_runs = load_runs_from_csv_paths(&eval_csv_paths)?;
        if eval_runs.is_empty() {
            bail!("evaluation dataset has no runs");
        }

        let eval_rows = eval_runs
            .iter()
            .flat_map(|r| build_feature_rows(&r.samples, args.window))
            .collect::<Vec<_>>();
        if eval_rows.is_empty() {
            bail!(
                "not enough rows after feature-window transform; lower --window or add more data"
            );
        }

        let model = RandomForestModel::load_from_path(model_path)
            .with_context(|| format!("failed to load model from '{}'", model_path))?;
        let y_true = eval_rows.iter().map(|r| r.label).collect::<Vec<_>>();
        let y_pred = model
            .predict_labels(&eval_rows)
            .context("prediction failed")?;
        let metrics = binary_metrics(&y_true, &y_pred);

        let split_mode = "saved_model_evaluation".to_string();
        println!("ml-trainer complete");
        println!("split_mode={}", split_mode);
        println!("loaded_model={}", model_path);
        println!("train_runs=0 valid_runs={}", eval_runs.len());
        println!(
            "train_rows=0 valid_rows={} window={}",
            eval_rows.len(),
            args.window
        );
        println!(
            "accuracy={:.4} precision={:.4} recall={:.4} f1={:.4}",
            metrics.accuracy, metrics.precision, metrics.recall, metrics.f1
        );

        if let Some(path) = args.out {
            let report = TrainingReport {
                split_mode,
                model_in: Some(model_path.clone()),
                model_out: None,
                train_runs: 0,
                valid_runs: eval_runs.len(),
                train_rows: 0,
                valid_rows: eval_rows.len(),
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

        return Ok(());
    }

    let train_csv_paths = csv_paths_from_args(
        &args.dataset_dir,
        &args.manifest,
        "./experiments/dataset_large",
    )?;
    let train_source_runs = load_runs_from_csv_paths(&train_csv_paths)?;

    let (split_mode, train_runs, valid_runs) =
        if args.valid_manifest.is_some() || args.valid_dataset_dir.is_some() {
            let valid_csv_paths = csv_paths_from_args(
                &args.valid_dataset_dir,
                &args.valid_manifest,
                "./experiments/dataset_large",
            )?;
            let valid_source_runs = load_runs_from_csv_paths(&valid_csv_paths)?;
            if train_source_runs.is_empty() {
                bail!("training dataset has no runs");
            }
            if valid_source_runs.is_empty() {
                bail!("validation dataset has no runs");
            }
            (
                "external_validation_dataset".to_string(),
                train_source_runs,
                valid_source_runs,
            )
        } else {
            if train_source_runs.len() < 2 {
                bail!("need at least 2 runs for train/validation split");
            }
            let (train, valid) = split_runs_by_ratio(train_source_runs, args.train_ratio);
            ("in_dataset_run_split".to_string(), train, valid)
        };

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
        bail!(
            "validation split has only one class; add more runs per class or adjust --train-ratio"
        );
    }

    let rf_config = RandomForestConfig::default();
    let model = RandomForestModel::train(&train_rows, &rf_config)
        .context("random forest training failed")?;
    if let Some(model_path) = &args.model_out {
        model
            .save_to_path(model_path)
            .with_context(|| format!("failed to save model to '{}'", model_path))?;
        println!("saved_model={}", model_path);
    }

    let y_true = valid_rows.iter().map(|r| r.label).collect::<Vec<_>>();
    let y_pred = model
        .predict_labels(&valid_rows)
        .context("prediction failed")?;
    let metrics = binary_metrics(&y_true, &y_pred);

    println!("ml-trainer complete");
    println!("split_mode={}", split_mode);
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
            split_mode,
            model_in: None,
            model_out: args.model_out,
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
