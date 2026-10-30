use crate::boolean_case::{boolean, operands, stage, table, Built, Entry};
use crate::fixtures::{rectangle, tilted_about_x, tilted_about_y};
use crate::kernel::BooleanOp;
use crate::planar_shapes::{angled, extrusion, framed, holed, oblique, span, square, wall};
use crate::runner::Case;

const SUBTRACT: BooleanOp = BooleanOp::Subtraction;

fn tall_cutter_profile() -> Vec<[f64; 2]> {
    rectangle([-0.5, -2.0], [0.5, 2.0])
}

fn single() -> Built {
    let cutter = oblique("oblique-cutter", [2.5, 0.15, 0.0], 1.0, square(0.5))?;
    Ok((angled("oblique-host")?, cutter))
}

fn chained() -> Built {
    let first = oblique("first-oblique", [2.5, 0.15, 0.0], 1.0, square(0.5))?;
    let once = stage(&angled("chained-oblique-host")?, &first, SUBTRACT, "once")?;
    Ok((
        once,
        oblique("second-oblique", [5.0, 0.15, 0.0], -1.0, square(0.5))?,
    ))
}

fn disconnected() -> Built {
    let through = span("through", -1.0, 5.0, rectangle([3.0, -1.0], [4.0, 1.0]))?;
    let split = stage(&wall("split-host", 10.0)?, &through, SUBTRACT, "split")?;
    Ok((
        split,
        oblique("split-oblique", [6.0, 0.15, 0.0], 1.0, square(0.5))?,
    ))
}

fn profile_hole() -> Built {
    let hole = rectangle([2.0, 1.0], [3.0, 2.0]);
    let host = holed(
        "profile-hole-host",
        0.0,
        3.0,
        rectangle([0.0, 0.0], [10.0, 3.0]),
        vec![hole],
    )?;
    Ok((
        host,
        oblique("pocket-oblique", [6.0, 1.5, 0.0], 1.0, square(0.5))?,
    ))
}

fn nonconvex() -> Built {
    let outer = vec![
        [-1.0, -1.0],
        [1.0, -1.0],
        [1.0, -0.05],
        [0.0, -0.05],
        [0.0, 1.0],
        [-1.0, 1.0],
    ];
    let cutter = oblique("nonconvex-cutter", [5.0, 0.15, 0.0], 1.0, outer)?;
    Ok((wall("nonconvex-host", 10.0)?, cutter))
}

fn after_cavity() -> Built {
    let host = extrusion("cavity-host", rectangle([0.0, 0.0], [10.0, 3.0]))?;
    let cavity = span(
        "internal-cavity",
        1.0,
        1.0,
        rectangle([2.0, 1.0], [3.0, 2.0]),
    )?;
    let host = stage(&host, &cavity, SUBTRACT, "cavity-host")?;
    Ok((
        host,
        oblique(
            "oblique-after-cavity",
            [6.0, 1.5, 0.0],
            1.0,
            tall_cutter_profile(),
        )?,
    ))
}

fn remote_cavity() -> Built {
    let left = extrusion("left-cavity-host", rectangle([0.0, 0.0], [4.0, 3.0]))?;
    let cavity = span("left-cavity", 1.0, 1.0, rectangle([1.0, 1.0], [2.0, 2.0]))?;
    let left = stage(&left, &cavity, SUBTRACT, "left-with-cavity")?;
    let right = extrusion("right-host", rectangle([6.0, 0.0], [10.0, 3.0]))?;
    let host = stage(&left, &right, BooleanOp::Union, "two-components")?;
    Ok((
        host,
        oblique(
            "right-oblique-cutter",
            [8.5, 1.5, 0.0],
            1.0,
            tall_cutter_profile(),
        )?,
    ))
}

const ENTRIES: [Entry; 7] = [
    ("cutter", "oblique-cut", SUBTRACT, single),
    ("chained", "twice", SUBTRACT, chained),
    (
        "disconnected-host",
        "split-oblique-result",
        SUBTRACT,
        disconnected,
    ),
    ("profile-hole-host", "two-voids", SUBTRACT, profile_hole),
    (
        "nonconvex-cutter",
        "nonconvex-oblique-cut",
        SUBTRACT,
        nonconvex,
    ),
    ("after-cavity", "oblique-cavity-cut", SUBTRACT, after_cavity),
    (
        "remote-cavity",
        "remote-cavity-cut",
        SUBTRACT,
        remote_cavity,
    ),
];

fn tilt(axis: usize, degrees: f64) -> Case {
    let pair = operands(move || {
        let angle = degrees.to_radians();
        let frame = if axis == 0 {
            tilted_about_y([2.5, 0.15, 0.0], angle)
        } else {
            tilted_about_x([2.5, 0.15, 0.0], angle)
        };
        let cutter = framed(&format!("tilt-{axis}-{degrees}"), frame, square(0.8), 2.1)?;
        Ok((wall("tilt-sweep-host", 10.0)?, cutter))
    });
    let sign = if degrees < 0.0 { "minus" } else { "plus" };
    let name = format!("booleans.oblique.tilt-{axis}-{sign}-{}", degrees.abs());
    boolean(
        &name,
        &format!("tilt-result-{axis}-{degrees}"),
        SUBTRACT,
        &pair,
    )
}

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = table("booleans.oblique", &ENTRIES);
    for axis in 0..2 {
        for degrees in [-25.0, -15.0, -5.0, 5.0, 15.0, 25.0] {
            cases.push(tilt(axis, degrees));
        }
    }
    cases
}
