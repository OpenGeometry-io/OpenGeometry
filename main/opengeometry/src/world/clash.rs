use super::graph::{Geometry, WorldError, WorldGraph};
use super::pairs::Leaf;
use crate::analytic::face_intersection::intersect_breps;
use crate::analytic::geometry::{add, norm, scale};
use crate::analytic::placement::placed;
use crate::analytic::query::{classify_point, PointClassification};
use crate::analytic::{BrepEnvelope, Curve, GeometryError, Point3};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClashKind {
    Touch,
    Hard,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Clash {
    pub a: String,
    pub b: String,
    pub kind: ClashKind,
    pub parts: [String; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl WorldGraph {
    pub fn clash(
        &self,
        items_a: &[String],
        items_b: &[String],
        depth: f64,
    ) -> Result<Vec<Clash>, WorldError> {
        if !depth.is_finite() || depth <= 0.0 {
            return Err(WorldError::InvalidInput(
                "clash depth must be finite and positive".into(),
            ));
        }
        let candidates = self.candidate_pairs(items_a, items_b, 0.0)?;
        let mut results: HashMap<(usize, usize), Clash> = HashMap::new();
        for &(index_a, index_b) in &candidates.pairs {
            let key = candidates.item_key(index_a, index_b);
            if results
                .get(&key)
                .is_some_and(|clash| clash.kind == ClashKind::Hard)
            {
                continue;
            }
            let (leaf_a, leaf_b) = (&candidates.leaves[index_a], &candidates.leaves[index_b]);
            let (kind, detail) = match self.classify_pair(leaf_a, leaf_b, depth) {
                Ok(Some(kind)) => (kind, None),
                Ok(None) => continue,
                Err(error) => (ClashKind::Unresolved, Some(format!("{error:?}"))),
            };
            let candidate = Clash {
                a: candidates.items[leaf_a.item].clone(),
                b: candidates.items[leaf_b.item].clone(),
                kind,
                parts: [leaf_a.node.clone(), leaf_b.node.clone()],
                detail,
            };
            match results.get(&key) {
                Some(existing) if existing.kind >= kind => {}
                _ => {
                    results.insert(key, candidate);
                }
            }
        }
        let mut clashes: Vec<Clash> = results.into_values().collect();
        clashes.sort_by(|x, y| (&x.a, &x.b).cmp(&(&y.a, &y.b)));
        Ok(clashes)
    }

    fn classify_pair(
        &self,
        a: &Leaf,
        b: &Leaf,
        depth: f64,
    ) -> Result<Option<ClashKind>, GeometryError> {
        let brep_a = self.definition_brep(&a.definition)?;
        let brep_b = self.placed_relative(a, b)?;
        let local_depth = depth / a.world.scale;
        let minimum = 16.0 * brep_a.accuracy.geometric.max(brep_b.accuracy.geometric);
        if local_depth <= minimum {
            return Err(GeometryError::InvalidGeometry(format!(
                "clash depth {depth} is below the geometric tolerance of the bodies"
            )));
        }
        let graph = intersect_breps(brep_a, &brep_b)?;
        let containment = [
            vertex_containment(brep_a, &brep_b)?,
            vertex_containment(&brep_b, brep_a)?,
        ];
        if containment.contains(&Containment::Inside) {
            return Ok(Some(ClashKind::Hard));
        }
        if graph.pairs.is_empty() {
            if containment.contains(&Containment::Undecided) {
                return Err(GeometryError::UnresolvedIntersection(
                    "containment could not be decided from any vertex".into(),
                ));
            }
            return Ok(None);
        }
        for pair in &graph.pairs {
            for branch in &pair.graph.branches {
                let curve = pair.graph.geometry.curve(branch.curve)?;
                let midpoint = curve.point_at(branch.range.lo / 2.0 + branch.range.hi / 2.0)?;
                let normal_a = brep_a.face_normal_at(pair.faces[0], midpoint)?;
                let normal_b = brep_b.face_normal_at(pair.faces[1], midpoint)?;
                let outward = add(normal_a, normal_b);
                let length = norm(outward);
                if length <= 1e-9 {
                    continue;
                }
                let probe = add(midpoint, scale(outward, -local_depth / length));
                if inside(brep_a, probe)? && inside(&brep_b, probe)? {
                    return Ok(Some(ClashKind::Hard));
                }
            }
        }
        Ok(Some(ClashKind::Touch))
    }

    pub(super) fn placed_relative(
        &self,
        a: &Leaf,
        b: &Leaf,
    ) -> Result<BrepEnvelope, GeometryError> {
        let relative = a.world.inverse().compose(&b.world);
        placed(
            self.definition_brep(&b.definition)?,
            relative.frame,
            relative.scale,
        )
    }

    pub(super) fn contains_either(&self, a: &Leaf, b: &Leaf) -> Result<bool, GeometryError> {
        let brep_a = self.definition_brep(&a.definition)?;
        let brep_b = self.placed_relative(a, b)?;
        Ok(vertex_containment(brep_a, &brep_b)? == Containment::Inside
            || vertex_containment(&brep_b, brep_a)? == Containment::Inside)
    }

    pub(super) fn definition_brep(&self, id: &str) -> Result<&BrepEnvelope, GeometryError> {
        match self
            .definitions
            .get(id)
            .map(|definition| &definition.geometry)
        {
            Some(Geometry::Analytic(brep)) => Ok(brep),
            Some(Geometry::Legacy(_)) => Err(GeometryError::UnsupportedGeometry(format!(
                "definition {id} is a legacy BRep; clash and clearance need analytic geometry"
            ))),
            None => Err(GeometryError::InvalidTopology(format!(
                "unknown definition {id}"
            ))),
        }
    }
}

fn inside(brep: &BrepEnvelope, point: Point3) -> Result<bool, GeometryError> {
    match classify_point(brep, point)? {
        PointClassification::Inside => Ok(true),
        PointClassification::Outside | PointClassification::Boundary => Ok(false),
        PointClassification::Unknown => Err(GeometryError::UnresolvedIntersection(
            "point classification was inconclusive".into(),
        )),
    }
}

#[derive(PartialEq)]
enum Containment {
    Inside,
    NotInside,
    Undecided,
}

fn vertex_containment(
    container: &BrepEnvelope,
    body: &BrepEnvelope,
) -> Result<Containment, GeometryError> {
    if body.topology.vertices.is_empty() {
        return Err(GeometryError::UnsupportedGeometry(
            "containment test needs a body with vertices".into(),
        ));
    }
    let mut decided = false;
    for vertex in &body.topology.vertices {
        match classify_point(container, vertex.position)? {
            PointClassification::Inside => return Ok(Containment::Inside),
            PointClassification::Outside | PointClassification::Boundary => decided = true,
            PointClassification::Unknown => {}
        }
    }
    Ok(if decided {
        Containment::NotInside
    } else {
        Containment::Undecided
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::primitives;
    use crate::analytic::topology::Accuracy;
    use crate::analytic::Frame3;
    use crate::world::{NodeSpec, Similarity3};

    const DEPTH: f64 = 1e-3;

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        }
    }

    fn at(x: f64, y: f64, z: f64) -> Similarity3 {
        Similarity3 {
            frame: Frame3 {
                origin: [x, y, z],
                ..Frame3::IDENTITY
            },
            scale: 1.0,
        }
    }

    fn quarter_turn_about_z(x: f64, y: f64, z: f64) -> Similarity3 {
        Similarity3 {
            frame: Frame3 {
                origin: [x, y, z],
                x: [0.0, 1.0, 0.0],
                y: [-1.0, 0.0, 0.0],
                z: [0.0, 0.0, 1.0],
            },
            scale: 1.0,
        }
    }

    fn graph_with_shapes() -> WorldGraph {
        let mut graph = WorldGraph::new();
        graph
            .define(
                "unit",
                primitives::cuboid("unit".into(), Frame3::IDENTITY, [1.0, 1.0, 1.0], accuracy())
                    .unwrap(),
            )
            .unwrap();
        graph
            .define(
                "small",
                primitives::cuboid(
                    "small".into(),
                    Frame3::IDENTITY,
                    [0.2, 0.2, 0.2],
                    accuracy(),
                )
                .unwrap(),
            )
            .unwrap();
        graph
            .define(
                "post",
                primitives::cylinder("post".into(), Frame3::IDENTITY, 0.25, 1.0, accuracy())
                    .unwrap(),
            )
            .unwrap();
        graph
    }

    fn place(
        graph: &mut WorldGraph,
        id: &str,
        parent: Option<&str>,
        definition: Option<&str>,
        local: Similarity3,
    ) {
        graph
            .add_node(NodeSpec {
                id: id.into(),
                parent: parent.map(str::to_string),
                definition: definition.map(str::to_string),
                local,
                kind: None,
            })
            .unwrap();
    }

    fn kinds(graph: &WorldGraph, a: &[&str], b: &[&str]) -> Vec<(String, String, ClashKind)> {
        let owned = |ids: &[&str]| ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        graph
            .clash(&owned(a), &owned(b), DEPTH)
            .unwrap()
            .into_iter()
            .map(|clash| (clash.a, clash.b, clash.kind))
            .collect()
    }

    fn pair(a: &str, b: &str, kind: ClashKind) -> Vec<(String, String, ClashKind)> {
        vec![(a.into(), b.into(), kind)]
    }

    #[test]
    fn separated_bodies_do_not_clash() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("unit"), at(1.5, 0.0, 0.0));
        assert!(kinds(&graph, &["a"], &["b"]).is_empty());
    }

    #[test]
    fn face_to_face_contact_is_a_touch() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("unit"), at(1.0, 0.25, 0.0));
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Touch)
        );
    }

    #[test]
    fn edge_contact_is_a_touch() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("unit"), at(1.0, 1.0, 0.0));
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Touch)
        );
    }

    #[test]
    fn overlapping_bodies_are_a_hard_clash() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("unit"), at(0.6, 0.3, 0.2));
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Hard)
        );
    }

    #[test]
    fn a_body_inside_another_is_a_hard_clash() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("small"), at(0.4, 0.4, 0.4));
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Hard)
        );
        assert_eq!(
            kinds(&graph, &["b"], &["a"]),
            pair("b", "a", ClashKind::Hard)
        );
    }

    #[test]
    fn a_flush_body_inside_another_is_a_hard_clash() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("small"), at(0.4, 0.4, 0.8));
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Hard)
        );
    }

    #[test]
    fn an_overlap_shallower_than_the_depth_is_not_hard() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(
            &mut graph,
            "b",
            None,
            Some("unit"),
            at(1.0 - DEPTH / 4.0, 0.0, 0.0),
        );
        assert_eq!(
            kinds(&graph, &["a"], &["b"]),
            pair("a", "b", ClashKind::Touch)
        );
    }

    #[test]
    fn clashes_follow_parent_transforms() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(
            &mut graph,
            "group",
            None,
            None,
            quarter_turn_about_z(2.5, 0.0, 0.0),
        );
        place(
            &mut graph,
            "b",
            Some("group"),
            Some("unit"),
            at(0.0, 0.0, 0.0),
        );
        assert!(kinds(&graph, &["a"], &["group"]).is_empty());
        graph
            .set_local("group", quarter_turn_about_z(1.5, 0.2, 0.0))
            .unwrap();
        assert_eq!(
            kinds(&graph, &["a"], &["group"]),
            pair("a", "group", ClashKind::Hard)
        );
    }

    #[test]
    fn parts_of_one_item_are_not_tested_against_each_other() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "cabinet", None, None, at(0.0, 0.0, 0.0));
        place(
            &mut graph,
            "cabinet/left",
            Some("cabinet"),
            Some("unit"),
            at(0.0, 0.0, 0.0),
        );
        place(
            &mut graph,
            "cabinet/right",
            Some("cabinet"),
            Some("unit"),
            at(0.5, 0.0, 0.0),
        );
        place(&mut graph, "chair", None, Some("small"), at(3.0, 0.0, 0.0));
        assert!(kinds(&graph, &["cabinet", "chair"], &["cabinet", "chair"]).is_empty());
        graph.set_local("chair", at(1.3, 0.4, 0.4)).unwrap();
        assert_eq!(
            kinds(&graph, &["cabinet", "chair"], &["cabinet", "chair"]),
            pair("cabinet", "chair", ClashKind::Hard)
        );
    }

    #[test]
    fn nested_items_are_skipped() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "room", None, None, at(0.0, 0.0, 0.0));
        place(
            &mut graph,
            "box",
            Some("room"),
            Some("unit"),
            at(0.0, 0.0, 0.0),
        );
        assert!(kinds(&graph, &["room"], &["box"]).is_empty());
    }

    #[test]
    fn a_cylinder_through_a_box_is_a_hard_clash() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "box", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "post", None, Some("post"), at(1.1, 0.5, 0.0));
        assert_eq!(
            kinds(&graph, &["box"], &["post"]),
            pair("box", "post", ClashKind::Hard)
        );
        graph.set_local("post", at(1.4, 0.5, 0.0)).unwrap();
        assert!(kinds(&graph, &["box"], &["post"]).is_empty());
    }

    #[test]
    fn a_depth_below_geometric_tolerance_is_rejected() {
        let mut graph = graph_with_shapes();
        place(&mut graph, "a", None, Some("unit"), at(0.0, 0.0, 0.0));
        place(&mut graph, "b", None, Some("unit"), at(0.5, 0.0, 0.0));
        let clashes = graph.clash(&["a".into()], &["b".into()], 1e-9).unwrap();
        assert_eq!(clashes[0].kind, ClashKind::Unresolved);
        assert!(graph.clash(&["a".into()], &["b".into()], 0.0).is_err());
    }
}
