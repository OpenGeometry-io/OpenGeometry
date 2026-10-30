mod handler_id;
mod table;
mod trace;

pub(super) use handler_id::HandlerId;
pub(super) use trace::{record_handler, Trace};

use super::handlers::{coincident_boolean, disjoint_boolean};
use super::types::{BooleanOp, BooleanResult};
use crate::brep::{BrepEnvelope, GeometryError};
use table::{generic_boolean, specialized_boolean};
use trace::record_optional;

pub fn boolean_brep(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    boolean_brep_with_handlers(a, b, operation, id).map(|result| result.0)
}

pub fn boolean_brep_with_handlers(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<(BooleanResult, Vec<String>), GeometryError> {
    let (result, trace) = boolean_brep_outcome_with_trace(a, b, operation, id);
    result.map(|result| (result, trace.handlers))
}

pub fn boolean_brep_outcome_with_handlers(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> (Result<BooleanResult, GeometryError>, Vec<String>) {
    let (result, trace) = boolean_brep_outcome_with_trace(a, b, operation, id);
    (result, trace.handlers)
}

pub(crate) fn boolean_brep_outcome_with_trace(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> (Result<BooleanResult, GeometryError>, Trace) {
    let mut trace = Trace::default();
    let result = boolean_brep_inner(a, b, operation, id, &mut trace);
    (result, trace)
}

fn boolean_brep_inner(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    trace: &mut Trace,
) -> Result<BooleanResult, GeometryError> {
    a.validate()?;
    b.validate()?;
    if a.geometry == b.geometry && a.topology == b.topology && a.solids == b.solids {
        return record_handler(trace, HandlerId::CoincidentBoolean, || {
            coincident_boolean(a, b, operation, id)
        });
    }
    if let Some(result) = record_optional(trace, HandlerId::SeparateBoolean, || {
        disjoint_boolean(a, b, operation, id.clone())
    })? {
        return Ok(result);
    }
    let specialized = specialized_boolean(a, b, operation, id.clone(), trace);
    match specialized {
        Err(GeometryError::CoverageGap { families }) => {
            trace.families = Some(families);
            generic_boolean(a, b, operation, id, trace)
        }
        result => result,
    }
}
