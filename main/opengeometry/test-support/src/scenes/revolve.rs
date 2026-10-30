use crate::world_graph::named;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{CreateOptions, CreatingOperation, Plane, Primitive};
use serde_json::json;

pub struct RevolveScene {
    pub name: &'static str,
    pub profile: Primitive,
    pub options: CreateOptions,
    pub operation: CreatingOperation,
}

pub fn revolve(profile: &str, axis: Option<([f64; 2], [f64; 2])>) -> CreatingOperation {
    let axis = axis.map(|(point, direction)| json!({ "point": point, "direction": direction }));
    serde_json::from_value(json!({ "kind": "Revolve", "profile": profile, "axis": axis })).unwrap()
}

pub fn upright(og_id: &str) -> CreateOptions {
    CreateOptions {
        plane: Some(Plane {
            origin: None,
            normal: Some([0.0, 0.0, 1.0]),
            x_direction: Some([1.0, 0.0, 0.0]),
        }),
        ..named(og_id)
    }
}

pub fn polygon(points: &[[f64; 2]]) -> Primitive {
    Primitive::Profile {
        start: points[0],
        segments: points[1..]
            .iter()
            .chain(&points[..1])
            .map(|&to| ProfileSegment { to, arc: None })
            .collect(),
        closed: true,
    }
}

pub fn wc_bowl() -> Primitive {
    let line = |to: [f64; 2]| ProfileSegment { to, arc: None };
    let arc = |to: [f64; 2], center: [f64; 2], counter_clockwise: bool| ProfileSegment {
        to,
        arc: Some(SegmentArc {
            center,
            counter_clockwise,
        }),
    };
    Primitive::Profile {
        start: [0.0, 0.0],
        segments: vec![
            line([0.12, 0.0]),
            line([0.19, 0.3]),
            arc([0.19, 0.34], [0.19, 0.32], true),
            line([0.16, 0.34]),
            arc([0.0, 0.18], [0.0, 0.34], false),
            line([0.0, 0.0]),
        ],
        closed: true,
    }
}

pub fn revolve_scenes() -> Vec<RevolveScene> {
    let offset = [[0.0, 0.0], [1.0, 0.0], [1.0, 2.0], [0.0, 2.0]];
    let scene = |name, profile, options, axis| RevolveScene {
        name,
        profile,
        options,
        operation: revolve("profile", axis),
    };
    vec![
        scene("wc-bowl", wc_bowl(), upright("profile"), None),
        scene(
            "ground-tube",
            polygon(&[[1.0, 0.0], [2.0, 0.0], [2.0, 3.0], [1.0, 3.0]]),
            named("profile"),
            None,
        ),
        scene(
            "offset-axis",
            polygon(&offset),
            upright("profile"),
            Some(([-1.0, 0.0], [0.0, 1.0])),
        ),
        scene(
            "crossing-axis",
            polygon(&[[-1.0, 0.0], [1.0, 0.0], [1.0, 1.0], [-1.0, 1.0]]),
            upright("profile"),
            None,
        ),
        scene(
            "touching-axis",
            polygon(&[[0.0, 0.0], [1.0, -1.0], [1.0, 1.0]]),
            upright("profile"),
            None,
        ),
        scene(
            "zero-direction",
            polygon(&offset),
            upright("profile"),
            Some(([-1.0, 0.0], [0.0, 0.0])),
        ),
    ]
}
