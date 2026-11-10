use crate::brep::GeometryError;
use crate::primitives::profile_wire::ProfileSegment;
use serde::{Deserialize, Serialize};

pub(super) const MITER_LIMIT: f64 = 4.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolysolidPath {
    pub start: [f64; 2],
    pub segments: Vec<ProfileSegment>,
    pub closed: bool,
}

#[derive(Debug)]
pub enum PolysolidError {
    Parameter(String),
    Geometry(GeometryError),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolysolidJustify {
    #[default]
    Centre,
    Left,
    Right,
}

pub(super) fn check_path_ends(
    path: &PolysolidPath,
    fewest: usize,
    too_few: &str,
    g: f64,
) -> Result<(), PolysolidError> {
    let centres = path.segments.iter().filter_map(|segment| segment.arc);
    let values = path
        .segments
        .iter()
        .flat_map(|segment| segment.to)
        .chain(centres.flat_map(|arc| arc.center));
    if !path.start.into_iter().chain(values).all(f64::is_finite) {
        return Err(parameter("polysolid path points must be finite".into()));
    }
    if path.segments.len() < fewest {
        return Err(parameter(too_few.into()));
    }
    let last = path.segments[path.segments.len() - 1].to;
    if path.closed != (distance(last, path.start) <= 4.0 * g) {
        return Err(parameter(if path.closed {
            "closed polysolid path must end at its start".into()
        } else {
            "open polysolid path must not end at its start".into()
        }));
    }
    Ok(())
}

pub(super) fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

pub(super) fn parameter(message: String) -> PolysolidError {
    PolysolidError::Parameter(message)
}
