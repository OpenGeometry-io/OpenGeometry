use crate::brep::{Accuracy, Frame3, GeometryError};
use crate::operations::modifying::boolean::shell::shell_brep;
use crate::operations::modifying::boolean::types::BooleanResult;
use crate::primitives::{self, ProfileEdge};
use std::f64::consts::{FRAC_PI_2, PI};

const WALL: f64 = 0.3;

fn lines(points: &[[f64; 2]]) -> Vec<ProfileEdge> {
    points
        .windows(2)
        .map(|pair| ProfileEdge::Line {
            from: pair[0],
            to: pair[1],
        })
        .collect()
}

fn rounded(points: &[[f64; 2]]) -> Vec<ProfileEdge> {
    let mut edges = lines(points);
    edges.push(ProfileEdge::Arc {
        center: [0.5, 0.5],
        radius: 0.5,
        start_angle: PI,
        sweep_angle: FRAC_PI_2,
    });
    edges
}

fn slot(neck: f64) -> Vec<ProfileEdge> {
    let floor = 2.0 * WALL + neck;
    rounded(&[
        [0.5, 0.0],
        [6.0, 0.0],
        [6.0, 4.0],
        [3.2, 4.0],
        [3.2, floor],
        [2.8, floor],
        [2.8, 4.0],
        [0.0, 4.0],
        [0.0, 0.5],
    ])
}

fn notch(neck: f64) -> Vec<ProfileEdge> {
    rounded(&[
        [0.5, 0.0],
        [6.0, 0.0],
        [6.0, 4.0],
        [3.5, 4.0],
        [3.0, 2.0 * WALL + neck],
        [2.5, 4.0],
        [0.0, 4.0],
        [0.0, 0.5],
    ])
}

fn tooth(neck: f64) -> Vec<ProfileEdge> {
    let half = WALL + neck / 2.0;
    rounded(&[
        [0.5, 0.0],
        [6.0, 0.0],
        [6.0, 4.0],
        [3.0 + half, 4.0],
        [3.0, 7.0],
        [3.0 - half, 4.0],
        [0.0, 4.0],
        [0.0, 0.5],
    ])
}

fn pocket(neck: f64) -> Vec<ProfileEdge> {
    let radius = 4.0 - 2.0 * WALL - neck;
    let mut edges = lines(&[[0.5, 0.0], [10.0, 0.0], [10.0, 4.0], [5.0 + radius, 4.0]]);
    edges.push(ProfileEdge::Arc {
        center: [5.0, 4.0],
        radius,
        start_angle: 0.0,
        sweep_angle: -PI,
    });
    edges.extend(rounded(&[[5.0 - radius, 4.0], [0.0, 4.0], [0.0, 0.5]]));
    edges
}

fn shelled(
    outer: Vec<ProfileEdge>,
    holes: Vec<Vec<ProfileEdge>>,
) -> Result<BooleanResult, GeometryError> {
    let input = primitives::arc_edged_extrusion_with_holes(
        "necked".into(),
        Frame3::IDENTITY,
        outer,
        holes,
        3.0,
        Accuracy::STANDARD,
    )
    .unwrap();
    shell_brep(&input, WALL, "necked-shell".into())
}

fn holed(neck: f64) -> Result<BooleanResult, GeometryError> {
    let radius = 2.0 - WALL - 0.1 - neck;
    let hole = [0.0, PI]
        .map(|start_angle| ProfileEdge::Arc {
            center: [3.0, 2.2],
            radius,
            start_angle,
            sweep_angle: -PI,
        })
        .to_vec();
    let outer = lines(&[[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0], [0.0, 0.0]]);
    shelled(outer, vec![hole])
}

fn assert_refused(result: Result<BooleanResult, GeometryError>, label: &str) {
    assert!(
        matches!(result, Err(GeometryError::UnresolvedIntersection(_))),
        "{label} gave {:?}",
        result.map(|result| result.brep.topology.faces.len())
    );
}

fn assert_built(result: Result<BooleanResult, GeometryError>, label: &str) {
    let result = result.unwrap_or_else(|error| panic!("{label} gave {error:?}"));
    result.brep.validate().unwrap();
    assert_eq!(result.brep.solids.len(), 1, "{label}");
    assert_eq!(result.brep.solids[0].cavity_shells.len(), 1, "{label}");
}

#[test]
fn an_inset_neck_between_edges_that_do_not_meet_refuses_below_four_geometric_tolerances() {
    for neck in [0.0, 1e-10, 3e-8, 1e-7, 1e-4] {
        let cases = [
            ("notch", shelled(notch(neck), Vec::new())),
            ("pocket", shelled(pocket(neck), Vec::new())),
            ("hole", holed(neck)),
        ];
        for (name, result) in cases {
            let label = format!("{name} with a neck of {neck}");
            if neck < 4.0 * Accuracy::STANDARD.geometric {
                assert_refused(result, &label);
            } else {
                assert_built(result, &label);
            }
        }
    }
}

#[test]
fn a_parallel_inset_neck_within_the_inset_tolerance_splits_and_refuses() {
    for neck in [0.0, 1e-10, 3e-8, 1e-7] {
        assert_refused(shelled(slot(neck), Vec::new()), &format!("slot {neck}"));
    }
    assert_built(shelled(slot(1e-4), Vec::new()), "slot 1e-4");
}

#[test]
fn a_narrow_tooth_whose_inset_sides_cross_before_its_tip_builds() {
    for neck in [0.0, 1e-10, 3e-8, 1e-7, 1e-4] {
        assert_built(shelled(tooth(neck), Vec::new()), &format!("tooth {neck}"));
    }
}
