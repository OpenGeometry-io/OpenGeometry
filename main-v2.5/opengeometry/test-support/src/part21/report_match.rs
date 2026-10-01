use super::document::Document;
use super::values::ProductCounts;
use serde_json::Value;
use std::collections::BTreeSet;

const BODY_KEYS: [&str; 6] = [
    "solids",
    "faces",
    "edges",
    "cavityShells",
    "fittedCurves",
    "pcurvelessEdges",
];

pub fn check_report_matches(document: &Document, text: &str, report: &Value) -> Result<(), String> {
    let products = document.product_counts()?;
    check_figure("products", figure(report, "products")?, products.len())?;
    let bodies = report["bodies"].as_array().ok_or("report has no bodies")?;
    check_figure("bodies", bodies.len(), products.len())?;
    for (index, (body, counts)) in bodies.iter().zip(&products).enumerate() {
        for (key, counted) in BODY_KEYS.into_iter().zip(body_figures(counts)) {
            check_figure(
                &format!("bodies[{index}].{key}"),
                figure(body, key)?,
                counted,
            )?;
        }
    }
    check_body_sums(report, bodies)?;
    check_totals(document, text, report)
}

pub fn check_single_report_matches(document: &Document, report: &Value) -> Result<(), String> {
    check_shape_figures(
        document,
        report,
        ["solids", "faces", "edges", "cavity_shells"],
    )?;
    let unit = report["length_unit"]
        .as_str()
        .ok_or("report has no length_unit")?;
    document.check_length_unit(unit)
}

fn body_figures(counts: &ProductCounts) -> [usize; 6] {
    [
        counts.solids,
        counts.faces,
        counts.edges,
        counts.voids,
        counts.fitted_curves,
        counts.pcurveless_edges,
    ]
}

fn check_body_sums(report: &Value, bodies: &[Value]) -> Result<(), String> {
    for key in BODY_KEYS {
        let sum = bodies
            .iter()
            .map(|body| figure(body, key))
            .sum::<Result<usize, String>>()?;
        let total = figure(report, key)?;
        if sum != total {
            return Err(format!("report {key} is {total}, the bodies sum to {sum}"));
        }
    }
    Ok(())
}

fn check_totals(document: &Document, text: &str, report: &Value) -> Result<(), String> {
    check_shape_figures(
        document,
        report,
        ["solids", "faces", "edges", "cavityShells"],
    )?;
    let fitted = document
        .edge_supports()?
        .into_iter()
        .filter(|support| support.kind == "B_SPLINE_CURVE_WITH_KNOTS")
        .map(|support| support.curve)
        .collect::<BTreeSet<_>>();
    for (key, counted) in [
        (
            "products",
            document.kind_counts().get("PRODUCT").copied().unwrap_or(0),
        ),
        ("fittedCurves", fitted.len()),
        ("pcurvelessEdges", document.pcurveless_edges()?),
        ("entities", document.entity_count()),
        ("bytes", text.len()),
    ] {
        check_figure(key, figure(report, key)?, counted)?;
    }
    let unit = report["unit"].as_str().ok_or("report has no unit")?;
    document.check_length_unit(unit)
}

fn check_shape_figures(document: &Document, report: &Value, keys: [&str; 4]) -> Result<(), String> {
    for (key, counted) in keys.into_iter().zip(shape_figures(document)?) {
        check_figure(key, figure(report, key)?, counted)?;
    }
    Ok(())
}

fn shape_figures(document: &Document) -> Result<[usize; 4], String> {
    let kinds = document.kind_counts();
    let kind = |name: &str| kinds.get(name).copied().unwrap_or(0);
    let voids = document.solids()?.iter().map(|solid| solid.voids).sum();
    check_figure("void map", voids, kind("ORIENTED_CLOSED_SHELL"))?;
    Ok([
        kind("MANIFOLD_SOLID_BREP") + kind("BREP_WITH_VOIDS"),
        kind("ADVANCED_FACE"),
        kind("EDGE_CURVE"),
        voids,
    ])
}

fn figure(report: &Value, key: &str) -> Result<usize, String> {
    report
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("report has no {key}"))
}

fn check_figure(key: &str, reported: usize, counted: usize) -> Result<(), String> {
    if reported != counted {
        return Err(format!(
            "report {key} is {reported}, the file has {counted}"
        ));
    }
    Ok(())
}
