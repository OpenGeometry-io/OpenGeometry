pub mod projection;

#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;

pub use projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Line2D, Path2D, ProjectionMode, Scene2D,
    Scene2DLines, Segment2D, Vec2,
};
