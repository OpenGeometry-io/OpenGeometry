use super::*;
use crate::brep::{Frame3, Wrap};

fn wrap() -> Wrap {
    Wrap {
        frame: Frame3::from_axis([1.0, 2.0, 3.0], [0.2, 1.0, 0.1], [1.0, 0.0, 0.3]).unwrap(),
        radius: 2.8,
    }
}

#[test]
fn a_wrapped_plane_fits_within_tolerance_between_its_samples() {
    let surface = SurfaceGeometry::Wrapped {
        wrap: wrap(),
        surface: Box::new(SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0; 3], [0.24, -0.97, 0.0], [0.97, 0.24, 0.0]).unwrap(),
        }),
    };
    let domain = [
        Interval::new(-0.4, 4.5).unwrap(),
        Interval::new(1.8, 4.2).unwrap(),
    ];
    let fitted = fit_surface(&surface, domain, 1e-6, 1 << 16).unwrap();
    assert!(fitted.error_bound <= 1e-6);
    let n = [
        (fitted.controls.len() - 1) / 3,
        (fitted.controls[0].len() - 1) / 3,
    ];
    let step = [0, 1].map(|i| domain[i].width() / n[i] as f64);
    for i in 0..n[0] {
        for j in 0..n[1] {
            for (s, t) in [(0.37, 0.61), (0.93, 0.05), (0.5, 0.5)] {
                let uv = [
                    domain[0].lo + (i as f64 + s) * step[0],
                    domain[1].lo + (j as f64 + t) * step[1],
                ];
                let p = patch_point(&fitted.controls, [i, j], s, t);
                assert!(norm(sub(p, surface.point_at(uv).unwrap())) <= 2e-6);
            }
        }
    }
    assert_eq!(fitted.knots[0].len(), n[0] + 1);
    assert_eq!(fitted.multiplicities[1].iter().sum::<usize>(), 3 * n[1] + 5);
}

#[test]
fn a_wrapped_circle_fits_with_joined_spans() {
    let curve = CurveGeometry::Wrapped {
        wrap: wrap(),
        curve: Box::new(CurveGeometry::Circle {
            frame: Frame3 {
                origin: [1.0, 5.0, 3.0],
                ..Frame3::IDENTITY
            },
            radius: 0.5,
        }),
    };
    let range = Interval::new(0.0, std::f64::consts::TAU).unwrap();
    let fitted = fit_curve(&curve, range, 1e-6, 20_000).unwrap();
    assert!(fitted.error_bound <= 1e-6);
    let spans = fitted.knots.len() - 1;
    assert_eq!(fitted.controls.len(), 3 * spans + 1);
    assert!(norm(sub(fitted.controls[0], fitted.controls[3 * spans])) < 1e-12);
    for k in 0..spans {
        let (a, b) = (fitted.knots[k], fitted.knots[k + 1]);
        let w = bernstein(0.3);
        let p: Point3 =
            std::array::from_fn(|i| (0..4).map(|j| w[j] * fitted.controls[3 * k + j][i]).sum());
        let exact = curve.elementary_point(a + 0.3 * (b - a)).unwrap();
        assert!(norm(sub(p, exact)) <= 2e-6);
    }
}
