mod parts;
mod scenes;

use crate::write::{write_json, write_step_files};
use opengeometry::analytic::{
    booleans::{boolean_brep, subtract_planar_cutters, BooleanOp},
    topology::{Accuracy, BrepEnvelope},
    Frame3, GeometryError,
};
use parts::body;
use scenes::scenes;
use serde_json::{json, Value};
use std::{error::Error, fs, path::Path};
use traced_analytic::analytic as traced;

struct BatchRow<'a> {
    directory: &'a Path,
    name: &'a str,
    host: &'a BrepEnvelope,
    cutters: &'a [BrepEnvelope],
}

struct TracedInputs {
    host: traced::BrepEnvelope,
    cutters: Vec<traced::BrepEnvelope>,
}

pub(super) fn write_batch_matrix(
    output: &Path,
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<(), Box<dyn Error>> {
    let directory = output.join("batch-matrix");
    fs::create_dir_all(&directory)?;
    for scene in scenes(frame, accuracy) {
        let host = body(scene.host)?;
        let mut cutters = scene
            .cutters
            .into_iter()
            .map(body)
            .collect::<Result<Vec<_>, _>>()?;
        if !scene.ordered {
            write_batch_row(&BatchRow {
                directory: &directory,
                name: scene.name,
                host: &host,
                cutters: &cutters,
            })?;
            continue;
        }
        for order in ["forward", "reverse"] {
            if order == "reverse" {
                cutters.reverse();
            }
            write_batch_row(&BatchRow {
                directory: &directory,
                name: &format!("{}.{order}", scene.name),
                host: &host,
                cutters: &cutters,
            })?;
        }
    }
    Ok(())
}

fn write_batch_row(row: &BatchRow) -> Result<(), Box<dyn Error>> {
    let id = format!("batch-{}", row.name);
    let result = subtract_planar_cutters(row.host, row.cutters, id.clone());
    let inputs = traced_inputs(row)?;
    traced::booleans::take_handler_trace();
    let traced_result =
        traced::booleans::subtract_planar_cutters(&inputs.host, &inputs.cutters, id);
    let handlers = traced::booleans::take_handler_trace();
    let main_value = outcome(result.as_ref().map(|result| &result.brep))?;
    let traced_value = traced_outcome(traced_result.as_ref().map(|result| &result.brep))?;
    if main_value != traced_value {
        return Err(format!("traced batch result differs from main for {}", row.name).into());
    }
    if let Ok(output) = &result {
        write_step_files(row.directory, row.name, &output.brep)?;
    }
    write_row_json(row, "", main_value, handlers)?;
    if matches!(&result, Err(GeometryError::CoverageGap { families })
        if !families[0].contains("mixed cutter batch"))
    {
        write_fallback_row(row)?;
    }
    Ok(())
}

fn write_fallback_row(row: &BatchRow) -> Result<(), Box<dyn Error>> {
    let id = format!("batch-{}", row.name);
    let result =
        row.cutters
            .iter()
            .enumerate()
            .try_fold(row.host.clone(), |current, (index, cutter)| {
                let step_id = format!("{id}:part:{index}");
                boolean_brep(&current, cutter, BooleanOp::Subtraction, step_id)
                    .map(|step| step.brep)
            });
    let inputs = traced_inputs(row)?;
    traced::booleans::take_handler_trace();
    let traced_result = inputs.cutters.iter().enumerate().try_fold(
        inputs.host.clone(),
        |current, (index, cutter)| {
            let step_id = format!("{id}:part:{index}");
            let operation = traced::booleans::BooleanOp::Subtraction;
            traced::booleans::boolean_brep(&current, cutter, operation, step_id)
                .map(|step| step.brep)
        },
    );
    let handlers = traced::booleans::take_handler_trace();
    let main_value = outcome(result.as_ref())?;
    if main_value != traced_outcome(traced_result.as_ref())? {
        return Err(format!("traced serial fallback differs from main for {}", row.name).into());
    }
    write_row_json(row, ".fallback", main_value, handlers)
}

fn traced_inputs(row: &BatchRow) -> Result<TracedInputs, Box<dyn Error>> {
    Ok(TracedInputs {
        host: traced::BrepEnvelope::from_json(&row.host.to_json()?)?,
        cutters: row
            .cutters
            .iter()
            .map(|cutter| traced::BrepEnvelope::from_json(&cutter.to_json()?).map_err(Into::into))
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?,
    })
}

fn outcome(result: Result<&BrepEnvelope, &GeometryError>) -> Result<Value, serde_json::Error> {
    Ok(match result {
        Ok(brep) => json!({"brep": serde_json::to_value(brep)?}),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    })
}

fn traced_outcome(
    result: Result<&traced::BrepEnvelope, &traced::GeometryError>,
) -> Result<Value, serde_json::Error> {
    Ok(match result {
        Ok(brep) => json!({"brep": serde_json::to_value(brep)?}),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    })
}

fn write_row_json(
    row: &BatchRow,
    suffix: &str,
    result: Value,
    handlers: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    write_json(
        row.directory.join(format!("{}{suffix}.json", row.name)),
        &json!({
            "host": serde_json::to_value(row.host)?,
            "cutters": serde_json::to_value(row.cutters)?,
            "result": result,
            "handlers": handlers,
        }),
    )
}
