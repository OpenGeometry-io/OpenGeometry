use opengeometry::brep::{Accuracy, BodyType, CurveGeometry, Frame3};
use opengeometry::primitives;
use opengeometry_test_support::stored;
use serde_json::json;

#[test]
fn primitive_builders_match_stored_json() {
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
    let bodies = [
        (
            "cuboid",
            primitives::cuboid("cuboid".into(), frame, [2.0, 1.0, 3.0], accuracy).unwrap(),
        ),
        (
            "cylinder",
            primitives::cylinder("cylinder".into(), frame, 1.0, 2.0, accuracy).unwrap(),
        ),
        (
            "sphere",
            primitives::sphere("sphere".into(), frame, 1.0, accuracy).unwrap(),
        ),
        (
            "cone",
            primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy).unwrap(),
        ),
        (
            "frustum",
            primitives::frustum("frustum".into(), frame, 1.0, 0.4, 2.0, accuracy).unwrap(),
        ),
        (
            "torus",
            primitives::torus("torus".into(), frame, 2.0, 0.5, accuracy).unwrap(),
        ),
        (
            "annular-cylinder",
            primitives::annular_cylinder("annular-cylinder".into(), frame, 0.4, 1.0, 2.0, accuracy)
                .unwrap(),
        ),
        (
            "cylinder-with-hole",
            primitives::cylinder_with_circular_hole(
                "cylinder-with-hole".into(),
                frame,
                frame,
                0.4,
                1.0,
                2.0,
                accuracy,
            )
            .unwrap(),
        ),
        (
            "circle",
            primitives::arc_wire(
                "circle".into(),
                CurveGeometry::Circle { frame, radius: 1.0 },
                0.0,
                std::f64::consts::TAU,
                accuracy,
            )
            .unwrap(),
        ),
    ];
    for (name, body) in bodies {
        assert_eq!(
            body.body_type().unwrap(),
            if name == "circle" {
                BodyType::Wire
            } else {
                BodyType::Solid
            }
        );
        if stored::compared() {
            let expected = match name {
                "cuboid" => include_str!("../fixtures/cases/cuboid.brep.json"),
                "cylinder" => include_str!("../fixtures/cases/cylinder.brep.json"),
                "sphere" => include_str!("../fixtures/cases/sphere.brep.json"),
                "cone" => include_str!("../fixtures/cases/cone.brep.json"),
                "frustum" => include_str!("../fixtures/cases/frustum.brep.json"),
                "torus" => include_str!("../fixtures/cases/torus.brep.json"),
                "annular-cylinder" => include_str!("../fixtures/cases/annular-cylinder.brep.json"),
                "cylinder-with-hole" => {
                    include_str!("../fixtures/cases/cylinder-with-hole.brep.json")
                }
                "circle" => include_str!("../fixtures/cases/circle.brep.json"),
                _ => unreachable!(),
            };
            assert_eq!(body.to_json().unwrap(), expected, "{name}");
        }
    }
}

#[test]
fn extrusions_match_stored_json() {
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
    let outer = json!([
        { "kind": "line", "from": [-2.0, -2.0], "to": [2.0, -2.0] },
        { "kind": "arc", "center": [2.0, 0.0], "radius": 2.0, "start_angle": -std::f64::consts::FRAC_PI_2, "sweep_angle": std::f64::consts::PI },
        { "kind": "line", "from": [2.0, 2.0], "to": [-2.0, 2.0] },
        { "kind": "line", "from": [-2.0, 2.0], "to": [-2.0, -2.0] }
    ]);
    let hole = json!([
        { "kind": "line", "from": [-0.5, -0.5], "to": [0.5, -0.5] },
        { "kind": "line", "from": [0.5, -0.5], "to": [0.5, 0.5] },
        { "kind": "line", "from": [0.5, 0.5], "to": [-0.5, 0.5] },
        { "kind": "line", "from": [-0.5, 0.5], "to": [-0.5, -0.5] }
    ]);
    let bodies = [
        (
            "linear-extrusion",
            primitives::linear_extrusion(
                "linear-extrusion".into(),
                frame,
                vec![[-1.0, -0.5], [1.0, -0.5], [1.0, 0.5], [-1.0, 0.5]],
                Vec::new(),
                2.0,
                accuracy,
            )
            .unwrap(),
        ),
        (
            "arc-edged-extrusion",
            primitives::arc_edged_extrusion(
                "arc-edged-extrusion".into(),
                frame,
                serde_json::from_value(outer.clone()).unwrap(),
                2.0,
                accuracy,
            )
            .unwrap(),
        ),
        (
            "arc-edged-extrusion-with-holes",
            primitives::arc_edged_extrusion_with_holes(
                "arc-edged-extrusion-with-holes".into(),
                frame,
                serde_json::from_value(outer).unwrap(),
                vec![serde_json::from_value(hole).unwrap()],
                2.0,
                accuracy,
            )
            .unwrap(),
        ),
    ];
    for (name, body) in bodies {
        assert_eq!(body.body_type().unwrap(), BodyType::Solid);
        if stored::compared() {
            let expected = match name {
                "linear-extrusion" => include_str!("../fixtures/cases/linear-extrusion.brep.json"),
                "arc-edged-extrusion" => {
                    include_str!("../fixtures/cases/arc-edged-extrusion.brep.json")
                }
                "arc-edged-extrusion-with-holes" => {
                    include_str!("../fixtures/cases/arc-edged-extrusion-with-holes.brep.json")
                }
                _ => unreachable!(),
            };
            assert_eq!(body.to_json().unwrap(), expected, "{name}");
        }
    }
}
