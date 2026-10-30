use super::curves::EdgeEntities;
use super::entities::{boolean, enumeration, reference, references, Entities, IfcValue};
use crate::brep::{BrepEnvelope, Face, GeometryError};

pub(super) fn add_faces(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    surfaces: &[Option<u32>],
    edges: &EdgeEntities,
) -> Result<Vec<u32>, GeometryError> {
    let mut faces = Vec::new();
    for face in &brep.topology.faces {
        let bounds = add_bounds(entities, brep, face, edges)?;
        let surface = surfaces[face.surface as usize]
            .ok_or_else(|| GeometryError::InvalidTopology("missing prepared IFC surface".into()))?;
        faces.push(entities.add(
            "IfcAdvancedFace",
            vec![references(bounds), reference(surface), boolean(face.sense)],
        ));
    }
    Ok(faces)
}

fn add_bounds(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    face: &Face,
    edges: &EdgeEntities,
) -> Result<Vec<u32>, GeometryError> {
    let mut bounds = Vec::new();
    for loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
        let loop_ = &brep.topology.loops[*loop_id as usize];
        let uses = add_oriented_edges(entities, brep, loop_.start_halfedge, edges)?;
        let loop_entity = if uses.is_empty() {
            let start = &brep.topology.halfedges[loop_.start_halfedge as usize];
            entities.add(
                "IfcVertexLoop",
                vec![reference(edges.vertices[start.from as usize])],
            )
        } else {
            entities.add("IfcEdgeLoop", vec![references(uses)])
        };
        bounds.push(entities.add(
            if loop_.is_hole {
                "IfcFaceBound"
            } else {
                "IfcFaceOuterBound"
            },
            vec![reference(loop_entity), IfcValue::Bool { value: true }],
        ));
    }
    Ok(bounds)
}

fn add_oriented_edges(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    start: u32,
    edges: &EdgeEntities,
) -> Result<Vec<u32>, GeometryError> {
    let mut current = start;
    let mut uses = Vec::new();
    for _ in 0..=brep.topology.halfedges.len() {
        let use_ = &brep.topology.halfedges[current as usize];
        if let Some(edge) = edges.edges[use_.edge as usize] {
            uses.push(entities.add(
                "IfcOrientedEdge",
                vec![
                    IfcValue::Derived,
                    IfcValue::Derived,
                    reference(edge),
                    boolean(use_.geometry_use.sense),
                ],
            ));
        }
        current = use_
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open IFC loop".into()))?;
        if current == start {
            break;
        }
    }
    Ok(uses)
}

pub(super) fn add_solids(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    faces: &[u32],
    has_cavities: bool,
) -> Vec<u32> {
    let shells: Vec<_> = brep
        .topology
        .shells
        .iter()
        .map(|shell| {
            entities.add(
                "IfcClosedShell",
                vec![references(shell.faces.iter().map(|id| faces[*id as usize]))],
            )
        })
        .collect();
    let solids: Vec<_> = brep
        .solids
        .iter()
        .map(|solid| {
            let mut root = entities.add(
                "IfcAdvancedBrep",
                vec![reference(shells[solid.outer_shell as usize])],
            );
            for shell in &solid.cavity_shells {
                let cutter =
                    entities.add("IfcAdvancedBrep", vec![reference(shells[*shell as usize])]);
                root = add_boolean(entities, "DIFFERENCE", root, cutter);
            }
            root
        })
        .collect();
    if !has_cavities || solids.len() < 2 {
        return solids;
    }
    let mut root = solids[0];
    for id in &solids[1..] {
        root = add_boolean(entities, "UNION", root, *id);
    }
    vec![root]
}

fn add_boolean(entities: &mut Entities, operator: &str, first: u32, second: u32) -> u32 {
    entities.add(
        "IfcBooleanResult",
        vec![enumeration(operator), reference(first), reference(second)],
    )
}
