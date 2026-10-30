mod faces;
mod levels;
mod regions;

use crate::brep::{Accuracy, BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::handlers::disjoint::separate_boolean;
use crate::operations::modifying::boolean::handlers::vertical_arc_extrusion::{finish, Facets};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use faces::{add_faces, Operands};
use levels::{common_levels, gap};
use regions::{exposed_caps, layer_regions, split_at_shared_points};

pub(crate) fn prismatic_curved_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    match prismatic_boolean(a, b, operation, id) {
        Ok(result) => Ok(Some(result)),
        Err(GeometryError::CoverageGap { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn prismatic_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let curved = [a, b].iter().any(|body| {
        body.geometry
            .surfaces
            .iter()
            .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
    });
    if !curved || a.solids.is_empty() || b.solids.is_empty() {
        return Err(gap());
    }
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    let levels = common_levels(a, b, accuracy.geometric)?;
    let mut layers = layer_regions([a, b], &levels, operation, accuracy)?;
    if layers.iter().all(Vec::is_empty) {
        return empty_result(a, b, operation, id);
    }
    let mut caps = exposed_caps(&layers, accuracy.intersection)?;
    split_at_shared_points(&mut layers, &mut caps, accuracy.intersection);
    let operands = Operands {
        a,
        b,
        operation,
        base: levels.base,
        accuracy,
    };
    let mut facets = Facets::new(id, accuracy, levels.base)?;
    add_faces(&mut facets, &operands, &layers, &caps, &levels.levels)?;
    finish(facets, a, &[b], operation)
}

fn empty_result(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let mut result = separate_boolean(a, b, BooleanOp::Intersection, id, Vec::new())?;
    result.report.operation = operation;
    Ok(result)
}
