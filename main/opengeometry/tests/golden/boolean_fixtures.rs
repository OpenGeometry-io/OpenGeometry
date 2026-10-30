use crate::boolean_case::{boolean, operands};
use crate::fixtures::{ground, moved, standard};
use crate::kernel::{primitives, BooleanOp, BrepEnvelope, GeometryError};
use crate::runner::Case;

type Build = fn() -> Result<BrepEnvelope, GeometryError>;

fn box_host() -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid("box-host".into(), ground(), [3.0; 3], standard())
}

fn box_cutter() -> Result<BrepEnvelope, GeometryError> {
    let frame = ground();
    let origin = frame.point([1.5, 0.5, 0.5]);
    primitives::cuboid(
        "box-cutter".into(),
        moved(frame, origin),
        [3.0; 3],
        standard(),
    )
}

fn box_inner() -> Result<BrepEnvelope, GeometryError> {
    let frame = ground();
    let origin = frame.point([1.0; 3]);
    primitives::cuboid(
        "box-inner".into(),
        moved(frame, origin),
        [1.0; 3],
        standard(),
    )
}

fn sphere_host() -> Result<BrepEnvelope, GeometryError> {
    primitives::sphere("sphere-host".into(), ground(), 2.0, standard())
}

fn sphere_cutter() -> Result<BrepEnvelope, GeometryError> {
    let frame = ground();
    let origin = frame.point([0.7, 0.0, 0.0]);
    primitives::sphere(
        "sphere-cutter".into(),
        moved(frame, origin),
        1.0,
        standard(),
    )
}

const FIXTURES: [(&str, Build, Build, BooleanOp); 5] = [
    ("box-union", box_host, box_cutter, BooleanOp::Union),
    (
        "box-intersection",
        box_host,
        box_cutter,
        BooleanOp::Intersection,
    ),
    ("box-cut", box_host, box_cutter, BooleanOp::Subtraction),
    ("box-cavity", box_host, box_inner, BooleanOp::Subtraction),
    (
        "sphere-cut",
        sphere_host,
        sphere_cutter,
        BooleanOp::Subtraction,
    ),
];

pub(crate) fn cases() -> Vec<Case> {
    FIXTURES
        .iter()
        .map(|&(name, a, b, operation)| {
            let pair = operands(move || Ok((a()?, b()?)));
            boolean(&format!("booleans.fixture.{name}"), name, operation, &pair)
        })
        .collect()
}
