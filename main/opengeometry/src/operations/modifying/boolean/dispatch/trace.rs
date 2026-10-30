use super::handler_id::HandlerId;
use crate::brep::GeometryError;
use crate::operations::modifying::boolean::types::BooleanResult;

#[derive(Default)]
pub(crate) struct Trace {
    pub(crate) handlers: Vec<String>,
    pub(crate) attempted: Vec<String>,
    pub(crate) families: Option<[String; 2]>,
}

impl Trace {
    pub(crate) fn add_attempts(&mut self, nested: &mut Trace) {
        self.attempted.append(&mut nested.attempted);
        self.families = self.families.take().or(nested.families.take());
    }
}

pub(crate) fn record_handler(
    trace: &mut Trace,
    name: HandlerId,
    call: impl FnOnce() -> Result<BooleanResult, GeometryError>,
) -> Result<BooleanResult, GeometryError> {
    let before = trace.handlers.len();
    trace.attempted.push(name.as_str().into());
    match call() {
        Ok(result) => {
            trace.handlers.insert(before, name.as_str().into());
            Ok(result)
        }
        Err(error) => {
            trace.handlers.truncate(before);
            Err(error)
        }
    }
}

pub(super) fn record_optional(
    trace: &mut Trace,
    name: HandlerId,
    call: impl FnOnce() -> Result<Option<BooleanResult>, GeometryError>,
) -> Result<Option<BooleanResult>, GeometryError> {
    let before = trace.handlers.len();
    trace.attempted.push(name.as_str().into());
    match call() {
        Ok(Some(result)) => {
            trace.handlers.insert(before, name.as_str().into());
            Ok(Some(result))
        }
        Ok(None) => {
            trace.handlers.truncate(before);
            Ok(None)
        }
        Err(error) => {
            trace.handlers.truncate(before);
            Err(error)
        }
    }
}
