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
        let ends = [
            (vertices[index], points[index]),
            (vertices[next], points[next]),
        ];
        uses.push(line_use(&mut builder, ends, accuracy.geometric)?);
    }
    builder.finish_wire(uses, closed)
}

pub(super) fn line_use(
    builder: &mut Builder,
    ends: [(u32, Point3); 2],
    geometric: f64,
) -> Result<WireUse, GeometryError> {
    let delta = sub(ends[1].1, ends[0].1);
    let length = norm(delta);
    if length <= 4.0 * geometric {
        return Err(GeometryError::InvalidGeometry(
            "wire edge is below geometric resolution".into(),
        ));
    }
    let edge = builder.edge(
        CurveGeometry::Line {
            origin: ends[0].1,
            direction: scale(delta, 1.0 / length),
        },
        Interval::new(0.0, length)?,
        false,
    );
    Ok(WireUse {
        edge,
        from: ends[0].0,
        to: ends[1].0,
        sense: Orientation::Forward,
    })
}
