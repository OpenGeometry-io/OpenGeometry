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
