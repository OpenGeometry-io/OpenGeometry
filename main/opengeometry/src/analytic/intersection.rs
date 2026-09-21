use super::{
    geometry::{add, cross, dot, norm, scale, sub, unit, PatchBounds, Surface, UVBox},
    topology::GeometryStore,
    GeometryError, Point3, UV,
};
use crate::math::{finite, interval::Interval, solve::solve};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SsiBudget {
    pub max_patch_pairs: usize,
    pub max_steps_per_branch: usize,
    pub max_newton_iterations: usize,
    pub max_subdivision_depth: usize,
}
impl Default for SsiBudget {
    fn default() -> Self {
        Self {
            max_patch_pairs: 200_000,
            max_steps_per_branch: 50_000,
            max_newton_iterations: 12,
            max_subdivision_depth: 48,
        }
    }
}
impl SsiBudget {
    pub fn validate(self) -> Result<(), GeometryError> {
        if self.max_patch_pairs == 0
            || self.max_steps_per_branch == 0
            || self.max_newton_iterations == 0
            || self.max_newton_iterations > 64
            || self.max_subdivision_depth == 0
            || self.max_subdivision_depth > 128
        {
            return Err(GeometryError::InvalidGeometry(
                "invalid SSI resource limits".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceAnchor {
    pub parameter: f64,
    pub point: Point3,
    pub uv_a: UV,
    pub uv_b: UV,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntersectionDefinition {
    pub surfaces: [u32; 2],
    pub anchors: Vec<TraceAnchor>,
    pub uv_tubes: Vec<[Interval; 4]>,
    pub residual_tolerance: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct IntersectionPoint {
    pub point: Point3,
    pub uv_a: UV,
    pub uv_b: UV,
    pub tangent: Point3,
    pub support_error: f64,
}

impl IntersectionDefinition {
    fn shape(&self) -> Result<(), GeometryError> {
        if self.anchors.len() < 2
            || self.uv_tubes.len() + 1 != self.anchors.len()
            || !self.residual_tolerance.is_finite()
            || self.residual_tolerance <= 0.0
        {
            return Err(GeometryError::InvalidGeometry(
                "invalid intersection branch shape or tolerance".into(),
            ));
        }
        Ok(())
    }

    pub fn validate(&self, store: &GeometryStore) -> Result<(), GeometryError> {
        self.shape()?;
        let a = store.surface(self.surfaces[0])?;
        let b = store.surface(self.surfaces[1])?;
        for (i, anchor) in self.anchors.iter().enumerate() {
            finite(anchor.parameter)?;
            for v in anchor.point {
                finite(v)?;
            }
            if i > 0 && anchor.parameter <= self.anchors[i - 1].parameter {
                return Err(GeometryError::InvalidGeometry(
                    "intersection parameters are not increasing".into(),
                ));
            }
            if norm(sub(a.point_at(anchor.uv_a)?, anchor.point)) > self.residual_tolerance
                || norm(sub(b.point_at(anchor.uv_b)?, anchor.point)) > self.residual_tolerance
            {
                return Err(GeometryError::InvalidGeometry(
                    "intersection anchor misses its supports".into(),
                ));
            }
        }
        for (i, tube) in self.uv_tubes.iter().enumerate() {
            for d in tube {
                Interval::new(d.lo, d.hi)?;
            }
            for anchor in [&self.anchors[i], &self.anchors[i + 1]] {
                let uv = [
                    anchor.uv_a[0],
                    anchor.uv_a[1],
                    anchor.uv_b[0],
                    anchor.uv_b[1],
                ];
                if (0..4).any(|j| !tube[j].contains(uv[j])) {
                    return Err(GeometryError::InvalidGeometry(
                        "intersection anchor leaves its UV tube".into(),
                    ));
                }
            }
            if norm(sub(self.anchors[i + 1].point, self.anchors[i].point)) == 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "zero-length tracing guide".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn evaluate(
        &self,
        t: f64,
        store: &GeometryStore,
    ) -> Result<IntersectionPoint, GeometryError> {
        self.evaluate_with_budget(t, store, SsiBudget::default())
    }

    pub fn evaluate_with_budget(
        &self,
        t: f64,
        store: &GeometryStore,
        budget: SsiBudget,
    ) -> Result<IntersectionPoint, GeometryError> {
        self.shape()?;
        budget.validate()?;
        finite(t)?;
        let first = &self.anchors[0];
        let last = &self.anchors[self.anchors.len() - 1];
        if t < first.parameter || t > last.parameter {
            return Err(GeometryError::InvalidGeometry(
                "parameter outside intersection branch".into(),
            ));
        }
        let index = self
            .anchors
            .partition_point(|a| a.parameter <= t)
            .saturating_sub(1)
            .min(self.anchors.len() - 2);
        let left = &self.anchors[index];
        let right = &self.anchors[index + 1];
        let span = finite(right.parameter - left.parameter)?;
        if span <= 0.0 {
            return Err(GeometryError::InvalidGeometry(
                "invalid trace parameter interval".into(),
            ));
        }
        let w = (t - left.parameter) / span;
        let guide = add(scale(left.point, 1.0 - w), scale(right.point, w));
        let velocity = scale(sub(right.point, left.point), 1.0 / span);
        let gauge = unit(velocity)?;
        let tube = self.uv_tubes[index];
        for d in tube {
            Interval::new(d.lo, d.hi)?;
        }
        let a = store.surface(self.surfaces[0])?;
        let b = store.surface(self.surfaces[1])?;
        let mut uv = [
            left.uv_a[0] * (1.0 - w) + right.uv_a[0] * w,
            left.uv_a[1] * (1.0 - w) + right.uv_a[1] * w,
            left.uv_b[0] * (1.0 - w) + right.uv_b[0] * w,
            left.uv_b[1] * (1.0 - w) + right.uv_b[1] * w,
        ];
        if (0..4).any(|i| !tube[i].contains(uv[i])) {
            return Err(GeometryError::InvalidGeometry(
                "trace predictor leaves its UV tube".into(),
            ));
        }
        for _ in 0..budget.max_newton_iterations {
            let ja = a.derivatives([uv[0], uv[1]])?;
            let jb = b.derivatives([uv[2], uv[3]])?;
            let diff = sub(ja.point, jb.point);
            let residual = [diff[0], diff[1], diff[2], dot(sub(ja.point, guide), gauge)];
            let error = residual.into_iter().map(f64::abs).fold(0.0, f64::max);
            if norm(diff) <= self.residual_tolerance && residual[3].abs() <= self.residual_tolerance
            {
                let direction = unit(cross(
                    a.normal_at([uv[0], uv[1]])?,
                    b.normal_at([uv[2], uv[3]])?,
                ))?;
                let projection = dot(direction, gauge);
                if projection.abs() <= 64.0 * f64::EPSILON {
                    return Err(GeometryError::UnresolvedIntersection(
                        "trace guide is tangent to correction plane".into(),
                    ));
                }
                return Ok(IntersectionPoint {
                    point: ja.point,
                    uv_a: [uv[0], uv[1]],
                    uv_b: [uv[2], uv[3]],
                    tangent: scale(direction, dot(velocity, gauge) / projection),
                    support_error: norm(diff),
                });
            }
            let mut jacobian = [[0.0; 4]; 4];
            for i in 0..3 {
                jacobian[i] = [ja.du[i], ja.dv[i], -jb.du[i], -jb.dv[i]];
            }
            jacobian[3] = [dot(ja.du, gauge), dot(ja.dv, gauge), 0.0, 0.0];
            let step = solve(jacobian, residual.map(|v| -v))
                .map_err(|e| {
                    GeometryError::UnresolvedIntersection(format!("intersection correction: {e}"))
                })?
                .value;
            let mut accepted = false;
            let mut fraction = 1.0;
            for _ in 0..12 {
                let candidate = std::array::from_fn(|i| uv[i] + fraction * step[i]);
                if (0..4).all(|i| tube[i].contains(candidate[i]))
                    && a.parameter_in_domain([candidate[0], candidate[1]])
                    && b.parameter_in_domain([candidate[2], candidate[3]])
                {
                    let pa = a.point_at([candidate[0], candidate[1]])?;
                    let pb = b.point_at([candidate[2], candidate[3]])?;
                    let diff = sub(pa, pb);
                    let trial = diff
                        .into_iter()
                        .map(f64::abs)
                        .fold(dot(sub(pa, guide), gauge).abs(), f64::max);
                    if trial < error {
                        uv = candidate;
                        accepted = true;
                        break;
                    }
                }
                fraction *= 0.5;
            }
            if !accepted {
                return Err(GeometryError::UnresolvedIntersection(
                    "correction leaves branch tube or does not decrease residual".into(),
                ));
            }
        }
        Err(GeometryError::UnresolvedIntersection(
            "intersection correction budget exhausted".into(),
        ))
    }

    pub fn enclose(
        &self,
        range: Interval,
        store: &GeometryStore,
    ) -> Result<PatchBounds, GeometryError> {
        self.shape()?;
        Interval::new(range.lo, range.hi)?;
        if range.lo < self.anchors[0].parameter
            || range.hi > self.anchors[self.anchors.len() - 1].parameter
        {
            return Err(GeometryError::InvalidGeometry(
                "bounds outside intersection branch".into(),
            ));
        }
        let a = store.surface(self.surfaces[0])?;
        let b = store.surface(self.surfaces[1])?;
        let mut result: Option<PatchBounds> = None;
        for (i, tube) in self.uv_tubes.iter().enumerate() {
            if self.anchors[i + 1].parameter < range.lo || self.anchors[i].parameter > range.hi {
                continue;
            }
            let box_a: UVBox = [tube[0], tube[1]];
            let box_b: UVBox = [tube[2], tube[3]];
            let ba = a.enclose(box_a)?;
            let bb = b.enclose(box_b)?;
            let mut axes = [Interval::point(0.0)?; 3];
            for j in 0..3 {
                axes[j] = Interval::new(
                    ba.axes[j].lo.max(bb.axes[j].lo),
                    ba.axes[j].hi.min(bb.axes[j].hi),
                )
                .map_err(|_| {
                    GeometryError::InvalidGeometry("disjoint trace support boxes".into())
                })?;
            }
            result = Some(match result {
                None => PatchBounds { axes },
                Some(previous) => PatchBounds {
                    axes: std::array::from_fn(|j| previous.axes[j].hull(axes[j])),
                },
            });
        }
        result.ok_or_else(|| GeometryError::InvalidGeometry("empty trace interval".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{Curve, CurveGeometry, Frame3, SurfaceGeometry};
    fn circle_trace() -> GeometryStore {
        let mut store = GeometryStore::new();
        store.surfaces.push(SurfaceGeometry::Sphere {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        });
        store.surfaces.push(SurfaceGeometry::Plane {
            frame: Frame3::IDENTITY,
        });
        let mut anchors = Vec::new();
        let mut uv_tubes = Vec::new();
        for i in 0..=8 {
            let u = i as f64 * 0.1;
            anchors.push(TraceAnchor {
                parameter: u,
                point: [u.cos(), u.sin(), 0.0],
                uv_a: [u, 0.0],
                uv_b: [u.cos(), u.sin()],
            });
            if i > 0 {
                let left = (i - 1) as f64 * 0.1;
                uv_tubes.push([
                    Interval::new(left - 0.02, u + 0.02).unwrap(),
                    Interval::new(-0.02, 0.02).unwrap(),
                    Interval::new(
                        left.cos().min(u.cos()) - 0.02,
                        left.cos().max(u.cos()) + 0.02,
                    )
                    .unwrap(),
                    Interval::new(
                        left.sin().min(u.sin()) - 0.02,
                        left.sin().max(u.sin()) + 0.02,
                    )
                    .unwrap(),
                ]);
            }
        }
        store.intersections.push(IntersectionDefinition {
            surfaces: [0, 1],
            anchors,
            uv_tubes,
            residual_tolerance: 1e-12,
        });
        store
            .curves
            .push(CurveGeometry::Intersection { definition: 0 });
        store
    }
    #[test]
    fn intersection_evaluation_corrects_off_linear_predictor() {
        let store = circle_trace();
        store.intersections[0].validate(&store).unwrap();
        let point = store.intersections[0].evaluate(0.35, &store).unwrap();
        assert!((norm(point.point) - 1.0).abs() < 1e-12);
        assert!(point.point[2].abs() < 1e-12);
        assert!(point.support_error < 1e-12);
        let chord = scale(
            add(
                store.intersections[0].anchors[3].point,
                store.intersections[0].anchors[4].point,
            ),
            0.5,
        );
        assert!(norm(sub(point.point, chord)) > 0.001);
        assert!(dot(point.point, point.tangent).abs() < 1e-12);
        assert!(store
            .curve(0)
            .unwrap()
            .enclose(Interval::new(0.3, 0.4).unwrap())
            .unwrap()
            .contains(point.point));
    }
    #[test]
    fn budget_failure_and_invalid_anchor_are_explicit() {
        let mut store = circle_trace();
        assert!(matches!(
            store.intersections[0].evaluate_with_budget(
                0.35,
                &store,
                SsiBudget {
                    max_newton_iterations: 1,
                    ..SsiBudget::default()
                }
            ),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
        store.intersections[0].anchors[3].point[2] = 0.1;
        assert!(store.intersections[0].validate(&store).is_err());
    }
}
