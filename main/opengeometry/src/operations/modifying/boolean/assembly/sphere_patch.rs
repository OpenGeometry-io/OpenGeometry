use super::provenance::{provenance, reverse_face};
use crate::brep::{
    boundary, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry, EdgeGeometry, FaceRole,
    Frame3, GeometryError, Orientation, Surface, SurfaceGeometry,
};
use crate::math::{scale, Interval};
use crate::operations::modifying::boolean::operands::SphereInput;
use crate::primitives;
use Orientation::{Forward as F, Reverse as R};

pub(crate) struct SpherePatch {
    pub(crate) frame: Frame3,
    pub(crate) latitude: f64,
    pub(crate) north: bool,
    pub(crate) shared_edge: u32,
    pub(crate) shared_vertex: u32,
}

pub(crate) fn append_sphere(
    out: &mut BrepEnvelope,
    input: &SphereInput<'_>,
    reversed: bool,
) -> Result<u32, GeometryError> {
    let mut part = oriented_sphere_part(input, out.accuracy, reversed)?;
    let [v, e, h, l, f, s, c, p, surface] = [
        out.topology.vertices.len(),
        out.topology.edges.len(),
        out.topology.halfedges.len(),
        out.topology.loops.len(),
        out.topology.faces.len(),
        out.topology.shells.len(),
        out.geometry.curves.len(),
        out.geometry.pcurves.len(),
        out.geometry.surfaces.len(),
    ]
    .map(|n| n as u32);
    for vertex in &mut part.topology.vertices {
        vertex.id += v;
        vertex.outgoing_halfedge = vertex.outgoing_halfedge.map(|id| id + h);
    }
    for edge in &mut part.topology.edges {
        edge.id += e;
        edge.halfedge += h;
        edge.twin_halfedge = edge.twin_halfedge.map(|id| id + h);
        match &mut edge.geometry {
            EdgeGeometry::Curve { curve, .. } => *curve += c,
            EdgeGeometry::Collapsed { vertex } => *vertex += v,
        }
    }
    for halfedge in &mut part.topology.halfedges {
        halfedge.id += h;
        halfedge.from += v;
        halfedge.to += v;
        halfedge.edge += e;
        halfedge.twin = halfedge.twin.map(|id| id + h);
        halfedge.next = halfedge.next.map(|id| id + h);
        halfedge.prev = halfedge.prev.map(|id| id + h);
        halfedge.face = halfedge.face.map(|id| id + f);
        halfedge.loop_ref = halfedge.loop_ref.map(|id| id + l);
        halfedge.geometry_use.pcurve = halfedge.geometry_use.pcurve.map(|id| id + p);
    }
    for loop_ in &mut part.topology.loops {
        loop_.id += l;
        loop_.start_halfedge += h;
        loop_.face_ref += f;
    }
    for face in &mut part.topology.faces {
        face.id += f;
        face.key = format!("{f}:{}", face.key);
        face.surface += surface;
        face.trim.outer += l;
        face.shell_ref = Some(s);
    }
    for shell in &mut part.topology.shells {
        shell.id += s;
        for face in &mut shell.faces {
            *face += f;
        }
    }
    add_part(out, part);
    Ok(s)
}

fn oriented_sphere_part(
    input: &SphereInput<'_>,
    accuracy: Accuracy,
    reversed: bool,
) -> Result<BrepEnvelope, GeometryError> {
    let mut part = primitives::sphere(input.brep.id.clone(), input.frame, input.radius, accuracy)?;
    part.topology.faces[0].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Preserved
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut part, 0);
    }
    Ok(part)
}

fn add_part(out: &mut BrepEnvelope, part: BrepEnvelope) {
    out.geometry.surfaces.extend(part.geometry.surfaces);
    out.geometry.curves.extend(part.geometry.curves);
    out.geometry.pcurves.extend(part.geometry.pcurves);
    out.topology.vertices.extend(part.topology.vertices);
    out.topology.edges.extend(part.topology.edges);
    out.topology.halfedges.extend(part.topology.halfedges);
    out.topology.loops.extend(part.topology.loops);
    out.topology.faces.extend(part.topology.faces);
    out.topology.shells.extend(part.topology.shells);
}

pub(crate) fn patch(
    builder: &mut Builder,
    input: &SphereInput<'_>,
    sphere_patch: &SpherePatch,
    reversed: bool,
) -> Result<(), GeometryError> {
    let SpherePatch {
        frame,
        latitude,
        north,
        shared_edge,
        shared_vertex,
    } = *sphere_patch;
    let pole_lat = if north {
        std::f64::consts::FRAC_PI_2
    } else {
        -std::f64::consts::FRAC_PI_2
    };
    let (lo, hi) = if north {
        (latitude, pole_lat)
    } else {
        (pole_lat, latitude)
    };
    let surface = SurfaceGeometry::Sphere {
        frame,
        radius: input.radius,
    };
    let pole = builder.vertex(surface.point_at([0.0, pole_lat])?);
    let collapse = builder.edge_geometry(EdgeGeometry::Collapsed { vertex: pole }, true);
    let seam = builder.edge(
        CurveGeometry::Circle {
            frame: Frame3 {
                x: frame.x,
                y: frame.z,
                z: scale(frame.y, -1.0),
                ..frame
            },
            radius: input.radius,
        },
        Interval::new(lo, hi)?,
        true,
    );
    let tau = std::f64::consts::TAU;
    let (low, high) = if north {
        (shared_vertex, pole)
    } else {
        (pole, shared_vertex)
    };
    let bottom = if north {
        boundary(shared_edge, low, low, F, uv_line([0.0, lo], [1.0, 0.0]))
    } else {
        boundary(collapse, low, low, F, uv_line([0.0, lo], [tau, 0.0]))
    };
    let top = if north {
        boundary(collapse, high, high, F, uv_line([tau, hi], [-tau, 0.0]))
    } else {
        boundary(shared_edge, high, high, R, uv_line([0.0, hi], [1.0, 0.0]))
    };
    let face = builder.brep.topology.faces.len() as u32;
    builder.face(
        &format!("{face}:{}", input.brep.topology.faces[0].key),
        surface,
        [[0.0, tau], [lo, hi]],
        vec![
            bottom,
            boundary(seam, low, high, F, uv_line([tau, 0.0], [0.0, 1.0])),
            top,
            boundary(seam, high, low, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ],
    )?;
    set_patch_provenance(builder, input, face, reversed);
    Ok(())
}

fn set_patch_provenance(builder: &mut Builder, input: &SphereInput<'_>, face: u32, reversed: bool) {
    builder.brep.topology.faces[face as usize].provenance = provenance(
        input,
        if reversed {
            FaceRole::Cut
        } else {
            FaceRole::Split
        },
        reversed,
    );
    if reversed {
        reverse_face(&mut builder.brep, face);
    }
}
