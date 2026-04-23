use system_monitor::{
    anomaly::{
        RandomForestConfig, RandomForestModel, binary_metrics, build_feature_rows,
        load_runs_from_csv_paths, split_runs_by_ratio,
    },
    util::ParseError,
};

use super::args::TrainAnomalyArgs;

pub fn run_training(args: &TrainAnomalyArgs) -> Result<(), ParseError> {
    let runs = load_runs_from_csv_paths(&args.csv_paths)?;
    if runs.len() < 2 {
        return Err(ParseError::ParsingError(
            "need at least 2 runs for train/validation split".to_string(),
        ));
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
        return Err(ParseError::ParsingError(
            "not enough rows after feature-window transform; lower --window or add more data"
                .to_string(),
        ));
    }

    let train_has_pos = train_rows.iter().any(|r| r.label == 1);
    let train_has_neg = train_rows.iter().any(|r| r.label == 0);
    if !(train_has_pos && train_has_neg) {
        return Err(ParseError::ParsingError(
            "training split has only one class; add more runs per class or adjust --train-ratio"
                .to_string(),
        ));
    }

    let valid_has_pos = valid_rows.iter().any(|r| r.label == 1);
    let valid_has_neg = valid_rows.iter().any(|r| r.label == 0);
    if !(valid_has_pos && valid_has_neg) {
        return Err(ParseError::ParsingError(
            "validation split has only one class; add more runs per class or adjust --train-ratio"
                .to_string(),
        ));
    }

    let model = RandomForestModel::train(&train_rows, RandomForestConfig::default())
        .map_err(|e| ParseError::ParsingError(format!("random forest training failed: {}", e)))?;

    let y_true = valid_rows.iter().map(|r| r.label).collect::<Vec<_>>();
    let y_pred = model
        .predict_labels(&valid_rows)
        .map_err(|e| ParseError::ParsingError(format!("prediction failed: {}", e)))?;

    let metrics = binary_metrics(&y_true, &y_pred);

    println!("anomaly training complete");
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

    Ok(())
}
