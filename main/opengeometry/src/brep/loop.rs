use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Loop {
    pub id: u32,
    pub halfedge: u32,
    pub face: u32,
    pub is_hole: bool,
}

impl Loop {
    pub fn new(id: u32, halfedge: u32, face: u32, is_hole: bool) -> Self {
        Self {
            id,
            halfedge,
            face,
            is_hole,
        }
    }
}
