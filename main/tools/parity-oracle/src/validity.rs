use crate::write::write_json;
use opengeometry::analytic::{
    primitives,
    topology::{Accuracy, BrepEnvelope},
    CurveGeometry, Frame3,
};
use serde_json::{json, Value};
use std::{error::Error, fs, path::Path};

pub(super) fn write_validity(
    output: &Path,
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<(), Box<dyn Error>> {
    let directory = output.join("validity");
    fs::create_dir_all(&directory)?;
    let cuboid = primitives::cuboid("validity-cuboid".into(), frame, [2.0; 3], accuracy)?;
    let circle = primitives::arc_wire(
        "validity-circle".into(),
        CurveGeometry::Circle { frame, radius: 1.0 },
        0.0,
        std::f64::consts::TAU,
        accuracy,
    )?;
    let arc = primitives::arc_wire(
        "validity-arc".into(),
        CurveGeometry::Circle { frame, radius: 1.0 },
        0.0,
        std::f64::consts::PI,
        accuracy,
    )?;
    for (name, body) in [("cuboid", &cuboid), ("circle", &circle), ("arc", &arc)] {
        let value = serde_json::to_value(body)?;
        write_validity_case(&directory, name, &value, true)?;
    }
    let mut mutation = serde_json::to_value(&cuboid)?;
    mutation["topology"]["vertices"][0]["id"] = json!(9);
    write_validity_case(&directory, "dense-ids", &mutation, false)?;
    let mut mutation = serde_json::to_value(&cuboid)?;
    mutation["topology"]["halfedges"][0]["twin"] = Value::Null;
    write_validity_case(&directory, "twin-symmetry", &mutation, false)?;
    let mut mutation = serde_json::to_value(&cuboid)?;
    mutation["topology"]["halfedges"][0]["next"] = Value::Null;
    write_validity_case(&directory, "next-prev", &mutation, false)?;
    let mut mutation = serde_json::to_value(&cuboid)?;
    mutation["topology"]["halfedges"][0]["loop_ref"] = Value::Null;
    write_validity_case(&directory, "loop-membership", &mutation, false)?;
    let mut mutation = serde_json::to_value(&cuboid)?;
    mutation["topology"]["halfedges"][0]["geometry_use"]["pcurve"] = Value::Null;
    write_validity_case(&directory, "face-pcurve", &mutation, false)?;
    let mut mutation = serde_json::to_value(&circle)?;
    mutation["topology"]["halfedges"][0]["wire_ref"] = Value::Null;
    write_validity_case(&directory, "wire-membership", &mutation, false)?;
    let mut mutation = serde_json::to_value(&circle)?;
    mutation["topology"]["wires"][0]["is_closed"] = json!(false);
    write_validity_case(&directory, "closed-wire-cycle", &mutation, false)?;
    let mut mutation = serde_json::to_value(&arc)?;
    mutation["topology"]["wires"][0]["is_closed"] = json!(true);
    write_validity_case(&directory, "open-wire-end", &mutation, false)?;
    Ok(())
}

pub(super) fn write_validity_case(
    directory: &Path,
    name: &str,
    value: &Value,
    expected_valid: bool,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let source = serde_json::to_string(value)?;
    let verdict = BrepEnvelope::from_json(&source);
    if verdict.is_ok() != expected_valid {
        return Err(format!("validity fixture {name} has unexpected verdict").into());
    }
    fs::write(directory.join(format!("{name}.json")), source)?;
    let result = match verdict {
        Ok(_) => json!({ "valid": true }),
        Err(error) => json!({ "valid": false, "error": error }),
    };
    write_json(directory.join(format!("{name}.verdict.json")), &result)?;
    Ok(())
}
