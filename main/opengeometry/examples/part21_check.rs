use opengeometry_test_support::part21::{bits, check_report_matches, Document};
use opengeometry_test_support::scenes::acceptance::{acceptance_scene, level_export};
use serde_json::Value;
use std::{env, error::Error, fs};

const USAGE: &str = "usage: part21_check acceptance <step> <report.json> <unit> <upAxis> | storey <step> <report.json>";

struct BrowserExport {
    text: String,
    report: Value,
    document: Document,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["acceptance", step, report, unit, up_axis] => {
            check_acceptance(step, report, unit, up_axis)?
        }
        ["storey", step, report] => check_storey(step, report)?,
        _ => return Err(USAGE.into()),
    }
    println!("part21_check {}: ok", args[0]);
    Ok(())
}

fn browser_export(step: &str, report: &str, unit: &str) -> Result<BrowserExport, Box<dyn Error>> {
    let text = fs::read_to_string(step)?;
    let report: Value = serde_json::from_str(&fs::read_to_string(report)?)?;
    let document = Document::parse(&text)?;
    document.check_length_unit(unit)?;
    check_report_matches(&document, &text, &report)?;
    Ok(BrowserExport {
        text,
        report,
        document,
    })
}

fn check_acceptance(
    step: &str,
    report: &str,
    unit: &str,
    up_axis: &str,
) -> Result<(), Box<dyn Error>> {
    let browser = browser_export(step, report, unit)?;
    let (native_text, native_report) = level_export(&acceptance_scene(), unit, up_axis);
    let native_report = serde_json::to_value(&native_report)?;
    check_same_value("report", &browser.report, &native_report)?;
    check_same_text(&browser.text, &native_text)?;
    Ok(())
}

fn check_same_value(path: &str, browser: &Value, native: &Value) -> Result<(), String> {
    match (browser, native) {
        (Value::Object(left), Value::Object(right)) => {
            if !left.keys().eq(right.keys()) {
                return Err(format!("{path}: keys differ from the native report"));
            }
            left.iter().try_for_each(|(key, value)| {
                check_same_value(&format!("{path}.{key}"), value, &right[key])
            })
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Err(format!(
                    "{path}: {} items, native {}",
                    left.len(),
                    right.len()
                ));
            }
            left.iter()
                .zip(right)
                .enumerate()
                .try_for_each(|(index, (a, b))| check_same_value(&format!("{path}[{index}]"), a, b))
        }
        (Value::Number(left), Value::Number(right))
            if left
                .as_f64()
                .zip(right.as_f64())
                .is_some_and(|(a, b)| bits([a]) == bits([b])) =>
        {
            Ok(())
        }
        _ if browser == native => Ok(()),
        _ => Err(format!(
            "{path}: browser {browser} differs from native {native}"
        )),
    }
}

fn check_same_text(browser: &str, native: &str) -> Result<(), String> {
    if browser == native {
        return Ok(());
    }
    let (line, left, right) = browser
        .lines()
        .zip(native.lines())
        .enumerate()
        .find(|(_, (left, right))| left != right)
        .map_or((0, "", ""), |(index, (left, right))| {
            (index + 1, left, right)
        });
    Err(format!(
        "text differs from the native export at line {line}: browser {left:?}, native {right:?} ({} and {} bytes)",
        browser.len(),
        native.len()
    ))
}

fn check_storey(step: &str, report: &str) -> Result<(), Box<dyn Error>> {
    let browser = browser_export(step, report, "millimetre")?;
    let products = browser.document.product_names()?.len();
    let entities = browser.document.entity_count();
    let bytes = browser.text.len();
    if products != 1200 || entities > 2_000_000 || bytes > 64 * 1024 * 1024 {
        return Err(format!(
            "storey export has {products} products (want 1200), {entities} entities (at most 2000000) and {bytes} bytes (at most 64 MiB)"
        )
        .into());
    }
    Ok(())
}
