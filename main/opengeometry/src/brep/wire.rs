use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Wire {
    pub id: u32,
    pub halfedges: Vec<u32>,
    pub is_closed: bool,
}

impl Wire {
    pub fn new(id: u32, halfedges: Vec<u32>, is_closed: bool) -> Self {
        Self {
            id,
            halfedges,
            is_closed,
        }
    }
}
