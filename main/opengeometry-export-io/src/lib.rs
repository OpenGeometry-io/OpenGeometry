pub mod ifc;
pub mod part21;
pub mod stl;

pub use ifc::{
    export_snapshot_to_ifc_text, IfcErrorPolicy, IfcExportConfig, IfcExportError, IfcExportReport,
    IfcSchemaVersion,
};
pub use stl::{
    export_snapshot_to_stl_bytes, StlErrorPolicy, StlExportConfig, StlExportError, StlExportReport,
};

#[cfg(not(target_arch = "wasm32"))]
pub use ifc::export_snapshot_to_ifc_file;
#[cfg(not(target_arch = "wasm32"))]
pub use stl::export_snapshot_to_stl_file;
