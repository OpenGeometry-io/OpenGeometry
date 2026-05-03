pub mod drawing;
pub mod dxf;
pub mod ifc;
pub mod part21;
pub mod pdf;
pub mod projection;
pub mod step;
pub mod stl;

pub use drawing::{
    DrawingDocument, DrawingExportConfig, DrawingGeometry, DrawingPrimitive, DrawingStyle,
    DrawingText, DrawingView,
};
pub use dxf::{
    export_dxf_string, export_scene_to_dxf_string, DxfExportConfig, DxfExportError, DxfExportResult,
};
pub use ifc::{
    export_brep_to_ifc_text, export_breps_to_ifc_text, export_scene_entities_to_ifc_text,
    IfcEntityInput, IfcEntitySemantics, IfcErrorPolicy, IfcExportConfig, IfcExportError,
    IfcExportReport, IfcSchemaVersion,
};
pub use pdf::{
    export_krilla_probe_pdf_bytes, export_pdf_bytes, export_scene_to_pdf_bytes,
    export_scene_to_pdf_with_config, PdfExportConfig, PdfExportError, PdfExportResult,
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
