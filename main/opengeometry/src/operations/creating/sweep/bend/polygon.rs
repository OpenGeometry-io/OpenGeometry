use super::cone::{cone_ruling, Ruling};
use super::frame::BendFrame;
use crate::brep::{
    boundary, plane_boundary, unit, uv_line, Accuracy, Builder, CurveGeometry, Frame3,
    GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{add, dot, scale, sub, Interval, Point3};
use crate::operations::OperationError;

pub(crate) struct PolygonBend<'a> {
    pub(crate) frame: BendFrame,
    pub(crate) segment: usize,
    pub(crate) start: &'a [u32],
    pub(crate) end: &'a [u32],
    pub(crate) start_edges: &'a [u32],
    pub(crate) end_edges: &'a [u32],
    pub(crate) arcs: &'a [u32],
}

#[derive(Clone, Copy)]
struct LoopEnd {
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
}

pub(crate) fn add_bend_arcs(
    builder: &mut Builder,
    frame: BendFrame,
    start: &[u32],
) -> Result<Vec<u32>, OperationError> {
    let mut arcs = Vec::with_capacity(start.len());
    for &vertex in start {
        let [radius, height] = frame.section(position(builder, vertex));
        let circle = Frame3::from_axis(
            add(frame.centre, scale(frame.axis, height)),
            frame.axis,
            frame.radial,
        )?;
        arcs.push(builder.edge(
            CurveGeometry::Circle {
                frame: circle,
                radius,
            },
            Interval::new(0.0, frame.angle).map_err(GeometryError::from)?,
            false,
        ));
    }
    Ok(arcs)
}

pub(crate) fn add_polygon_bend(
    builder: &mut Builder,
    bend: &PolygonBend,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let count = bend.start.len();
    for edge in 0..count {
        let next = (edge + 1) % count;
        let from = bend.frame.section(position(builder, bend.start[edge]));
        let to = bend.frame.section(position(builder, bend.start[next]));
        let key = format!("seg-{}:side-0-{edge}", bend.segment);
        let ends = loop_ends(bend, edge, next);
        if (from[1] - to[1]).abs() <= accuracy.geometric {
            let reach = from[0].max(to[0]) + accuracy.geometric;
            add_plane_side(builder, bend, &key, ends, to[0] > from[0], [from[1], reach])?;
            continue;
        }
        let ruling = if (from[0] - to[0]).abs() <= accuracy.geometric {
            cylinder_ruling(bend.frame, from[0])?
        } else {
            cone_ruling(bend.frame, from, to)?
        };
        add_ruled_side(builder, bend, &key, ends, ruling, from[1] > to[1])?;
    }
    Ok(())
}

fn loop_ends(bend: &PolygonBend, edge: usize, next: usize) -> [LoopEnd; 4] {
    let end = |edge, from, to, sense| LoopEnd {
        edge,
        from,
        to,
        sense,
    };
    [
        end(
            bend.start_edges[edge],
            bend.start[edge],
            bend.start[next],
            Orientation::Forward,
        ),
        end(
            bend.arcs[next],
            bend.start[next],
            bend.end[next],
            Orientation::Forward,
        ),
        end(
            bend.end_edges[edge],
            bend.end[next],
            bend.end[edge],
            Orientation::Reverse,
        ),
        end(
            bend.arcs[edge],
            bend.end[edge],
            bend.start[edge],
            Orientation::Reverse,
        ),
    ]
}

fn cylinder_ruling(frame: BendFrame, radius: f64) -> Result<Ruling, OperationError> {
    let axis = Frame3::from_axis(frame.centre, frame.axis, frame.radial)?;
    Ok(Ruling {
        frame: axis,
        surface: SurfaceGeometry::Cylinder {
            frame: axis,
            radius,
        },
        turn: 1.0,
    })
}

fn add_ruled_side(
    builder: &mut Builder,
    bend: &PolygonBend,
    key: &str,
    ends: [LoopEnd; 4],
    ruling: Ruling,
    outer: bool,
) -> Result<(), OperationError> {
    let axis = ruling.frame;
    let [start, _, end, _] = ends;
    let [start_k, start_next, end_k, end_next] =
        [start.from, start.to, end.to, end.from].map(|vertex| position(builder, vertex));
    let along = |point: Point3| dot(sub(point, axis.origin), axis.z);
    let rate =
        |from: Point3, to: Point3| unit(sub(to, from)).map(|direction| dot(direction, axis.z));
    let end_u = ruling.turn * bend.frame.angle;
    let pcurves = [
        uv_line([0.0, along(start_k)], [0.0, rate(start_k, start_next)?]),
        uv_line([0.0, along(start_next)], [ruling.turn, 0.0]),
        uv_line([end_u, along(end_k)], [0.0, rate(end_k, end_next)?]),
        uv_line([0.0, along(start_k)], [ruling.turn, 0.0]),
    ];
    let heights = [start_k, start_next, end_k, end_next].map(along);
    let bounds = [
        [end_u.min(0.0), end_u.max(0.0)],
        [
            heights.iter().copied().fold(f64::INFINITY, f64::min),
            heights.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ],
    ];
    let uses = ends
        .into_iter()
        .zip(pcurves)
        .map(|(end, pcurve)| boundary(end.edge, end.from, end.to, end.sense, pcurve))
        .collect::<Vec<Use>>();
    let face = builder.brep.topology.faces.len();
    builder.face(key, ruling.surface, bounds, uses)?;
    if !outer {
        builder.brep.topology.faces[face].sense = Orientation::Reverse;
    }
    Ok(())
}

fn add_plane_side(
    builder: &mut Builder,
    bend: &PolygonBend,
    key: &str,
    ends: [LoopEnd; 4],
    upward: bool,
    [height, reach]: [f64; 2],
) -> Result<(), OperationError> {
    let frame = bend.frame;
    let normal = if upward {
        frame.axis
    } else {
        scale(frame.axis, -1.0)
    };
    let plane = Frame3::from_axis(
        add(frame.centre, scale(frame.axis, height)),
        normal,
        frame.radial,
    )?;
    let uses = ends
        .into_iter()
        .map(|end| plane_boundary(builder, plane, end.edge, end.from, end.to, end.sense))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face(
        key,
        SurfaceGeometry::Plane { frame: plane },
        [[-reach, reach]; 2],
        uses,
    )?;
    Ok(())
}

fn position(builder: &Builder, vertex: u32) -> Point3 {
    builder.brep.topology.vertices[vertex as usize].position
}
