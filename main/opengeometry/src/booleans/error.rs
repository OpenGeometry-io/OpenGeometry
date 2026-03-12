use crate::brep::BrepError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanErrorKind {
    InvalidOperand,
    MixedOperandKinds,
    UnsupportedOperandKind,
    NonCoplanarPlanarOperands,
    TopologyError,
    KernelFailure,
}

#[derive(Debug, Clone)]
pub struct BooleanError {
    kind: BooleanErrorKind,
    message: String,
}

impl BooleanError {
    pub fn new(kind: BooleanErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> BooleanErrorKind {
        self.kind
    }
}

impl core::fmt::Display for BooleanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BooleanError {}

impl From<BrepError> for BooleanError {
    fn from(error: BrepError) -> Self {
        BooleanError::new(BooleanErrorKind::TopologyError, error.to_string())
    }
}
