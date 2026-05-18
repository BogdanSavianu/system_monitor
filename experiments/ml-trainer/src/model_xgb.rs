use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use smartcore::error::Failed;
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::xgboost::{XGRegressor, XGRegressorParameters};

use crate::features::FeatureRow;

#[derive(Debug, Clone)]
pub struct XGBoostConfig {
    pub n_estimators: usize,
    pub max_depth: u16,
    pub learning_rate: f64,
    pub min_child_weight: usize,
    pub lambda: f64,
    pub gamma: f64,
    pub subsample: f64,
    pub threshold: f64,
}

impl Default for XGBoostConfig {
    fn default() -> Self {
        Self {
            n_estimators: 500,
            max_depth: 6,
            learning_rate: 0.05,
            min_child_weight: 20,
            lambda: 1.0,
            gamma: 0.0,
            subsample: 1.0,
            threshold: 0.2,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XGBoostModel {
    model_type: String,
    threshold: f64,
    model: XGRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
}

impl XGBoostModel {
    pub fn train(rows: &[FeatureRow], config: &XGBoostConfig) -> Result<Self, Failed> {
        let x = rows.iter().map(FeatureRow::as_vec).collect::<Vec<_>>();
        let y = rows.iter().map(|r| r.label as f64).collect::<Vec<_>>();

        let x = DenseMatrix::from_2d_vec(&x)?;
        let params = XGRegressorParameters::default()
            .with_n_estimators(config.n_estimators)
            .with_max_depth(config.max_depth)
            .with_learning_rate(config.learning_rate)
            .with_min_child_weight(config.min_child_weight)
            .with_lambda(config.lambda)
            .with_gamma(config.gamma)
            .with_subsample(config.subsample)
            .with_seed(42);

        let model = XGRegressor::fit(&x, &y, params)?;
        Ok(Self {
            model_type: "xgboost".to_string(),
            threshold: config.threshold,
            model,
        })
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn set_threshold(&mut self, threshold: f64) {
        self.threshold = threshold;
    }

    pub fn predict_scores(&self, rows: &[FeatureRow]) -> Result<Vec<f64>, Failed> {
        let x = rows.iter().map(FeatureRow::as_vec).collect::<Vec<_>>();
        let x = DenseMatrix::from_2d_vec(&x)?;
        self.model.predict(&x)
    }

    pub fn predict_labels(&self, rows: &[FeatureRow]) -> Result<Vec<u8>, Failed> {
        let pred = self.predict_scores(rows)?;
        Ok(pred
            .into_iter()
            .map(|v| if v >= self.threshold { 1 } else { 0 })
            .collect())
    }

    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string(self).context("serialize xgboost model")?;
        fs::write(path.as_ref(), json)
            .with_context(|| format!("write model file '{}'", path.as_ref().display()))?;
        Ok(())
    }

    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = fs::read_to_string(path.as_ref())
            .with_context(|| format!("read model file '{}'", path.as_ref().display()))?;
        let model = serde_json::from_str(&data).context("deserialize xgboost model")?;
        Ok(model)
    }
}
