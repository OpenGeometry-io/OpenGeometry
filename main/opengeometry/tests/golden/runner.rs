use crate::record::{Failure, Findings, Record};
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

type CaseBody = Box<dyn Fn(&mut Record) -> Result<(), Failure> + Send + Sync>;

const SLOWEST_CASES: [&str; 12] = [
    "batch.planar.fifty-openings",
    "booleans.cylinders.nonparallel-through-cut",
    "booleans.matrix.cylinder-cross",
    "booleans.generic.periodic-winding",
    "batch.staged.overlapping-planar-cutters",
    "batch.staged.mixed-curved-openings.reverse",
    "batch.staged.mixed-curved-openings.forward",
    "booleans.sphere-cylinder.crossing.subtraction",
    "booleans.sphere-cylinder.crossing.union",
    "booleans.sphere-cylinder.crossing.intersection",
    "booleans.sphere-cylinder.crossing.reversed-subtraction",
    "booleans.generic.sphere-cone.subtraction",
];

#[derive(Default)]
pub(crate) struct Run {
    pub(crate) records: BTreeMap<String, String>,
    pub(crate) findings: Findings,
}

impl Run {
    fn absorb(&mut self, other: Run) {
        self.records.extend(other.records);
        self.findings.absorb(other.findings);
    }
}

pub(crate) struct Case {
    name: String,
    body: CaseBody,
    required_verdicts: &'static [&'static str],
}

impl Case {
    pub(crate) fn new(
        name: impl Into<String>,
        body: impl Fn(&mut Record) -> Result<(), Failure> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            body: Box::new(body),
            required_verdicts: &[],
        }
    }

    pub(crate) fn requiring_verdicts(self, verdicts: &'static [&'static str]) -> Self {
        Self {
            required_verdicts: verdicts,
            ..self
        }
    }

    fn check_verdicts(&self, record: &mut Record, problem: Option<&str>) {
        if self.required_verdicts.is_empty() {
            return;
        }
        if let Some(problem) = problem {
            record.break_case(problem);
        }
        for subject in self.required_verdicts {
            if !record.has_verdict(subject) {
                record.break_case(&format!("lacks the {subject} verdict"));
            }
        }
    }

    fn execute(&self) -> (String, Findings) {
        let mut record = Record::new(&self.name);
        let outcome = catch_unwind(AssertUnwindSafe(|| (self.body)(&mut record)));
        let problem = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(failure)) => {
                record.error("case-failure", failure);
                Some("returned an error")
            }
            Err(payload) => {
                record.error("case-panic", panic_message(payload.as_ref()));
                Some("panicked")
            }
        };
        self.check_verdicts(&mut record, problem);
        record.finish()
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "non-text panic payload".to_string()
    }
}

fn check_names(cases: &[Case]) -> Result<(), Failure> {
    let mut seen = BTreeSet::new();
    for case in cases {
        let valid = !case.name.is_empty()
            && case.name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte)
            });
        if !valid {
            return Err(format!(
                "case name {:?} is not lowercase, digits, dots and dashes",
                case.name
            )
            .into());
        }
        if !seen.insert(case.name.as_str()) {
            return Err(format!("duplicate case name {}", case.name).into());
        }
    }
    Ok(())
}

fn slowest_first(cases: &[Case]) -> Result<Vec<&Case>, Failure> {
    let missing = SLOWEST_CASES
        .iter()
        .find(|slow| !cases.iter().any(|case| case.name == **slow));
    if let Some(missing) = missing {
        return Err(
            format!("the slowest-first schedule names {missing}, which is not a case").into(),
        );
    }
    let mut order = cases.iter().collect::<Vec<_>>();
    order.sort_by_key(|case| {
        SLOWEST_CASES
            .iter()
            .position(|slow| *slow == case.name)
            .unwrap_or(SLOWEST_CASES.len())
    });
    Ok(order)
}

fn work(cases: &[&Case], next: &AtomicUsize) -> Run {
    let mut run = Run::default();
    loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        let Some(case) = cases.get(index) else {
            return run;
        };
        let (text, case_findings) = case.execute();
        run.records.insert(case.name.clone(), text);
        run.findings.absorb(case_findings);
    }
}

pub(crate) fn run(cases: &[Case]) -> Result<Run, Failure> {
    check_names(cases)?;
    let order = slowest_first(cases)?;
    let next = AtomicUsize::new(0);
    let workers = thread::available_parallelism().map_or(1, |count| count.get());
    let results = thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| scope.spawn(|| work(&order, &next)))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join())
            .collect::<Vec<_>>()
    });
    let mut run = Run::default();
    for result in results {
        run.absorb(result.map_err(|payload| panic_message(payload.as_ref()))?);
    }
    Ok(run)
}
