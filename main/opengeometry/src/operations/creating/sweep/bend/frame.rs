use crate::brep::{unit, Accuracy};
use crate::math::{add, cross, dot, norm, scale, sub, Point3};
use crate::operations::{invalid, OperationError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bend {
    pub(crate) centre: Point3,
    pub(crate) axis: Point3,
    pub(crate) angle: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BendFrame {
    pub(crate) centre: Point3,
    pub(crate) axis: Point3,
    pub(crate) radial: Point3,
    pub(crate) radius: f64,
    pub(crate) angle: f64,
}

pub(crate) fn bend_frame(
    bend: Bend,
    start: Point3,
    reach: f64,
    accuracy: Accuracy,
) -> Result<BendFrame, OperationError> {
    let finite = bend
        .centre
        .iter()
        .chain(&bend.axis)
        .chain(&start)
        .chain([&bend.angle, &reach])
        .all(|value| value.is_finite());
    if !finite || bend.angle <= 0.0 || bend.angle > std::f64::consts::PI {
        return Err(invalid("sweep bend is invalid"));
    }
    let axis = unit(bend.axis)?;
    let offset = sub(start, bend.centre);
    let height = dot(offset, axis);
    if height.abs() > 4.0 * accuracy.geometric {
        return Err(invalid("sweep bend centre is off the path plane"));
    }
    let in_plane = sub(offset, scale(axis, height));
    let radius = norm(in_plane);
    if radius * bend.angle <= 4.0 * accuracy.geometric {
        return Err(invalid("sweep path has a short segment"));
    }
    if radius <= reach + 4.0 * accuracy.geometric {
        return Err(OperationError::SweepSelfIntersection(
            "bend radius is within the profile reach".into(),
        ));
    }
    Ok(BendFrame {
        centre: bend.centre,
        axis,
        radial: unit(in_plane)?,
        radius,
        angle: bend.angle,
    })
}

impl BendFrame {
    pub(crate) fn rotated(&self, vector: Point3) -> Point3 {
        rotation(self.axis, self.angle, vector)
    }

    pub(crate) fn section(&self, point: Point3) -> [f64; 2] {
        let offset = sub(point, self.centre);
        [dot(offset, self.radial), dot(offset, self.axis)]
    }
}

pub(crate) fn rotation(axis: Point3, angle: f64, vector: Point3) -> Point3 {
    let (sin, cos) = angle.sin_cos();
    let along = scale(axis, dot(axis, vector) * (1.0 - cos));
    add(
        add(scale(vector, cos), scale(cross(axis, vector), sin)),
        along,
    )
}
