use super::prism::{cap_height, gap, outward, Affine, Cap, Prism};
use crate::brep::{BrepEnvelope, FaceRole, GeometryError, SurfaceGeometry};
use crate::math::{dot, scale, sub, Point3};
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::primitives::ProfileEdge;
use std::f64::consts::TAU;

pub(super) struct Plane {
    pub(super) point: Point3,
    pub(super) outward: Point3,
    pub(super) face: u32,
}

pub(super) fn extent<'a>(edges: impl IntoIterator<Item = &'a ProfileEdge>, f: Affine) -> [f64; 2] {
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    let mut add = |value: f64| {
        range[0] = range[0].min(value);
        range[1] = range[1].max(value);
    };
    for edge in edges {
        let (from, to) = edge.endpoints();
        add(f.at(from));
        add(f.at(to));
        if let ProfileEdge::Arc {
            center,
            radius,
            start_angle,
            sweep_angle,
        } = *edge
        {
            let steepest = f.g[1].atan2(f.g[0]);
            for turn in [steepest, steepest + 0.5 * TAU] {
                let offset = if sweep_angle > 0.0 {
                    (turn - start_angle).rem_euclid(TAU)
                } else {
                    (start_angle - turn).rem_euclid(TAU)
                };
                if offset < sweep_angle.abs() {
                    add(f.at([
                        center[0] + radius * turn.cos(),
                        center[1] + radius * turn.sin(),
                    ]));
                }
            }
        }
    }
    range
}

pub(super) fn cut_cap(
    prism: &Prism,
    cutter: &BrepEnvelope,
    clearance: f64,
) -> Result<(usize, Cap), GeometryError> {
    let planes = convex_planes(cutter, clearance)?;
    for (index, plane) in planes.iter().enumerate() {
        let normal = scale(plane.outward, -1.0);
        let along = dot(normal, prism.frame.z);
        if along.abs() < 1.0e-6 {
            continue;
        }
        let height = cap_height(prism.frame, plane.point, normal);
        let [low, high] = &prism.caps;
        if extent(profile(prism), height.minus(low.height))[0] <= clearance
            || extent(profile(prism), high.height.minus(height))[0] <= clearance
        {
            continue;
        }
        let replaced = usize::from(along > 0.0);
        let removed = [height, prism.caps[replaced].height];
        let contained = planes.iter().enumerate().all(|(other, face)| {
            other == index
                || removed.iter().all(|cap| {
                    extent(profile(prism), plane_at_cap(prism, face, *cap))[1] < -clearance
                })
        });
        if contained {
            let cap = Cap {
                point: plane.point,
                normal,
                height,
                source: brep_face_source(cutter, plane.face),
                role: FaceRole::Cut,
                reversed: true,
            };
            return Ok((replaced, cap));
        }
    }
    Err(gap())
}

pub(super) fn profile(prism: &Prism) -> impl Iterator<Item = &ProfileEdge> {
    prism.loops.iter().flatten().map(|span| &span.edge)
}

pub(super) fn plane_at_cap(prism: &Prism, plane: &Plane, cap: Affine) -> Affine {
    let frame = prism.frame;
    let rise = dot(plane.outward, frame.z);
    Affine {
        k: dot(plane.outward, sub(frame.origin, plane.point)) + rise * cap.k,
        g: [
            dot(plane.outward, frame.x) + rise * cap.g[0],
            dot(plane.outward, frame.y) + rise * cap.g[1],
        ],
    }
}

pub(super) fn convex_planes(
    cutter: &BrepEnvelope,
    clearance: f64,
) -> Result<Vec<Plane>, GeometryError> {
    if cutter.solids.len() != 1 || cutter.topology.shells.len() != 1 {
        return Err(gap());
    }
    let mut planes = Vec::with_capacity(cutter.topology.faces.len());
    for face in &cutter.topology.faces {
        let SurfaceGeometry::Plane { frame } = &cutter.geometry.surfaces[face.surface as usize]
        else {
            return Err(gap());
        };
        planes.push(Plane {
            point: frame.origin,
            outward: outward(frame.z, face.sense),
            face: face.id,
        });
    }
    let convex = cutter.topology.vertices.iter().all(|vertex| {
        planes
            .iter()
            .all(|plane| dot(plane.outward, sub(vertex.position, plane.point)) <= clearance)
    });
    if convex {
        Ok(planes)
    } else {
        Err(gap())
    }
}
