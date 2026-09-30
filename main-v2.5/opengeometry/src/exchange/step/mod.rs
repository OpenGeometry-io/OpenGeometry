mod body;
mod entities;
mod product;
mod report;
#[cfg(test)]
mod tests;

pub(crate) use report::StepBodyInput;
pub use report::{StepBodyReport, StepExportReport, StepReport, StepSkipped};

use super::preflight::preflight;
use crate::brep::{BrepEnvelope, GeometryError};
use crate::exchange::part21::Part21Writer;
use body::{emit_body, BodyEmission};
use product::{emit_context, emit_products, emit_representations};

pub fn export_step(brep: &BrepEnvelope, unit: &str) -> Result<(String, StepReport), GeometryError> {
    let (text, report) = export_bodies(
        &[StepBodyInput {
            og_id: &brep.id,
            shape_id: &brep.id,
            shape_revision: brep.revision,
            brep,
        }],
        unit,
        "Y",
        &brep.id,
        "1970-01-01T00:00:00",
        Vec::new(),
    )?;
    Ok((
        text,
        StepReport {
            schema_version: 2,
            revision: brep.revision.to_string(),
            length_unit: unit.into(),
            quality: brep.quality.clone(),
            faces: report.faces,
            edges: report.edges,
            solids: report.solids,
            cavity_shells: report.cavity_shells,
            collapsed_chart_uses: report.collapsed_chart_uses,
            geometric_tolerance: report.geometric_tolerance,
            exchange_error_bound: report.exchange_error_bound,
            validation_level: report.validation_level,
        },
    ))
}

pub(crate) fn export_bodies(
    bodies: &[StepBodyInput<'_>],
    unit: &str,
    up_axis: &str,
    name: &str,
    timestamp: &str,
    skipped: Vec<StepSkipped>,
) -> Result<(String, StepExportReport), GeometryError> {
    let scale = match unit {
        "metre" => 1.0,
        "millimetre" => 1000.0,
        _ => {
            return Err(GeometryError::InvalidGeometry(
                "STEP length unit must be metre or millimetre".into(),
            ))
        }
    };
    if bodies.is_empty() {
        return Err(GeometryError::InvalidGeometry(
            "STEP export has no solid bodies".into(),
        ));
    }
    let mut writer = Part21Writer::new("AUTOMOTIVE_DESIGN");
    writer.set_file_name(name);
    writer.set_timestamp(timestamp);
    writer.set_description("OpenGeometry analytic BRep");
    let context2 =
        writer.add_entity("(GEOMETRIC_REPRESENTATION_CONTEXT(2) REPRESENTATION_CONTEXT('',''))");
    let emitted = add_bodies(&mut writer, bodies, scale, context2)?;
    let bound = emitted
        .iter()
        .map(|body| body.exchange_bound)
        .fold(0.0_f64, f64::max);
    let context = emit_context(&mut writer, unit, bound, scale);
    let representations = emit_representations(&mut writer, &emitted, context);
    let names = bodies.iter().map(|body| body.og_id).collect::<Vec<_>>();
    emit_products(&mut writer, &names, &representations);
    let entities = writer.entity_count();
    if entities > 2_000_000 {
        return Err(GeometryError::LimitExceeded(
            "STEP file exceeds 2,000,000 entities".into(),
        ));
    }
    let text = writer.build()?;
    let body_reports = step_body_reports(bodies, &emitted, scale);
    let report = StepExportReport {
        unit: unit.into(),
        up_axis: up_axis.into(),
        timestamp: timestamp.into(),
        products: bodies.len(),
        solids: emitted.iter().map(|body| body.solids.len()).sum(),
        faces: emitted.iter().map(|body| body.faces).sum(),
        edges: emitted.iter().map(|body| body.edges).sum(),
        cavity_shells: emitted.iter().map(|body| body.cavity_shells).sum(),
        collapsed_chart_uses: emitted.iter().map(|body| body.collapsed_chart_uses).sum(),
        fitted_curves: emitted.iter().map(|body| body.fitted_curves).sum(),
        pcurveless_edges: emitted.iter().map(|body| body.pcurveless_edges).sum(),
        entities,
        bytes: text.len(),
        exchange_error_bound: bound * scale,
        geometric_tolerance: bodies
            .iter()
            .map(|body| body.brep.accuracy.geometric)
            .fold(0.0_f64, f64::max)
            * scale,
        validation_level: "structure, sampled residuals and Part-21 references",
        bodies: body_reports,
        skipped,
    };
    Ok((text, report))
}

fn add_bodies(
    writer: &mut Part21Writer,
    bodies: &[StepBodyInput<'_>],
    scale: f64,
    context2: usize,
) -> Result<Vec<BodyEmission>, GeometryError> {
    let mut emitted = Vec::with_capacity(bodies.len());
    for input in bodies {
        let initial_bound = preflight(input.brep, scale)?;
        emitted.push(emit_body(
            writer,
            input.brep,
            scale,
            context2,
            initial_bound,
        )?);
    }
    Ok(emitted)
}

fn step_body_reports(
    bodies: &[StepBodyInput<'_>],
    emitted: &[BodyEmission],
    scale: f64,
) -> Vec<StepBodyReport> {
    bodies
        .iter()
        .zip(emitted)
        .map(|(input, output)| StepBodyReport {
            og_id: input.og_id.into(),
            shape_id: input.shape_id.into(),
            shape_revision: input.shape_revision,
            solids: output.solids.len(),
            faces: output.faces,
            edges: output.edges,
            cavity_shells: output.cavity_shells,
            collapsed_chart_uses: output.collapsed_chart_uses,
            fitted_curves: output.fitted_curves,
            pcurveless_edges: output.pcurveless_edges,
            exchange_error_bound: output.exchange_bound * scale,
            geometric_tolerance: input.brep.accuracy.geometric * scale,
        })
        .collect::<Vec<_>>()
}
