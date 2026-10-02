use opengeometry::analytic::{topology::Accuracy, Frame3};
use serde_json::{json, Value};
use std::error::Error;
use traced_analytic::analytic as traced;

pub(super) fn fixture_parameters(name: &str) -> &'static str {
    match name {
        "cuboid" => "size=[2,1,3]",
        "cylinder" => "radius=1, height=2",
        "sphere" => "radius=1",
        "cone" => "radius=1, height=2",
        "frustum" => "lowerRadius=1, upperRadius=0.4, height=2",
        "torus" => "majorRadius=2, minorRadius=0.5",
        "annular-cylinder" | "cylinder-with-hole" => "innerRadius=0.4, outerRadius=1, height=2",
        "circle" => "radius=1, start=0, sweep=TAU",
        "linear-extrusion" => "outer=[[-1,-0.5],[1,-0.5],[1,0.5],[-1,0.5]], height=2",
        "arc-edged-extrusion" => "rounded outer profile, height=2",
        "arc-edged-extrusion-with-holes" => {
            "rounded outer profile, square hole [-0.5,0.5], height=2"
        }
        "box-union" | "box-intersection" | "box-cut" => {
            "box-host size=[3,3,3], box-cutter origin=[1.5,0.5,0.5] size=[3,3,3]"
        }
        "box-cavity" => "box-host size=[3,3,3], box-inner origin=[1,1,1] size=[1,1,1]",
        "sphere-cut" => "sphere-host radius=2, sphere-cutter origin=[0.7,0,0] radius=1",
        _ => "unknown",
    }
}

pub(super) fn traced_fixture(
    name: &str,
    frame: Frame3,
    accuracy: Accuracy,
) -> Result<traced::BrepEnvelope, Box<dyn Error>> {
    let frame = traced::Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: frame.y,
        z: frame.z,
    };
    let accuracy = traced::topology::Accuracy {
        geometric: accuracy.geometric,
        intersection: accuracy.intersection,
        tessellation: accuracy.tessellation,
        exchange: accuracy.exchange,
    };
    let result = match name {
        "cuboid" => traced::primitives::cuboid(name.into(), frame, [2.0, 1.0, 3.0], accuracy)?,
        "cylinder" => traced::primitives::cylinder(name.into(), frame, 1.0, 2.0, accuracy)?,
        "sphere" => traced::primitives::sphere(name.into(), frame, 1.0, accuracy)?,
        "cone" => traced::primitives::cone(name.into(), frame, 1.0, 2.0, accuracy)?,
        "frustum" => traced::primitives::frustum(name.into(), frame, 1.0, 0.4, 2.0, accuracy)?,
        "torus" => traced::primitives::torus(name.into(), frame, 2.0, 0.5, accuracy)?,
        "annular-cylinder" => {
            traced::primitives::annular_cylinder(name.into(), frame, 0.4, 1.0, 2.0, accuracy)?
        }
        "cylinder-with-hole" => traced::primitives::cylinder_with_circular_hole(
            name.into(),
            frame,
            frame,
            0.4,
            1.0,
            2.0,
            accuracy,
        )?,
        "circle" => traced::primitives::arc_wire(
            name.into(),
            traced::CurveGeometry::Circle { frame, radius: 1.0 },
            0.0,
            std::f64::consts::TAU,
            accuracy,
        )?,
        "linear-extrusion" => traced::primitives::linear_extrusion(
            name.into(),
            frame,
            vec![[-1.0, -0.5], [1.0, -0.5], [1.0, 0.5], [-1.0, 0.5]],
            Vec::new(),
            2.0,
            accuracy,
        )?,
        "arc-edged-extrusion" => traced::primitives::arc_edged_extrusion(
            name.into(),
            frame,
            serde_json::from_value(arc_outer())?,
            2.0,
            accuracy,
        )?,
        "arc-edged-extrusion-with-holes" => traced::primitives::arc_edged_extrusion_with_holes(
            name.into(),
            frame,
            serde_json::from_value(arc_outer())?,
            vec![serde_json::from_value(arc_hole())?],
            2.0,
            accuracy,
        )?,
        "box-union" | "box-intersection" | "box-cut" | "box-cavity" => {
            traced_box_boolean_fixture(name, frame, accuracy)?
        }
        "sphere-cut" => traced_sphere_cut_fixture(name, frame, accuracy)?,
        _ => return Err(format!("unknown fixture {name}").into()),
    };
    Ok(result)
}

fn traced_box_boolean_fixture(
    name: &str,
    frame: traced::Frame3,
    accuracy: traced::topology::Accuracy,
) -> Result<traced::BrepEnvelope, Box<dyn Error>> {
    let host = traced::primitives::cuboid("box-host".into(), frame, [3.0; 3], accuracy)?;
    let (cutter, operation) = if name == "box-cavity" {
        (
            traced::primitives::cuboid(
                "box-inner".into(),
                traced::Frame3 {
                    origin: frame.point([1.0; 3]),
                    ..frame
                },
                [1.0; 3],
                accuracy,
            )?,
            traced::booleans::BooleanOp::Subtraction,
        )
    } else {
        let cutter = traced::primitives::cuboid(
            "box-cutter".into(),
            traced::Frame3 {
                origin: frame.point([1.5, 0.5, 0.5]),
                ..frame
            },
            [3.0; 3],
            accuracy,
        )?;
        let operation = match name {
            "box-union" => traced::booleans::BooleanOp::Union,
            "box-intersection" => traced::booleans::BooleanOp::Intersection,
            _ => traced::booleans::BooleanOp::Subtraction,
        };
        (cutter, operation)
    };
    Ok(traced::booleans::boolean_brep(&host, &cutter, operation, name.into())?.brep)
}

fn traced_sphere_cut_fixture(
    name: &str,
    frame: traced::Frame3,
    accuracy: traced::topology::Accuracy,
) -> Result<traced::BrepEnvelope, Box<dyn Error>> {
    let host = traced::primitives::sphere("sphere-host".into(), frame, 2.0, accuracy)?;
    let cutter = traced::primitives::sphere(
        "sphere-cutter".into(),
        traced::Frame3 {
            origin: frame.point([0.7, 0.0, 0.0]),
            ..frame
        },
        1.0,
        accuracy,
    )?;
    Ok(traced::booleans::boolean_brep(
        &host,
        &cutter,
        traced::booleans::BooleanOp::Subtraction,
        name.into(),
    )?
    .brep)
}

pub(super) fn arc_outer() -> Value {
    json!([
        { "kind": "line", "from": [-2.0, -2.0], "to": [2.0, -2.0] },
        { "kind": "arc", "center": [2.0, 0.0], "radius": 2.0,
          "start_angle": -std::f64::consts::FRAC_PI_2,
          "sweep_angle": std::f64::consts::PI },
        { "kind": "line", "from": [2.0, 2.0], "to": [-2.0, 2.0] },
        { "kind": "line", "from": [-2.0, 2.0], "to": [-2.0, -2.0] }
    ])
}

pub(super) fn arc_hole() -> Value {
    json!([
        { "kind": "line", "from": [-0.5, -0.5], "to": [0.5, -0.5] },
        { "kind": "line", "from": [0.5, -0.5], "to": [0.5, 0.5] },
        { "kind": "line", "from": [0.5, 0.5], "to": [-0.5, 0.5] },
        { "kind": "line", "from": [-0.5, 0.5], "to": [-0.5, -0.5] }
    ])
}
