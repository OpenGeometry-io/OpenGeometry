use crate::boolean_case::{families, stage, table, Built, Entry, Family};
use crate::fixtures::{fine, ground, moved, rectangle, turned_about_z, upright};
use crate::kernel::{primitives, BooleanOp};
use crate::planar_shapes::{angled, extrusion, framed, opening, span, square, wall, Shape};
use crate::runner::Case;
use std::f64::consts::FRAC_PI_6;

const SUBTRACT: BooleanOp = BooleanOp::Subtraction;

fn coextensive() -> Built {
    let a = extrusion("a", rectangle([0.0, 0.0], [4.0, 4.0]))?;
    Ok((a, extrusion("b", rectangle([2.0, -1.0], [5.0, 2.0]))?))
}

fn axial_base() -> Shape {
    span("a", 0.0, 4.0, rectangle([0.0, 0.0], [4.0, 4.0]))
}

fn axial() -> Built {
    let reversed = vec![[4.0, 4.0], [0.0, 4.0], [0.0, 0.0], [4.0, 0.0]];
    Ok((axial_base()?, span("b", 2.0, 4.0, reversed)?))
}

fn axial_embedded() -> Built {
    let embedded = span("embedded", 1.0, 2.0, rectangle([0.0, 0.0], [4.0, 4.0]))?;
    Ok((axial_base()?, embedded))
}

fn axial_disjoint() -> Built {
    let disjoint = span("disjoint", 6.0, 2.0, rectangle([0.0, 0.0], [4.0, 4.0]))?;
    Ok((axial_base()?, disjoint))
}

fn square_host() -> Shape {
    extrusion("host", rectangle([0.0, 0.0], [10.0, 10.0]))
}

fn through_hole() -> Built {
    Ok((
        square_host()?,
        extrusion("inner", rectangle([4.0, 4.0], [6.0, 6.0]))?,
    ))
}

fn band_split() -> Built {
    Ok((
        square_host()?,
        extrusion("band", rectangle([-1.0, 4.0], [11.0, 6.0]))?,
    ))
}

fn lower_cutout() -> Shape {
    opening("lower_cutout", 0.0, 2.1, 2.0, 3.0)
}

fn raised_cutout() -> Shape {
    opening("raised_cutout", 0.8, 1.2, 6.0, 7.0)
}

fn flush_lower() -> Built {
    Ok((wall("host", 10.0)?, lower_cutout()?))
}

fn flush_chained() -> Built {
    let (host, cutter) = flush_lower()?;
    let cut = stage(&host, &cutter, SUBTRACT, "one-lower_cutout")?;
    Ok((cut, raised_cutout()?))
}

fn mitered_first() -> Built {
    let outer = vec![
        [-0.16, -0.14],
        [0.16, -0.46],
        [13.84, -0.46],
        [14.16, -0.14],
    ];
    let host = span("mitered-host", 0.0, 3.8, outer)?;
    let lower = vec![[1.2, -0.14], [2.8, -0.14], [2.8, -0.46], [1.2, -0.46]];
    Ok((host, span("lower_cutout", 0.0, 2.45, lower)?))
}

fn mitered_chained() -> Built {
    let (host, cutter) = mitered_first()?;
    let first = stage(&host, &cutter, SUBTRACT, "lower_cutout-cut")?;
    let raised = vec![
        [3.625, -0.14],
        [5.175, -0.14],
        [5.175, -0.46],
        [3.625, -0.46],
    ];
    Ok((first, span("raised_cutout", 0.3, 2.8, raised)?))
}

fn angled_lower() -> Built {
    Ok((angled("angled-host")?, lower_cutout()?))
}

fn angled_chained() -> Built {
    let (host, cutter) = angled_lower()?;
    let cut = stage(&host, &cutter, SUBTRACT, "angled-cut")?;
    Ok((cut, raised_cutout()?))
}

fn angled_full_height() -> Built {
    let through = extrusion("through-lower_cutout", rectangle([4.0, 0.0], [5.0, 0.3]))?;
    Ok((angled("angled-host")?, through))
}

fn angled_twice() -> Built {
    let first = opening("first", 0.0, 2.1, 2.0, 3.0)?;
    let once = stage(&angled("angled-host")?, &first, SUBTRACT, "once")?;
    Ok((once, opening("second", 0.0, 2.1, 2.5, 3.5)?))
}

fn layered_cavity() -> Built {
    let outer = vec![
        [0.0, 0.0],
        [10.0, 0.0],
        [10.0, 10.0],
        [0.3, 10.0],
        [0.0, 9.8],
    ];
    let cavity = span("cavity", 1.0, 1.0, rectangle([4.0, 4.0], [6.0, 6.0]))?;
    Ok((extrusion("angled-host", outer)?, cavity))
}

fn angled_rotated() -> Built {
    let frame = turned_about_z([2.5, 0.15, 0.0], FRAC_PI_6);
    let cutter = framed("rotated-cutter", frame, square(0.5), 2.1)?;
    Ok((angled("angled-host")?, cutter))
}

fn round_through() -> Built {
    let frame = moved(ground(), [2.0, -0.5, 1.5]);
    let cutter = primitives::cylinder("round-opening".into(), frame, 0.5, 1.3, fine())?;
    Ok((wall("round-host", 4.0)?, cutter))
}

fn arched_cap() -> Built {
    let frame = moved(ground(), [2.0, -0.5, 2.0]);
    let cap = primitives::cylinder("arched-cap".into(), frame, 0.5, 1.3, fine())?;
    Ok((wall("arched-host", 4.0)?, cap))
}

fn arched_chained() -> Built {
    let (host, cap) = arched_cap()?;
    let capped = stage(&host, &cap, SUBTRACT, "cap")?;
    let lower = rectangle([1.5, 0.5], [2.5, 2.0]);
    Ok((
        capped,
        framed("arched-lower", upright([0.0, 0.8, 0.0]), lower, 1.3)?,
    ))
}

const FAMILIES: [Family; 2] = [
    ("coextensive", "coextensive", coextensive),
    ("axial", "axial", axial),
];

const ENTRIES: [Entry; 18] = [
    ("through-hole", "cavity", SUBTRACT, through_hole),
    ("band-split", "split", SUBTRACT, band_split),
    ("axial-embedded", "axial-split", SUBTRACT, axial_embedded),
    (
        "axial-disjoint.union",
        "axial-disjoint",
        BooleanOp::Union,
        axial_disjoint,
    ),
    (
        "axial-disjoint.intersection",
        "axial-empty",
        BooleanOp::Intersection,
        axial_disjoint,
    ),
    (
        "flush-lower-cutout",
        "one-lower_cutout",
        SUBTRACT,
        flush_lower,
    ),
    (
        "flush-chained",
        "lower_cutout-raised_cutout",
        SUBTRACT,
        flush_chained,
    ),
    ("mitered-first", "lower_cutout-cut", SUBTRACT, mitered_first),
    (
        "mitered-chained",
        "raised_cutout-cut",
        SUBTRACT,
        mitered_chained,
    ),
    ("angled-lower-cutout", "angled-cut", SUBTRACT, angled_lower),
    ("angled-chained", "angled-chain", SUBTRACT, angled_chained),
    (
        "angled-full-height",
        "split-angled",
        SUBTRACT,
        angled_full_height,
    ),
    ("angled-overlapping-twice", "twice", SUBTRACT, angled_twice),
    ("layered-cavity", "cavity-result", SUBTRACT, layered_cavity),
    (
        "angled-rotated-cutter",
        "rotated-cut",
        SUBTRACT,
        angled_rotated,
    ),
    (
        "round-through-cutter",
        "round-host-cut",
        SUBTRACT,
        round_through,
    ),
    ("arched-cap", "cap", SUBTRACT, arched_cap),
    ("arched-chained", "arch", SUBTRACT, arched_chained),
];

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = families("booleans.planar", &FAMILIES);
    cases.extend(table("booleans.planar", &ENTRIES));
    cases
}
