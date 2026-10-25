use crate::brep::{BrepEnvelope, GeometryError, Orientation};

pub(super) fn reverse_cavities(brep: &mut BrepEnvelope) -> Result<bool, GeometryError> {
    let cavity_faces: Vec<u32> = brep
        .solids
        .iter()
        .flat_map(|solid| &solid.cavity_shells)
        .flat_map(|shell| brep.topology.shells[*shell as usize].faces.clone())
        .collect();
    if cavity_faces.is_empty() {
        return Ok(false);
    }
    for face in cavity_faces {
        let record = &mut brep.topology.faces[face as usize];
        record.sense = opposite(record.sense);
        for use_ in brep
            .topology
            .halfedges
            .iter_mut()
            .filter(|use_| use_.face == Some(face))
        {
            std::mem::swap(&mut use_.from, &mut use_.to);
            std::mem::swap(&mut use_.next, &mut use_.prev);
            use_.geometry_use.sense = opposite(use_.geometry_use.sense);
        }
    }
    for vertex in &mut brep.topology.vertices {
        vertex.outgoing_halfedge = None;
    }
    for use_ in &brep.topology.halfedges {
        brep.topology.vertices[use_.from as usize]
            .outgoing_halfedge
            .get_or_insert(use_.id);
    }
    brep.validate()?;
    Ok(true)
}

fn opposite(sense: Orientation) -> Orientation {
    if sense == Orientation::Forward {
        Orientation::Reverse
    } else {
        Orientation::Forward
    }
}
