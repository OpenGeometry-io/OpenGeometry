use super::transform::Similarity3;
use crate::analytic::{BrepEnvelope, GeometryError, Point3};
use crate::brep::Brep;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum WorldError {
    DuplicateNode(String),
    UnknownNode(String),
    DuplicateDefinition(String),
    UnknownDefinition(String),
    DefinitionInUse(String),
    Cycle(String),
    InvalidInput(String),
    Export(String),
    Geometry(GeometryError),
}

impl From<GeometryError> for WorldError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    pub id: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub definition: Option<String>,
    #[serde(default)]
    pub local: Similarity3,
    #[serde(default)]
    pub kind: Option<String>,
}

pub type Bounds3 = [f64; 6];

pub(super) enum Geometry {
    Analytic(BrepEnvelope),
    Legacy(Brep),
}

pub(super) struct Definition {
    pub(super) geometry: Geometry,
    pub(super) local_bounds: Option<Bounds3>,
    pub(super) users: usize,
}

pub(super) struct Node {
    pub(super) parent: Option<String>,
    pub(super) children: Vec<String>,
    pub(super) definition: Option<String>,
    pub(super) local: Similarity3,
    pub(super) kind: Option<String>,
}

#[derive(Default)]
pub struct WorldGraph {
    pub(super) definitions: HashMap<String, Definition>,
    pub(super) nodes: HashMap<String, Node>,
}

impl WorldGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn has_definition(&self, id: &str) -> bool {
        self.definitions.contains_key(id)
    }

    pub fn define(&mut self, id: &str, brep: BrepEnvelope) -> Result<(), WorldError> {
        if self.definitions.contains_key(id) {
            return Err(WorldError::DuplicateDefinition(id.into()));
        }
        let local_bounds = brep.bounds()?.map(|bounds| {
            [
                bounds.axes[0].lo,
                bounds.axes[1].lo,
                bounds.axes[2].lo,
                bounds.axes[0].hi,
                bounds.axes[1].hi,
                bounds.axes[2].hi,
            ]
        });
        self.insert_definition(id, Geometry::Analytic(brep), local_bounds)
    }

    pub fn define_legacy(&mut self, id: &str, brep: Brep) -> Result<(), WorldError> {
        if self.definitions.contains_key(id) {
            return Err(WorldError::DuplicateDefinition(id.into()));
        }
        brep.validate_topology()
            .map_err(|error| WorldError::InvalidInput(format!("legacy BRep {id}: {error}")))?;
        let local_bounds = brep.vertices.iter().fold(None, |bounds, vertex| {
            Some(include(
                bounds,
                [vertex.position.x, vertex.position.y, vertex.position.z],
            ))
        });
        self.insert_definition(id, Geometry::Legacy(brep), local_bounds)
    }

    fn insert_definition(
        &mut self,
        id: &str,
        geometry: Geometry,
        local_bounds: Option<Bounds3>,
    ) -> Result<(), WorldError> {
        self.definitions.insert(
            id.into(),
            Definition {
                geometry,
                local_bounds,
                users: 0,
            },
        );
        Ok(())
    }

    pub fn undefine(&mut self, id: &str) -> Result<(), WorldError> {
        match self.definitions.get(id) {
            None => Err(WorldError::UnknownDefinition(id.into())),
            Some(definition) if definition.users > 0 => Err(WorldError::DefinitionInUse(id.into())),
            Some(_) => {
                self.definitions.remove(id);
                Ok(())
            }
        }
    }

    pub fn add_node(&mut self, spec: NodeSpec) -> Result<(), WorldError> {
        if spec.id.is_empty() {
            return Err(WorldError::InvalidInput("node id must not be empty".into()));
        }
        if self.nodes.contains_key(&spec.id) {
            return Err(WorldError::DuplicateNode(spec.id));
        }
        spec.local.validate()?;
        if let Some(parent) = &spec.parent {
            if !self.nodes.contains_key(parent) {
                return Err(WorldError::UnknownNode(parent.clone()));
            }
        }
        if let Some(definition) = &spec.definition {
            self.definitions
                .get_mut(definition)
                .ok_or_else(|| WorldError::UnknownDefinition(definition.clone()))?
                .users += 1;
        }
        if let Some(parent) = &spec.parent {
            self.node_mut(parent)?.children.push(spec.id.clone());
        }
        self.nodes.insert(
            spec.id,
            Node {
                parent: spec.parent,
                children: Vec::new(),
                definition: spec.definition,
                local: spec.local,
                kind: spec.kind,
            },
        );
        Ok(())
    }

    pub fn set_local(&mut self, id: &str, local: Similarity3) -> Result<(), WorldError> {
        local.validate()?;
        self.node_mut(id)?.local = local;
        Ok(())
    }

    pub fn reparent(&mut self, id: &str, parent: Option<&str>) -> Result<(), WorldError> {
        let previous = self.node(id)?.parent.clone();
        if let Some(parent) = parent {
            let mut cursor = Some(parent.to_string());
            while let Some(current) = cursor {
                if current == id {
                    return Err(WorldError::Cycle(id.into()));
                }
                cursor = self.node(&current)?.parent.clone();
            }
        }
        if let Some(previous) = previous {
            self.node_mut(&previous)?
                .children
                .retain(|child| child != id);
        }
        if let Some(parent) = parent {
            self.node_mut(parent)?.children.push(id.into());
        }
        self.node_mut(id)?.parent = parent.map(str::to_string);
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), WorldError> {
        if let Some(parent) = self.node(id)?.parent.clone() {
            self.node_mut(&parent)?.children.retain(|child| child != id);
        }
        let mut pending = vec![id.to_string()];
        while let Some(current) = pending.pop() {
            let node = self
                .nodes
                .remove(&current)
                .ok_or_else(|| WorldError::UnknownNode(current.clone()))?;
            if let Some(definition) = node.definition {
                if let Some(definition) = self.definitions.get_mut(&definition) {
                    definition.users -= 1;
                }
            }
            pending.extend(node.children);
        }
        Ok(())
    }

    pub fn world(&self, id: &str) -> Result<Similarity3, WorldError> {
        let mut chain = vec![self.node(id)?.local];
        let mut cursor = self.node(id)?.parent.clone();
        while let Some(current) = cursor {
            let node = self.node(&current)?;
            chain.push(node.local);
            cursor = node.parent.clone();
        }
        Ok(chain
            .iter()
            .rev()
            .fold(Similarity3::IDENTITY, |world, local| world.compose(local)))
    }

    pub fn world_bounds(&self, id: &str) -> Result<Option<Bounds3>, WorldError> {
        let mut bounds: Option<Bounds3> = None;
        let mut pending = vec![(id.to_string(), self.world(id)?)];
        while let Some((current, world)) = pending.pop() {
            let node = self.node(&current)?;
            if let Some(definition) = &node.definition {
                let local_bounds = self
                    .definitions
                    .get(definition)
                    .ok_or_else(|| WorldError::UnknownDefinition(definition.clone()))?
                    .local_bounds;
                if let Some(local_bounds) = local_bounds {
                    for corner in corners(local_bounds) {
                        bounds = Some(include(bounds, world.apply_point(corner)));
                    }
                }
            }
            for child in &node.children {
                pending.push((child.clone(), world.compose(&self.node(child)?.local)));
            }
        }
        Ok(bounds)
    }

    pub(super) fn kind_of(&self, id: &str) -> Result<Option<&str>, WorldError> {
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            let node = self.node(current)?;
            if let Some(kind) = &node.kind {
                return Ok(Some(kind));
            }
            cursor = node.parent.as_deref();
        }
        Ok(None)
    }

    pub(super) fn node(&self, id: &str) -> Result<&Node, WorldError> {
        self.nodes
            .get(id)
            .ok_or_else(|| WorldError::UnknownNode(id.into()))
    }

    fn node_mut(&mut self, id: &str) -> Result<&mut Node, WorldError> {
        self.nodes
            .get_mut(id)
            .ok_or_else(|| WorldError::UnknownNode(id.into()))
    }
}

pub(super) fn corners(bounds: Bounds3) -> [Point3; 8] {
    std::array::from_fn(|index| {
        [
            bounds[if index & 1 == 0 { 0 } else { 3 }],
            bounds[if index & 2 == 0 { 1 } else { 4 }],
            bounds[if index & 4 == 0 { 2 } else { 5 }],
        ]
    })
}

pub(super) fn include(bounds: Option<Bounds3>, point: Point3) -> Bounds3 {
    match bounds {
        None => [point[0], point[1], point[2], point[0], point[1], point[2]],
        Some(b) => [
            b[0].min(point[0]),
            b[1].min(point[1]),
            b[2].min(point[2]),
            b[3].max(point[0]),
            b[4].max(point[1]),
            b[5].max(point[2]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::primitives;
    use crate::analytic::topology::Accuracy;
    use crate::analytic::Frame3;

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        }
    }

    fn unit_box() -> BrepEnvelope {
        primitives::cuboid("box".into(), Frame3::IDENTITY, [1.0, 2.0, 3.0], accuracy()).unwrap()
    }

    fn translation(x: f64, y: f64, z: f64) -> Similarity3 {
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

    fn node(
        id: &str,
        parent: Option<&str>,
        definition: Option<&str>,
        local: Similarity3,
    ) -> NodeSpec {
        NodeSpec {
            id: id.into(),
            parent: parent.map(str::to_string),
            definition: definition.map(str::to_string),
            local,
            kind: None,
        }
    }

    fn assert_bounds_close(actual: Bounds3, expected: Bounds3) {
        for index in 0..6 {
            assert!(
                (actual[index] - expected[index]).abs() <= 2.0 * accuracy().geometric,
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn room() -> WorldGraph {
        let mut graph = WorldGraph::new();
        graph.define("box", unit_box()).unwrap();
        graph
            .add_node(node("room", None, None, translation(10.0, 0.0, 0.0)))
            .unwrap();
        graph
            .add_node(node(
                "cluster",
                Some("room"),
                None,
                quarter_turn_about_z(0.0, 5.0, 0.0),
            ))
            .unwrap();
        graph
            .add_node(node(
                "item",
                Some("cluster"),
                Some("box"),
                translation(1.0, 0.0, 0.0),
            ))
            .unwrap();
        graph
    }

    #[test]
    fn world_transform_composes_every_ancestor() {
        let graph = room();
        let world = graph.world("item").unwrap();
        let origin = world.apply_point([0.0, 0.0, 0.0]);
        assert_bounds_close(
            [origin[0], origin[1], origin[2], 0.0, 0.0, 0.0],
            [10.0, 6.0, 0.0, 0.0, 0.0, 0.0],
        );
    }

    #[test]
    fn moving_a_parent_moves_its_descendants() {
        let mut graph = room();
        graph
            .set_local("room", translation(-3.0, 0.0, 2.0))
            .unwrap();
        assert_bounds_close(
            graph.world_bounds("item").unwrap().unwrap(),
            [-5.0, 6.0, 2.0, -3.0, 7.0, 5.0],
        );
    }

    #[test]
    fn world_bounds_cover_the_whole_subtree() {
        let mut graph = room();
        graph
            .add_node(node(
                "second",
                Some("room"),
                Some("box"),
                translation(0.0, 0.0, 0.0),
            ))
            .unwrap();
        assert_bounds_close(
            graph.world_bounds("room").unwrap().unwrap(),
            [8.0, 0.0, 0.0, 11.0, 7.0, 3.0],
        );
        assert_eq!(
            graph.world_bounds("cluster").unwrap(),
            graph.world_bounds("item").unwrap()
        );
    }

    #[test]
    fn reparent_rejects_cycles_and_keeps_the_tree_intact() {
        let mut graph = room();
        assert_eq!(
            graph.reparent("room", Some("item")),
            Err(WorldError::Cycle("room".into()))
        );
        assert_eq!(
            graph.reparent("room", Some("room")),
            Err(WorldError::Cycle("room".into()))
        );
        graph.reparent("item", Some("room")).unwrap();
        assert_bounds_close(
            graph.world_bounds("item").unwrap().unwrap(),
            [11.0, 0.0, 0.0, 12.0, 2.0, 3.0],
        );
        assert_eq!(graph.world_bounds("cluster").unwrap(), None);
    }

    #[test]
    fn shared_definitions_are_stored_once_and_protected_while_used() {
        let mut graph = room();
        graph
            .add_node(node(
                "copy",
                Some("room"),
                Some("box"),
                translation(4.0, 0.0, 0.0),
            ))
            .unwrap();
        assert_eq!(graph.definition_count(), 1);
        assert_eq!(
            graph.undefine("box"),
            Err(WorldError::DefinitionInUse("box".into()))
        );
        graph.remove_node("room").unwrap();
        assert_eq!(graph.node_count(), 0);
        graph.undefine("box").unwrap();
        assert_eq!(graph.definition_count(), 0);
    }

    #[test]
    fn invalid_references_are_rejected() {
        let mut graph = room();
        assert_eq!(
            graph.add_node(node("item", None, None, Similarity3::IDENTITY)),
            Err(WorldError::DuplicateNode("item".into()))
        );
        assert_eq!(
            graph.add_node(node("orphan", Some("missing"), None, Similarity3::IDENTITY)),
            Err(WorldError::UnknownNode("missing".into()))
        );
        assert_eq!(
            graph.add_node(node("ghost", None, Some("missing"), Similarity3::IDENTITY)),
            Err(WorldError::UnknownDefinition("missing".into()))
        );
        assert_eq!(
            graph.define("box", unit_box()),
            Err(WorldError::DuplicateDefinition("box".into()))
        );
        assert!(graph
            .set_local(
                "item",
                Similarity3 {
                    scale: -1.0,
                    ..Similarity3::IDENTITY
                }
            )
            .is_err());
    }
}
