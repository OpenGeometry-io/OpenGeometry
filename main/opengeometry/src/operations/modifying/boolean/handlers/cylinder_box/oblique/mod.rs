mod chart;
mod ellipse;
mod faces;
mod ruled;

use super::span::cylinder_spans;
use crate::brep::{Accuracy, Builder, GeometryError};
use crate::math::dot;
use crate::operations::modifying::boolean::assembly::finish_analytic_result;
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use chart::{gap, half_space_chart};
use ellipse::ellipse_uses;
use faces::{add_half_space_faces, Sources};
use ruled::ruled_uses;

pub(super) fn cylinder_half_space_boolean(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let keeps_cylinder_side = operation == BooleanOp::Intersection
        || (operation == BooleanOp::Subtraction && cylinder_is_a);
    let aligned = [box_.frame.x, box_.frame.y, box_.frame.z]
        .iter()
        .any(|axis| 1.0 - dot(cylinder.frame.z, *axis).abs() <= 1.0e-12);
    if !keeps_cylinder_side || aligned {
        return Err(gap());
    }
    let clearance = 4.0 * accuracy.geometric;
    let (axis, high) = crossing_face(box_, cylinder, clearance)?;
    let chart = half_space_chart(box_, cylinder, [axis, usize::from(high)], operation)?;
    let sliver = clearance.max(1.0e-3 * cylinder.radius);
    let mut builder = Builder::new(id, accuracy)?;
    let uses = if cylinder.height * chart.c <= 0.25 * accuracy.geometric {
        ruled_uses(&mut builder, &chart, sliver)?
    } else {
        ellipse_uses(&mut builder, &chart, sliver)?
    };
    let sources = Sources {
        box_,
        cylinder,
        operation,
    };
    add_half_space_faces(&mut builder, &chart, uses, &sources)?;
    let out = builder.finish_solid()?;
    finish_analytic_result(
        out,
        cylinder.brep,
        box_.brep,
        cylinder_is_a,
        operation,
        false,
    )
}

fn crossing_face(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    clearance: f64,
) -> Result<(usize, bool), GeometryError> {
    let mut cut = None;
    for (axis, [lo, hi]) in cylinder_spans(box_, cylinder).into_iter().enumerate() {
        for (high, margin, beyond) in [
            (false, lo, hi),
            (true, box_.size[axis] - hi, box_.size[axis] - lo),
        ] {
            if margin.abs() <= clearance {
                return Err(gap());
            }
            if margin < 0.0 {
                if beyond <= clearance || cut.is_some() {
                    return Err(gap());
                }
                cut = Some((axis, high));
            }
        }
    }
    cut.ok_or_else(gap)
}
