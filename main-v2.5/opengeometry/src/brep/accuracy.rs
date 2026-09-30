use super::error::GeometryError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accuracy {
    pub geometric: f64,
    pub intersection: f64,
    pub tessellation: f64,
    pub exchange: f64,
}
impl Accuracy {
    pub const STANDARD: Accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };

    pub(crate) fn validate(self) -> Result<(), GeometryError> {
        for value in [
            self.geometric,
            self.intersection,
            self.tessellation,
            self.exchange,
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "accuracy budgets must be positive and finite".into(),
                ));
            }
        }
        if self.intersection > self.geometric {
            return Err(GeometryError::InvalidGeometry(
                "intersection budget exceeds geometric tolerance".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn combined(a: Accuracy, b: Accuracy) -> Accuracy {
        Accuracy {
            geometric: a.geometric.max(b.geometric),
            intersection: a.intersection.max(b.intersection),
            tessellation: a.tessellation.max(b.tessellation),
            exchange: a.exchange.max(b.exchange),
        }
    }
}
