use super::outline::arc_polysolid;
use super::stroke::stroked_polysolid;
use super::{PolysolidJustify, PolysolidPath};
use crate::brep::{Accuracy, BrepEnvelope, GROUND};
use crate::primitives::profile_wire::ProfileSegment;

fn straight(points: &[[f64; 2]], closed: bool) -> PolysolidPath {
    PolysolidPath {
        start: points[0],
        segments: points[1..]
            .iter()
            .map(|&to| ProfileSegment { to, arc: None })
            .collect(),
        closed,
    }
}

fn corners(body: &BrepEnvelope) -> Vec<[f64; 3]> {
    body.topology
        .vertices
        .iter()
        .map(|vertex| vertex.position)
        .collect()
}

fn on_segment(point: [f64; 3], a: [f64; 3], b: [f64; 3]) -> bool {
    let along = (0..3).map(|axis| (b[axis] - a[axis]) * (point[axis] - a[axis]));
    let length = (0..3).map(|axis| (b[axis] - a[axis]).powi(2)).sum::<f64>();
    let t = along.sum::<f64>() / length;
    (0.0..=1.0).contains(&t)
        && (0..3).all(|axis| (a[axis] + t * (b[axis] - a[axis]) - point[axis]).abs() <= 1e-9)
}

fn same_outline(stroke: &[[f64; 3]], outline: &[[f64; 3]]) -> bool {
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|axis| (a[axis] - b[axis]).abs() <= 1e-9);
    outline.iter().all(|&o| stroke.iter().any(|&s| near(o, s)))
        && stroke.iter().all(|&s| {
            outline
                .iter()
                .any(|&a| outline.iter().any(|&b| !near(a, b) && on_segment(s, a, b)))
        })
}

#[test]
fn straight_path_through_the_arc_outline_matches_the_stroke() {
    let justified = [
        PolysolidJustify::Centre,
        PolysolidJustify::Left,
        PolysolidJustify::Right,
    ];
    let cases = [
        (
            straight(&[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0]], false),
            &justified[..],
        ),
        (
            straight(
                &[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0], [0.0, 0.0]],
                true,
            ),
            &justified[..],
        ),
        (
            straight(&[[0.0, 0.0], [3.0, 0.0], [1.5, 2.6]], false),
            &justified[..],
        ),
        (
            straight(&[[0.0, 0.0], [3.0, 0.0], [0.0, 0.5]], false),
            &justified[..1],
        ),
    ];
    for (path, justifies) in &cases {
        for &justify in *justifies {
            let sizes = [0.2, 1.0];
            let accuracy = Accuracy::STANDARD;
            let stroke =
                stroked_polysolid("s".into(), GROUND, path, justify, sizes, accuracy).unwrap();
            let outline =
                arc_polysolid("o".into(), GROUND, path, justify, sizes, accuracy).unwrap();
            let label = format!("{path:?} {justify:?}");
            assert!(
                same_outline(&corners(&stroke), &corners(&outline)),
                "{label}"
            );
        }
    }
}
