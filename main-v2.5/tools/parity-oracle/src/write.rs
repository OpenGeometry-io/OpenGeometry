use opengeometry::analytic::{exchange::export_step, topology::BrepEnvelope};
use serde_json::Value;
use std::{error::Error, fs, path::Path, path::PathBuf};

pub(super) fn write_json(path: PathBuf, value: &Value) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string(value)?)?;
    Ok(())
}

pub(super) fn write_step_files(
    directory: &Path,
    name: &str,
    brep: &BrepEnvelope,
) -> Result<(), Box<dyn Error>> {
    for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
        match export_step(brep, unit) {
            Ok((text, report)) => {
                fs::write(directory.join(format!("{name}.step.{suffix}")), text)?;
                write_json(
                    directory.join(format!("{name}.step.{suffix}.report.json")),
                    &serde_json::to_value(report)?,
                )?;
            }
            Err(error) => {
                write_json(
                    directory.join(format!("{name}.step.{suffix}.error.json")),
                    &serde_json::to_value(error)?,
                )?;
            }
        }
    }
    Ok(())
}
