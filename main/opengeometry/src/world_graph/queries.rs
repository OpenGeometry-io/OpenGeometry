use super::error::{ErrorCode, ErrorDetails, GraphError};
use super::graph::WorldGraph;
use super::node::{EditScope, Node};
use super::placement::is_identity;
use super::shape_store::Shape;
use crate::brep::{placed, BrepEnvelope};
use crate::math::Point3;
use crate::query::{classify_point, PointClassification};
use crate::tessellation::display::{display_buffers, DisplayBuffers};
use std::sync::Arc;

impl WorldGraph {
    pub fn node(&self, og_id: &str) -> Result<&Node, GraphError> {
        self.nodes
            .get(og_id)
            .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, og_id))
    }

    pub fn shape(&self, og_id: &str) -> Result<&Shape, GraphError> {
        let node = self.node(og_id)?;
        let id = node
            .shape
            .as_ref()
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "node is not a body"))?;
        self.shapes.shapes.get(id).ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
        })
    }

    pub fn brep(&self, og_id: &str) -> Result<Arc<BrepEnvelope>, GraphError> {
        Ok(Arc::clone(&self.shape(og_id)?.brep))
    }

    pub fn placed_brep(&self, og_id: &str) -> Result<Arc<BrepEnvelope>, GraphError> {
        let shape = self.shape(og_id)?;
        let world = self.world_placement(og_id)?;
        if is_identity(world) {
            return Ok(Arc::clone(&shape.brep));
        }
        let mut brep = placed(&shape.brep, world.frame, world.scale)?;
        brep.revision = shape.revision;
        Ok(Arc::new(brep))
    }

    pub fn face_normal_at(
        &self,
        og_id: &str,
        face_id: u32,
        point: Point3,
    ) -> Result<Point3, GraphError> {
        let brep = &self.shape(og_id)?.brep;
        if face_id as usize >= brep.topology.faces.len() {
            return Err(GraphError::with_details(
                ErrorCode::InvalidParameter,
                "face id is out of range",
                ErrorDetails::MissingReference {
                    kind: "face".into(),
                    index: face_id,
                },
            ));
        }
        Ok(brep.face_normal_at(face_id, point)?)
    }

    pub fn classify_point(
        &self,
        og_id: &str,
        point: Point3,
    ) -> Result<PointClassification, GraphError> {
        let brep = &self.shape(og_id)?.brep;
        if brep.solids.is_empty() {
            return Err(GraphError::code(
                ErrorCode::InvalidOperand,
                "point classification needs a solid",
            ));
        }
        Ok(classify_point(brep, point)?)
    }

    pub fn snapshot(&self, shape_id: &str) -> Result<Vec<u8>, GraphError> {
        let shape = self
            .shapes
            .shapes
            .get(shape_id)
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "shape is not live"))?;
        let bytes = shape.brep.to_json()?.into_bytes();
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(GraphError::code(
                ErrorCode::LimitExceeded,
                "snapshot exceeds 64 MiB",
            ));
        }
        Ok(bytes)
    }

    pub fn buffers(
        &self,
        shape_id: &str,
        bucket: f64,
        max_triangles: usize,
    ) -> Result<DisplayBuffers, GraphError> {
        let shape = self
            .shapes
            .shapes
            .get(shape_id)
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "shape is not live"))?;
        Ok(display_buffers(&shape.brep, bucket, max_triangles)?)
    }

    pub(crate) fn edge_keys(&self, og_id: &str) -> Result<&[String], GraphError> {
        Ok(&self.shape(og_id)?.edge_keys)
    }

    pub fn instance_count(&self, og_id: &str) -> Result<u32, GraphError> {
        Ok(self.shape(og_id)?.users)
    }

    pub fn ensure_editable(&self, og_id: &str, scope: EditScope) -> Result<&Shape, GraphError> {
        let shape = self.shape(og_id)?;
        if shape.users > 1 && scope == EditScope::Node {
            let shape_id = self.node(og_id)?.shape.as_ref();
            let sharing: Vec<_> = self
                .nodes
                .values()
                .filter(|node| node.shape.as_ref() == shape_id)
                .take(16)
                .map(|node| node.og_id.clone())
                .collect();
            return Err(GraphError::with_details(
                ErrorCode::SharedShape,
                "shape has multiple instances",
                ErrorDetails::SharedShape {
                    shape_id: shape_id.cloned(),
                    instance_count: shape.users,
                    sharing,
                },
            ));
        }
        Ok(shape)
    }

    pub fn node_by_handle(&self, handle: u32, generation: u32) -> Result<&Node, GraphError> {
        self.nodes
            .values()
            .find(|node| node.handle == handle && node.generation == generation)
            .ok_or_else(|| GraphError::code(ErrorCode::Disposed, "node handle is stale"))
    }
}
