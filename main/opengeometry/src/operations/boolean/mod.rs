use serde::{Serialize, Deserialize};
use crate::brep::Brep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BooleanOperation {
    Union,
    Intersection,
    Difference,
    SymmetricDifference,
}

pub trait BooleanEngine {
    fn execute(&self, op: BooleanOperation, a: &Brep, b: &Brep) -> Result<Brep, String>;
}

pub mod simple_engine;
// pub mod truck_engine;  // Disabled due to compilation issues
pub mod validation;
pub mod wasm_bindings;