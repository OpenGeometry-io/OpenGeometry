use opengeometry::operations::CreatingOperation;

pub(super) fn extrude(profile: &str, distance: f64) -> CreatingOperation {
    CreatingOperation::Extrude {
        profile: profile.into(),
        holes: Vec::new(),
        distance,
    }
}
