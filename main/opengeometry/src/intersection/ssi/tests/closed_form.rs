use crate::brep::{
    fine_accuracy, Curve, CurveGeometry, Frame3, GeometryError, GeometryStore, Surface,
    SurfaceGeometry,
};
use crate::intersection::ssi::intersect_surfaces;
use crate::math::{norm, sub, Point3};

#[test]
fn plane_sphere_emits_shared_curve_and_both_pcurves() {
    let mut s = GeometryStore::new();
    s.surfaces.push(SurfaceGeometry::Plane {
        frame: Frame3 {
            origin: [0.0, 0.0, 0.5],
            ..Frame3::IDENTITY
        },
    });
    s.surfaces.push(SurfaceGeometry::Sphere {
        frame: Frame3::IDENTITY,
        radius: 1.0,
    });
    let r = intersect_surfaces(&mut s, 0, 1, fine_accuracy()).unwrap();
    assert_eq!(r.curves.len(), 1);
    let curve = &r.curves[0];
    for i in 0..=32 {
        let t = std::f64::consts::TAU * i as f64 / 32.0;
        let p = s.curve(curve.curve).unwrap().point_at(t).unwrap();
        for j in 0..2 {
            let uv = s.pcurve_at(curve.pcurves[j], t).unwrap();
            assert!(norm(sub(s.surfaces[j].point_at(uv).unwrap(), p)) < 1e-10);
        }
    }
}

#[test]
fn sphere_contacts_and_near_misses_do_not_become_polylines() {
    for (distance, curves, contacts) in [
        (1.0, 1, 0),
        (2.0, 0, 1),
        (2.0 + 1e-9, 0, 0),
        (2.0 - 1e-9, 1, 0),
    ] {
        let mut s = GeometryStore::new();
        s.surfaces = vec![
            SurfaceGeometry::Sphere {
                frame: Frame3::IDENTITY,
                radius: 1.0,
            },
            SurfaceGeometry::Sphere {
                frame: Frame3 {
                    origin: [distance, 0.0, 0.0],
                    ..Frame3::IDENTITY
                },
                radius: 1.0,
            },
        ];
        let r = intersect_surfaces(&mut s, 0, 1, fine_accuracy()).unwrap();
        assert_eq!(r.curves.len(), curves);
        assert_eq!(r.contacts.len(), contacts);
    }
}

#[test]
fn plane_cylinder_sections_cover_circle_ellipse_generators_and_tangency() {
    let cylinder = SurfaceGeometry::Cylinder {
        frame: Frame3::IDENTITY,
        radius: 1.0,
    };
    let section = |frame: Frame3| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![SurfaceGeometry::Plane { frame }, cylinder.clone()];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy()).unwrap();
        (store, result)
    };

    let (circle_store, circle_result) = section(Frame3::IDENTITY);
    assert_eq!(circle_result.curves.len(), 1);
    assert!(matches!(
        circle_store.curves[circle_result.curves[0].curve as usize],
        CurveGeometry::Circle { .. }
    ));

    let diagonal = 0.5_f64.sqrt();
    let oblique = Frame3::from_axis([0.0; 3], [0.0, diagonal, diagonal], [1.0, 0.0, 0.0]).unwrap();
    let (ellipse_store, ellipse_result) = section(oblique);
    let CurveGeometry::Ellipse {
        major_radius,
        minor_radius,
        ..
    } = ellipse_store.curves[ellipse_result.curves[0].curve as usize]
    else {
        panic!("oblique section must be an ellipse")
    };
    assert!((major_radius - 2.0_f64.sqrt()).abs() < 1e-12);
    assert!((minor_radius - 1.0).abs() < 1e-12);
    for i in 0..=32 {
        let t = std::f64::consts::TAU * i as f64 / 32.0;
        let point = ellipse_store
            .curve(ellipse_result.curves[0].curve)
            .unwrap()
            .point_at(t)
            .unwrap();
        for side in 0..2 {
            let uv = ellipse_store
                .pcurve_at(ellipse_result.curves[0].pcurves[side], t)
                .unwrap();
            assert!(
                norm(sub(
                    ellipse_store.surfaces[side].point_at(uv).unwrap(),
                    point
                )) < 1e-10
            );
        }
    }

    let parallel = |x| Frame3::from_axis([x, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    assert_eq!(section(parallel(0.0)).1.curves.len(), 2);
    assert_eq!(section(parallel(1.0)).1.curves.len(), 1);
    let outside = section(parallel(1.0 + 1e-9)).1;
    assert!(outside.curves.is_empty() && outside.contacts.is_empty());
}

#[test]
fn parallel_cylinders_cover_two_generators_tangency_coaxial_and_gap() {
    let intersect = |origin: Point3, radius: f64, axis: Point3| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Cylinder {
                frame: Frame3::IDENTITY,
                radius: 1.0,
            },
            SurfaceGeometry::Cylinder {
                frame: Frame3::from_axis(origin, axis, [1.0, 0.0, 0.0]).unwrap(),
                radius,
            },
        ];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy());
        (store, result)
    };

    let (store, Ok(two)) = intersect([1.0, 0.0, 3.0], 1.0, [0.0, 0.0, -1.0]) else {
        panic!("parallel cylinders should intersect")
    };
    assert_eq!(two.curves.len(), 2);
    for branch in two.curves {
        for t in [-3.0, 0.0, 4.0] {
            let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
            for side in 0..2 {
                let uv = store.pcurve_at(branch.pcurves[side], t).unwrap();
                assert!(norm(sub(store.surfaces[side].point_at(uv).unwrap(), point)) < 1e-10);
            }
        }
    }

    assert_eq!(
        intersect([2.0, 0.0, 0.0], 1.0, [0.0, 0.0, 1.0])
            .1
            .unwrap()
            .curves
            .len(),
        1
    );
    assert!(
        intersect([0.0, 0.0, 2.0], 1.0, [0.0, 0.0, -1.0])
            .1
            .unwrap()
            .coincident
    );
    assert!(intersect([3.0, 0.0, 0.0], 1.0, [0.0, 0.0, 1.0])
        .1
        .unwrap()
        .curves
        .is_empty());
    assert!(matches!(
        intersect([1.0, 0.0, 0.0], 1.0, [0.0, 1.0, 1.0]).1,
        Err(GeometryError::CoverageGap { .. })
    ));
}

#[test]
fn coaxial_cylinder_cone_and_cone_pairs_are_classified_without_faceting() {
    let mut cylinder_cone = GeometryStore::new();
    cylinder_cone.surfaces = vec![
        SurfaceGeometry::Cylinder {
            frame: Frame3 {
                origin: [0.0, 0.0, -7.0],
                ..Frame3::IDENTITY
            },
            radius: 2.0,
        },
        SurfaceGeometry::Cone {
            frame: Frame3::IDENTITY,
            semi_angle: std::f64::consts::FRAC_PI_4,
        },
    ];
    let section = intersect_surfaces(&mut cylinder_cone, 0, 1, fine_accuracy()).unwrap();
    assert_eq!(section.curves.len(), 1);
    let CurveGeometry::Circle { frame, radius } =
        cylinder_cone.curves[section.curves[0].curve as usize]
    else {
        panic!("coaxial cylinder/cone intersection must be a circle")
    };
    assert!((radius - 2.0).abs() < 1e-12);
    assert!(norm(sub(frame.origin, [0.0, 0.0, 2.0])) < 1e-12);

    let cone = |origin, axis, semi_angle| SurfaceGeometry::Cone {
        frame: Frame3::from_axis(origin, axis, [1.0, 0.0, 0.0]).unwrap(),
        semi_angle,
    };
    let intersect = |a: SurfaceGeometry, b: SurfaceGeometry| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![a, b];
        intersect_surfaces(&mut store, 0, 1, fine_accuracy()).unwrap()
    };
    assert!(
        intersect(
            cone([0.0; 3], [0.0, 0.0, 1.0], 0.4),
            cone([0.0; 3], [0.0, 0.0, 1.0], 0.4),
        )
        .coincident
    );
    let apex_contact = intersect(
        cone([0.0; 3], [0.0, 0.0, 1.0], 0.3),
        cone([0.0; 3], [0.0, 0.0, 1.0], 0.6),
    );
    assert_eq!(apex_contact.contacts, vec![[0.0; 3]]);
    let opposed = intersect(
        cone([0.0; 3], [0.0, 0.0, 1.0], 0.4),
        cone([0.0, 0.0, 4.0], [0.0, 0.0, -1.0], 0.4),
    );
    assert_eq!(opposed.curves.len(), 1);
}

#[test]
fn perpendicular_plane_cone_sections_keep_circle_and_apex_contact() {
    let section = |height: f64| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Plane {
                frame: Frame3 {
                    origin: [0.0, 0.0, height],
                    ..Frame3::IDENTITY
                },
            },
            SurfaceGeometry::Cone {
                frame: Frame3::IDENTITY,
                semi_angle: std::f64::consts::FRAC_PI_4,
            },
        ];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy()).unwrap();
        (store, result)
    };
    let (store, positive) = section(2.0);
    let CurveGeometry::Circle { radius, .. } = store.curves[positive.curves[0].curve as usize]
    else {
        panic!("perpendicular plane/cone section must be a circle")
    };
    assert!((radius - 2.0).abs() < 1e-12);
    assert_eq!(section(0.0).1.contacts, vec![[0.0; 3]]);
    assert!(section(-1.0).1.curves.is_empty());

    let mut oblique = GeometryStore::new();
    oblique.surfaces = vec![
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0]).unwrap(),
        },
        SurfaceGeometry::Cone {
            frame: Frame3::IDENTITY,
            semi_angle: 0.4,
        },
    ];
    let ellipse = intersect_surfaces(&mut oblique, 0, 1, fine_accuracy()).unwrap();
    assert!(matches!(
        oblique.curves[ellipse.curves[0].curve as usize],
        CurveGeometry::Ellipse { .. }
    ));
    for i in 0..=32 {
        let t = std::f64::consts::TAU * i as f64 / 32.0;
        let point = oblique
            .curve(ellipse.curves[0].curve)
            .unwrap()
            .point_at(t)
            .unwrap();
        for side in 0..2 {
            let uv = oblique
                .pcurve_at(ellipse.curves[0].pcurves[side], t)
                .unwrap();
            assert!(norm(sub(oblique.surfaces[side].point_at(uv).unwrap(), point)) < 1e-10);
        }
    }

    let mut hyperbolic = GeometryStore::new();
    hyperbolic.surfaces = vec![
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([0.0, 0.0, 1.0], [0.0, 1.0, 0.1], [1.0, 0.0, 0.0]).unwrap(),
        },
        SurfaceGeometry::Cone {
            frame: Frame3::IDENTITY,
            semi_angle: 0.4,
        },
    ];
    assert!(matches!(
        intersect_surfaces(&mut hyperbolic, 0, 1, fine_accuracy()),
        Err(GeometryError::CoverageGap { .. })
    ));
}

#[test]
fn coaxial_sphere_cone_classifies_two_single_and_empty_sections() {
    let intersect = |center_height: f64, radius: f64| {
        let mut store = GeometryStore::new();
        store.surfaces = vec![
            SurfaceGeometry::Sphere {
                frame: Frame3 {
                    origin: [0.0, 0.0, center_height],
                    ..Frame3::IDENTITY
                },
                radius,
            },
            SurfaceGeometry::Cone {
                frame: Frame3::IDENTITY,
                semi_angle: std::f64::consts::FRAC_PI_4,
            },
        ];
        let result = intersect_surfaces(&mut store, 0, 1, fine_accuracy());
        (store, result)
    };
    let (store, Ok(two)) = intersect(3.0, 2.5) else {
        panic!("coaxial sphere/cone should intersect twice")
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
    assert_eq!(intersect(0.0, 1.0).1.unwrap().curves.len(), 1);
    assert!(intersect(3.0, 1.0).1.unwrap().curves.is_empty());
    let (_, apex) = intersect(3.0, 3.0);
    let apex = apex.unwrap();
    assert_eq!(apex.curves.len(), 1);
    assert_eq!(apex.contacts, vec![[0.0; 3]]);
}

#[test]
fn canonical_fast_dispatch_covers_all_fifteen_unordered_family_pairs() {
    let families = vec![
        SurfaceGeometry::Plane {
            frame: Frame3::IDENTITY,
        },
        SurfaceGeometry::Sphere {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        },
        SurfaceGeometry::Cylinder {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        },
        SurfaceGeometry::Cone {
            frame: Frame3::IDENTITY,
            semi_angle: 0.4,
        },
        SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 3.0,
            minor_radius: 0.5,
        },
    ];
    let mut count = 0;
    for i in 0..families.len() {
        for j in i..families.len() {
            let mut store = GeometryStore::new();
            store.surfaces = vec![families[i].clone(), families[j].clone()];
            intersect_surfaces(&mut store, 0, 1, fine_accuracy())
                .unwrap_or_else(|error| panic!("family pair {i}/{j}: {error}"));
            count += 1;
        }
    }
    assert_eq!(count, 15);
}
