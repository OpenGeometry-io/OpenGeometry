use super::*;
use crate::brep::geometry::{Curve, Surface};
use crate::brep::topology::coarse_accuracy;
use crate::math::sub;
use crate::primitives;

#[test]
fn similarity_preserves_all_families_trims_and_curve_parameterization() {
    let placement = Frame3::from_axis([10.0, -3.0, 5.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap();
    let bodies = [
        primitives::cylinder(
            "c".into(),
            Frame3::IDENTITY,
            1.0,
            2.0,
            coarse_accuracy(1e-6),
        )
        .unwrap(),
        primitives::cone(
            "cone".into(),
            Frame3::IDENTITY,
            1.0,
            2.0,
            coarse_accuracy(1e-6),
        )
        .unwrap(),
        primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, coarse_accuracy(1e-6)).unwrap(),
        primitives::torus(
            "t".into(),
            Frame3::IDENTITY,
            2.0,
            0.5,
            coarse_accuracy(1e-6),
        )
        .unwrap(),
        primitives::cylinder_with_circular_hole(
            "w".into(),
            Frame3::IDENTITY,
            Frame3::IDENTITY,
            0.5,
            2.0,
            2.0,
            coarse_accuracy(1e-6),
        )
        .unwrap(),
    ];
    for input in bodies {
        let json = input.to_json().unwrap();
        let output = placed(&input, placement, 3.5).unwrap();
        assert_eq!(input.to_json().unwrap(), json);
        assert_eq!(output.revision, input.revision + 1);
        for edge in &input.topology.edges {
            if let EdgeGeometry::Curve { curve, range } = edge.geometry {
                let t = range.lo / 2.0 + range.hi / 2.0;
                let expected = placement.point(vector_scale(
                    input.geometry.curve(curve).unwrap().point_at(t).unwrap(),
                    3.5,
                ));
                let output_t = if matches!(
                    input.geometry.curves[curve as usize],
                    CurveGeometry::Line { .. }
                ) {
                    t * 3.5
                } else {
                    t
                };
                assert!(
                    norm(sub(
                        output
                            .geometry
                            .curve(curve)
                            .unwrap()
                            .point_at(output_t)
                            .unwrap(),
                        expected
                    )) < 1e-12
                );
            }
        }
        for face in &input.topology.faces {
            let uv = face.trim.uv_bounds.map(|d| d.lo / 2.0 + d.hi / 2.0);
            let metric = metric(input.geometry.surface(face.surface).unwrap(), 3.5);
            let expected = placement.point(vector_scale(
                input
                    .geometry
                    .surface(face.surface)
                    .unwrap()
                    .point_at(uv)
                    .unwrap(),
                3.5,
            ));
            let actual = output
                .geometry
                .surface(face.surface)
                .unwrap()
                .point_at(std::array::from_fn(|i| uv[i] * metric[i]))
                .unwrap();
            assert!(norm(sub(actual, expected)) < 1e-12);
        }
        BrepEnvelope::from_json(&output.to_json().unwrap()).unwrap();
    }
}
#[test]
fn invalid_scale_revision_overflow_and_precision_are_structured_errors() {
    let input =
        primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, coarse_accuracy(1e-6)).unwrap();
    for scale in [0.0, -1.0, f64::NAN] {
        assert!(placed(&input, Frame3::IDENTITY, scale).is_err());
    }
    let mut bad = input.clone();
    bad.revision = u64::MAX;
    assert!(matches!(
        placed(&bad, Frame3::IDENTITY, 1.0),
        Err(GeometryError::LimitExceeded(_))
    ));
    assert!(matches!(
        placed(
            &input,
            Frame3 {
                origin: [1e12; 3],
                ..Frame3::IDENTITY
            },
            1.0
        ),
        Err(GeometryError::LimitExceeded(_))
    ));
}
