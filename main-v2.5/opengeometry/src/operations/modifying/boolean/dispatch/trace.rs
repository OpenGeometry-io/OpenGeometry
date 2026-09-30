use super::handler_id::HandlerId;
use crate::brep::GeometryError;
use crate::operations::modifying::boolean::types::BooleanResult;

pub(super) fn record_handler(
    handlers: &mut Vec<String>,
    name: HandlerId,
    call: impl FnOnce() -> Result<BooleanResult, GeometryError>,
) -> Result<BooleanResult, GeometryError> {
    let before = handlers.len();
    match call() {
        Ok(result) => {
            handlers.insert(before, name.as_str().into());
            Ok(result)
        }
        Err(error) => {
            handlers.truncate(before);
            Err(error)
        }
    }
}

pub(super) fn record_optional(
    handlers: &mut Vec<String>,
    name: HandlerId,
    call: impl FnOnce() -> Result<Option<BooleanResult>, GeometryError>,
) -> Result<Option<BooleanResult>, GeometryError> {
    let before = handlers.len();
    match call() {
        Ok(Some(result)) => {
            handlers.insert(before, name.as_str().into());
            Ok(Some(result))
        }
        Ok(None) => {
            handlers.truncate(before);
            Ok(None)
        }
        Err(error) => {
            handlers.truncate(before);
            Err(error)
        }
    }
}
