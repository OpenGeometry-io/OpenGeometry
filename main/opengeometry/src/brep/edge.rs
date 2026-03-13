use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: u32,
    pub halfedge: u32,
    pub twin_halfedge: Option<u32>,
}

impl Edge {
    pub fn new(id: u32, halfedge: u32, twin_halfedge: Option<u32>) -> Self {
        Self {
            id,
            halfedge,
            twin_halfedge,
        }
    }
}
