use super::sizes::{parameter, required_size};
use crate::brep::{Accuracy, BrepEnvelope, Frame3, GROUND};
use crate::math::scale;
use crate::primitives;
use crate::world_graph::error::GraphError;

pub(super) fn sphere_body(
    shape_id: String,
    radius: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(radius, accuracy.geometric, "radius")?;
    Ok((
        primitives::sphere(shape_id, GROUND, radius, accuracy)?,
        Vec::new(),
    ))
}

pub(super) fn cone_body(
    shape_id: String,
    [bottom, top]: [f64; 2],
    height: f64,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(height, accuracy.geometric, "height")?;
    for (name, radius) in [("radiusBottom", bottom), ("radiusTop", top)] {
        if radius != 0.0 {
            required_size(radius, accuracy.geometric, name)?;
        }
    }
    if bottom == 0.0 && top == 0.0 {
        return Err(parameter("cone needs at least one nonzero radius"));
    }
    let brep = if top == 0.0 {
        primitives::cone(shape_id, GROUND, bottom, height, accuracy)?
    } else if bottom == 0.0 {
        let inverted = Frame3 {
            origin: [0.0, height, 0.0],
            x: GROUND.x,
            y: scale(GROUND.y, -1.0),
            z: scale(GROUND.z, -1.0),
        };
        primitives::cone(shape_id, inverted, top, height, accuracy)?
    } else if (bottom - top).abs() <= 4.0 * accuracy.geometric {
        primitives::cylinder(shape_id, GROUND, bottom, height, accuracy)?
    } else {
        primitives::frustum(shape_id, GROUND, bottom, top, height, accuracy)?
    };
    Ok((brep, Vec::new()))
}

pub(super) fn torus_body(
    shape_id: String,
    [major, minor]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    required_size(major, accuracy.geometric, "majorRadius")?;
    required_size(minor, accuracy.geometric, "minorRadius")?;
    if major - minor <= 4.0 * accuracy.geometric {
        return Err(parameter(
            "torus minor radius must be below its major radius",
        ));
    }
    let frame = Frame3 {
        origin: [0.0, minor, 0.0],
        ..GROUND
    };
    Ok((
        primitives::torus(shape_id, frame, major, minor, accuracy)?,
        Vec::new(),
    ))
}
