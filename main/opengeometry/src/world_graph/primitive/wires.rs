use super::kind::Primitive;
use super::sizes::{check_arc, parameter, required_size, wrong_body_type};
use crate::brep::{Accuracy, BrepEnvelope, CurveGeometry, GROUND};
use crate::math::{norm, sub, Point3};
use crate::primitives::{self, ProfileEdge, ProfileSegment};
use crate::world_graph::error::{ErrorCode, GraphError};
use std::f64::consts::TAU;

pub(super) fn build_wire(
    shape_id: String,
    primitive: Primitive,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let result = match primitive {
        Primitive::Rectangle { width, breadth } => {
            rectangle_body(shape_id, [width, breadth], accuracy)?
        }
        Primitive::Circle { radius } => {
            required_size(radius, accuracy.geometric, "radius")?;
            arc_body(shape_id, radius, [0.0, TAU], accuracy)?
        }
        Primitive::Arc {
            radius,
            start_angle,
            sweep_angle,
        } => {
            check_arc("arc", radius, [start_angle, sweep_angle], accuracy)?;
            arc_body(shape_id, radius, [start_angle, sweep_angle], accuracy)?
        }
        Primitive::Polyline { points, closed } => {
            check_polyline(&points, closed, accuracy)?;
            let (brep, keys) = primitives::polyline_with_keys(shape_id, &points, closed, accuracy)?;
            (brep, keys)
        }
        Primitive::Profile {
            start,
            segments,
            closed,
        } => profile_body(shape_id, start, &segments, closed, accuracy)?,
        _ => return Err(wrong_body_type("wire")),
    };
    Ok(result)
}

fn rectangle_body(
    shape_id: String,
    [width, breadth]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(width, accuracy.geometric, "width")?;
    required_size(breadth, accuracy.geometric, "breadth")?;
    Ok(primitives::rectangle_with_keys(
        shape_id, GROUND, width, breadth, accuracy,
    )?)
}

fn arc_body(
    shape_id: String,
    radius: f64,
    [start_angle, sweep_angle]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let curve = CurveGeometry::Circle {
        frame: GROUND,
        radius,
    };
    Ok((
        primitives::arc_wire(shape_id, curve, start_angle, sweep_angle, accuracy)?,
        vec!["edge-0".into()],
    ))
}

fn check_polyline(points: &[Point3], closed: bool, accuracy: Accuracy) -> Result<(), GraphError> {
    if points.len() < if closed { 3 } else { 2 } {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "polyline has too few points",
        ));
    }
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "polyline points must be finite",
        ));
    }
    for index in 0..if closed {
        points.len()
    } else {
        points.len() - 1
    } {
        let next = (index + 1) % points.len();
        if norm(sub(points[next], points[index])) <= 4.0 * accuracy.geometric {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "polyline has a zero-length segment",
            ));
        }
    }
    Ok(())
}

fn profile_body(
    shape_id: String,
    start: [f64; 2],
    segments: &[ProfileSegment],
    closed: bool,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let edges = primitives::profile_segment_edges(start, segments, closed);
    check_profile(start, segments, &edges, closed, accuracy.geometric)?;
    Ok(primitives::profile_wire_with_keys(
        shape_id, start, segments, closed, accuracy,
    )?)
}

fn check_profile(
    start: [f64; 2],
    segments: &[ProfileSegment],
    edges: &[ProfileEdge],
    closed: bool,
    g: f64,
) -> Result<(), GraphError> {
    if segments.len() < if closed { 2 } else { 1 } {
        return Err(parameter("profile has too few segments"));
    }
    let values = segments.iter().flat_map(|segment| {
        segment
            .to
            .into_iter()
            .chain(segment.arc.into_iter().flat_map(|arc| arc.center))
    });
    if !start.into_iter().chain(values).all(f64::is_finite) {
        return Err(parameter("profile points must be finite"));
    }
    let last = segments[segments.len() - 1].to;
    if closed != ((last[0] - start[0]).hypot(last[1] - start[1]) <= 4.0 * g) {
        return Err(parameter(if closed {
            "closed profile must end at its start"
        } else {
            "open profile must not end at its start"
        }));
    }
    for (index, (edge, segment)) in edges.iter().zip(segments).enumerate() {
        let to = if closed && index + 1 == segments.len() {
            start
        } else {
            segment.to
        };
        check_profile_edge(edge, to, g)?;
    }
    if primitives::profile_self_intersects(edges, closed, g) {
        return Err(parameter("profile self-intersects"));
    }
    if closed && primitives::profile_area_twice(edges).abs() <= 16.0 * g * g {
        return Err(parameter("profile area is below geometric resolution"));
    }
    Ok(())
}

fn check_profile_edge(edge: &ProfileEdge, to: [f64; 2], g: f64) -> Result<(), GraphError> {
    let (from, _) = edge.endpoints();
    if (to[0] - from[0]).hypot(to[1] - from[1]) <= 4.0 * g {
        return Err(parameter("profile has a zero-length segment"));
    }
    if let ProfileEdge::Arc {
        center,
        radius,
        sweep_angle,
        ..
    } = *edge
    {
        if radius <= 4.0 * g
            || ((to[0] - center[0]).hypot(to[1] - center[1]) - radius).abs() > g / 4.0
        {
            return Err(parameter("profile arc does not end on its circle"));
        }
        if sweep_angle.abs() >= TAU || radius * sweep_angle.abs() <= 4.0 * g {
            return Err(parameter("profile arc is below geometric resolution"));
        }
    }
    Ok(())
}
