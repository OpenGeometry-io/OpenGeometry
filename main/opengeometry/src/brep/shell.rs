use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Shell {
    pub id: u32,
    pub faces: Vec<u32>,
    pub is_closed: bool,
}

impl Shell {
    pub fn new(id: u32, faces: Vec<u32>, is_closed: bool) -> Self {
        Self {
            id,
            faces,
            is_closed,
        }
    }
}
