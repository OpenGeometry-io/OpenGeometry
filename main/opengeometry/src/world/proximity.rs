use super::bvh::{overlaps, Bvh};
use super::graph::{WorldError, WorldGraph};
use super::pairs::{inflate, ItemPairs, Leaf};
use super::triangle::{triangle_distance, Triangle};
use crate::analytic::geometry::{norm, sub};
use crate::analytic::tessellation::tessellate;
use crate::analytic::Point3;
use serde::Serialize;
use std::collections::HashMap;

const MAX_TRIANGLES_PER_PART: usize = 2_000_000;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proximity {
    pub a: String,
    pub b: String,
    pub distance: f64,
    pub error_bound: f64,
    pub parts: [String; 2],
    pub points: [Point3; 2],
}

struct Mesh {
    triangles: Vec<Triangle>,
    bvh: Bvh,
    error: f64,
}

impl WorldGraph {
    pub fn clearance(
        &self,
        items_a: &[String],
        items_b: &[String],
        clearance: f64,
    ) -> Result<Vec<Proximity>, WorldError> {
        if !clearance.is_finite() || clearance <= 0.0 {
            return Err(WorldError::InvalidInput(
                "clearance must be finite and positive".into(),
            ));
        }
        let candidates = self.candidate_pairs(items_a, items_b, clearance)?;
        Ok(self
            .nearest_per_item_pair(&candidates, clearance, clearance / 20.0)?
            .into_iter()
            .filter(|proximity| proximity.distance < clearance)
            .collect())
    }

    pub fn distance(
        &self,
        item_a: &str,
        item_b: &str,
        tolerance: f64,
    ) -> Result<Option<Proximity>, WorldError> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(WorldError::InvalidInput(
                "distance tolerance must be finite and positive".into(),
            ));
        }
        let candidates = self.candidate_pairs(&[item_a.into()], &[item_b.into()], f64::INFINITY)?;
        Ok(self
            .nearest_per_item_pair(&candidates, f64::INFINITY, tolerance)?
            .into_iter()
            .next())
    }

    fn nearest_per_item_pair(
        &self,
        candidates: &ItemPairs,
        reach: f64,
        tolerance: f64,
    ) -> Result<Vec<Proximity>, WorldError> {
        let mut meshes: HashMap<usize, Mesh> = HashMap::new();
        let mut nearest: HashMap<(usize, usize), Proximity> = HashMap::new();
        for &(index_a, index_b) in &candidates.pairs {
            let key = candidates.item_key(index_a, index_b);
            let limit = nearest
                .get(&key)
                .map_or(reach, |proximity| proximity.distance.min(reach));
            if limit == 0.0 {
                continue;
            }
            let (leaf_a, leaf_b) = (&candidates.leaves[index_a], &candidates.leaves[index_b]);
            for index in [index_a, index_b] {
                if !meshes.contains_key(&index) {
                    meshes.insert(
                        index,
                        self.world_mesh(&candidates.leaves[index], tolerance)?,
                    );
                }
            }
            let (mesh_a, mesh_b) = (&meshes[&index_a], &meshes[&index_b]);
            let surface = mesh_distance(mesh_a, mesh_b, limit);
            let contained = surface.as_ref().is_none_or(|(distance, _)| *distance > 0.0)
                && overlaps(&leaf_a.bounds, &leaf_b.bounds)
                && self.contains_either(leaf_a, leaf_b)?;
            let (distance, points) = match (surface, contained) {
                (_, true) => {
                    let inside = overlap_center(&leaf_a.bounds, &leaf_b.bounds);
                    (0.0, [inside, inside])
                }
                (Some(found), false) => found,
                (None, false) => continue,
            };
            nearest.insert(
                key,
                Proximity {
                    a: candidates.items[leaf_a.item].clone(),
                    b: candidates.items[leaf_b.item].clone(),
                    distance,
                    error_bound: mesh_a.error + mesh_b.error,
                    parts: [leaf_a.node.clone(), leaf_b.node.clone()],
                    points,
                },
            );
        }
        let mut proximities: Vec<Proximity> = nearest.into_values().collect();
        proximities.sort_by(|x, y| (&x.a, &x.b).cmp(&(&y.a, &y.b)));
        Ok(proximities)
    }

    fn world_mesh(&self, leaf: &Leaf, tolerance: f64) -> Result<Mesh, WorldError> {
        let brep = self.definition_brep(&leaf.definition)?;
        let deflection = tolerance / 2.0 / leaf.world.scale;
        if deflection <= 2.0 * brep.accuracy.geometric {
            return Err(WorldError::InvalidInput(format!(
                "tolerance {tolerance} is below the geometric tolerance of {}",
                leaf.node
            )));
        }
        let mesh = tessellate(brep, deflection, MAX_TRIANGLES_PER_PART)?;
        let point = |index: u32| -> Point3 {
            let offset = index as usize * 3;
            leaf.world.apply_point([
                mesh.positions[offset],
                mesh.positions[offset + 1],
                mesh.positions[offset + 2],
            ])
        };
        let triangles: Vec<Triangle> = mesh
            .indices
            .chunks_exact(3)
            .map(|corner| [point(corner[0]), point(corner[1]), point(corner[2])])
            .collect();
        let bvh = Bvh::build(triangles.iter().map(triangle_bounds).collect());
        Ok(Mesh {
            triangles,
            bvh,
            error: mesh.achieved_deflection * leaf.world.scale,
        })
    }
}

fn mesh_distance(a: &Mesh, b: &Mesh, limit: f64) -> Option<(f64, [Point3; 2])> {
    let (first_a, first_b) = (a.triangles.first()?, b.triangles.first()?);
    let mut bound = if limit.is_finite() {
        limit
    } else {
        norm(sub(first_a[0], first_b[0]))
    };
    let mut best = None;
    let mut hits = Vec::new();
    for triangle in &a.triangles {
        hits.clear();
        b.bvh
            .query(&inflate(triangle_bounds(triangle), bound), &mut hits);
        for &index in &hits {
            let (distance, on_a, on_b) = triangle_distance(*triangle, b.triangles[index]);
            if distance <= bound {
                bound = distance;
                best = Some((distance, [on_a, on_b]));
                if distance == 0.0 {
                    return best;
                }
            }
        }
    }
    best
}

fn overlap_center(a: &[f64; 6], b: &[f64; 6]) -> Point3 {
    std::array::from_fn(|axis| a[axis].max(b[axis]) / 2.0 + a[axis + 3].min(b[axis + 3]) / 2.0)
}

fn triangle_bounds(triangle: &Triangle) -> [f64; 6] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in triangle {
        for axis in 0..3 {
            bounds[axis] = bounds[axis].min(point[axis]);
            bounds[axis + 3] = bounds[axis + 3].max(point[axis]);
        }
    }
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::primitives;
    use crate::analytic::topology::Accuracy;
    use crate::analytic::Frame3;
    use crate::world::{NodeSpec, Similarity3};

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

    fn graph_with(bodies: &[(&str, &str, Similarity3)]) -> WorldGraph {
        let mut graph = WorldGraph::new();
        graph
            .define(
                "unit",
                primitives::cuboid("unit".into(), Frame3::IDENTITY, [1.0; 3], accuracy()).unwrap(),
            )
            .unwrap();
        graph
            .define(
                "small",
                primitives::cuboid("small".into(), Frame3::IDENTITY, [0.2; 3], accuracy()).unwrap(),
            )
            .unwrap();
        graph
            .define(
                "post",
                primitives::cylinder("post".into(), Frame3::IDENTITY, 0.25, 1.0, accuracy())
                    .unwrap(),
            )
            .unwrap();
        for (id, definition, local) in bodies {
            graph
                .add_node(NodeSpec {
                    id: (*id).into(),
                    parent: None,
                    definition: Some((*definition).into()),
                    local: *local,
                    kind: None,
                })
                .unwrap();
        }
        graph
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn planar_bodies_report_their_exact_gap() {
        let graph = graph_with(&[
            ("a", "unit", at(0.0, 0.0, 0.0)),
            ("b", "unit", at(1.3, 0.2, 0.0)),
        ]);
        let found = graph.clearance(&ids(&["a"]), &ids(&["b"]), 0.5).unwrap();
        assert_eq!(found.len(), 1);
        assert!((found[0].distance - 0.3).abs() < 1e-12);
        assert!(found[0].error_bound <= 0.5 / 20.0);
        assert!((found[0].points[0][0] - 1.0).abs() < 1e-12);
        assert!((found[0].points[1][0] - 1.3).abs() < 1e-12);
    }

    #[test]
    fn bodies_farther_than_the_clearance_are_not_reported() {
        let graph = graph_with(&[
            ("a", "unit", at(0.0, 0.0, 0.0)),
            ("b", "unit", at(1.6, 0.0, 0.0)),
        ]);
        assert!(graph
            .clearance(&ids(&["a"]), &ids(&["b"]), 0.5)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn overlapping_and_contained_bodies_are_zero_apart() {
        let graph = graph_with(&[
            ("a", "unit", at(0.0, 0.0, 0.0)),
            ("overlap", "unit", at(0.5, 0.5, 0.0)),
            ("inside", "small", at(0.4, 0.4, 0.4)),
        ]);
        let found = graph
            .clearance(&ids(&["a"]), &ids(&["overlap", "inside"]), 0.1)
            .unwrap();
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|proximity| proximity.distance == 0.0));
    }

    #[test]
    fn curved_bodies_stay_within_the_reported_error_bound() {
        let graph = graph_with(&[
            ("box", "unit", at(0.0, 0.0, 0.0)),
            ("post", "post", at(1.5, 0.5, 0.0)),
        ]);
        let found = graph
            .clearance(&ids(&["box"]), &ids(&["post"]), 0.4)
            .unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].error_bound > 0.0 && found[0].error_bound <= 0.4 / 20.0);
        assert!((found[0].distance - 0.25).abs() <= found[0].error_bound);
    }

    #[test]
    fn distance_measures_far_apart_items() {
        let graph = graph_with(&[
            ("a", "unit", at(0.0, 0.0, 0.0)),
            ("b", "unit", at(6.0, 0.0, 0.0)),
        ]);
        let found = graph.distance("a", "b", 0.01).unwrap().unwrap();
        assert!((found.distance - 5.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_clearance_and_tolerance_are_rejected() {
        let graph = graph_with(&[
            ("a", "unit", at(0.0, 0.0, 0.0)),
            ("b", "unit", at(2.0, 0.0, 0.0)),
        ]);
        assert!(graph.clearance(&ids(&["a"]), &ids(&["b"]), 0.0).is_err());
        assert!(graph.distance("a", "b", f64::NAN).is_err());
    }
}
