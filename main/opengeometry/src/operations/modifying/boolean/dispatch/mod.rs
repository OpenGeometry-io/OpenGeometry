mod handler_id;
mod table;
mod trace;

pub(super) use handler_id::HandlerId;
pub(super) use trace::record_handler;

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
    let (result, handlers) = boolean_brep_outcome_with_handlers(a, b, operation, id);
    result.map(|result| (result, handlers))
}

pub fn boolean_brep_outcome_with_handlers(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> (Result<BooleanResult, GeometryError>, Vec<String>) {
    let mut handlers = Vec::new();
    let result = boolean_brep_inner(a, b, operation, id, &mut handlers);
    (result, handlers)
}

fn boolean_brep_inner(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    a.validate()?;
    b.validate()?;
    if a.geometry == b.geometry && a.topology == b.topology && a.solids == b.solids {
        return record_handler(handlers, HandlerId::CoincidentBoolean, || {
            coincident_boolean(a, b, operation, id)
        });
    }
    if let Some(result) = record_optional(handlers, HandlerId::SeparateBoolean, || {
        disjoint_boolean(a, b, operation, id.clone())
    })? {
        return Ok(result);
    }
    let specialized = specialized_boolean(a, b, operation, id.clone(), handlers);
    match specialized {
        Err(GeometryError::CoverageGap { .. }) => generic_boolean(a, b, operation, id, handlers),
        result => result,
    }
}
