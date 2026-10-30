use super::band_chain::band_edges_inside;
use super::band_chain_seam::{clear_seam_angle, refined_extremes};
use crate::brep::{coarse_accuracy, Frame3};
use crate::operations::modifying::boolean::dispatch::boolean_brep;
use crate::operations::modifying::boolean::types::BooleanOp;
use crate::primitives::{self, ProfileEdge};
use std::f64::consts::TAU;

#[test]
fn seam_inside_an_opening_moves_to_the_middle_of_the_widest_free_gap() {
    let opening = [-0.15, -0.05, 0.05, 0.15];
    let (middle, gap) = clear_seam_angle(&opening, 0.0, TAU).unwrap();
    assert!((middle - (0.15 + 0.5 * (TAU - 0.3))).abs() < 1e-12);
    assert!((gap - (TAU - 0.3)).abs() < 1e-12);
    assert!(clear_seam_angle(&opening, 3.0, TAU).is_none());
}

#[test]
fn openings_round_the_whole_circle_leave_no_clear_seam_gap() {
    let around = (0..10_000)
        .map(|index| TAU * f64::from(index) / 10_000.0)
        .collect::<Vec<_>>();
    let (_, gap) = clear_seam_angle(&around, 0.0, TAU).unwrap();
    assert!(gap <= 1e-3);
}

#[test]
fn refined_extremes_find_a_peak_between_samples() {
    let [low, high] = refined_extremes(|t| Ok((t + 0.1).sin()), 0.0, std::f64::consts::PI).unwrap();
    assert!((high - 1.0).abs() < 1e-12, "{high}");
    assert!((low + 0.1_f64.sin()).abs() < 1e-12, "{low}");
}

#[test]
fn an_existing_window_inside_the_cutter_counts_as_a_band_edge_inside_it() {
    let accuracy = coarse_accuracy(1e-5);
    let ring =
        primitives::annular_cylinder("ring".into(), Frame3::IDENTITY, 2.7, 3.0, 3.0, accuracy)
            .unwrap();
    let (sin, cos) = 0.7_f64.sin_cos();
    let radial = |height: f64| {
        Frame3::from_axis(
            [2.55 * cos, 2.55 * sin, height],
            [cos, sin, 0.0],
            [-sin, cos, 0.0],
        )
        .unwrap()
    };
    let window = primitives::cylinder("window".into(), radial(1.5), 0.3, 0.6, accuracy).unwrap();
    let pierced = boolean_brep(&ring, &window, BooleanOp::Subtraction, "pierced".into())
        .unwrap()
        .brep;
    let side = |from: [f64; 2], to: [f64; 2]| ProfileEdge::Line { from, to };
    let arch = primitives::arc_edged_extrusion(
        "arch".into(),
        radial(0.0),
        vec![
            side([-0.45, 0.2], [0.45, 0.2]),
            side([0.45, 0.2], [0.45, 2.2]),
            ProfileEdge::Arc {
                center: [0.0, 2.2],
                radius: 0.45,
                start_angle: 0.0,
                sweep_angle: std::f64::consts::PI,
            },
            side([-0.45, 2.2], [-0.45, 0.2]),
        ],
        0.6,
        accuracy,
    )
    .unwrap();
    assert!(band_edges_inside(&pierced, &arch, [0, 1], None).unwrap());
    assert!(!band_edges_inside(&ring, &arch, [0, 1], None).unwrap());
}
