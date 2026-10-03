mod coaxial;
mod faces;
mod offset;

use super::containment::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::operands::{full_cylinder, full_sphere};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use offset::offset_sphere_cylinder_boolean;

pub(crate) fn sphere_cylinder_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (sphere, cylinder, sphere_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Sphere { .. })
    ) {
        (full_sphere(a)?, full_cylinder(b)?, true)
    } else {
        (full_sphere(b)?, full_cylinder(a)?, false)
    };
    let clearance = 4.0
        * sphere
            .brep
            .accuracy
            .geometric
            .max(cylinder.brep.accuracy.geometric);
    let local = cylinder.frame.local(sphere.frame.origin);
    let radial = local[0].hypot(local[1]);
    let sphere_margin = (cylinder.radius - radial - sphere.radius)
        .min(local[2] - sphere.radius)
        .min(cylinder.height - local[2] - sphere.radius);
    let axial_extent = local[2].abs().max((cylinder.height - local[2]).abs());
    let cylinder_extent = (radial + cylinder.radius).hypot(axial_extent);
    let cylinder_margin = sphere.radius - cylinder_extent;
    let sphere_inside_cylinder = sphere_margin > clearance;
    let cylinder_inside_sphere = cylinder_margin > clearance;
    if sphere_inside_cylinder || cylinder_inside_sphere {
        let b_inside_a = if sphere_is_a {
            cylinder_inside_sphere
        } else {
            sphere_inside_cylinder
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if sphere_margin.abs() <= clearance || cylinder_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cylinder containment boundary is below geometric resolution".into(),
        ));
    }
    offset_sphere_cylinder_boolean(&sphere, &cylinder, sphere_is_a, operation, id, clearance)
}
