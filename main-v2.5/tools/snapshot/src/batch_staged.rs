use crate::batch_case::{batch_table, both_orders, BatchBuilt, BatchEntry};
use crate::fixtures::{accuracy, at, ground, tilted_about_x, tilted_about_y};
use crate::kernel::{primitives, Accuracy, Frame3};
use crate::planar_shapes::Shape;
use crate::profiles::band;
use crate::runner::Case;

const OPENING: [f64; 3] = [1.0, 0.5, 0.4];

fn staged_accuracy() -> Accuracy {
    accuracy(6.3e-8, 1.575e-8, 0.01, 1e-5)
}

fn host() -> Shape {
    let outer = band(3.0, 2.7, 0.8, -1.6);
    primitives::arc_edged_extrusion(
        "mixed-curved-host".into(),
        ground(),
        outer,
        3.0,
        staged_accuracy(),
    )
}

fn opening(name: &str, frame: Frame3) -> Shape {
    primitives::cuboid(name.into(), frame, OPENING, staged_accuracy())
}

fn rectangular() -> Shape {
    opening("rectangular-opening", at([2.2, 1.0, 1.1]))
}

fn round_opening() -> Shape {
    let frame = Frame3 {
        origin: [2.65, 1.5, 0.0],
        x: [0.0, 0.0, -1.0],
        y: [0.0, 1.0, 0.0],
        z: [1.0, 0.0, 0.0],
    };
    primitives::cylinder("round-opening".into(), frame, 0.5, 0.4, staged_accuracy())
}

fn mixed() -> BatchBuilt {
    Ok((host()?, vec![rectangular()?, round_opening()?]))
}

fn overlapping() -> BatchBuilt {
    let overlapping = opening("overlapping-opening", at([2.3, 1.2, 1.0]))?;
    Ok((host()?, vec![rectangular()?, overlapping, round_opening()?]))
}

fn oblique_planar() -> BatchBuilt {
    let oblique = opening("oblique-opening", tilted_about_y([2.2, 1.0, 0.9], 0.5))?;
    Ok((host()?, vec![oblique, round_opening()?]))
}

fn oblique_planar_pair() -> BatchBuilt {
    let first = opening("oblique-first", tilted_about_y([2.2, 1.0, 0.9], 0.5))?;
    let second = opening("oblique-second", tilted_about_y([2.3, 1.2, 0.8], 0.5))?;
    Ok((host()?, vec![first, second, round_opening()?]))
}

fn pitched_planar() -> BatchBuilt {
    let pitched = opening("pitched-opening", tilted_about_x([2.2, 1.0, 1.1], 0.4))?;
    Ok((host()?, vec![pitched, round_opening()?]))
}

fn skew_planar() -> BatchBuilt {
    let frame = Frame3::from_axis([2.2, 1.0, 1.1], [1.0, 1.0, 1.0], [1.0, 0.0, 0.0])?;
    Ok((
        host()?,
        vec![opening("skew-opening", frame)?, round_opening()?],
    ))
}

fn oblique_round() -> BatchBuilt {
    let frame = Frame3::from_axis([2.4, 1.2, 0.0], [0.8, 0.6, 0.0], [0.0, 0.0, 1.0])?;
    let oblique = primitives::cylinder("oblique-round".into(), frame, 0.2, 0.9, staged_accuracy())?;
    Ok((host()?, vec![rectangular()?, oblique]))
}

const ORDERED: [BatchEntry; 1] = [("staged.mixed-curved-openings", "mixed-curved-cut", mixed)];

const ENTRIES: [BatchEntry; 6] = [
    (
        "staged.overlapping-planar-cutters",
        "mixed-overlap",
        overlapping,
    ),
    (
        "staged.oblique-planar-stage",
        "mixed-oblique-planar",
        oblique_planar,
    ),
    (
        "staged.overlapping-oblique-planar-stage",
        "mixed-oblique-pair",
        oblique_planar_pair,
    ),
    (
        "staged.pitched-planar-stage",
        "mixed-pitched",
        pitched_planar,
    ),
    ("staged.skew-planar-stage", "mixed-skew", skew_planar),
    (
        "staged.oblique-cylinder-stage",
        "mixed-oblique-round",
        oblique_round,
    ),
];

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = both_orders(&ORDERED);
    cases.extend(batch_table(&ENTRIES));
    cases
}
