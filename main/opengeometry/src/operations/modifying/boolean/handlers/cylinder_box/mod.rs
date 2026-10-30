mod oblique;
mod perpendicular;
mod span;
#[cfg(test)]
mod tests;

use super::containment::analytic_containment_boolean;
use crate::brep::{Accuracy, BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::operands::{
    full_box, full_cylinder, BoxInput, CylinderInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use oblique::cylinder_half_space_boolean;
use perpendicular::perpendicular_cylinder_box_boolean;
use span::cylinder_spans;

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
    let mut cylinder_margin = f64::INFINITY;
    for (axis, [lo, hi]) in cylinder_spans(&box_, &cylinder).into_iter().enumerate() {
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
    perpendicular_or_half_space(&box_, &cylinder, cylinder_is_a, operation, id, accuracy)
}

fn perpendicular_or_half_space(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    match perpendicular_cylinder_box_boolean(
        box_,
        cylinder,
        cylinder_is_a,
        operation,
        id.clone(),
        accuracy,
    ) {
        Err(error @ GeometryError::CoverageGap { .. }) => {
            match cylinder_half_space_boolean(
                box_,
                cylinder,
                cylinder_is_a,
                operation,
                id,
                accuracy,
            ) {
                result @ (Ok(_) | Err(GeometryError::UnresolvedIntersection(_))) => result,
                Err(_) => Err(error),
            }
        }
        result => result,
    }
}
