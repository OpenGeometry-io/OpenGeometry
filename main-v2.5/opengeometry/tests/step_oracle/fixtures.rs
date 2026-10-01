use crate::support::{fixture_names, fixture_text, parse_and_match, unit_scale};
use opengeometry::{brep::BrepEnvelope, exchange::export_step};
use opengeometry_test_support::part21::{normalise_step, Document};
use serde_json::Value;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::slice::from_ref;

const UNITS: [(&str, &str); 2] = [("metre", "m"), ("millimetre", "mm")];

struct ConePcurve {
    point: usize,
    point_line: String,
    pair: [u64; 4],
}

struct ConeUse {
    pair: [u64; 4],
    axial: f64,
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

#[test]
fn nonparallel_cylinder_step_has_fitted_curves_within_exchange_budget() {
    let fixture: Value =
        serde_json::from_str(&fixture_text("boolean-matrix/cylinder-cross.json").unwrap()).unwrap();
    let body: BrepEnvelope = serde_json::from_value(fixture["result"]["brep"].clone()).unwrap();
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

fn entity_lines(text: &str) -> BTreeMap<usize, &str> {
    text.lines()
        .filter_map(|line| {
            let (id, expression) = line.strip_prefix('#')?.split_once('=')?;
            Some((id.parse().unwrap(), expression))
        })
        .collect()
}

fn references(expression: &str) -> Vec<usize> {
    expression
        .split('#')
        .skip(1)
        .map(|part| {
            let digits = part
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(part.len());
            part[..digits].parse().unwrap()
        })
        .collect()
}

fn cone_pcurves(text: &str, parsed: &Document) -> Vec<ConePcurve> {
    let entities = entity_lines(text);
    let (cone, surface) = entities
        .iter()
        .find(|(_, expression)| expression.starts_with("CONICAL_SURFACE("))
        .unwrap();
    assert_eq!(surface.split(',').nth(2), Some("0."));
    let points = parsed.points2().unwrap();
    let directions = parsed.directions2().unwrap();
    let mut pcurves = Vec::new();
    for expression in entities.values() {
        if !expression.starts_with("PCURVE(") || references(expression)[0] != *cone {
            continue;
        }
        let line = references(entities[&references(expression)[1]])[0];
        let [point, vector] = references(entities[&line])[..] else {
            panic!("#{line} is not a pcurve LINE");
        };
        let direction = references(entities[&vector])[0];
        let ([u, v], [du, dv]) = (points[&point], directions[&direction]);
        pcurves.push(ConePcurve {
            point,
            point_line: format!("#{point}={}", entities[&point]),
            pair: [u, v, du, dv].map(f64::to_bits),
        });
    }
    pcurves
}

fn cone_uses(body: &Value, scale: f64) -> Vec<ConeUse> {
    let number = |value: &Value| value.as_f64().unwrap();
    let item = |key: &str, id: &Value| &body["topology"][key][id.as_u64().unwrap() as usize];
    let mut uses = Vec::new();
    for use_ in body["topology"]["halfedges"].as_array().unwrap() {
        let surface = &body["geometry"]["surfaces"]
            [item("faces", &use_["face"])["surface"].as_u64().unwrap() as usize];
        if surface["kind"] != "Cone" || item("edges", &use_["edge"])["geometry"]["kind"] != "Curve"
        {
            continue;
        }
        let metric = scale / number(&surface["semi_angle"]).cos();
        let geometry = &use_["geometry_use"];
        let pcurve = &body["geometry"]["pcurves"][geometry["pcurve"].as_u64().unwrap() as usize];
        let lift = [0, 1].map(|axis| number(&geometry["periodic_lift"][axis]));
        let [u, v] = [0, 1].map(|axis| number(&pcurve["origin"][axis]));
        let velocity = [
            number(&pcurve["direction"][0]) * 1.0,
            number(&pcurve["direction"][1]) * metric,
        ];
        let length = velocity.iter().fold(0.0_f64, |n, v| n.hypot(*v));
        let pair = [
            (u + lift[0] * TAU) * 1.0,
            (v + lift[1] * 0.0) * metric,
            velocity[0] / length,
            velocity[1] / length,
        ];
        uses.push(ConeUse {
            pair: pair.map(|v| if v == 0.0 { 0.0_f64 } else { v }.to_bits()),
            axial: (v + lift[1] * 0.0) * scale,
        });
    }
    uses
}

fn axial_rewrite(text: &str, pcurves: &[ConePcurve], uses: &[ConeUse]) -> String {
    let mut rewritten = text.to_string();
    for pcurve in pcurves {
        let use_ = uses.iter().find(|use_| use_.pair == pcurve.pair).unwrap();
        let line = format!(
            "#{}=CARTESIAN_POINT('',({:.17E},{:.17E}));",
            pcurve.point,
            f64::from_bits(use_.pair[0]),
            use_.axial
        );
        rewritten = rewritten.replace(&pcurve.point_line, &line);
    }
    rewritten
}

#[test]
fn cone_pcurve_v_measures_distance_along_the_generator() {
    let body: Value = serde_json::from_str(&fixture_text("cone.brep.json").unwrap()).unwrap();
    let brep = BrepEnvelope::from_json(&body.to_string()).unwrap();
    for (unit, _) in UNITS {
        let (text, report) = export_step(&brep, unit).unwrap();
        let report = serde_json::to_value(&report).unwrap();
        let parsed = parse_and_match(&text, &report, unit, from_ref(&brep));
        let pcurves = cone_pcurves(&text, &parsed);
        let uses = cone_uses(&body, unit_scale(unit));
        let mut actual = pcurves.iter().map(|pcurve| pcurve.pair).collect::<Vec<_>>();
        let mut expected = uses.iter().map(|use_| use_.pair).collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{unit}");
        let rewritten = axial_rewrite(&text, &pcurves, &uses);
        assert_ne!(rewritten, text, "{unit}");
        match Document::parse(&rewritten) {
            Ok(_) => panic!("{unit}: the axial-height pcurve parsed"),
            Err(error) => assert!(error.contains("misses 3D curve"), "{unit}: {error}"),
        }
    }
}

fn conic_uses(body: &Value) -> usize {
    let pcurve_kind = |use_: &Value| {
        let halfedge = &body["topology"]["halfedges"][use_.as_u64().unwrap() as usize];
        let pcurve = halfedge["geometry_use"]["pcurve"].as_u64().unwrap() as usize;
        body["geometry"]["pcurves"][pcurve]["kind"].clone()
    };
    let mut count = 0;
    for edge in body["topology"]["edges"].as_array().unwrap() {
        if edge["geometry"]["kind"] != "Curve" {
            continue;
        }
        let curve =
            &body["geometry"]["curves"][edge["geometry"]["curve"].as_u64().unwrap() as usize];
        let kinds = [&edge["halfedge"], &edge["twin_halfedge"]]
            .into_iter()
            .filter(|use_| !use_.is_null())
            .map(pcurve_kind)
            .collect::<Vec<_>>();
        if (curve["kind"] == "Circle" || curve["kind"] == "Ellipse")
            && kinds.iter().all(|kind| kind != "ProjectedCurve")
        {
            count += kinds.len();
        }
    }
    count
}

#[test]
fn conic_pcurves_are_assessed_for_parameter_direction() {
    for (name, body) in single_body_fixtures() {
        let json: Value = serde_json::from_str(&body.to_json().unwrap()).unwrap();
        for (unit, suffix) in UNITS {
            if fixture_text(&format!("{name}.step.{suffix}")).is_none() {
                continue;
            }
            let (text, _) = export_step(&body, unit).unwrap();
            let direction = Document::parse(&text)
                .and_then(|parsed| parsed.parameter_direction_report())
                .unwrap_or_else(|error| panic!("{name} {unit}: {error}"));
            assert_eq!(direction.conic_assessed, conic_uses(&json), "{name} {unit}");
            assert_eq!(
                direction.conic_assessed,
                direction.conic_aligned + direction.conic_reversed,
                "{name} {unit}"
            );
        }
    }
}
