use super::build::{add_cap, add_face, add_rings, add_sides, cap_loops, Ring};
use super::cut::{convex_planes, extent, plane_at_cap, profile, Plane};
use super::prism::{cap_height, face_loops, gap, loop_halfedges, prism, Cap, Prism};
use crate::brep::{
    padded_uv_bounds, plane_boundary, Accuracy, BrepEnvelope, Builder, EdgeGeometry, FaceRole,
    Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{dot, scale};
use crate::operations::modifying::boolean::assembly::{face_provenance, reverse_face};
use crate::operations::modifying::boolean::operands::brep_face_source;

struct HostCopy {
    vertices: Vec<u32>,
    edges: Vec<u32>,
    holes: [Option<u32>; 2],
}

pub(super) fn notch(
    host: &BrepEnvelope,
    tool: &BrepEnvelope,
    id: String,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let clearance = 4.0 * accuracy.geometric;
    let planes = convex_planes(host, clearance)?;
    let mut prism = prism(tool)?;
    if prism.loops.len() != 1 {
        return Err(gap());
    }
    let entries = clip(&mut prism, host, &planes, clearance)?;
    for (cap, entry) in prism.caps.iter_mut().zip(entries) {
        if entry.is_none() {
            cap.role = FaceRole::Cut;
            cap.reversed = true;
        }
    }
    let mut builder = Builder::new(id, accuracy)?;
    let rings = add_rings(&mut builder, &prism)?;
    let mut reversed = add_sides(&mut builder, &prism, &rings, FaceRole::Cut, true)?;
    for (cap, entry) in entries.iter().enumerate() {
        if entry.is_none() {
            reversed.push(add_cap(&mut builder, &prism, &rings, cap)?);
        }
    }
    let copy = host_copy(
        &mut builder,
        host,
        entries.map(|e| e.map(|i| planes[i].face)),
    )?;
    for face in &host.topology.faces {
        add_host_face(&mut builder, host, &copy, &rings, face.id)?;
    }
    for face in reversed {
        reverse_face(&mut builder.brep, face);
    }
    builder.finish_solid()
}

fn crossing(prism: &Prism, plane: &Plane, clearance: f64) -> Option<usize> {
    let along = dot(plane.outward, prism.frame.z);
    if along.abs() < 1.0e-6 {
        return None;
    }
    let height = cap_height(prism.frame, plane.point, plane.outward);
    let [low, high] = &prism.caps;
    let between = extent(profile(prism), height.minus(low.height))[0] > clearance
        && extent(profile(prism), high.height.minus(height))[0] > clearance;
    between.then_some(usize::from(along > 0.0))
}

fn inside(prism: &Prism, plane: &Plane, clearance: f64) -> bool {
    prism
        .caps
        .iter()
        .all(|cap| extent(profile(prism), plane_at_cap(prism, plane, cap.height))[1] < -clearance)
}

fn clip(
    prism: &mut Prism,
    host: &BrepEnvelope,
    planes: &[Plane],
    clearance: f64,
) -> Result<[Option<usize>; 2], GeometryError> {
    let mut entries = [None, None];
    let mut changed = true;
    while changed {
        changed = false;
        for (index, plane) in planes.iter().enumerate() {
            if entries.contains(&Some(index)) || inside(prism, plane, clearance) {
                continue;
            }
            if let Some(cap) = crossing(prism, plane, clearance) {
                prism.caps[cap] = Cap {
                    point: plane.point,
                    normal: plane.outward,
                    height: cap_height(prism.frame, plane.point, plane.outward),
                    source: brep_face_source(host, plane.face),
                    role: FaceRole::Split,
                    reversed: false,
                };
                entries[cap] = Some(index);
                changed = true;
            }
        }
    }
    let settled = planes
        .iter()
        .enumerate()
        .all(|(index, plane)| entries.contains(&Some(index)) || inside(prism, plane, clearance));
    let oblique = entries
        .iter()
        .flatten()
        .any(|&index| 1.0 - dot(planes[index].outward, prism.frame.z).abs() > 1.0e-9);
    if settled && oblique {
        Ok(entries)
    } else {
        Err(gap())
    }
}

fn host_copy(
    builder: &mut Builder,
    host: &BrepEnvelope,
    holes: [Option<u32>; 2],
) -> Result<HostCopy, GeometryError> {
    let vertices = host
        .topology
        .vertices
        .iter()
        .map(|vertex| builder.vertex(vertex.position))
        .collect();
    let mut edges = Vec::with_capacity(host.topology.edges.len());
    for edge in &host.topology.edges {
        let EdgeGeometry::Curve { curve, range } = &edge.geometry else {
            return Err(gap());
        };
        edges.push(builder.edge(host.geometry.curves[*curve as usize].clone(), *range, false));
    }
    Ok(HostCopy {
        vertices,
        edges,
        holes,
    })
}

fn add_host_face(
    builder: &mut Builder,
    host: &BrepEnvelope,
    copy: &HostCopy,
    rings: &[Ring],
    face: u32,
) -> Result<(), GeometryError> {
    let source = &host.topology.faces[face as usize];
    let plane = *host.geometry.surfaces[source.surface as usize].frame();
    let frame = if source.sense == Orientation::Reverse {
        Frame3 {
            y: scale(plane.y, -1.0),
            z: scale(plane.z, -1.0),
            ..plane
        }
    } else {
        plane
    };
    let mut loops = Vec::new();
    let mut corners = Vec::new();
    for start in face_loops(host, face) {
        let mut uses = Vec::new();
        for halfedge in loop_halfedges(host, start)? {
            let from = copy.vertices[halfedge.from as usize];
            let local = frame.local(host.topology.vertices[halfedge.from as usize].position);
            corners.push([local[0], local[1]]);
            uses.push(plane_boundary(
                builder,
                frame,
                copy.edges[halfedge.edge as usize],
                from,
                copy.vertices[halfedge.to as usize],
                halfedge.geometry_use.sense,
            )?);
        }
        loops.push(uses);
    }
    let holed = copy.holes.iter().position(|hole| *hole == Some(face));
    if let Some(cap) = holed {
        loops.extend(cap_loops(builder, frame, rings, cap, cap == 0)?);
    }
    let role = if holed.is_some() {
        FaceRole::Split
    } else {
        FaceRole::Preserved
    };
    add_face(
        builder,
        "notch-host",
        SurfaceGeometry::Plane { frame },
        padded_uv_bounds(corners, builder.brep.accuracy.geometric),
        loops,
        face_provenance(vec![brep_face_source(host, face)], role, false),
    )?;
    Ok(())
}
