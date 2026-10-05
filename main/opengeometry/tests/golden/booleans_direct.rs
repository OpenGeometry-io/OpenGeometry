use crate::boolean_case::{boolean, operands, record_operands, result_body, Operands, OPERATIONS};
use crate::fine_solids::sphere_at;
use crate::fixtures::{
    accuracy, at, fine, ground, moved, scaled_accuracy, tilted_about_y, y_axis_frame,
};
use crate::kernel::{
    boolean_boxes, boolean_spheres, placed, primitives, Accuracy, BooleanOp, BooleanResult,
    BrepEnvelope, Frame3, GeometryError, Point3,
};
use crate::runner::Case;
use std::sync::Arc;

type Direct =
    fn(&BrepEnvelope, &BrepEnvelope, BooleanOp, String) -> Result<BooleanResult, GeometryError>;

fn direct(name: &str, operation: BooleanOp, pair: &Operands, call: Direct) -> Case {
    let pair = Arc::clone(pair);
    Case::new(name, move |record| {
        let (a, b) = pair()?;
        record_operands(record, &a, &b);
        record.section("direct");
        record.debug("operation", operation);
        result_body(record, call(&a, &b, operation, "r".into()));
        Ok(())
    })
}

fn every_direct(prefix: &str, pair: &Operands, call: Direct) -> Vec<Case> {
    OPERATIONS
        .iter()
        .map(|(label, operation)| direct(&format!("{prefix}.{label}"), *operation, pair, call))
        .collect()
}

fn spheres(a: (Point3, f64), b: (Point3, f64)) -> Operands {
    operands(move || Ok((sphere_at("a", a.0, a.1)?, sphere_at("b", b.0, b.1)?)))
}

type SpherePair = (&'static str, BooleanOp, (Point3, f64), (Point3, f64));

const ORIGIN: Point3 = [0.0; 3];

const SPHERE_PAIRS: [SpherePair; 9] = [
    (
        "enclosed",
        BooleanOp::Subtraction,
        (ORIGIN, 2.0),
        ([0.2, 0.0, 0.0], 0.5),
    ),
    (
        "contact",
        BooleanOp::Union,
        (ORIGIN, 1.0),
        ([2.0, 0.0, 0.0], 1.0),
    ),
    (
        "disjoint",
        BooleanOp::Union,
        (ORIGIN, 1.0),
        ([2.5, 0.0, 0.0], 1.0),
    ),
    ("coincident", BooleanOp::Union, (ORIGIN, 1.0), (ORIGIN, 1.0)),
    (
        "inside-big.subtraction",
        BooleanOp::Subtraction,
        (ORIGIN, 1.0),
        (ORIGIN, 2.0),
    ),
    (
        "inside-big.intersection",
        BooleanOp::Intersection,
        (ORIGIN, 1.0),
        (ORIGIN, 2.0),
    ),
    (
        "internal-tangency",
        BooleanOp::Subtraction,
        (ORIGIN, 2.0),
        ([1.0, 0.0, 0.0], 1.0),
    ),
    (
        "small-cap",
        BooleanOp::Intersection,
        (ORIGIN, 1.0),
        ([2.0 - 1e-10, 0.0, 0.0], 1.0),
    ),
    (
        "rotated-unequal",
        BooleanOp::Subtraction,
        ([5.0, -3.0, 8.0], 2.0),
        ([6.0, -2.0, 9.0], 1.25),
    ),
];

fn sphere_cases() -> Vec<Case> {
    let transverse = spheres((ORIGIN, 1.0), ([1.0, 0.0, 0.0], 1.0));
    let prefix = "booleans.spheres-direct";
    let mut cases = every_direct(
        &format!("{prefix}.transverse"),
        &transverse,
        boolean_spheres,
    );
    for (label, operation) in OPERATIONS {
        let name = format!("booleans.spheres.transverse.{label}");
        cases.push(boolean(
            &name,
            &format!("r-{label}"),
            operation,
            &transverse,
        ));
    }
    for (name, operation, a, b) in SPHERE_PAIRS {
        let pair = spheres(a, b);
        cases.push(direct(
            &format!("{prefix}.{name}"),
            operation,
            &pair,
            boolean_spheres,
        ));
    }
    let enclosed = spheres((ORIGIN, 2.0), ([0.2, 0.0, 0.0], 0.5));
    let subtract = BooleanOp::Subtraction;
    cases.push(boolean(
        "booleans.spheres.enclosed",
        "r",
        subtract,
        &enclosed,
    ));
    let cylinder = operands(|| {
        let cylinder = primitives::cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, fine())?;
        Ok((sphere_at("a", ORIGIN, 1.0)?, cylinder))
    });
    let name = format!("{prefix}.unsupported-cylinder");
    cases.push(direct(&name, BooleanOp::Union, &cylinder, boolean_spheres));
    cases.extend(scaled_spheres());
    cases
}

fn scaled_spheres() -> Vec<Case> {
    let mut cases = Vec::new();
    for (label, size) in [("micro", 1e-6), ("unit", 1.0), ("mega", 1e6)] {
        let pair = operands(move || {
            let a = primitives::sphere("a".into(), Frame3::IDENTITY, size, scaled_accuracy(size))?;
            let b = primitives::sphere(
                "b".into(),
                at([size, 0.0, 0.0]),
                size,
                scaled_accuracy(size),
            )?;
            Ok((a, b))
        });
        cases.push(direct(
            &format!("booleans.spheres-direct.scaled-{label}"),
            BooleanOp::Subtraction,
            &pair,
            boolean_spheres,
        ));
    }
    let far = operands(|| {
        let far_accuracy = accuracy(1e-6, 5e-7, 0.01, 1e-5);
        let a = primitives::sphere("a".into(), at([1e9; 3]), 1.0, far_accuracy)?;
        let b = primitives::sphere("b".into(), at([1e9 + 1.0, 1e9, 1e9]), 1.0, far_accuracy)?;
        Ok((a, b))
    });
    cases.push(direct(
        "booleans.spheres-direct.far-coordinates",
        BooleanOp::Intersection,
        &far,
        boolean_spheres,
    ));
    let placed_pair = operands(|| {
        let placement = tilted_about_y([0.2, 0.1, 0.2], std::f64::consts::PI / 7.0);
        let frame = moved(ground(), [0.0, 1.2, 0.0]);
        let first = primitives::sphere("a".into(), frame, 1.0, fine())?;
        let second = primitives::sphere("b".into(), moved(frame, [1.0, 1.2, 0.0]), 1.0, fine())?;
        Ok((
            placed(&first, placement, 1.25)?,
            placed(&second, placement, 1.25)?,
        ))
    });
    cases.extend(every_direct(
        "booleans.spheres-direct.placed",
        &placed_pair,
        boolean_spheres,
    ));
    cases
}

fn box_accuracy() -> Accuracy {
    accuracy(1e-8, 1e-9, 1e-3, 1e-6)
}

fn box_at(name: &str, origin: Point3, size: Point3) -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid(name.into(), at(origin), size, box_accuracy())
}

fn boxes(a: (&'static str, Point3, Point3), b: (&'static str, Point3, Point3)) -> Operands {
    operands(move || Ok((box_at(a.0, a.1, a.2)?, box_at(b.0, b.1, b.2)?)))
}

fn box_cases() -> Vec<Case> {
    let aligned = boxes(
        ("host", [0.0; 3], [2.0; 3]),
        ("cutter", [1.0, 0.5, 0.5], [2.0; 3]),
    );
    let mut cases = every_direct("booleans.boxes-direct.aligned", &aligned, boolean_boxes);
    let cavity = boxes(("host", [0.0; 3], [3.0; 3]), ("cavity", [1.0; 3], [1.0; 3]));
    cases.push(direct(
        "booleans.boxes-direct.cavity",
        BooleanOp::Subtraction,
        &cavity,
        boolean_boxes,
    ));
    let through = boxes(
        ("host", [0.0; 3], [3.0; 3]),
        ("through", [1.0, -1.0, -1.0], [1.0, 5.0, 5.0]),
    );
    cases.push(direct(
        "booleans.boxes-direct.through",
        BooleanOp::Subtraction,
        &through,
        boolean_boxes,
    ));
    for (label, origin) in [
        ("face", [3.0, 0.0, 0.0]),
        ("edge", [3.0, 3.0, 0.0]),
        ("corner", [3.0; 3]),
    ] {
        let pair = boxes(("host", [0.0; 3], [3.0; 3]), ("contact", origin, [3.0; 3]));
        cases.push(direct(
            &format!("booleans.boxes-direct.contact-{label}.union"),
            BooleanOp::Union,
            &pair,
            boolean_boxes,
        ));
        cases.push(direct(
            &format!("booleans.boxes-direct.contact-{label}.intersection"),
            BooleanOp::Intersection,
            &pair,
            boolean_boxes,
        ));
    }
    let identity = boxes(("a", [0.0; 3], [2.0; 3]), ("a", [0.0; 3], [2.0; 3]));
    for (label, operation) in OPERATIONS {
        cases.push(boolean(
            &format!("booleans.boxes.identity.{label}"),
            "identity",
            operation,
            &identity,
        ));
    }
    for (label, factor) in [("tiny", 1e-8), ("unit", 1.0), ("large", 1e5)] {
        let pair = operands(move || {
            let frame = y_axis_frame([0.0; 3])?;
            let a = placed(&box_at("a", [0.0; 3], [2.0; 3])?, frame, factor)?;
            let b = placed(&box_at("b", [1.0, 0.5, 0.5], [2.0; 3])?, frame, factor)?;
            Ok((a, b))
        });
        cases.push(direct(
            &format!("booleans.boxes-direct.scaled-{label}"),
            BooleanOp::Subtraction,
            &pair,
            boolean_boxes,
        ));
    }
    cases.extend(box_errors());
    cases
}

fn box_errors() -> Vec<Case> {
    let similar = operands(|| {
        let frame = y_axis_frame([5.0, 6.0, 7.0])?;
        let a = placed(&box_at("a", [0.0; 3], [2.0; 3])?, frame, 1.25)?;
        let b = placed(&box_at("b", [1.0, 0.5, 0.5], [2.0; 3])?, frame, 1.25)?;
        Ok((a, b))
    });
    let mixed_axes = operands(|| {
        let a = placed(
            &box_at("a", [0.0; 3], [2.0; 3])?,
            y_axis_frame([5.0, 6.0, 7.0])?,
            1.25,
        )?;
        Ok((a, box_at("different-axes", [0.0; 3], [2.0; 3])?))
    });
    let unresolved = boxes(
        ("a", [0.0; 3], [2.0; 3]),
        ("b", [2.0 + 2.0 * 1e-8, 0.0, 0.0], [2.0; 3]),
    );
    vec![
        direct(
            "booleans.boxes-direct.placed-similarity",
            BooleanOp::Subtraction,
            &similar,
            boolean_boxes,
        ),
        direct(
            "booleans.boxes-direct.different-axes",
            BooleanOp::Union,
            &mixed_axes,
            boolean_boxes,
        ),
        direct(
            "booleans.boxes-direct.unresolved-gap",
            BooleanOp::Union,
            &unresolved,
            boolean_boxes,
        ),
    ]
}

pub(crate) fn cases() -> Vec<Case> {
    [sphere_cases(), box_cases()]
        .into_iter()
        .flatten()
        .collect()
}
