use crate::fixtures::polar;
use crate::kernel::ProfileEdge;
use std::f64::consts::{FRAC_PI_2, PI};

pub(crate) fn line(from: [f64; 2], to: [f64; 2]) -> ProfileEdge {
    ProfileEdge::Line { from, to }
}

pub(crate) fn arc(radius: f64, start_angle: f64, sweep_angle: f64) -> ProfileEdge {
    arc_at([0.0, 0.0], radius, start_angle, sweep_angle)
}

pub(crate) fn arc_at(
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    sweep_angle: f64,
) -> ProfileEdge {
    ProfileEdge::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    }
}

pub(crate) fn polygon(points: &[[f64; 2]]) -> Vec<ProfileEdge> {
    (0..points.len())
        .map(|index| line(points[index], points[(index + 1) % points.len()]))
        .collect()
}

pub(crate) fn rounded_outer() -> Vec<ProfileEdge> {
    vec![
        line([-2.0, -2.0], [2.0, -2.0]),
        arc_at([2.0, 0.0], 2.0, -FRAC_PI_2, PI),
        line([2.0, 2.0], [-2.0, 2.0]),
        line([-2.0, 2.0], [-2.0, -2.0]),
    ]
}

pub(crate) fn square_hole() -> Vec<ProfileEdge> {
    polygon(&[[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]])
}

pub(crate) fn quarter_annulus(outer: f64, inner: f64) -> Vec<ProfileEdge> {
    vec![
        arc(outer, 0.0, FRAC_PI_2),
        line([0.0, outer], [0.0, inner]),
        arc(inner, FRAC_PI_2, -FRAC_PI_2),
        line([inner, 0.0], [outer, 0.0]),
    ]
}

pub(crate) fn split_quarter_annulus() -> Vec<ProfileEdge> {
    let quarter = FRAC_PI_2;
    vec![
        arc(2.0, 0.0, quarter / 2.0),
        arc(2.0, quarter / 2.0, quarter / 2.0),
        line([0.0, 2.0], [0.0, 1.5]),
        arc(1.5, quarter, -quarter),
        line([1.5, 0.0], [2.0, 0.0]),
    ]
}

pub(crate) fn band(outer: f64, inner: f64, start: f64, sweep: f64) -> Vec<ProfileEdge> {
    vec![
        arc(outer, start, sweep),
        line(polar(outer, start + sweep), polar(inner, start + sweep)),
        arc(inner, start + sweep, -sweep),
        line(polar(inner, start), polar(outer, start)),
    ]
}

pub(crate) fn wide_arc() -> Vec<ProfileEdge> {
    band(2.1, 1.9, -FRAC_PI_2, 3.0 * FRAC_PI_2)
}

pub(crate) fn sector(reach: f64, start: f64, end: f64) -> Vec<[f64; 2]> {
    vec![[0.0, 0.0], polar(reach, start), polar(reach, end)]
}
