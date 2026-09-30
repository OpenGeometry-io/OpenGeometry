use crate::write::write_json;
use opengeometry::analytic::{
    booleans::{boolean_brep, BooleanOp},
    exchange::export_step,
    primitives,
    topology::{Accuracy, BrepEnvelope},
    Frame3,
};
use serde_json::json;
use std::{error::Error, fs, path::Path};
use traced_analytic::analytic as traced;

struct PlanarOperands {
    box_host: BrepEnvelope,
    box_cutter: BrepEnvelope,
    wall: BrepEnvelope,
    wall_cutter: BrepEnvelope,
    profile_cutter: BrepEnvelope,
    round_cutter: BrepEnvelope,
    box_round_cutter: BrepEnvelope,
}

struct CurvedOperands {
    cylinder_host: BrepEnvelope,
    cylinder_inner: BrepEnvelope,
    cylinder_offset: BrepEnvelope,
    cylinder_cross: BrepEnvelope,
    cylinder_shifted: BrepEnvelope,
    annular_host: BrepEnvelope,
    sphere_host: BrepEnvelope,
    sphere_inner: BrepEnvelope,
    cone_host: BrepEnvelope,
    cone_inner: BrepEnvelope,
    torus_host: BrepEnvelope,
    torus_inner: BrepEnvelope,
}

struct MatrixCase<'a> {
    name: &'static str,
    a: &'a BrepEnvelope,
    b: &'a BrepEnvelope,
    operation: BooleanOp,
}

pub(super) fn write_boolean_matrix(
    output: &Path,
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<(), Box<dyn Error>> {
    let directory = output.join("boolean-matrix");
    fs::create_dir_all(&directory)?;
    let planar = planar_operands(frame, accuracy)?;
    let curved = curved_operands(frame, accuracy)?;
    let rotation = Frame3::from_axis(
        frame.point([1.0, 0.5, 0.5]),
        frame.z,
        [0.8660254037844386, 0.0, 0.5],
    )?;
    let rotated_cutter =
        primitives::cuboid("matrix-rotated-cutter".into(), rotation, [2.0; 3], accuracy)?;
    let cases = planar_cases(&planar)
        .into_iter()
        .chain(curved_cases(&curved, &planar.wall_cutter))
        .chain(rotated_cases(&planar.box_host, &rotated_cutter));
    for case in cases {
        write_matrix_case(&directory, case.name, case.a, case.b, case.operation)?;
    }
    Ok(())
}

fn planar_operands(frame: Frame3, accuracy: Accuracy) -> Result<PlanarOperands, Box<dyn Error>> {
    let box_host = primitives::cuboid("matrix-box-host".into(), frame, [3.0; 3], accuracy)?;
    let box_cutter = primitives::cuboid(
        "matrix-box-cutter".into(),
        Frame3 {
            origin: frame.point([1.0, 0.5, 0.5]),
            ..frame
        },
        [2.0; 3],
        accuracy,
    )?;
    let wall = primitives::linear_extrusion(
        "matrix-wall".into(),
        frame,
        vec![[-2.0, -0.25], [2.0, -0.25], [2.0, 0.25], [-2.0, 0.25]],
        Vec::new(),
        3.0,
        accuracy,
    )?;
    let wall_cutter = primitives::cuboid(
        "matrix-wall-cutter".into(),
        Frame3 {
            origin: frame.point([-0.4, -0.5, 0.0]),
            ..frame
        },
        [0.8, 1.0, 3.0],
        accuracy,
    )?;
    let profile_cutter = primitives::linear_extrusion(
        "matrix-profile-cutter".into(),
        frame,
        vec![[-0.4, -0.5], [0.4, -0.5], [0.4, 0.5], [-0.4, 0.5]],
        Vec::new(),
        3.0,
        accuracy,
    )?;
    let round_cutter = primitives::cylinder(
        "matrix-round-cutter".into(),
        Frame3::from_axis(frame.point([0.0, -1.0, 1.5]), frame.y, frame.x)?,
        0.5,
        2.0,
        accuracy,
    )?;
    let box_round_cutter = primitives::cylinder(
        "matrix-box-round-cutter".into(),
        Frame3 {
            origin: frame.point([1.5, 1.5, -1.0]),
            ..frame
        },
        0.4,
        5.0,
        accuracy,
    )?;
    Ok(PlanarOperands {
        box_host,
        box_cutter,
        wall,
        wall_cutter,
        profile_cutter,
        round_cutter,
        box_round_cutter,
    })
}

fn curved_operands(frame: Frame3, accuracy: Accuracy) -> Result<CurvedOperands, Box<dyn Error>> {
    let cylinder_host =
        primitives::cylinder("matrix-cylinder-host".into(), frame, 1.0, 3.0, accuracy)?;
    let cylinder_inner =
        primitives::cylinder("matrix-cylinder-inner".into(), frame, 0.4, 3.0, accuracy)?;
    let cylinder_offset = primitives::cylinder(
        "matrix-cylinder-offset".into(),
        Frame3 {
            origin: frame.point([0.35, 0.0, 0.0]),
            ..frame
        },
        0.4,
        3.0,
        accuracy,
    )?;
    let cylinder_cross = primitives::cylinder(
        "matrix-cylinder-cross".into(),
        Frame3::from_axis(frame.point([-2.0, 0.0, 1.5]), frame.x, frame.z)?,
        0.35,
        4.0,
        accuracy,
    )?;
    let cylinder_shifted = primitives::cylinder(
        "matrix-cylinder-shifted".into(),
        Frame3 {
            origin: frame.point([0.0, 0.0, 0.5]),
            ..frame
        },
        0.4,
        3.0,
        accuracy,
    )?;
    let annular_host =
        primitives::annular_cylinder("matrix-annular-host".into(), frame, 0.3, 1.0, 3.0, accuracy)?;
    let sphere_host = primitives::sphere("matrix-sphere-host".into(), frame, 2.0, accuracy)?;
    let sphere_inner = primitives::sphere("matrix-sphere-inner".into(), frame, 0.5, accuracy)?;
    let cone_host = primitives::cone("matrix-cone-host".into(), frame, 1.0, 3.0, accuracy)?;
    let cone_inner = primitives::cone(
        "matrix-cone-inner".into(),
        Frame3 {
            origin: frame.point([0.0, 0.0, 0.5]),
            ..frame
        },
        0.2,
        1.0,
        accuracy,
    )?;
    let torus_host = primitives::torus("matrix-torus-host".into(), frame, 2.0, 0.6, accuracy)?;
    let torus_inner = primitives::torus("matrix-torus-inner".into(), frame, 2.0, 0.2, accuracy)?;
    Ok(CurvedOperands {
        cylinder_host,
        cylinder_inner,
        cylinder_offset,
        cylinder_cross,
        cylinder_shifted,
        annular_host,
        sphere_host,
        sphere_inner,
        cone_host,
        cone_inner,
        torus_host,
        torus_inner,
    })
}

fn planar_cases(planar: &PlanarOperands) -> [MatrixCase<'_>; 7] {
    let PlanarOperands {
        box_host,
        box_cutter,
        wall,
        wall_cutter,
        profile_cutter,
        round_cutter,
        box_round_cutter,
    } = planar;
    [
        MatrixCase {
            name: "box-union",
            a: box_host,
            b: box_cutter,
            operation: BooleanOp::Union,
        },
        MatrixCase {
            name: "box-intersection",
            a: box_host,
            b: box_cutter,
            operation: BooleanOp::Intersection,
        },
        MatrixCase {
            name: "box-subtract",
            a: box_host,
            b: box_cutter,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "wall-rectilinear",
            a: wall,
            b: wall_cutter,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "planar-extrusions",
            a: wall,
            b: profile_cutter,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "wall-round",
            a: wall,
            b: round_cutter,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "box-round",
            a: box_host,
            b: box_round_cutter,
            operation: BooleanOp::Subtraction,
        },
    ]
}

fn curved_cases<'a>(
    curved: &'a CurvedOperands,
    wall_cutter: &'a BrepEnvelope,
) -> [MatrixCase<'a>; 9] {
    let CurvedOperands {
        cylinder_host,
        cylinder_inner,
        cylinder_offset,
        cylinder_cross,
        cylinder_shifted,
        annular_host,
        sphere_host,
        sphere_inner,
        cone_host,
        cone_inner,
        torus_host,
        torus_inner,
    } = curved;
    [
        MatrixCase {
            name: "cylinder-coaxial",
            a: cylinder_host,
            b: cylinder_inner,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "cylinder-offset",
            a: cylinder_host,
            b: cylinder_offset,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "cylinder-cross",
            a: cylinder_host,
            b: cylinder_cross,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "cylinder-sliced",
            a: cylinder_host,
            b: cylinder_shifted,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "cylinder-empty",
            a: cylinder_inner,
            b: cylinder_host,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "annular-first",
            a: annular_host,
            b: wall_cutter,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "sphere-contained",
            a: sphere_host,
            b: sphere_inner,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "cone-contained",
            a: cone_host,
            b: cone_inner,
            operation: BooleanOp::Subtraction,
        },
        MatrixCase {
            name: "torus-contained",
            a: torus_host,
            b: torus_inner,
            operation: BooleanOp::Subtraction,
        },
    ]
}

fn rotated_cases<'a>(
    box_host: &'a BrepEnvelope,
    rotated_cutter: &'a BrepEnvelope,
) -> [MatrixCase<'a>; 2] {
    [
        MatrixCase {
            name: "rotated-box-union",
            a: box_host,
            b: rotated_cutter,
            operation: BooleanOp::Union,
        },
        MatrixCase {
            name: "rotated-box-intersection",
            a: box_host,
            b: rotated_cutter,
            operation: BooleanOp::Intersection,
        },
    ]
}

fn write_matrix_case(
    directory: &Path,
    name: &str,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
) -> Result<(), Box<dyn Error>> {
    let id = format!("matrix-{name}");
    let result = boolean_brep(a, b, operation, id.clone());
    let traced_a = traced::BrepEnvelope::from_json(&a.to_json()?)?;
    let traced_b = traced::BrepEnvelope::from_json(&b.to_json()?)?;
    traced::booleans::take_handler_trace();
    let traced_result = traced::booleans::boolean_brep(
        &traced_a,
        &traced_b,
        match operation {
            BooleanOp::Union => traced::booleans::BooleanOp::Union,
            BooleanOp::Intersection => traced::booleans::BooleanOp::Intersection,
            BooleanOp::Subtraction => traced::booleans::BooleanOp::Subtraction,
        },
        id,
    );
    let handlers = traced::booleans::take_handler_trace();
    if let Ok(output) = &result {
        for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
            match export_step(&output.brep, unit) {
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
    }
    let main_value = match result {
        Ok(result) => json!({"brep": serde_json::to_value(result.brep)?}),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    };
    let traced_value = match traced_result {
        Ok(result) => json!({"brep": serde_json::to_value(result.brep)?}),
        Err(error) => json!({"error": serde_json::to_value(error)?}),
    };
    if main_value != traced_value {
        return Err(format!("traced matrix result differs from main for {name}").into());
    }
    write_json(
        directory.join(format!("{name}.json")),
        &json!({
            "a": serde_json::to_value(a)?,
            "b": serde_json::to_value(b)?,
            "operation": operation,
            "result": main_value,
            "handlers": handlers,
        }),
    )?;
    Ok(())
}
