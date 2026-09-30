mod circle;

use super::extrude;
use super::profile::{check_lines, profile_frame, ProfileLoop};
use crate::brep::{
    padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder, CurveGeometry, Frame3,
    GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::{invalid, OperationError};

use circle as sweep_circle;

struct SweepTopology {
    vertices: Vec<Vec<u32>>,
    ring_edges: Vec<Vec<u32>>,
    longitudinal: Vec<Vec<u32>>,
}

pub(crate) fn build(
    id: String,
    profile: ProfileLoop,
    mut path: Vec<Point3>,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, OperationError> {
    if path.len() < 2 || path.iter().flatten().any(|value| !value.is_finite()) {
        return Err(invalid("sweep path is invalid"));
    }
    remove_collinear_joints(&mut path, accuracy)?;
    let directions = path_directions(&path, accuracy)?;
    let frame = profile_frame(&profile, accuracy.geometric)?;
    check_lines(&profile, frame, accuracy.geometric)?;
    if dot(sub(path[0], frame.origin), frame.z).abs() > 4.0 * accuracy.geometric
        || (1.0 - dot(frame.z, directions[0]).abs()).abs() > 1e-9
    {
        return Err(invalid(
            "profile plane is not at the start and perpendicular to the path",
        ));
    }
    if directions.len() == 1 {
        return extrude::build(
            id,
            profile,
            Vec::new(),
            dot(sub(path[1], path[0]), frame.z),
            accuracy,
        );
    }
    check_segment_overlap(&profile, &path, &directions, accuracy)?;
    let points = match profile {
        ProfileLoop::Lines(points) => points,
        ProfileLoop::Circle { frame, radius } => {
            return sweep_circle::build(id, frame, radius, path, directions, accuracy)
        }
    };
    let mut polygon = points;
    let area_normal = polygon
        .iter()
        .enumerate()
        .fold([0.0; 3], |sum, (index, point)| {
            add(sum, cross(*point, polygon[(index + 1) % polygon.len()]))
        });
    if dot(area_normal, directions[0]) < 0.0 {
        polygon.reverse();
    }
    let count = polygon.len();
    let rings = miter_rings(polygon, &path, &directions, count)?;
    check_miter_spans(&rings, &directions, count, accuracy)?;
    let mut builder = Builder::new(id, accuracy)?;
    let topology = add_sweep_topology(&mut builder, &rings, &directions, count)?;
    add_caps(
        &mut builder,
        &topology,
        &path,
        &directions,
        &rings,
        count,
        accuracy,
    )?;
    add_side_faces(
        &mut builder,
        &topology,
        &directions,
        &rings,
        count,
        accuracy,
    )?;
    Ok(builder.finish_solid()?)
}

fn remove_collinear_joints(
    path: &mut Vec<Point3>,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let mut index = 1;
    while index + 1 < path.len() {
        let before = sub(path[index], path[index - 1]);
        let after = sub(path[index + 1], path[index]);
        if norm(before) <= 4.0 * accuracy.geometric || norm(after) <= 4.0 * accuracy.geometric {
            return Err(invalid("sweep path has a short segment"));
        }
        let alignment = dot(unit(before)?, unit(after)?);
        if norm(add(unit(before)?, unit(after)?)) <= 1e-9 {
            return Err(OperationError::SweepSelfIntersection(
                "sweep path reverses".into(),
            ));
        }
        if alignment >= 1.0 - 1e-18 {
            path.remove(index);
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn path_directions(path: &[Point3], accuracy: Accuracy) -> Result<Vec<Point3>, OperationError> {
    path.windows(2)
        .map(|pair| {
            let delta = sub(pair[1], pair[0]);
            if norm(delta) <= 4.0 * accuracy.geometric {
                return Err(invalid("sweep path has a short segment"));
            }
            unit(delta).map_err(Into::into)
        })
        .collect::<Result<Vec<_>, OperationError>>()
}

fn check_segment_overlap(
    profile: &ProfileLoop,
    path: &[Point3],
    directions: &[Point3],
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let radius = match profile {
        ProfileLoop::Lines(points) => points
            .iter()
            .map(|point| norm(sub(*point, path[0])))
            .fold(0.0_f64, f64::max),
        ProfileLoop::Circle { frame, radius } => norm(sub(frame.origin, path[0])) + radius,
    };
    for first in 0..directions.len() {
        for second in first + 2..directions.len() {
            if segment_distance(path[first], path[first + 1], path[second], path[second + 1])
                <= 2.0 * radius + 4.0 * accuracy.geometric
            {
                return Err(OperationError::SweepSelfIntersection(
                    "non-adjacent sweep segments overlap".into(),
                ));
            }
        }
    }
    Ok(())
}

fn segment_distance(a: Point3, b: Point3, c: Point3, d: Point3) -> f64 {
    let u = sub(b, a);
    let v = sub(d, c);
    let w = sub(a, c);
    let (aa, bb, cc, dd, ee) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let denominator = aa * cc - bb * bb;
    let mut s = if denominator.abs() <= 1e-20 {
        0.0
    } else {
        ((bb * ee - cc * dd) / denominator).clamp(0.0, 1.0)
    };
    let t = ((bb * s + ee) / cc).clamp(0.0, 1.0);
    s = ((bb * t - dd) / aa).clamp(0.0, 1.0);
    norm(sub(add(a, scale(u, s)), add(c, scale(v, t))))
}

fn miter_rings(
    polygon: Vec<Point3>,
    path: &[Point3],
    directions: &[Point3],
    count: usize,
) -> Result<Vec<Vec<Point3>>, OperationError> {
    let mut offsets = polygon
        .iter()
        .map(|point| sub(*point, path[0]))
        .collect::<Vec<_>>();
    let mut rings = Vec::with_capacity(path.len());
    rings.push(polygon);
    for joint in 1..path.len() - 1 {
        let previous = directions[joint - 1];
        let bisector = unit(add(previous, directions[joint]))
            .map_err(|_| OperationError::SweepSelfIntersection("sweep path reverses".into()))?;
        let mut ring = Vec::with_capacity(count);
        for offset in &mut offsets {
            let along = -dot(*offset, bisector) / dot(previous, bisector);
            ring.push(add(path[joint], add(*offset, scale(previous, along))));
            *offset = sub(*offset, scale(bisector, 2.0 * dot(*offset, bisector)));
        }
        rings.push(ring);
    }
    rings.push(
        offsets
            .iter()
            .map(|offset| add(path[path.len() - 1], *offset))
            .collect(),
    );
    Ok(rings)
}

fn check_miter_spans(
    rings: &[Vec<Point3>],
    directions: &[Point3],
    count: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..directions.len() {
        for vertex in 0..count {
            let span = dot(
                sub(rings[segment + 1][vertex], rings[segment][vertex]),
                directions[segment],
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
    directions: &[Point3],
    count: usize,
) -> Result<SweepTopology, OperationError> {
    let vertices = rings
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
    let mut longitudinal = Vec::with_capacity(directions.len());
    for segment in 0..directions.len() {
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
    path: &[Point3],
    directions: &[Point3],
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
        path[0],
        scale(directions[0], -1.0),
        sub(rings[0][1], rings[0][0]),
    )?;
    let last = directions.len();
    let end_frame = Frame3::from_axis(
        path[last],
        directions[last - 1],
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
    directions: &[Point3],
    rings: &[Vec<Point3>],
    count: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..directions.len() {
        for edge in 0..count {
            let next = (edge + 1) % count;
            let origin = rings[segment][edge];
            let along_edge = unit(sub(rings[segment][next], origin))?;
            let side_frame = Frame3::from_axis(
                origin,
                unit(cross(along_edge, directions[segment]))?,
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
