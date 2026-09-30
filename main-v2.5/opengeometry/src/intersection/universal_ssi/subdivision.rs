use crate::brep::{GeometryError, PatchBounds, UVBox};
use crate::math::Interval;

pub(super) fn split(domain: UVBox, axis: usize) -> Result<[UVBox; 2], GeometryError> {
    let midpoint = domain[axis].midpoint();
    let mut left = domain;
    let mut right = domain;
    left[axis] = Interval::new(domain[axis].lo, midpoint)?;
    right[axis] = Interval::new(midpoint, domain[axis].hi)?;
    Ok([left, right])
}

pub(super) fn bounds_diameter(bounds: PatchBounds) -> f64 {
    bounds
        .axes
        .iter()
        .map(|axis| axis.width() * axis.width())
        .sum::<f64>()
        .sqrt()
}
