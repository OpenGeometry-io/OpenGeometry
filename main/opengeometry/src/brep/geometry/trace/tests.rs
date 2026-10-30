use super::SsiBudget;
use crate::brep::error::GeometryError;
use crate::brep::frame::Frame3;
use crate::brep::geometry::curve::{Curve, CurveGeometry};
use crate::brep::geometry::intersection_definition::{IntersectionDefinition, TraceAnchor};
use crate::brep::geometry::store::GeometryStore;
use crate::brep::geometry::surface::SurfaceGeometry;
use crate::math::{add, dot, norm, scale, sub, Interval};

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

#[test]
fn evaluation_accepts_an_end_parameter_rounded_one_ulp_past_the_last_anchor() {
    let store = circle_trace();
    let definition = &store.intersections[0];
    let last = &definition.anchors[8];
    let point = definition
        .evaluate(f64::from_bits(last.parameter.to_bits() + 1), &store)
        .unwrap();
    assert!(norm(sub(point.point, last.point)) < 1e-12);
    assert!(matches!(
        definition.evaluate(last.parameter + 1e-6, &store),
        Err(GeometryError::InvalidGeometry(_))
    ));
}

#[test]
fn evaluation_converges_where_the_axial_tube_is_thinner_than_the_peak_between_anchors() {
    let mut store = GeometryStore::new();
    store.surfaces.push(SurfaceGeometry::Cylinder {
        frame: Frame3::IDENTITY,
        radius: 3.0,
    });
    let axis = [0.7648421872844885, 0.644217687237691, 0.0];
    store.surfaces.push(SurfaceGeometry::Cylinder {
        frame: Frame3 {
            origin: [1.9503475775754455, 1.642755102456112, 2.2],
            x: [-0.644217687237691, 0.7648421872844885, 0.0],
            y: [0.0, -0.0, 1.0],
            z: axis,
        },
        radius: 0.45,
    });
    let anchors = vec![
        TraceAnchor {
            parameter: 0.7128960125719093,
            point: [2.319031360088156, 1.903179852491002, 2.6483647006160944],
            uv_a: [0.6872233929727672, 2.648364700617094],
            uv_b: [-4.627110592037641, 0.4497551408002707],
        },
        TraceAnchor {
            parameter: 0.7865256619134415,
            point: [2.2716265395193083, 1.9595185288615027, 2.6486132842284404],
            uv_a: [0.7117670855789375, 2.6486132842255756],
            uv_b: [-4.790915085586551, 0.4497923059420013],
        },
    ];
    let uv_tubes = vec![[
        Interval::new(0.6859962083424562, 0.7129942702092485).unwrap(),
        Interval::new(2.6226562499999906, 2.6742187500000094).unwrap(),
        Interval::new(-5.118524072684371, -4.299501604939821).unwrap(),
        Interval::new(0.4496808105168094, 0.4498666362254626).unwrap(),
    ]];
    store.intersections.push(IntersectionDefinition {
        surfaces: [0, 1],
        anchors,
        uv_tubes,
        residual_tolerance: 1e-9,
    });
    let point = store.intersections[0]
        .evaluate(0.7313034249072923, &store)
        .unwrap();
    assert!(point.point[2] > 2.6, "{:?}", point.point);
    assert!((point.point[0].hypot(point.point[1]) - 3.0).abs() <= 1e-9);
    let offset = sub(point.point, [1.9503475775754455, 1.642755102456112, 2.2]);
    let radial = sub(offset, scale(axis, dot(offset, axis)));
    assert!((norm(radial) - 0.45).abs() <= 1e-9);
}
