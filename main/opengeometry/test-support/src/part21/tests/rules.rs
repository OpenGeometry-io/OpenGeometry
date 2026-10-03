use super::base::{base, real, UNCERTAINTY};
use super::{appended, assert_error, assert_rejected, edited, parsed, without};
use crate::part21::Document;
use std::collections::BTreeMap;
use std::f64::consts::FRAC_PI_3;

#[test]
fn base_text_parses_under_every_rule() {
    let base = base();
    let document = parsed(&base.text);
    assert_eq!(document.entity_count(), base.entities);
    assert_eq!(document.count("EDGE_CURVE"), 5);
}

#[test]
fn vertex_off_its_curve_by_more_than_the_file_uncertainty_is_rejected() {
    let base = base();
    let moved = |factor: f64| {
        let to = format!(",{}))", real(factor * UNCERTAINTY));
        edited(
            &base.text,
            base.cylinder.seam_origin,
            &format!(",{}))", real(0.0)),
            &to,
        )
    };
    parsed(&moved(0.95));
    assert_rejected(&moved(1.05), "misses curve");
}

#[test]
fn pcurve_sample_beyond_the_file_uncertainty_is_rejected() {
    let base = base();
    let grown = |factor: f64| {
        let to = format!(",{})", real(1.0 + factor * UNCERTAINTY));
        edited(
            &base.text,
            base.cylinder.lower.conic,
            &format!(",{})", real(1.0)),
            &to,
        )
    };
    parsed(&grown(0.95));
    assert_rejected(&grown(1.05), "misses 3D curve");
}

#[test]
fn ellipse_distance_is_the_closest_point_distance() {
    let document = Document {
        entities: BTreeMap::from([
            (1, "CARTESIAN_POINT('',(0.0,0.0,0.0))".to_string()),
            (2, "DIRECTION('',(0.0,0.0,1.0))".to_string()),
            (3, "DIRECTION('',(1.0,0.0,0.0))".to_string()),
            (4, "AXIS2_PLACEMENT_3D('',#1,#2,#3)".to_string()),
            (5, "ELLIPSE('',#4,2.0,1.0)".to_string()),
        ]),
    };
    let distance = |point: [f64; 3]| document.curve_distance(5, point).unwrap();
    assert!((distance([2.25, 0.0, 0.0]) - 0.25).abs() < 1e-12);
    let (sin, cos) = FRAC_PI_3.sin_cos();
    let normal = [cos, 2.0 * sin];
    let length = normal[0].hypot(normal[1]);
    let point = [
        2.0 * cos + 0.25 * normal[0] / length,
        sin + 0.25 * normal[1] / length,
        0.0,
    ];
    assert!((distance(point) - 0.25).abs() < 1e-12);
}

#[test]
fn vertex_off_an_ellipse_by_more_than_the_uncertainty_is_rejected() {
    let base = base();
    let moved = |factor: f64| {
        let to = format!("({},", real(12.0 + factor * UNCERTAINTY));
        edited(
            &base.text,
            base.lens.vertex_point,
            &format!("({},", real(12.0)),
            &to,
        )
    };
    parsed(&moved(0.95));
    assert_rejected(&moved(1.5), "misses curve");
}

#[test]
fn file_without_application_context_is_rejected() {
    let base = base();
    let text = without(&base.text, base.products.application);
    assert_rejected(&text, "exactly one APPLICATION_CONTEXT");
}

#[test]
fn file_without_a_2d_context_is_rejected() {
    let text = without(&base().text, 1);
    assert_rejected(&text, "exactly one 2D GEOMETRIC_REPRESENTATION_CONTEXT");
}

#[test]
fn shape_representation_must_reference_the_3d_context() {
    let base = base();
    let from = format!("),#{})", base.context.context3);
    let text = edited(&base.text, base.representations[0], &from, "),#1)");
    assert_rejected(&text, "does not reference the 3D context");
}

#[test]
fn definitional_representation_must_reference_the_2d_context() {
    let base = base();
    let to = format!("),#{})", base.context.context3);
    let text = edited(&base.text, base.cylinder.lower.definition, "),#1)", &to);
    assert_rejected(&text, "does not reference the 2D context");
}

#[test]
fn product_contexts_must_reference_the_application_context() {
    let base = base();
    let from = format!("'',#{},", base.products.application);
    let to = format!("'',#{},", base.context.context3);
    let text = edited(&base.text, base.products.product_context, &from, &to);
    assert_rejected(&text, "does not reference the APPLICATION_CONTEXT");
}

#[test]
fn unit_assignment_must_name_length_angle_and_solid_angle_units() {
    let base = base();
    let from = format!(",#{}))", base.context.solid_angle);
    let to = format!(",#{}))", base.context.angle);
    let text = edited(&base.text, base.context.context3, &from, &to);
    assert_rejected(&text, "GLOBAL_UNIT_ASSIGNED_CONTEXT does not name");
}

#[test]
fn uncertainty_must_name_the_length_unit_and_belong_to_the_3d_context() {
    let base = base();
    let context = &base.context;
    let from = format!("),#{},", context.length);
    let to = format!("),#{},", context.angle);
    let text = edited(&base.text, context.uncertainty, &from, &to);
    assert_rejected(&text, "does not name the length unit");
    let from = format!("((#{}))", context.uncertainty);
    let to = format!("((#{}))", context.length);
    let text = edited(&base.text, context.context3, &from, &to);
    assert_rejected(
        &text,
        "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT does not reference",
    );
}

#[test]
fn length_unit_must_match_the_requested_unit() {
    let base = base();
    let prefixed = |prefix: &str| {
        let to = format!("SI_UNIT({prefix},");
        parsed(&edited(&base.text, base.context.length, "SI_UNIT($,", &to))
    };
    let millimetre = prefixed(".MILLI.");
    assert_eq!(millimetre.length_unit(), Ok("millimetre"));
    assert_eq!(millimetre.check_length_unit("millimetre"), Ok(()));
    assert_error(millimetre.check_length_unit("metre"), "millimetre");
    assert_eq!(parsed(&base.text).length_unit(), Ok("metre"));
    assert!(prefixed(".KILO.").length_unit().is_err());
}

#[test]
fn brep_with_voids_without_a_void_is_rejected() {
    let base = base();
    let from = format!("(#{})", base.oriented_shell);
    let text = edited(&base.text, base.solids[0], &from, "()");
    assert_rejected(&text, "has no void");
}

#[test]
fn void_must_be_an_oriented_closed_shell_over_a_closed_shell() {
    let base = base();
    let from = format!("(#{})", base.oriented_shell);
    let to = format!("(#{})", base.lens.shell);
    let text = edited(&base.text, base.solids[0], &from, &to);
    assert_rejected(&text, "is not an ORIENTED_CLOSED_SHELL");
    let from = format!("*,#{},", base.lens.shell);
    let to = format!("*,#{},", base.lens.faces[0]);
    let text = edited(&base.text, base.oriented_shell, &from, &to);
    assert_rejected(&text, "does not orient a CLOSED_SHELL");
}

#[test]
fn oriented_closed_shell_must_be_used_by_exactly_one_solid() {
    let base = base();
    let from = format!("MANIFOLD_SOLID_BREP('b-0',#{})", base.spline.shell);
    let to = format!(
        "BREP_WITH_VOIDS('b-0',#{},(#{}))",
        base.spline.shell, base.oriented_shell
    );
    let text = edited(&base.text, base.solids[1], &from, &to);
    assert_rejected(&text, "is used by 2 solids");
    let expression = format!("ORIENTED_CLOSED_SHELL('',*,#{},.T.)", base.lens.shell);
    assert_rejected(&appended(&base.text, &expression), "is used by 0 solids");
}
