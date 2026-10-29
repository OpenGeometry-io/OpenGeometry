use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum Orientation {
    Forward,
    Reverse,
}
impl Orientation {
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Forward => Self::Reverse,
            Self::Reverse => Self::Forward,
        }
    }
    pub(crate) fn multiplier(self) -> f64 {
        if self == Self::Forward {
            1.0
        } else {
            -1.0
        }
    }
}
