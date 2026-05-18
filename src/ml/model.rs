use std::{fs, path::Path};

use serde::Deserialize;
use smartcore::{error::Failed, linalg::basic::matrix::DenseMatrix, xgboost::XGRegressor};

use super::FeatureRow;

#[derive(Debug, Deserialize)]
struct StoredXGBoostModel {
    model_type: String,
    threshold: f64,
    model: XGRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
}

#[derive(Debug)]
pub struct RuntimeLeakModel {
    threshold: f64,
    model: XGRegressor<f64, f64, DenseMatrix<f64>, Vec<f64>>,
}

impl RuntimeLeakModel {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let data = fs::read_to_string(path_ref)
            .map_err(|err| format!("read model file '{}': {}", path_ref.display(), err))?;

        let stored: StoredXGBoostModel = serde_json::from_str(&data)
            .map_err(|err| format!("deserialize model '{}': {}", path_ref.display(), err))?;

        if stored.model_type != "xgboost" {
            return Err(format!(
                "unsupported model_type '{}' in model '{}'; only 'xgboost' is allowed",
                stored.model_type,
                path_ref.display()
            ));
        }

        Ok(Self {
            threshold: stored.threshold,
            model: stored.model,
        })
    }

    pub fn predict_labels(&self, rows: &[FeatureRow]) -> Result<Vec<u8>, Failed> {
        let x = rows.iter().map(FeatureRow::as_vec).collect::<Vec<_>>();
        let x = DenseMatrix::from_2d_vec(&x)?;
        let pred = self.model.predict(&x)?;
        Ok(pred
            .into_iter()
            .map(|v| if v >= self.threshold { 1 } else { 0 })
            .collect())
    }
}
