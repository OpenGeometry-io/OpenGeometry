use super::standard::{
    assert_bar, assert_exact_volume, assert_on_plane, assert_refused, assert_step_surfaces,
};
use opengeometry::world_graph::Primitive;
use opengeometry_test_support::scenes::bar::shell_is_valid;
use std::f64::consts::PI;

fn rounded(size: [f64; 3], radius: f64) -> Primitive {
    Primitive::RoundedBox {
        width: size[0],
        height: size[1],
        depth: size[2],
        radius,
    }
}

#[test]
fn a_rounded_box_is_closed_with_the_exact_volume_less_its_four_rounded_corners() {
    assert_exact_volume(rounded([1.0, 1.0, 1.0], 0.1), 1.0 - (4.0 - PI) * 0.01);
    assert_exact_volume(
        rounded([2.0, 0.5, 0.6], 0.25),
        (1.2 - (4.0 - PI) * 0.0625) * 0.5,
    );
}

#[test]
fn invalid_rounded_boxes_are_invalid_parameter_naming_the_value() {
    let corner = "rounded box radius must be below half its width and depth";
    let size = |name: &str| format!("{name} must exceed four times geometric tolerance");
    for (primitive, message) in [
        (rounded([f64::NAN, 1.0, 1.0], 0.1), size("width")),
        (rounded([1.0, 0.0, 1.0], 0.1), size("height")),
        (rounded([1.0, 1.0, -1.0], 0.1), size("depth")),
        (rounded([1.0, 1.0, 1.0], 0.0), size("radius")),
        (rounded([1.0, 1.0, 1.0], 0.5), corner.to_string()),
        (rounded([2.0, 1.0, 0.8], 0.45), corner.to_string()),
        (rounded([0.8, 1.0, 2.0], 0.4), corner.to_string()),
    ] {
        assert_refused(primitive, &message);
    }
}

#[test]
fn rounded_box_json_parses_its_fields() {
    let json = r#"{"kind":"RoundedBox","width":1,"height":2,"depth":3,"radius":0.25}"#;
    let parsed = serde_json::from_str::<Primitive>(json).unwrap();
    assert!(matches!(
        parsed,
        Primitive::RoundedBox { width, height, depth, radius }
            if width == 1.0 && height == 2.0 && depth == 3.0 && radius == 0.25
    ));
}

#[test]
fn a_rounded_box_on_a_plane_keeps_width_along_x_direction_and_height_along_the_normal() {
    let corners = [[1.0, 2.4, 3.5], [2.0, 1.7, 2.4], [1.0, 1.6, 2.5]];
    assert_on_plane(
        rounded([1.2, 1.0, 0.8], 0.1),
        0.96 - (4.0 - PI) * 0.01,
        &corners,
    );
}

#[test]
fn a_rounded_box_passes_the_boolean_bar_with_a_box_and_a_cylinder() {
    assert_bar(
        rounded([1.0, 1.0, 1.0], 0.1),
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
fn a_rounded_box_shells_to_a_valid_closed_solid() {
    assert!(shell_is_valid(&rounded([1.0, 1.0, 1.0], 0.1), 0.03));
}

#[test]
fn a_rounded_box_exports_one_solid_with_four_cylindrical_faces() {
    assert_step_surfaces(rounded([1.0, 1.0, 1.0], 0.1), 4, 6);
}
