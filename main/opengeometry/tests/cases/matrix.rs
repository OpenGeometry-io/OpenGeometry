use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3};
use opengeometry::operations::modifying::boolean::{
    boolean_brep_outcome_with_handlers, boolean_brep_with_handlers, BooleanOp,
};
use opengeometry::primitives;
use opengeometry::query::validity;
use opengeometry_test_support::{stored, volume};
use serde_json::json;

#[test]
fn boolean_fixtures_match_stored_brep_and_handlers() {
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
    let host = primitives::cuboid("box-host".into(), frame, [3.0; 3], accuracy).unwrap();
    let cutter = primitives::cuboid(
        "box-cutter".into(),
        Frame3 {
            origin: frame.point([1.5, 0.5, 0.5]),
            ..frame
        },
        [3.0; 3],
        accuracy,
    )
    .unwrap();
    let inner = primitives::cuboid(
        "box-inner".into(),
        Frame3 {
            origin: frame.point([1.0; 3]),
            ..frame
        },
        [1.0; 3],
        accuracy,
    )
    .unwrap();
    let sphere = primitives::sphere("sphere-host".into(), frame, 2.0, accuracy).unwrap();
    let sphere_cutter = primitives::sphere(
        "sphere-cutter".into(),
        Frame3 {
            origin: frame.point([0.7, 0.0, 0.0]),
            ..frame
        },
        1.0,
        accuracy,
    )
    .unwrap();
    macro_rules! check {
        ($name:literal, $a:expr, $b:expr, $operation:expr) => {{
            let (result, handlers) =
                boolean_brep_with_handlers($a, $b, $operation, $name.into()).unwrap();
            let json = result.brep.to_json().unwrap();
            if stored::compared() {
                assert_eq!(
                    json,
                    include_str!(concat!("../fixtures/cases/", $name, ".brep.json")),
                    $name
                );
            }
            let expected: Vec<String> = serde_json::from_str(include_str!(concat!(
                "../fixtures/cases/",
                $name,
                ".handlers.json"
            )))
            .unwrap();
            assert_eq!(handlers, expected, $name);
        }};
    }
    check!("box-union", &host, &cutter, BooleanOp::Union);
    check!("box-intersection", &host, &cutter, BooleanOp::Intersection);
    check!("box-cut", &host, &cutter, BooleanOp::Subtraction);
    check!("box-cavity", &host, &inner, BooleanOp::Subtraction);
    check!(
        "sphere-cut",
        &sphere,
        &sphere_cutter,
        BooleanOp::Subtraction
    );
}

#[test]
fn boolean_support_matrix_matches_stored_outcomes_and_handlers() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases/boolean-matrix");
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files.len(), 18);
    for path in files {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let a: BrepEnvelope = serde_json::from_value(fixture["a"].clone()).unwrap();
        let b: BrepEnvelope = serde_json::from_value(fixture["b"].clone()).unwrap();
        let operation: BooleanOp = serde_json::from_value(fixture["operation"].clone()).unwrap();
        let (result, handlers) =
            boolean_brep_outcome_with_handlers(&a, &b, operation, format!("matrix-{name}"));
        let actual = match result {
            Ok(result) => {
                result.brep.validate().unwrap();
                let expected_volume = match name {
                    "box-union" => Some(27.0),
                    "box-intersection" => Some(8.0),
                    "box-subtract" => Some(19.0),
                    "wall-rectilinear" | "planar-extrusions" => Some(4.8),
                    "wall-round" => Some(6.0 - std::f64::consts::PI / 8.0),
                    "box-round" => Some(27.0 - 3.0 * std::f64::consts::PI * 0.4 * 0.4),
                    "cylinder-coaxial" => Some(3.0 * std::f64::consts::PI * (1.0 - 0.4 * 0.4)),
                    "cylinder-sliced" => Some(std::f64::consts::PI * (3.0 - 0.4 * 0.4 * 2.5)),
                    "sphere-contained" => Some(10.5 * std::f64::consts::PI),
                    "cone-contained" => Some(std::f64::consts::PI * (1.0 - 0.04 / 3.0)),
                    "torus-contained" => Some(1.28 * std::f64::consts::PI.powi(2)),
                    _ => None,
                };
                if let Some(expected) = expected_volume {
                    let deflection = if name == "cylinder-sliced" {
                        1e-4
                    } else {
                        0.01
                    };
                    let measured = volume::estimate(&result.brep, deflection);
                    assert!(
                        (measured.value - expected).abs() <= measured.error_bound,
                        "{name}: measured={} expected={} bound={}",
                        measured.value,
                        expected,
                        measured.error_bound,
                    );
                }
                json!({"brep": serde_json::to_value(result.brep).unwrap()})
            }
            Err(error) => json!({"error": serde_json::to_value(error).unwrap()}),
        };
        if stored::compared() || fixture["result"].get("error").is_some() {
            assert_eq!(actual, fixture["result"], "{name}");
        }
        assert_eq!(json!(handlers), fixture["handlers"], "{name}");
    }
}

#[test]
fn rotated_box_union_and_intersection_build_through_planar_polyhedra() {
    let fixture = |row: &str| -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/cases/boolean-matrix/{row}.json"));
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };
    let rows = [
        (fixture("rotated-box-union"), BooleanOp::Union),
        (fixture("rotated-box-intersection"), BooleanOp::Intersection),
    ];
    assert_eq!(rows[0].0["a"], rows[1].0["a"]);
    assert_eq!(rows[0].0["b"], rows[1].0["b"]);
    let a: BrepEnvelope = serde_json::from_value(rows[0].0["a"].clone()).unwrap();
    let b: BrepEnvelope = serde_json::from_value(rows[0].0["b"].clone()).unwrap();
    let [union, intersection] = rows.map(|(_, operation)| {
        let (result, handlers) =
            boolean_brep_outcome_with_handlers(&a, &b, operation, format!("{operation:?}"));
        let body = result.unwrap().brep;
        body.validate().unwrap();
        let checked = validity(&body);
        assert!(checked.valid && checked.closed, "{operation:?}");
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(
            value["solids"].as_array().unwrap().len(),
            1,
            "{operation:?}"
        );
        assert_eq!(handlers, ["boolean_planar_polyhedra"], "{operation:?}");
        volume::estimate(&body, 0.01)
    });
    let [a, b] = [&a, &b].map(|body| volume::estimate(body, 0.01));
    let slack = a.error_bound + b.error_bound + union.error_bound + intersection.error_bound;
    assert!(intersection.value > 0.0);
    assert!(
        (union.value - (a.value + b.value - intersection.value)).abs() <= slack + 1e-9,
        "union={} a={} b={} intersection={}",
        union.value,
        a.value,
        b.value,
        intersection.value
    );
}

#[test]
fn parallel_boolean_candidates_have_identical_outputs() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cases/boolean-matrix/wall-round.json");
    let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let a: BrepEnvelope = serde_json::from_value(fixture["a"].clone()).unwrap();
    let b: BrepEnvelope = serde_json::from_value(fixture["b"].clone()).unwrap();
    let outputs = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            let (result, handlers) = boolean_brep_outcome_with_handlers(
                &a,
                &b,
                BooleanOp::Subtraction,
                "matrix-wall-round".into(),
            );
            (result.unwrap().brep.to_json().unwrap(), handlers)
        });
        let second = scope.spawn(|| {
            let (result, handlers) = boolean_brep_outcome_with_handlers(
                &a,
                &b,
                BooleanOp::Subtraction,
                "matrix-wall-round".into(),
            );
            (result.unwrap().brep.to_json().unwrap(), handlers)
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    let (first, second) = outputs;
    assert_eq!(first, second);
    assert_eq!(json!(first.1), fixture["handlers"]);
}
