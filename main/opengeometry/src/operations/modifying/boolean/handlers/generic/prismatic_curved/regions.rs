use super::levels::{tangent, Levels};
use crate::brep::{Accuracy, BrepEnvelope, GeometryError};
use crate::geom2d::{boolean_curved_regions, CurveEdge2, CurveRegion2, PlanarBooleanOp, Pt2};
use crate::operations::modifying::boolean::handlers::vertical_arc_extrusion::{
    append_points, sectional_regions, split_regions,
};
use crate::operations::modifying::boolean::types::BooleanOp;

pub(super) struct Caps {
    pub(super) up: Vec<CurveRegion2>,
    pub(super) down: Vec<CurveRegion2>,
}

struct Circle {
    center: Pt2,
    radius: f64,
}

pub(super) fn layer_regions(
    operands: [&BrepEnvelope; 2],
    levels: &Levels,
    operation: BooleanOp,
    accuracy: Accuracy,
) -> Result<Vec<Vec<CurveRegion2>>, GeometryError> {
    let planar = match operation {
        BooleanOp::Union => PlanarBooleanOp::Union,
        BooleanOp::Intersection => PlanarBooleanOp::Intersection,
        BooleanOp::Subtraction => PlanarBooleanOp::Subtraction,
    };
    levels
        .levels
        .windows(2)
        .map(|pair| {
            let middle = (pair[0] + pair[1]) / 2.0;
            let [a, b] = [0, 1].map(|index| {
                let (lo, hi) = levels.spans[index];
                if middle > lo && middle < hi {
                    sectional_regions(operands[index], levels.base, middle, accuracy.intersection)
                } else {
                    Ok(Vec::new())
                }
            });
            let (a, b) = (a?, b?);
            check_tangency(&a, &b, 4.0 * accuracy.geometric)?;
            boolean_curved_regions(&a, &b, planar, accuracy.intersection)
        })
        .collect()
}

pub(super) fn exposed_caps(
    layers: &[Vec<CurveRegion2>],
    tolerance: f64,
) -> Result<Vec<Caps>, GeometryError> {
    let empty = Vec::new();
    (0..=layers.len())
        .map(|index| {
            let below = index.checked_sub(1).map_or(&empty, |at| &layers[at]);
            let above = layers.get(index).unwrap_or(&empty);
            Ok(Caps {
                up: boolean_curved_regions(below, above, PlanarBooleanOp::Subtraction, tolerance)?,
                down: boolean_curved_regions(
                    above,
                    below,
                    PlanarBooleanOp::Subtraction,
                    tolerance,
                )?,
            })
        })
        .collect()
}

pub(super) fn split_at_shared_points(
    layers: &mut [Vec<CurveRegion2>],
    caps: &mut [Caps],
    tolerance: f64,
) {
    let mut points = Vec::new();
    for regions in layers.iter() {
        append_points(regions, &mut points);
    }
    for cap in caps.iter() {
        append_points(&cap.up, &mut points);
        append_points(&cap.down, &mut points);
    }
    for regions in layers.iter_mut() {
        split_regions(regions, &points, tolerance);
    }
    for cap in caps.iter_mut() {
        split_regions(&mut cap.up, &points, tolerance);
        split_regions(&mut cap.down, &points, tolerance);
    }
}

fn check_tangency(a: &[CurveRegion2], b: &[CurveRegion2], reach: f64) -> Result<(), GeometryError> {
    for first in edges(a) {
        for second in edges(b) {
            let touch = match (circle(first), circle(second)) {
                (Some(circle), None) => line_touch(&circle, second, reach)
                    .filter(|point| !own_joint(*point, &circle, second, b, reach)),
                (None, Some(circle)) => line_touch(&circle, first, reach)
                    .filter(|point| !own_joint(*point, &circle, first, a, reach)),
                (Some(left), Some(right)) => circle_touch(&left, &right, reach),
                (None, None) => None,
            };
            if touch.is_some_and(|point| {
                first.parameter(point, reach).is_some() && second.parameter(point, reach).is_some()
            }) {
                return Err(tangent());
            }
        }
    }
    Ok(())
}

fn edges(regions: &[CurveRegion2]) -> impl Iterator<Item = &CurveEdge2> {
    regions
        .iter()
        .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes))
        .flatten()
}

fn own_joint(
    point: Pt2,
    touched: &Circle,
    line: &CurveEdge2,
    regions: &[CurveRegion2],
    reach: f64,
) -> bool {
    let near = |left: Pt2, right: Pt2| (left.x - right.x).hypot(left.z - right.z) <= reach;
    let at_end = near(point, line.point(0.0)) || near(point, line.point(1.0));
    at_end
        && edges(regions).any(|edge| {
            circle(edge).is_some_and(|own| {
                near(own.center, touched.center) && (own.radius - touched.radius).abs() <= reach
            }) && (near(point, edge.point(0.0)) || near(point, edge.point(1.0)))
        })
}

fn circle(edge: &CurveEdge2) -> Option<Circle> {
    match *edge {
        CurveEdge2::Arc { center, radius, .. } => Some(Circle {
            center: Pt2::new(center[0], center[1]),
            radius,
        }),
        CurveEdge2::Line { .. } => None,
    }
}

fn line_touch(circle: &Circle, line: &CurveEdge2, reach: f64) -> Option<Pt2> {
    let CurveEdge2::Line { from, to } = *line else {
        return None;
    };
    let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dz);
    if length <= reach {
        return None;
    }
    let along = ((circle.center.x - from[0]) * dx + (circle.center.z - from[1]) * dz) / length;
    let foot = Pt2::new(from[0] + dx * along / length, from[1] + dz * along / length);
    let distance = (circle.center.x - foot.x).hypot(circle.center.z - foot.z);
    ((distance - circle.radius).abs() <= reach).then_some(foot)
}

fn circle_touch(left: &Circle, right: &Circle, reach: f64) -> Option<Pt2> {
    let (dx, dz) = (
        right.center.x - left.center.x,
        right.center.z - left.center.z,
    );
    let distance = dx.hypot(dz);
    if distance <= reach {
        return None;
    }
    let outward = if (distance - left.radius - right.radius).abs() <= reach {
        left.radius
    } else if (distance - (left.radius - right.radius).abs()).abs() <= reach {
        if left.radius >= right.radius {
            left.radius
        } else {
            -left.radius
        }
    } else {
        return None;
    };
    Some(Pt2::new(
        left.center.x + dx * outward / distance,
        left.center.z + dz * outward / distance,
    ))
}
