use super::bend::{add_torus_bend, BendFrame, CircleBend, CircleEnds};
use super::circle_ring::{mitre_ring, reflection, section, turned, Ring};
use super::path::Course;
use crate::brep::{
    boundary, plane_boundary, unit, uv_line, Accuracy, Builder, CurveGeometry, EdgeGeometry,
    Frame3, GeometryError, Orientation, PcurveGeometry, SurfaceGeometry, Use,
};
use crate::math::{add, dot, norm, scale, sub, Interval, Point3};
use crate::operations::OperationError;

struct SweptRings {
    rings: Vec<Ring>,
    cylinders: Vec<Frame3>,
    x: Point3,
    offset: Point3,
}

struct SurfacePoint {
    surface: u32,
    theta: f64,
    v: f64,
}

pub(super) fn build(
    id: String,
    profile: Frame3,
    radius: f64,
    course: &Course,
    caps: bool,
    accuracy: Accuracy,
) -> Result<Builder, OperationError> {
    let mut builder = Builder::new(id, accuracy)?;
    let first_axis = course.starts[0];
    let SweptRings {
        mut rings,
        cylinders,
        x,
        offset,
    } = add_rings(&mut builder, profile, radius, course)?;
    let last = course.bends.len();
    let end = if course.closed {
        rings[0]
    } else {
        let end_centre = add(course.points[last], offset);
        section(&mut builder, end_centre, course.ends[last - 1], x, radius)?
    };
    rings.push(end);
    check_miter_planes(&rings, &cylinders, course, radius, accuracy)?;
    let seams = add_seams(&mut builder, &rings, course)?;
    if caps && !course.closed {
        let start_frame = Frame3::from_axis(rings[0].centre, scale(first_axis, -1.0), profile.x)?;
        let end_frame = Frame3::from_axis(rings[last].centre, course.ends[last - 1], x)?;
        add_caps(
            &mut builder,
            &rings,
            last,
            start_frame,
            end_frame,
            radius,
            accuracy,
        )?;
    }
    add_side_faces(
        &mut builder,
        &rings,
        &cylinders,
        &seams,
        course,
        radius,
        accuracy,
    )?;
    Ok(builder)
}

fn add_rings(
    builder: &mut Builder,
    profile: Frame3,
    radius: f64,
    course: &Course,
) -> Result<SweptRings, OperationError> {
    let (path, first_axis) = (&course.points, course.starts[0]);
    let mut x = profile.x;
    let mut offset = sub(profile.origin, path[0]);
    let mut rings = Vec::with_capacity(path.len());
    let closing = course.closing_bisector()?;
    rings.push(match closing {
        Some(bisector) => mitre_ring(builder, path[0], offset, first_axis, bisector, x, radius)?,
        None => section(builder, profile.origin, first_axis, x, radius)?,
    });
    let mut cylinders = Vec::with_capacity(course.bends.len());
    cylinders.push(Frame3::from_axis(add(path[0], offset), first_axis, x)?);
    for joint in 1..path.len() - 1 {
        [offset, x] = turned(course, joint - 1, [offset, x]);
        let next = course.starts[joint];
        if course.bends[joint - 1].is_some() || course.bends[joint].is_some() {
            let centre = add(path[joint], offset);
            rings.push(section(builder, centre, next, x, radius)?);
            cylinders.push(Frame3::from_axis(centre, next, x)?);
            continue;
        }
        let previous = course.ends[joint - 1];
        let bisector = unit(add(previous, next))
            .map_err(|_| OperationError::SweepSelfIntersection("sweep path reverses".into()))?;
        rings.push(mitre_ring(
            builder,
            path[joint],
            offset,
            previous,
            bisector,
            x,
            radius,
        )?);
        offset = reflection(offset, bisector);
        x = reflection(x, bisector);
        cylinders.push(Frame3::from_axis(add(path[joint], offset), next, x)?);
    }
    [offset, x] = turned(course, path.len() - 2, [offset, x]);
    if course.closed {
        let returned = match closing {
            Some(bisector) => [reflection(offset, bisector), reflection(x, bisector)],
            None => [offset, x],
        };
        let start = sub(profile.origin, path[0]);
        if norm(sub(returned[0], start)).max(radius * norm(sub(returned[1], profile.x)))
            > 4.0 * builder.brep.accuracy.geometric
        {
            return Err(OperationError::InvalidParameter(
                "closed path does not return the profile to its start".into(),
            ));
        }
    }
    Ok(SweptRings {
        rings,
        cylinders,
        x,
        offset,
    })
}

fn check_miter_planes(
    rings: &[Ring],
    cylinders: &[Frame3],
    course: &Course,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..course.bends.len() {
        if course.bends[segment].is_some() {
            continue;
        }
        let axis = course.starts[segment];
        let start = &rings[segment];
        let end = &rings[segment + 1];
        let start_denominator = dot(axis, start.frame.z);
        let end_denominator = dot(axis, end.frame.z);
        let x = cylinders[segment].x;
        let y = cylinders[segment].y;
        let coefficient_x =
            dot(x, end.frame.z) / end_denominator - dot(x, start.frame.z) / start_denominator;
        let coefficient_y =
            dot(y, end.frame.z) / end_denominator - dot(y, start.frame.z) / start_denominator;
        let minimum =
            dot(sub(end.centre, start.centre), axis) - radius * coefficient_x.hypot(coefficient_y);
        if minimum <= 4.0 * accuracy.geometric {
            return Err(OperationError::SweepSelfIntersection(
                "circular miter planes meet within a segment".into(),
            ));
        }
    }
    Ok(())
}

fn add_seams(
    builder: &mut Builder,
    rings: &[Ring],
    course: &Course,
) -> Result<Vec<Option<[u32; 2]>>, OperationError> {
    let mut seams = Vec::with_capacity(course.bends.len());
    for segment in 0..course.bends.len() {
        if course.bends[segment].is_some() {
            seams.push(None);
            continue;
        }
        seams.push(Some([
            line_edge(
                builder,
                rings[segment].vertices[0],
                rings[segment + 1].vertices[0],
            )?,
            line_edge(
                builder,
                rings[segment].vertices[1],
                rings[segment + 1].vertices[1],
            )?,
        ]));
    }
    Ok(seams)
}

fn line_edge(builder: &mut Builder, from: u32, to: u32) -> Result<u32, OperationError> {
    let a = builder.brep.topology.vertices[from as usize].position;
    let b = builder.brep.topology.vertices[to as usize].position;
    let delta = sub(b, a);
    let length = norm(delta);
    if length <= 4.0 * builder.brep.accuracy.geometric {
        return Err(OperationError::SweepSelfIntersection(
            "circular sweep seam collapses".into(),
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
    rings: &[Ring],
    last: usize,
    start_frame: Frame3,
    end_frame: Frame3,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    let start_uses = vec![
        plane_boundary(
            builder,
            start_frame,
            rings[0].edges[1],
            rings[0].vertices[0],
            rings[0].vertices[1],
            Orientation::Reverse,
        )?,
        plane_boundary(
            builder,
            start_frame,
            rings[0].edges[0],
            rings[0].vertices[1],
            rings[0].vertices[0],
            Orientation::Reverse,
        )?,
    ];
    let end_uses = vec![
        plane_boundary(
            builder,
            end_frame,
            rings[last].edges[0],
            rings[last].vertices[0],
            rings[last].vertices[1],
            Orientation::Forward,
        )?,
        plane_boundary(
            builder,
            end_frame,
            rings[last].edges[1],
            rings[last].vertices[1],
            rings[last].vertices[0],
            Orientation::Forward,
        )?,
    ];
    builder.face(
        "cap-start",
        SurfaceGeometry::Plane { frame: start_frame },
        [[-radius - accuracy.geometric, radius + accuracy.geometric]; 2],
        start_uses,
    )?;
    builder.face(
        "cap-end",
        SurfaceGeometry::Plane { frame: end_frame },
        [[-radius - accuracy.geometric, radius + accuracy.geometric]; 2],
        end_uses,
    )?;
    Ok(())
}

fn add_side_faces(
    builder: &mut Builder,
    rings: &[Ring],
    cylinders: &[Frame3],
    seams: &[Option<[u32; 2]>],
    course: &Course,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..course.bends.len() {
        let seam = match (course.bends[segment], seams[segment]) {
            (Some(frame), _) => {
                add_torus_bend(builder, &torus_bend(rings, frame, segment, radius))?;
                continue;
            }
            (None, Some(seam)) => seam,
            (None, None) => {
                return Err(OperationError::InvalidTopology(
                    "circular sweep segment has no seams".into(),
                ))
            }
        };
        let cylinder = cylinders[segment];
        let low = v_extent(&rings[segment], cylinder.origin, cylinder.z);
        let high = v_extent(&rings[segment + 1], cylinder.origin, cylinder.z);
        let v_bounds = [
            low.0.min(high.0) - accuracy.geometric,
            low.1.max(high.1) + accuracy.geometric,
        ];
        for half in 0..2 {
            let surface = builder.brep.geometry.surfaces.len() as u32;
            let uses = side_uses(builder, rings, seam, segment, half, surface, cylinder)?;
            builder.face(
                &format!("seg-{segment}:side-0-{half}"),
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius,
                },
                [
                    [
                        half as f64 * std::f64::consts::PI,
                        (half + 1) as f64 * std::f64::consts::PI,
                    ],
                    v_bounds,
                ],
                uses,
            )?;
        }
    }
    Ok(())
}

fn torus_bend(rings: &[Ring], frame: BendFrame, segment: usize, radius: f64) -> CircleBend {
    let [start, end] = [&rings[segment], &rings[segment + 1]];
    CircleBend {
        frame,
        segment,
        centre: start.centre,
        x: start.frame.x,
        radius,
        start: CircleEnds {
            vertices: start.vertices,
            edges: start.edges,
        },
        end: CircleEnds {
            vertices: end.vertices,
            edges: end.edges,
        },
    }
}

fn v_extent(ring: &Ring, base: Point3, axis: Point3) -> (f64, f64) {
    let centre = dot(sub(ring.centre, base), axis);
    let amplitude =
        (ring.major * dot(ring.frame.x, axis)).hypot(ring.minor * dot(ring.frame.y, axis));
    (centre - amplitude, centre + amplitude)
}

fn side_uses(
    builder: &Builder,
    rings: &[Ring],
    seams: [u32; 2],
    segment: usize,
    half: usize,
    surface: u32,
    cylinder: Frame3,
) -> Result<Vec<Use>, OperationError> {
    let next = (half + 1) % 2;
    let lower = &rings[segment];
    let upper = &rings[segment + 1];
    let start_v = dot(sub(lower.centre, cylinder.origin), cylinder.z);
    let end_v = dot(sub(upper.centre, cylinder.origin), cylinder.z);
    Ok(vec![
        ring_use(
            builder,
            lower.edges[half],
            &SurfacePoint {
                surface,
                theta: half as f64 * std::f64::consts::PI,
                v: start_v,
            },
            lower.vertices[half],
            lower.vertices[next],
            Orientation::Forward,
            lower.perpendicular,
        )?,
        seam_use(
            builder,
            seams[next],
            lower.vertices[next],
            upper.vertices[next],
            (half + 1) as f64 * std::f64::consts::PI,
            cylinder,
            Orientation::Forward,
        )?,
        ring_use(
            builder,
            upper.edges[half],
            &SurfacePoint {
                surface,
                theta: half as f64 * std::f64::consts::PI,
                v: end_v,
            },
            upper.vertices[next],
            upper.vertices[half],
            Orientation::Reverse,
            upper.perpendicular,
        )?,
        seam_use(
            builder,
            seams[half],
            upper.vertices[half],
            lower.vertices[half],
            half as f64 * std::f64::consts::PI,
            cylinder,
            Orientation::Reverse,
        )?,
    ])
}

fn ring_use(
    builder: &Builder,
    edge: u32,
    point: &SurfacePoint,
    from: u32,
    to: u32,
    sense: Orientation,
    cap: bool,
) -> Result<Use, OperationError> {
    if cap {
        return Ok(boundary(
            edge,
            from,
            to,
            sense,
            uv_line([0.0, point.v], [1.0, 0.0]),
        ));
    }
    projected(builder, edge, point, from, to, sense)
}

fn projected(
    builder: &Builder,
    edge: u32,
    point: &SurfacePoint,
    from: u32,
    to: u32,
    sense: Orientation,
) -> Result<Use, OperationError> {
    let SurfacePoint { surface, theta, v } = *point;
    let EdgeGeometry::Curve { curve, range } = builder.brep.topology.edges[edge as usize].geometry
    else {
        return Err(OperationError::InvalidTopology(
            "sweep edge has no curve".into(),
        ));
    };
    Ok(boundary(
        edge,
        from,
        to,
        sense,
        PcurveGeometry::ProjectedCurve {
            curve,
            surface,
            chart: 0,
            uv_hint: [theta, v],
            uv_rate: [1.0, 0.0],
            parameter_origin: range.lo,
        },
    ))
}

fn seam_use(
    builder: &Builder,
    edge: u32,
    from: u32,
    to: u32,
    theta: f64,
    cylinder: Frame3,
    sense: Orientation,
) -> Result<Use, OperationError> {
    let EdgeGeometry::Curve { curve, .. } = builder.brep.topology.edges[edge as usize].geometry
    else {
        return Err(OperationError::InvalidTopology(
            "sweep seam has no curve".into(),
        ));
    };
    let CurveGeometry::Line { origin, direction } = builder.brep.geometry.curves[curve as usize]
    else {
        return Err(OperationError::InvalidTopology(
            "sweep seam is not a line".into(),
        ));
    };
    let v = dot(sub(origin, cylinder.origin), cylinder.z);
    let dv = dot(direction, cylinder.z);
    Ok(boundary(
        edge,
        from,
        to,
        sense,
        uv_line([theta, v], [0.0, dv]),
    ))
}
