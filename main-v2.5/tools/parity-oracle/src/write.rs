use serde_json::Value;
use std::{error::Error, fs, path::PathBuf};

pub(super) fn write_json(path: PathBuf, value: &Value) -> Result<(), Box<dyn Error>> {
    fs::write(path, serde_json::to_string(value)?)?;
    Ok(())
}
