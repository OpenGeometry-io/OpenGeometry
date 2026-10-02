use super::enclosure::{intersect_interval, perpendicular_cylinder_enclosure};
use super::{IntersectionPoint, SsiBudget};
use crate::brep::bounds::PatchBounds;
use crate::brep::error::GeometryError;
use crate::brep::frame::{unit, UVBox};
use crate::brep::geometry::intersection_definition::IntersectionDefinition;
use crate::brep::geometry::store::GeometryStore;
use crate::brep::geometry::surface::{Surface, SurfaceGeometry};
use crate::math::{add, cross, dot, finite, norm, scale, solve, sub, Interval, Point3};

struct TracePredictor<'a> {
    a: &'a SurfaceGeometry,
    b: &'a SurfaceGeometry,
    tube: [Interval; 4],
    guide: Point3,
    velocity: Point3,
    gauge: Point3,
    uv: [f64; 4],
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

    pub(crate) fn validate(&self, store: &GeometryStore) -> Result<(), GeometryError> {
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

    pub(crate) fn evaluate(
        &self,
        t: f64,
        store: &GeometryStore,
    ) -> Result<IntersectionPoint, GeometryError> {
        self.evaluate_with_budget(t, store, SsiBudget::default())
    }

    pub(super) fn evaluate_with_budget(
        &self,
        t: f64,
        store: &GeometryStore,
        budget: SsiBudget,
    ) -> Result<IntersectionPoint, GeometryError> {
        self.shape()?;
        budget.validate()?;
        finite(t)?;
        let TracePredictor {
            a,
            b,
            tube,
            guide,
            velocity,
            gauge,
            mut uv,
        } = self.trace_predictor(t, store)?;
        for _ in 0..budget.max_newton_iterations {
            let ja = a.derivatives([uv[0], uv[1]])?;
            let jb = b.derivatives([uv[2], uv[3]])?;
            let diff = sub(ja.point, jb.point);
            let residual = [diff[0], diff[1], diff[2], dot(sub(ja.point, guide), gauge)];
            let error = residual.into_iter().map(f64::abs).fold(0.0, f64::max);
            if norm(diff) <= self.residual_tolerance && residual[3].abs() <= self.residual_tolerance
            {
                return traced_point(a, b, uv, ja.point, diff, velocity, gauge);
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

    fn trace_predictor<'a>(
        &self,
        t: f64,
        store: &'a GeometryStore,
    ) -> Result<TracePredictor<'a>, GeometryError> {
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
        let uv = [
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
        Ok(TracePredictor {
            a,
            b,
            tube,
            guide,
            velocity,
            gauge,
            uv,
        })
    }

    pub(crate) fn enclose(
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
            if let Some((tighter, _)) = perpendicular_cylinder_enclosure(
                self,
                store,
                i,
                Interval::new(
                    range.lo.max(self.anchors[i].parameter),
                    range.hi.min(self.anchors[i + 1].parameter),
                )?,
                PatchBounds { axes },
            )? {
                for axis in 0..3 {
                    axes[axis] =
                        intersect_interval(axes[axis], tighter.axes[axis]).unwrap_or(axes[axis]);
                }
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

    pub(crate) fn certified_chord_deviation(
        &self,
        range: Interval,
        store: &GeometryStore,
    ) -> Result<Option<f64>, GeometryError> {
        let Some(segment) = self
            .anchors
            .windows(2)
            .position(|pair| range.lo >= pair[0].parameter && range.hi <= pair[1].parameter)
        else {
            return Ok(None);
        };
        let a = store.surface(self.surfaces[0])?;
        let b = store.surface(self.surfaces[1])?;
        let tube = self.uv_tubes[segment];
        let box_a = a.enclose([tube[0], tube[1]])?;
        let box_b = b.enclose([tube[2], tube[3]])?;
        let mut axes = [Interval::point(0.0)?; 3];
        for coordinate in 0..3 {
            let Some(overlap) = intersect_interval(box_a.axes[coordinate], box_b.axes[coordinate])
            else {
                return Ok(None);
            };
            axes[coordinate] = overlap;
        }
        Ok(
            perpendicular_cylinder_enclosure(self, store, segment, range, PatchBounds { axes })?
                .map(|(_, error)| error),
        )
    }
}

fn traced_point(
    a: &SurfaceGeometry,
    b: &SurfaceGeometry,
    uv: [f64; 4],
    point: Point3,
    diff: Point3,
    velocity: Point3,
    gauge: Point3,
) -> Result<IntersectionPoint, GeometryError> {
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
    Ok(IntersectionPoint {
        point,
        uv_a: [uv[0], uv[1]],
        uv_b: [uv[2], uv[3]],
        tangent: scale(direction, dot(velocity, gauge) / projection),
        support_error: norm(diff),
    })
}
