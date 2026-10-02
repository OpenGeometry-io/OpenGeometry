mod grid;

pub(super) use grid::boolean_grid;

use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::operands::{coverage, rectilinear_input};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn boolean_rectilinear(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let axes = match b.geometry.surfaces.first() {
        Some(SurfaceGeometry::Plane { frame }) => *frame,
        _ => return Err(coverage()),
    };
    let a = rectilinear_input(a, axes)?;
    let b = rectilinear_input(b, axes)?;
    boolean_grid(a, b, operation, id, false)
}
