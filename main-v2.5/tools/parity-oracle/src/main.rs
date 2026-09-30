mod builders;
mod compare;
mod matrix;
mod validity;
mod write;

use crate::builders::{arc_hole, arc_outer, fixture_parameters};
use crate::compare::write_fixture;
use crate::matrix::write_boolean_matrix;
use crate::validity::{write_validity, write_validity_case};
use opengeometry::analytic::{
    booleans::{boolean_brep, BooleanOp},
    primitives,
    topology::{Accuracy, BrepEnvelope},
    CurveGeometry, Frame3,
};
use std::{error::Error, fs, path::PathBuf};

struct BooleanOperands {
    box_host: BrepEnvelope,
    box_cutter: BrepEnvelope,
    box_inner: BrepEnvelope,
    sphere_host: BrepEnvelope,
    sphere_cutter: BrepEnvelope,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../opengeometry/tests/fixtures/parity"));
    fs::create_dir_all(&output)?;
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = Frame3 {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    };
    let operands = boolean_operands(frame, accuracy)?;
    let bodies: Vec<(&str, BrepEnvelope)> = primitive_bodies(frame, accuracy)?
        .into_iter()
        .chain(extrusion_bodies(frame, accuracy)?)
        .chain(boolean_bodies(&operands)?)
        .collect();
    let mut corpus = String::from("# Parity corpus\n\nAccuracy: geometric=1e-8, intersection=1e-9, tessellation=0.01, exchange=1e-6. Frame: origin=[0,0,0], x=[1,0,0], y=[0,0,-1], z=[0,1,0].\n\n| Fixture | Pinned id | Parameters |\n|---|---|---|\n");
    for (name, body) in bodies {
        write_fixture(&output, name, &body, frame, accuracy)?;
        write_validity_case(
            &output.join("validity"),
            &format!("builder-{name}"),
            &serde_json::to_value(&body)?,
            true,
        )?;
        corpus.push_str(&format!(
            "| {name} | {} | {} |\n",
            body.id,
            fixture_parameters(name)
        ));
    }
    write_validity(&output, frame, accuracy)?;
    write_boolean_matrix(&output, frame, accuracy)?;
    fs::write(output.join("CORPUS.md"), corpus)?;
    Ok(())
}

fn boolean_operands(frame: Frame3, accuracy: Accuracy) -> Result<BooleanOperands, Box<dyn Error>> {
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
    let sphere_host = primitives::sphere("sphere-host".into(), frame, 2.0, accuracy)?;
    let sphere_cutter = primitives::sphere(
        "sphere-cutter".into(),
        Frame3 {
            origin: frame.point([0.7, 0.0, 0.0]),
            ..frame
        },
        1.0,
        accuracy,
    )?;
    Ok(BooleanOperands {
        box_host,
        box_cutter,
        box_inner,
        sphere_host,
        sphere_cutter,
    })
}

fn primitive_bodies(
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<[(&'static str, BrepEnvelope); 9], Box<dyn Error>> {
    Ok([
        (
            "cuboid",
            primitives::cuboid("cuboid".into(), frame, [2.0, 1.0, 3.0], accuracy)?,
        ),
        (
            "cylinder",
            primitives::cylinder("cylinder".into(), frame, 1.0, 2.0, accuracy)?,
        ),
        (
            "sphere",
            primitives::sphere("sphere".into(), frame, 1.0, accuracy)?,
        ),
        (
            "cone",
            primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy)?,
        ),
        (
            "frustum",
            primitives::frustum("frustum".into(), frame, 1.0, 0.4, 2.0, accuracy)?,
        ),
        (
            "torus",
            primitives::torus("torus".into(), frame, 2.0, 0.5, accuracy)?,
        ),
        (
            "annular-cylinder",
            primitives::annular_cylinder(
                "annular-cylinder".into(),
                frame,
                0.4,
                1.0,
                2.0,
                accuracy,
            )?,
        ),
        (
            "cylinder-with-hole",
            primitives::annular_cylinder(
                "cylinder-with-hole".into(),
                frame,
                0.4,
                1.0,
                2.0,
                accuracy,
            )?,
        ),
        (
            "circle",
            primitives::arc_wire(
                "circle".into(),
                CurveGeometry::Circle { frame, radius: 1.0 },
                0.0,
                std::f64::consts::TAU,
                accuracy,
            )?,
        ),
    ])
}

fn extrusion_bodies(
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<[(&'static str, BrepEnvelope); 3], Box<dyn Error>> {
    Ok([
        (
            "linear-extrusion",
            primitives::linear_extrusion(
                "linear-extrusion".into(),
                frame,
                vec![[-1.0, -0.5], [1.0, -0.5], [1.0, 0.5], [-1.0, 0.5]],
                Vec::new(),
                2.0,
                accuracy,
            )?,
        ),
        (
            "arc-edged-extrusion",
            primitives::arc_edged_extrusion(
                "arc-edged-extrusion".into(),
                frame,
                serde_json::from_value(arc_outer())?,
                2.0,
                accuracy,
            )?,
        ),
        (
            "arc-edged-extrusion-with-holes",
            primitives::arc_edged_extrusion_with_holes(
                "arc-edged-extrusion-with-holes".into(),
                frame,
                serde_json::from_value(arc_outer())?,
                vec![serde_json::from_value(arc_hole())?],
                2.0,
                accuracy,
            )?,
        ),
    ])
}

fn boolean_bodies(
    operands: &BooleanOperands,
) -> Result<[(&'static str, BrepEnvelope); 5], Box<dyn Error>> {
    let BooleanOperands {
        box_host,
        box_cutter,
        box_inner,
        sphere_host,
        sphere_cutter,
    } = operands;
    Ok([
        (
            "box-union",
            boolean_brep(box_host, box_cutter, BooleanOp::Union, "box-union".into())?.brep,
        ),
        (
            "box-intersection",
            boolean_brep(
                box_host,
                box_cutter,
                BooleanOp::Intersection,
                "box-intersection".into(),
            )?
            .brep,
        ),
        (
            "box-cut",
            boolean_brep(
                box_host,
                box_cutter,
                BooleanOp::Subtraction,
                "box-cut".into(),
            )?
            .brep,
        ),
        (
            "box-cavity",
            boolean_brep(
                box_host,
                box_inner,
                BooleanOp::Subtraction,
                "box-cavity".into(),
            )?
            .brep,
        ),
        (
            "sphere-cut",
            boolean_brep(
                sphere_host,
                sphere_cutter,
                BooleanOp::Subtraction,
                "sphere-cut".into(),
            )?
            .brep,
        ),
    ])
}
