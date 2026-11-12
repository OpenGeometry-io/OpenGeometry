use crate::brep::{GeometryError, PatchBounds, Surface, SurfaceGeometry, UVBox};
use crate::math::{add, cross, dot, norm, scale, sub, Interval, Point3};

const MAX_VISITS: usize = 50_000;
const MAX_DEPTH: usize = 24;
const LEAF_FRACTION: f64 = 1.0 / 64.0;
const NEWTON_STEPS: usize = 32;

pub(super) struct Ray {
    pub(super) origin: Point3,
    pub(super) direction: Point3,
    pub(super) domain: Interval,
    pub(super) tolerance: f64,
}

fn ray_in_box(bounds: PatchBounds, ray: &Ray) -> Option<Interval> {
    let (o, d) = (ray.origin, ray.direction);
    let (mut lo, mut hi) = (ray.domain.lo, ray.domain.hi);
    for axis in 0..3 {
        let (a, b) = (bounds.axes[axis].lo, bounds.axes[axis].hi);
        if d[axis] == 0.0 {
            if o[axis] < a || o[axis] > b {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((a - o[axis]) / d[axis], (b - o[axis]) / d[axis]);
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        lo = lo.max(t0);
        hi = hi.min(t1);
    }
    Interval::new(lo, hi).ok().filter(|_| lo <= hi)
}

fn diagonal(bounds: PatchBounds) -> f64 {
    norm(std::array::from_fn(|i| bounds.axes[i].width()))
}

pub(super) fn wrapped_ray_roots(
    surface: &SurfaceGeometry,
    uv: UVBox,
    ray: &Ray,
) -> Result<Option<Vec<f64>>, GeometryError> {
    let leaf = diagonal(surface.enclose(uv)?) * LEAF_FRACTION;
    let mut roots: Vec<f64> = Vec::new();
    let mut stack = vec![(uv, 0usize)];
    let mut visits = 0usize;
    while let Some((b, depth)) = stack.pop() {
        visits += 1;
        if visits > MAX_VISITS {
            return Ok(None);
        }
        let bounds = surface.enclose(b)?;
        let Some(span) = ray_in_box(bounds, ray) else {
            continue;
        };
        if diagonal(bounds) <= leaf || depth >= MAX_DEPTH {
            let start = [b[0].midpoint(), b[1].midpoint(), span.midpoint()];
            if let Some([u, v, t]) = ray_newton(surface, start, ray)? {
                if uv[0].contains(u)
                    && uv[1].contains(v)
                    && ray.domain.contains(t)
                    && roots.iter().all(|r| (r - t).abs() > ray.tolerance)
                {
                    roots.push(t);
                }
            }
            continue;
        }
        for child in halves(surface, b)? {
            stack.push((child, depth + 1));
        }
    }
    Ok(Some(roots))
}

fn halves(surface: &SurfaceGeometry, b: UVBox) -> Result<[UVBox; 2], GeometryError> {
    let jet = surface.derivatives([b[0].midpoint(), b[1].midpoint()])?;
    let axis = usize::from(norm(jet.dv) * b[1].width() > norm(jet.du) * b[0].width());
    let m = b[axis].midpoint();
    let (mut low, mut high) = (b, b);
    low[axis] = Interval::new(b[axis].lo, m)?;
    high[axis] = Interval::new(m, b[axis].hi)?;
    Ok([low, high])
}

fn ray_newton(
    surface: &SurfaceGeometry,
    start: [f64; 3],
    ray: &Ray,
) -> Result<Option<[f64; 3]>, GeometryError> {
    let mut x = start;
    for _ in 0..NEWTON_STEPS {
        let uv = [x[0], x[1]];
        let (point, jet) = match surface
            .point_at(uv)
            .and_then(|p| Ok((p, surface.derivatives(uv)?)))
        {
            Ok(value) => value,
            Err(GeometryError::SingularParameterization) => return Ok(None),
            Err(error) => return Err(error),
        };
        let r = sub(point, add(ray.origin, scale(ray.direction, x[2])));
        if norm(r) <= ray.tolerance {
            return Ok(Some(x));
        }
        let m = [jet.du, jet.dv, scale(ray.direction, -1.0)];
        let det = dot(m[0], cross(m[1], m[2]));
        if det.abs() <= f64::EPSILON * norm(m[0]) * norm(m[1]) {
            return Ok(None);
        }
        let rhs = scale(r, -1.0);
        x[0] += dot(rhs, cross(m[1], m[2])) / det;
        x[1] += dot(m[0], cross(rhs, m[2])) / det;
        x[2] += dot(m[0], cross(m[1], rhs)) / det;
        if x.iter().any(|v| !v.is_finite()) {
            return Ok(None);
        }
    }
    Ok(None)
}
