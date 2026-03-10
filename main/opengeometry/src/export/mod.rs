pub mod part21;
pub mod projection;
pub mod step;

#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;

pub use projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Line2D, Path2D, ProjectionMode, Scene2D,
    Scene2DLines, Segment2D, Vec2,
};
pub use step::{
    export_brep_to_step_text, export_breps_to_step_text, StepErrorPolicy, StepExportConfig,
    StepExportError, StepExportReport, StepSchema,
};

#[cfg(not(target_arch = "wasm32"))]
pub use step::{export_brep_to_step_file, export_breps_to_step_file};
