use super::rectilinear::boolean_grid;
use crate::brep::{BrepEnvelope, GeometryError};
use crate::operations::modifying::boolean::operands::full_box;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub fn boolean_boxes(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_box(a)?;
    let b = full_box(b)?;
    boolean_grid(a, b, operation, id, true)
}
