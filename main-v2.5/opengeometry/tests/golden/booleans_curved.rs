use crate::boolean_case::{boolean, operands, stage, Operands};
use crate::fine_solids::cuboid_at;
use crate::fixtures::{at, fine, rectangle};
use crate::kernel::{primitives, BooleanOp, BrepEnvelope, Frame3, GeometryError};
use crate::profiles::{quarter_annulus, sector, split_quarter_annulus};
use crate::record::Failure;
use crate::runner::Case;
use std::f64::consts::FRAC_PI_2;

fn prism(
    name: &str,
    bottom: f64,
    outer: Vec<[f64; 2]>,
    height: f64,
) -> Result<BrepEnvelope, GeometryError> {
    primitives::linear_extrusion(
        name.into(),
        at([0.0, 0.0, bottom]),
        outer,
        Vec::new(),
        height,
        fine(),
    )
}

fn quarter(name: &str, inner: f64) -> Result<BrepEnvelope, GeometryError> {
    primitives::arc_edged_extrusion(
        name.into(),
        Frame3::IDENTITY,
        quarter_annulus(2.0, inner),
        3.0,
        fine(),
    )
}

fn curved_subtraction(name: &str, id: &str, pair: Operands) -> Case {
    boolean(
        &format!("booleans.curved.{name}"),
        id,
        BooleanOp::Subtraction,
        &pair,
    )
}

fn ring_sectors() -> Vec<Case> {
    let ring = || {
        primitives::annular_cylinder("ring-host".into(), Frame3::IDENTITY, 1.9, 2.1, 3.0, fine())
    };
    let first = || prism("ring-opening", 0.5, sector(2.3, -0.1, 0.1), 2.0);
    let chained = operands(move || {
        let cut = stage(&ring()?, &first()?, BooleanOp::Subtraction, "ring-cut")?;
        let second = prism(
            "second-ring-opening",
            0.8,
            sector(2.3, FRAC_PI_2 - 0.1, FRAC_PI_2 + 0.1),
            1.3,
        )?;
        Ok((cut, second))
    });
    vec![
        curved_subtraction(
            "ring-sector",
            "ring-cut",
            operands(move || Ok((ring()?, first()?))),
        ),
        curved_subtraction("ring-sector-chained", "ring-cut-chained", chained),
    ]
}

fn arc_host_openings() -> Vec<Case> {
    let opening = || cuboid_at("opening", [1.1, 0.9, 0.5], [1.0, 0.35, 1.5]);
    let second = || cuboid_at("second-opening", [0.6, 1.3, 0.8], [0.45, 0.8, 1.2]);
    let chained = operands(move || {
        let cut = stage(
            &quarter("arc-host", 1.5)?,
            &opening()?,
            BooleanOp::Subtraction,
            "arc-cut",
        )?;
        Ok((cut, second()?))
    });
    let third = operands(move || {
        let cut = stage(
            &quarter("arc-host", 1.5)?,
            &opening()?,
            BooleanOp::Subtraction,
            "arc-cut",
        )?;
        let chained = stage(&cut, &second()?, BooleanOp::Subtraction, "arc-cut-chained")?;
        Ok((
            chained,
            cuboid_at("overlapping-opening", [1.4, 0.95, 0.2], [0.5, 0.4, 2.2])?,
        ))
    });
    let split_arcs = operands(|| {
        let host = primitives::arc_edged_extrusion(
            "split-arc-host".into(),
            Frame3::IDENTITY,
            split_quarter_annulus(),
            3.0,
            fine(),
        )?;
        Ok((
            host,
            cuboid_at("split-arc-opening", [1.1, 0.9, 0.5], [1.0, 0.35, 1.5])?,
        ))
    });
    vec![
        curved_subtraction(
            "arc-host-opening",
            "arc-cut",
            operands(move || Ok((quarter("arc-host", 1.5)?, opening()?))),
        ),
        curved_subtraction("arc-host-chained", "arc-cut-chained", chained),
        curved_subtraction("arc-host-third", "arc-cut-third", third),
        curved_subtraction("split-arc-provenance", "split-arc-cut", split_arcs),
    ]
}

fn cavities() -> Vec<Case> {
    let host = || quarter("curved-cavity-host", 1.0);
    let interior = || {
        prism(
            "curved-interior-cutter",
            1.0,
            rectangle([1.0, 1.0], [1.2, 1.2]),
            1.0,
        )
    };
    let splitter = || prism("curved-split-sector", -1.0, sector(3.0, 0.2, 0.3), 5.0);
    let other = || {
        prism(
            "curved-other-component-cutter",
            1.1,
            rectangle([1.55, 0.1], [1.67, 0.22]),
            0.8,
        )
    };
    let split_with_cavity = move || -> Result<BrepEnvelope, Failure> {
        let split = stage(
            &host()?,
            &splitter()?,
            BooleanOp::Subtraction,
            "curved-split",
        )?;
        stage(
            &split,
            &interior()?,
            BooleanOp::Subtraction,
            "curved-split-with-cavity",
        )
    };
    let split_then_cavity = operands(move || {
        let split = stage(
            &host()?,
            &splitter()?,
            BooleanOp::Subtraction,
            "curved-split",
        )?;
        Ok((split, interior()?))
    });
    let same_component = operands(move || {
        let cavity = stage(
            &host()?,
            &interior()?,
            BooleanOp::Subtraction,
            "curved-cavity",
        )?;
        Ok((cavity, other()?))
    });
    vec![
        curved_subtraction(
            "internal-cavity",
            "curved-cavity",
            operands(move || Ok((host()?, interior()?))),
        ),
        curved_subtraction(
            "split-sector",
            "curved-split",
            operands(move || Ok((host()?, splitter()?))),
        ),
        curved_subtraction(
            "split-with-cavity",
            "curved-split-with-cavity",
            split_then_cavity,
        ),
        curved_subtraction(
            "same-component-cavities",
            "curved-same-component-cavities",
            same_component,
        ),
        curved_subtraction(
            "two-cavities",
            "curved-two-cavities",
            operands(move || Ok((split_with_cavity()?, other()?))),
        ),
    ]
}

fn split_caps() -> Vec<Case> {
    let host = || quarter("split-cap-arc-host", 1.5);
    let first = || prism("full-height-sector", -1.0, sector(3.0, 0.7, 0.9), 5.0);
    let second = operands(move || {
        let cut = stage(
            &host()?,
            &first()?,
            BooleanOp::Subtraction,
            "split-cap-first",
        )?;
        Ok((
            cut,
            prism("lower_cutout-sector", 0.5, sector(3.0, 0.2, 0.3), 1.5)?,
        ))
    });
    vec![
        curved_subtraction(
            "split-cap-first",
            "split-cap-first",
            operands(move || Ok((host()?, first()?))),
        ),
        curved_subtraction("split-cap-second", "split-cap-second", second),
    ]
}

pub(crate) fn cases() -> Vec<Case> {
    [
        ring_sectors(),
        arc_host_openings(),
        cavities(),
        split_caps(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
