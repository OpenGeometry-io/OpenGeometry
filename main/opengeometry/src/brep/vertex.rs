use openmaths::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: u32,
    pub position: Vector3,
    pub outgoing_halfedge: Option<u32>,
}

impl Vertex {
    pub fn new(id: u32, position: Vector3) -> Self {
        Self {
            id,
            position,
            outgoing_halfedge: None,
        }
    }
}
