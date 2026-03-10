use opengeometry::export::{
    export_brep_to_ifc_text, export_brep_to_step_text, IfcExportConfig, StepExportConfig,
};
use opengeometry::primitives::cuboid::OGCuboid;
use openmaths::Vector3;
use std::env;
use std::fs;
use std::path::PathBuf;

fn js_err_to_string(err: wasm_bindgen::JsValue) -> String {
    err.as_string()
        .unwrap_or_else(|| "unknown js error".to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/export-validation"));
    fs::create_dir_all(&out_dir)?;

    let mut cuboid = OGCuboid::new("validation-cuboid".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 1.0, 0.0), 2.0, 2.0, 2.0)
        .map_err(js_err_to_string)?;

    let step_config = StepExportConfig::default();
    let ifc_config = IfcExportConfig::default();

    let (step_text, step_report) = export_brep_to_step_text(cuboid.brep(), &step_config)?;
    let (ifc_text, ifc_report) = export_brep_to_ifc_text(cuboid.brep(), &ifc_config)?;

    let step_path = out_dir.join("validation-cuboid.step");
    let ifc_path = out_dir.join("validation-cuboid.ifc");

    fs::write(&step_path, step_text)?;
    fs::write(&ifc_path, ifc_text)?;

    println!("STEP fixture: {}", step_path.display());
    println!("IFC fixture: {}", ifc_path.display());
    println!(
        "STEP report: solids={}, triangles={}, skipped_entities={}",
        step_report.exported_solids, step_report.exported_triangles, step_report.skipped_entities
    );
    println!(
        "IFC report: elements={}, triangles={}, skipped_entities={}",
        ifc_report.exported_elements, ifc_report.exported_triangles, ifc_report.skipped_entities
    );

    Ok(())
}
