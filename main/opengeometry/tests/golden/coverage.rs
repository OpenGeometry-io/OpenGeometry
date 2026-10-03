use crate::batch_route::batch_route_names;
use crate::handler_source::boolean_handler_names;
use crate::record::Failure;
use std::collections::BTreeSet;

const UNSCANNED_EXEMPTION: &str = "batch-route:other-";

pub(crate) struct Listings {
    pub(crate) files: [(&'static str, String); 3],
    pub(crate) uncovered: Vec<String>,
}

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

pub(crate) fn listings(expected: &BTreeSet<String>, covered: &BTreeSet<String>) -> Listings {
    let uncovered = expected.difference(covered).cloned().collect::<Vec<_>>();
    Listings {
        files: [
            ("handlers-expected", listing(expected)),
            ("handlers-covered", listing(covered)),
            ("handlers-uncovered", listing(&uncovered)),
        ],
        uncovered,
    }
}
