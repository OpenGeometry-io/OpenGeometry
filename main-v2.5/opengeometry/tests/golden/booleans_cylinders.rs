use crate::boolean_case::{boolean, every_operation, operands, swapped, Operands};
use crate::fixtures::{accuracy, at, fine, ground, moved, reversed_z, tilted_about_y};
use crate::kernel::{placed, primitives, BooleanOp, BrepEnvelope, Frame3, GeometryError, Point3};
use crate::runner::Case;

const SIZE: [f64; 3] = [4.0, 3.0, 2.0];

fn cylinder(
    name: &str,
    frame: Frame3,
    radius: f64,
    height: f64,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::cylinder(name.into(), frame, radius, height, fine())
}

fn host_box() -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid("box".into(), Frame3::IDENTITY, SIZE, fine())
}

fn unit_axis(axis: usize, sign: f64) -> Point3 {
    std::array::from_fn(|coordinate| if coordinate == axis { sign } else { 0.0 })
}

fn full_family(prefix: &str, id: &str, pair: &Operands) -> Vec<Case> {
    let mut cases = every_operation(prefix, id, pair);
    let reverse = format!("{prefix}.reversed-subtraction");
    cases.push(boolean(
        &reverse,
        &format!("{id}-reversed"),
        BooleanOp::Subtraction,
        &swapped(pair),
    ));
    cases
}

fn through_axes() -> Vec<Case> {
    (0..3)
        .flat_map(|axis| {
            let pair = operands(move || {
                let mut origin = [2.0, 1.5, 1.0];
                origin[axis] = -1.0;
                let frame = Frame3::from_axis(
                    origin,
                    unit_axis(axis, 1.0),
                    unit_axis((axis + 1) % 3, 1.0),
                )?;
                Ok((
                    host_box()?,
                    cylinder("round-opening", frame, 0.4, SIZE[axis] + 2.0)?,
                ))
            });
            full_family(
                &format!("booleans.cylinder-box.through-{axis}"),
                &format!("through-{axis}"),
                &pair,
            )
        })
        .collect()
}

fn blind_pockets() -> Vec<Case> {
    let mut cases = Vec::new();
    for axis in 0..3 {
        for (side, low) in [("low", true), ("high", false)] {
            let pair = operands(move || {
                let mut origin = [2.0, 1.5, 1.0];
                origin[axis] = if low { -1.0 } else { SIZE[axis] + 1.0 };
                let direction = unit_axis(axis, if low { 1.0 } else { -1.0 });
                let frame = Frame3::from_axis(origin, direction, unit_axis((axis + 1) % 3, 1.0))?;
                Ok((host_box()?, cylinder("pocket", frame, 0.4, 1.75)?))
            });
            let prefix = format!("booleans.cylinder-box.pocket-{axis}-{side}");
            cases.extend(full_family(
                &prefix,
                &format!("pocket-{axis}-{side}"),
                &pair,
            ));
        }
    }
    cases
}

fn through_cut() -> Vec<Case> {
    let pair = operands(|| {
        let cut_accuracy = accuracy(4e-8, 1e-8, 0.01, 1e-5);
        let host =
            primitives::cylinder("host".into(), at([0.0, 0.0, -2.0]), 1.0, 4.0, cut_accuracy)?;
        let frame = Frame3::from_axis([-2.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0])?;
        let cutter = primitives::cylinder("cutter".into(), frame, 0.6, 4.0, cut_accuracy)?;
        Ok((host, cutter))
    });
    vec![boolean(
        "booleans.cylinders.nonparallel-through-cut",
        "through-cut",
        BooleanOp::Subtraction,
        &pair,
    )]
}

fn coaxial() -> Vec<Case> {
    let split = operands(|| {
        Ok((
            cylinder("a", at([0.0; 3]), 1.0, 4.0)?,
            cylinder("b", reversed_z([0.0, 0.0, 3.0]), 1.0, 2.0)?,
        ))
    });
    let contact = operands(|| {
        Ok((
            cylinder("a", at([0.0; 3]), 1.0, 4.0)?,
            cylinder("b", at([0.0, 0.0, 4.0]), 1.0, 2.0)?,
        ))
    });
    let near = operands(|| {
        Ok((
            cylinder("a", at([0.0; 3]), 1.0, 4.0)?,
            cylinder("near", at([0.0, 0.0, 4.0 + 2e-9]), 1.0, 2.0)?,
        ))
    });
    let split_y = operands(|| {
        let host = cylinder("host", ground(), 1.0, 2.0)?;
        let cutter = placed(
            &cylinder("cutter", ground(), 1.0, 0.8)?,
            at([0.0, 0.6, 0.0]),
            1.0,
        )?;
        Ok((host, cutter))
    });
    let mut cases = every_operation("booleans.cylinders.coaxial-split", "coaxial", &split);
    cases.extend(every_operation(
        "booleans.cylinders.coaxial-contact",
        "contact",
        &contact,
    ));
    cases.push(boolean(
        "booleans.cylinders.coaxial-near-gap",
        "near",
        BooleanOp::Union,
        &near,
    ));
    cases.push(boolean(
        "booleans.cylinders.translated-y-split",
        "result",
        BooleanOp::Subtraction,
        &split_y,
    ));
    cases
}

fn radial() -> Vec<Case> {
    let coextensive = operands(|| {
        Ok((
            cylinder("outer", at([0.0; 3]), 2.0, 3.0)?,
            cylinder("inner", at([0.0; 3]), 1.0, 3.0)?,
        ))
    });
    let shifted = operands(|| {
        Ok((
            cylinder("outer", at([0.0; 3]), 2.0, 3.0)?,
            cylinder("shifted", at([0.0, 0.0, 0.5]), 1.0, 3.0)?,
        ))
    });
    let enclosed = operands(|| {
        Ok((
            cylinder("host", at([0.0; 3]), 2.0, 4.0)?,
            cylinder("cutter", at([0.0, 0.0, 1.0]), 0.75, 2.0)?,
        ))
    });
    let mut cases = full_family(
        "booleans.cylinders.coextensive",
        "coextensive",
        &coextensive,
    );
    cases.push(boolean(
        "booleans.cylinders.coextensive-shifted",
        "gap",
        BooleanOp::Subtraction,
        &shifted,
    ));
    cases.extend(full_family(
        "booleans.cylinders.enclosed",
        "enclosed",
        &enclosed,
    ));
    cases
}

fn transverse() -> Vec<Case> {
    let neighbour = |name: &'static str, frame: Frame3| {
        operands(move || {
            Ok((
                cylinder("a", Frame3::IDENTITY, 1.0, 2.0)?,
                cylinder(name, frame, 1.0, 2.0)?,
            ))
        })
    };
    let mut cases = every_operation(
        "booleans.cylinders.transverse",
        "transverse",
        &neighbour("b", at([1.0, 0.0, 0.0])),
    );
    let reversed = neighbour("reversed", reversed_z([1.0, 0.0, 2.0]));
    cases.push(boolean(
        "booleans.cylinders.transverse-reversed",
        "reversed-result",
        BooleanOp::Intersection,
        &reversed,
    ));
    let shifted = neighbour("shifted", at([1.0, 0.0, 0.25]));
    cases.push(boolean(
        "booleans.cylinders.transverse-shifted",
        "gap",
        BooleanOp::Union,
        &shifted,
    ));
    cases.extend(every_operation(
        "booleans.cylinders.tangent",
        "contact",
        &neighbour("tangent", at([2.0, 0.0, 0.0])),
    ));
    let near = neighbour("near-tangent", at([2.0 + 2e-9, 0.0, 0.0]));
    cases.push(boolean(
        "booleans.cylinders.near-tangent",
        "near-tangent-result",
        BooleanOp::Union,
        &near,
    ));
    let diagonal = neighbour("diagonal", at([1.5, 1.5, 0.0]));
    cases.push(boolean(
        "booleans.cylinders.diagonal",
        "diagonal-union",
        BooleanOp::Union,
        &diagonal,
    ));
    cases
}

fn parallel() -> Vec<Case> {
    let eccentric = operands(|| {
        Ok((
            cylinder("outer", Frame3::IDENTITY, 2.0, 2.0)?,
            cylinder("inner", at([0.75, 0.0, 0.0]), 0.5, 2.0)?,
        ))
    });
    let touching = operands(|| {
        Ok((
            cylinder("outer", Frame3::IDENTITY, 2.0, 2.0)?,
            cylinder("touching", at([1.5, 0.0, 0.0]), 0.5, 2.0)?,
        ))
    });
    let partial = operands(|| {
        Ok((
            cylinder("host", Frame3::IDENTITY, 1.0, 3.0)?,
            cylinder("short", at([1.0, 0.0, 1.0]), 1.0, 1.0)?,
        ))
    });
    let through = operands(|| {
        Ok((
            cylinder("host", Frame3::IDENTITY, 1.0, 3.0)?,
            cylinder("through", at([1.0, 0.0, -1.0]), 1.0, 5.0)?,
        ))
    });
    let mut cases = every_operation("booleans.cylinders.eccentric", "eccentric", &eccentric);
    cases.push(boolean(
        "booleans.cylinders.eccentric-touching",
        "touching-result",
        BooleanOp::Subtraction,
        &touching,
    ));
    cases.extend(every_operation(
        "booleans.cylinders.partial-span",
        "partial",
        &partial,
    ));
    cases.push(boolean(
        "booleans.cylinders.partial-span.reversed-union",
        "reversed-partial-union",
        BooleanOp::Union,
        &swapped(&partial),
    ));
    cases.push(boolean(
        "booleans.cylinders.partial-through",
        "through-cut",
        BooleanOp::Subtraction,
        &through,
    ));
    cases
}

fn placed_roundoff() -> Vec<Case> {
    let pair = operands(|| {
        let source = cylinder("c", moved(ground(), [0.0, 1.2, 0.0]), 1.0, 2.0)?;
        let frame = tilted_about_y([0.2, 0.1, 0.2], std::f64::consts::PI / 7.0);
        let host = placed(&source, frame, 1.25)?;
        let inner = placed(
            &cylinder("inner", moved(ground(), [0.0, 1.2, 0.0]), 0.5, 2.0)?,
            frame,
            1.25,
        )?;
        Ok((host, inner))
    });
    every_operation("booleans.cylinders.placed-roundoff", "placed", &pair)
}

pub(crate) fn cases() -> Vec<Case> {
    [
        through_axes(),
        blind_pockets(),
        through_cut(),
        coaxial(),
        radial(),
        transverse(),
        parallel(),
        placed_roundoff(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
