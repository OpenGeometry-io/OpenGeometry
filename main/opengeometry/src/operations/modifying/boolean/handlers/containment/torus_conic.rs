use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::{cross, norm};
use crate::operations::modifying::boolean::operands::{full_conic_section, full_torus};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn torus_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, conic, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, full_conic_section(b)?, true)
    } else {
        (full_torus(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    if norm(cross(torus.frame.z, conic.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial cone or frustum".into()],
        });
    }
    let center = conic.frame.local(torus.frame.origin);
    let radial = center[0].hypot(center[1]);
    let (sin_angle, cos_angle) = conic.semi_angle.sin_cos();
    let torus_margin = (center[2] - conic.axial_range.lo - torus.minor_radius)
        .min(conic.axial_range.hi - center[2] - torus.minor_radius)
        .min(
            center[2] * sin_angle - (radial + torus.major_radius) * cos_angle - torus.minor_radius,
        );
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting cone or frustum".into()],
    })
}
