use super::base::{base, Base};
use super::{assert_error, parsed};
use crate::part21::{check_report_matches, check_single_report_matches, normalise_step};
use serde_json::{json, Value};

fn matching_report(base: &Base) -> Value {
    json!({
        "unit": "metre",
        "products": 2,
        "solids": 2,
        "faces": 7,
        "edges": 5,
        "cavityShells": 1,
        "fittedCurves": 1,
        "pcurvelessEdges": 2,
        "entities": base.entities,
        "bytes": base.text.len(),
        "bodies": [
            {"solids": 1, "faces": 5, "edges": 4, "cavityShells": 1, "fittedCurves": 0, "pcurvelessEdges": 1},
            {"solids": 1, "faces": 2, "edges": 1, "cavityShells": 0, "fittedCurves": 1, "pcurvelessEdges": 1},
        ],
    })
}

#[test]
fn report_matcher_rejects_a_product_count_off_by_one() {
    let base = base();
    let document = parsed(&base.text);
    let mut report = matching_report(&base);
    assert_eq!(check_report_matches(&document, &base.text, &report), Ok(()));
    report["products"] = json!(3);
    let result = check_report_matches(&document, &base.text, &report);
    assert_error(result, "report products is 3, the file has 2");
}

#[test]
fn report_matcher_checks_each_body_against_its_product() {
    let base = base();
    let mut report = matching_report(&base);
    report["bodies"][1]["fittedCurves"] = json!(0);
    let result = check_report_matches(&parsed(&base.text), &base.text, &report);
    assert_error(result, "report bodies[1].fittedCurves is 0, the file has 1");
}

#[test]
fn report_matcher_checks_body_sums_against_totals() {
    let base = base();
    let mut report = matching_report(&base);
    report["faces"] = json!(8);
    let result = check_report_matches(&parsed(&base.text), &base.text, &report);
    assert_error(result, "report faces is 8, the bodies sum to 7");
}

#[test]
fn report_matcher_checks_bytes_and_unit_against_the_file() {
    let base = base();
    let document = parsed(&base.text);
    let mut report = matching_report(&base);
    report["bytes"] = json!(base.text.len() + 1);
    let result = check_report_matches(&document, &base.text, &report);
    assert_error(result, "report bytes is");
    let mut report = matching_report(&base);
    report["unit"] = json!("millimetre");
    let result = check_report_matches(&document, &base.text, &report);
    assert_error(result, "file length unit is metre, expected millimetre");
}

#[test]
fn single_report_matcher_checks_the_free_function_figures() {
    let document = parsed(&base().text);
    let mut report = json!({
        "length_unit": "metre",
        "faces": 7,
        "edges": 5,
        "solids": 2,
        "cavity_shells": 1,
    });
    assert_eq!(check_single_report_matches(&document, &report), Ok(()));
    report["faces"] = json!(6);
    let result = check_single_report_matches(&document, &report);
    assert_error(result, "report faces is 6, the file has 7");
}

#[test]
fn normalise_step_erases_names_and_negative_zero() {
    let source = [
        "ISO-10303-21;",
        "HEADER;",
        "FILE_DESCRIPTION(('OpenGeometry authoritative analytic BRep v2'),'3;1');",
        "FILE_NAME('cylinder','1970-01-01T00:00:00',('OpenGeometry'),('OpenGeometry'),'OpenGeometry','OpenGeometry','');",
        "DATA;",
        "#1=DIRECTION('',(-0.00000000000000000E0,-1.00000000000000000E0,-0.00000000000000000E0));",
        "#2=CARTESIAN_POINT('-0.0',(-5.0E-1,-1.0E-5,-0.0,1.0E-0));",
        "#3=MANIFOLD_SOLID_BREP('cylinder-0',#9);",
        "#4=BREP_WITH_VOIDS('box-cavity-0',#9,(#8));",
        "#5=PRODUCT('cylinder','cylinder','',(#7));",
        "#6=PRODUCT_CONTEXT('',#100,'mechanical');",
        "END-ISO-10303-21;",
        "",
    ];
    let expected = [
        "ISO-10303-21;",
        "HEADER;",
        "FILE_DESCRIPTION(('description'),'3;1');",
        "FILE_NAME('name','1970-01-01T00:00:00',('OpenGeometry'),('OpenGeometry'),'OpenGeometry','OpenGeometry','');",
        "DATA;",
        "#1=DIRECTION('',(0.00000000000000000E0,-1.00000000000000000E0,0.00000000000000000E0));",
        "#2=CARTESIAN_POINT('-0.0',(-5.0E-1,-1.0E-5,0.0,1.0E-0));",
        "#3=MANIFOLD_SOLID_BREP('solid',#9);",
        "#4=BREP_WITH_VOIDS('solid',#9,(#8));",
        "#5=PRODUCT('product','product','',(#7));",
        "#6=PRODUCT_CONTEXT('',#100,'mechanical');",
        "END-ISO-10303-21;",
        "",
    ];
    assert_eq!(normalise_step(&source.join("\n")), expected.join("\n"));
}
