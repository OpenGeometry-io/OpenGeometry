use opengeometry::{brep::BrepEnvelope, exchange::export_step};
use opengeometry_test_support::part21;
use std::{fs, path::Path};

fn assert_placed_values(body: &BrepEnvelope, parsed: &part21::Document, unit: &str) {
    let scale = if unit == "millimetre" { 1000.0 } else { 1.0 };
    let normalize = |value: f64| if value == 0.0 { 0.0 } else { value };
    let positions = parsed.vertex_positions().unwrap();
    assert_eq!(positions.len(), body.topology.vertices.len());
    for (actual, vertex) in positions.into_iter().zip(&body.topology.vertices) {
        for (axis, value) in actual.iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                normalize(vertex.position[axis] * scale).to_bits()
            );
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    let expected = body
        .topology
        .faces
        .iter()
        .filter_map(|face| {
            if !seen.insert(face.surface) {
                return None;
            }
            match &body.geometry.surfaces[face.surface as usize] {
                opengeometry::brep::SurfaceGeometry::Cylinder { radius, .. } => {
                    Some(("cylinder", vec![*radius * scale]))
                }
                opengeometry::brep::SurfaceGeometry::Sphere { radius, .. } => {
                    Some(("sphere", vec![*radius * scale]))
                }
                opengeometry::brep::SurfaceGeometry::Torus {
                    major_radius,
                    minor_radius,
                    ..
                } => Some(("torus", vec![*major_radius * scale, *minor_radius * scale])),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let actual = parsed.surface_radii().unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((actual_kind, actual_radii), (expected_kind, expected_radii)) in
        actual.iter().zip(expected)
    {
        assert_eq!(actual_kind, expected_kind);
        for (actual, expected) in actual_radii.iter().zip(expected_radii) {
            assert_eq!(actual.to_bits(), normalize(expected).to_bits());
        }
    }
}

fn normalize_source(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with("FILE_DESCRIPTION(") {
                "FILE_DESCRIPTION(('OpenGeometry analytic BRep'),'2;1');".to_string()
            } else {
                line.replace("-0.00000000000000000E0", "0.00000000000000000E0")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[test]
fn single_body_step_matches_source_fixtures_in_both_units() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");
    let mut files = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".brep.json"))
        })
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files.len(), 17);
    for path in files {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .trim_end_matches(".brep.json")
            .to_string();
        let body = BrepEnvelope::from_json(&fs::read_to_string(&path).unwrap()).unwrap();
        for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
            let source_path = directory.join(format!("{name}.step.{suffix}"));
            let error_path = directory.join(format!("{name}.step.{suffix}.error.json"));
            let actual = export_step(&body, unit);
            if source_path.exists() {
                let (text, _) = actual.unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
                let parsed = part21::Document::parse(&text)
                    .unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
                assert_eq!(parsed.count("POLY_LOOP"), 0);
                assert_placed_values(&body, &parsed, unit);
                let source = fs::read_to_string(source_path).unwrap();
                assert_eq!(text, normalize_source(&source), "{name} {unit}");
            } else {
                let error = actual.unwrap_err();
                let source: serde_json::Value =
                    serde_json::from_slice(&fs::read(error_path).unwrap()).unwrap();
                assert_eq!(
                    serde_json::to_value(error).unwrap(),
                    source,
                    "{name} {unit}"
                );
            }
        }
    }
}

fn assert_matrix_step_matches_source(matrix: &str, count: usize, skipped: &[&str]) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity")
        .join(matrix);
    let mut cases = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            path.extension()
                .is_some_and(|extension| extension == "json")
                && !skipped.iter().any(|part| name.contains(part))
        })
        .collect::<Vec<_>>();
    cases.sort();
    assert_eq!(cases.len(), count);
    for case in cases {
        let name = case.file_stem().unwrap().to_string_lossy().to_string();
        let fixture: serde_json::Value = serde_json::from_slice(&fs::read(&case).unwrap()).unwrap();
        if fixture["result"].get("error").is_some() {
            continue;
        }
        let body: BrepEnvelope = serde_json::from_value(fixture["result"]["brep"].clone()).unwrap();
        for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
            let source_path = directory.join(format!("{name}.step.{suffix}"));
            let error_path = directory.join(format!("{name}.step.{suffix}.error.json"));
            let actual = export_step(&body, unit);
            if source_path.exists() {
                let (text, _) = actual.unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
                let parsed = part21::Document::parse(&text)
                    .unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
                assert_placed_values(&body, &parsed, unit);
                assert_eq!(
                    text,
                    normalize_source(&fs::read_to_string(source_path).unwrap()),
                    "{name} {unit}"
                );
            } else {
                assert!(
                    error_path.exists(),
                    "missing STEP oracle outcome for {name} {unit}"
                );
                let source: serde_json::Value =
                    serde_json::from_slice(&fs::read(error_path).unwrap()).unwrap();
                assert_eq!(
                    serde_json::to_value(actual.unwrap_err()).unwrap(),
                    source,
                    "{name} {unit}"
                );
            }
        }
    }
}

#[test]
fn boolean_matrix_step_matches_source_or_pins_an_error() {
    assert_matrix_step_matches_source("boolean-matrix", 18, &[".step."]);
}

#[test]
fn batch_matrix_step_matches_source_or_pins_an_error() {
    assert_matrix_step_matches_source("batch-matrix", 23, &[".step.", ".fallback."]);
}

#[test]
fn nonparallel_cylinder_step_has_fitted_curves_within_exchange_budget() {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity/boolean-matrix");
    let fixture: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("cylinder-cross.json")).unwrap()).unwrap();
    let body: BrepEnvelope = serde_json::from_value(fixture["result"]["brep"].clone()).unwrap();
    let (text, report) = export_step(&body, "metre").unwrap();
    assert!(text.contains("B_SPLINE_CURVE_WITH_KNOTS"));
    assert!(report.exchange_error_bound <= 1e-6);
    let parsed = part21::Document::parse(&text).unwrap();
    assert!(parsed.count("B_SPLINE_CURVE_WITH_KNOTS") > 0);
    let direction = parsed.parameter_direction_report().unwrap();
    assert!(direction.assessed > 0);
    assert_eq!(direction.assessed, direction.aligned + direction.reversed);
}
