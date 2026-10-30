use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::operands::{full_conic_section, full_sphere};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn sphere_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, conic, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_conic_section(b)?, true)
    } else {
        (full_sphere(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    let center = conic.frame.local(sphere.frame.origin);
    let radial = center[0].hypot(center[1]);
    let (sin_angle, cos_angle) = conic.semi_angle.sin_cos();
    let sphere_margin = (center[2] - conic.axial_range.lo - sphere.radius)
        .min(conic.axial_range.hi - center[2] - sphere.radius)
        .min(center[2] * sin_angle - radial * cos_angle - sphere.radius);
    let conic_extent = [
        (radial + conic.lower_radius).hypot(conic.axial_range.lo - center[2]),
        (radial + conic.upper_radius).hypot(conic.axial_range.hi - center[2]),
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    let conic_margin = sphere.radius - conic_extent;
    let sphere_inside_conic = sphere_margin > clearance;
    let conic_inside_sphere = conic_margin > clearance;
    if sphere_inside_conic || conic_inside_sphere {
        let b_inside_a = if sphere_is_a {
            conic_inside_sphere
        } else {
            sphere_inside_conic
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || conic_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["sphere".into(), "intersecting cone or frustum".into()],
    })
}
