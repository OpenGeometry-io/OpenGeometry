use super::curve::CurveGeometry;
use super::surface::{Surface, SurfaceGeometry};
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
    ]
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
