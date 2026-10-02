use crate::brep::{BrepEnvelope, FaceProvenance, FaceRole, FaceSource, Orientation};
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput, SphereInput};
use crate::operations::modifying::boolean::types::FaceMapping;

pub(crate) fn source(input: &SphereInput<'_>) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[0].key.clone(),
        face: 0,
    }
}

pub(crate) fn provenance(
    input: &SphereInput<'_>,
    role: FaceRole,
    reversed: bool,
) -> FaceProvenance {
    FaceProvenance {
        sources: vec![source(input)],
        role,
        reversed,
    }
}

pub(crate) fn reverse(sense: Orientation) -> Orientation {
    if sense == Orientation::Forward {
        Orientation::Reverse
    } else {
        Orientation::Forward
    }
}

pub(crate) fn reverse_face(brep: &mut BrepEnvelope, face: u32) {
    brep.topology.faces[face as usize].sense = reverse(brep.topology.faces[face as usize].sense);
    for h in &mut brep.topology.halfedges {
        if h.face == Some(face) {
            std::mem::swap(&mut h.from, &mut h.to);
            std::mem::swap(&mut h.next, &mut h.prev);
            h.geometry_use.sense = reverse(h.geometry_use.sense);
        }
    }
    for vertex in &mut brep.topology.vertices {
        vertex.outgoing_halfedge = brep
            .topology
            .halfedges
            .iter()
            .find(|h| h.from == vertex.id)
            .map(|h| h.id);
    }
}

pub(crate) fn box_source(input: &BoxInput<'_>, face: usize) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[face].key.clone(),
        face: face as u32,
    }
}

pub(crate) fn cylinder_source(input: &CylinderInput<'_>, face: u32) -> FaceSource {
    FaceSource {
        entity: input.brep.id.clone(),
        body: input.brep.id.clone(),
        key: input.brep.topology.faces[face as usize].key.clone(),
        face,
    }
}

pub(crate) fn face_provenance(
    sources: Vec<FaceSource>,
    role: FaceRole,
    reversed: bool,
) -> FaceProvenance {
    FaceProvenance {
        sources,
        role,
        reversed,
    }
}

pub(crate) fn cylinder_face_mappings(
    out: &BrepEnvelope,
    inputs: [&CylinderInput<'_>; 2],
) -> Vec<FaceMapping> {
    let mut mappings = Vec::with_capacity(6);
    for input in inputs {
        for face in 0..3 {
            let source = cylinder_source(input, face);
            let result_faces = out
                .topology
                .faces
                .iter()
                .filter(|result| {
                    result.provenance.sources.iter().any(|candidate| {
                        candidate.entity == source.entity
                            && candidate.body == source.body
                            && candidate.key == source.key
                            && candidate.face == source.face
                    })
                })
                .map(|result| result.id)
                .collect();
            mappings.push(FaceMapping {
                source,
                result_faces,
            });
        }
    }
    mappings
}
