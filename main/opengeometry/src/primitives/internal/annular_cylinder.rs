use super::cylinder_with_circular_hole::cylinder_with_circular_hole;
use crate::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError};

pub fn annular_cylinder(
    id: String,
    frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    cylinder_with_circular_hole(
        id,
        frame,
        frame,
        inner_radius,
        outer_radius,
        height,
        accuracy,
    )
}
