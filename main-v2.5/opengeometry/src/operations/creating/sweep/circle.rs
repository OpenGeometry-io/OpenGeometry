use crate::brep::{
    boundary, plane_boundary, unit, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    EdgeGeometry, Frame3, GeometryError, Orientation, PcurveGeometry, SurfaceGeometry, Use,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::OperationError;

struct Ring {
    centre: Point3,
    frame: Frame3,
    major: f64,
    minor: f64,
    vertices: [u32; 2],
    edges: [u32; 2],
}

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
    path: Vec<Point3>,
    directions: Vec<Point3>,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, OperationError> {
    let mut builder = Builder::new(id, accuracy)?;
    let first_axis = directions[0];
    let SweptRings {
        mut rings,
        cylinders,
        x,
        offset,
    } = add_rings(
        &mut builder,
        profile,
        radius,
        &path,
        &directions,
        first_axis,
    )?;
    let last = directions.len();
    let end_centre = add(path[last], offset);
    rings.push(ring(
        &mut builder,
        end_centre,
        directions[last - 1],
        scale(x, radius),
        x,
        radius,
        radius,
    )?);
    check_miter_planes(&rings, &cylinders, &directions, radius, last, accuracy)?;
    let seams = add_seams(&mut builder, &rings, last)?;
    let start_frame = Frame3::from_axis(rings[0].centre, scale(first_axis, -1.0), profile.x)?;
    let end_frame = Frame3::from_axis(rings[last].centre, directions[last - 1], x)?;
    add_caps(
        &mut builder,
        &rings,
        last,
        start_frame,
        end_frame,
        radius,
        accuracy,
    )?;
    add_side_faces(
        &mut builder,
        &rings,
        &cylinders,
        &seams,
        last,
        radius,
        accuracy,
    )?;
    Ok(builder.finish_solid()?)
}

fn add_rings(
    builder: &mut Builder,
    profile: Frame3,
    radius: f64,
    path: &[Point3],
    directions: &[Point3],
    first_axis: Point3,
) -> Result<SweptRings, OperationError> {
    let mut x = profile.x;
    let mut offset = sub(profile.origin, path[0]);
    let mut rings = Vec::with_capacity(path.len());
    rings.push(ring(
        builder,
        profile.origin,
        first_axis,
        scale(x, radius),
        x,
        radius,
        radius,
    )?);
    let mut cylinders = Vec::with_capacity(directions.len());
    cylinders.push(Frame3::from_axis(add(path[0], offset), first_axis, x)?);
    for joint in 1..path.len() - 1 {
        let previous = directions[joint - 1];
        let bisector = unit(add(previous, directions[joint]))
            .map_err(|_| OperationError::SweepSelfIntersection("sweep path reverses".into()))?;
        let denominator = dot(previous, bisector);
        let centre = add(
            add(path[joint], offset),
            scale(previous, -dot(offset, bisector) / denominator),
        );
        let projection =
            |vector: Point3| sub(vector, scale(previous, dot(vector, bisector) / denominator));
        let radial = scale(projection(x), radius);
        let major_axis = unit(sub(previous, scale(bisector, denominator)))?;
        let major = radius / denominator;
        rings.push(ring(
            builder, centre, bisector, radial, major_axis, major, radius,
        )?);
        offset = reflection(offset, bisector);
        x = reflection(x, bisector);
        cylinders.push(Frame3::from_axis(
            add(path[joint], offset),
            directions[joint],
            x,
        )?);
    }
    Ok(SweptRings {
        rings,
        cylinders,
        x,
        offset,
    })
}

fn ring(
    builder: &mut Builder,
    centre: Point3,
    axis: Point3,
    radial: Point3,
    major_axis: Point3,
    major: f64,
    minor: f64,
) -> Result<Ring, OperationError> {
    let y_axis = unit(cross(axis, major_axis))?;
    let frame = Frame3::from_axis(centre, axis, major_axis)?;
    let start = (dot(radial, y_axis) / minor).atan2(dot(radial, major_axis) / major);
    let curve = if (major - minor).abs() <= 1e-14 * major {
        CurveGeometry::Circle {
            frame,
            radius: major,
        }
    } else {
        CurveGeometry::Ellipse {
            frame,
            major_radius: major,
            minor_radius: minor,
        }
    };
    let vertices = [
        builder.vertex(curve.elementary_point(start)?),
        builder.vertex(curve.elementary_point(start + std::f64::consts::PI)?),
    ];
    let edges = [
        builder.edge(
            curve.clone(),
            Interval::new(start, start + std::f64::consts::PI).map_err(GeometryError::from)?,
            false,
        ),
        builder.edge(
            curve,
            Interval::new(start + std::f64::consts::PI, start + std::f64::consts::TAU)
                .map_err(GeometryError::from)?,
            false,
        ),
    ];
    Ok(Ring {
        centre,
        frame,
        major,
        minor,
        vertices,
        edges,
    })
}

fn reflection(vector: Point3, normal: Point3) -> Point3 {
    sub(vector, scale(normal, 2.0 * dot(vector, normal)))
}

fn check_miter_planes(
    rings: &[Ring],
    cylinders: &[Frame3],
    directions: &[Point3],
    radius: f64,
    last: usize,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..last {
        let axis = directions[segment];
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
    last: usize,
) -> Result<Vec<[u32; 2]>, OperationError> {
    let mut seams = Vec::with_capacity(last);
    for segment in 0..last {
        seams.push([
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
        ]);
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
        true,
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
    seams: &[[u32; 2]],
    last: usize,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(), OperationError> {
    for segment in 0..last {
        let cylinder = cylinders[segment];
        let low = v_extent(&rings[segment], cylinder.origin, cylinder.z);
        let high = v_extent(&rings[segment + 1], cylinder.origin, cylinder.z);
        let v_bounds = [
            low.0.min(high.0) - accuracy.geometric,
            low.1.max(high.1) + accuracy.geometric,
        ];
        for half in 0..2 {
            let surface = builder.brep.geometry.surfaces.len() as u32;
            let uses = side_uses(builder, rings, seams, segment, half, surface, cylinder)?;
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

fn v_extent(ring: &Ring, base: Point3, axis: Point3) -> (f64, f64) {
    let centre = dot(sub(ring.centre, base), axis);
    let amplitude =
        (ring.major * dot(ring.frame.x, axis)).hypot(ring.minor * dot(ring.frame.y, axis));
    (centre - amplitude, centre + amplitude)
}

fn side_uses(
    builder: &Builder,
    rings: &[Ring],
    seams: &[[u32; 2]],
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
        projected(
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
        )?,
        seam_use(
            builder,
            seams[segment][next],
            lower.vertices[next],
            upper.vertices[next],
            (half + 1) as f64 * std::f64::consts::PI,
            cylinder,
            Orientation::Forward,
        )?,
        projected(
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
        )?,
        seam_use(
            builder,
            seams[segment][half],
            upper.vertices[half],
            lower.vertices[half],
            half as f64 * std::f64::consts::PI,
            cylinder,
            Orientation::Reverse,
        )?,
    ])
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
