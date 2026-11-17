use crate::brep::GeometryError;
use std::ops::Mul;

#[derive(Clone, Copy)]
pub(super) struct Scale {
    times: f64,
    per: f64,
}

impl Scale {
    pub(super) fn between(model_unit: &str, unit: &str) -> Result<Self, GeometryError> {
        let per = millimetres_in(unit).ok_or_else(|| {
            GeometryError::InvalidGeometry("STEP length unit must be metre or millimetre".into())
        })?;
        let times = millimetres_in(model_unit).ok_or_else(|| {
            GeometryError::InvalidGeometry("model length unit must be metre or millimetre".into())
        })?;
        Ok(if times >= per {
            Self {
                times: times / per,
                per: 1.0,
            }
        } else {
            Self {
                times: 1.0,
                per: per / times,
            }
        })
    }

    pub(super) fn ratio(self) -> f64 {
        1.0 * self
    }
}

impl Mul<Scale> for f64 {
    type Output = f64;

    fn mul(self, scale: Scale) -> f64 {
        self * scale.times / scale.per
    }
}

impl Mul<Scale> for &f64 {
    type Output = f64;

    fn mul(self, scale: Scale) -> f64 {
        *self * scale
    }
}

fn millimetres_in(unit: &str) -> Option<f64> {
    match unit {
        "metre" => Some(1000.0),
        "millimetre" => Some(1.0),
        _ => None,
    }
}
