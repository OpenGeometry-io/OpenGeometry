use super::{
    geometry::{
        add, cross, dot, norm, scale, sub, unit, PatchBounds, Surface, SurfaceGeometry, UVBox,
    },
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

fn intersect_interval(a: Interval, b: Interval) -> Option<Interval> {
    Interval::new(a.lo.max(b.lo), a.hi.min(b.hi)).ok()
}

fn interval_dot(
    bounds: PatchBounds,
    origin: Point3,
    axis: Point3,
) -> Result<Interval, GeometryError> {
    let mut value = Interval::point(0.0)?;
    for coordinate in 0..3 {
        value = value.add(
            bounds.axes[coordinate]
                .sub(Interval::point(origin[coordinate])?)?
                .mul(Interval::point(axis[coordinate])?)?,
        )?;
    }
    Ok(value)
}

fn signed_root(radial: Interval, sign: Interval) -> Result<Option<Interval>, GeometryError> {
    if radial.hi < 0.0 {
        return Ok(None);
    }
    let root = Interval::new(radial.lo.max(0.0), radial.hi)?.sqrt()?;
    Ok(Some(if sign.lo >= 0.0 {
        root
    } else if sign.hi <= 0.0 {
        Interval::new(-root.hi, -root.lo)?
    } else {
        Interval::new(-root.hi, root.hi)?
    }))
}

fn perpendicular_cylinder_enclosure(
    definition: &IntersectionDefinition,
    store: &GeometryStore,
    segment: usize,
    range: Interval,
    parent: PatchBounds,
) -> Result<Option<(PatchBounds, f64)>, GeometryError> {
    let (
        SurfaceGeometry::Cylinder {
            frame: host,
            radius: host_radius,
        },
        SurfaceGeometry::Cylinder {
            frame: cutter,
            radius: cutter_radius,
        },
    ) = (
        store.surface(definition.surfaces[0])?,
        store.surface(definition.surfaces[1])?,
    )
    else {
        return Ok(None);
    };
    let u_axis = cutter.z;
    let v_axis = host.z;
    if dot(u_axis, v_axis).abs() > 1e-12 {
        return Ok(None);
    }
    let w_axis = cross(u_axis, v_axis);
    let host_to_cutter = sub(cutter.origin, host.origin);
    let centre = add(host.origin, scale(v_axis, dot(host_to_cutter, v_axis)));
    let residual = sub(
        sub(centre, cutter.origin),
        scale(u_axis, dot(sub(centre, cutter.origin), u_axis)),
    );
    if norm(residual) > 1e-12 {
        return Ok(None);
    }

    let first = &definition.anchors[segment];
    let last = &definition.anchors[segment + 1];
    let guide_axis = unit(sub(last.point, first.point))?;
    let certificate = definition.residual_tolerance * 8.0
        + 128.0 * f64::EPSILON * host_radius.max(*cutter_radius).max(norm(centre)).max(1.0);
    let theta_parent = definition.uv_tubes[segment][2];
    if theta_parent.width() >= std::f64::consts::PI {
        return Ok(None);
    }
    let u_parent = interval_dot(parent, centre, u_axis)?;
    if u_parent.contains(0.0) {
        return Ok(None);
    }
    let host_squared = Interval::new(
        (host_radius - certificate).max(0.0).powi(2),
        (host_radius + certificate).powi(2),
    )?;
    let v_cos = dot(cutter.x, v_axis);
    let v_sin = dot(cutter.y, v_axis);
    let w_cos = dot(cutter.x, w_axis);
    let w_sin = dot(cutter.y, w_axis);
    let trig = |cosine: Interval,
                sine: Interval,
                along_cosine: f64,
                along_sine: f64|
     -> Result<Interval, GeometryError> {
        Ok(cosine
            .mul(Interval::point(along_cosine)?)?
            .add(sine.mul(Interval::point(along_sine)?)?)?
            .mul(Interval::point(*cutter_radius)?)?)
    };
    let parent_cosine = theta_parent.cos()?;
    let parent_sine = theta_parent.sin()?;
    let parent_w = trig(parent_cosine, parent_sine, w_cos, w_sin)?;
    let parent_dw = trig(parent_sine, parent_cosine, -w_cos, w_sin)?;
    let parent_dv = trig(parent_sine, parent_cosine, -v_cos, v_sin)?;
    let Some(parent_u) = signed_root(host_squared.sub(parent_w.square()?)?, u_parent)? else {
        return Ok(None);
    };
    if parent_u.contains(0.0) {
        return Ok(None);
    }
    let parent_du = Interval::point(-1.0)?
        .mul(parent_w)?
        .mul(parent_dw)?
        .div(parent_u)?;
    let derivative = parent_du
        .mul(Interval::point(dot(guide_axis, u_axis))?)?
        .add(parent_dv.mul(Interval::point(dot(guide_axis, v_axis))?)?)?
        .add(parent_dw.mul(Interval::point(dot(guide_axis, w_axis))?)?)?;
    if derivative.contains(0.0) {
        return Ok(None);
    }

    let lo = definition.evaluate(range.lo, store)?.uv_b[0];
    let hi = definition.evaluate(range.hi, store)?.uv_b[0];
    let theta_pad = certificate / cutter_radius + 64.0 * f64::EPSILON;
    let theta = Interval::new(lo.min(hi) - theta_pad, lo.max(hi) + theta_pad)?;
    if theta.lo < theta_parent.lo - theta_pad || theta.hi > theta_parent.hi + theta_pad {
        return Ok(None);
    }
    let cosine = theta.cos()?;
    let sine = theta.sin()?;
    let v = trig(cosine, sine, v_cos, v_sin)?;
    let w = trig(cosine, sine, w_cos, w_sin)?;
    let Some(u) = signed_root(host_squared.sub(w.square()?)?, u_parent)? else {
        return Ok(None);
    };
    let local = [u, v, w];
    let axes = [u_axis, v_axis, w_axis];

    let mut world = [Interval::point(0.0)?; 3];
    for coordinate in 0..3 {
        let mut value = Interval::point(centre[coordinate])?;
        for axis in 0..3 {
            value = value.add(local[axis].mul(Interval::point(axes[axis][coordinate])?)?)?;
        }
        world[coordinate] = value.add(Interval::new(-certificate, certificate)?)?;
    }
    let minimum_u = ((host_radius - certificate).powi(2) - (cutter_radius + certificate).powi(2))
        .max(0.0)
        .sqrt();
    if minimum_u <= certificate {
        return Ok(None);
    }
    let radius = cutter_radius + certificate;
    let axial_curvature = radius.powi(2) / minimum_u + radius.powi(4) / minimum_u.powi(3);
    let host_angle_curvature = radius / minimum_u + radius.powi(3) / minimum_u.powi(3);
    // The exported XYZ chord and both surface pcurves use the same linear
    // parameter. Bound their support-space deviations over that parameter.
    let host_support_curvature =
        host_radius * (host_angle_curvature + (radius / minimum_u).powi(2));
    let curvature = (radius + axial_curvature).max(host_support_curvature);
    let chord_error = curvature * theta.width().powi(2) / 8.0 + certificate * 4.0;
    Ok(Some((PatchBounds { axes: world }, chord_error)))
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

    pub fn certified_chord_deviation(
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
