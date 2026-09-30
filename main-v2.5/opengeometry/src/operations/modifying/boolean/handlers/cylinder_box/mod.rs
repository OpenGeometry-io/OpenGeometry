mod perpendicular;

use super::containment::analytic_containment_boolean;
use crate::brep::{Accuracy, BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::dot;
use crate::operations::modifying::boolean::operands::{full_box, full_cylinder};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use perpendicular::perpendicular_cylinder_box_boolean;

pub(crate) fn cylinder_box_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (cylinder, box_, cylinder_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    ) {
        (full_cylinder(a)?, full_box(b)?, true)
    } else {
        (full_cylinder(b)?, full_box(a)?, false)
    };
    let clearance = 4.0
        * cylinder
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let origin = box_.frame.local(cylinder.frame.origin);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let mut cylinder_margin = f64::INFINITY;
    for axis in 0..3 {
        let axial_rate = dot(cylinder.frame.z, box_axes[axis]);
        let radial_extent = cylinder.radius * (1.0 - axial_rate * axial_rate).max(0.0).sqrt();
        let other = origin[axis] + axial_rate * cylinder.height;
        let lo = origin[axis].min(other) - radial_extent;
        let hi = origin[axis].max(other) + radial_extent;
        cylinder_margin = cylinder_margin.min(lo).min(box_.size[axis] - hi);
    }
    let mut box_margin = f64::INFINITY;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        let local = cylinder.frame.local(point);
        box_margin = box_margin
            .min(cylinder.radius - local[0].hypot(local[1]))
            .min(local[2])
            .min(cylinder.height - local[2]);
    }
    let cylinder_inside_box = cylinder_margin > clearance;
    let box_inside_cylinder = box_margin > clearance;
    if cylinder_inside_box || box_inside_cylinder {
        let b_inside_a = if cylinder_is_a {
            box_inside_cylinder
        } else {
            cylinder_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if cylinder_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    let accuracy = Accuracy::combined(cylinder.brep.accuracy, box_.brep.accuracy);
    perpendicular_cylinder_box_boolean(&box_, &cylinder, cylinder_is_a, operation, id, accuracy)
}
