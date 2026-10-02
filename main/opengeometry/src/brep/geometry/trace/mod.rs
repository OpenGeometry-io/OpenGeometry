mod enclosure;
mod evaluate;
#[cfg(test)]
mod tests;

use crate::brep::error::GeometryError;
use crate::brep::frame::UV;
use crate::math::Point3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SsiBudget {
    pub(crate) max_patch_pairs: usize,
    pub(crate) max_steps_per_branch: usize,
    max_newton_iterations: usize,
    pub(crate) max_subdivision_depth: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct IntersectionPoint {
    pub(crate) point: Point3,
    pub(crate) uv_a: UV,
    pub(crate) uv_b: UV,
    pub(super) tangent: Point3,
    pub support_error: f64,
}

impl Default for SsiBudget {
    fn default() -> Self {
        Self {
            max_patch_pairs: 200_000,
            max_steps_per_branch: 50_000,
            max_newton_iterations: 12,
            max_subdivision_depth: 48,
        }
    }
}
impl SsiBudget {
    pub(crate) fn validate(self) -> Result<(), GeometryError> {
        if self.max_patch_pairs == 0
            || self.max_steps_per_branch == 0
            || self.max_newton_iterations == 0
            || self.max_newton_iterations > 64
            || self.max_subdivision_depth == 0
            || self.max_subdivision_depth > 128
        {
            return Err(GeometryError::InvalidGeometry(
                "invalid SSI resource limits".into(),
            ));
        }
        Ok(())
    }
}
