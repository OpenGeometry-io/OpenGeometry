use super::extruded_loop::{add_plane_side, ExtrudedLoop, SideFace};
use crate::brep::{
    boundary, dimensions, plane_boundary, unit, uv_line, Accuracy, BrepEnvelope, Builder,
    CurveGeometry, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{add, norm, scale, Interval, Point3};
use crate::primitives::profile::{
    arc_profile_contains, profile_edge_intersections, validate_arc_profile_loop, ProfileEdge,
};

struct EdgeCurves {
    bottom: CurveGeometry,
    top: CurveGeometry,
    range: Interval,
}

struct OrientedSide {
    face: SideFace,
    forward: bool,
    bottom_sense: Orientation,
    top_sense: Orientation,
}

pub fn arc_edged_extrusion(
    id: String,
    frame: Frame3,
    outer: Vec<ProfileEdge>,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    arc_edged_extrusion_with_holes(id, frame, outer, Vec::new(), height, accuracy)
}

pub fn arc_edged_extrusion_with_holes(
    id: String,
    frame: Frame3,
    mut outer: Vec<ProfileEdge>,
    mut holes: Vec<Vec<ProfileEdge>>,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[height])?;
    let g = accuracy.geometric;
    if height <= 4.0 * g {
        return Err(GeometryError::UnresolvedIntersection(
            "arc-edged extrusion height is below geometric resolution".into(),
        ));
    }
    if validate_arc_profile_loop(&outer, g)? < 0.0 {
        outer = outer.iter().rev().map(ProfileEdge::reversed).collect();
    }
    for (index, hole) in holes.iter_mut().enumerate() {
        if validate_arc_profile_loop(hole, g)? > 0.0 {
            *hole = hole.iter().rev().map(ProfileEdge::reversed).collect();
        }
        for hole_edge in hole.iter() {
            for outer_edge in &outer {
                if !profile_edge_intersections(hole_edge, outer_edge, g).is_empty() {
                    return Err(GeometryError::InvalidGeometry(format!(
                        "arc-edged profile hole {index} touches its outer boundary"
                    )));
                }
            }
        }
        if !arc_profile_contains(&outer, hole[0].endpoints().0, g) {
            return Err(GeometryError::InvalidGeometry(format!(
                "arc-edged profile hole {index} lies outside the outer boundary"
            )));
        }
    }
    check_hole_pairs(&holes, g)?;

    let mut builder = Builder::new(id, accuracy)?;
    let profiles = std::iter::once(outer).chain(holes).collect::<Vec<_>>();
    let loops = add_arc_extrusion_loops(&mut builder, frame, &profiles, height)?;

    let bounds = cap_bounds(&profiles[0], g);
    let top_frame = Frame3 {
        origin: frame.point([0.0, 0.0, height]),
        ..frame
    };
    let bottom_frame = Frame3 {
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
        ..frame
    };
    add_cap_faces(
        &mut builder,
        &profiles,
        &loops,
        top_frame,
        bottom_frame,
        bounds,
    )?;

    add_side_faces(&mut builder, &profiles, &loops, frame, height, g)?;
    builder.finish_solid()
}

fn check_hole_pairs(holes: &[Vec<ProfileEdge>], g: f64) -> Result<(), GeometryError> {
    for first in 0..holes.len() {
        for second in (first + 1)..holes.len() {
            if holes[first].iter().any(|a| {
                holes[second]
                    .iter()
                    .any(|b| !profile_edge_intersections(a, b, g).is_empty())
            }) || arc_profile_contains(&holes[first], holes[second][0].endpoints().0, g)
                || arc_profile_contains(&holes[second], holes[first][0].endpoints().0, g)
            {
                return Err(GeometryError::InvalidGeometry(format!(
                    "arc-edged profile holes {first} and {second} overlap or nest"
                )));
            }
        }
    }
    Ok(())
}

fn add_arc_extrusion_loops(
    builder: &mut Builder,
    frame: Frame3,
    profiles: &[Vec<ProfileEdge>],
    height: f64,
) -> Result<Vec<ExtrudedLoop>, GeometryError> {
    let mut loops = Vec::with_capacity(profiles.len());
    for edges in profiles {
        let points = edges
            .iter()
            .map(|edge| edge.endpoints().0)
            .collect::<Vec<_>>();
        let bottom_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], 0.0])))
            .collect::<Vec<_>>();
        let top_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], height])))
            .collect::<Vec<_>>();
        let mut bottom_edges = Vec::with_capacity(edges.len());
        let mut top_edges = Vec::with_capacity(edges.len());
        let mut vertical_edges = Vec::with_capacity(edges.len());
        for (index, edge) in edges.iter().enumerate() {
            let from = builder.brep.topology.vertices[bottom_vertices[index] as usize].position;
            let top_from = builder.brep.topology.vertices[top_vertices[index] as usize].position;
            let EdgeCurves { bottom, top, range } =
                profile_edge_curves(edge, frame, from, top_from, height)?;
            bottom_edges.push(builder.edge(bottom, range, false));
            top_edges.push(builder.edge(top, range, false));
            vertical_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin: from,
                    direction: frame.z,
                },
                Interval::new(0.0, height)?,
                false,
            ));
        }
        loops.push(ExtrudedLoop {
            bottom_vertices,
            top_vertices,
            bottom_edges,
            top_edges,
            vertical_edges,
        });
    }
    Ok(loops)
}

fn profile_edge_curves(
    edge: &ProfileEdge,
    frame: Frame3,
    from: Point3,
    top_from: Point3,
    height: f64,
) -> Result<EdgeCurves, GeometryError> {
    let curves = match edge {
        ProfileEdge::Line { from: p, to: q } => {
            let vector = frame.vector([q[0] - p[0], q[1] - p[1], 0.0]);
            let length = norm(vector);
            let direction = unit(vector)?;
            EdgeCurves {
                bottom: CurveGeometry::Line {
                    origin: from,
                    direction,
                },
                top: CurveGeometry::Line {
                    origin: top_from,
                    direction,
                },
                range: Interval::new(0.0, length)?,
            }
        }
        ProfileEdge::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let origin = frame.point([center[0], center[1], 0.0]);
            let circle_frame = Frame3 { origin, ..frame };
            let top_circle_frame = Frame3 {
                origin: add(origin, scale(frame.z, height)),
                ..frame
            };
            let end_angle = start_angle + sweep_angle;
            EdgeCurves {
                bottom: CurveGeometry::Circle {
                    frame: circle_frame,
                    radius: *radius,
                },
                top: CurveGeometry::Circle {
                    frame: top_circle_frame,
                    radius: *radius,
                },
                range: Interval::new(start_angle.min(end_angle), start_angle.max(end_angle))?,
            }
        }
    };
    Ok(curves)
}

fn cap_bounds(edges: &[ProfileEdge], g: f64) -> [[f64; 2]; 2] {
    let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
    for edge in edges {
        match edge {
            ProfileEdge::Line { from, to } => {
                for point in [from, to] {
                    for axis in 0..2 {
                        bounds[axis][0] = bounds[axis][0].min(point[axis]);
                        bounds[axis][1] = bounds[axis][1].max(point[axis]);
                    }
                }
            }
            ProfileEdge::Arc { center, radius, .. } => {
                for axis in 0..2 {
                    bounds[axis][0] = bounds[axis][0].min(center[axis] - radius);
                    bounds[axis][1] = bounds[axis][1].max(center[axis] + radius);
                }
            }
        }
    }
    for bound in &mut bounds {
        bound[0] -= g;
        bound[1] += g;
    }
    bounds
}

fn add_cap_faces(
    builder: &mut Builder,
    profiles: &[Vec<ProfileEdge>],
    loops: &[ExtrudedLoop],
    top_frame: Frame3,
    bottom_frame: Frame3,
    bounds: [[f64; 2]; 2],
) -> Result<(), GeometryError> {
    let top_outer = cap_uses(
        builder,
        &profiles[0],
        &loops[0],
        true,
        top_frame,
        bottom_frame,
    )?;
    let top_holes = profiles[1..]
        .iter()
        .zip(&loops[1..])
        .map(|(edges, profile)| cap_uses(builder, edges, profile, true, top_frame, bottom_frame))
        .collect::<Result<Vec<_>, _>>()?;
    let bottom_outer = cap_uses(
        builder,
        &profiles[0],
        &loops[0],
        false,
        top_frame,
        bottom_frame,
    )?;
    let bottom_holes = profiles[1..]
        .iter()
        .zip(&loops[1..])
        .map(|(edges, profile)| cap_uses(builder, edges, profile, false, top_frame, bottom_frame))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face_with_holes(
        "top",
        SurfaceGeometry::Plane { frame: top_frame },
        bounds,
        top_outer,
        top_holes,
    )?;
    builder.face_with_holes(
        "bottom",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        [bounds[0], [-bounds[1][1], -bounds[1][0]]],
        bottom_outer,
        bottom_holes,
    )
}

fn cap_uses(
    builder: &Builder,
    edges: &[ProfileEdge],
    profile: &ExtrudedLoop,
    top: bool,
    top_frame: Frame3,
    bottom_frame: Frame3,
) -> Result<Vec<Use>, GeometryError> {
    let count = edges.len();
    let mut uses = Vec::with_capacity(count);
    for index in 0..count {
        let next = (index + 1) % count;
        let forward = !matches!(edges[index], ProfileEdge::Arc { sweep_angle, .. }
            if sweep_angle < 0.0);
        let top_sense = if forward {
            Orientation::Forward
        } else {
            Orientation::Reverse
        };
        let bottom_sense = if forward {
            Orientation::Reverse
        } else {
            Orientation::Forward
        };
        uses.push(if top {
            plane_boundary(
                builder,
                top_frame,
                profile.top_edges[index],
                profile.top_vertices[index],
                profile.top_vertices[next],
                top_sense,
            )?
        } else {
            plane_boundary(
                builder,
                bottom_frame,
                profile.bottom_edges[index],
                profile.bottom_vertices[next],
                profile.bottom_vertices[index],
                bottom_sense,
            )?
        });
    }
    if !top {
        uses.reverse();
    }
    Ok(uses)
}

fn add_side_faces(
    builder: &mut Builder,
    profiles: &[Vec<ProfileEdge>],
    loops: &[ExtrudedLoop],
    frame: Frame3,
    height: f64,
    g: f64,
) -> Result<(), GeometryError> {
    for (loop_index, (edges, profile)) in profiles.iter().zip(loops).enumerate() {
        for (index, edge) in edges.iter().enumerate() {
            let next = (index + 1) % edges.len();
            let bottom_from = profile.bottom_vertices[index];
            let bottom_to = profile.bottom_vertices[next];
            let top_from = profile.top_vertices[index];
            let top_to = profile.top_vertices[next];
            let forward =
                !matches!(edge, ProfileEdge::Arc { sweep_angle, .. } if *sweep_angle < 0.0);
            let bottom_sense = if forward {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            let top_sense = if forward {
                Orientation::Reverse
            } else {
                Orientation::Forward
            };
            let side = OrientedSide {
                face: SideFace {
                    frame,
                    height,
                    g,
                    loop_index,
                    index,
                    next,
                    bottom_from,
                    bottom_to,
                    top_from,
                    top_to,
                },
                forward,
                bottom_sense,
                top_sense,
            };
            match edge {
                ProfileEdge::Line { .. } => {
                    add_plane_side(builder, profile, &side.face)?;
                }
                ProfileEdge::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep_angle,
                } => {
                    add_cylinder_side(
                        builder,
                        profile,
                        &side,
                        *center,
                        *radius,
                        *start_angle,
                        *sweep_angle,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn add_cylinder_side(
    builder: &mut Builder,
    profile: &ExtrudedLoop,
    side: &OrientedSide,
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    sweep_angle: f64,
) -> Result<(), GeometryError> {
    let &OrientedSide {
        face:
            SideFace {
                frame,
                height,
                g,
                loop_index,
                index,
                ..
            },
        forward,
        ..
    } = side;
    let end_angle = start_angle + sweep_angle;
    let circle_frame = Frame3 {
        origin: frame.point([center[0], center[1], 0.0]),
        ..frame
    };
    let uses = cylinder_side_uses(profile, side, start_angle, end_angle);
    let face_id = builder.brep.topology.faces.len();
    builder.face(
        &format!("side-{loop_index}-{index}"),
        SurfaceGeometry::Cylinder {
            frame: circle_frame,
            radius,
        },
        [
            [
                start_angle.min(end_angle) - g / radius,
                start_angle.max(end_angle) + g / radius,
            ],
            [-g, height + g],
        ],
        uses,
    )?;
    if !forward {
        builder.brep.topology.faces[face_id].sense = Orientation::Reverse;
    }
    Ok(())
}

fn cylinder_side_uses(
    profile: &ExtrudedLoop,
    side: &OrientedSide,
    start_angle: f64,
    end_angle: f64,
) -> Vec<Use> {
    let &OrientedSide {
        face:
            SideFace {
                height,
                index,
                next,
                bottom_from,
                bottom_to,
                top_from,
                top_to,
                ..
            },
        bottom_sense,
        top_sense,
        ..
    } = side;
    vec![
        boundary(
            profile.bottom_edges[index],
            bottom_from,
            bottom_to,
            bottom_sense,
            uv_line([0.0, 0.0], [1.0, 0.0]),
        ),
        boundary(
            profile.vertical_edges[next],
            bottom_to,
            top_to,
            Orientation::Forward,
            uv_line([end_angle, 0.0], [0.0, 1.0]),
        ),
        boundary(
            profile.top_edges[index],
            top_to,
            top_from,
            top_sense,
            uv_line([0.0, height], [1.0, 0.0]),
        ),
        boundary(
            profile.vertical_edges[index],
            top_from,
            bottom_from,
            Orientation::Reverse,
            uv_line([start_angle, 0.0], [0.0, 1.0]),
        ),
    ]
}
