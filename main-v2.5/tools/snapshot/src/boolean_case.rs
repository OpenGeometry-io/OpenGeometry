use crate::body::record_body;
use crate::json::{brep_sha256, pretty};
use crate::kernel::{
    boolean_brep_outcome_with_handlers, BooleanOp, BooleanResult, BrepEnvelope, GeometryError,
};
use crate::record::{note_staged_handlers, Failure, Record};
use crate::runner::Case;
use serde_json::to_value;
use std::sync::Arc;

pub(crate) type Operands =
    Arc<dyn Fn() -> Result<(BrepEnvelope, BrepEnvelope), Failure> + Send + Sync>;

pub(crate) type Built = Result<(BrepEnvelope, BrepEnvelope), Failure>;

pub(crate) type Pair = fn() -> Built;

pub(crate) type Entry = (&'static str, &'static str, BooleanOp, Pair);

pub(crate) type Family = (&'static str, &'static str, Pair);

pub(crate) const OPERATIONS: [(&str, BooleanOp); 3] = [
    ("union", BooleanOp::Union),
    ("intersection", BooleanOp::Intersection),
    ("subtraction", BooleanOp::Subtraction),
];

pub(crate) fn operands(
    build: impl Fn() -> Result<(BrepEnvelope, BrepEnvelope), Failure> + Send + Sync + 'static,
) -> Operands {
    Arc::new(build)
}

pub(crate) fn boolean(name: &str, id: &str, operation: BooleanOp, operands: &Operands) -> Case {
    let operands = Arc::clone(operands);
    let id = id.to_string();
    Case::new(name, move |record| {
        let (a, b) = operands()?;
        let (result, _) = record_boolean_handlers(record, &a, &b, operation, &id);
        result_body(record, result);
        Ok(())
    })
}

pub(crate) fn every_operation(prefix: &str, id: &str, operands: &Operands) -> Vec<Case> {
    OPERATIONS
        .iter()
        .map(|(label, operation)| {
            boolean(
                &format!("{prefix}.{label}"),
                &format!("{id}-{label}"),
                *operation,
                operands,
            )
        })
        .collect()
}

pub(crate) fn swapped(operands: &Operands) -> Operands {
    let operands = Arc::clone(operands);
    Arc::new(move || {
        let (a, b) = operands()?;
        Ok((b, a))
    })
}

pub(crate) fn record_operands(record: &mut Record, a: &BrepEnvelope, b: &BrepEnvelope) {
    record.section("operands");
    record.field("a", &a.id);
    record.field("a.sha256", brep_sha256(a));
    record.field("b", &b.id);
    record.field("b.sha256", brep_sha256(b));
}

pub(crate) fn record_boolean_handlers(
    record: &mut Record,
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: &str,
) -> (Result<BooleanResult, GeometryError>, Vec<String>) {
    record_operands(record, a, b);
    record.section("boolean");
    record.debug("operation", operation);
    record.field("id", id);
    let (result, handlers) = boolean_brep_outcome_with_handlers(a, b, operation, id.to_string());
    record.debug("handlers", &handlers);
    record.cover_all(&handlers);
    (result, handlers)
}

pub(crate) fn result_body(record: &mut Record, result: Result<BooleanResult, GeometryError>) {
    match result {
        Ok(result) => {
            record.block("report", &pretty(to_value(&result.report)));
            record_body(record, "result", &result.brep);
        }
        Err(error) => record.error("result", error),
    }
}

pub(crate) fn stage(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: &str,
) -> Result<BrepEnvelope, Failure> {
    let (result, handlers) = boolean_brep_outcome_with_handlers(a, b, operation, id.to_string());
    note_staged_handlers(handlers);
    Ok(result?.brep)
}

pub(crate) fn table(prefix: &str, entries: &[Entry]) -> Vec<Case> {
    entries
        .iter()
        .map(|(name, id, operation, pair)| {
            boolean(
                &format!("{prefix}.{name}"),
                id,
                *operation,
                &operands(*pair),
            )
        })
        .collect()
}

pub(crate) fn families(prefix: &str, entries: &[Family]) -> Vec<Case> {
    entries
        .iter()
        .flat_map(|(name, id, pair)| {
            every_operation(&format!("{prefix}.{name}"), id, &operands(*pair))
        })
        .collect()
}
