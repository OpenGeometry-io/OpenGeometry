pub mod ifc;
pub mod part21;
pub mod projection;
pub mod step;
pub mod stl;

#[cfg(not(target_arch = "wasm32"))]
pub mod pdf;

pub use ifc::{
    export_brep_to_ifc_text, export_breps_to_ifc_text, export_scene_entities_to_ifc_text,
    IfcEntityInput, IfcEntitySemantics, IfcErrorPolicy, IfcExportConfig, IfcExportError,
    IfcExportReport, IfcSchemaVersion,
};
pub use projection::{
    project_brep_to_scene, CameraParameters, ClassifiedSegment, EdgeClass, HlrOptions, Line2D,
    Path2D, ProjectionMode, Scene2D, Scene2DLines, Segment2D, Vec2,
};
pub use step::{
    export_brep_to_step_text, export_breps_to_step_text, StepErrorPolicy, StepExportConfig,
    StepExportError, StepExportReport, StepSchema,
};
pub use stl::{
    export_brep_to_stl_bytes, export_breps_to_stl_bytes, StlErrorPolicy, StlExportConfig,
    StlExportError, StlExportReport,
};

#[cfg(not(target_arch = "wasm32"))]
pub use ifc::{
    export_brep_to_ifc_file, export_breps_to_ifc_file, export_scene_entities_to_ifc_file,
};
#[cfg(not(target_arch = "wasm32"))]
pub use step::{export_brep_to_step_file, export_breps_to_step_file};
#[cfg(not(target_arch = "wasm32"))]
pub use stl::{export_brep_to_stl_file, export_breps_to_stl_file};
