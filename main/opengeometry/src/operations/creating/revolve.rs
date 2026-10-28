use super::profile::{edges, profile_frame, ProfileLoop};
use crate::brep::{unit, Accuracy, BrepEnvelope, Frame3, GeometryError};
use crate::math::{cross, norm, scale, Point3};
use crate::operations::{invalid, OperationError};
use crate::primitives;

pub(crate) struct Axis {
    pub(crate) point: Point3,
    pub(crate) direction: Point3,
}

pub(crate) fn build(
    id: String,
    profile: ProfileLoop,
    axis: Axis,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, OperationError> {
    let g = accuracy.geometric;
    let plane = profile_frame(&profile, g)?;
    let frame = revolve_frame(&profile, plane, axis)?;
    let profile_edges = edges(&profile, meridian(frame.origin, frame.x, frame.z));
    primitives::revolved_profile(id, frame, profile_edges, accuracy).map_err(|error| match error {
        GeometryError::InvalidGeometry(message)
        | GeometryError::UnresolvedIntersection(message) => invalid(message),
        other => other.into(),
    })
}

fn revolve_frame(
    profile: &ProfileLoop,
    plane: Frame3,
    axis: Axis,
) -> Result<Frame3, OperationError> {
    let refusal = || invalid("revolve axis needs a finite point and a nonzero direction");
    if !axis.point.iter().all(|value| value.is_finite()) || norm(axis.direction) < f64::MIN_POSITIVE
    {
        return Err(refusal());
    }
    let z = unit(axis.direction).map_err(|_| refusal())?;
    let mut x = unit(cross(plane.z, z))?;
    if profile_side(profile, meridian(axis.point, x, z)) < 0.0 {
        x = scale(x, -1.0);
    }
    Ok(Frame3 {
        origin: axis.point,
        x,
        y: cross(z, x),
        z,
    })
}

fn meridian(origin: Point3, radial: Point3, axis: Point3) -> Frame3 {
    Frame3 {
        origin,
        x: radial,
        y: axis,
        z: cross(radial, axis),
    }
}

fn profile_side(profile: &ProfileLoop, meridian: Frame3) -> f64 {
    edges(profile, meridian)
        .iter()
        .flat_map(|edge| [edge.endpoints().0[0], edge.midpoint()[0]])
        .fold(0.0, |far: f64, r| if r.abs() > far.abs() { r } else { far })
}
