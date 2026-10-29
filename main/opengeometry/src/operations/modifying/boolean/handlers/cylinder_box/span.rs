use crate::math::dot;
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};

pub(super) fn cylinder_spans(box_: &BoxInput<'_>, cylinder: &CylinderInput<'_>) -> [[f64; 2]; 3] {
    let origin = box_.frame.local(cylinder.frame.origin);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    std::array::from_fn(|axis| {
        let axial_rate = dot(cylinder.frame.z, box_axes[axis]);
        let radial_extent = cylinder.radius * (1.0 - axial_rate * axial_rate).max(0.0).sqrt();
        let other = origin[axis] + axial_rate * cylinder.height;
        [
            origin[axis].min(other) - radial_extent,
            origin[axis].max(other) + radial_extent,
        ]
    })
}
