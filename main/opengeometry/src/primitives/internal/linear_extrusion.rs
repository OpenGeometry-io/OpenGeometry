use super::extruded_loop::{add_plane_side, ExtrudedLoop, SideFace};
use crate::brep::{
    dimensions, padded_uv_bounds, plane_boundary, unit, Accuracy, BrepEnvelope, Builder,
    CurveGeometry, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{norm, scale, sub, Interval};
use crate::primitives::profile::{
    loops_touch, orient_profile_loop, profile_point_inside, validate_profile_loop,
};

pub fn linear_extrusion(
    id: String,
    frame: Frame3,
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[height])?;
    if height <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "extrusion height is below geometric resolution".into(),
        ));
    }
    validate_profile_loop(&outer, accuracy.geometric, "outer profile")?;
    for (index, hole) in holes.iter().enumerate() {
        validate_profile_loop(hole, accuracy.geometric, &format!("profile hole {index}"))?;
        if !profile_point_inside(hole[0], &outer) || loops_touch(&outer, hole, accuracy.geometric) {
            return Err(GeometryError::InvalidGeometry(format!(
                "profile hole {index} is not strictly inside the outer profile"
            )));
        }
    }
    for first in 0..holes.len() {
        for second in (first + 1)..holes.len() {
            if loops_touch(&holes[first], &holes[second], accuracy.geometric)
                || profile_point_inside(holes[first][0], &holes[second])
                || profile_point_inside(holes[second][0], &holes[first])
            {
                return Err(GeometryError::InvalidGeometry(format!(
                    "profile holes {first} and {second} overlap or nest"
                )));
            }
        }
    }

    let mut loops = Vec::with_capacity(holes.len() + 1);
    loops.push(orient_profile_loop(outer, true));
    loops.extend(
        holes
            .into_iter()
            .map(|hole| orient_profile_loop(hole, false)),
    );
    let mut builder = Builder::new(id, accuracy)?;
    let built = add_extrusion_loops(&mut builder, frame, &loops, height)?;

    let top_bounds = padded_uv_bounds(loops.iter().flatten(), accuracy.geometric);
    let bottom_bounds = [top_bounds[0], [-top_bounds[1][1], -top_bounds[1][0]]];
    let mut top_frame = frame;
    top_frame.origin = frame.point([0.0, 0.0, height]);
    let bottom_frame = Frame3 {
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
        ..frame
    };
    add_cap_faces(
        &mut builder,
        &loops,
        &built,
        top_frame,
        bottom_frame,
        top_bounds,
        bottom_bounds,
    )?;
    add_side_faces(&mut builder, &loops, &built, frame, height, accuracy)?;
    builder.finish_solid()
}

fn add_extrusion_loops(
    builder: &mut Builder,
    frame: Frame3,
    loops: &[Vec<[f64; 2]>],
    height: f64,
) -> Result<Vec<ExtrudedLoop>, GeometryError> {
    let mut built = Vec::with_capacity(loops.len());
    for points in loops {
        let bottom_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], 0.0])))
            .collect::<Vec<_>>();
        let top_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], height])))
            .collect::<Vec<_>>();
        let mut bottom_edges = Vec::with_capacity(points.len());
        let mut top_edges = Vec::with_capacity(points.len());
        let mut vertical_edges = Vec::with_capacity(points.len());
        for index in 0..points.len() {
            let next = (index + 1) % points.len();
            let direction = unit(sub(
                builder.brep.topology.vertices[bottom_vertices[next] as usize].position,
                builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
            ))?;
            let length = norm(sub(
                builder.brep.topology.vertices[bottom_vertices[next] as usize].position,
                builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
            ));
            bottom_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin:
                        builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
                    direction,
                },
                Interval::new(0.0, length)?,
                false,
            ));
            top_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin: builder.brep.topology.vertices[top_vertices[index] as usize].position,
                    direction,
                },
                Interval::new(0.0, length)?,
                false,
            ));
            vertical_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin:
                        builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
                    direction: frame.z,
                },
                Interval::new(0.0, height)?,
                false,
            ));
        }
        built.push(ExtrudedLoop {
            bottom_vertices,
            top_vertices,
            bottom_edges,
            top_edges,
            vertical_edges,
        });
    }
    Ok(built)
}

fn add_cap_faces(
    builder: &mut Builder,
    loops: &[Vec<[f64; 2]>],
    built: &[ExtrudedLoop],
    top_frame: Frame3,
    bottom_frame: Frame3,
    top_bounds: [[f64; 2]; 2],
    bottom_bounds: [[f64; 2]; 2],
) -> Result<(), GeometryError> {
    let top_outer = cap_uses(builder, &loops[0], &built[0], true, top_frame, bottom_frame)?;
    let top_holes = loops[1..]
        .iter()
        .zip(&built[1..])
        .map(|(points, profile)| cap_uses(builder, points, profile, true, top_frame, bottom_frame))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face_with_holes(
        "top",
        SurfaceGeometry::Plane { frame: top_frame },
        top_bounds,
        top_outer,
        top_holes,
    )?;
    let bottom_outer = cap_uses(
        builder,
        &loops[0],
        &built[0],
        false,
        top_frame,
        bottom_frame,
    )?;
    let bottom_holes = loops[1..]
        .iter()
        .zip(&built[1..])
        .map(|(points, profile)| cap_uses(builder, points, profile, false, top_frame, bottom_frame))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face_with_holes(
        "bottom",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        bottom_bounds,
        bottom_outer,
        bottom_holes,
    )
}

fn cap_uses(
    builder: &Builder,
    points: &[[f64; 2]],
    profile: &ExtrudedLoop,
    top: bool,
    top_frame: Frame3,
    bottom_frame: Frame3,
) -> Result<Vec<Use>, GeometryError> {
    let count = points.len();
    (0..count)
        .map(|offset| {
            let index = if top { offset } else { count - 1 - offset };
            let next = (index + 1) % count;
            if top {
                plane_boundary(
                    builder,
                    top_frame,
                    profile.top_edges[index],
                    profile.top_vertices[index],
                    profile.top_vertices[next],
                    Orientation::Forward,
                )
            } else {
                plane_boundary(
                    builder,
                    bottom_frame,
                    profile.bottom_edges[index],
                    profile.bottom_vertices[next],
                    profile.bottom_vertices[index],
                    Orientation::Reverse,
                )
            }
        })
        .collect()
}

fn add_side_faces(
    builder: &mut Builder,
    loops: &[Vec<[f64; 2]>],
    built: &[ExtrudedLoop],
    frame: Frame3,
    height: f64,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for (loop_index, (points, profile)) in loops.iter().zip(built).enumerate() {
        for index in 0..points.len() {
            let next = (index + 1) % points.len();
            let bottom_from = profile.bottom_vertices[index];
            let bottom_to = profile.bottom_vertices[next];
            let top_from = profile.top_vertices[index];
            let top_to = profile.top_vertices[next];
            let side = SideFace {
                frame,
                height,
                g: accuracy.geometric,
                loop_index,
                index,
                next,
                bottom_from,
                bottom_to,
                top_from,
                top_to,
            };
            add_plane_side(builder, profile, &side)?;
        }
    }
    Ok(())
}
