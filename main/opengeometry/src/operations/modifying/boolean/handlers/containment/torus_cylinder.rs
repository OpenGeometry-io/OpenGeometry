use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::{cross, norm};
use crate::operations::modifying::boolean::operands::{full_cylinder, full_torus};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn torus_cylinder_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, cylinder, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, full_cylinder(b)?, true)
    } else {
        (full_torus(b)?, full_cylinder(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(cylinder.brep.accuracy.geometric);
    if norm(cross(torus.frame.z, cylinder.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["ring torus".into(), "noncoaxial cylinder".into()],
        });
    }
    let center = cylinder.frame.local(torus.frame.origin);
    let radial = center[0].hypot(center[1]);
    let torus_margin = (cylinder.radius - radial - torus.major_radius - torus.minor_radius)
        .min(center[2] - torus.minor_radius)
        .min(cylinder.height - center[2] - torus.minor_radius);
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cylinder containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "intersecting cylinder".into()],
    })
}
