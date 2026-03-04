pub mod projection;
pub mod stl;

#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;

pub use projection::{
    project_brep_to_scene, CameraParameters, HlrOptions, Line2D, Path2D, ProjectionMode, Scene2D,
    Scene2DLines, Segment2D, Vec2,
};
pub use stl::{
    export_brep_to_stl_bytes, export_breps_to_stl_bytes, StlErrorPolicy, StlExportConfig,
    StlExportError, StlExportReport,
};

#[cfg(not(target_arch = "wasm32"))]
pub use stl::{export_brep_to_stl_file, export_breps_to_stl_file};
