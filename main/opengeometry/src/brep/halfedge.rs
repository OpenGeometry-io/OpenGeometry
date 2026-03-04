use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct HalfEdge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub twin: Option<u32>,
    pub next: Option<u32>,
    pub prev: Option<u32>,
    pub edge: u32,
    pub face: Option<u32>,
    pub loop_ref: Option<u32>,
    pub wire_ref: Option<u32>,
}

impl HalfEdge {
    pub fn new(
        id: u32,
        from: u32,
        to: u32,
        edge: u32,
        face: Option<u32>,
        loop_ref: Option<u32>,
        wire_ref: Option<u32>,
    ) -> Self {
        Self {
            id,
            from,
            to,
            twin: None,
            next: None,
            prev: None,
            edge,
            face,
            loop_ref,
            wire_ref,
        }
    }
}
