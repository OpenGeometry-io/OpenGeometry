use super::profile::ProfileEdge;
use super::wire_points::line_use;
use crate::brep::{
    checked, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3, GeometryError, Orientation,
    WireUse, GROUND,
};
use crate::math::{Interval, Point3};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SegmentArc {
    pub center: [f64; 2],
    pub counter_clockwise: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileSegment {
    pub to: [f64; 2],
    pub arc: Option<SegmentArc>,
}

pub(crate) fn profile_segment_edges(
    start: [f64; 2],
    segments: &[ProfileSegment],
    closed: bool,
) -> Vec<ProfileEdge> {
    let mut from = start;
    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let to = if closed && index + 1 == segments.len() {
                start
            } else {
                segment.to
            };
            let edge = segment_edge(from, to, segment.arc);
            from = to;
            edge
        })
        .collect()
}

fn segment_edge(from: [f64; 2], to: [f64; 2], arc: Option<SegmentArc>) -> ProfileEdge {
    let Some(arc) = arc else {
        return ProfileEdge::Line { from, to };
    };
    let [cu, cv] = arc.center;
    let start_angle = (from[1] - cv).atan2(from[0] - cu);
    let travel = ((to[1] - cv).atan2(to[0] - cu) - start_angle).rem_euclid(TAU);
    ProfileEdge::Arc {
        center: arc.center,
        radius: (from[0] - cu).hypot(from[1] - cv),
        start_angle,
        sweep_angle: if arc.counter_clockwise {
            travel
        } else {
            travel - TAU
        },
    }
}

pub fn profile_wire_with_keys(
    id: String,
    start: [f64; 2],
    segments: &[ProfileSegment],
    closed: bool,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GeometryError> {
    accuracy.validate()?;
    let edges = profile_segment_edges(start, segments, closed);
    let mut builder = Builder::new(id, accuracy)?;
    let points = std::iter::once(start)
        .chain(segments.iter().map(|segment| segment.to))
        .take(segments.len() + usize::from(!closed))
        .map(ground_point)
        .collect::<Vec<_>>();
    let vertices = points
        .iter()
        .map(|&point| checked(point).map(|valid| builder.vertex(valid)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut uses = Vec::with_capacity(edges.len());
    for (index, edge) in edges.iter().enumerate() {
        let next = (index + 1) % points.len();
        let ends = [
            (vertices[index], points[index]),
            (vertices[next], points[next]),
        ];
        uses.push(match edge {
            ProfileEdge::Line { .. } => line_use(&mut builder, ends, accuracy.geometric)?,
            ProfileEdge::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => arc_use(
                &mut builder,
                (*center, *radius),
                [*start_angle, *sweep_angle],
                [ends[0].0, ends[1].0],
            )?,
        });
    }
    let keys = (0..uses.len())
        .map(|index| format!("seg-{index}"))
        .collect();
    Ok((builder.finish_wire(uses, closed)?, keys))
}

fn ground_point(point: [f64; 2]) -> Point3 {
    GROUND.point([point[0], point[1], 0.0])
}

fn arc_use(
    builder: &mut Builder,
    (center, radius): ([f64; 2], f64),
    [start_angle, sweep_angle]: [f64; 2],
    vertices: [u32; 2],
) -> Result<WireUse, GeometryError> {
    let end = start_angle + sweep_angle;
    let frame = Frame3 {
        origin: ground_point(center),
        ..GROUND
    };
    let edge = builder.edge(
        CurveGeometry::Circle { frame, radius },
        Interval::new(start_angle.min(end), start_angle.max(end))?,
        false,
    );
    Ok(WireUse {
        edge,
        from: vertices[0],
        to: vertices[1],
        sense: if sweep_angle > 0.0 {
            Orientation::Forward
        } else {
            Orientation::Reverse
        },
    })
}
