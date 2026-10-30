use crate::brep::{
    boundary, plane_boundary, unit, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    EdgeGeometry, Frame3, GeometryError, Orientation, Surface, SurfaceGeometry, Use,
};
use crate::math::{dot, norm, scale, sub, Interval, Point3};
use crate::primitives::profile::{validate_arc_profile_loop, ProfileEdge};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Copy)]
struct Station {
    r: f64,
    z: f64,
}

#[derive(Clone, Copy)]
struct Ring {
    vertex: u32,
    edge: u32,
    collapsed: bool,
}

struct Lateral {
    surface: SurfaceGeometry,
    turn: f64,
    v: [f64; 2],
    seam: CurveGeometry,
    seam_range: Interval,
    seam_v: [f64; 2],
    forward: bool,
    outward: Point3,
}

struct Support {
    surface: SurfaceGeometry,
    turn: f64,
    v: [f64; 2],
}

enum Generator {
    Axis,
    Plane,
    Lateral(Box<Lateral>),
}

struct MeridianArc {
    center: [f64; 2],
    radius: f64,
    start: f64,
    sweep: f64,
}

pub fn revolved_profile(
    id: String,
    frame: Frame3,
    mut edges: Vec<ProfileEdge>,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    let g = accuracy.geometric;
    let mut keys = (0..edges.len()).collect::<Vec<_>>();
    if validate_arc_profile_loop(&edges, g)? < 0.0 {
        edges = edges.iter().rev().map(ProfileEdge::reversed).collect();
        keys.reverse();
    }
    let stations = stations(&edges, g)?;
    let count = edges.len();
    let generators = (0..count)
        .map(|index| {
            let ends = [stations[index], stations[(index + 1) % count]];
            generator(frame, &edges[index], ends, g)
        })
        .collect::<Result<Vec<_>, _>>()?;
    check_axis_contact(&stations, &generators)?;
    let mut b = Builder::new(id, accuracy)?;
    let rings = add_rings(&mut b, frame, &stations, &generators)?;
    for (index, generator) in generators.iter().enumerate() {
        let next = (index + 1) % count;
        let key = format!("edge-{}", keys[index]);
        let ends = [rings[index], rings[next]];
        match generator {
            Generator::Axis => {}
            Generator::Plane => {
                add_plane_face(&mut b, frame, &key, [stations[index], stations[next]], ends)?
            }
            Generator::Lateral(lateral) => add_lateral_face(&mut b, &key, lateral, ends)?,
        }
    }
    b.finish_solid()
}

fn stations(edges: &[ProfileEdge], g: f64) -> Result<Vec<Station>, GeometryError> {
    edges
        .iter()
        .map(|edge| {
            let [r, z] = edge.endpoints().0;
            if r < -g {
                return Err(GeometryError::InvalidGeometry(
                    "revolved profile crosses the revolve axis".into(),
                ));
            }
            Ok(Station {
                r: if r <= g { 0.0 } else { r },
                z,
            })
        })
        .collect()
}

fn generator(
    frame: Frame3,
    edge: &ProfileEdge,
    ends: [Station; 2],
    g: f64,
) -> Result<Generator, GeometryError> {
    let [from, to] = ends;
    let lateral = match *edge {
        ProfileEdge::Line { .. } => {
            if from.r == 0.0 && to.r == 0.0 {
                return Ok(Generator::Axis);
            }
            if (to.r - from.r).abs() > g && (to.z - from.z).abs() <= g {
                return Ok(Generator::Plane);
            }
            line_lateral(frame, ends, g)?
        }
        ProfileEdge::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } => {
            let arc = MeridianArc {
                center,
                radius,
                start: start_angle,
                sweep: sweep_angle,
            };
            arc_lateral(frame, &arc, ends, g)?
        }
    };
    Ok(Generator::Lateral(Box::new(lateral)))
}

fn line_lateral(frame: Frame3, ends: [Station; 2], g: f64) -> Result<Lateral, GeometryError> {
    let [from, to] = ends;
    let (dr, dz) = (to.r - from.r, to.z - from.z);
    let Support { surface, turn, v } = if dr.abs() <= g {
        Support {
            surface: SurfaceGeometry::Cylinder {
                frame,
                radius: from.r,
            },
            turn: 1.0,
            v: [from.z, to.z],
        }
    } else {
        cone(frame, ends, g)?
    };
    let vector = frame.vector([dr, 0.0, dz]);
    let length = norm(vector);
    Ok(Lateral {
        surface,
        turn,
        v,
        seam: CurveGeometry::Line {
            origin: frame.point([from.r, 0.0, from.z]),
            direction: unit(vector)?,
        },
        seam_range: Interval::new(0.0, length)?,
        seam_v: [v[0], (v[1] - v[0]) / length],
        forward: true,
        outward: frame.vector([dz, 0.0, -dr]),
    })
}

fn cone(frame: Frame3, ends: [Station; 2], g: f64) -> Result<Support, GeometryError> {
    let [from, to] = ends;
    let (dr, dz) = (to.r - from.r, to.z - from.z);
    let slope = dr.abs() / dz.abs();
    let v = [from.r / slope, to.r / slope];
    let turn = if dr * dz > 0.0 { 1.0 } else { -1.0 };
    let mut apex = Frame3 {
        origin: frame.point([0.0, 0.0, from.z - turn * v[0]]),
        ..frame
    };
    apex.y = scale(frame.y, turn);
    apex.z = scale(frame.z, turn);
    let surface = SurfaceGeometry::Cone {
        frame: apex,
        semi_angle: dr.abs().atan2(dz.abs()),
    };
    for (station, height) in ends.iter().zip(v) {
        let expected = frame.point([station.r, 0.0, station.z]);
        if norm(sub(surface.point_at([0.0, height])?, expected)) > g {
            return Err(GeometryError::InvalidGeometry(
                "revolved profile line is too close to a constant-z line for a cone".into(),
            ));
        }
    }
    Ok(Support { surface, turn, v })
}

fn arc_lateral(
    frame: Frame3,
    arc: &MeridianArc,
    ends: [Station; 2],
    g: f64,
) -> Result<Lateral, GeometryError> {
    let &MeridianArc {
        center,
        radius,
        start,
        sweep,
    } = arc;
    let first = (start + PI).rem_euclid(TAU) - PI;
    let mut v = [first, first + sweep];
    let on_axis = center[0].abs() <= g;
    let surface_frame = Frame3 {
        origin: frame.point([0.0, 0.0, center[1]]),
        ..frame
    };
    let surface = if on_axis {
        check_meridian_arc(v, radius, g)?;
        for (value, station) in v.iter_mut().zip(ends) {
            *value = if station.r == 0.0 {
                FRAC_PI_2.copysign(*value)
            } else {
                value.clamp(-FRAC_PI_2, FRAC_PI_2)
            };
        }
        SurfaceGeometry::Sphere {
            frame: surface_frame,
            radius,
        }
    } else if center[0] - radius > g {
        SurfaceGeometry::Torus {
            frame: surface_frame,
            major_radius: center[0],
            minor_radius: radius,
        }
    } else {
        return Err(GeometryError::InvalidGeometry(
            "revolved profile arc circle meets the revolve axis".into(),
        ));
    };
    let middle = start + sweep / 2.0;
    let side = sweep.signum();
    Ok(Lateral {
        surface,
        turn: 1.0,
        v,
        seam: CurveGeometry::Circle {
            frame: Frame3 {
                origin: frame.point([if on_axis { 0.0 } else { center[0] }, 0.0, center[1]]),
                x: frame.x,
                y: frame.z,
                z: scale(frame.y, -1.0),
            },
            radius,
        },
        seam_range: Interval::new(v[0].min(v[1]), v[0].max(v[1]))?,
        seam_v: [0.0, 1.0],
        forward: v[1] > v[0],
        outward: frame.vector([side * middle.cos(), 0.0, side * middle.sin()]),
    })
}

fn check_meridian_arc(v: [f64; 2], radius: f64, g: f64) -> Result<(), GeometryError> {
    let reach = FRAC_PI_2 + g / radius;
    if v[0].min(v[1]) < -reach || v[0].max(v[1]) > reach {
        return Err(GeometryError::InvalidGeometry(
            "revolved profile arc passes through a pole of the revolve axis".into(),
        ));
    }
    Ok(())
}

fn check_axis_contact(stations: &[Station], generators: &[Generator]) -> Result<(), GeometryError> {
    let count = stations.len();
    let axis = |index: usize| matches!(generators[index % count], Generator::Axis);
    if (0..count).any(|index| stations[index].r == 0.0 && !axis(index + count - 1) && !axis(index))
    {
        return Err(GeometryError::InvalidGeometry(
            "revolved profile touches the revolve axis at a point".into(),
        ));
    }
    if (0..count)
        .filter(|&index| axis(index) && !axis(index + 1))
        .count()
        > 1
    {
        return Err(GeometryError::InvalidGeometry(
            "revolved profile meets the revolve axis along more than one segment".into(),
        ));
    }
    Ok(())
}

fn add_rings(
    b: &mut Builder,
    frame: Frame3,
    stations: &[Station],
    generators: &[Generator],
) -> Result<Vec<Option<Ring>>, GeometryError> {
    let count = stations.len();
    let mut rings = Vec::with_capacity(count);
    for (index, station) in stations.iter().enumerate() {
        let lateral = [(index + count - 1) % count, index]
            .iter()
            .any(|&near| matches!(generators[near], Generator::Lateral(_)));
        let position = frame.point([station.r, 0.0, station.z]);
        rings.push(if station.r > 0.0 {
            let vertex = b.vertex(position);
            let circle = CurveGeometry::Circle {
                frame: Frame3 {
                    origin: frame.point([0.0, 0.0, station.z]),
                    ..frame
                },
                radius: station.r,
            };
            let edge = b.edge(circle, Interval::new(0.0, TAU)?, false);
            Some(Ring {
                vertex,
                edge,
                collapsed: false,
            })
        } else if lateral {
            let vertex = b.vertex(position);
            let edge = b.edge_geometry(EdgeGeometry::Collapsed { vertex }, true);
            Some(Ring {
                vertex,
                edge,
                collapsed: true,
            })
        } else {
            None
        });
    }
    Ok(rings)
}

fn add_plane_face(
    b: &mut Builder,
    frame: Frame3,
    key: &str,
    ends: [Station; 2],
    rings: [Option<Ring>; 2],
) -> Result<(), GeometryError> {
    let [from, to] = ends;
    let facing = if to.r > from.r { -1.0 } else { 1.0 };
    let plane = Frame3 {
        origin: frame.point([0.0, 0.0, from.z]),
        x: frame.x,
        y: scale(frame.y, facing),
        z: scale(frame.z, facing),
    };
    let plane_ring = |ring: Option<Ring>, sense| {
        ring.map(|Ring { vertex, edge, .. }| plane_boundary(b, plane, edge, vertex, vertex, sense))
            .transpose()
    };
    let start = plane_ring(rings[0], Orientation::Forward)?;
    let end = plane_ring(rings[1], Orientation::Reverse)?;
    let (outer, inner, radius) = if from.r > to.r {
        (start, end, from.r)
    } else {
        (end, start, to.r)
    };
    let outer = outer.ok_or_else(|| {
        GeometryError::InvalidTopology("revolved plane face has no outer ring".into())
    })?;
    b.face_with_holes(
        key,
        SurfaceGeometry::Plane { frame: plane },
        [[-radius, radius]; 2],
        vec![outer],
        inner.into_iter().map(|hole| vec![hole]).collect(),
    )
}

fn add_lateral_face(
    b: &mut Builder,
    key: &str,
    lateral: &Lateral,
    rings: [Option<Ring>; 2],
) -> Result<(), GeometryError> {
    let [Some(start), Some(end)] = rings else {
        return Err(GeometryError::InvalidTopology(
            "revolved lateral face misses a ring".into(),
        ));
    };
    let seam = b.edge(lateral.seam.clone(), lateral.seam_range, true);
    let v = lateral.v;
    let normal = lateral.surface.normal_at([0.0, (v[0] + v[1]) / 2.0])?;
    let face = b.brep.topology.faces.len();
    b.face(
        key,
        lateral.surface.clone(),
        [[0.0, TAU], [v[0].min(v[1]), v[0].max(v[1])]],
        lateral_uses(lateral, seam, [start, end]),
    )?;
    if dot(normal, lateral.outward) < 0.0 {
        b.brep.topology.faces[face].sense = Orientation::Reverse;
    }
    Ok(())
}

fn lateral_uses(lateral: &Lateral, seam: u32, rings: [Ring; 2]) -> Vec<Use> {
    let [start, end] = rings;
    let turn = lateral.turn;
    let u0 = if turn > 0.0 { 0.0 } else { TAU };
    let [origin, rate] = lateral.seam_v;
    let (along, back) = if lateral.forward {
        (Orientation::Forward, Orientation::Reverse)
    } else {
        (Orientation::Reverse, Orientation::Forward)
    };
    vec![
        ring_use(start, lateral.v[0], turn, true),
        boundary(
            seam,
            start.vertex,
            end.vertex,
            along,
            uv_line([TAU - u0, origin], [0.0, rate]),
        ),
        ring_use(end, lateral.v[1], turn, false),
        boundary(
            seam,
            end.vertex,
            start.vertex,
            back,
            uv_line([u0, origin], [0.0, rate]),
        ),
    ]
}

fn ring_use(ring: Ring, v: f64, turn: f64, leading: bool) -> Use {
    let u0 = if turn > 0.0 { 0.0 } else { TAU };
    let (sense, pcurve) = match (ring.collapsed, leading) {
        (false, true) => (Orientation::Forward, uv_line([u0, v], [turn, 0.0])),
        (false, false) => (Orientation::Reverse, uv_line([u0, v], [turn, 0.0])),
        (true, true) => (Orientation::Forward, uv_line([u0, v], [turn * TAU, 0.0])),
        (true, false) => (
            Orientation::Forward,
            uv_line([TAU - u0, v], [-turn * TAU, 0.0]),
        ),
    };
    boundary(ring.edge, ring.vertex, ring.vertex, sense, pcurve)
}
