use super::wire_points::wire_from_points;
use crate::brep::{dimensions, Accuracy, BrepEnvelope, Frame3, GeometryError};

pub fn rectangle(
    id: String,
    frame: Frame3,
    width: f64,
    breadth: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&[width, breadth])?;
    if width <= 4.0 * accuracy.geometric || breadth <= 4.0 * accuracy.geometric {
        return Err(GeometryError::InvalidGeometry(
            "rectangle is below geometric resolution".into(),
        ));
    }
    let half_width = width / 2.0;
    let half_breadth = breadth / 2.0;
    let points = [
        frame.point([half_width, -half_breadth, 0.0]),
        frame.point([half_width, half_breadth, 0.0]),
        frame.point([-half_width, half_breadth, 0.0]),
        frame.point([-half_width, -half_breadth, 0.0]),
    ];
    wire_from_points(id, &points, true, accuracy)
}

pub fn rectangle_with_keys(
    id: String,
    frame: Frame3,
    width: f64,
    breadth: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GeometryError> {
    let brep = rectangle(id, frame, width, breadth, accuracy)?;
    let keys = (0..brep.topology.edges.len())
        .map(|index| format!("edge-{index}"))
        .collect();
    Ok((brep, keys))
}
