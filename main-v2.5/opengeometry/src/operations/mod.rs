pub(super) mod creating;
mod error;
pub mod modifying;

pub use error::OperationError;

pub(super) fn invalid(message: impl Into<String>) -> OperationError {
    OperationError::InvalidParameter(message.into())
}
