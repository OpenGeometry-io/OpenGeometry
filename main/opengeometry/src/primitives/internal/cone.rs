use crate::brep::{dimensions, Accuracy, BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::math::scale;
use crate::primitives::revolved_section::{section, SectionExtent};

pub fn cone(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    dimensions(&[radius, height])?;
    let semi_angle = radius.atan2(height);
    let frame = Frame3 {
        origin: frame.point([0.0, 0.0, height]),
        x: frame.x,
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
    };
    section(
        id,
        frame,
        SurfaceGeometry::Cone { frame, semi_angle },
        &SectionExtent {
            r0: 0.0,
            r1: radius,
            z0: 0.0,
            z1: height,
        },
        accuracy,
    )
}
