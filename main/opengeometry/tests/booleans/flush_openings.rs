use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3};
use opengeometry::exchange::export_step;
use opengeometry::operations::modifying::boolean::subtract_planar_cutters_with_handlers;
use opengeometry::primitives;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume;

const FRAME: f64 = 0.05;

fn prism(name: &str, span: [f64; 2], bottom: f64, height: f64) -> BrepEnvelope {
    primitives::linear_extrusion(
        name.into(),
        Frame3 {
            origin: [0.0, bottom, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
        },
        vec![
            [span[0], -0.1],
            [span[1], -0.1],
            [span[1], 0.1],
            [span[0], 0.1],
        ],
        Vec::new(),
        height,
        Accuracy::STANDARD,
    )
    .unwrap()
}

fn opening(centre: f64, width: f64) -> [f64; 2] {
    let half = width / 2.0 + FRAME;
    [centre - half, centre + half]
}

fn assert_cut(door_centre: f64, door_width: f64, window_centre: f64) {
    let label = format!("door {door_width} at {door_centre}, window at {window_centre}");
    let wall = prism("wall", [0.0, 6.0], 0.0, 3.0);
    let door = opening(door_centre, door_width);
    let window = opening(window_centre, 1.2);
    let cutters = [
        prism("door", door, 0.0, 2.1),
        prism("window", window, 0.9, 1.2),
    ];
    let (result, handlers) =
        subtract_planar_cutters_with_handlers(&wall, &cutters, "wall-cut".into())
            .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        handlers.last().map(String::as_str),
        Some("boolean_rectilinear"),
        "{label}"
    );
    result.brep.validate().unwrap();
    let (text, _) = export_step(&result.brep, "metre").unwrap();
    assert_eq!(
        Document::parse(&text).unwrap().count("MANIFOLD_SOLID_BREP"),
        1,
        "{label}"
    );
    let expected =
        6.0 * 0.2 * 3.0 - (door[1] - door[0]) * 0.2 * 2.1 - (window[1] - window[0]) * 0.2 * 1.2;
    let measured = volume::estimate(&result.brep, 0.01);
    assert!(
        (measured.value - expected).abs() <= measured.error_bound.max(1e-9),
        "{label}: {} versus {expected}",
        measured.value
    );
}

#[test]
fn flush_door_and_window_that_failed_in_floorkit_cut_both_openings() {
    assert_cut(3.375, 0.85, 4.5);
    assert_cut(3.3753, 0.85, 4.5003);
}

#[test]
fn flush_door_and_window_that_passed_in_floorkit_still_cut_both_openings() {
    assert_cut(2.6875, 2.225, 4.5);
    assert_cut(3.3753, 0.85, 4.5013);
}

#[test]
fn flush_door_and_window_cut_both_openings_for_every_door_width() {
    let flush = 4.5 - 0.6 - FRAME;
    for step in 0..37 {
        let width = 0.6 + 0.05 * f64::from(step);
        assert_cut(flush - width / 2.0 - FRAME, width, 4.5);
    }
}
