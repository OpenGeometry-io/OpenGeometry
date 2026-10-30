use crate::brep::{dimensions, Accuracy, BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::math::scale;
use crate::primitives::cylinder::cylinder;
use crate::primitives::revolved_section::{section, SectionExtent};

pub fn frustum(
    id: String,
    frame: Frame3,
    lower_radius: f64,
    upper_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&[lower_radius, height])?;
    if !upper_radius.is_finite() || upper_radius < 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "invalid frustum upper radius".into(),
        ));
    }
    if lower_radius == upper_radius {
        return cylinder(id, frame, lower_radius, height, accuracy);
    }
    let k = (upper_radius - lower_radius).abs() / height;
    let semi_angle = k.atan();
    let mut cone_frame = frame;
    let (r0, r1, z0, z1) = if upper_radius > lower_radius {
        cone_frame.origin = frame.point([0.0, 0.0, -lower_radius / k]);
        (
            lower_radius,
            upper_radius,
            lower_radius / k,
            upper_radius / k,
        )
    } else {
        cone_frame.origin = frame.point([0.0, 0.0, lower_radius / k]);
        cone_frame.y = scale(frame.y, -1.0);
        cone_frame.z = scale(frame.z, -1.0);
        (
            upper_radius,
            lower_radius,
            upper_radius / k,
            lower_radius / k,
        )
    };
    section(
        id,
        cone_frame,
        SurfaceGeometry::Cone {
            frame: cone_frame,
            semi_angle,
        },
        &SectionExtent { r0, r1, z0, z1 },
        accuracy,
    )
}
