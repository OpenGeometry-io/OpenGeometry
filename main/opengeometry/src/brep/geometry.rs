//! Analytic curve & surface geometry attached to B-rep edges and faces
//! (debt item D1).
//!
//! Historically the kernel destroyed curvature at construction time: an arc or
//! circle was tessellated into straight `segments` edges and the center/radius
//! discarded, so exactness could never be recovered for export, offset, or any
//! future analytic feature. This module restores the defining property of a
//! B-rep kernel — edges carry an exact [`CurveGeometry`] and faces an exact
//! [`SurfaceGeometry`]. The tessellated vertices remain in the B-rep as the
//! *derived* discretization used by the mesh/topology pipeline; the analytic
//! geometry is the source of truth consulted by inspection and exchange (D9).
//!
//! All geometry is stored in the primitive's local frame; world placement is
//! applied by `Brep::transformed`.

use openmaths::Vector3;
use serde::{Deserialize, Serialize};

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

/// Exact analytic geometry of an edge. `None` on an edge means a plain straight
/// segment between its endpoint vertices (the legacy/default interpretation,
/// equivalent to a [`CurveGeometry::Line`]).
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CurveGeometry {
    /// A straight segment. Endpoints are the edge's vertices; stored explicitly
    /// so the curve is self-describing for export.
    Line { start: Vector3, end: Vector3 },
    /// A circular arc (or full circle when `start_angle`/`end_angle` span 2π).
    /// The arc lies on the plane through `center` with the given `normal`;
    /// angle 0 points along `x_axis` and sweeps toward `x_axis × normal`.
    Circle {
        center: Vector3,
        normal: Vector3,
        x_axis: Vector3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
}

impl CurveGeometry {
    /// Whether this curve closes on itself (full circle).
    pub fn is_closed(&self) -> bool {
        match self {
            CurveGeometry::Line { .. } => false,
            CurveGeometry::Circle {
                start_angle,
                end_angle,
                ..
            } => (end_angle - start_angle).abs() >= TWO_PI - 1.0e-9,
        }
    }

    /// A short stable tag for inspection / serialization consumers.
    pub fn kind(&self) -> &'static str {
        match self {
            CurveGeometry::Line { .. } => "line",
            CurveGeometry::Circle { .. } => "circle",
        }
    }

    /// Samples the curve into `segments` straight chords. A `Line` always yields
    /// its two endpoints; a `Circle` yields `segments + 1` points (or `segments`
    /// distinct points for a full circle, the closing point omitted).
    pub fn tessellate(&self, segments: u32) -> Vec<Vector3> {
        match self {
            CurveGeometry::Line { start, end } => vec![*start, *end],
            CurveGeometry::Circle {
                center,
                normal,
                x_axis,
                radius,
                start_angle,
                end_angle,
            } => {
                let n = segments.max(1);
                let closed = self.is_closed();
                let x = normalize(*x_axis);
                // Second in-plane axis. `x_axis × normal` matches the sweep the
                // arc/cylinder primitives generate (x = r·cosθ on x_axis,
                // advancing toward this axis), so the analytic curve's
                // tessellation reproduces the stored facets exactly.
                let y = normalize(cross(x, *normal));
                let step = (end_angle - start_angle) / n as f64;
                let count = if closed { n } else { n + 1 };
                let mut points = Vec::with_capacity(count as usize);
                for i in 0..count {
                    let a = start_angle + step * i as f64;
                    let c = a.cos() * radius;
                    let s = a.sin() * radius;
                    points.push(Vector3::new(
                        center.x + x.x * c + y.x * s,
                        center.y + x.y * c + y.y * s,
                        center.z + x.z * c + y.z * s,
                    ));
                }
                points
            }
        }
    }
}

impl CurveGeometry {
    /// Applies a placement to this curve. `transform_point` maps a local point
    /// to world; `scale` is the placement's uniform scale factor (radii are
    /// lengths and must scale, directions are renormalized so scale drops out).
    pub fn transformed_with(
        &self,
        transform_point: &impl Fn(Vector3) -> Vector3,
        scale: f64,
    ) -> CurveGeometry {
        match self {
            CurveGeometry::Line { start, end } => CurveGeometry::Line {
                start: transform_point(*start),
                end: transform_point(*end),
            },
            CurveGeometry::Circle {
                center,
                normal,
                x_axis,
                radius,
                start_angle,
                end_angle,
            } => {
                let c = transform_point(*center);
                CurveGeometry::Circle {
                    center: c,
                    normal: transform_direction(transform_point, *center, *normal),
                    x_axis: transform_direction(transform_point, *center, *x_axis),
                    radius: radius * scale,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                }
            }
        }
    }
}

/// Maps a local direction to world by transforming two points and subtracting,
/// then renormalizing. Correct under rotation + translation + uniform scale.
fn transform_direction(
    transform_point: &impl Fn(Vector3) -> Vector3,
    base: Vector3,
    dir: Vector3,
) -> Vector3 {
    let p0 = transform_point(base);
    let p1 = transform_point(Vector3::new(base.x + dir.x, base.y + dir.y, base.z + dir.z));
    normalize(Vector3::new(p1.x - p0.x, p1.y - p0.y, p1.z - p0.z))
}

/// Exact analytic geometry of a face. `None` means a general planar polygon
/// whose plane is implied by its loop (the legacy/default interpretation).
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SurfaceGeometry {
    /// A planar face through `origin` with outward `normal`.
    Plane { origin: Vector3, normal: Vector3 },
    /// A (possibly partial) cylindrical face: the lateral surface swept by a
    /// circle of `radius` centred on `origin`, extruded along `axis` for
    /// `height`. `ref_direction` fixes angle 0 in the cross-section plane.
    Cylinder {
        origin: Vector3,
        axis: Vector3,
        ref_direction: Vector3,
        radius: f64,
        height: f64,
    },
}

impl SurfaceGeometry {
    pub fn kind(&self) -> &'static str {
        match self {
            SurfaceGeometry::Plane { .. } => "plane",
            SurfaceGeometry::Cylinder { .. } => "cylinder",
        }
    }

    pub fn transformed_with(
        &self,
        transform_point: &impl Fn(Vector3) -> Vector3,
        scale: f64,
    ) -> SurfaceGeometry {
        match self {
            SurfaceGeometry::Plane { origin, normal } => SurfaceGeometry::Plane {
                origin: transform_point(*origin),
                normal: transform_direction(transform_point, *origin, *normal),
            },
            SurfaceGeometry::Cylinder {
                origin,
                axis,
                ref_direction,
                radius,
                height,
            } => SurfaceGeometry::Cylinder {
                origin: transform_point(*origin),
                axis: transform_direction(transform_point, *origin, *axis),
                ref_direction: transform_direction(transform_point, *origin, *ref_direction),
                radius: radius * scale,
                height: height * scale,
            },
        }
    }
}

fn cross(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn normalize(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len <= f64::EPSILON {
        v
    } else {
        Vector3::new(v.x / len, v.y / len, v.z / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_circle(radius: f64) -> CurveGeometry {
        CurveGeometry::Circle {
            center: Vector3::new(0.0, 0.0, 0.0),
            normal: Vector3::new(0.0, 1.0, 0.0),
            x_axis: Vector3::new(1.0, 0.0, 0.0),
            radius,
            start_angle: 0.0,
            end_angle: TWO_PI,
        }
    }

    #[test]
    fn line_tessellates_to_endpoints() {
        let line = CurveGeometry::Line {
            start: Vector3::new(0.0, 0.0, 0.0),
            end: Vector3::new(1.0, 0.0, 0.0),
        };
        assert_eq!(line.tessellate(8).len(), 2);
        assert!(!line.is_closed());
    }

    #[test]
    fn full_circle_omits_closing_point() {
        let circle = full_circle(2.0);
        assert!(circle.is_closed());
        let pts = circle.tessellate(16);
        assert_eq!(pts.len(), 16);
        // Every sampled point lies exactly on the circle of radius 2.
        for p in &pts {
            let r = (p.x * p.x + p.z * p.z).sqrt();
            assert!((r - 2.0).abs() < 1.0e-12, "radius drift: {}", r);
        }
    }

    #[test]
    fn analytic_circle_is_exact_unlike_stored_facets() {
        // The whole point of D1: refining the tessellation recovers exactness
        // because the curve — not the facets — is the source of truth.
        let circle = full_circle(1.0);
        let coarse = circle.tessellate(4);
        let fine = circle.tessellate(256);
        // Coarse facets miss the true arc midpoint badly; fine ones do not.
        let coarse_mid_err = chord_sagitta_error(&coarse, 1.0);
        let fine_mid_err = chord_sagitta_error(&fine, 1.0);
        assert!(fine_mid_err < coarse_mid_err);
        assert!(fine_mid_err < 1.0e-4);
    }

    fn chord_sagitta_error(points: &[Vector3], radius: f64) -> f64 {
        // Max deviation of chord midpoints from the true radius.
        let mut max = 0.0_f64;
        for i in 0..points.len() {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let mx = (a.x + b.x) * 0.5;
            let mz = (a.z + b.z) * 0.5;
            let r = (mx * mx + mz * mz).sqrt();
            max = max.max((radius - r).abs());
        }
        max
    }

    #[test]
    fn arc_quarter_sweep_has_correct_endpoints() {
        let arc = CurveGeometry::Circle {
            center: Vector3::new(0.0, 0.0, 0.0),
            normal: Vector3::new(0.0, 1.0, 0.0),
            x_axis: Vector3::new(1.0, 0.0, 0.0),
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        let pts = arc.tessellate(8);
        assert_eq!(pts.len(), 9);
        let first = pts.first().unwrap();
        let last = pts.last().unwrap();
        assert!((first.x - 1.0).abs() < 1.0e-12 && first.z.abs() < 1.0e-12);
        assert!(last.x.abs() < 1.0e-12 && (last.z - 1.0).abs() < 1.0e-12);
    }
}
