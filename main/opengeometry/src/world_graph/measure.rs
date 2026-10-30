use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use crate::query::{validity, Validity};
use crate::tessellation::{measure, Measure};

impl WorldGraph {
    pub fn measure(&self, og_id: &str, deflection: Option<f64>) -> Result<Measure, GraphError> {
        let brep = &self.shape(og_id)?.brep;
        let deflection = deflection.unwrap_or(self.accuracy().tessellation);
        if !deflection.is_finite() || deflection <= 0.0 {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "deflection must be a positive finite number",
            ));
        }
        Ok(measure(brep, deflection)?)
    }

    pub fn validity(&self, og_id: &str) -> Result<Validity, GraphError> {
        Ok(validity(&self.shape(og_id)?.brep))
    }
}
