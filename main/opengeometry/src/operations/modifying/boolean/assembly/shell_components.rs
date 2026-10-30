use crate::brep::{BrepEnvelope, GeometryError};
use crate::math::Point3;
use std::collections::VecDeque;

pub(crate) fn assign_shell_faces(
    brep: &mut BrepEnvelope,
    assigned: &mut [bool],
    start: usize,
    shell: u32,
    open_boundary: impl Fn() -> GeometryError,
    faceless_twin: impl Fn() -> GeometryError,
) -> Result<Vec<u32>, GeometryError> {
    let mut faces = Vec::new();
    let mut queue = VecDeque::from([start]);
    assigned[start] = true;
    while let Some(face) = queue.pop_front() {
        brep.topology.faces[face].shell_ref = Some(shell);
        faces.push(face as u32);
        for halfedge in brep
            .topology
            .halfedges
            .iter()
            .filter(|edge| edge.face == Some(face as u32))
        {
            let twin = halfedge.twin.ok_or_else(&open_boundary)?;
            let adjacent = brep.topology.halfedges[twin as usize]
                .face
                .ok_or_else(&faceless_twin)? as usize;
            if !assigned[adjacent] {
                assigned[adjacent] = true;
                queue.push_back(adjacent);
            }
        }
    }
    Ok(faces)
}

pub(crate) fn enclosing_solid(
    brep: &BrepEnvelope,
    outer_shells: &[(u32, f64)],
    cavity_shell: u32,
    contains: impl Fn(&BrepEnvelope, &[u32], Point3) -> Result<bool, GeometryError>,
    unenclosed: impl Fn() -> GeometryError,
) -> Result<usize, GeometryError> {
    let shell = &brep.topology.shells[cavity_shell as usize];
    let face = &brep.topology.faces[shell.faces[0] as usize];
    let loop_start = brep.topology.loops[face.trim.outer as usize].start_halfedge;
    let vertex = brep.topology.halfedges[loop_start as usize].from;
    let point = brep.topology.vertices[vertex as usize].position;
    let mut enclosing = Vec::new();
    for (index, &(outer_shell, volume)) in outer_shells.iter().enumerate() {
        let outer = &brep.topology.shells[outer_shell as usize];
        if contains(brep, &outer.faces, point)? {
            enclosing.push((index, volume));
        }
    }
    let Some(&(index, _)) = enclosing
        .iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
    else {
        return Err(unenclosed());
    };
    Ok(index)
}
