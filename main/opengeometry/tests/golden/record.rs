use crate::kernel::GraphError;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::{Debug, Display};

pub(crate) type Failure = Box<dyn std::error::Error + Send + Sync>;

thread_local! {
    static STAGED_HANDLERS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn note_staged_handlers(handlers: Vec<String>) {
    STAGED_HANDLERS.with_borrow_mut(|staged| staged.extend(handlers));
}

#[derive(Default)]
pub(crate) struct Findings {
    pub(crate) covered: BTreeSet<String>,
    pub(crate) disagreements: BTreeSet<String>,
    pub(crate) broken_cases: BTreeSet<String>,
    pub(crate) staged_handlers: BTreeSet<String>,
}

impl Findings {
    pub(crate) fn absorb(&mut self, other: Findings) {
        self.covered.extend(other.covered);
        self.disagreements.extend(other.disagreements);
        self.broken_cases.extend(other.broken_cases);
        self.staged_handlers.extend(other.staged_handlers);
    }
}

pub(crate) struct Record {
    name: String,
    text: String,
    verdicts: BTreeSet<String>,
    findings: Findings,
}

impl Record {
    pub(crate) fn new(name: &str) -> Self {
        let mut record = Self {
            name: name.to_string(),
            text: String::new(),
            verdicts: BTreeSet::new(),
            findings: Findings::default(),
        };
        record.section("case");
        record.line(name);
        record
    }

    pub(crate) fn section(&mut self, title: &str) {
        self.text.push_str("### ");
        self.text.push_str(title);
        self.text.push('\n');
    }

    pub(crate) fn line(&mut self, text: &str) {
        self.text.push_str(text);
        self.text.push('\n');
    }

    pub(crate) fn field(&mut self, key: &str, value: impl Display) {
        self.line(&format!("{key}: {value}"));
    }

    pub(crate) fn debug(&mut self, key: &str, value: impl Debug) {
        self.line(&format!("{key}: {value:?}"));
    }

    pub(crate) fn block(&mut self, title: &str, body: &str) {
        self.section(title);
        self.text.push_str(body);
        if !body.ends_with('\n') {
            self.text.push('\n');
        }
    }

    pub(crate) fn error(&mut self, title: &str, error: impl Debug) {
        self.section(title);
        self.line(&format!("error: {error:?}"));
    }

    pub(crate) fn graph_error(&mut self, title: &str, error: &GraphError) {
        self.section(title);
        self.graph_error_field("error", error);
    }

    pub(crate) fn graph_error_field(&mut self, key: &str, error: &GraphError) {
        self.line(&format!("{key}: {}", graph_error_json(error)));
    }

    pub(crate) fn graph_result<T: Debug>(&mut self, key: &str, result: &Result<T, GraphError>) {
        match result {
            Ok(value) => self.line(&format!("{key}: Ok({value:?})")),
            Err(error) => self.line(&format!("{key}: Err({})", graph_error_json(error))),
        }
    }

    pub(crate) fn cover(&mut self, name: &str) {
        self.findings.covered.insert(name.to_string());
    }

    pub(crate) fn cover_all<'a>(&mut self, names: impl IntoIterator<Item = &'a String>) {
        for name in names {
            self.cover(name);
        }
    }

    pub(crate) fn disagree(&mut self, subject: &str) {
        let disagreement = format!("{} {subject}", self.name);
        self.findings.disagreements.insert(disagreement);
    }

    pub(crate) fn note_verdict(&mut self, subject: &str) {
        self.verdicts.insert(subject.to_string());
    }

    pub(crate) fn has_verdict(&self, subject: &str) -> bool {
        self.verdicts.contains(subject)
    }

    pub(crate) fn break_case(&mut self, problem: &str) {
        let broken = format!("{} {problem}", self.name);
        self.findings.broken_cases.insert(broken);
    }

    pub(crate) fn finish(mut self) -> (String, Findings) {
        let staged = STAGED_HANDLERS.with_borrow_mut(std::mem::take);
        self.findings.staged_handlers.extend(staged);
        (self.text, self.findings)
    }
}

fn graph_error_json(error: &GraphError) -> String {
    let details = match serde_json::to_value(error.details()) {
        Ok(Value::Null) => json!({}),
        Ok(details) => details,
        Err(failure) => json!(format!("serialization error: {failure:?}")),
    };
    json!({"code": error.error_code(), "details": details, "message": error.message()}).to_string()
}
