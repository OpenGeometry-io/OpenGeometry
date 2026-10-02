use crate::brep::{bounds_chord_deviation, Curve, GeometryError};
use crate::math::Interval;

pub(super) fn bounded_curve_count(
    curve: &impl Curve,
    range: Interval,
    error: f64,
    max_segments: usize,
) -> Result<usize, GeometryError> {
    let mut stack = vec![(range, 0usize)];
    let mut maximum_depth = 0usize;
    let mut visits = 0usize;
    while let Some((interval, depth)) = stack.pop() {
        visits = visits
            .checked_add(1)
            .ok_or_else(|| GeometryError::LimitExceeded("curve tessellation visits".into()))?;
        if visits > max_segments.saturating_mul(2) {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation visits".into(),
            ));
        }
        let a = curve.point_at(interval.lo)?;
        let b = curve.point_at(interval.hi)?;
        if bounds_chord_deviation(curve.enclose(interval)?, a, b) <= error {
            maximum_depth = maximum_depth.max(depth);
            continue;
        }
        if depth >= usize::BITS as usize - 1 || (1usize << (depth + 1)) > max_segments {
            return Err(GeometryError::LimitExceeded(
                "curve tessellation segments".into(),
            ));
        }
        let midpoint = interval.midpoint();
        let left = Interval::new(interval.lo, midpoint)?;
        let right = Interval::new(midpoint, interval.hi)?;
        stack.push((right, depth + 1));
        stack.push((left, depth + 1));
    }
    Ok(1usize << maximum_depth)
}
