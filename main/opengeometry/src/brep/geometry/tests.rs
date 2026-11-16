use super::curve::CurveGeometry;
use super::surface::{Surface, SurfaceGeometry};
use super::wrap::Wrap;
use crate::brep::bounds::PatchBounds;
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
        SurfaceGeometry::Wrapped {
            wrap: cone_wrap(0.5),
            surface: Box::new(SurfaceGeometry::Cylinder {
                frame: Frame3 {
                    origin: [1.0, 0.5, 3.0],
                    ..Frame3::IDENTITY
                },
                radius: 0.8,
            }),
        },
        SurfaceGeometry::Wrapped {
            wrap: cone_wrap(-0.7),
            surface: Box::new(SurfaceGeometry::Plane {
                frame: Frame3::from_axis([0.3, -0.2, 2.5], [1.0, 2.0, 0.3], [0.0, 0.0, 1.0])
                    .unwrap(),
            }),
        },
    ]
}

fn cone_wrap(semi_angle: f64) -> Wrap {
    Wrap {
        semi_angle,
        ..wrap()
    }
}

#[test]
fn a_cone_wrap_develops_its_cone_and_round_trips() {
    for semi_angle in [0.5, -0.7, 1e-3] {
        let w = cone_wrap(semi_angle);
        let (s, c) = semi_angle.sin_cos();
        let r = w.radius;
        for psi in [0.0_f64, 0.4, -1.1] {
            let k = s / r;
            let p = [psi.sin() / k, (psi.cos() - 1.0) / k, r];
            let l = w.frame.local(w.point(p));
            assert!((l[0].hypot(l[1]) - r).abs() < 1e-12 && l[2].abs() < 1e-12);
            let turn = psi / s;
            assert!((l[0] - r * turn.cos()).abs() < 1e-9 && (l[1] - r * turn.sin()).abs() < 1e-9);
        }
        let l = w.frame.local(w.point([0.0, 1.5, r + 0.25]));
        assert!((l[0] - (r + 1.5 * s + 0.25 * c)).abs() < 1e-12);
        assert!((l[2] - (1.5 * c - 0.25 * s)).abs() < 1e-12);
        for p in [[0.9, -0.6, r], [-1.7, 0.8, r]] {
            let (dx, dy) = (w.push(p, [1.0, 0.0, 0.0]), w.push(p, [0.0, 1.0, 0.0]));
            assert!((norm(dx) - 1.0).abs() < 1e-12 && (norm(dy) - 1.0).abs() < 1e-12);
            assert!(dot(dx, dy).abs() < 1e-12);
            let h = 1e-6;
            for (i, v) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
                .iter()
                .enumerate()
            {
                let at =
                    |t: f64| w.point([p[0] + t * v[0], p[1] + t * v[1], p[2] + 0.3 + t * v[2]]);
                let fd = scale(sub(at(h), at(-h)), 0.5 / h);
                let q = [p[0], p[1], p[2] + 0.3];
                assert!(norm(sub(fd, w.push(q, *v))) < 1e-8, "{semi_angle} push {i}");
                for (j, u) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
                    .iter()
                    .enumerate()
                {
                    let pu =
                        |t: f64| w.push([q[0] + t * u[0], q[1] + t * u[1], q[2] + t * u[2]], *v);
                    let fd = scale(sub(pu(h), pu(-h)), 0.5 / h);
                    assert!(
                        norm(sub(fd, w.second(q, *v, *u))) < 1e-7,
                        "{semi_angle} second {i} {j}"
                    );
                }
            }
            let back = w.flatten(w.point(p), Some(p)).unwrap();
            assert!(norm(sub(back, p)) < 1e-11);
        }
        let far = [r * 4.0 * s.abs().max(0.6), 0.3, r - 0.2];
        if w.on_cone_theta(far).abs() > std::f64::consts::PI {
            assert!(norm(sub(w.flatten(w.point(far), None).unwrap(), far)) > 1e-3);
        }
        assert!(norm(sub(w.flatten(w.point(far), Some(far)).unwrap(), far)) < 1e-10);
        let curve = CurveGeometry::Wrapped {
            wrap: w,
            curve: Box::new(CurveGeometry::Circle {
                frame: Frame3 {
                    origin: [1.0, 0.5, r + 0.4],
                    ..Frame3::IDENTITY
                },
                radius: 0.8,
            }),
        };
        let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
        let bounds = curve.elementary_bounds(range).unwrap();
        let samples = curve.sample_elementary(range, 1e-3, 100_000).unwrap();
        let n = samples.len() - 1;
        for i in 0..n {
            for k in 1..8 {
                let t = range.width() * (i as f64 + k as f64 / 8.0) / n as f64;
                let p = curve.elementary_point(t).unwrap();
                assert!(bounds.contains(p));
                let (a, b) = (samples[i], samples[i + 1]);
                let ab = sub(b, a);
                let along = dot(sub(p, a), ab) / dot(ab, ab);
                let foot = [0, 1, 2].map(|j| a[j] + along * ab[j]);
                assert!(norm(sub(p, foot)) <= 1e-3);
            }
        }
    }
    for semi_angle in [0.9_f64, -0.9] {
        let w = cone_wrap(semi_angle);
        let k = semi_angle.sin() / w.radius;
        for (psi, rho) in [(2.0_f64, 0.7), (-2.6, 1.3), (3.0, 0.5)] {
            let over_the_cut = psi == 3.0;
            let centre = [rho * psi.sin() / k, (rho * psi.cos() - 1.0) / k];
            let half = 0.2 * rho / k.abs();
            let flat = PatchBounds {
                axes: [
                    Interval::new(centre[0] - half, centre[0] + half).unwrap(),
                    Interval::new(centre[1] - half, centre[1] + half).unwrap(),
                    Interval::new(w.radius - 0.3, w.radius + 0.3).unwrap(),
                ],
            };
            if over_the_cut {
                assert!(w.enclose(flat).is_err());
                continue;
            }
            let bounds = w.enclose(flat).unwrap();
            assert!(w.derivative_bounds(flat).is_ok());
            for i in 0..=10 {
                for j in 0..=10 {
                    for z in [-0.3, 0.3] {
                        let p = [
                            centre[0] - half + 2.0 * half * i as f64 / 10.0,
                            centre[1] - half + 2.0 * half * j as f64 / 10.0,
                            w.radius + z,
                        ];
                        assert!(bounds.contains(w.point(p)), "{semi_angle} {psi}");
                    }
                }
            }
        }
    }
    let bad = Wrap {
        semi_angle: std::f64::consts::FRAC_PI_2,
        ..wrap()
    };
    assert!(bad.validate().is_err());
}

fn wrap() -> Wrap {
    Wrap {
        frame: Frame3::from_axis([3.0, -2.0, 7.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap(),
        radius: 2.6,
        semi_angle: 0.0,
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
    assert!(norm(sub(w.flatten(w.point(far), Some(far)).unwrap(), far)) < 1e-12);
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

#[test]
fn a_wrapped_patch_round_a_cone_apex_bounds_in_parts() {
    let w = cone_wrap(0.5);
    let apex = -w.radius / 0.5f64.sin();
    let surface = SurfaceGeometry::Wrapped {
        wrap: w,
        surface: Box::new(SurfaceGeometry::Cylinder {
            frame: Frame3 {
                origin: [0.0, apex, w.radius],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
                z: [0.0, 0.0, 1.0],
            },
            radius: 3.0,
        }),
    };
    let domain = [
        Interval::new(
            0.3 - std::f64::consts::FRAC_PI_2,
            1.5 * std::f64::consts::PI - 0.3,
        )
        .unwrap(),
        Interval::new(0.0, 0.5).unwrap(),
    ];
    assert!(matches!(
        w.enclose(
            SurfaceGeometry::Cylinder {
                frame: Frame3::IDENTITY,
                radius: 1.0
            }
            .enclose(domain)
            .map(|_| PatchBounds {
                axes: [
                    Interval::new(-3.0, 3.0).unwrap(),
                    Interval::new(apex - 3.0, apex + 3.0).unwrap(),
                    domain[1],
                ]
            })
            .unwrap()
        ),
        Err(GeometryError::SingularParameterization)
    ));
    let bounds = surface.enclose(domain).unwrap();
    for i in 0..=16 {
        for j in 0..=2 {
            let uv = [
                domain[0].lo + domain[0].width() * i as f64 / 16.0,
                0.25 * j as f64,
            ];
            assert!(bounds.contains(surface.point_at(uv).unwrap()));
        }
    }
}
