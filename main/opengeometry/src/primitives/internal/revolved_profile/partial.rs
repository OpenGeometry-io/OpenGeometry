use super::{check_axis_contact, generator, revolved_profile, stations, Generator, Lateral};
use crate::brep::{
    boundary, plane_boundary, unit, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    EdgeGeometry, Frame3, GeometryError, Orientation, Surface, SurfaceGeometry, Use,
};
use crate::math::add;
use crate::math::{dot, norm, scale, sub, Interval, Point3};
use crate::primitives::profile::{validate_arc_profile_loop, ProfileEdge};
use std::f64::consts::TAU;

#[derive(Clone, Copy)]
struct Ends {
    vertices: [u32; 2],
    ring: Option<u32>,
    collapsed: bool,
}

pub fn partially_revolved_profile(
    id: String,
    frame: Frame3,
    mut edges: Vec<ProfileEdge>,
    angle: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    if !angle.is_finite() || angle <= 0.0 || angle > TAU {
        return Err(GeometryError::InvalidGeometry(
            "revolve angle must be positive and no greater than a full turn".into(),
        ));
    }
    if angle == TAU {
        return revolved_profile(id, frame, edges, accuracy);
    }
    let g = accuracy.geometric;
    let mut keys = (0..edges.len()).collect::<Vec<_>>();
    if validate_arc_profile_loop(&edges, g)? < 0.0 {
        edges = edges.iter().rev().map(ProfileEdge::reversed).collect();
        keys.reverse();
    }
    let stations = stations(&edges, g)?;
    let mut radius = stations
        .iter()
        .filter(|station| station.r > 0.0)
        .map(|station| station.r)
        .fold(f64::INFINITY, f64::min);
    if !radius.is_finite() {
        radius = edges.iter().fold(0.0_f64, |reach, edge| match edge {
            ProfileEdge::Arc { center, radius, .. } => reach.max(center[0] + radius),
            ProfileEdge::Line { from, to } => reach.max(from[0]).max(to[0]),
        });
    }
    if 2.0 * radius * (angle.min(TAU - angle) / 2.0).sin() <= g {
        return Err(GeometryError::InvalidGeometry(
            "partial revolve cap separation is below geometric accuracy".into(),
        ));
    }
    let count = edges.len();
    let generators = (0..count)
        .map(|i| {
            generator(
                frame,
                &edges[i],
                [stations[i], stations[(i + 1) % count]],
                g,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    check_axis_contact(&stations, &generators)?;
    let end_frame = rotated(frame, frame, angle);
    let mut b = Builder::new(id, accuracy)?;
    let ends = stations
        .iter()
        .enumerate()
        .map(|(i, station)| {
            let start = b.vertex(frame.point([station.r, 0.0, station.z]));
            if station.r == 0.0 {
                let lateral = matches!(generators[i], Generator::Lateral(_))
                    || matches!(generators[(i + count - 1) % count], Generator::Lateral(_));
                let ring = lateral
                    .then(|| b.edge_geometry(EdgeGeometry::Collapsed { vertex: start }, true));
                Ok(Ends {
                    vertices: [start, start],
                    ring,
                    collapsed: true,
                })
            } else {
                let end = b.vertex(end_frame.point([station.r, 0.0, station.z]));
                let circle = CurveGeometry::Circle {
                    frame: Frame3 {
                        origin: frame.point([0.0, 0.0, station.z]),
                        ..frame
                    },
                    radius: station.r,
                };
                let ring = b.edge(circle, Interval::new(0.0, angle)?, false);
                Ok(Ends {
                    vertices: [start, end],
                    ring: Some(ring),
                    collapsed: false,
                })
            }
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut meridians = Vec::with_capacity(count);
    for (i, generator) in generators.iter().enumerate() {
        let (curve, range, forward) = match generator {
            Generator::Lateral(lateral) => {
                (lateral.seam.clone(), lateral.seam_range, lateral.forward)
            }
            _ => {
                let from = frame.point([stations[i].r, 0.0, stations[i].z]);
                let next = stations[(i + 1) % count];
                let delta = sub(frame.point([next.r, 0.0, next.z]), from);
                (
                    CurveGeometry::Line {
                        origin: from,
                        direction: unit(delta)?,
                    },
                    Interval::new(0.0, norm(delta))?,
                    true,
                )
            }
        };
        let start = b.edge(curve.clone(), range, false);
        let end = if matches!(generator, Generator::Axis) {
            start
        } else {
            b.edge(rotate_curve(curve, frame, angle)?, range, false)
        };
        meridians.push(([start, end], forward));
    }
    for (i, generator) in generators.iter().enumerate() {
        let next = (i + 1) % count;
        let pair = [ends[i], ends[next]];
        let key = format!("edge-{}", keys[i]);
        match generator {
            Generator::Axis => {}
            Generator::Plane => {
                let facing = if stations[next].r > stations[i].r {
                    -1.0
                } else {
                    1.0
                };
                let plane = Frame3 {
                    origin: frame.point([0.0, 0.0, stations[i].z]),
                    x: frame.x,
                    y: scale(frame.y, facing),
                    z: scale(frame.z, facing),
                };
                let mut uses = Vec::new();
                if let Some(edge) = pair[0].ring {
                    uses.push(plane_boundary(
                        &b,
                        plane,
                        edge,
                        pair[0].vertices[0],
                        pair[0].vertices[1],
                        Orientation::Forward,
                    )?);
                }
                uses.push(plane_boundary(
                    &b,
                    plane,
                    meridians[i].0[1],
                    pair[0].vertices[1],
                    pair[1].vertices[1],
                    Orientation::Forward,
                )?);
                if let Some(edge) = pair[1].ring {
                    uses.push(plane_boundary(
                        &b,
                        plane,
                        edge,
                        pair[1].vertices[1],
                        pair[1].vertices[0],
                        Orientation::Reverse,
                    )?);
                }
                uses.push(plane_boundary(
                    &b,
                    plane,
                    meridians[i].0[0],
                    pair[1].vertices[0],
                    pair[0].vertices[0],
                    Orientation::Reverse,
                )?);
                let radius = stations[i].r.max(stations[next].r);
                b.face(
                    &key,
                    SurfaceGeometry::Plane { frame: plane },
                    [[-radius, radius]; 2],
                    uses,
                )?;
            }
            Generator::Lateral(lateral) => {
                add_lateral(&mut b, &key, lateral, pair, meridians[i].0, angle)?
            }
        }
    }
    for side in 0..2 {
        let radial = if side == 0 { frame } else { end_frame };
        let sign = if side == 0 { 1.0 } else { -1.0 };
        let plane = Frame3 {
            origin: frame.origin,
            x: radial.x,
            y: scale(frame.z, sign),
            z: scale(radial.y, -sign),
        };
        let mut uses = Vec::with_capacity(count);
        for i in 0..count {
            let next = (i + 1) % count;
            let (edge, forward) = (meridians[i].0[side], meridians[i].1);
            let (from, to) = if side == 0 {
                (ends[i].vertices[side], ends[next].vertices[side])
            } else {
                (ends[next].vertices[side], ends[i].vertices[side])
            };
            let sense = if forward == (side == 0) {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            uses.push(plane_boundary(&b, plane, edge, from, to, sense)?);
        }
        if side == 1 {
            uses.reverse();
        }
        let extent = edges
            .iter()
            .fold(0.0_f64, |extent, edge| match edge {
                ProfileEdge::Line { from, to } => from
                    .iter()
                    .chain(to.iter())
                    .fold(extent, |n, v| n.max(v.abs())),
                ProfileEdge::Arc { center, radius, .. } => extent
                    .max(center[0].abs() + radius)
                    .max(center[1].abs() + radius),
            })
            .max(g);
        b.face(
            if side == 0 { "start-cap" } else { "end-cap" },
            SurfaceGeometry::Plane { frame: plane },
            [[-extent, extent]; 2],
            uses,
        )?;
    }
    b.finish_solid()
}

fn add_lateral(
    b: &mut Builder,
    key: &str,
    lateral: &Lateral,
    ends: [Ends; 2],
    meridians: [u32; 2],
    angle: f64,
) -> Result<(), GeometryError> {
    let turn = lateral.turn;
    let u0 = if turn > 0.0 { 0.0 } else { TAU };
    let u1 = u0 + turn * angle;
    let ring = |end: Ends, v: f64, leading: bool| -> Result<Use, GeometryError> {
        let edge = end.ring.ok_or_else(|| {
            GeometryError::InvalidTopology("partial revolve lateral face misses a ring".into())
        })?;
        let (from, to) = if leading {
            (end.vertices[0], end.vertices[1])
        } else {
            (end.vertices[1], end.vertices[0])
        };
        let (sense, pcurve) = if end.collapsed {
            (
                Orientation::Forward,
                uv_line(
                    [if leading { u0 } else { u1 }, v],
                    [if leading { turn * angle } else { -turn * angle }, 0.0],
                ),
            )
        } else {
            (
                if leading {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                },
                uv_line([u0, v], [turn, 0.0]),
            )
        };
        Ok(boundary(edge, from, to, sense, pcurve))
    };
    let along = if lateral.forward {
        Orientation::Forward
    } else {
        Orientation::Reverse
    };
    let back = if lateral.forward {
        Orientation::Reverse
    } else {
        Orientation::Forward
    };
    let [origin, rate] = lateral.seam_v;
    let uses = vec![
        ring(ends[0], lateral.v[0], true)?,
        boundary(
            meridians[1],
            ends[0].vertices[1],
            ends[1].vertices[1],
            along,
            uv_line([u1, origin], [0.0, rate]),
        ),
        ring(ends[1], lateral.v[1], false)?,
        boundary(
            meridians[0],
            ends[1].vertices[0],
            ends[0].vertices[0],
            back,
            uv_line([u0, origin], [0.0, rate]),
        ),
    ];
    let face = b.brep.topology.faces.len();
    b.face(
        key,
        lateral.surface.clone(),
        [
            [u0.min(u1), u0.max(u1)],
            [
                lateral.v[0].min(lateral.v[1]),
                lateral.v[0].max(lateral.v[1]),
            ],
        ],
        uses,
    )?;
    if dot(
        lateral
            .surface
            .normal_at([0.0, (lateral.v[0] + lateral.v[1]) / 2.0])?,
        lateral.outward,
    ) < 0.0
    {
        b.brep.topology.faces[face].sense = Orientation::Reverse;
    }
    Ok(())
}

fn rotate_vector(v: Point3, axis: Frame3, angle: f64) -> Point3 {
    let x = dot(v, axis.x);
    let y = dot(v, axis.y);
    let z = dot(v, axis.z);
    axis.vector([
        x * angle.cos() - y * angle.sin(),
        x * angle.sin() + y * angle.cos(),
        z,
    ])
}
fn rotated(frame: Frame3, axis: Frame3, angle: f64) -> Frame3 {
    Frame3 {
        origin: add(
            axis.origin,
            rotate_vector(sub(frame.origin, axis.origin), axis, angle),
        ),
        x: rotate_vector(frame.x, axis, angle),
        y: rotate_vector(frame.y, axis, angle),
        z: rotate_vector(frame.z, axis, angle),
    }
}
fn rotate_curve(
    curve: CurveGeometry,
    axis: Frame3,
    angle: f64,
) -> Result<CurveGeometry, GeometryError> {
    Ok(match curve {
        CurveGeometry::Line { origin, direction } => CurveGeometry::Line {
            origin: add(
                axis.origin,
                rotate_vector(sub(origin, axis.origin), axis, angle),
            ),
            direction: rotate_vector(direction, axis, angle),
        },
        CurveGeometry::Circle { frame, radius } => CurveGeometry::Circle {
            frame: rotated(frame, axis, angle),
            radius,
        },
        _ => {
            return Err(GeometryError::UnsupportedGeometry(
                "partial revolve meridian curve".into(),
            ))
        }
    })
}
