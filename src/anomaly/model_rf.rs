use smartcore::ensemble::random_forest_classifier::{
    RandomForestClassifier, RandomForestClassifierParameters,
};
use smartcore::error::Failed;
use smartcore::linalg::basic::matrix::DenseMatrix;

use super::features::FeatureRow;

#[derive(Debug, Clone)]
pub struct RandomForestConfig {
    pub n_trees: usize,
    pub max_depth: Option<usize>,
    pub min_samples_split: usize,
}

impl Default for RandomForestConfig {
    fn default() -> Self {
        Self {
            n_trees: 200,
            max_depth: Some(12),
            min_samples_split: 4,
        }
    }
}

#[derive(Debug)]
pub struct RandomForestModel {
    pub config: RandomForestConfig,
    model: RandomForestClassifier<f64, u32, DenseMatrix<f64>, Vec<u32>>,
}

impl RandomForestModel {
    pub fn train(rows: &[FeatureRow], config: RandomForestConfig) -> Result<Self, Failed> {
        let x = rows.iter().map(|r| r.as_vec()).collect::<Vec<_>>();
        let y = rows.iter().map(|r| r.label as u32).collect::<Vec<_>>();

        let x = DenseMatrix::from_2d_vec(&x)?;
        let mut params = RandomForestClassifierParameters::default();
        params.n_trees = config.n_trees as u16;
        params.min_samples_split = config.min_samples_split;
        params.max_depth = config.max_depth.map(|d| d as u16);

        let model = RandomForestClassifier::fit(&x, &y, params)?;

        Ok(Self { config, model })
    }

    pub fn predict_label(&self, row: &FeatureRow) -> Result<u8, Failed> {
        let row = vec![row.as_vec()];
        let x = DenseMatrix::from_2d_vec(&row)?;
        let pred = self.model.predict(&x)?;
        Ok(pred.first().copied().unwrap_or(0) as u8)
    }

    pub fn predict_labels(&self, rows: &[FeatureRow]) -> Result<Vec<u8>, Failed> {
        let x = rows.iter().map(|r| r.as_vec()).collect::<Vec<_>>();
        let x = DenseMatrix::from_2d_vec(&x)?;
        let pred = self.model.predict(&x)?;

        Ok(pred.into_iter().map(|v| v as u8).collect())
    }
}
