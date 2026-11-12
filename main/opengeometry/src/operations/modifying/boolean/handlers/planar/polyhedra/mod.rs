mod keep;
mod merge;
mod sections;

use super::facets::{coverage, finish, PlanarFacets};
use crate::brep::{Accuracy, BrepEnvelope, FaceProvenance, FaceRole, Frame3, GeometryError};
use crate::geom2d::{
    boolean_oriented_regions, regions_from_edges_by, winding_number2, PlanarBooleanOp, Pt2,
    RingRegion,
};
use crate::math::{dot, scale, sub, Point3};
use crate::operations::modifying::boolean::handlers::disjoint::separate_boolean;
use crate::operations::modifying::boolean::operands::brep_face_source;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::query::{Classifier, PointClassification};
use keep::{cutter_frame, open_boundary_gap, piece_role, turns_cutter, Keeper};
use merge::merged_coplanar;
use sections::{
    convex_faces, convex_section_polygon, local_ring, planar_faces, planar_section_segments,
    ring_vertex_candidates, split_planar_ring, PlanarFace, PlanarFacet,
};
use std::cell::RefCell;

pub(crate) fn boolean_planar_polyhedra(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    planar_polyhedra(host, cutter, operation, id)
        .map_err(|error| open_boundary_gap(operation, error))
}

fn planar_polyhedra(
    host: &BrepEnvelope,
    cutter: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy::combined(host.accuracy, cutter.accuracy);
    let Some(host_faces) = planar_faces(host, accuracy.intersection)? else {
        return Err(coverage());
    };
    let Some(cutter_faces) = planar_faces(cutter, accuracy.intersection)? else {
        return Err(coverage());
    };
    let convex_cutter_faces = if operation == BooleanOp::Subtraction {
        convex_faces(cutter, accuracy.intersection)?
    } else {
        None
    };
    check_not_nested(host, cutter)?;
    let keeper = Keeper::new(operation);
    let mut pieces = Vec::new();
    for face in &host_faces {
        let base = face_rings(face);
        let regions = if let Some(convex_faces) = &convex_cutter_faces {
            convex_host_regions(cutter, convex_faces, face, &base, accuracy)?
        } else {
            classified_host_regions(cutter, &cutter_faces, face, &base, accuracy, &keeper)?
        };
        for region in regions {
            let role = piece_role(operation, false, &region, &base, accuracy.geometric);
            pieces.push(host_piece(host, face, &region, role));
        }
    }
    let classifier = Classifier::new(host);
    for face in &cutter_faces {
        let base = face_rings(face);
        let regions = cutter_face_regions(
            host,
            &classifier,
            &host_faces,
            face,
            &base,
            accuracy,
            &keeper,
        )?;
        for region in regions {
            let role = piece_role(operation, true, &region, &base, accuracy.geometric);
            pieces.push(cutter_piece(cutter, face, operation, &region, role));
        }
    }
    if operation != BooleanOp::Subtraction && !keeper.shares_volume_or_face() {
        return match operation {
            BooleanOp::Intersection => separate_boolean(host, cutter, operation, id, Vec::new()),
            _ => Err(coverage()),
        };
    }
    if pieces.is_empty() {
        return Err(coverage());
    }
    if operation != BooleanOp::Subtraction {
        pieces = merged_coplanar(
            pieces,
            [(host, &host_faces), (cutter, &cutter_faces)],
            accuracy,
        )?;
    }
    let facets = welded_facets(pieces, id, accuracy)?;
    let mut result = finish(facets, host, cutter, operation)?;
    result.report.coincident = false;
    Ok(result)
}

fn welded_facets(
    pieces: Vec<PlanarFacet>,
    id: String,
    accuracy: Accuracy,
) -> Result<PlanarFacets, GeometryError> {
    let vertices = pieces
        .iter()
        .flat_map(|piece| {
            piece
                .outer
                .iter()
                .chain(piece.holes.iter().flatten())
                .copied()
        })
        .collect::<Vec<_>>();
    let tolerance = accuracy.geometric / 4.0;
    let rings = pieces
        .iter()
        .flat_map(|piece| std::iter::once(&piece.outer).chain(&piece.holes))
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let near = ring_vertex_candidates(&rings, &vertices, tolerance);
    let mut offset = 0;
    let mut split = |ring: &[Point3]| {
        offset += ring.len();
        split_planar_ring(ring, &vertices, &near, offset - ring.len(), tolerance)
    };
    let mut facets = PlanarFacets::new(id, accuracy)?;
    for piece in pieces {
        let outer = split(&piece.outer);
        let holes = piece.holes.iter().map(|ring| split(ring)).collect();
        facets.face(&piece.key, piece.frame, outer, holes, piece.provenance)?;
    }
    Ok(facets)
}

fn face_rings(face: &PlanarFace) -> Vec<Vec<Pt2>> {
    std::iter::once(&face.polygon)
        .chain(&face.holes)
        .map(|ring| local_ring(ring, face.frame))
        .collect()
}

fn check_not_nested(host: &BrepEnvelope, cutter: &BrepEnvelope) -> Result<(), GeometryError> {
    let (in_host, in_cutter) = (Classifier::new(host), Classifier::new(cutter));
    if cutter.topology.vertices.iter().all(|vertex| {
        matches!(
            in_host.classify(vertex.position),
            Ok(PointClassification::Inside)
        )
    }) || host.topology.vertices.iter().all(|vertex| {
        matches!(
            in_cutter.classify(vertex.position),
            Ok(PointClassification::Inside)
        )
    }) {
        return Err(coverage());
    }
    Ok(())
}

fn convex_host_regions(
    cutter: &BrepEnvelope,
    convex_faces: &[PlanarFace],
    face: &PlanarFace,
    base: &[Vec<Pt2>],
    accuracy: Accuracy,
) -> Result<Vec<RingRegion>, GeometryError> {
    let touches_from_outside = cutter.topology.vertices.iter().all(|vertex| {
        dot(face.frame.z, sub(vertex.position, face.frame.origin)) >= -accuracy.intersection
    });
    let section = if touches_from_outside {
        Vec::new()
    } else {
        convex_section_polygon(cutter, convex_faces, face.frame, accuracy.intersection)
    };
    let cut = if section.is_empty() {
        Vec::new()
    } else {
        vec![section]
    };
    Ok(boolean_oriented_regions(
        base,
        &cut,
        PlanarBooleanOp::Subtraction,
        accuracy.intersection,
    )?)
}

fn classified_host_regions(
    cutter: &BrepEnvelope,
    cutter_faces: &[PlanarFace],
    face: &PlanarFace,
    base: &[Vec<Pt2>],
    accuracy: Accuracy,
    keeper: &Keeper,
) -> Result<Vec<RingRegion>, GeometryError> {
    let mut edges =
        planar_section_segments(cutter, cutter_faces, face.frame, accuracy.intersection)?;
    let coplanar = coplanar_rings(cutter_faces, face, accuracy);
    for ring in base.iter().chain(&coplanar) {
        edges.extend(
            ring.iter()
                .enumerate()
                .map(|(index, &point)| (point, ring[(index + 1) % ring.len()])),
        );
    }
    let failure = RefCell::new(None);
    let classifier = Classifier::new(cutter);
    let regions = regions_from_edges_by(&edges, accuracy.intersection, |point| {
        if base
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            == 0
        {
            return false;
        }
        let position = face.frame.point([point.x, point.z, 0.0]);
        let classification = classifier.classify(position);
        let classification = match classification {
            Ok(PointClassification::Boundary) => classifier.classify(sub(
                position,
                scale(
                    face.frame.z,
                    keeper.probe_side() * accuracy.geometric.max(accuracy.intersection) * 4.0,
                ),
            )),
            other => other,
        };
        match classification.map(|place| keeper.host(place)) {
            Ok(Some(keep)) => keep,
            Ok(None) => {
                *failure.borrow_mut() = Some(coverage());
                false
            }
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                false
            }
        }
    });
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    Ok(regions?)
}

fn coplanar_rings(faces: &[PlanarFace], face: &PlanarFace, accuracy: Accuracy) -> Vec<Vec<Pt2>> {
    faces
        .iter()
        .filter(|other| {
            dot(other.frame.z, face.frame.z).abs() >= 1.0 - 1.0e-10
                && dot(other.frame.z, sub(face.frame.origin, other.frame.origin)).abs()
                    <= accuracy.intersection
        })
        .flat_map(|other| {
            std::iter::once(&other.polygon)
                .chain(other.holes.iter())
                .map(|ring| local_ring(ring, face.frame))
        })
        .collect::<Vec<_>>()
}

fn host_piece(
    host: &BrepEnvelope,
    face: &PlanarFace,
    region: &RingRegion,
    role: FaceRole,
) -> PlanarFacet {
    PlanarFacet {
        key: format!("planar-host-{}", face.id),
        frame: face.frame,
        outer: lifted_ring(&region.outer, face.frame, false),
        holes: region
            .holes
            .iter()
            .map(|ring| lifted_ring(ring, face.frame, false))
            .collect(),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(host, face.id)],
            role,
            reversed: false,
        },
    }
}

fn cutter_face_regions(
    host: &BrepEnvelope,
    classifier: &Classifier,
    host_faces: &[PlanarFace],
    face: &PlanarFace,
    cutter_region: &[Vec<Pt2>],
    accuracy: Accuracy,
    keeper: &Keeper,
) -> Result<Vec<RingRegion>, GeometryError> {
    let mut edges = planar_section_segments(host, host_faces, face.frame, accuracy.intersection)?;
    let coplanar = coplanar_rings(host_faces, face, accuracy);
    for ring in cutter_region.iter().chain(&coplanar) {
        edges.extend(
            ring.iter()
                .enumerate()
                .map(|(index, &point)| (point, ring[(index + 1) % ring.len()])),
        );
    }
    let failure = RefCell::new(None);
    let regions = regions_from_edges_by(&edges, accuracy.intersection, |point| {
        if cutter_region
            .iter()
            .map(|ring| winding_number2(point, ring))
            .sum::<i32>()
            == 0
            || coplanar
                .iter()
                .map(|ring| winding_number2(point, ring))
                .sum::<i32>()
                != 0
        {
            return false;
        }
        match classifier
            .classify(face.frame.point([point.x, point.z, 0.0]))
            .map(|place| keeper.cutter(place))
        {
            Ok(Some(keep)) => keep,
            Ok(None) => {
                *failure.borrow_mut() = Some(coverage());
                false
            }
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                false
            }
        }
    });
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    Ok(regions?)
}

fn cutter_piece(
    cutter: &BrepEnvelope,
    face: &PlanarFace,
    operation: BooleanOp,
    region: &RingRegion,
    role: FaceRole,
) -> PlanarFacet {
    let reversed = turns_cutter(operation);
    PlanarFacet {
        key: format!("planar-cut-{}", face.id),
        frame: cutter_frame(operation, face.frame),
        outer: lifted_ring(&region.outer, face.frame, reversed),
        holes: region
            .holes
            .iter()
            .map(|ring| lifted_ring(ring, face.frame, reversed))
            .collect(),
        provenance: FaceProvenance {
            sources: vec![brep_face_source(cutter, face.id)],
            role,
            reversed,
        },
    }
}

fn lifted_ring(ring: &[Pt2], frame: Frame3, reversed: bool) -> Vec<Point3> {
    let mut points = ring
        .iter()
        .map(|point| frame.point([point.x, point.z, 0.0]))
        .collect::<Vec<_>>();
    if reversed {
        points.reverse();
    }
    points
}
