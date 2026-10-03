use super::*;
use crate::brep::{Accuracy, CurveGeometry, Frame3, SsiBudget, SurfaceGeometry};
use crate::intersection::intersect_patches;

#[test]
fn fitted_numerical_curve_has_cubic_shape_and_support_bound() {
    let mut store = GeometryStore::new();
    store.surfaces = vec![
        SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 2.0,
            minor_radius: 0.5,
        },
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0, 0.0, 0.15], [0.35, -0.2, 1.0], [1.0, 0.0, 0.0]).unwrap(),
        },
    ];
    let tau = std::f64::consts::TAU;
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [Interval::new(0.0, tau).unwrap(); 2],
            [Interval::new(-4.0, 4.0).unwrap(); 2],
        ],
        Accuracy {
            geometric: 1e-5,
            intersection: 2.5e-6,
            tessellation: 1e-3,
            exchange: 0.05,
        },
        SsiBudget::default(),
    )
    .unwrap();
    let definition = match store.curves[result.curves[0].curve as usize] {
        CurveGeometry::Intersection { definition } => definition,
        _ => panic!("universal SSI must create an intersection definition"),
    };
    let fitted = fit_intersection_curve(&store, definition, 1.0, 20_000).unwrap();
    assert!(fitted.controls.len() >= 4);
    assert_eq!((fitted.controls.len() - 1) % 3, 0);
    assert_eq!(fitted.controls.len(), fitted.pcurve_a.len());
    assert_eq!(fitted.controls.len(), fitted.pcurve_b.len());
    assert!(fitted.error_bound <= 1.0);
    assert_eq!(fitted.knots.len(), fitted.multiplicities.len());
    assert_eq!(fitted.multiplicities.first(), Some(&4));
    assert_eq!(fitted.multiplicities.last(), Some(&4));
}
