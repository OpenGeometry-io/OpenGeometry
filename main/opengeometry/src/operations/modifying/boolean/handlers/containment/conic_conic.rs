use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError};
use crate::math::{cross, dot, norm, scale, sub, Interval};
use crate::operations::modifying::boolean::handlers::coincident::coincident_boolean;
use crate::operations::modifying::boolean::operands::{
    conic_radius_at, full_conic_section, ConicSectionInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn conic_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let a = full_conic_section(a)?;
    let b = full_conic_section(b)?;
    let accuracy = a.brep.accuracy.geometric.max(b.brep.accuracy.geometric);
    if norm(cross(a.frame.z, b.frame.z)) > 1.0e-12 || dot(a.frame.z, b.frame.z) <= 0.0 {
        return Err(GeometryError::CoverageGap {
            families: [
                "cone or frustum".into(),
                "noncoaxial cone or frustum".into(),
            ],
        });
    }
    let displacement = sub(b.frame.origin, a.frame.origin);
    let axial_offset = dot(displacement, a.frame.z);
    let radial_offset = norm(sub(displacement, scale(a.frame.z, axial_offset)));
    if radial_offset > accuracy {
        return Err(GeometryError::CoverageGap {
            families: [
                "cone or frustum".into(),
                "noncoaxial cone or frustum".into(),
            ],
        });
    }

    let b_in_a_range = Interval::new(
        axial_offset + b.axial_range.lo,
        axial_offset + b.axial_range.hi,
    )?;
    let a_in_b_range = Interval::new(
        a.axial_range.lo - axial_offset,
        a.axial_range.hi - axial_offset,
    )?;
    let containment_margins =
        |inner: &ConicSectionInput<'_>, outer: &ConicSectionInput<'_>, inner_in_outer: Interval| {
            [
                inner_in_outer.lo - outer.axial_range.lo,
                outer.axial_range.hi - inner_in_outer.hi,
                conic_radius_at(outer, inner_in_outer.lo) - inner.lower_radius,
                conic_radius_at(outer, inner_in_outer.hi) - inner.upper_radius,
            ]
        };
    let b_in_a = containment_margins(&b, &a, b_in_a_range);
    let a_in_b = containment_margins(&a, &b, a_in_b_range);
    let strictly_inside = |margins: [f64; 4]| margins.into_iter().all(|margin| margin > accuracy);
    let near_inside = |margins: [f64; 4]| {
        margins.into_iter().all(|margin| margin >= -accuracy)
            && margins.into_iter().any(|margin| margin.abs() <= accuracy)
    };
    let b_inside_a = strictly_inside(b_in_a);
    let a_inside_b = strictly_inside(a_in_b);
    if b_inside_a || a_inside_b {
        return analytic_containment_boolean(a.brep, b.brep, b_inside_a, operation, id);
    }

    let same = a.brep.topology.faces.len() == b.brep.topology.faces.len()
        && (b_in_a_range.lo - a.axial_range.lo).abs() <= accuracy
        && (b_in_a_range.hi - a.axial_range.hi).abs() <= accuracy
        && (b.lower_radius - a.lower_radius).abs() <= accuracy
        && (b.upper_radius - a.upper_radius).abs() <= accuracy;
    if same {
        return coincident_boolean(a.brep, b.brep, operation, id);
    }
    if near_inside(b_in_a) || near_inside(a_in_b) {
        return Err(GeometryError::UnresolvedIntersection(
            "cone/frustum containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: [
            "cone or frustum".into(),
            "intersecting cone or frustum".into(),
        ],
    })
}
