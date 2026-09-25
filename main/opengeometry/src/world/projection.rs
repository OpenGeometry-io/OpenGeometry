use super::graph::{Geometry, WorldError, WorldGraph};
use super::transform::Similarity3;
use crate::analytic::placement::placed;
use crate::brep::Brep;
use crate::export::projection::{
    project_analytic_brep_to_scene, project_brep_to_scene, CameraParameters, HlrOptions, Scene2D,
};
use openmaths::Vector3;
use serde::Deserialize;
use std::collections::HashMap;

const MAX_PROJECTION_TRIANGLES: usize = 2_000_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionView {
    pub id: String,
    pub camera: CameraParameters,
    #[serde(default)]
    pub hlr: HlrOptions,
}

impl WorldGraph {
    pub fn project(
        &self,
        items: &[String],
        camera: &CameraParameters,
        hlr: &HlrOptions,
        deflection: f64,
    ) -> Result<Scene2D, WorldError> {
        if !deflection.is_finite() || deflection <= 0.0 {
            return Err(WorldError::InvalidInput(
                "projection deflection must be finite and positive".into(),
            ));
        }
        let mut scene = Scene2D::new();
        for leaf in self.leaves_of(items)? {
            let definition = self
                .definitions
                .get(&leaf.definition)
                .ok_or_else(|| WorldError::UnknownDefinition(leaf.definition.clone()))?;
            let mut projected = match &definition.geometry {
                Geometry::Analytic(brep) => project_analytic_brep_to_scene(
                    &placed(brep, leaf.world.frame, leaf.world.scale)?,
                    camera,
                    hlr,
                    deflection,
                    MAX_PROJECTION_TRIANGLES,
                )?,
                Geometry::Legacy(brep) => {
                    project_brep_to_scene(&legacy_in_world(brep, &leaf.world), camera, hlr)
                }
            };
            let layer = self
                .kind_of(&leaf.node)?
                .and_then(aia_layer)
                .map(str::to_string);
            for segment in &mut projected.segments {
                segment.layer = layer.clone();
                segment.source_entity_id = Some(leaf.node.clone());
            }
            scene.extend(projected);
        }
        Ok(scene)
    }

    pub fn project_views(
        &self,
        items: &[String],
        views: &[ProjectionView],
        deflection: f64,
    ) -> Result<HashMap<String, Scene2D>, WorldError> {
        let mut projected = HashMap::with_capacity(views.len());
        for view in views {
            let mut scene = self.project(items, &view.camera, &view.hlr, deflection)?;
            scene.name = Some(view.id.clone());
            projected.insert(view.id.clone(), scene);
        }
        Ok(projected)
    }
}

pub(super) fn legacy_in_world(brep: &Brep, world: &Similarity3) -> Brep {
    let mut placed = brep.clone();
    if *world != Similarity3::IDENTITY {
        placed.apply_point_transform(
            |point: Vector3| {
                let [x, y, z] = world.apply_point([point.x, point.y, point.z]);
                Vector3::new(x, y, z)
            },
            world.scale,
        );
    }
    placed
}

fn aia_layer(kind: &str) -> Option<&'static str> {
    match kind.to_lowercase().trim() {
        "wall" => Some("A-WALL"),
        "door" => Some("A-DOOR"),
        "window" | "glazing" | "glaz" => Some("A-GLAZ"),
        "slab" | "floor" | "ceiling" => Some("A-FLOR"),
        "stair" | "stairs" => Some("A-FLOR-STRS"),
        "column" | "col" => Some("A-COLS"),
        "beam" => Some("S-BEAM"),
        "roof" => Some("A-ROOF"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::primitives;
    use crate::analytic::topology::Accuracy;
    use crate::analytic::Frame3;
    use crate::brep::BrepBuilder;
    use crate::world::NodeSpec;
    use uuid::Uuid;

    fn wire() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)]);
        builder.add_wire(&[0, 1], false).unwrap();
        builder.build().unwrap()
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

    fn node(
        id: &str,
        parent: Option<&str>,
        definition: Option<&str>,
        kind: Option<&str>,
        local: Similarity3,
    ) -> NodeSpec {
        NodeSpec {
            id: id.into(),
            parent: parent.map(str::to_string),
            definition: definition.map(str::to_string),
            local,
            kind: kind.map(str::to_string),
        }
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    fn line_keys(scene: &Scene2D) -> Vec<String> {
        let mut keys: Vec<String> = scene
            .to_lines()
            .lines
            .iter()
            .map(|line| {
                format!(
                    "{:.9},{:.9},{:.9},{:.9},{:?}",
                    line.start.x, line.start.y, line.end.x, line.end.y, line.class
                )
            })
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn legacy_lines_carry_the_node_id_and_the_inherited_aia_layer() {
        let mut graph = WorldGraph::new();
        graph.define_legacy("wire", wire()).unwrap();
        graph
            .add_node(node(
                "walls",
                None,
                None,
                Some("wall"),
                Similarity3::IDENTITY,
            ))
            .unwrap();
        graph
            .add_node(node(
                "wall-1",
                Some("walls"),
                Some("wire"),
                None,
                Similarity3::IDENTITY,
            ))
            .unwrap();
        let scene = graph
            .project(
                &ids(&["walls"]),
                &CameraParameters::default(),
                &HlrOptions::default(),
                0.01,
            )
            .unwrap();
        assert!(!scene.segments.is_empty());
        for segment in &scene.segments {
            assert_eq!(segment.layer.as_deref(), Some("A-WALL"));
            assert_eq!(segment.source_entity_id.as_deref(), Some("wall-1"));
        }
    }

    #[test]
    fn legacy_geometry_follows_its_parent_transform() {
        let mut graph = WorldGraph::new();
        graph.define_legacy("wire", wire()).unwrap();
        graph
            .add_node(node("group", None, None, None, at(0.5, 2.0, -3.0)))
            .unwrap();
        graph
            .add_node(node(
                "piece",
                Some("group"),
                Some("wire"),
                None,
                Similarity3::IDENTITY,
            ))
            .unwrap();
        let camera = CameraParameters::default();
        let hlr = HlrOptions::default();
        let projected = graph
            .project(&ids(&["group"]), &camera, &hlr, 0.01)
            .unwrap();
        let mut moved = wire();
        moved.apply_point_transform(|p| Vector3::new(p.x + 0.5, p.y + 2.0, p.z - 3.0), 1.0);
        assert_eq!(
            line_keys(&projected),
            line_keys(&project_brep_to_scene(&moved, &camera, &hlr))
        );
    }

    #[test]
    fn analytic_projection_matches_projecting_the_placed_body_directly() {
        let accuracy = Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        };
        let body =
            primitives::cylinder("post".into(), Frame3::IDENTITY, 0.3, 1.2, accuracy).unwrap();
        let mut graph = WorldGraph::new();
        graph.define("post", body.clone()).unwrap();
        graph
            .add_node(node("post", None, Some("post"), None, at(1.0, 0.0, 2.0)))
            .unwrap();
        let camera = CameraParameters::default();
        let hlr = HlrOptions::default();
        let expected = project_analytic_brep_to_scene(
            &placed(&body, at(1.0, 0.0, 2.0).frame, 1.0).unwrap(),
            &camera,
            &hlr,
            0.01,
            MAX_PROJECTION_TRIANGLES,
        )
        .unwrap();
        let projected = graph.project(&ids(&["post"]), &camera, &hlr, 0.01).unwrap();
        assert!(!expected.segments.is_empty());
        assert_eq!(line_keys(&projected), line_keys(&expected));
    }

    #[test]
    fn every_requested_view_is_returned_and_removed_nodes_disappear() {
        let mut graph = WorldGraph::new();
        graph.define_legacy("wire", wire()).unwrap();
        graph
            .add_node(node("a", None, Some("wire"), None, Similarity3::IDENTITY))
            .unwrap();
        let views: Vec<ProjectionView> = serde_json::from_str(&format!(
            r#"[{{"id":"plan","camera":{camera}}},{{"id":"elevation","camera":{camera}}}]"#,
            camera = serde_json::to_string(&CameraParameters::default()).unwrap()
        ))
        .unwrap();
        let projected = graph.project_views(&ids(&["a"]), &views, 0.01).unwrap();
        assert_eq!(projected.len(), 2);
        assert_eq!(projected["plan"].name.as_deref(), Some("plan"));
        assert!(!projected["elevation"].segments.is_empty());
        graph.remove_node("a").unwrap();
        assert!(graph
            .project(
                &ids(&["a"]),
                &CameraParameters::default(),
                &HlrOptions::default(),
                0.01
            )
            .is_err());
    }

    #[test]
    fn nested_items_are_projected_once() {
        let mut graph = WorldGraph::new();
        graph.define_legacy("wire", wire()).unwrap();
        graph
            .add_node(node("group", None, None, None, Similarity3::IDENTITY))
            .unwrap();
        graph
            .add_node(node(
                "piece",
                Some("group"),
                Some("wire"),
                None,
                Similarity3::IDENTITY,
            ))
            .unwrap();
        let camera = CameraParameters::default();
        let hlr = HlrOptions::default();
        let once = graph
            .project(&ids(&["group"]), &camera, &hlr, 0.01)
            .unwrap();
        let twice = graph
            .project(&ids(&["group", "piece"]), &camera, &hlr, 0.01)
            .unwrap();
        assert_eq!(once.segments.len(), twice.segments.len());
    }

    #[test]
    fn aia_layer_maps_known_kinds() {
        assert_eq!(aia_layer("wall"), Some("A-WALL"));
        assert_eq!(aia_layer("door"), Some("A-DOOR"));
        assert_eq!(aia_layer("window"), Some("A-GLAZ"));
        assert_eq!(aia_layer("slab"), Some("A-FLOR"));
        assert_eq!(aia_layer("stair"), Some("A-FLOR-STRS"));
        assert_eq!(aia_layer("column"), Some("A-COLS"));
        assert_eq!(aia_layer("unknown"), None);
    }

    #[test]
    fn invalid_deflection_is_rejected() {
        let graph = WorldGraph::new();
        assert!(graph
            .project(
                &[],
                &CameraParameters::default(),
                &HlrOptions::default(),
                0.0
            )
            .is_err());
    }
}
