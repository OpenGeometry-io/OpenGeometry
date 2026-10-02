use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError};
use crate::math::{cross, dot, norm};
use crate::operations::modifying::boolean::handlers::coincident::coincident_boolean;
use crate::operations::modifying::boolean::operands::full_torus;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn torus_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_torus(a)?;
    let b = full_torus(b)?;
    let accuracy = a.brep.accuracy.geometric.max(b.brep.accuracy.geometric);
    let axes_match = norm(cross(a.frame.z, b.frame.z)) <= 1e-12 && dot(a.frame.z, b.frame.z) > 0.0;
    let displacement = a.frame.local(b.frame.origin);
    let radial_offset = displacement[0].hypot(displacement[1]);
    if !axes_match || radial_offset > accuracy {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial ring torus".into()],
        });
    }
    let centerline_distance = (a.major_radius - b.major_radius).hypot(displacement[2]);
    let a_contains_b = centerline_distance + b.minor_radius < a.minor_radius - accuracy;
    let b_contains_a = centerline_distance + a.minor_radius < b.minor_radius - accuracy;
    let same =
        centerline_distance <= accuracy && (a.minor_radius - b.minor_radius).abs() <= accuracy;
    if same {
        return coincident_boolean(a.brep, b.brep, operation, id);
    }
    if a_contains_b || b_contains_a {
        return analytic_containment_boolean(a.brep, b.brep, a_contains_b, operation, id);
    }
    if (centerline_distance + b.minor_radius - a.minor_radius).abs() <= accuracy
        || (centerline_distance + a.minor_radius - b.minor_radius).abs() <= accuracy
    {
        return Err(GeometryError::UnresolvedIntersection(
            "torus containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting ring torus".into()],
    })
}
