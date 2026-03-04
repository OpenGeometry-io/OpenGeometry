use openmaths::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Face {
    pub id: u32,
    pub normal: Vector3,
    pub outer_loop: u32,
    pub inner_loops: Vec<u32>,
    pub shell_ref: Option<u32>,
}

impl Face {
    pub fn new(
        id: u32,
        normal: Vector3,
        outer_loop: u32,
        inner_loops: Vec<u32>,
        shell_ref: Option<u32>,
    ) -> Self {
        Self {
            id,
            normal,
            outer_loop,
            inner_loops,
            shell_ref,
        }
    }

    pub fn set_normal(&mut self, normal: Vector3) {
        self.normal = normal;
    }
}
