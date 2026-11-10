use super::standard::{
    assert_bar, assert_exact_volume, assert_on_plane, assert_refused, assert_step_surfaces,
};
use opengeometry::world_graph::Primitive;
use opengeometry_test_support::scenes::bar::shell_is_valid;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn pipe(radii: [f64; 2], height: f64, angles: [f64; 2]) -> Primitive {
    Primitive::PipeSegment {
        outer_radius: radii[0],
        inner_radius: radii[1],
        height,
        start_angle: angles[0],
        sweep_angle: angles[1],
    }
}

#[test]
fn a_pipe_segment_is_closed_with_the_exact_ring_sector_volume() {
    for sweep in [FRAC_PI_2, -1.2, 5.0] {
        let expected = sweep.abs() / 2.0 * (0.25 - 0.09) * 2.0;
        assert_exact_volume(pipe([0.5, 0.3], 2.0, [0.3, sweep]), expected);
    }
}

#[test]
fn invalid_pipe_segments_are_invalid_parameter_naming_the_value() {
    let size = |name: &str| format!("{name} must exceed four times geometric tolerance");
    let sweep = "pipe segment sweep must be nonzero and less than one turn".to_string();
    let order = "pipe segment inner radius must be below its outer radius".to_string();
    for (primitive, message) in [
        (pipe([f64::NAN, 0.3], 1.0, [0.0, 1.0]), size("outerRadius")),
        (pipe([0.5, 0.0], 1.0, [0.0, 1.0]), size("innerRadius")),
        (pipe([0.5, 0.3], 0.0, [0.0, 1.0]), size("height")),
        (pipe([0.5, 0.3], 1.0, [0.0, 0.0]), sweep.clone()),
        (pipe([0.5, 0.3], 1.0, [0.0, TAU]), sweep.clone()),
        (pipe([0.5, 0.3], 1.0, [f64::INFINITY, 1.0]), sweep),
        (pipe([0.3, 0.5], 1.0, [0.0, 1.0]), order.clone()),
        (pipe([0.5, 0.5 - 1e-8], 1.0, [0.0, 1.0]), order),
    ] {
        assert_refused(primitive, &message);
    }
}

#[test]
fn pipe_segment_json_uses_camel_case_fields() {
    let json = r#"{"kind":"PipeSegment","outerRadius":0.5,"innerRadius":0.3,"height":1,"startAngle":0,"sweepAngle":1}"#;
    let parsed = serde_json::from_str::<Primitive>(json).unwrap();
    assert!(matches!(
        parsed,
        Primitive::PipeSegment { outer_radius, inner_radius, sweep_angle, .. }
            if outer_radius == 0.5 && inner_radius == 0.3 && sweep_angle == 1.0
    ));
    let json = r#"{"kind":"PipeSegment","outer_radius":0.5,"inner_radius":0.3,"height":1,"start_angle":0,"sweep_angle":1}"#;
    assert!(serde_json::from_str::<Primitive>(json).is_err());
}

#[test]
fn a_pipe_segment_on_a_plane_sweeps_counter_clockwise_from_the_x_direction() {
    let corners = [[1.0, 2.0, 3.5], [2.0, 1.7, 3.0], [1.0, 1.5, 3.0]];
    let expected = PI / 4.0 * (0.25 - 0.09);
    assert_on_plane(pipe([0.5, 0.3], 1.0, [0.0, FRAC_PI_2]), expected, &corners);
}

#[test]
fn a_pipe_segment_passes_the_boolean_bar_with_a_box_and_a_cylinder() {
    assert_bar(
        pipe([0.5, 0.3], 1.0, [0.0, FRAC_PI_2]),
        [
            "A/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "A/cyl S=prismatic_curved_boolean U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "B/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "B/cyl S=prismatic_curved_boolean U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "T/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "T/cyl S=prismatic_curved_boolean U=prismatic_curved_boolean I=prismatic_curved_boolean",
        ],
    );
}

#[test]
fn a_pipe_segment_shells_to_a_valid_closed_solid() {
    assert!(shell_is_valid(
        &pipe([0.5, 0.3], 1.0, [0.0, FRAC_PI_2]),
        0.03
    ));
}

#[test]
fn a_pipe_segment_exports_one_solid_with_two_cylindrical_faces() {
    assert_step_surfaces(pipe([0.5, 0.3], 1.0, [0.0, FRAC_PI_2]), 2, 4);
}
