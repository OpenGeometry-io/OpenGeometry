use crate::brep::{
    fine_accuracy, Curve, CurveGeometry, Frame3, GeometryStore, Surface, SurfaceGeometry,
};
use crate::intersection::ssi::intersect_surfaces;
use crate::intersection::ssi_result::SsiResult;
use crate::math::{norm, sub, Point3};

fn assert_numerical_branches(store: &GeometryStore, result: &SsiResult) {
    assert!(!result.curves.is_empty());
    for branch in &result.curves {
        let CurveGeometry::Intersection { definition } = store.curves[branch.curve as usize] else {
            panic!("expected a numerical intersection curve")
        };
        let support_ids = store.intersections[definition as usize].surfaces;
        let domain = branch
            .domain
            .expect("numerical branch needs a finite domain");
        for i in 0..=8 {
            let t = domain.lo + domain.width() * i as f64 / 8.0;
            let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
            for side in 0..2 {
                let uv = store.pcurve_at(branch.pcurves[side], t).unwrap();
                assert!(
                    norm(sub(
                        store.surfaces[support_ids[side] as usize]
                            .point_at(uv)
                            .unwrap(),
                        point
                    )) <= fine_accuracy().intersection
                );
            }
        }
    }
}

#[test]
fn plane_plane_and_universal_torus_intersections_are_distinct() {
    let mut s = GeometryStore::new();
    s.surfaces = vec![
        SurfaceGeometry::Plane {
            frame: Frame3::IDENTITY,
        },
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
        },
        SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 2.0,
            minor_radius: 0.5,
        },
        SurfaceGeometry::Torus {
            frame: Frame3 {
                origin: [0.5, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            major_radius: 2.0,
            minor_radius: 0.5,
        },
    ];
    let r = intersect_surfaces(&mut s, 0, 1, fine_accuracy()).unwrap();
    assert!(matches!(
        s.curves[r.curves[0].curve as usize],
        CurveGeometry::Line { .. }
    ));
    let numerical = intersect_surfaces(&mut s, 2, 3, fine_accuracy()).unwrap();
    assert_numerical_branches(&s, &numerical);
}

#[test]
fn coaxial_sphere_cylinder_has_two_circles_tangent_circle_or_empty() {
    let intersect = |sphere_origin: Point3, cylinder_origin: Point3, cylinder_radius: f64| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Sphere {
                frame: Frame3 {
                    origin: sphere_origin,
                    ..Frame3::IDENTITY
                },
                radius: 2.0,
            },
            SurfaceGeometry::Cylinder {
                frame: Frame3 {
                    origin: cylinder_origin,
                    ..Frame3::IDENTITY
                },
                radius: cylinder_radius,
            },
        ];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy());
        (store, result)
    };
    let (store, Ok(two)) = intersect([0.0, 0.0, 3.0], [0.0; 3], 1.0) else {
        panic!("coaxial sphere/cylinder should intersect")
    };
    assert_eq!(two.curves.len(), 2);
    for branch in two.curves {
        for i in 0..=16 {
            let t = std::f64::consts::TAU * i as f64 / 16.0;
            let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
            for side in 0..2 {
                let uv = store.pcurve_at(branch.pcurves[side], t).unwrap();
                assert!(norm(sub(store.surfaces[side].point_at(uv).unwrap(), point)) < 1e-10);
            }
        }
    }
    assert_eq!(
        intersect([0.0; 3], [0.0, 0.0, 9.0], 2.0)
            .1
            .unwrap()
            .curves
            .len(),
        1
    );
    assert!(intersect([0.0; 3], [0.0; 3], 3.0)
        .1
        .unwrap()
        .curves
        .is_empty());
    let (store, result) = intersect([0.1, 0.0, 0.0], [0.0; 3], 1.0);
    assert_numerical_branches(&store, &result.unwrap());
}

#[test]
fn torus_special_sections_cover_plane_sphere_cylinder_and_cone() {
    let torus = SurfaceGeometry::Torus {
        frame: Frame3::IDENTITY,
        major_radius: 3.0,
        minor_radius: 1.0,
    };
    let assert_curves = |other: SurfaceGeometry, expected: usize| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![other, torus.clone()];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy()).unwrap();
        assert_eq!(result.curves.len(), expected);
        for branch in &result.curves {
            assert!(matches!(
                store.curves[branch.curve as usize],
                CurveGeometry::Circle { .. }
            ));
            for i in 0..=16 {
                let t = std::f64::consts::TAU * i as f64 / 16.0;
                let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
                for side in 0..2 {
                    let uv = store.pcurve_at(branch.pcurves[side], t).unwrap();
                    assert!(norm(sub(store.surfaces[side].point_at(uv).unwrap(), point)) < 1e-10);
                }
            }
        }
        store
    };

    let horizontal = assert_curves(
        SurfaceGeometry::Plane {
            frame: Frame3::IDENTITY,
        },
        2,
    );
    let mut radii: Vec<_> = horizontal
        .curves
        .iter()
        .filter_map(|curve| match curve {
            CurveGeometry::Circle { radius, .. } => Some(*radius),
            _ => None,
        })
        .collect();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii, vec![2.0, 4.0]);
    assert_curves(
        SurfaceGeometry::Plane {
            frame: Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
        },
        1,
    );
    assert_curves(
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap(),
        },
        2,
    );
    assert_curves(
        SurfaceGeometry::Sphere {
            frame: Frame3::IDENTITY,
            radius: 10.0_f64.sqrt(),
        },
        2,
    );
    assert_curves(
        SurfaceGeometry::Cylinder {
            frame: Frame3::IDENTITY,
            radius: 3.0,
        },
        2,
    );
    assert_curves(
        SurfaceGeometry::Cone {
            frame: Frame3 {
                origin: [0.0, 0.0, -3.0],
                ..Frame3::IDENTITY
            },
            semi_angle: std::f64::consts::FRAC_PI_4,
        },
        2,
    );
    assert_curves(
        SurfaceGeometry::Torus {
            frame: Frame3 {
                origin: [0.0, 0.0, 1.0],
                ..Frame3::IDENTITY
            },
            major_radius: 3.0,
            minor_radius: 1.0,
        },
        2,
    );
    assert_curves(
        SurfaceGeometry::Torus {
            frame: Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
            major_radius: 3.0,
            minor_radius: 1.0,
        },
        1,
    );

    let mut coincident = GeometryStore::new();
    coincident.surfaces = vec![torus.clone(), torus.clone()];
    assert!(
        intersect_surfaces(&mut coincident, 0, 1, fine_accuracy())
            .unwrap()
            .coincident
    );
    let mut skew = GeometryStore::new();
    skew.surfaces = vec![
        torus,
        SurfaceGeometry::Torus {
            frame: Frame3 {
                origin: [0.5, 0.0, 0.0],
                ..Frame3::IDENTITY
            },
            major_radius: 3.0,
            minor_radius: 1.0,
        },
    ];
    let numerical = intersect_surfaces(&mut skew, 0, 1, fine_accuracy()).unwrap();
    assert_numerical_branches(&skew, &numerical);
}
