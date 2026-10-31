use crate::brep::error::GeometryError;
use crate::brep::frame::{Frame3, UVBox};
use crate::math::Interval;

pub(super) fn reached(frame: &Frame3, axis: usize) -> usize {
    [frame.x[axis], frame.y[axis], frame.z[axis]]
        .iter()
        .filter(|v| **v != 0.0)
        .count()
}

pub(super) fn narrowed(axis: Interval, exact: Interval) -> Result<Interval, GeometryError> {
    Ok(Interval::new(axis.lo.max(exact.lo), axis.hi.min(exact.hi))?)
}

pub(super) fn turned_axis(
    center: [f64; 2],
    along: [f64; 2],
    range: Interval,
) -> Result<Interval, GeometryError> {
    let [a, b] = along;
    let middle = center[0] + center[1];
    let value = |t: f64| middle + a * t.cos() + b * t.sin();
    let (first, last) = (value(range.lo), value(range.hi));
    let (mut lo, mut hi) = (first.min(last), first.max(last));
    let peak = a.hypot(b);
    let phase = b.atan2(a);
    if reaches(range, phase) {
        hi = hi.max(middle + peak);
    }
    if reaches(range, phase + std::f64::consts::PI) {
        lo = lo.min(middle - peak);
    }
    let size = center[0].abs() + center[1].abs() + a.abs() + b.abs();
    let pad = 16.0 * f64::EPSILON * size + f64::MIN_POSITIVE;
    Ok(Interval::new(lo - pad, hi + pad)?)
}

fn reaches(range: Interval, angle: f64) -> bool {
    let tau = std::f64::consts::TAU;
    let slack = 1e-9;
    if range.width() >= tau {
        return true;
    }
    let turns = ((range.lo - slack - angle) / tau).ceil();
    angle + turns * tau <= range.hi + slack
}

pub(super) fn swept_axis(
    frame: &Frame3,
    axis: usize,
    radius_at: impl Fn(f64) -> f64,
    domain: UVBox,
) -> Result<Interval, GeometryError> {
    let ring = |v: f64| {
        let radius = radius_at(v);
        let center = [frame.origin[axis], v * frame.z[axis]];
        turned_axis(
            center,
            [radius * frame.x[axis], radius * frame.y[axis]],
            domain[0],
        )
    };
    Ok(ring(domain[1].lo)?.hull(ring(domain[1].hi)?))
}

pub(super) fn ring_axis(
    frame: &Frame3,
    axis: usize,
    major_radius: f64,
    minor_radius: f64,
) -> Result<Interval, GeometryError> {
    let planar = Interval::point(frame.x[axis])?
        .square()?
        .add_interval(Interval::point(frame.y[axis])?.square()?)?;
    let whole = planar.add_interval(Interval::point(frame.z[axis])?.square()?)?;
    let half = Interval::point(major_radius)?
        .mul_interval(planar.sqrt()?)?
        .add_interval(Interval::point(minor_radius)?.mul_interval(whole.sqrt()?)?)?;
    Ok(Interval::point(frame.origin[axis])?.add_interval(Interval::new(-half.hi, half.hi)?)?)
}
