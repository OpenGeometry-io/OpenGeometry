use super::sections::{local_ring, PlanarFace, PlanarFacet};
use crate::brep::{Accuracy, BrepEnvelope, FaceProvenance, FaceRole, Frame3, GeometryError};
use crate::geom2d::{boolean_oriented_regions, PlanarBooleanOp, Pt2};
use crate::math::{dot, sub, Point3};
use crate::operations::modifying::boolean::operands::brep_face_source;

type Inputs<'a> = [(&'a BrepEnvelope, &'a [PlanarFace]); 2];

struct Merge {
    members: Vec<usize>,
    facet: PlanarFacet,
}

pub(super) fn merged_coplanar(
    pieces: Vec<PlanarFacet>,
    inputs: Inputs<'_>,
    accuracy: Accuracy,
) -> Result<Vec<PlanarFacet>, GeometryError> {
    let mut grouped = vec![false; pieces.len()];
    let mut merges = Vec::new();
    for start in 0..pieces.len() {
        if grouped[start] {
            continue;
        }
        let frame = pieces[start].frame;
        let group = (start..pieces.len())
            .filter(|&index| !grouped[index] && same_plane(frame, pieces[index].frame, accuracy))
            .collect::<Vec<_>>();
        for &index in &group {
            grouped[index] = true;
        }
        merges.extend(coincident_faces(&pieces, &group, inputs, accuracy)?);
    }
    let mut slots = pieces.into_iter().map(Some).collect::<Vec<_>>();
    for merge in merges {
        for &member in &merge.members {
            slots[member] = None;
        }
        slots[merge.members[0]] = Some(merge.facet);
    }
    Ok(slots.into_iter().flatten().collect())
}

fn coincident_faces(
    pieces: &[PlanarFacet],
    group: &[usize],
    inputs: Inputs<'_>,
    accuracy: Accuracy,
) -> Result<Vec<Merge>, GeometryError> {
    let frame = pieces[group[0]].frame;
    let candidates = inputs
        .iter()
        .enumerate()
        .flat_map(|(input, &(body, faces))| {
            faces
                .iter()
                .filter(move |face| same_plane(frame, face.frame, accuracy))
                .map(move |face| (input, body, face))
        })
        .collect::<Vec<_>>();
    if (0..2).any(|input| candidates.iter().all(|candidate| candidate.0 != input)) {
        return Ok(Vec::new());
    }
    let local = |outer: &[Point3], holes: &[Vec<Point3>]| {
        std::iter::once(outer)
            .chain(holes.iter().map(Vec::as_slice))
            .map(|ring| local_ring(ring, frame))
            .collect::<Vec<_>>()
    };
    let member_rings = group
        .iter()
        .map(|&index| local(&pieces[index].outer, &pieces[index].holes))
        .collect::<Vec<_>>();
    let union = boolean_oriented_regions(
        &member_rings.concat(),
        &[],
        PlanarBooleanOp::Union,
        accuracy.intersection,
    )?;
    let mut merges = Vec::new();
    for region in union {
        let rings = std::iter::once(region.outer)
            .chain(region.holes)
            .collect::<Vec<_>>();
        let (mut sources, mut from) = (Vec::new(), [false; 2]);
        for &(input, body, face) in &candidates {
            if overlaps(&rings, &local(&face.polygon, &face.holes), accuracy)? {
                sources.push(brep_face_source(body, face.id));
                from[input] = true;
            }
        }
        let mut members = Vec::new();
        for (&index, member) in group.iter().zip(&member_rings) {
            if overlaps(&rings, member, accuracy)? {
                members.push(index);
            }
        }
        if from.contains(&false) || members.is_empty() {
            continue;
        }
        let lifted = lifted_rings(&rings, frame);
        let facet = PlanarFacet {
            key: pieces[members[0]].key.clone(),
            frame,
            outer: lifted[0].clone(),
            holes: lifted[1..].to_vec(),
            provenance: FaceProvenance {
                sources,
                role: FaceRole::Coincident,
                reversed: false,
            },
        };
        merges.push(Merge { members, facet });
    }
    Ok(merges)
}

fn lifted_rings(rings: &[Vec<Pt2>], frame: Frame3) -> Vec<Vec<Point3>> {
    rings
        .iter()
        .map(|ring| {
            ring.iter()
                .map(|point| frame.point([point.x, point.z, 0.0]))
                .collect()
        })
        .collect()
}

fn overlaps(a: &[Vec<Pt2>], b: &[Vec<Pt2>], accuracy: Accuracy) -> Result<bool, GeometryError> {
    Ok(
        !boolean_oriented_regions(a, b, PlanarBooleanOp::Intersection, accuracy.intersection)?
            .is_empty(),
    )
}

fn same_plane(frame: Frame3, other: Frame3, accuracy: Accuracy) -> bool {
    dot(frame.z, other.z) >= 1.0 - 1.0e-10
        && dot(frame.z, sub(other.origin, frame.origin)).abs() <= accuracy.intersection
}
