#[cfg(test)]
mod tests;

use crate::brep::{CurveGeometry, GeometryError, Surface, SurfaceGeometry, SurfaceJet2, UVBox};
use crate::math::{norm, sub, Interval, Point3};

const SAMPLES: usize = 16;
const MAX_DEPTH: usize = 40;

#[derive(Clone, Debug)]
pub(super) struct FittedCurve {
    pub(super) controls: Vec<Point3>,
    pub(super) knots: Vec<f64>,
    pub(super) multiplicities: Vec<usize>,
    pub(super) error_bound: f64,
}

#[derive(Clone, Debug)]
pub(super) struct FittedSurface {
    pub(super) controls: Vec<Vec<Point3>>,
    pub(super) knots: [Vec<f64>; 2],
    pub(super) multiplicities: [Vec<usize>; 2],
    pub(super) error_bound: f64,
}

struct Grid {
    values: [Vec<f64>; 2],
    step: [f64; 2],
}

fn bernstein(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
}

fn knot_vector(values: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let last = values.len() - 1;
    let multiplicities = (0..=last)
        .map(|i| if i == 0 || i == last { 4 } else { 3 })
        .collect();
    (values.to_vec(), multiplicities)
}

fn positive_budget(tolerance: f64, budget: usize) -> Result<(), GeometryError> {
    if !tolerance.is_finite() || tolerance <= 0.0 || budget == 0 {
        return Err(GeometryError::InvalidGeometry(
            "B-spline fitting requires a positive tolerance and budget".into(),
        ));
    }
    Ok(())
}

fn hermite_span(
    curve: &CurveGeometry,
    a: f64,
    b: f64,
) -> Result<([Point3; 4], f64), GeometryError> {
    let h = b - a;
    let (p0, p3) = (curve.elementary_point(a)?, curve.elementary_point(b)?);
    let (t0, t1) = (curve.elementary_tangent(a)?, curve.elementary_tangent(b)?);
    let c = [
        p0,
        std::array::from_fn(|i| p0[i] + t0[i] * h / 3.0),
        std::array::from_fn(|i| p3[i] - t1[i] * h / 3.0),
        p3,
    ];
    let mut error: f64 = 0.0;
    for k in 1..SAMPLES {
        let s = k as f64 / SAMPLES as f64;
        let w = bernstein(s);
        let fitted: Point3 = std::array::from_fn(|i| (0..4).map(|j| w[j] * c[j][i]).sum());
        error = error.max(norm(sub(fitted, curve.elementary_point(a + s * h)?)));
    }
    Ok((c, error))
}

pub(super) fn fit_curve(
    curve: &CurveGeometry,
    range: Interval,
    tolerance: f64,
    max_segments: usize,
) -> Result<FittedCurve, GeometryError> {
    positive_budget(tolerance, max_segments)?;
    let mut stack = vec![(range.lo, range.hi, 0usize)];
    let mut accepted = Vec::new();
    while let Some((a, b, depth)) = stack.pop() {
        let (c, error) = hermite_span(curve, a, b)?;
        if error <= tolerance {
            accepted.push((a, c, error));
            continue;
        }
        if depth >= MAX_DEPTH || accepted.len() + stack.len() + 2 > max_segments {
            return Err(GeometryError::LimitExceeded(
                "B-spline curve fit spans".into(),
            ));
        }
        let m = 0.5 * (a + b);
        stack.push((m, b, depth + 1));
        stack.push((a, m, depth + 1));
    }
    accepted.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut controls = Vec::with_capacity(accepted.len() * 3 + 1);
    let mut error_bound: f64 = 0.0;
    for (index, (_, c, error)) in accepted.iter().enumerate() {
        controls.extend_from_slice(&c[usize::from(index > 0)..]);
        error_bound = error_bound.max(*error);
    }
    let mut values: Vec<f64> = accepted.iter().map(|x| x.0).collect();
    values.push(range.hi);
    let (knots, multiplicities) = knot_vector(&values);
    Ok(FittedCurve {
        controls,
        knots,
        multiplicities,
        error_bound,
    })
}

fn grid(domain: UVBox, n: [usize; 2]) -> Grid {
    Grid {
        values: std::array::from_fn(|axis| {
            (0..=n[axis])
                .map(|i| domain[axis].lo + domain[axis].width() * i as f64 / n[axis] as f64)
                .collect()
        }),
        step: std::array::from_fn(|axis| domain[axis].width() / n[axis] as f64),
    }
}

fn corner(index: usize) -> (usize, f64) {
    match index % 3 {
        0 => (index / 3, 0.0),
        1 => (index / 3, 1.0),
        _ => (index / 3 + 1, -1.0),
    }
}

fn grid_jets(
    surface: &SurfaceGeometry,
    grid: &Grid,
) -> Result<Vec<Vec<(Point3, SurfaceJet2)>>, GeometryError> {
    grid.values[0]
        .iter()
        .map(|&u| {
            grid.values[1]
                .iter()
                .map(|&v| Ok((surface.point_at([u, v])?, surface.derivatives([u, v])?)))
                .collect()
        })
        .collect()
}

fn hermite_controls(
    surface: &SurfaceGeometry,
    grid: &Grid,
) -> Result<Vec<Vec<Point3>>, GeometryError> {
    let jets = grid_jets(surface, grid)?;
    let [nu, nv] = grid.values.each_ref().map(|v| v.len() - 1);
    Ok((0..=3 * nu)
        .map(|i| {
            (0..=3 * nv)
                .map(|j| {
                    let ((ci, su), (cj, sv)) = (corner(i), corner(j));
                    let (p, jet) = &jets[ci][cj];
                    let (a, b) = (su * grid.step[0] / 3.0, sv * grid.step[1] / 3.0);
                    std::array::from_fn(|k| {
                        p[k] + a * jet.du[k] + b * jet.dv[k] + a * b * jet.duv[k]
                    })
                })
                .collect()
        })
        .collect())
}

fn patch_point(controls: &[Vec<Point3>], patch: [usize; 2], s: f64, t: f64) -> Point3 {
    let (wu, wv) = (bernstein(s), bernstein(t));
    std::array::from_fn(|k| {
        (0..4)
            .map(|x| {
                (0..4)
                    .map(|y| wu[x] * wv[y] * controls[3 * patch[0] + x][3 * patch[1] + y][k])
                    .sum::<f64>()
            })
            .sum()
    })
}

fn patch_errors(
    surface: &SurfaceGeometry,
    grid: &Grid,
    controls: &[Vec<Point3>],
) -> Result<[f64; 3], GeometryError> {
    let mut errors = [0.0_f64; 3];
    for pi in 0..grid.values[0].len() - 1 {
        for pj in 0..grid.values[1].len() - 1 {
            for a in 0..=SAMPLES {
                for b in 0..=SAMPLES {
                    let (s, t) = (a as f64 / SAMPLES as f64, b as f64 / SAMPLES as f64);
                    let exact = surface.point_at([
                        grid.values[0][pi] + s * grid.step[0],
                        grid.values[1][pj] + t * grid.step[1],
                    ])?;
                    let e = norm(sub(patch_point(controls, [pi, pj], s, t), exact));
                    errors[0] = errors[0].max(e);
                    if b == 0 || b == SAMPLES {
                        errors[1] = errors[1].max(e);
                    }
                    if a == 0 || a == SAMPLES {
                        errors[2] = errors[2].max(e);
                    }
                }
            }
        }
    }
    Ok(errors)
}

pub(super) fn fit_surface(
    surface: &SurfaceGeometry,
    domain: UVBox,
    tolerance: f64,
    max_patches: usize,
) -> Result<FittedSurface, GeometryError> {
    positive_budget(tolerance, max_patches)?;
    let mut n = [1usize, 1usize];
    loop {
        let grid = grid(domain, n);
        let controls = hermite_controls(surface, &grid)?;
        let [error, along_u, along_v] = patch_errors(surface, &grid, &controls)?;
        if error <= tolerance {
            let [ku, kv] = grid.values.each_ref().map(|v| knot_vector(v));
            return Ok(FittedSurface {
                controls,
                knots: [ku.0, kv.0],
                multiplicities: [ku.1, kv.1],
                error_bound: error,
            });
        }
        let (refine_u, refine_v) = (along_u > tolerance / 4.0, along_v > tolerance / 4.0);
        if refine_u || !refine_v {
            n[0] *= 2;
        }
        if refine_v || !refine_u {
            n[1] *= 2;
        }
        if n[0].saturating_mul(n[1]) > max_patches {
            return Err(GeometryError::LimitExceeded(
                "B-spline surface fit patches".into(),
            ));
        }
    }
}
