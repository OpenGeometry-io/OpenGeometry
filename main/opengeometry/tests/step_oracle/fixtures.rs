use crate::support::{fixture_names, fixture_text, parse_and_match, unit_scale};
use opengeometry::{brep::BrepEnvelope, exchange::export_step};
use opengeometry_test_support::part21::{
    normalise_step, pcurve_expectations, references, Document,
};
use serde_json::Value;
use std::slice::from_ref;

const UNITS: [(&str, &str); 2] = [("metre", "m"), ("millimetre", "mm")];

struct ConePcurve {
    point: usize,
    point_line: String,
    pair: [u64; 4],
}

fn single_body_fixtures() -> Vec<(String, BrepEnvelope)> {
    fixture_names("")
        .into_iter()
        .filter_map(|file| {
            let name = file.strip_suffix(".brep.json")?.to_string();
            let body = BrepEnvelope::from_json(&fixture_text(&file).unwrap()).unwrap();
            Some((name, body))
        })
        .collect()
}

fn named_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            line.starts_with("FILE_NAME(")
                || ["=PRODUCT(", "=MANIFOLD_SOLID_BREP(", "=BREP_WITH_VOIDS("]
                    .iter()
                    .any(|kind| line.contains(kind))
        })
        .collect()
}

fn assert_report_fixture(report: &Value, source: &str, label: &str) {
    let source: Value = serde_json::from_str(source).unwrap();
    let (actual, source) = (report.as_object().unwrap(), source.as_object().unwrap());
    assert!(actual.keys().eq(source.keys()), "{label} report keys");
    for (key, value) in actual {
        if key != "validation_level" {
            assert_eq!(value, &source[key], "{label} report {key}");
        }
    }
}

fn assert_export_matches_source(directory: &str, name: &str, body: &BrepEnvelope) {
    for (unit, suffix) in UNITS {
        let stem = format!("{directory}{name}.step.{suffix}");
        let actual = export_step(body, unit);
        let Some(source) = fixture_text(&stem) else {
            let error = fixture_text(&format!("{stem}.error.json"))
                .unwrap_or_else(|| panic!("missing STEP oracle outcome for {name} {unit}"));
            assert_eq!(
                serde_json::to_value(actual.unwrap_err()).unwrap(),
                serde_json::from_str::<Value>(&error).unwrap(),
                "{name} {unit}"
            );
            continue;
        };
        let (text, report) = actual.unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
        let report = serde_json::to_value(&report).unwrap();
        let parsed = parse_and_match(&text, &report, unit, from_ref(body));
        assert_eq!(parsed.count("POLY_LOOP"), 0);
        assert!(text.contains("\nFILE_DESCRIPTION(('OpenGeometry analytic BRep'),'2;1');\n"));
        assert_eq!(named_lines(&text), named_lines(&source), "{name} {unit}");
        assert_report_fixture(
            &report,
            &fixture_text(&format!("{stem}.report.json")).unwrap(),
            &format!("{name} {unit}"),
        );
        assert_eq!(
            normalise_step(&text),
            normalise_step(&source),
            "{name} {unit}"
        );
    }
}

#[test]
fn single_body_step_matches_source_fixtures_in_both_units() {
    let fixtures = single_body_fixtures();
    assert_eq!(fixtures.len(), 17);
    for (name, body) in fixtures {
        assert_export_matches_source("", &name, &body);
    }
}

fn assert_matrix_step_matches_source(matrix: &str, count: usize, skipped: &[&str]) {
    let cases = fixture_names(matrix)
        .into_iter()
        .filter(|name| name.ends_with(".json") && !skipped.iter().any(|part| name.contains(part)))
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), count);
    for case in cases {
        let fixture: Value =
            serde_json::from_str(&fixture_text(&format!("{matrix}/{case}")).unwrap()).unwrap();
        if fixture["result"].get("error").is_some() {
            continue;
        }
        let body: BrepEnvelope = serde_json::from_value(fixture["result"]["brep"].clone()).unwrap();
        let name = case.trim_end_matches(".json");
        assert_export_matches_source(&format!("{matrix}/"), name, &body);
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

fn cylinder_cross() -> BrepEnvelope {
    let fixture: Value =
        serde_json::from_str(&fixture_text("boolean-matrix/cylinder-cross.json").unwrap()).unwrap();
    serde_json::from_value(fixture["result"]["brep"].clone()).unwrap()
}

#[test]
fn nonparallel_cylinder_step_has_fitted_curves_within_exchange_budget() {
    let body = cylinder_cross();
    let (text, report) = export_step(&body, "metre").unwrap();
    assert!(text.contains("B_SPLINE_CURVE_WITH_KNOTS"));
    assert!(report.exchange_error_bound <= 1e-6);
    let report = serde_json::to_value(&report).unwrap();
    let parsed = parse_and_match(&text, &report, "metre", from_ref(&body));
    assert!(parsed.count("B_SPLINE_CURVE_WITH_KNOTS") > 0);
    let direction = parsed.parameter_direction_report().unwrap();
    assert!(direction.assessed > 0);
    assert_eq!(direction.assessed, direction.aligned + direction.reversed);
}

#[test]
fn nonparallel_cylinder_step_size_stays_within_its_baseline() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../scripts/bench/performance-baseline.json"
    ))
    .unwrap();
    let budget = baseline["sizes"]["nonparallelCylinderStepBytes"]
        .as_u64()
        .expect("sizes.nonparallelCylinderStepBytes in the performance baseline");
    let (text, _) = export_step(&cylinder_cross(), "metre").unwrap();
    let bytes = text.len() as u64;
    assert!(
        bytes * 4 <= budget * 5,
        "{bytes} bytes exceed 1.25 x {budget}"
    );
}

fn cone_pcurves(parsed: &Document) -> Vec<ConePcurve> {
    let entity = |id: usize| parsed.entity(id).unwrap();
    let cone = (1..=parsed.entity_count())
        .find(|id| entity(*id).starts_with("CONICAL_SURFACE("))
        .unwrap();
    assert_eq!(entity(cone).split(',').nth(2), Some("0."));
    let points = parsed.points2().unwrap();
    let directions = parsed.directions2().unwrap();
    let mut pcurves = Vec::new();
    for id in 1..=parsed.entity_count() {
        let expression = entity(id);
        if !expression.starts_with("PCURVE(") || references(expression)[0] != cone {
            continue;
        }
        let line = references(entity(references(expression)[1]))[0];
        let [point, vector] = references(entity(line))[..] else {
            panic!("#{line} is not a pcurve LINE");
        };
        let direction = references(entity(vector))[0];
        let ([u, v], [du, dv]) = (points[&point], directions[&direction]);
        pcurves.push(ConePcurve {
            point,
            point_line: format!("#{point}={};", entity(point)),
            pair: [u, v, du, dv].map(f64::to_bits),
        });
    }
    pcurves
}

fn cone_expectations(brep: &BrepEnvelope, unit: &str) -> Vec<[u64; 4]> {
    pcurve_expectations(brep, unit_scale(unit))
        .into_iter()
        .filter(|expectation| expectation.surface == "Cone")
        .filter_map(|expectation| expectation.bits)
        .collect()
}

fn axial_rewrite(text: &str, pcurves: &[ConePcurve], semi_angle: f64) -> String {
    let mut rewritten = text.to_string();
    for pcurve in pcurves {
        let line = format!(
            "#{}=CARTESIAN_POINT('',({:.17E},{:.17E}));",
            pcurve.point,
            f64::from_bits(pcurve.pair[0]),
            f64::from_bits(pcurve.pair[1]) * semi_angle.cos()
        );
        rewritten = rewritten.replace(&pcurve.point_line, &line);
    }
    rewritten
}

#[test]
fn cone_pcurve_v_measures_distance_along_the_generator() {
    let body: Value = serde_json::from_str(&fixture_text("cone.brep.json").unwrap()).unwrap();
    let brep = BrepEnvelope::from_json(&body.to_string()).unwrap();
    let semi_angle = body["geometry"]["surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|surface| surface["kind"] == "Cone")
        .unwrap()["semi_angle"]
        .as_f64()
        .unwrap();
    for (unit, _) in UNITS {
        let (text, report) = export_step(&brep, unit).unwrap();
        let report = serde_json::to_value(&report).unwrap();
        let parsed = parse_and_match(&text, &report, unit, from_ref(&brep));
        let pcurves = cone_pcurves(&parsed);
        let mut actual = pcurves.iter().map(|pcurve| pcurve.pair).collect::<Vec<_>>();
        let mut expected = cone_expectations(&brep, unit);
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{unit}");
        let rewritten = axial_rewrite(&text, &pcurves, semi_angle);
        assert_ne!(rewritten, text, "{unit}");
        match Document::parse(&rewritten) {
            Ok(_) => panic!("{unit}: the axial-height pcurve parsed"),
            Err(error) => assert!(error.contains("misses 3D curve"), "{unit}: {error}"),
        }
    }
}

fn conic_uses(body: &BrepEnvelope) -> usize {
    pcurve_expectations(body, 1.0)
        .iter()
        .filter(|expectation| expectation.curve == "Circle" || expectation.curve == "Ellipse")
        .count()
}

#[test]
fn conic_pcurves_are_assessed_for_parameter_direction() {
    for (name, body) in single_body_fixtures() {
        for (unit, suffix) in UNITS {
            if fixture_text(&format!("{name}.step.{suffix}")).is_none() {
                continue;
            }
            let (text, _) = export_step(&body, unit).unwrap();
            let direction = Document::parse(&text)
                .and_then(|parsed| parsed.parameter_direction_report())
                .unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
            assert_eq!(direction.conic_assessed, conic_uses(&body), "{name} {unit}");
            assert_eq!(
                direction.conic_assessed,
                direction.conic_aligned + direction.conic_reversed,
                "{name} {unit}"
            );
        }
    }
}
