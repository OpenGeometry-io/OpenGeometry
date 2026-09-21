pub mod interval;
pub mod predicates;
pub mod roots;
pub mod solve;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MathError {
    NonFinite,
    ArithmeticRange,
    InvalidInterval,
    DivisionByZero,
    SingularSystem,
    InvalidDimension,
    IterationLimit,
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MathError {}

pub(crate) fn finite(value: f64) -> Result<f64, MathError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(MathError::NonFinite)
    }
}
