use crate::brep::{GeometryError, Surface, SurfaceGeometry};
use crate::math::{norm, Interval, Point3};

const MARGIN: f64 = 1.5;
const GRID: usize = 8;

pub(super) fn second_derivative_bounds(
    surface: &SurfaceGeometry,
    bounds: [Interval; 2],
) -> Result<[f64; 2], GeometryError> {
    let mut maximum = [0.0_f64; 2];
    for i in 0..=GRID {
        for j in 0..=GRID {
            let uv = [
                bounds[0].lo + bounds[0].width() * i as f64 / GRID as f64,
                bounds[1].lo + bounds[1].width() * j as f64 / GRID as f64,
            ];
            let jet = surface.derivatives(uv)?;
            maximum[0] = maximum[0].max(norm(jet.duu));
            maximum[1] = maximum[1].max(norm(jet.dvv));
        }
    }
    Ok(maximum.map(|m| m * MARGIN))
}

pub(super) fn chord_deviation(surface: &SurfaceGeometry, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (du, dv) = (b[0] - a[0], b[1] - a[1]);
    let mut maximum = 0.0_f64;
    for k in 0..=2 {
        let t = k as f64 / 2.0;
        let Ok(jet) = surface.derivatives([a[0] + t * du, a[1] + t * dv]) else {
            return f64::INFINITY;
        };
        let acceleration: Point3 = std::array::from_fn(|i| {
            jet.duu[i] * du * du + 2.0 * jet.duv[i] * du * dv + jet.dvv[i] * dv * dv
        });
        maximum = maximum.max(norm(acceleration));
    }
    maximum * MARGIN / 8.0
}
