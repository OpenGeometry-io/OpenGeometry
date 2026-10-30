use crate::brep::{BrepEnvelope, GeometryError};
use crate::intersection::intersect_breps;
use crate::math::{norm, sub};
use crate::operations::modifying::boolean::handlers::containment::{
    analytic_containment_boolean, interior_sample,
};
use crate::operations::modifying::boolean::handlers::disjoint::separate_boolean;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::query::{classify_point, PointClassification};

pub(crate) fn generic_non_intersecting_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let graph = match intersect_breps(a, b) {
        Ok(graph) => graph,
        Err(GeometryError::CoverageGap { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    if graph
        .pairs
        .iter()
        .any(|pair| pair.graph.coincident || !pair.graph.branches.is_empty())
    {
        return Ok(None);
    }
    let mut contacts = graph
        .pairs
        .into_iter()
        .flat_map(|pair| pair.graph.contacts)
        .collect::<Vec<_>>();
    let tolerance = a.accuracy.geometric.max(b.accuracy.geometric);
    let mut unique = Vec::new();
    for point in contacts.drain(..) {
        if unique
            .iter()
            .all(|existing| norm(sub(*existing, point)) > tolerance)
        {
            unique.push(point);
        }
    }

    let a_in_b = classify_point(b, interior_sample(a)?)?;
    let b_in_a = classify_point(a, interior_sample(b)?)?;
    if matches!(
        a_in_b,
        PointClassification::Unknown | PointClassification::Boundary
    ) || matches!(
        b_in_a,
        PointClassification::Unknown | PointClassification::Boundary
    ) {
        return Err(GeometryError::UnresolvedIntersection(
            "generic boolean containment classification is unresolved".into(),
        ));
    }
    let a_inside_b = a_in_b == PointClassification::Inside;
    let b_inside_a = b_in_a == PointClassification::Inside;
    if a_inside_b && b_inside_a {
        return Err(GeometryError::UnresolvedIntersection(
            "mutual containment without a coincident boundary is unresolved".into(),
        ));
    }
    if a_inside_b || b_inside_a {
        return analytic_containment_boolean(a, b, b_inside_a, operation, id).map(Some);
    }
    separate_boolean(a, b, operation, id, unique).map(Some)
}
