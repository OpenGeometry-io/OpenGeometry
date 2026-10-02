use super::wire_points::wire_from_points;
use crate::brep::{Accuracy, BrepEnvelope, GeometryError};
use crate::math::Point3;

pub fn polyline(
    id: String,
    points: &[Point3],
    closed: bool,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    wire_from_points(id, points, closed, accuracy)
}

pub fn polyline_with_keys(
    id: String,
    points: &[Point3],
    closed: bool,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GeometryError> {
    let brep = polyline(id, points, closed, accuracy)?;
    let keys = (0..brep.topology.edges.len())
        .map(|index| format!("seg-{index}"))
        .collect();
    Ok((brep, keys))
}
