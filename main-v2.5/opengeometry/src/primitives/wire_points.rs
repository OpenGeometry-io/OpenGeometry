use crate::brep::{
    checked, Accuracy, BrepEnvelope, Builder, CurveGeometry, GeometryError, Orientation, WireUse,
};
use crate::math::{norm, scale, sub, Interval, Point3};

pub(super) fn wire_from_points(
    id: String,
    points: &[Point3],
    closed: bool,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    accuracy.validate()?;
    if points.len() < if closed { 3 } else { 2 } {
        return Err(GeometryError::InvalidGeometry(
            "wire has too few points".into(),
        ));
    }
    if points.len() > u32::MAX as usize {
        return Err(GeometryError::LimitExceeded("wire vertex count".into()));
    }
    let mut builder = Builder::new(id, accuracy)?;
    let vertices = points
        .iter()
        .map(|&point| checked(point).map(|valid| builder.vertex(valid)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut uses = Vec::with_capacity(if closed {
        points.len()
    } else {
        points.len() - 1
    });
    for index in 0..if closed {
        points.len()
    } else {
        points.len() - 1
    } {
        let next = (index + 1) % points.len();
        let delta = sub(points[next], points[index]);
        let length = norm(delta);
        if length <= 4.0 * accuracy.geometric {
            return Err(GeometryError::InvalidGeometry(
                "wire edge is below geometric resolution".into(),
            ));
        }
        let edge = builder.edge(
            CurveGeometry::Line {
                origin: points[index],
                direction: scale(delta, 1.0 / length),
            },
            Interval::new(0.0, length)?,
            false,
        );
        uses.push(WireUse {
            edge,
            from: vertices[index],
            to: vertices[next],
            sense: Orientation::Forward,
        });
    }
    builder.finish_wire(uses, closed)
}
