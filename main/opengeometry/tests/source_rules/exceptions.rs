use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const SOURCE_ROOT: &str = "src";
const OWNERS: [&str; 11] = [
    "A8b", "A8c", "A9a", "A9b", "A9c", "A9d", "A9e", "A9f", "A9g", "A9h", "B5",
];

type FrozenSet = BTreeMap<Vec<String>, FrozenKey>;

struct FrozenKey {
    max: Option<usize>,
    owner: String,
    well_formed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Violation {
    pub(crate) location: String,
    pub(crate) key: Vec<String>,
    pub(crate) measure: Option<usize>,
    pub(crate) detail: String,
}

impl Violation {
    pub(crate) fn new(
        file: &str,
        line: usize,
        key: &[&str],
        detail: impl Into<String>,
    ) -> Violation {
        Violation {
            location: format!("{SOURCE_ROOT}/{file}:{line}"),
            key: key.iter().map(|part| (*part).to_string()).collect(),
            measure: None,
            detail: detail.into(),
        }
    }

    pub(crate) fn with_measure(mut self, measure: usize) -> Violation {
        self.measure = Some(measure);
        self
    }

    pub(crate) fn under(mut self, root: &str) -> Violation {
        self.location = self.location.replacen(SOURCE_ROOT, root, 1);
        self
    }
}

pub(crate) struct ListSpec {
    pub(crate) file: &'static str,
    pub(crate) key_fields: usize,
    pub(crate) measured: bool,
    pub(crate) frozen: &'static [&'static str],
}

struct Entry {
    key: Vec<String>,
    measure: Option<usize>,
    frozen_max: Option<usize>,
    line: usize,
}

pub(crate) struct ExceptionList {
    name: String,
    entries: Vec<Entry>,
    problems: Vec<(usize, String)>,
}

fn field_count(spec: &ListSpec) -> usize {
    spec.key_fields + usize::from(spec.measured) + 1
}

fn frozen_set(spec: &ListSpec) -> FrozenSet {
    spec.frozen
        .iter()
        .map(|text| {
            let fields: Vec<&str> = text.split_whitespace().collect();
            let key = fields.iter().take(spec.key_fields);
            let max = fields
                .get(spec.key_fields)
                .filter(|_| spec.measured)
                .and_then(|field| field.parse().ok());
            let frozen = FrozenKey {
                max,
                owner: fields
                    .last()
                    .map_or_else(String::new, |owner| (*owner).to_string()),
                well_formed: fields.len() == field_count(spec),
            };
            (key.map(|field| (*field).to_string()).collect(), frozen)
        })
        .collect()
}

impl ExceptionList {
    pub(crate) fn load(path: &Path, spec: &ListSpec) -> ExceptionList {
        match fs::read_to_string(path) {
            Ok(text) => ExceptionList::parse(&text, spec),
            Err(error) => ExceptionList {
                name: spec.file.to_string(),
                entries: Vec::new(),
                problems: vec![(0, format!("cannot read the list: {error}"))],
            },
        }
    }

    fn parse(text: &str, spec: &ListSpec) -> ExceptionList {
        let mut list = ExceptionList {
            name: spec.file.to_string(),
            entries: Vec::new(),
            problems: Vec::new(),
        };
        let frozen = frozen_set(spec);
        for (index, line) in text.lines().enumerate() {
            list.parse_entry(index + 1, line, spec, &frozen);
        }
        list.check_retired(&frozen);
        list
    }

    fn check_retired(&mut self, frozen: &FrozenSet) {
        for (key, frozen_key) in frozen {
            let key_text = key.join(" ");
            let problem = match self.entry(key).map(|(_, entry)| entry) {
                None => (
                    0,
                    format!("frozen key `{key_text}` has no entry; retire it in frozen_lists.rs"),
                ),
                Some(entry) => match frozen_key.max.zip(entry.measure) {
                    Some((max, listed)) if max > listed => (
                        entry.line,
                        format!(
                            "frozen maximum of `{key_text}` is {max}, above the listed {listed}; lower it to {listed}"
                        ),
                    ),
                    _ => continue,
                },
            };
            self.problems.push(problem);
        }
    }

    fn frozen_max(
        &mut self,
        line: usize,
        key: &[String],
        owner: &str,
        spec: &ListSpec,
        frozen: &FrozenSet,
    ) -> Option<usize> {
        let key_text = key.join(" ");
        let Some(frozen_key) = frozen.get(key) else {
            self.problems.push((
                line,
                format!("entry `{key_text}` is not in the frozen set; the list may only shrink"),
            ));
            return None;
        };
        if !frozen_key.well_formed || (spec.measured && frozen_key.max.is_none()) {
            self.problems.push((
                line,
                format!("frozen key `{key_text}` must give its key, maximum and owner"),
            ));
        } else if frozen_key.owner != owner {
            self.problems.push((
                line,
                format!(
                    "entry `{key_text}` belongs to {} in the frozen set, not {owner}",
                    frozen_key.owner
                ),
            ));
        }
        frozen_key.max
    }

    fn parse_entry(&mut self, line: usize, text: &str, spec: &ListSpec, frozen: &FrozenSet) {
        let fields: Vec<&str> = text.split_whitespace().collect();
        let expected = field_count(spec);
        if fields.len() != expected {
            self.problems
                .push((line, format!("expected {expected} fields, found `{text}`")));
            return;
        }
        let owner = fields[expected - 1];
        if !OWNERS.contains(&owner) {
            let runs = OWNERS.join(", ");
            self.problems.push((
                line,
                format!("`{owner}` is not a run the plan defines ({runs})"),
            ));
        }
        let key: Vec<String> = fields[..spec.key_fields]
            .iter()
            .map(|field| (*field).to_string())
            .collect();
        if self.entries.iter().any(|entry| entry.key == key) {
            self.problems
                .push((line, format!("duplicate entry `{}`", key.join(" "))));
        }
        let measure = if spec.measured {
            match fields[spec.key_fields].parse::<usize>() {
                Ok(measure) => Some(measure),
                Err(_) => {
                    self.problems.push((
                        line,
                        format!("`{}` is not a line count", fields[spec.key_fields]),
                    ));
                    None
                }
            }
        } else {
            None
        };
        let frozen_max = self.frozen_max(line, &key, owner, spec, frozen);
        if let Some((listed, max)) = measure.zip(frozen_max).filter(|(listed, max)| listed > max) {
            self.problems.push((
                line,
                format!("lists {listed} lines, above its frozen maximum of {max}"),
            ));
        }
        self.entries.push(Entry {
            key,
            measure,
            frozen_max,
            line,
        });
    }

    fn entry(&self, key: &[String]) -> Option<(usize, &Entry)> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.key == key)
    }

    fn location(&self, line: usize) -> String {
        format!("tests/source_rules/{}:{line}", self.name)
    }
}

fn measure_failure(rule: &str, violation: &Violation, entry: &Entry, list: &str) -> Option<String> {
    let (now, listed) = (violation.measure?, entry.measure?);
    let change = if let Some(max) = entry.frozen_max.filter(|max| now > *max) {
        format!("grew past its frozen maximum of {max} lines")
    } else if now > listed {
        format!("grew from the {listed} lines listed in {list}")
    } else if now < listed {
        format!("is shorter than the {listed} lines listed in {list}; set the entry to {now}")
    } else {
        return None;
    };
    Some(format!(
        "{}: {rule}: {} {change}",
        violation.location, violation.detail
    ))
}

fn list_failures(
    rule: &str,
    list: &ExceptionList,
    matches: &BTreeMap<usize, Vec<&str>>,
) -> Vec<String> {
    let mut failures = Vec::new();
    for (line, problem) in &list.problems {
        failures.push(format!("{}: {rule}: {problem}", list.location(*line)));
    }
    for (index, entry) in list.entries.iter().enumerate() {
        let key = entry.key.join(" ");
        let location = list.location(entry.line);
        match matches.get(&index) {
            None => failures.push(format!(
                "{location}: {rule}: stale entry `{key}` no longer violates the rule; remove it"
            )),
            Some(found) if found.len() > 1 => failures.push(format!(
                "{location}: {rule}: entry `{key}` matches {} violations ({}); one entry covers exactly one violation",
                found.len(),
                found.join(", ")
            )),
            Some(_) => {}
        }
    }
    failures
}

fn failures(rule: &str, violations: &[Violation], list: Option<&ExceptionList>) -> Vec<String> {
    let mut failures = Vec::new();
    let mut matches: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for violation in violations {
        let Some(list) = list else {
            failures.push(format!(
                "{}: {rule}: {}",
                violation.location, violation.detail
            ));
            continue;
        };
        let Some((index, entry)) = list.entry(&violation.key) else {
            failures.push(format!(
                "{}: {rule}: {} (not in {})",
                violation.location, violation.detail, list.name
            ));
            continue;
        };
        matches
            .entry(index)
            .or_default()
            .push(violation.location.as_str());
        failures.extend(measure_failure(rule, violation, entry, &list.name));
    }
    if let Some(list) = list {
        failures.extend(list_failures(rule, list, &matches));
    }
    failures
}

pub(crate) fn enforce(rule: &str, violations: &[Violation], list: Option<&ExceptionList>) {
    let failures = failures(rule, violations, list);
    assert!(
        failures.is_empty(),
        "rule `{rule}` failed {} time(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

const RATCHET: ListSpec = ListSpec {
    file: "ratchet.txt",
    key_fields: 2,
    measured: true,
    frozen: &["math/solve.rs solve 90 A9f"],
};

fn problems_of(text: &str, spec: &ListSpec) -> Vec<String> {
    ExceptionList::parse(text, spec)
        .problems
        .into_iter()
        .map(|(_, problem)| problem)
        .collect()
}

#[test]
fn frozen_list_rejects_a_deleted_entry_that_comes_back() {
    assert_eq!(
        problems_of("", &RATCHET),
        ["frozen key `math/solve.rs solve` has no entry; retire it in frozen_lists.rs"]
    );
    let retired = ListSpec {
        frozen: &[],
        ..RATCHET
    };
    assert!(problems_of("", &retired).is_empty());
    assert_eq!(
        problems_of("math/solve.rs solve 90 A9f\n", &retired),
        ["entry `math/solve.rs solve` is not in the frozen set; the list may only shrink"]
    );
}

#[test]
fn frozen_maximum_never_rises_above_the_listed_length() {
    assert!(problems_of("math/solve.rs solve 90 A9f\n", &RATCHET).is_empty());
    assert_eq!(
        problems_of("math/solve.rs solve 95 A9f\n", &RATCHET),
        ["lists 95 lines, above its frozen maximum of 90"]
    );
    assert_eq!(
        problems_of("math/solve.rs solve 85 A9f\n", &RATCHET),
        ["frozen maximum of `math/solve.rs solve` is 90, above the listed 85; lower it to 85"]
    );
}

#[test]
fn frozen_entry_stays_with_its_owner() {
    assert_eq!(
        problems_of("math/solve.rs solve 90 B5\n", &RATCHET),
        ["entry `math/solve.rs solve` belongs to A9f in the frozen set, not B5"]
    );
    let ownerless = ListSpec {
        frozen: &["math/solve.rs solve 90"],
        ..RATCHET
    };
    assert_eq!(
        problems_of("math/solve.rs solve 90 A9f\n", &ownerless),
        ["frozen key `math/solve.rs solve` must give its key, maximum and owner"]
    );
}
