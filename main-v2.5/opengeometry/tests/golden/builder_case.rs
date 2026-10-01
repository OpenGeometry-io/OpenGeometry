use crate::body::record_body;
use crate::kernel::{BrepEnvelope, GeometryError};
use crate::runner::Case;

pub(crate) fn built(
    name: &str,
    build: impl Fn() -> Result<BrepEnvelope, GeometryError> + Send + Sync + 'static,
) -> Case {
    Case::new(format!("builders.{name}"), move |record| {
        match build() {
            Ok(shape) => record_body(record, "body", &shape),
            Err(error) => record.error("body", error),
        }
        Ok(())
    })
}

pub(crate) fn keyed(
    name: &str,
    build: impl Fn() -> Result<(BrepEnvelope, Vec<String>), GeometryError> + Send + Sync + 'static,
) -> Case {
    Case::new(format!("builders.{name}"), move |record| {
        match build() {
            Ok((shape, keys)) => {
                record.section("keys");
                for key in &keys {
                    record.line(key);
                }
                record_body(record, "body", &shape);
            }
            Err(error) => record.error("body", error),
        }
        Ok(())
    })
}
