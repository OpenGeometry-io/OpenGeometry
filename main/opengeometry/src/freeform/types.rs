use openmaths::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ObjectTransformation {
    pub anchor: Vector3,
    pub translation: Vector3,
    pub rotation: Vector3,
    pub scale: Vector3,
}
