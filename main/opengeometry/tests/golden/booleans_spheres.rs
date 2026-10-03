use crate::boolean_case::{boolean, every_operation, operands, swapped, Operands};
use crate::fine_solids::{cuboid_at, sphere_at};
use crate::fixtures::{at, fine};
use crate::kernel::{primitives, BooleanOp, BrepEnvelope, Frame3, GeometryError, Point3};
use crate::runner::Case;

const SIZE: [f64; 3] = [4.0, 3.0, 2.0];

fn cylinder(
    name: &str,
    origin: Point3,
    radius: f64,
    height: f64,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::cylinder(name.into(), at(origin), radius, height, fine())
}

fn pocket_center(axis: usize, upper: bool, inside: bool) -> Point3 {
    let mut center = [2.0, 1.5, 1.0];
    center[axis] = match (upper, inside) {
        (false, false) => -0.25,
        (false, true) => 0.25,
        (true, false) => SIZE[axis] + 0.25,
        (true, true) => SIZE[axis] - 0.25,
    };
    center
}

fn pockets() -> Vec<Case> {
    let mut cases = Vec::new();
    for axis in 0..3 {
        for upper in [false, true] {
            for inside in [false, true] {
                let pair = operands(move || {
                    let center = pocket_center(axis, upper, inside);
                    Ok((
                        cuboid_at("box", [0.0; 3], SIZE)?,
                        sphere_at("spherical-pocket", center, 0.75)?,
                    ))
                });
                let label = format!(
                    "{axis}-{}-{}",
                    if upper { "upper" } else { "lower" },
                    if inside { "inside" } else { "outside" }
                );
                let prefix = format!("booleans.sphere-box.pocket-{label}");
                cases.extend(every_operation(
                    &prefix,
                    &format!("sphere-pocket-{label}"),
                    &pair,
                ));
                let reverse = format!("{prefix}.reversed-subtraction");
                cases.push(boolean(
                    &reverse,
                    &format!("sphere-cut-{label}"),
                    BooleanOp::Subtraction,
                    &swapped(&pair),
                ));
            }
        }
    }
    cases
}

fn sphere_cylinder() -> Vec<Case> {
    let contained = operands(|| {
        Ok((
            cylinder("cylinder", [0.0; 3], 2.0, 4.0)?,
            sphere_at("sphere", [0.0, 0.0, 2.0], 0.5)?,
        ))
    });
    let cavity = operands(|| {
        Ok((
            sphere_at("sphere-host", [0.0; 3], 3.0)?,
            cylinder("cylinder-cutter", [0.0, 0.0, -0.5], 0.5, 1.0)?,
        ))
    });
    let crossing = operands(|| {
        Ok((
            cylinder("cylinder", [0.0; 3], 2.0, 4.0)?,
            sphere_at("crossing", [1.8, 0.0, 2.0], 0.5)?,
        ))
    });
    let coaxial = operands(|| {
        Ok((
            sphere_at("sphere", [0.0; 3], 1.5)?,
            cylinder("cylinder", [0.0, 0.0, -2.0], 0.75, 4.0)?,
        ))
    });
    let noncoaxial = operands(|| {
        Ok((
            sphere_at("sphere", [0.0; 3], 1.5)?,
            cylinder("noncoaxial", [0.2, 0.0, -2.0], 0.75, 4.0)?,
        ))
    });
    let mut cases = every_operation(
        "booleans.sphere-cylinder.contained",
        "contained",
        &contained,
    );
    cases.push(boolean(
        "booleans.sphere-cylinder.spherical-cavity",
        "spherical-cavity",
        BooleanOp::Subtraction,
        &cavity,
    ));
    cases.extend(every_operation(
        "booleans.sphere-cylinder.crossing",
        "crossing",
        &crossing,
    ));
    cases.push(boolean(
        "booleans.sphere-cylinder.crossing.reversed-subtraction",
        "crossing-sphere-cut",
        BooleanOp::Subtraction,
        &swapped(&crossing),
    ));
    cases.extend(every_operation(
        "booleans.sphere-cylinder.coaxial",
        "coaxial",
        &coaxial,
    ));
    cases.push(boolean(
        "booleans.sphere-cylinder.coaxial.reversed-subtraction",
        "cylinder-cut",
        BooleanOp::Subtraction,
        &swapped(&coaxial),
    ));
    cases.push(boolean(
        "booleans.sphere-cylinder.noncoaxial",
        "gap",
        BooleanOp::Union,
        &noncoaxial,
    ));
    cases
}

fn cuboid_containment() -> Vec<Case> {
    let box_host = || cuboid_at("box", [-2.0; 3], [4.0; 3]);
    let small_box = || cuboid_at("small-box", [-0.5; 3], [1.0; 3]);
    let pairs: [(&str, &str, BooleanOp, Operands); 5] = [
        (
            "sphere-in-box",
            "sphere-cavity",
            BooleanOp::Subtraction,
            operands(move || Ok((box_host()?, sphere_at("sphere", [0.0; 3], 0.5)?))),
        ),
        (
            "cylinder-in-box",
            "cylinder-cavity",
            BooleanOp::Subtraction,
            operands(move || {
                Ok((
                    box_host()?,
                    cylinder("cylinder", [0.0, 0.0, -0.5], 0.5, 1.0)?,
                ))
            }),
        ),
        (
            "box-in-sphere",
            "sphere-box",
            BooleanOp::Subtraction,
            operands(move || Ok((sphere_at("sphere-host", [0.0; 3], 4.0)?, small_box()?))),
        ),
        (
            "box-in-cylinder",
            "cylinder-box",
            BooleanOp::Subtraction,
            operands(move || {
                Ok((
                    cylinder("cylinder-host", [0.0, 0.0, -2.0], 3.0, 4.0)?,
                    small_box()?,
                ))
            }),
        ),
        (
            "crossing-union",
            "crossing-union",
            BooleanOp::Union,
            operands(move || Ok((box_host()?, sphere_at("crossing", [1.8, 0.0, 0.0], 0.5)?))),
        ),
    ];
    pairs
        .iter()
        .map(|(name, id, operation, pair)| {
            boolean(
                &format!("booleans.cuboid-containment.{name}"),
                id,
                *operation,
                pair,
            )
        })
        .collect()
}

fn torus_family() -> Vec<Case> {
    let torus = || primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, fine());
    let concentric = operands(|| {
        let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, fine())?;
        let cutter = primitives::torus("cutter".into(), Frame3::IDENTITY, 3.0, 0.4, fine())?;
        Ok((host, cutter))
    });
    let identical = operands(|| {
        let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, fine())?;
        Ok((host.clone(), host))
    });
    let small = operands(move || Ok((torus()?, sphere_at("small", [3.0, 0.0, 0.0], 0.25)?)));
    let large = operands(move || Ok((sphere_at("large", [0.0; 3], 5.0)?, torus()?)));
    let mut cases = every_operation("booleans.torus.concentric", "torus", &concentric);
    cases.push(boolean(
        "booleans.torus.identical",
        "empty-torus",
        BooleanOp::Subtraction,
        &identical,
    ));
    cases.push(boolean(
        "booleans.sphere-torus.small-sphere",
        "torus-cut",
        BooleanOp::Subtraction,
        &small,
    ));
    cases.push(boolean(
        "booleans.sphere-torus.large-sphere",
        "sphere-cut",
        BooleanOp::Subtraction,
        &large,
    ));
    let poking = operands(|| {
        let host = primitives::torus("host".into(), Frame3::IDENTITY, 3.0, 1.0, fine())?;
        let ring = primitives::torus("ring".into(), at([3.0, 0.0, 0.0]), 0.8, 0.3, fine())?;
        Ok((host, ring))
    });
    cases.extend(every_operation(
        "booleans.torus.poking-ring",
        "poking",
        &poking,
    ));
    cases.extend(ring_hosts());
    cases
}

fn ring_hosts() -> Vec<Case> {
    let ring = || primitives::torus("torus".into(), at([0.0, 0.0, 2.0]), 2.0, 0.5, fine());
    let hosts: [(&str, Operands); 3] = [
        (
            "cylinder",
            operands(move || Ok((cylinder("cylinder", [0.0; 3], 4.0, 4.0)?, ring()?))),
        ),
        (
            "box",
            operands(move || {
                Ok((
                    cuboid_at("box", [-4.0, -4.0, -1.0], [8.0, 8.0, 6.0])?,
                    ring()?,
                ))
            }),
        ),
        (
            "frustum",
            operands(move || {
                Ok((
                    primitives::frustum("frustum".into(), Frame3::IDENTITY, 4.0, 5.0, 4.0, fine())?,
                    ring()?,
                ))
            }),
        ),
    ];
    hosts
        .iter()
        .map(|(name, pair)| {
            boolean(
                &format!("booleans.torus-containment.{name}-minus-torus"),
                &format!("{name}-minus-torus"),
                BooleanOp::Subtraction,
                pair,
            )
        })
        .collect()
}

fn frustum(
    name: &str,
    origin: Point3,
    lower: f64,
    upper: f64,
    height: f64,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::frustum(name.into(), at(origin), lower, upper, height, fine())
}

fn conic_family() -> Vec<Case> {
    let host = || frustum("host", [0.0; 3], 3.0, 4.0, 4.0);
    let coaxial =
        operands(move || Ok((host()?, frustum("cutter", [0.0, 0.0, 1.0], 1.0, 1.5, 1.0)?)));
    let touching = operands(move || Ok((host()?, frustum("touching", [0.0; 3], 1.0, 1.5, 1.0)?)));
    let mut cases = every_operation("booleans.conic.coaxial", "conic", &coaxial);
    cases.push(boolean(
        "booleans.conic.touching",
        "touching-result",
        BooleanOp::Subtraction,
        &touching,
    ));
    let conic = || frustum("frustum", [0.0; 3], 3.0, 4.0, 4.0);
    let mixed: [(&str, Operands); 6] = [
        (
            "frustum-minus-inner-sphere",
            operands(move || Ok((conic()?, sphere_at("inner-sphere", [0.0, 0.0, 2.0], 0.5)?))),
        ),
        (
            "outer-sphere-minus-frustum",
            operands(move || Ok((sphere_at("outer-sphere", [0.0, 0.0, 2.0], 6.0)?, conic()?))),
        ),
        (
            "frustum-minus-inner-cylinder",
            operands(move || {
                Ok((
                    conic()?,
                    cylinder("inner-cylinder", [0.0, 0.0, 1.0], 0.5, 1.0)?,
                ))
            }),
        ),
        (
            "outer-cylinder-minus-frustum",
            operands(move || {
                Ok((
                    cylinder("outer-cylinder", [0.0, 0.0, -1.0], 5.0, 6.0)?,
                    conic()?,
                ))
            }),
        ),
        (
            "frustum-minus-inner-box",
            operands(move || {
                Ok((
                    conic()?,
                    cuboid_at("inner-box", [-0.25, -0.25, 1.75], [0.5; 3])?,
                ))
            }),
        ),
        (
            "outer-box-minus-frustum",
            operands(move || {
                Ok((
                    cuboid_at("outer-box", [-5.0, -5.0, -1.0], [10.0, 10.0, 6.0])?,
                    conic()?,
                ))
            }),
        ),
    ];
    cases.extend(mixed.iter().map(|(name, pair)| {
        boolean(
            &format!("booleans.conic-containment.{name}"),
            name,
            BooleanOp::Subtraction,
            pair,
        )
    }));
    cases.extend(generic_loops());
    cases
}

fn generic_loops() -> Vec<Case> {
    let cone = || primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, fine());
    let off_axis = operands(move || Ok((cone()?, sphere_at("sphere", [1.42, 0.0, 1.0], 0.35)?)));
    let winding = operands(move || Ok((cone()?, sphere_at("sphere", [1.1, 0.0, 0.8], 0.35)?)));
    let mut cases = every_operation("booleans.generic.sphere-cone", "sphere-cone", &off_axis);
    cases.push(boolean(
        "booleans.generic.periodic-winding",
        "winding",
        BooleanOp::Intersection,
        &winding,
    ));
    let prism = operands(|| {
        let triangle = vec![[-0.2, -0.2], [0.2, -0.2], [0.0, 0.2]];
        let prism = primitives::linear_extrusion(
            "prism".into(),
            at([0.0, 0.0, 0.8]),
            triangle,
            Vec::new(),
            0.4,
            fine(),
        )?;
        Ok((cylinder("cylinder", [0.0; 3], 2.0, 2.0)?, prism))
    });
    cases.extend(every_operation(
        "booleans.generic.noncanonical-planar-containment",
        "noncanonical",
        &prism,
    ));
    let annular = operands(|| {
        let host =
            primitives::annular_cylinder("host".into(), Frame3::IDENTITY, 0.4, 2.0, 3.0, fine())?;
        Ok((host.clone(), host))
    });
    cases.push(boolean(
        "booleans.coincident.annular-union",
        "same",
        BooleanOp::Union,
        &annular,
    ));
    cases.push(boolean(
        "booleans.coincident.annular-subtraction",
        "empty",
        BooleanOp::Subtraction,
        &annular,
    ));
    cases
}

pub(crate) fn cases() -> Vec<Case> {
    [
        pockets(),
        sphere_cylinder(),
        cuboid_containment(),
        torus_family(),
        conic_family(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
