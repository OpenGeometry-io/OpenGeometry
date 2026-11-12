use opengeometry::brep::{BrepEnvelope, FaceRole};
use opengeometry::math::Point3;
use opengeometry::operations::modifying::boolean::{boolean_brep_outcome_with_handlers, BooleanOp};
use opengeometry::query::{classify_point, validity, PointClassification};
use opengeometry::tessellation::tessellate;
use opengeometry::world_graph::{Primitive, Transform};
use opengeometry_test_support::volume::{self, VolumeEstimate};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const OFFSET: f64 = 1e-4;

const NEAR: f64 = 1e-7;

pub(super) struct Pair {
    a: BrepEnvelope,
    b: BrepEnvelope,
}

pub(super) struct Built {
    brep: BrepEnvelope,
    handlers: Vec<String>,
    report: Value,
    volume: VolumeEstimate,
}

struct FaceSample {
    face: u32,
    point: Point3,
    normal: Point3,
}

impl Pair {
    pub(super) fn new(a: Primitive, b: Primitive) -> Self {
        Self::placed([(a, &[]), (b, &[])])
    }

    pub(super) fn placed(parts: [(Primitive, &[Transform]); 2]) -> Self {
        let mut world = graph();
        for (og_id, (primitive, transforms)) in ["a", "b"].into_iter().zip(parts) {
            world.create_primitive(primitive, named(og_id)).unwrap();
            for transform in transforms {
                world.transform(og_id, *transform).unwrap();
            }
        }
        let [a, b] = ["a", "b"].map(|og_id| world.placed_brep(og_id).unwrap().as_ref().clone());
        assert_ne!(a.id, b.id);
        Self { a, b }
    }

    pub(super) fn built(&self, operation: BooleanOp, solids: usize) -> Built {
        let label = format!("{operation:?}");
        let (result, handlers) =
            boolean_brep_outcome_with_handlers(&self.a, &self.b, operation, label.clone());
        let result = result.unwrap_or_else(|error| panic!("{label} via {handlers:?}: {error:?}"));
        let report = serde_json::to_value(&result.report).unwrap();
        assert_eq!(report["operation"], json!(operation), "{label}");
        assert_closed_solids(&result.brep, solids, &label);
        assert_outward(&result.brep, &label);
        let volume = volume::estimate(&result.brep, 0.001);
        Built {
            brep: result.brep,
            handlers,
            report,
            volume,
        }
    }

    pub(super) fn planar(&self, operation: BooleanOp, solids: usize) -> Built {
        let built = self.built(operation, solids);
        assert_eq!(
            built.handlers,
            ["boolean_planar_polyhedra"],
            "{operation:?}"
        );
        for face in &built.brep.topology.faces {
            let provenance = &face.provenance;
            assert_ne!(provenance.role, FaceRole::Cut, "{operation:?} {}", face.id);
            assert!(!provenance.sources.is_empty(), "{operation:?} {}", face.id);
            assert!(
                provenance
                    .sources
                    .iter()
                    .all(|source| source.body == self.a.id || source.body == self.b.id),
                "{operation:?} {}",
                face.id
            );
        }
        for input in [&self.a, &self.b] {
            let every_face = (0..input.topology.faces.len() as u64).collect::<BTreeSet<_>>();
            assert_eq!(
                mapped_faces(&built.report, &input.id),
                every_face,
                "{operation:?}"
            );
        }
        built
    }

    pub(super) fn assert_touching_kept_apart(&self) {
        let [a, b] = [&self.a, &self.b].map(|body| volume::estimate(body, 0.001));
        let union = self.built(BooleanOp::Union, 2);
        assert_eq!(union.handlers, ["generic_non_intersecting_boolean"]);
        union.assert_volume(a.value + b.value);
        self.assert_empty_intersection();
        let difference = self.built(BooleanOp::Subtraction, 1);
        assert_eq!(difference.handlers, ["subtract_planar_polyhedra"]);
        difference.assert_volume(a.value);
    }

    pub(super) fn assert_empty_intersection(&self) {
        let operation = BooleanOp::Intersection;
        let (result, handlers) =
            boolean_brep_outcome_with_handlers(&self.a, &self.b, operation, "touching".into());
        let result = result.unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
        assert_eq!(handlers, ["boolean_planar_polyhedra"]);
        result.brep.validate().unwrap();
        let topology = &result.brep.topology;
        assert!(topology.vertices.is_empty() && topology.edges.is_empty());
        assert!(topology.faces.is_empty());
        let value = serde_json::to_value(&result.brep).unwrap();
        assert_eq!(value["solids"], json!([]));
        assert_eq!(value["topology"]["shells"], json!([]));
        let report = serde_json::to_value(&result.report).unwrap();
        assert_eq!(report["operation"], json!(operation));
    }

    pub(super) fn assert_identities(
        &self,
        union: &Built,
        intersection: &Built,
        difference: &Built,
    ) {
        let [a, b] = [&self.a, &self.b].map(|body| volume::estimate(body, 0.001));
        let [u, i, d] = [union, intersection, difference].map(|built| &built.volume);
        let union_slack = a.error_bound + b.error_bound + u.error_bound + i.error_bound + 1e-9;
        assert!(
            (u.value - (a.value + b.value - i.value)).abs() <= union_slack,
            "union {} versus {} + {} - {}",
            u.value,
            a.value,
            b.value,
            i.value
        );
        let difference_slack = a.error_bound + d.error_bound + i.error_bound + 1e-9;
        assert!(
            (d.value - (a.value - i.value)).abs() <= difference_slack,
            "difference {} versus {} - {}",
            d.value,
            a.value,
            i.value
        );
    }

    pub(super) fn assert_both_contribute(&self, built: &Built) {
        for input in [&self.a, &self.b] {
            assert!(
                built
                    .brep
                    .topology
                    .faces
                    .iter()
                    .any(|face| source_bodies(&built.brep, face.id).contains(&input.id)),
                "{}",
                input.id
            );
        }
    }

    pub(super) fn assert_edge_between_inputs(&self, built: &Built, ends: [Point3; 2]) {
        let faces = edge_faces(&built.brep, ends);
        assert_eq!(faces.len(), 2, "{ends:?}");
        let bodies = faces
            .into_iter()
            .map(|face| source_bodies(&built.brep, face))
            .collect::<Vec<_>>();
        let [a, b] = [&self.a.id, &self.b.id].map(|id| BTreeSet::from([id.clone()]));
        assert!(
            bodies == [a.clone(), b.clone()] || bodies == [b, a],
            "{ends:?}: {bodies:?}"
        );
    }

    pub(super) fn assert_one_coincident_face(&self, built: &Built, point: Point3, normal: Point3) {
        let faces = built.faces_on_plane(point, normal);
        assert_eq!(faces.len(), 1, "{faces:?}");
        let face = &built.brep.topology.faces[faces[0] as usize];
        assert_eq!(face.provenance.role, FaceRole::Coincident);
        assert_eq!(
            source_bodies(&built.brep, face.id),
            BTreeSet::from([self.a.id.clone(), self.b.id.clone()])
        );
    }
}

impl Built {
    pub(super) fn assert_volume(&self, expected: f64) {
        assert!(
            (self.volume.value - expected).abs() <= self.volume.error_bound,
            "{} versus {expected} with bound {}",
            self.volume.value,
            self.volume.error_bound
        );
    }

    pub(super) fn handlers(&self) -> &[String] {
        &self.handlers
    }

    pub(super) fn faces_on_plane(&self, point: Point3, normal: Point3) -> Vec<u32> {
        face_samples(&self.brep)
            .into_iter()
            .filter(|sample| {
                dot(sample.normal, normal) > 1.0 - NEAR
                    && dot(difference(sample.point, point), normal).abs() < NEAR
            })
            .map(|sample| sample.face)
            .collect()
    }

    pub(super) fn assert_vertex(&self, point: Point3) {
        assert!(
            self.brep
                .topology
                .vertices
                .iter()
                .any(|vertex| near(vertex.position, point)),
            "{point:?}"
        );
    }
}

fn assert_closed_solids(body: &BrepEnvelope, solids: usize, label: &str) {
    body.validate().unwrap();
    let checked = validity(body);
    assert!(checked.valid && checked.closed, "{label}: {checked:?}");
    let value = serde_json::to_value(body).unwrap();
    assert_eq!(value["solids"].as_array().unwrap().len(), solids, "{label}");
}

fn assert_outward(body: &BrepEnvelope, label: &str) {
    for sample in face_samples(body) {
        let side = |sign: f64| {
            let point = std::array::from_fn(|i| sample.point[i] + sign * OFFSET * sample.normal[i]);
            classify_point(body, point).unwrap()
        };
        assert_eq!(
            [side(1.0), side(-1.0)],
            [PointClassification::Outside, PointClassification::Inside],
            "{label} face {} at {:?}",
            sample.face,
            sample.point
        );
    }
}

fn face_samples(body: &BrepEnvelope) -> Vec<FaceSample> {
    let mesh = tessellate(body, 0.01, 1 << 20).unwrap();
    let corner =
        |index: u32| -> Point3 { std::array::from_fn(|i| mesh.positions[index as usize * 3 + i]) };
    let triangles = mesh
        .indices
        .chunks(3)
        .zip(&mesh.triangle_face_ids)
        .map(|(corners, face)| (*face, [0, 1, 2].map(|i| corner(corners[i]))))
        .collect::<Vec<_>>();
    body.topology
        .faces
        .iter()
        .map(|face| {
            let [p, q, r] = triangles
                .iter()
                .filter(|(owner, _)| *owner == face.id)
                .map(|(_, points)| *points)
                .max_by(|x, y| area(*x).total_cmp(&area(*y)))
                .unwrap_or_else(|| panic!("face {} has no triangle", face.id));
            let point = std::array::from_fn(|i| (p[i] + q[i] + r[i]) / 3.0);
            FaceSample {
                face: face.id,
                point,
                normal: body.face_normal_at(face.id, point).unwrap(),
            }
        })
        .collect()
}

fn edge_faces(body: &BrepEnvelope, ends: [Point3; 2]) -> Vec<u32> {
    let value = serde_json::to_value(body).unwrap();
    let halfedges = value["topology"]["halfedges"].as_array().unwrap();
    let position = |vertex: u32| body.topology.vertices[vertex as usize].position;
    body.topology
        .halfedges
        .iter()
        .zip(halfedges)
        .filter(|(halfedge, _)| {
            let found = [position(halfedge.from), position(halfedge.to)];
            (near(found[0], ends[0]) && near(found[1], ends[1]))
                || (near(found[0], ends[1]) && near(found[1], ends[0]))
        })
        .map(|(_, json)| json["face"].as_u64().unwrap() as u32)
        .collect()
}

fn source_bodies(body: &BrepEnvelope, face: u32) -> BTreeSet<String> {
    body.topology.faces[face as usize]
        .provenance
        .sources
        .iter()
        .map(|source| source.body.clone())
        .collect()
}

fn mapped_faces(report: &Value, body: &str) -> BTreeSet<u64> {
    report["face_mappings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|mapping| mapping["source"]["body"] == json!(body))
        .map(|mapping| mapping["source"]["face"].as_u64().unwrap())
        .collect()
}

fn area([p, q, r]: [Point3; 3]) -> f64 {
    let [u, v] = [difference(q, p), difference(r, p)];
    let normal = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    dot(normal, normal).sqrt() / 2.0
}

fn difference(p: Point3, q: Point3) -> Point3 {
    std::array::from_fn(|i| p[i] - q[i])
}

fn dot(p: Point3, q: Point3) -> f64 {
    (0..3).map(|i| p[i] * q[i]).sum()
}

fn near(p: Point3, q: Point3) -> bool {
    (0..3).all(|i| (p[i] - q[i]).abs() < NEAR)
}
