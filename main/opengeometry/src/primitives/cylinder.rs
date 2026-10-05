use crate::brep::{dimensions, Accuracy, BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::primitives::revolved_section::{section, SectionExtent};

pub fn cylinder(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    dimensions(&[radius, height])?;
    section(
        id,
        frame,
        SurfaceGeometry::Cylinder { frame, radius },
        &SectionExtent {
            r0: radius,
            r1: radius,
            z0: 0.0,
            z1: height,
        },
        accuracy,
    )
}
