use crate::batch_route::batch_route_names;
use crate::handler_source::boolean_handler_names;
use crate::record::Failure;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const UNSCANNED_EXEMPTION: &str = "batch-route:other-";

pub(crate) fn expected_names() -> Result<BTreeSet<String>, Failure> {
    let mut expected = boolean_handler_names()?;
    expected.extend(batch_route_names().map(str::to_string));
    Ok(expected)
}

pub(crate) fn unscanned_names<'a>(
    expected: &BTreeSet<String>,
    seen: impl IntoIterator<Item = &'a String>,
) -> Vec<String> {
    seen.into_iter()
        .filter(|name| !expected.contains(*name) && !name.starts_with(UNSCANNED_EXEMPTION))
        .cloned()
        .collect()
}

fn listing<'a>(names: impl IntoIterator<Item = &'a String>) -> String {
    names.into_iter().map(|name| format!("{name}\n")).collect()
}

pub(crate) fn write_listings(
    output: &Path,
    expected: &BTreeSet<String>,
    covered: &BTreeSet<String>,
) -> Result<Vec<String>, Failure> {
    let uncovered = expected.difference(covered).cloned().collect::<Vec<_>>();
    fs::write(output.join("handlers-expected.txt"), listing(expected))?;
    fs::write(output.join("handlers-covered.txt"), listing(covered))?;
    fs::write(output.join("handlers-uncovered.txt"), listing(&uncovered))?;
    Ok(uncovered)
}
