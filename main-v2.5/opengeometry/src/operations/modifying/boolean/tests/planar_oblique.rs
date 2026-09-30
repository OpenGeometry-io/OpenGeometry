use super::support::{extrusion, extrusion_span, volume};
use crate::brep::{Frame3, GeometryQuality};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives;
use crate::query::{classify_point, PointClassification};

#[test]
fn oblique_planar_cutter_crosses_a_host_without_tessellating_the_boolean() {
    let host = extrusion(
        "oblique-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let angle = std::f64::consts::PI / 12.0;
    let cutter = primitives::linear_extrusion(
        "oblique-cutter".into(),
        Frame3 {
            origin: [2.5, 0.15, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result =
        boolean_brep(&host, &cutter, BooleanOp::Subtraction, "oblique-cut".into()).unwrap();
    result.brep.validate().unwrap();
    assert!(matches!(result.report.quality, GeometryQuality::Analytic));
    assert_eq!(
        classify_point(&result.brep, [2.8, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [2.8, 0.15, 2.5]).unwrap(),
        PointClassification::Inside
    );
    assert!(volume(&result.brep).abs() < volume(&host).abs());
}

#[test]
fn chained_oblique_planar_cuts_keep_both_voids_and_analytic_faces() {
    let host = extrusion(
        "chained-oblique-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]],
        Vec::new(),
    );
    let angle = std::f64::consts::PI / 12.0;
    let cutter = |name: &str, station: f64, sign: f64| {
        primitives::linear_extrusion(
            name.into(),
            Frame3 {
                origin: [station, 0.15, 0.0],
                x: [angle.cos(), 0.0, -sign * angle.sin()],
                y: [0.0, 1.0, 0.0],
                z: [sign * angle.sin(), 0.0, angle.cos()],
            },
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            Vec::new(),
            2.1,
            host.accuracy,
        )
        .unwrap()
    };
    let first = cutter("first-oblique", 2.5, 1.0);
    let second = cutter("second-oblique", 5.0, -1.0);
    let once = boolean_brep(&host, &first, BooleanOp::Subtraction, "once".into()).unwrap();
    let twice = boolean_brep(&once.brep, &second, BooleanOp::Subtraction, "twice".into()).unwrap();
    twice.brep.validate().unwrap();
    assert!(matches!(twice.report.quality, GeometryQuality::Analytic));
    for point in [[2.8, 0.15, 1.0], [4.7, 0.15, 1.0]] {
        assert_eq!(
            classify_point(&twice.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
    assert_eq!(
        classify_point(&twice.brep, [3.8, 0.15, 1.0]).unwrap(),
        PointClassification::Inside
    );
    assert!(volume(&twice.brep).abs() < volume(&once.brep).abs());
}

#[test]
fn oblique_cut_accepts_a_disconnected_planar_host() {
    let host = extrusion(
        "split-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let through = extrusion_span(
        "through",
        -1.0,
        5.0,
        vec![[3.0, -1.0], [4.0, -1.0], [4.0, 1.0], [3.0, 1.0]],
        Vec::new(),
    );
    let split = boolean_brep(&host, &through, BooleanOp::Subtraction, "split".into()).unwrap();
    assert_eq!(split.brep.solids.len(), 2);
    let angle = std::f64::consts::PI / 12.0;
    let oblique = primitives::linear_extrusion(
        "split-oblique".into(),
        Frame3 {
            origin: [6.0, 0.15, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &split.brep,
        &oblique,
        BooleanOp::Subtraction,
        "split-oblique-result".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 2);
    assert_eq!(
        classify_point(&result.brep, [6.3, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [1.0, 0.15, 1.0]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn planar_cutter_tilts_about_both_host_axes_keep_exact_occupancy() {
    let host = extrusion(
        "tilt-sweep-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    for axis in 0..2 {
        for degrees in [-25.0_f64, -15.0, -5.0, 5.0, 15.0, 25.0] {
            let angle = degrees.to_radians();
            let frame = if axis == 0 {
                Frame3 {
                    origin: [2.5, 0.15, 0.0],
                    x: [angle.cos(), 0.0, -angle.sin()],
                    y: [0.0, 1.0, 0.0],
                    z: [angle.sin(), 0.0, angle.cos()],
                }
            } else {
                Frame3 {
                    origin: [2.5, 0.15, 0.0],
                    x: [1.0, 0.0, 0.0],
                    y: [0.0, angle.cos(), -angle.sin()],
                    z: [0.0, angle.sin(), angle.cos()],
                }
            };
            let cutter = primitives::linear_extrusion(
                format!("tilt-{axis}-{degrees}"),
                frame,
                vec![[-0.8, -0.8], [0.8, -0.8], [0.8, 0.8], [-0.8, 0.8]],
                Vec::new(),
                2.1,
                host.accuracy,
            )
            .unwrap();
            let result = boolean_brep(
                &host,
                &cutter,
                BooleanOp::Subtraction,
                format!("tilt-result-{axis}-{degrees}"),
            )
            .unwrap();
            result.brep.validate().unwrap();
            assert_eq!(
                classify_point(&result.brep, [2.5, 0.15, 1.0]).unwrap(),
                PointClassification::Outside,
                "axis={axis}, degrees={degrees}"
            );
            assert_eq!(
                classify_point(&result.brep, [8.0, 0.15, 1.0]).unwrap(),
                PointClassification::Inside,
                "axis={axis}, degrees={degrees}"
            );
            assert!(volume(&result.brep).abs() < volume(&host).abs());
        }
    }
}

#[test]
fn oblique_cut_preserves_a_planar_host_with_profile_hole_loops() {
    let host = extrusion(
        "profile-hole-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
        vec![vec![[2.0, 1.0], [3.0, 1.0], [3.0, 2.0], [2.0, 2.0]]],
    );
    assert!(host
        .topology
        .faces
        .iter()
        .any(|face| !face.trim.holes.is_empty()));
    let angle = std::f64::consts::PI / 12.0;
    let oblique = primitives::linear_extrusion(
        "pocket-oblique".into(),
        Frame3 {
            origin: [6.0, 1.5, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(&host, &oblique, BooleanOp::Subtraction, "two-voids".into()).unwrap();
    result.brep.validate().unwrap();
    for point in [[2.5, 1.5, 1.5], [6.3, 1.5, 1.0]] {
        assert_eq!(
            classify_point(&result.brep, point).unwrap(),
            PointClassification::Outside
        );
    }
    for point in [[4.0, 1.5, 1.0], [6.3, 1.5, 2.5]] {
        assert_eq!(
            classify_point(&result.brep, point).unwrap(),
            PointClassification::Inside
        );
    }
}

#[test]
fn oblique_nonconvex_planar_cutter_keeps_exact_host_material() {
    let host = extrusion(
        "nonconvex-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.0, 0.3]],
        Vec::new(),
    );
    let angle = std::f64::consts::PI / 12.0;
    let cutter = primitives::linear_extrusion(
        "nonconvex-cutter".into(),
        Frame3 {
            origin: [5.0, 0.15, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, -0.05],
            [0.0, -0.05],
            [0.0, 1.0],
            [-1.0, 1.0],
        ],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "nonconvex-oblique-cut".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert!(matches!(result.report.quality, GeometryQuality::Analytic));
    assert_eq!(
        classify_point(&result.brep, [4.5, 0.15, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [5.5, 0.15, 1.0]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn oblique_planar_cut_preserves_an_existing_internal_cavity_shell() {
    let host = extrusion(
        "cavity-host",
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
        Vec::new(),
    );
    let cavity = extrusion_span(
        "internal-cavity",
        1.0,
        1.0,
        vec![[2.0, 1.0], [3.0, 1.0], [3.0, 2.0], [2.0, 2.0]],
        Vec::new(),
    );
    let host = boolean_brep(&host, &cavity, BooleanOp::Subtraction, "cavity-host".into())
        .unwrap()
        .brep;
    assert_eq!(host.solids[0].cavity_shells.len(), 1);
    let angle = std::f64::consts::PI / 12.0;
    let cutter = primitives::linear_extrusion(
        "oblique-after-cavity".into(),
        Frame3 {
            origin: [6.0, 1.5, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![[-0.5, -2.0], [0.5, -2.0], [0.5, 2.0], [-0.5, 2.0]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "oblique-cavity-cut".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 1);
    assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
    assert_eq!(
        classify_point(&result.brep, [2.5, 1.5, 1.5]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [6.2, 1.5, 1.0]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [6.2, 1.5, 2.6]).unwrap(),
        PointClassification::Inside
    );
}

#[test]
fn oblique_cut_assigns_a_remote_cavity_to_its_original_material_component() {
    let left = extrusion(
        "left-cavity-host",
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]],
        Vec::new(),
    );
    let cavity = extrusion_span(
        "left-cavity",
        1.0,
        1.0,
        vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
        Vec::new(),
    );
    let left_with_cavity = boolean_brep(
        &left,
        &cavity,
        BooleanOp::Subtraction,
        "left-with-cavity".into(),
    )
    .unwrap()
    .brep;
    let right = extrusion(
        "right-host",
        vec![[6.0, 0.0], [10.0, 0.0], [10.0, 3.0], [6.0, 3.0]],
        Vec::new(),
    );
    let host = boolean_brep(
        &left_with_cavity,
        &right,
        BooleanOp::Union,
        "two-components".into(),
    )
    .unwrap()
    .brep;
    assert_eq!(host.solids.len(), 2);
    let angle = std::f64::consts::PI / 12.0;
    let cutter = primitives::linear_extrusion(
        "right-oblique-cutter".into(),
        Frame3 {
            origin: [8.5, 1.5, 0.0],
            x: [angle.cos(), 0.0, -angle.sin()],
            y: [0.0, 1.0, 0.0],
            z: [angle.sin(), 0.0, angle.cos()],
        },
        vec![[-0.5, -2.0], [0.5, -2.0], [0.5, 2.0], [-0.5, 2.0]],
        Vec::new(),
        2.1,
        host.accuracy,
    )
    .unwrap();
    let result = boolean_brep(
        &host,
        &cutter,
        BooleanOp::Subtraction,
        "remote-cavity-cut".into(),
    )
    .unwrap();
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 2);
    assert_eq!(
        result
            .brep
            .solids
            .iter()
            .map(|solid| solid.cavity_shells.len())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        classify_point(&result.brep, [1.5, 1.5, 1.5]).unwrap(),
        PointClassification::Outside
    );
    assert_eq!(
        classify_point(&result.brep, [8.5, 1.5, 1.0]).unwrap(),
        PointClassification::Outside
    );
}
