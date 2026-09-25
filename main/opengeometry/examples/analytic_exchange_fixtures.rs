use opengeometry::analytic::{
    booleans::{boolean_brep, BooleanOp},
    ifc_exchange::prepare_ifc_body,
    primitives,
    topology::Accuracy,
    Frame3,
};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("provide an output JSON path")?;
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = Frame3 {
        origin: [1.0, 2.0, 3.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    };
    let host = primitives::sphere("host".into(), frame, 2.0, accuracy)?;
    let cutter = primitives::sphere(
        "cutter".into(),
        Frame3 {
            origin: [2.7, 2.0, 3.0],
            ..frame
        },
        1.0,
        accuracy,
    )?;
    let inner = primitives::sphere("inner".into(), frame, 0.5, accuracy)?;
    let box_host = primitives::cuboid("box-host".into(), frame, [3.0; 3], accuracy)?;
    let box_cutter = primitives::cuboid(
        "box-cutter".into(),
        Frame3 {
            origin: frame.point([1.5, 0.5, 0.5]),
            ..frame
        },
        [3.0; 3],
        accuracy,
    )?;
    let box_inner = primitives::cuboid(
        "box-inner".into(),
        Frame3 {
            origin: frame.point([1.0; 3]),
            ..frame
        },
        [1.0; 3],
        accuracy,
    )?;
    let bodies = vec![
        primitives::cuboid("cuboid".into(), frame, [2.0, 1.0, 3.0], accuracy)?,
        primitives::cylinder("cylinder".into(), frame, 1.0, 2.0, accuracy)?,
        primitives::sphere("sphere".into(), frame, 1.0, accuracy)?,
        primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy)?,
        primitives::frustum("frustum".into(), frame, 1.0, 0.4, 2.0, accuracy)?,
        primitives::torus("torus".into(), frame, 2.0, 0.5, accuracy)?,
        primitives::annular_sector_extrusion(
            "wall".into(),
            frame,
            2.4,
            0.3,
            1.4,
            0.15,
            -1.8,
            accuracy,
        )?,
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "cut".into())?.brep,
        boolean_brep(&host, &inner, BooleanOp::Subtraction, "cavity".into())?.brep,
        boolean_brep(&box_host, &box_cutter, BooleanOp::Union, "box-union".into())?.brep,
        boolean_brep(
            &box_host,
            &box_cutter,
            BooleanOp::Intersection,
            "box-intersection".into(),
        )?
        .brep,
        boolean_brep(
            &box_host,
            &box_cutter,
            BooleanOp::Subtraction,
            "box-cut".into(),
        )?
        .brep,
        boolean_brep(
            &box_host,
            &box_inner,
            BooleanOp::Subtraction,
            "box-cavity".into(),
        )?
        .brep,
    ];
    let mut records = Vec::new();
    for body in bodies {
        records.push(serde_json::json!({
            "name": body.id, "serialized_brep": body.to_json()?,
            "exchange_body": prepare_ifc_body(&body)?,
        }));
    }
    fs::write(output, serde_json::to_string_pretty(&records)?)?;
    Ok(())
}
