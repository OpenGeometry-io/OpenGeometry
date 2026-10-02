use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceSource {
    pub entity: String,
    pub body: String,
    pub(crate) key: String,
    pub(crate) face: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FaceRole {
    Authored,
    Preserved,
    Split,
    Cut,
    Coincident,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceProvenance {
    pub sources: Vec<FaceSource>,
    pub role: FaceRole,
    pub(crate) reversed: bool,
}
