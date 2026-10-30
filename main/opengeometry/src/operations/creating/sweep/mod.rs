mod bend;
mod circle;
mod circle_ring;
mod overlap;
mod path;
#[cfg(test)]
mod tests;

pub(crate) use bend::Bend;
pub(crate) use path::SweepPath;

use super::extrude;
use super::profile::{check_lines, newell_normal, profile_frame, ProfileLoop};
use crate::brep::{
    padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3,
    GeometryError, Orientation, Shell, SurfaceGeometry, Use,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::{invalid, OperationError};
use bend::{add_bend_arcs, add_polygon_bend, BendFrame, PolygonBend};
use circle as sweep_circle;
use overlap::{check_segment_overlap, profile_reach};
use path::{checked_path, course, Course};

struct SweepTopology {
    vertices: Vec<Vec<u32>>,
    ring_edges: Vec<Vec<u32>>,
    longitudinal: Vec<Vec<u32>>,
}

pub(crate) fn build(
    id: String,
    profile: ProfileLoop,
    path: SweepPath,
    caps: bool,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, OperationError> {
    let path = checked_path(path, accuracy)?;
    let reach = profile_reach(&profile, path.points[0]);
    let course = course(path, reach, accuracy)?;
    if course.closed && !caps {
        return Err(invalid("a closed path has no ends to leave open"));
    }
    let frame = profile_frame(&profile, accuracy.geometric)?;
    check_lines(&profile, frame, accuracy.geometric)?;
    let points = &course.points;
    if dot(sub(points[0], frame.origin), frame.z).abs() > 4.0 * accuracy.geometric
        || norm(cross(frame.z, course.starts[0])) > 1e-9
    {
        return Err(invalid(
            "profile plane is not at the start and perpendicular to the path",
        ));
    }
    if course.bends == [None] && caps {
        return extrude::build(
            id,
            profile,
            Vec::new(),
            dot(sub(points[1], points[0]), frame.z),
            accuracy,
        );
    }
    check_segment_overlap(reach, &course, accuracy)?;
    let points = match profile {
        ProfileLoop::Lines(points) => points,
        ProfileLoop::Circle { frame, radius } => {
            let builder = sweep_circle::build(id, frame, radius, &course, caps, accuracy)?;
            return finish_sweep(builder, caps);
        }
        ProfileLoop::Curves(_) => {
            return Err(
                GeometryError::UnsupportedGeometry("sweep of an arc-edged profile".into()).into(),
            )
        }
    };
    let mut polygon = points;
    let area_normal = newell_normal(&polygon, accuracy.geometric)
        .ok_or_else(|| invalid("profile is collinear"))?;
    if dot(area_normal, course.starts[0]) < 0.0 {
        polygon.reverse();
    }
    let count = polygon.len();
    let rings = miter_rings(polygon, &course, count, accuracy)?;
    check_miter_spans(&rings, &course, count, accuracy)?;
    let mut builder = Builder::new(id, accuracy)?;
    let topology = add_sweep_topology(&mut builder, &rings, &course, count)?;
    if caps && !course.closed {
        add_caps(&mut builder, &topology, &course, &rings, count, accuracy)?;
    }
    add_side_faces(&mut builder, &topology, &course, &rings, count, accuracy)?;
    finish_sweep(builder, caps)
}

fn finish_sweep(builder: Builder, caps: bool) -> Result<BrepEnvelope, OperationError> {
    if caps {
        return Ok(builder.finish_solid()?);
    }
    finish_sheet(builder)
}

fn finish_sheet(mut builder: Builder) -> Result<BrepEnvelope, OperationError> {
    let faces = (0..builder.brep.topology.faces.len() as u32).collect();
    builder.brep.topology.shells.push(Shell {
        id: 0,
        faces,
        is_closed: false,
    });
    builder.brep.validate()?;
    Ok(builder.brep)
}

fn miter_rings(
    polygon: Vec<Point3>,
    course: &Course,
    count: usize,
    accuracy: Accuracy,
) -> Result<Vec<Vec<Point3>>, OperationError> {
    let path = &course.points;
    let mut offsets = polygon
        .iter()
        .map(|point| sub(*point, path[0]))
        .collect::<Vec<_>>();
    let starts = offsets.clone();
    let mut rings = Vec::with_capacity(path.len());
    rings.push(if course.closed {
        closing_ring(&polygon, &offsets, course)?
    } else {
        polygon
    });
    for joint in 1..path.len() - 1 {
        turn_offsets(&mut offsets, course, joint - 1);
        let previous = course.ends[joint - 1];
        let bisector = unit(add(previous, course.starts[joint]))
            .map_err(|_| OperationError::SweepSelfIntersection("sweep path reverses".into()))?;
        let mut ring = Vec::with_capacity(count);
        for offset in &mut offsets {
            let along = -dot(*offset, bisector) / dot(previous, bisector);
            ring.push(add(path[joint], add(*offset, scale(previous, along))));
            *offset = sub(*offset, scale(bisector, 2.0 * dot(*offset, bisector)));
        }
        rings.push(ring);
    }
    turn_offsets(&mut offsets, course, path.len() - 2);
    if course.closed {
        check_return(&offsets, &starts, course, accuracy)?;
        rings.push(rings[0].clone());
        return Ok(rings);
    }
    rings.push(
        offsets
            .iter()
            .map(|offset| add(path[path.len() - 1], *offset))
            .collect(),
    );
    Ok(rings)
}

fn closing_ring(
    polygon: &[Point3],
    offsets: &[Point3],
    course: &Course,
) -> Result<Vec<Point3>, OperationError> {
    let Some(bisector) = course.closing_bisector()? else {
        return Ok(polygon.to_vec());
    };
    let start = course.starts[0];
    Ok(offsets
        .iter()
        .map(|offset| {
            let along = -dot(*offset, bisector) / dot(start, bisector);
            add(course.points[0], add(*offset, scale(start, along)))
        })
        .collect())
}

fn check_return(
    offsets: &[Point3],
    starts: &[Point3],
    course: &Course,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let bisector = course.closing_bisector()?;
    for (offset, start) in offsets.iter().zip(starts) {
        let returned = match bisector {
            Some(bisector) => sub(*offset, scale(bisector, 2.0 * dot(*offset, bisector))),
            None => *offset,
        };
        if norm(sub(returned, *start)) > 4.0 * accuracy.geometric {
            return Err(invalid(
                "closed path does not return the profile to its start",
            ));
        }
    }
    Ok(())
}

fn turn_offsets(offsets: &mut [Point3], course: &Course, segment: usize) {
    if let Some(frame) = course.bends[segment] {
        for offset in offsets {
            *offset = frame.rotated(*offset);
        }
    }
}

fn check_miter_spans(
    rings: &[Vec<Point3>],
    course: &Course,
    count: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..course.bends.len() {
        if course.bends[segment].is_some() {
            continue;
        }
        for vertex in 0..count {
            let span = dot(
                sub(rings[segment + 1][vertex], rings[segment][vertex]),
                course.starts[segment],
            );
            if span <= 4.0 * accuracy.geometric {
                return Err(OperationError::SweepSelfIntersection(
                    "miter planes meet within a segment".into(),
                ));
            }
        }
    }
    Ok(())
}

fn add_sweep_topology(
    builder: &mut Builder,
    rings: &[Vec<Point3>],
    course: &Course,
    count: usize,
) -> Result<SweepTopology, OperationError> {
    let own = rings.len() - usize::from(course.closed);
    let mut vertices = rings[..own]
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| builder.vertex(*point))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut ring_edges = Vec::with_capacity(rings.len());
    for ring in &vertices {
        ring_edges.push(
            (0..count)
                .map(|edge| line_edge(builder, ring[edge], ring[(edge + 1) % count]))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if course.closed {
        vertices.push(vertices[0].clone());
        ring_edges.push(ring_edges[0].clone());
    }
    let mut longitudinal = Vec::with_capacity(course.bends.len());
    for segment in 0..course.bends.len() {
        if let Some(frame) = course.bends[segment] {
            longitudinal.push(add_bend_arcs(builder, frame, &vertices[segment])?);
            continue;
        }
        longitudinal.push(
            (0..count)
                .map(|vertex| {
                    line_edge(
                        builder,
                        vertices[segment][vertex],
                        vertices[segment + 1][vertex],
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    Ok(SweepTopology {
        vertices,
        ring_edges,
        longitudinal,
    })
}

fn line_edge(builder: &mut Builder, from: u32, to: u32) -> Result<u32, OperationError> {
    let a = builder.brep.topology.vertices[from as usize].position;
    let b = builder.brep.topology.vertices[to as usize].position;
    let delta = sub(b, a);
    let length = norm(delta);
    if length <= 4.0 * builder.brep.accuracy.geometric {
        return Err(OperationError::SweepSelfIntersection(
            "sweep edge collapses".into(),
        ));
    }
    Ok(builder.edge(
        CurveGeometry::Line {
            origin: a,
            direction: unit(delta)?,
        },
        Interval::new(0.0, length).map_err(GeometryError::from)?,
        false,
    ))
}

fn add_caps(
    builder: &mut Builder,
    topology: &SweepTopology,
    course: &Course,
    rings: &[Vec<Point3>],
    count: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let SweepTopology {
        vertices,
        ring_edges,
        ..
    } = topology;
    let start_frame = Frame3::from_axis(
        course.points[0],
        scale(course.starts[0], -1.0),
        sub(rings[0][1], rings[0][0]),
    )?;
    let last = course.bends.len();
    let end_frame = Frame3::from_axis(
        course.points[last],
        course.ends[last - 1],
        sub(rings[last][1], rings[last][0]),
    )?;
    let mut start_uses = Vec::new();
    let mut end_uses = Vec::new();
    for edge in (0..count).rev() {
        start_uses.push(plane_boundary(
            builder,
            start_frame,
            ring_edges[0][edge],
            vertices[0][(edge + 1) % count],
            vertices[0][edge],
            Orientation::Reverse,
        )?);
    }
    for edge in 0..count {
        end_uses.push(plane_boundary(
            builder,
            end_frame,
            ring_edges[last][edge],
            vertices[last][edge],
            vertices[last][(edge + 1) % count],
            Orientation::Forward,
        )?);
    }
    builder.face(
        "cap-start",
        SurfaceGeometry::Plane { frame: start_frame },
        padded_uv_bounds(
            rings[0].iter().map(|point| start_frame.local(*point)),
            accuracy.geometric,
        ),
        start_uses,
    )?;
    builder.face(
        "cap-end",
        SurfaceGeometry::Plane { frame: end_frame },
        padded_uv_bounds(
            rings[last].iter().map(|point| end_frame.local(*point)),
            accuracy.geometric,
        ),
        end_uses,
    )?;
    Ok(())
}

fn add_side_faces(
    builder: &mut Builder,
    topology: &SweepTopology,
    course: &Course,
    rings: &[Vec<Point3>],
    count: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..course.bends.len() {
        if let Some(frame) = course.bends[segment] {
            add_polygon_bend(builder, &polygon_bend(topology, frame, segment), accuracy)?;
            continue;
        }
        for edge in 0..count {
            let next = (edge + 1) % count;
            let origin = rings[segment][edge];
            let along_edge = unit(sub(rings[segment][next], origin))?;
            let side_frame = Frame3::from_axis(
                origin,
                unit(cross(along_edge, course.starts[segment]))?,
                along_edge,
            )?;
            let uses = side_uses(builder, topology, segment, edge, next, side_frame)?;
            let quad = [
                rings[segment][edge],
                rings[segment][next],
                rings[segment + 1][edge],
                rings[segment + 1][next],
            ];
            builder.face(
                &format!("seg-{segment}:side-0-{edge}"),
                SurfaceGeometry::Plane { frame: side_frame },
                padded_uv_bounds(
                    quad.iter().map(|point| side_frame.local(*point)),
                    accuracy.geometric,
                ),
                uses,
            )?;
        }
    }
    Ok(())
}

fn polygon_bend(topology: &SweepTopology, frame: BendFrame, segment: usize) -> PolygonBend<'_> {
    PolygonBend {
        frame,
        segment,
        start: &topology.vertices[segment],
        end: &topology.vertices[segment + 1],
        start_edges: &topology.ring_edges[segment],
        end_edges: &topology.ring_edges[segment + 1],
        arcs: &topology.longitudinal[segment],
    }
}

fn side_uses(
    builder: &Builder,
    topology: &SweepTopology,
    segment: usize,
    edge: usize,
    next: usize,
    side_frame: Frame3,
) -> Result<Vec<Use>, OperationError> {
    let SweepTopology {
        vertices,
        ring_edges,
        longitudinal,
    } = topology;
    Ok(vec![
        plane_boundary(
            builder,
            side_frame,
            ring_edges[segment][edge],
            vertices[segment][edge],
            vertices[segment][next],
            Orientation::Forward,
        )?,
        plane_boundary(
            builder,
            side_frame,
            longitudinal[segment][next],
            vertices[segment][next],
            vertices[segment + 1][next],
            Orientation::Forward,
        )?,
        plane_boundary(
            builder,
            side_frame,
            ring_edges[segment + 1][edge],
            vertices[segment + 1][next],
            vertices[segment + 1][edge],
            Orientation::Reverse,
        )?,
        plane_boundary(
            builder,
            side_frame,
            longitudinal[segment][edge],
            vertices[segment + 1][edge],
            vertices[segment][edge],
            Orientation::Reverse,
        )?,
    ])
}
