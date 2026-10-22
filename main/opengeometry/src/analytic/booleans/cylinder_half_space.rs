//! Exact Boolean of a full finite cylinder with a cuboid that acts as a
//! half-space for it: exactly one cuboid face plane crosses the cylinder and
//! the other five faces lie clear of it. The kept part is bounded by the
//! trimmed cylindrical side, partial or full caps, and one planar cut face.
//! The cut curve on the side is an ellipse whose parameter is the cylinder
//! angle, or a pair of rulings when the plane is parallel to the axis.

use super::{
    analytic_face_mappings, box_source, cylinder_source, face_provenance, BooleanOp, BooleanReport,
    BooleanResult, CylinderInput,
};
use crate::analytic::{
    box_booleans::BoxInput,
    geometry::{add, cross, dot, norm, scale, sub, unit},
    primitives::{boundary, uv_line, Builder, Use},
    topology::*,
    CurveGeometry, Frame3, GeometryError, Point3, SurfaceGeometry,
};
use crate::math::interval::Interval;
use std::f64::consts::{PI, TAU};
use Orientation::{Forward as F, Reverse as R};

fn gap() -> GeometryError {
    GeometryError::CoverageGap {
        families: ["cylinder".into(), "half-space cuboid".into()],
    }
}

fn plane_uv(frame: Frame3, p: Point3) -> [f64; 2] {
    let local = frame.local(p);
    [local[0], local[1]]
}

fn line_pcurve(frame: Frame3, origin: Point3, direction: Point3) -> PcurveGeometry {
    uv_line(
        plane_uv(frame, origin),
        [dot(direction, frame.x), dot(direction, frame.y)],
    )
}

/// A straight edge from `from` to `to`, parameterized by arc length.
fn segment(builder: &mut Builder, a: Point3, b: Point3) -> Result<(u32, Point3), GeometryError> {
    let direction = unit(sub(b, a))?;
    let edge = builder.edge(
        CurveGeometry::Line {
            origin: a,
            direction,
        },
        Interval::new(0.0, norm(sub(b, a)))?,
        false,
    );
    Ok((edge, direction))
}

fn curve_of(builder: &Builder, edge: u32) -> u32 {
    match builder.brep.topology.edges[edge as usize].geometry {
        EdgeGeometry::Curve { curve, .. } => curve,
        EdgeGeometry::Collapsed { .. } => u32::MAX,
    }
}

/// Subtracts a half-space cuboid from a cylinder, or intersects them. Any
/// other configuration returns a coverage gap so callers keep their own
/// fallbacks.
pub(super) fn cylinder_half_space_boolean(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    if !(operation == BooleanOp::Intersection
        || (operation == BooleanOp::Subtraction && cylinder_is_a))
    {
        return Err(gap());
    }
    // A subtraction with the axis along a cuboid axis is the circular or
    // ruled cut that the aligned cylinder/cuboid builders already own.
    if operation == BooleanOp::Subtraction
        && [box_.frame.x, box_.frame.y, box_.frame.z]
            .iter()
            .any(|axis| 1.0 - dot(cylinder.frame.z, *axis).abs() <= 1.0e-12)
    {
        return Err(gap());
    }
    let clearance = 4.0 * accuracy.geometric;
    let (radius, height) = (cylinder.radius, cylinder.height);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let local_origin = box_.frame.local(cylinder.frame.origin);
    let mut cut = None;
    for axis in 0..3 {
        let rate = dot(cylinder.frame.z, box_axes[axis]);
        let radial = radius * (1.0 - rate * rate).max(0.0).sqrt();
        let other = local_origin[axis] + rate * height;
        let lo = local_origin[axis].min(other) - radial;
        let hi = local_origin[axis].max(other) + radial;
        for (high, margin, beyond) in [
            (false, lo, hi),
            (true, box_.size[axis] - hi, box_.size[axis] - lo),
        ] {
            if margin.abs() <= clearance {
                return Err(gap());
            }
            if margin < 0.0 {
                if beyond <= clearance || cut.is_some() {
                    return Err(gap());
                }
                cut = Some((axis, high));
            }
        }
    }
    let (axis, high) = cut.ok_or_else(gap)?;
    let box_face = [2, 4, 0][axis] + usize::from(high);
    let mut plane_point = [0.0; 3];
    plane_point[axis] = if high { box_.size[axis] } else { 0.0 };
    let plane_point = box_.frame.point(plane_point);
    let box_outward = scale(box_axes[axis], if high { 1.0 } else { -1.0 });
    // `normal` points into the kept material: away from the cuboid for a
    // subtraction, into it for an intersection.
    let normal = if operation == BooleanOp::Subtraction {
        box_outward
    } else {
        scale(box_outward, -1.0)
    };

    // Re-chart the cylinder so its axis leans into the kept side and the
    // chart angle zero is where the cut plane rises highest.
    let mut origin = cylinder.frame.origin;
    let mut z = cylinder.frame.z;
    let mut flipped = false;
    let mut c = dot(normal, z);
    if c < 0.0 {
        origin = add(origin, scale(z, height));
        z = scale(z, -1.0);
        c = -c;
        flipped = true;
    }
    let perpendicular = sub(normal, scale(z, c));
    let a = norm(perpendicular);
    if a <= 1.0e-12 {
        return Err(gap());
    }
    let x = scale(perpendicular, -1.0 / a);
    let y = unit(cross(z, x))?;
    let frame = Frame3 { origin, x, y, z };
    frame.validate()?;
    let e = dot(normal, sub(plane_point, origin));
    let point = |theta: f64, w: f64| frame.point([radius * theta.cos(), radius * theta.sin(), w]);
    let circle = |w: f64| CurveGeometry::Circle {
        frame: Frame3 {
            origin: frame.point([0.0, 0.0, w]),
            ..frame
        },
        radius,
    };
    let bottom_face = if flipped { 2 } else { 1 };
    let top_face = if flipped { 1 } else { 2 };
    let cap_bottom = Frame3 {
        origin,
        x,
        y: scale(y, -1.0),
        z: scale(z, -1.0),
    };
    let cap_top = Frame3 {
        origin: frame.point([0.0, 0.0, height]),
        ..frame
    };
    let pad = accuracy.geometric;
    let mut builder = Builder::new(id, accuracy)?;

    // Side face uses (counterclockwise in angle/height), cap and cut uses.
    let side: Vec<Use>;
    let mut bottom: Option<Vec<Use>> = None;
    let top: Vec<Use>;
    let cut_face: Vec<Use>;
    let cut_frame: Frame3;
    let cut_bounds: [[f64; 2]; 2];
    let side_bounds: [[f64; 2]; 2];
    let top_partial;

    if height * c <= 0.25 * accuracy.geometric {
        // Plane parallel to the axis: two rulings bound the kept band.
        let u0 = -e / a;
        if u0.abs() >= radius - clearance {
            return Err(gap());
        }
        let t1 = (u0 / radius).acos();
        let t2 = TAU - t1;
        let half = radius * t1.sin();
        let corners = [
            point(t1, 0.0),
            point(t2, 0.0),
            point(t1, height),
            point(t2, height),
        ];
        let [vb1, vb2, vt1, vt2] = corners.map(|p| builder.vertex(p));
        let [pb1, pb2, pt1, pt2] = corners;
        let range = Interval::new(t1, t2)?;
        let bottom_arc = builder.edge(circle(0.0), range, false);
        let top_arc = builder.edge(circle(height), range, false);
        let (l1, _) = segment(&mut builder, pb1, pt1)?;
        let (l2, _) = segment(&mut builder, pb2, pt2)?;
        let (cb, cb_dir) = segment(&mut builder, pb1, pb2)?;
        let (ct, ct_dir) = segment(&mut builder, pt2, pt1)?;
        cut_frame = Frame3 {
            origin: frame.point([u0, 0.0, 0.0]),
            x: z,
            y: scale(y, -1.0),
            z: x,
        };
        side = vec![
            boundary(bottom_arc, vb1, vb2, F, uv_line([0.0, 0.0], [1.0, 0.0])),
            boundary(l2, vb2, vt2, F, uv_line([t2, 0.0], [0.0, 1.0])),
            boundary(top_arc, vt2, vt1, R, uv_line([0.0, height], [1.0, 0.0])),
            boundary(l1, vt1, vb1, R, uv_line([t1, 0.0], [0.0, 1.0])),
        ];
        side_bounds = [[t1, t2], [0.0, height]];
        bottom = Some(vec![
            boundary(bottom_arc, vb2, vb1, R, conic(radius, -radius)),
            boundary(cb, vb1, vb2, F, line_pcurve(cap_bottom, pb1, cb_dir)),
        ]);
        top = vec![
            boundary(top_arc, vt1, vt2, F, conic(radius, radius)),
            boundary(ct, vt2, vt1, F, line_pcurve(cap_top, pt2, ct_dir)),
        ];
        top_partial = true;
        cut_face = vec![
            boundary(l2, vt2, vb2, R, line_pcurve(cut_frame, pb2, z)),
            boundary(cb, vb2, vb1, R, line_pcurve(cut_frame, pb1, cb_dir)),
            boundary(l1, vb1, vt1, F, line_pcurve(cut_frame, pb1, z)),
            boundary(ct, vt1, vt2, R, line_pcurve(cut_frame, pt2, ct_dir)),
        ];
        cut_bounds = [[-pad, height + pad], [-half - pad, half + pad]];
    } else {
        if c < 1.0e-7 {
            return Err(GeometryError::UnresolvedIntersection(
                "cut plane is nearly parallel to the cylinder axis".into(),
            ));
        }
        // Kept side: w >= f(theta) = m + k cos(theta).
        let k = a * radius / c;
        let m = e / c;
        let (fmin, fmax) = (m - k, m + k);
        if fmax <= clearance || fmin >= height - clearance {
            return Err(gap());
        }
        if (fmax - height).abs() <= clearance || fmin.abs() <= clearance {
            return Err(GeometryError::UnresolvedIntersection(
                "cut ellipse meets a cylinder rim within geometric resolution".into(),
            ));
        }
        let cuts_top = fmax > height;
        let cuts_bottom = fmin < 0.0;
        let major = radius.hypot(k);
        let g = unit(add(scale(x, radius), scale(z, k)))?;
        let center = frame.point([0.0, 0.0, m]);
        let ellipse = CurveGeometry::Ellipse {
            frame: Frame3 {
                origin: center,
                x: g,
                y,
                z: unit(cross(g, y))?,
            },
            major_radius: major,
            minor_radius: radius,
        };
        cut_frame = Frame3 {
            origin: center,
            x: g,
            y: scale(y, -1.0),
            z: unit(cross(y, g))?,
        };
        cut_frame.validate()?;
        cut_bounds = [[-major - pad, major + pad], [-radius - pad, radius + pad]];
        let ellipse_on_cut = PcurveGeometry::Conic2 {
            origin: [0.0; 2],
            axis_a: [major, 0.0],
            axis_b: [0.0, -radius],
        };
        let t_top = if cuts_top {
            ((height - m) / k).clamp(-1.0, 1.0).acos()
        } else {
            0.0
        };
        let t_bottom = if cuts_bottom {
            (-m / k).clamp(-1.0, 1.0).acos()
        } else {
            PI
        };
        if cuts_top && cuts_bottom && t_bottom - t_top <= 1.0e-9 {
            return Err(GeometryError::UnresolvedIntersection(
                "cut ellipse arc is below angular resolution".into(),
            ));
        }
        let side_surface = 0u32;
        let ellipse_edge = |builder: &mut Builder,
                            lo: f64,
                            hi: f64|
         -> Result<(u32, PcurveGeometry), GeometryError> {
            let edge = builder.edge(ellipse.clone(), Interval::new(lo, hi)?, false);
            let pcurve = PcurveGeometry::ProjectedCurve {
                curve: curve_of(builder, edge),
                surface: side_surface,
                chart: 0,
                uv_hint: [lo, m + k * lo.cos()],
                uv_rate: [1.0, -k * lo.sin()],
                parameter_origin: lo,
            };
            Ok((edge, pcurve))
        };
        let mut side_uses = Vec::new();
        let mut cut_uses = Vec::new();
        let (start, end, seam) = if cuts_top {
            let vt1 = builder.vertex(point(t_top, height));
            let vt2 = builder.vertex(point(TAU - t_top, height));
            (vt1, vt2, None)
        } else {
            let v0 = builder.vertex(point(0.0, fmax));
            (v0, v0, Some(builder.vertex(point(0.0, height))))
        };
        let (lo, hi) = (t_top, TAU - t_top);
        if cuts_bottom {
            let vb1 = builder.vertex(point(t_bottom, 0.0));
            let vb2 = builder.vertex(point(TAU - t_bottom, 0.0));
            let (e1, p1) = ellipse_edge(&mut builder, lo, t_bottom)?;
            let arc = builder.edge(circle(0.0), Interval::new(t_bottom, TAU - t_bottom)?, false);
            let (e2, p2) = ellipse_edge(&mut builder, TAU - t_bottom, hi)?;
            let positions = |v: u32| builder.brep.topology.vertices[v as usize].position;
            let (vb1_p, vb2_p) = (positions(vb1), positions(vb2));
            let (cb, cb_dir) = segment(&mut builder, vb1_p, vb2_p)?;
            side_uses.push(boundary(e1, start, vb1, F, p1));
            side_uses.push(boundary(arc, vb1, vb2, F, uv_line([0.0, 0.0], [1.0, 0.0])));
            side_uses.push(boundary(e2, vb2, end, F, p2));
            bottom = Some(vec![
                boundary(arc, vb2, vb1, R, conic(radius, -radius)),
                boundary(cb, vb1, vb2, F, line_pcurve(cap_bottom, vb1_p, cb_dir)),
            ]);
            cut_uses.push(boundary(e2, end, vb2, R, ellipse_on_cut.clone()));
            cut_uses.push(boundary(
                cb,
                vb2,
                vb1,
                R,
                line_pcurve(cut_frame, vb1_p, cb_dir),
            ));
            cut_uses.push(boundary(e1, vb1, start, R, ellipse_on_cut.clone()));
        } else {
            let (edge, pcurve) = ellipse_edge(&mut builder, lo, hi)?;
            side_uses.push(boundary(edge, start, end, F, pcurve));
            cut_uses.push(boundary(edge, end, start, R, ellipse_on_cut.clone()));
        }
        let positions = |b: &Builder, v: u32| b.brep.topology.vertices[v as usize].position;
        if let Some(vs) = seam {
            let top_circle = builder.edge(circle(height), Interval::new(0.0, TAU)?, false);
            let seam_edge = builder.edge(
                CurveGeometry::Line {
                    origin: positions(&builder, start),
                    direction: z,
                },
                Interval::new(0.0, height - fmax)?,
                true,
            );
            side_uses.push(boundary(
                seam_edge,
                start,
                vs,
                F,
                uv_line([TAU, fmax], [0.0, 1.0]),
            ));
            side_uses.push(boundary(
                top_circle,
                vs,
                vs,
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ));
            side_uses.push(boundary(
                seam_edge,
                vs,
                start,
                R,
                uv_line([0.0, fmax], [0.0, 1.0]),
            ));
            top = vec![boundary(top_circle, vs, vs, F, conic(radius, radius))];
            top_partial = false;
        } else {
            let arc = builder.edge(circle(height), Interval::new(lo, hi)?, false);
            let (start_p, end_p) = (positions(&builder, start), positions(&builder, end));
            let (ct, ct_dir) = segment(&mut builder, end_p, start_p)?;
            side_uses.push(boundary(
                arc,
                end,
                start,
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ));
            top = vec![
                boundary(arc, start, end, F, conic(radius, radius)),
                boundary(ct, end, start, F, line_pcurve(cap_top, end_p, ct_dir)),
            ];
            cut_uses.push(boundary(
                ct,
                start,
                end,
                R,
                line_pcurve(cut_frame, end_p, ct_dir),
            ));
            top_partial = true;
        }
        side = side_uses;
        cut_face = cut_uses;
        side_bounds = [[lo, hi], [fmin.max(0.0), height]];
    }

    // The side surface is the first surface so projected pcurves can name it.
    builder.face(
        "0:cylinder-side",
        SurfaceGeometry::Cylinder { frame, radius },
        [
            [side_bounds[0][0], side_bounds[0][1]],
            [side_bounds[1][0] - pad, side_bounds[1][1] + pad],
        ],
        side,
    )?;
    builder.brep.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false);
    let disk = [[-radius - pad, radius + pad]; 2];
    if let Some(uses) = bottom {
        let face = builder.brep.topology.faces.len();
        builder.face(
            &format!("{face}:cylinder-cap"),
            SurfaceGeometry::Plane { frame: cap_bottom },
            disk,
            uses,
        )?;
        builder.brep.topology.faces[face].provenance = face_provenance(
            vec![cylinder_source(cylinder, bottom_face)],
            FaceRole::Split,
            false,
        );
    }
    let face = builder.brep.topology.faces.len();
    builder.face(
        &format!("{face}:cylinder-cap"),
        SurfaceGeometry::Plane { frame: cap_top },
        disk,
        top,
    )?;
    builder.brep.topology.faces[face].provenance = face_provenance(
        vec![cylinder_source(cylinder, top_face)],
        if top_partial {
            FaceRole::Split
        } else {
            FaceRole::Preserved
        },
        false,
    );
    let face = builder.brep.topology.faces.len();
    builder.face(
        &format!("{face}:half-space-cut"),
        SurfaceGeometry::Plane { frame: cut_frame },
        cut_bounds,
        cut_face,
    )?;
    builder.brep.topology.faces[face].provenance = face_provenance(
        vec![box_source(box_, box_face)],
        FaceRole::Cut,
        operation == BooleanOp::Subtraction,
    );
    let mut out = builder.finish()?;
    out.revision = box_
        .brep
        .revision
        .max(cylinder.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let inputs = if cylinder_is_a {
        [cylinder.brep, box_.brep]
    } else {
        [box_.brep, cylinder.brep]
    };
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    })
}

fn conic(a: f64, b: f64) -> PcurveGeometry {
    PcurveGeometry::Conic2 {
        origin: [0.0; 2],
        axis_a: [a, 0.0],
        axis_b: [0.0, b],
    }
}

#[cfg(test)]
mod tests {
    use crate::analytic::{
        booleans::{boolean_brep, BooleanOp},
        geometry::{cross, dot, norm, scale, sub, unit},
        primitives,
        tessellation::tessellate,
        topology::*,
        CurveGeometry, Frame3, Point3, SurfaceGeometry,
    };
    use std::f64::consts::PI;

    const R: f64 = 0.5;

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 0.01,
            exchange: 1e-5,
        }
    }

    struct Cyl {
        origin: Point3,
        axis: Point3,
        height: f64,
    }

    fn cylinder(c: &Cyl) -> BrepEnvelope {
        let reference = if c.axis[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let frame = Frame3::from_axis(c.origin, c.axis, reference).unwrap();
        primitives::cylinder("cyl".into(), frame, R, c.height, accuracy()).unwrap()
    }

    /// A large cuboid whose low face lies on the plane through `point`; it
    /// fills the side that `normal` points to.
    fn half_space(point: Point3, normal: Point3, reference: Point3) -> BrepEnvelope {
        let frame = Frame3::from_axis([0.0; 3], normal, reference).unwrap();
        let origin = sub(sub(point, scale(frame.x, 50.0)), scale(frame.y, 50.0));
        primitives::cuboid(
            "half".into(),
            Frame3 { origin, ..frame },
            [100.0, 100.0, 50.0],
            accuracy(),
        )
        .unwrap()
    }

    fn volume(brep: &BrepEnvelope) -> f64 {
        let mesh = tessellate(brep, 0.0005, 4_000_000).unwrap();
        mesh.indices
            .chunks_exact(3)
            .map(|ids| {
                let p: [Point3; 3] = std::array::from_fn(|i| {
                    let s = ids[i] as usize * 3;
                    [
                        mesh.positions[s],
                        mesh.positions[s + 1],
                        mesh.positions[s + 2],
                    ]
                });
                dot(p[0], cross(p[1], p[2])) / 6.0
            })
            .sum()
    }

    /// Volume of the cylinder part with `normal . (x - point) <= 0`.
    fn kept_volume(c: &Cyl, point: Point3, normal: Point3) -> f64 {
        let z = unit(c.axis).unwrap();
        let rate = dot(normal, z);
        let rho = norm(sub(normal, scale(z, rate)));
        let g0 = dot(normal, sub(c.origin, point));
        let length = |u: f64| {
            let rhs = -(g0 + rho * u);
            if rate.abs() < 1e-14 {
                if rhs >= 0.0 {
                    c.height
                } else {
                    0.0
                }
            } else if rate > 0.0 {
                (rhs / rate).clamp(0.0, c.height)
            } else {
                c.height - (rhs / rate).clamp(0.0, c.height)
            }
        };
        let n = 400_000;
        let step = PI / n as f64;
        (0..=n)
            .map(|i| {
                let phi = i as f64 * step;
                let weight = if i == 0 || i == n {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                weight * 2.0 * R * R * phi.sin().powi(2) * length(R * phi.cos())
            })
            .sum::<f64>()
            * step
            / 3.0
    }

    /// Exact area of the planar section, clipped by the caps.
    fn section_area(c: &Cyl, point: Point3, normal: Point3) -> f64 {
        let z = unit(c.axis).unwrap();
        let rate = dot(normal, z).abs();
        let sine = (1.0 - rate * rate).max(0.0).sqrt();
        if rate < 1e-14 {
            let d = dot(normal, sub(point, c.origin)).abs();
            return c.height * 2.0 * (R * R - d * d).sqrt();
        }
        let major = R / rate;
        let center = dot(normal, sub(point, c.origin)) / dot(normal, z);
        let f = |x: f64| {
            let x = x.clamp(-1.0, 1.0);
            x * (1.0 - x * x).sqrt() + x.asin()
        };
        if sine < 1e-14 {
            return PI * R * R;
        }
        let s1 = -center / sine;
        let s2 = (c.height - center) / sine;
        R * major * (f(s2 / major) - f(s1 / major))
    }

    fn face_area(brep: &BrepEnvelope, face: &Face) -> f64 {
        let mut points = Vec::new();
        let start = brep.topology.loops[face.trim.outer as usize].start_halfedge;
        let mut h = start;
        loop {
            let half = &brep.topology.halfedges[h as usize];
            let range = brep.topology.edges[half.edge as usize].geometry.range();
            let pcurve = half.geometry_use.pcurve.unwrap();
            let samples = 20_000;
            for i in 0..samples {
                let s = i as f64 / samples as f64;
                let t = if half.geometry_use.sense == Orientation::Forward {
                    range.lo + (range.hi - range.lo) * s
                } else {
                    range.hi - (range.hi - range.lo) * s
                };
                points.push(brep.geometry.pcurve_at(pcurve, t).unwrap());
            }
            h = half.next.unwrap();
            if h == start {
                break;
            }
        }
        let n = points.len();
        (0..n)
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % n]);
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f64>()
            / 2.0
    }

    /// Runs subtraction and intersection; checks closure, face count, cut
    /// face area and both volumes.
    fn check(c: Cyl, point: Point3, normal: Point3, reference: Point3, faces: [usize; 2]) {
        let host = cylinder(&c);
        let cutter = half_space(point, normal, reference);
        let full = PI * R * R * c.height;
        let removed = kept_volume(&c, point, scale(normal, -1.0));
        let expected_area = section_area(&c, point, normal);
        for (index, (operation, expected)) in [
            (BooleanOp::Subtraction, kept_volume(&c, point, normal)),
            (BooleanOp::Intersection, removed),
        ]
        .into_iter()
        .enumerate()
        {
            let result = boolean_brep(&host, &cutter, operation, "out".into())
                .unwrap_or_else(|error| panic!("{operation:?}: {error:?}"));
            let brep = result.brep;
            brep.validate().unwrap();
            assert_eq!(brep.topology.shells.len(), 1);
            assert!(brep.topology.shells[0].is_closed);
            assert!(brep
                .topology
                .edges
                .iter()
                .all(|edge| edge.twin_halfedge.is_some()));
            assert_eq!(
                brep.topology.faces.len(),
                faces[index],
                "{operation:?} face count: {:?}",
                brep.topology
                    .faces
                    .iter()
                    .map(|face| (&face.key, face.provenance.role))
                    .collect::<Vec<_>>()
            );
            let measured = volume(&brep);
            assert!(measured > 0.0, "{operation:?}: shell must face outward");
            assert!(
                (measured - expected).abs() <= 1e-3 * full,
                "{operation:?}: volume {measured} expected {expected}"
            );
            let cut = brep
                .topology
                .faces
                .iter()
                .filter(
                    |face| match &brep.geometry.surfaces[face.surface as usize] {
                        SurfaceGeometry::Plane { frame } => {
                            1.0 - dot(frame.z, normal).abs() < 1e-12
                                && dot(normal, sub(frame.origin, point)).abs() < 1e-9
                        }
                        _ => false,
                    },
                )
                .collect::<Vec<_>>();
            assert_eq!(cut.len(), 1, "{operation:?}: one cut face");
            let rate = dot(unit(c.axis).unwrap(), normal).abs();
            if rate > 1e-12 && rate < 1.0 - 1e-12 {
                assert!(brep.geometry.curves.iter().any(|curve| matches!(
                    curve,
                    CurveGeometry::Ellipse { major_radius, minor_radius, .. }
                        if (major_radius - R / rate).abs() < 1e-9 && (minor_radius - R).abs() < 1e-12
                )));
            }
            BrepEnvelope::from_json(&brep.to_json().unwrap()).unwrap();
            let area = face_area(&brep, cut[0]).abs();
            assert!(
                (area - expected_area).abs() <= 1e-6 * expected_area,
                "{operation:?}: cut area {area} expected {expected_area}"
            );
        }
    }

    fn sloped(degrees: f64, height: f64) -> Cyl {
        let a = degrees.to_radians();
        Cyl {
            origin: [0.3, -0.2, 1.0],
            axis: [a.cos(), 0.0, a.sin()],
            height,
        }
    }

    fn along_axis(c: &Cyl, t: f64) -> Point3 {
        crate::analytic::geometry::add(c.origin, scale(unit(c.axis).unwrap(), t))
    }

    fn horizontal() -> Cyl {
        Cyl {
            origin: [0.0, 0.0, 1.0],
            axis: [1.0, 0.0, 0.0],
            height: 6.0,
        }
    }

    #[test]
    fn horizontal_cylinder_across_keeps_the_circular_cut() {
        check(
            horizontal(),
            [2.5, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [3, 3],
        );
    }

    #[test]
    fn horizontal_cylinder_along_keeps_the_ruled_cut() {
        check(
            horizontal(),
            [0.0, 0.2, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [6, 4],
        );
    }

    #[test]
    fn horizontal_cylinder_oblique_cut_is_an_exact_ellipse() {
        let a = 30f64.to_radians();
        let normal = [-a.sin(), a.cos(), 0.0];
        // Full ellipse on the side, then an ellipse clipped by an end cap.
        check(
            horizontal(),
            [3.0, 0.0, 1.0],
            normal,
            [0.0, 0.0, 1.0],
            [3, 3],
        );
        check(
            horizontal(),
            [5.6, 0.0, 1.0],
            normal,
            [0.0, 0.0, 1.0],
            [3, 4],
        );
        check(
            horizontal(),
            [0.4, 0.0, 1.0],
            normal,
            [0.0, 0.0, 1.0],
            [4, 3],
        );
    }

    #[test]
    fn sloped_cylinder_across_along_and_oblique_cuts() {
        let b = 30f64.to_radians();
        for degrees in [15.0, 40.0] {
            let c = sloped(degrees, 6.0);
            let across = [1.0, 0.0, 0.0];
            let oblique = [b.cos(), b.sin(), 0.0];
            let world_y = [0.0, 1.0, 0.0];
            check(
                sloped(degrees, 6.0),
                along_axis(&c, 3.0),
                across,
                world_y,
                [3, 3],
            );
            check(
                sloped(degrees, 6.0),
                along_axis(&c, 3.0),
                oblique,
                [0.0, 0.0, 1.0],
                [3, 3],
            );
            // Vertical plane parallel to the sloped axis: a pair of rulings.
            check(
                sloped(degrees, 6.0),
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [4, 4],
            );
        }
        // Ellipses clipped by the low cap, the high cap and both caps.
        let c = sloped(40.0, 6.0);
        check(
            sloped(40.0, 6.0),
            along_axis(&c, 0.2),
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [3, 4],
        );
        check(
            sloped(40.0, 6.0),
            along_axis(&c, 5.8),
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [4, 3],
        );
        let c = sloped(40.0, 0.3);
        check(
            sloped(40.0, 0.3),
            along_axis(&c, 0.15),
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [4, 4],
        );
    }

    #[test]
    fn vertical_cylinder_cuts_are_unchanged() {
        let vertical = || Cyl {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            height: 6.0,
        };
        check(
            vertical(),
            [0.0, 0.0, 2.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [3, 3],
        );
        check(
            vertical(),
            [0.2, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [5, 4],
        );
    }
}
