use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use smartcore::ensemble::random_forest_classifier::{
    RandomForestClassifier, RandomForestClassifierParameters,
};
use smartcore::error::Failed;
use smartcore::linalg::basic::matrix::DenseMatrix;

use crate::features::FeatureRow;

#[derive(Debug, Clone)]
pub struct RandomForestConfig {
    pub n_trees: usize,
    pub max_depth: Option<usize>,
    pub min_samples_split: usize,
}

impl Default for RandomForestConfig {
    fn default() -> Self {
        Self {
            n_trees: 300,
            max_depth: Some(16),
            min_samples_split: 4,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RandomForestModel {
    model: RandomForestClassifier<f64, u32, DenseMatrix<f64>, Vec<u32>>,
}

impl RandomForestModel {
    pub fn train(rows: &[FeatureRow], config: &RandomForestConfig) -> Result<Self, Failed> {
        let x = rows.iter().map(|r| r.as_vec()).collect::<Vec<_>>();
        let y = rows.iter().map(|r| r.label as u32).collect::<Vec<_>>();

        let x = DenseMatrix::from_2d_vec(&x)?;
        let params = RandomForestClassifierParameters {
            criterion: Default::default(),
            max_depth: config.max_depth.map(|d| d as u16),
            min_samples_leaf: 1,
            min_samples_split: config.min_samples_split,
            n_trees: config.n_trees as u16,
            m: None,
            keep_samples: false,
            seed: 42,
        };

        let model = RandomForestClassifier::fit(&x, &y, params)?;
        Ok(Self { model })
    }

    pub fn predict_labels(&self, rows: &[FeatureRow]) -> Result<Vec<u8>, Failed> {
        let x = rows.iter().map(|r| r.as_vec()).collect::<Vec<_>>();
        let x = DenseMatrix::from_2d_vec(&x)?;
        let pred = self.model.predict(&x)?;
        Ok(pred.into_iter().map(|v| v as u8).collect())
    }

    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string(self).context("serialize random forest model")?;
        fs::write(path.as_ref(), json)
            .with_context(|| format!("write model file '{}'", path.as_ref().display()))?;
        Ok(())
    }

    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = fs::read_to_string(path.as_ref())
            .with_context(|| format!("read model file '{}'", path.as_ref().display()))?;
        let model = serde_json::from_str(&data).context("deserialize random forest model")?;
        Ok(model)
    }
}
