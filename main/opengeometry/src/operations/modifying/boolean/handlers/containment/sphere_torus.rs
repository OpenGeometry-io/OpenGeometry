use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::operands::{full_sphere, full_torus};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn sphere_torus_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, torus, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_torus(b)?, true)
    } else {
        (full_sphere(b)?, full_torus(a)?, false)
    };
    let accuracy = sphere
        .brep
        .accuracy
        .geometric
        .max(torus.brep.accuracy.geometric);
    let center = torus.frame.local(sphere.frame.origin);
    let radial = center[0].hypot(center[1]);
    let centerline_minimum = (radial - torus.major_radius).hypot(center[2]);
    let centerline_maximum = (radial + torus.major_radius).hypot(center[2]);
    let sphere_inside_torus = centerline_minimum + sphere.radius < torus.minor_radius - accuracy;
    let torus_inside_sphere = centerline_maximum + torus.minor_radius < sphere.radius - accuracy;
    if sphere_inside_torus || torus_inside_sphere {
        let b_inside_a = if sphere_is_a {
            torus_inside_sphere
        } else {
            sphere_inside_torus
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if (centerline_minimum + sphere.radius - torus.minor_radius).abs() <= accuracy
        || (centerline_maximum + torus.minor_radius - sphere.radius).abs() <= accuracy
    {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/torus containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["sphere".into(), "intersecting ring torus".into()],
    })
}
