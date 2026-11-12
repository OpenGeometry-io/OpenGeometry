use super::standard::{
    assert_bar, assert_exact_volume, assert_on_plane, assert_refused, assert_step_surfaces,
};
use opengeometry::world_graph::Primitive;
use opengeometry_test_support::scenes::bar::shell_is_valid;
use std::f64::consts::PI;

fn slot(length: f64, width: f64, height: f64) -> Primitive {
    Primitive::SlotPrism {
        length,
        width,
        height,
    }
}

#[test]
fn a_slot_prism_is_closed_with_the_exact_stadium_volume() {
    assert_exact_volume(slot(1.0, 0.5, 1.0), 0.5 + PI * 0.0625);
    assert_exact_volume(slot(3.0, 0.2, 0.4), (0.6 + PI * 0.01) * 0.4);
}

#[test]
fn invalid_slot_prisms_are_invalid_parameter_naming_the_value() {
    for (primitive, message) in [
        (slot(0.0, 0.5, 1.0), "length"),
        (slot(f64::INFINITY, 0.5, 1.0), "length"),
        (slot(1.0, f64::NAN, 1.0), "width"),
        (slot(1.0, 1e-9, 1.0), "width"),
        (slot(1.0, 6e-8, 1.0), "width"),
        (slot(1.0, 0.5, -1.0), "height"),
    ] {
        let message = format!("{message} must exceed four times geometric tolerance");
        assert_refused(primitive, &message);
    }
}

#[test]
fn slot_prism_json_parses_its_fields() {
    let json = r#"{"kind":"SlotPrism","length":1,"width":0.5,"height":2}"#;
    let parsed = serde_json::from_str::<Primitive>(json).unwrap();
    assert!(matches!(
        parsed,
        Primitive::SlotPrism { length, width, height } if length == 1.0 && width == 0.5 && height == 2.0
    ));
}

#[test]
fn a_slot_prism_on_a_plane_runs_its_length_along_the_x_direction() {
    let corners = [[1.0, 2.25, 3.5], [2.0, 1.75, 2.5], [1.0, 1.75, 2.5]];
    assert_on_plane(slot(1.0, 0.5, 1.0), 0.5 + PI * 0.0625, &corners);
}

#[test]
fn a_slot_prism_passes_the_boolean_bar_with_a_box_and_a_cylinder() {
    assert_bar(
        slot(1.0, 0.5, 1.0),
        [
            "A/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "A/cyl S=prismatic_curved_boolean U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "B/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "B/cyl S=prismatic_curved_boolean U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "T/box S=subtract_vertical_arc_extrusion U=prismatic_curved_boolean I=prismatic_curved_boolean",
            "T/cyl S=generic_two_sided_cutter_band_subtraction U=prismatic_curved_boolean I=prismatic_curved_boolean",
        ],
    );
}

#[test]
fn a_slot_prism_shells_to_a_valid_closed_solid() {
    assert!(shell_is_valid(&slot(1.0, 0.5, 1.0), 0.03));
}

#[test]
fn a_slot_prism_exports_one_solid_with_two_cylindrical_faces() {
    assert_step_surfaces(slot(1.0, 0.5, 1.0), 2, 4);
}
