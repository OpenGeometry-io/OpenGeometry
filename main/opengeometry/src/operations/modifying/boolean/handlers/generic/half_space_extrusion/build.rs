use super::cut::extent;
use super::prism::{gap, Affine, Cap, Prism, Span};
use crate::brep::{
    boundary, padded_uv_bounds, plane_boundary, unit, uv_line, Accuracy, BrepEnvelope, Builder,
    CurveGeometry, FaceProvenance, FaceRole, Frame3, GeometryError, Orientation, PcurveGeometry,
    SurfaceGeometry, Use,
};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::face_provenance;
use crate::primitives::ProfileEdge;

struct Conic {
    curve: u32,
    alpha: f64,
    slope: f64,
    middle: f64,
    radius: f64,
    lo: f64,
}

struct CapEdge {
    edge: u32,
    forward: bool,
    conic: Option<Conic>,
}

pub(super) struct Ring {
    vertices: [Vec<u32>; 2],
    edges: [Vec<CapEdge>; 2],
    generators: Vec<u32>,
}

struct Side<'a> {
    span: &'a Span,
    ring: &'a Ring,
    index: usize,
    next: usize,
}

pub(super) fn build(
    prism: &Prism,
    id: String,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let mut builder = Builder::new(id, accuracy)?;
    let rings = add_rings(&mut builder, prism)?;
    add_sides(&mut builder, prism, &rings, FaceRole::Split, false)?;
    add_cap(&mut builder, prism, &rings, 1)?;
    add_cap(&mut builder, prism, &rings, 0)?;
    builder.finish_solid()
}

pub(super) fn add_rings(builder: &mut Builder, prism: &Prism) -> Result<Vec<Ring>, GeometryError> {
    prism
        .loops
        .iter()
        .map(|spans| add_ring(builder, prism, spans))
        .collect()
}

pub(super) fn add_sides(
    builder: &mut Builder,
    prism: &Prism,
    rings: &[Ring],
    role: FaceRole,
    reversed: bool,
) -> Result<Vec<u32>, GeometryError> {
    let mut faces = Vec::new();
    for (spans, ring) in prism.loops.iter().zip(rings) {
        for (index, span) in spans.iter().enumerate() {
            let side = Side {
                span,
                ring,
                index,
                next: (index + 1) % spans.len(),
            };
            let provenance = face_provenance(vec![span.source.clone()], role, reversed);
            faces.push(match span.edge {
                ProfileEdge::Line { .. } => add_plane_side(builder, prism, &side, provenance)?,
                ProfileEdge::Arc { .. } => add_cylinder_side(builder, prism, &side, provenance)?,
            });
        }
    }
    Ok(faces)
}

fn cap_point(prism: &Prism, cap: usize, q: [f64; 2]) -> Point3 {
    add(
        prism.frame.point([q[0], q[1], 0.0]),
        scale(prism.frame.z, prism.caps[cap].height.at(q)),
    )
}

fn sense(forward: bool) -> Orientation {
    if forward {
        Orientation::Forward
    } else {
        Orientation::Reverse
    }
}

fn add_ring(builder: &mut Builder, prism: &Prism, spans: &[Span]) -> Result<Ring, GeometryError> {
    let starts = spans
        .iter()
        .map(|span| span.edge.endpoints().0)
        .collect::<Vec<_>>();
    let vertices = [0, 1].map(|cap| {
        starts
            .iter()
            .map(|q| builder.vertex(cap_point(prism, cap, *q)))
            .collect::<Vec<_>>()
    });
    let mut edges = [Vec::new(), Vec::new()];
    for cap in 0..2 {
        for (index, span) in spans.iter().enumerate() {
            let ends = [starts[index], starts[(index + 1) % spans.len()]];
            edges[cap].push(add_cap_edge(builder, prism, cap, &span.edge, ends)?);
        }
    }
    let mut generators = Vec::with_capacity(spans.len());
    for q in &starts {
        let length = prism.caps[1].height.at(*q) - prism.caps[0].height.at(*q);
        generators.push(builder.edge(
            CurveGeometry::Line {
                origin: cap_point(prism, 0, *q),
                direction: prism.frame.z,
            },
            Interval::new(0.0, length)?,
            false,
        ));
    }
    Ok(Ring {
        vertices,
        edges,
        generators,
    })
}

fn add_cap_edge(
    builder: &mut Builder,
    prism: &Prism,
    cap: usize,
    edge: &ProfileEdge,
    ends: [[f64; 2]; 2],
) -> Result<CapEdge, GeometryError> {
    let ProfileEdge::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    } = *edge
    else {
        let [from, to] = ends.map(|q| cap_point(prism, cap, q));
        let edge = builder.edge(
            CurveGeometry::Line {
                origin: from,
                direction: unit(sub(to, from))?,
            },
            Interval::new(0.0, norm(sub(to, from)))?,
            false,
        );
        return Ok(CapEdge {
            edge,
            forward: true,
            conic: None,
        });
    };
    let height = prism.caps[cap].height;
    let slope = height.g[0].hypot(height.g[1]);
    let alpha = if slope > 1.0e-14 {
        height.g[1].atan2(height.g[0])
    } else {
        0.0
    };
    let [lo, hi] = [start_angle - alpha, start_angle + sweep_angle - alpha];
    let curve = builder.brep.geometry.curves.len() as u32;
    let geometry = cap_curve(prism, cap, center, radius, [alpha, slope])?;
    let edge = builder.edge(geometry, Interval::new(lo.min(hi), lo.max(hi))?, false);
    Ok(CapEdge {
        edge,
        forward: sweep_angle > 0.0,
        conic: Some(Conic {
            curve,
            alpha,
            slope: if slope > 1.0e-14 { slope } else { 0.0 },
            middle: height.at(center),
            radius,
            lo: lo.min(hi),
        }),
    })
}

fn cap_curve(
    prism: &Prism,
    cap: usize,
    center: [f64; 2],
    radius: f64,
    [alpha, slope]: [f64; 2],
) -> Result<CurveGeometry, GeometryError> {
    let frame = prism.frame;
    let x = frame.vector([alpha.cos(), alpha.sin(), 0.0]);
    let y = frame.vector([-alpha.sin(), alpha.cos(), 0.0]);
    let origin = cap_point(prism, cap, center);
    if slope <= 1.0e-14 {
        return Ok(CurveGeometry::Circle {
            frame: Frame3 {
                origin,
                x,
                y,
                z: frame.z,
            },
            radius,
        });
    }
    let major = unit(add(x, scale(frame.z, slope)))?;
    Ok(CurveGeometry::Ellipse {
        frame: Frame3 {
            origin,
            x: major,
            y,
            z: cross(major, y),
        },
        major_radius: radius * slope.hypot(1.0),
        minor_radius: radius,
    })
}

fn side_steps(side: &Side<'_>) -> [(u32, u32, u32, Orientation); 4] {
    let Side {
        ring, index, next, ..
    } = *side;
    let [low, high] = [&ring.edges[0][index], &ring.edges[1][index]];
    let [bottom, top] = [&ring.vertices[0], &ring.vertices[1]];
    [
        (low.edge, bottom[index], bottom[next], sense(low.forward)),
        (
            ring.generators[next],
            bottom[next],
            top[next],
            Orientation::Forward,
        ),
        (high.edge, top[next], top[index], sense(!high.forward)),
        (
            ring.generators[index],
            top[index],
            bottom[index],
            Orientation::Reverse,
        ),
    ]
}

pub(super) fn add_face(
    builder: &mut Builder,
    label: &str,
    surface: SurfaceGeometry,
    bounds: [[f64; 2]; 2],
    loops: Vec<Vec<Use>>,
    provenance: FaceProvenance,
) -> Result<u32, GeometryError> {
    let face = builder.brep.topology.faces.len();
    let mut loops = loops.into_iter();
    let outer = loops.next().unwrap_or_default();
    builder.face_with_holes(
        &format!("{face}:{label}"),
        surface,
        bounds,
        outer,
        loops.collect(),
    )?;
    builder.brep.topology.faces[face].provenance = provenance;
    Ok(face as u32)
}

fn add_plane_side(
    builder: &mut Builder,
    prism: &Prism,
    side: &Side<'_>,
    provenance: FaceProvenance,
) -> Result<u32, GeometryError> {
    let (from, to) = side.span.edge.endpoints();
    let along = unit(prism.frame.vector([to[0] - from[0], to[1] - from[1], 0.0]))?;
    let frame = Frame3 {
        origin: prism.frame.point([from[0], from[1], 0.0]),
        x: along,
        y: prism.frame.z,
        z: cross(along, prism.frame.z),
    };
    let ring = side.ring;
    let corners = [0, 1].map(|cap| [side.index, side.next].map(|at| ring.vertices[cap][at]));
    let uv = corners.iter().flatten().map(|vertex| {
        let local = frame.local(builder.brep.topology.vertices[*vertex as usize].position);
        [local[0], local[1]]
    });
    let bounds = padded_uv_bounds(uv.collect::<Vec<_>>(), builder.brep.accuracy.geometric);
    let mut uses = Vec::with_capacity(4);
    for (edge, from, to, sense) in side_steps(side) {
        uses.push(plane_boundary(builder, frame, edge, from, to, sense)?);
    }
    add_face(
        builder,
        "half-space-side",
        SurfaceGeometry::Plane { frame },
        bounds,
        vec![uses],
        provenance,
    )
}

fn cylinder_pcurve(conic: &Conic, surface: u32) -> PcurveGeometry {
    if conic.slope == 0.0 {
        return uv_line([conic.alpha, conic.middle], [1.0, 0.0]).into();
    }
    let rise = conic.radius * conic.slope;
    PcurveGeometry::ProjectedCurve {
        curve: conic.curve,
        surface,
        chart: 0,
        uv_hint: [conic.lo + conic.alpha, conic.middle + rise * conic.lo.cos()],
        uv_rate: [1.0, -rise * conic.lo.sin()],
        parameter_origin: conic.lo,
    }
}

fn add_cylinder_side(
    builder: &mut Builder,
    prism: &Prism,
    side: &Side<'_>,
    provenance: FaceProvenance,
) -> Result<u32, GeometryError> {
    let ProfileEdge::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    } = side.span.edge
    else {
        return Err(gap());
    };
    let end_angle = start_angle + sweep_angle;
    let surface = builder.brep.geometry.surfaces.len() as u32;
    let on_cap = |cap: usize| {
        side.ring.edges[cap][side.index]
            .conic
            .as_ref()
            .map(|conic| cylinder_pcurve(conic, surface))
            .ok_or_else(gap)
    };
    let (from, to) = side.span.edge.endpoints();
    let [low, high] = [prism.caps[0].height, prism.caps[1].height];
    let pcurves = [
        on_cap(0)?,
        uv_line([end_angle, low.at(to)], [0.0, 1.0]).into(),
        on_cap(1)?,
        uv_line([start_angle, low.at(from)], [0.0, 1.0]).into(),
    ];
    let uses = side_steps(side)
        .into_iter()
        .zip(pcurves)
        .map(|((edge, from, to, sense), pcurve)| boundary(edge, from, to, sense, pcurve))
        .collect();
    let g = builder.brep.accuracy.geometric;
    let arc = [&side.span.edge];
    let bounds = [
        [
            start_angle.min(end_angle) - g / radius,
            start_angle.max(end_angle) + g / radius,
        ],
        [extent(arc, low)[0] - g, extent(arc, high)[1] + g],
    ];
    let face = add_face(
        builder,
        "half-space-side",
        SurfaceGeometry::Cylinder {
            frame: Frame3 {
                origin: prism.frame.point([center[0], center[1], 0.0]),
                ..prism.frame
            },
            radius,
        },
        bounds,
        vec![uses],
        provenance,
    )?;
    if sweep_angle < 0.0 {
        builder.brep.topology.faces[face as usize].sense = Orientation::Reverse;
    }
    Ok(face)
}

pub(super) fn add_cap(
    builder: &mut Builder,
    prism: &Prism,
    rings: &[Ring],
    cap: usize,
) -> Result<u32, GeometryError> {
    let Cap {
        point,
        normal,
        height,
        ..
    } = &prism.caps[cap];
    let x = unit(sub(
        prism.frame.x,
        scale(*normal, dot(prism.frame.x, *normal)),
    ))?;
    let frame = Frame3 {
        origin: *point,
        x,
        y: cross(*normal, x),
        z: *normal,
    };
    let loops = cap_loops(builder, frame, rings, cap, cap == 1)?;
    let local = |axis: Point3| Affine {
        k: dot(axis, sub(prism.frame.origin, *point)) + dot(axis, prism.frame.z) * height.k,
        g: [
            dot(axis, prism.frame.x) + dot(axis, prism.frame.z) * height.g[0],
            dot(axis, prism.frame.y) + dot(axis, prism.frame.z) * height.g[1],
        ],
    };
    let pad = builder.brep.accuracy.geometric;
    let bounds = [frame.x, frame.y].map(|axis| {
        let [lo, hi] = extent(prism.loops[0].iter().map(|span| &span.edge), local(axis));
        [lo - pad, hi + pad]
    });
    let cap_ref = &prism.caps[cap];
    add_face(
        builder,
        if cap_ref.role == FaceRole::Cut {
            "half-space-cut"
        } else {
            "half-space-cap"
        },
        SurfaceGeometry::Plane { frame },
        bounds,
        loops,
        face_provenance(vec![cap_ref.source.clone()], cap_ref.role, cap_ref.reversed),
    )
}

pub(super) fn cap_loops(
    builder: &Builder,
    frame: Frame3,
    rings: &[Ring],
    cap: usize,
    along: bool,
) -> Result<Vec<Vec<Use>>, GeometryError> {
    let mut loops = Vec::with_capacity(rings.len());
    for ring in rings {
        let count = ring.vertices[cap].len();
        let mut uses = Vec::with_capacity(count);
        for index in 0..count {
            let next = (index + 1) % count;
            let edge = &ring.edges[cap][index];
            let vertices = &ring.vertices[cap];
            uses.push(if along {
                plane_boundary(
                    builder,
                    frame,
                    edge.edge,
                    vertices[index],
                    vertices[next],
                    sense(edge.forward),
                )?
            } else {
                plane_boundary(
                    builder,
                    frame,
                    edge.edge,
                    vertices[next],
                    vertices[index],
                    sense(!edge.forward),
                )?
            });
        }
        if !along {
            uses.reverse();
        }
        loops.push(uses);
    }
    Ok(loops)
}
