use crate::builders::traced_fixture;
use crate::write::write_json;
use opengeometry::analytic::{
    exchange::export_step,
    tessellation::tessellate,
    topology::{Accuracy, BrepEnvelope},
    Frame3,
};
use serde_json::json;
use std::{error::Error, fs, path::Path};
use traced_analytic::analytic as traced;

pub(super) fn write_fixture(
    output: &Path,
    name: &str,
    body: &BrepEnvelope,
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<(), Box<dyn Error>> {
    let brep = body.to_json()?;
    let parsed = BrepEnvelope::from_json(&brep)?;
    if parsed.to_json()? != brep {
        return Err(format!("BRep round-trip changed {name}").into());
    }
    traced::booleans::take_handler_trace();
    let traced_body = traced_fixture(name, frame, accuracy)?;
    let handlers = traced::booleans::take_handler_trace();
    if traced_body.to_json()? != brep {
        return Err(format!("traced BRep differs from main for {name}").into());
    }
    fs::write(output.join(format!("{name}.brep.json")), brep)?;
    write_json(
        output.join(format!("{name}.handlers.json")),
        &json!(handlers),
    )?;
    let mesh = tessellate(body, 0.01, 2_000_000)?;
    let tessellation = json!({
        "positions": mesh.positions,
        "normals": mesh.normals,
        "indices": mesh.indices,
        "triangleFaceIds": mesh.triangle_face_ids,
        "outlinePositions": mesh.outline_positions,
        "outlineEdgeIds": mesh.outline_edge_ids,
        "revision": mesh.revision,
        "achievedDeflection": mesh.achieved_deflection,
    });
    let traced_mesh = traced::tessellation::tessellate(&traced_body, 0.01, 2_000_000)?;
    let traced_tessellation = json!({
        "positions": traced_mesh.positions,
        "normals": traced_mesh.normals,
        "indices": traced_mesh.indices,
        "triangleFaceIds": traced_mesh.triangle_face_ids,
        "outlinePositions": traced_mesh.outline_positions,
        "outlineEdgeIds": traced_mesh.outline_edge_ids,
        "revision": traced_mesh.revision,
        "achievedDeflection": traced_mesh.achieved_deflection,
    });
    if traced_tessellation != tessellation {
        return Err(format!("traced tessellation differs from main for {name}").into());
    }
    write_json(output.join(format!("{name}.tess.json")), &tessellation)?;
    for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
        let main_step = export_step(body, unit);
        let traced_step = traced::exchange::export_step(&traced_body, unit);
        if format!("{main_step:?}") != format!("{traced_step:?}") {
            return Err(format!("traced STEP differs from main for {name} in {unit}").into());
        }
        match main_step {
            Ok((step, report)) => {
                fs::write(output.join(format!("{name}.step.{suffix}")), step)?;
                write_json(
                    output.join(format!("{name}.step.{suffix}.report.json")),
                    &serde_json::to_value(report)?,
                )?;
            }
            Err(error) => {
                write_json(
                    output.join(format!("{name}.step.{suffix}.error.json")),
                    &json!(error),
                )?;
            }
        }
    }
    Ok(())
}
