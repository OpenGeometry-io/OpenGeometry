use super::curve::CurveGeometry;
use super::surface::{Surface, SurfaceGeometry};
use super::wrap::Wrap;
use crate::brep::error::GeometryError;
use crate::brep::frame::Frame3;
use crate::math::{dot, norm, scale, sub, Interval};

fn surfaces() -> Vec<SurfaceGeometry> {
    let frame = Frame3::from_axis([3.0, -2.0, 7.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap();
    vec![
        SurfaceGeometry::Plane { frame },
        SurfaceGeometry::Sphere { frame, radius: 2.0 },
        SurfaceGeometry::Cylinder { frame, radius: 2.0 },
        SurfaceGeometry::Cone {
            frame,
            semi_angle: 0.4,
        },
        SurfaceGeometry::Torus {
            frame,
            major_radius: 3.0,
            minor_radius: 1.0,
        },
        SurfaceGeometry::Wrapped {
            wrap: wrap(),
            surface: Box::new(SurfaceGeometry::Cylinder {
                frame: Frame3 {
                    origin: [1.0, 0.5, 3.0],
                    ..Frame3::IDENTITY
                },
                radius: 0.8,
            }),
        },
        SurfaceGeometry::Wrapped {
            wrap: wrap(),
            surface: Box::new(SurfaceGeometry::Plane {
                frame: Frame3::from_axis([0.3, -0.2, 2.5], [1.0, 2.0, 0.3], [0.0, 0.0, 1.0])
                    .unwrap(),
            }),
        },
    ]
}

fn wrap() -> Wrap {
    Wrap {
        frame: Frame3::from_axis([3.0, -2.0, 7.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap(),
        radius: 2.6,
    }
}

#[test]
fn a_wrap_keeps_arc_length_on_its_radius_and_round_trips() {
    let w = wrap();
    let p = [1.3, -0.4, 2.6];
    let chord = norm(sub(w.point(p), w.point([0.0, -0.4, 2.6])));
    assert!((chord - 2.0 * 2.6 * (1.3_f64 / 5.2).sin()).abs() < 1e-12);
    let back = w.flatten(w.point(p), None).unwrap();
    assert!(norm(sub(back, p)) < 1e-12);
    let far = [2.6 * 4.0, 0.0, 3.0];
    assert!(norm(sub(w.flatten(w.point(far), Some(far[0])).unwrap(), far)) < 1e-12);
    let curve = CurveGeometry::Wrapped {
        wrap: w,
        curve: Box::new(CurveGeometry::Line {
            origin: [0.0, 0.0, 2.0],
            direction: [0.6, 0.8, 0.0],
        }),
    };
    let speed = |t: f64| norm(curve.elementary_tangent(t).unwrap());
    assert!((speed(0.1) - speed(3.0)).abs() < 1e-12);
    let h = 1e-6;
    let fd = scale(
        sub(
            curve.elementary_point(1.0 + h).unwrap(),
            curve.elementary_point(1.0 - h).unwrap(),
        ),
        0.5 / h,
    );
    assert!(norm(sub(fd, curve.elementary_tangent(1.0).unwrap())) < 1e-8);
    let bounds = curve
        .elementary_bounds(Interval::new(0.0, 4.0).unwrap())
        .unwrap();
    for i in 0..=40 {
        assert!(bounds.contains(curve.elementary_point(i as f64 / 10.0).unwrap()));
    }
    let nested = CurveGeometry::Wrapped {
        wrap: w,
        curve: Box::new(curve),
    };
    assert!(nested.validate().is_err());
    let nested_surface = SurfaceGeometry::Wrapped {
        wrap: w,
        surface: Box::new(surfaces().pop().unwrap()),
    };
    assert!(nested_surface.validate().is_err());
}

#[test]
fn wrapped_circle_samples_meet_their_chord_error() {
    let curve = CurveGeometry::Wrapped {
        wrap: wrap(),
        curve: Box::new(CurveGeometry::Circle {
            frame: Frame3 {
                origin: [1.0, 0.5, 3.0],
                ..Frame3::IDENTITY
            },
            radius: 0.8,
        }),
    };
    let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
    let samples = curve.sample_elementary(range, 1e-3, 100_000).unwrap();
    let n = samples.len() - 1;
    for i in 0..n {
        for k in 1..8 {
            let t = range.width() * (i as f64 + k as f64 / 8.0) / n as f64;
            let p = curve.elementary_point(t).unwrap();
            let (a, b) = (samples[i], samples[i + 1]);
            let ab = sub(b, a);
            let along = dot(sub(p, a), ab) / dot(ab, ab);
            let foot = [0, 1, 2].map(|j| a[j] + along * ab[j]);
            assert!(norm(sub(p, foot)) <= 1e-3);
        }
    }
}

#[test]
fn derivatives_projection_and_normals_for_all_families() {
    let uv = [0.7, 0.4];
    let h = 1e-5;
    for surface in surfaces() {
        let jet = surface.derivatives(uv).unwrap();
        let n = surface.normal_at(uv).unwrap();
        assert!((norm(n) - 1.0).abs() < 1e-12);
        assert!(dot(n, jet.du).abs() < 1e-12);
        assert!(dot(n, jet.dv).abs() < 1e-12);
        let projected = surface.project(jet.point, Some(uv)).unwrap();
        assert!((projected[0] - uv[0]).abs() < 1e-12);
        assert!((projected[1] - uv[1]).abs() < 1e-12);
        for dimension in 0..2 {
            let mut lo = uv;
            let mut hi = uv;
            lo[dimension] -= h;
            hi[dimension] += h;
            let jl = surface.derivatives(lo).unwrap();
            let jh = surface.derivatives(hi).unwrap();
            let derivative = scale(sub(jh.point, jl.point), 0.5 / h);
            assert!(
                norm(sub(
                    derivative,
                    if dimension == 0 { jet.du } else { jet.dv }
                )) < 1e-8
            );
            assert!(
                norm(sub(
                    scale(sub(jh.du, jl.du), 0.5 / h),
                    if dimension == 0 { jet.duu } else { jet.duv }
                )) < 1e-8
            );
            assert!(
                norm(sub(
                    scale(sub(jh.dv, jl.dv), 0.5 / h),
                    if dimension == 0 { jet.duv } else { jet.dvv }
                )) < 1e-8
            );
        }
    }
}

#[test]
fn conservative_boxes_cover_transformed_patches() {
    let box_uv = [
        Interval::new(0.2, 1.8).unwrap(),
        Interval::new(0.1, 0.9).unwrap(),
    ];
    for surface in surfaces() {
        let bounds = surface.enclose(box_uv).unwrap();
        for i in 0..=24 {
            for j in 0..=24 {
                let uv = [0.2 + 1.6 * i as f64 / 24.0, 0.1 + 0.8 * j as f64 / 24.0];
                assert!(bounds.contains(surface.point_at(uv).unwrap()));
            }
        }
    }
}

#[test]
fn circle_samples_meet_chord_error_at_multiple_lods() {
    let curve = CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 2.0,
    };
    let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
    for epsilon in [0.1, 0.01, 0.001] {
        let samples = curve.sample_elementary(range, epsilon, 100_000).unwrap();
        let n = samples.len() - 1;
        let sagitta = 2.0 * (1.0 - (std::f64::consts::PI / n as f64).cos());
        assert!(sagitta <= epsilon);
        assert!(norm(sub(samples[0], samples[n])) < 1e-12);
    }
}

#[test]
fn invalid_frames_tori_and_apices_fail_explicitly() {
    Frame3::from_axis([0.0; 3], [0.0, 0.0, 1e-320], [1.0, 0.0, 0.0])
        .unwrap()
        .validate()
        .unwrap();
    let mut bad = Frame3::IDENTITY;
    bad.y = [0.0, -1.0, 0.0];
    assert!(bad.validate().is_err());
    assert!(SurfaceGeometry::Torus {
        frame: Frame3::IDENTITY,
        major_radius: 1.0,
        minor_radius: 1.0
    }
    .validate()
    .is_err());
    let cone = SurfaceGeometry::Cone {
        frame: Frame3::IDENTITY,
        semi_angle: 0.3,
    };
    assert_eq!(
        cone.normal_at([0.0, 0.0]),
        Err(GeometryError::SingularParameterization)
    );
    assert!(CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 1.0
    }
    .sample_elementary(Interval::new(0.0, 1.0).unwrap(), 0.0, 100)
    .is_err());
}

#[test]
fn partial_cylinder_and_cone_patches_on_a_skew_frame_get_their_exact_box() {
    let box_uv = [
        Interval::new(0.2, 1.8).unwrap(),
        Interval::new(0.1, 0.9).unwrap(),
    ];
    for surface in surfaces().into_iter().skip(2).take(2) {
        let frame = *surface.frame();
        let mut angles = vec![box_uv[0].lo, box_uv[0].hi];
        for i in 0..3 {
            for k in -2..=2 {
                let u = frame.y[i].atan2(frame.x[i]) + k as f64 * std::f64::consts::PI;
                if box_uv[0].contains(u) {
                    angles.push(u);
                }
            }
        }
        let mut expected = [[f64::INFINITY, -f64::INFINITY]; 3];
        for &u in &angles {
            for v in [box_uv[1].lo, box_uv[1].hi] {
                let p = surface.point_at([u, v]).unwrap();
                for i in 0..3 {
                    expected[i] = [expected[i][0].min(p[i]), expected[i][1].max(p[i])];
                }
            }
        }
        let bounds = surface.enclose(box_uv).unwrap();
        let size = expected.iter().map(|[lo, hi]| hi - lo).fold(0.0, f64::max);
        for i in 0..3 {
            assert!(
                (bounds.axes[i].lo - expected[i][0]).abs() < 1e-10 * size,
                "{surface:?} {i}"
            );
            assert!(
                (bounds.axes[i].hi - expected[i][1]).abs() < 1e-10 * size,
                "{surface:?} {i}"
            );
        }
    }
}
