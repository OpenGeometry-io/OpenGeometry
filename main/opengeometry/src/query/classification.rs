#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointClassification {
    Inside,
    Outside,
    Boundary,
    Unknown,
}

impl PointClassification {
    #[cfg(test)]
    pub(super) fn is_proven_outside(self) -> bool {
        self == Self::Outside
    }
}
