use crate::support::{assert_valid_closed, assert_volume};
use opengeometry::brep::BrepEnvelope;
use opengeometry::operations::modifying::boolean::{multi_tool_boolean, BooleanOp};
use opengeometry_test_support::scenes::wc;
use opengeometry_test_support::stored;
use opengeometry_test_support::volume;
use sha2::{Digest, Sha256};

const SPECIALIZED: [&str; 4] = [
    "separate_boolean",
    "boolean_spheres",
    "subtract_vertical_arc_extrusion",
    "generic_two_sided_cutter_band_subtraction",
];

struct Expected {
    handler: &'static str,
    attempted: Vec<&'static str>,
    counts: [usize; 3],
    volume: f64,
    sha256: &'static str,
}

fn assert_subtract(host: BrepEnvelope, tool: BrepEnvelope, expected: Expected) {
    let outcome = multi_tool_boolean(&host, &[tool], BooleanOp::Subtraction, "wc".into());
    let label = expected.handler;
    assert_eq!(outcome.handlers, [expected.handler], "{label}");
    assert_eq!(outcome.attempted, expected.attempted, "{label}");
    let body = outcome.result.unwrap().brep;
    assert_valid_closed(&body, 1, label);
    let topology = &body.topology;
    assert_eq!(
        [
            topology.faces.len(),
            topology.edges.len(),
            topology.vertices.len()
        ],
        expected.counts,
        "{label}"
    );
    assert_volume(volume::estimate(&body, 1e-5), expected.volume, label);
    if stored::compared() {
        let digest = Sha256::digest(body.to_json().unwrap().as_bytes());
        assert_eq!(format!("{digest:x}"), expected.sha256, "{label}");
    }
}

#[test]
fn the_wc_pan_shell_minus_its_six_arc_bowl_opening_is_the_recorded_prismatic_result() {
    let mut attempted = SPECIALIZED.to_vec();
    attempted.extend([
        "generic_multiple_closed_loops_boolean",
        "generic_single_closed_loop_boolean",
        "generic_non_intersecting_boolean",
        "prismatic_curved_boolean",
    ]);
    assert_subtract(
        wc::pan_shell(),
        wc::bowl_opening(),
        Expected {
            handler: "prismatic_curved_boolean",
            attempted,
            counts: [22, 78, 60],
            volume: wc::PAN_VOLUME,
            sha256: "1dae88747896c1005a22f519077e3e3abf69fd5d015f1be46724fe28c7351f2a",
        },
    );
}

#[test]
fn the_wc_seat_minus_its_round_hole_is_the_recorded_band_result() {
    assert_subtract(
        wc::seat_solid(),
        wc::seat_hole(),
        Expected {
            handler: "generic_two_sided_cutter_band_subtraction",
            attempted: SPECIALIZED.to_vec(),
            counts: [10, 24, 16],
            volume: wc::SEAT_VOLUME,
            sha256: "ab0f88eefe4796a22faf23d55010a033f008c823f931f54093165550c18c3e58",
        },
    );
}
