use crate::builder_case::{built, keyed};
use crate::fixtures::{fine, ground, rectangle, standard, upright};
use crate::kernel::{primitives, CurveGeometry, Frame3};
use crate::planar_shapes::profile_shell;
use crate::profiles::{
    band, line, polygon, quarter_annulus, rounded_outer, sector, split_quarter_annulus,
    square_hole, wide_arc,
};
use crate::runner::Case;
use std::f64::consts::{PI, TAU};

fn wires() -> Vec<Case> {
    vec![
        built("rectangle.identity", || {
            primitives::rectangle("rectangle".into(), Frame3::IDENTITY, 4.0, 2.0, standard())
        }),
        keyed("rectangle-keys.ground", || {
            primitives::rectangle_with_keys("rectangle".into(), ground(), 4.0, 2.0, standard())
        }),
        built("rectangle.zero-width", || {
            primitives::rectangle("rectangle".into(), Frame3::IDENTITY, 0.0, 2.0, standard())
        }),
        built("polyline.open", || {
            let points = [[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]];
            primitives::polyline("path".into(), &points, false, standard())
        }),
        keyed("polyline-keys.closed", || {
            let points = [[0.0; 3], [2.0, 0.0, 0.0], [2.0, 3.0, 0.0], [0.0, 3.0, 0.0]];
            primitives::polyline_with_keys("closed".into(), &points, true, standard())
        }),
        built("polyline.noncoplanar", || {
            let points = [[0.0; 3], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0], [3.0, 3.0, 3.0]];
            primitives::polyline("path".into(), &points, false, standard())
        }),
        built("polyline.repeated-point", || {
            primitives::polyline("bad".into(), &[[0.0; 3], [0.0; 3]], false, standard())
        }),
    ]
}

fn arcs() -> Vec<Case> {
    let frame = ground();
    vec![
        built("arc-wire.circle", move || {
            let curve = CurveGeometry::Circle { frame, radius: 1.0 };
            primitives::arc_wire("circle".into(), curve, 0.0, TAU, standard())
        }),
        built("arc-wire.half", move || {
            let curve = CurveGeometry::Circle { frame, radius: 1.0 };
            primitives::arc_wire("arc".into(), curve, 0.0, PI, standard())
        }),
        built("arc-wire.ellipse", move || {
            let curve = CurveGeometry::Ellipse {
                frame,
                major_radius: 2.0,
                minor_radius: 1.0,
            };
            primitives::arc_wire("ellipse".into(), curve, 0.5, 2.0, standard())
        }),
        built("arc-wire.line-rejected", || {
            let curve = CurveGeometry::Line {
                origin: [0.0; 3],
                direction: [1.0, 0.0, 0.0],
            };
            primitives::arc_wire("line".into(), curve, 0.0, 1.0, standard())
        }),
        built("arc-wire.zero-sweep", move || {
            let curve = CurveGeometry::Circle { frame, radius: 1.0 };
            primitives::arc_wire("empty".into(), curve, 0.0, 0.0, standard())
        }),
    ]
}

fn linear() -> Vec<Case> {
    vec![
        built("linear-extrusion.oracle", || {
            let outer = rectangle([-1.0, -0.5], [1.0, 0.5]);
            primitives::linear_extrusion(
                "linear-extrusion".into(),
                ground(),
                outer,
                Vec::new(),
                2.0,
                standard(),
            )
        }),
        built("linear-extrusion.with-hole", profile_shell),
        built("linear-extrusion.angled", || {
            let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]];
            primitives::linear_extrusion(
                "angled-host".into(),
                Frame3::IDENTITY,
                outer,
                Vec::new(),
                3.0,
                fine(),
            )
        }),
        built("linear-extrusion.upright-sector", || {
            let outer = sector(2.3, -0.1, 0.1);
            primitives::linear_extrusion(
                "sector".into(),
                upright([0.0, 0.8, 0.0]),
                outer,
                Vec::new(),
                2.0,
                fine(),
            )
        }),
        built("linear-extrusion.self-crossing", || {
            let outer = vec![[0.0, 0.0], [1.0, 1.0], [0.0, 1.0], [1.0, 0.0]];
            primitives::linear_extrusion(
                "crossing".into(),
                Frame3::IDENTITY,
                outer,
                Vec::new(),
                1.0,
                standard(),
            )
        }),
        built("linear-extrusion.hole-outside", || {
            let outer = rectangle([0.0, 0.0], [1.0, 1.0]);
            let hole = rectangle([2.0, 2.0], [3.0, 3.0]);
            primitives::linear_extrusion(
                "outside".into(),
                Frame3::IDENTITY,
                outer,
                vec![hole],
                1.0,
                standard(),
            )
        }),
    ]
}

fn curved() -> Vec<Case> {
    vec![
        built("arc-edged-extrusion.oracle", || {
            primitives::arc_edged_extrusion(
                "arc-edged-extrusion".into(),
                ground(),
                rounded_outer(),
                2.0,
                standard(),
            )
        }),
        built("arc-edged-extrusion.quarter-annulus", || {
            primitives::arc_edged_extrusion(
                "arc-host".into(),
                Frame3::IDENTITY,
                quarter_annulus(2.0, 1.5),
                3.0,
                fine(),
            )
        }),
        built("arc-edged-extrusion.split-arcs", || {
            primitives::arc_edged_extrusion(
                "split-arc-host".into(),
                Frame3::IDENTITY,
                split_quarter_annulus(),
                3.0,
                fine(),
            )
        }),
        built("arc-edged-extrusion.ground-band", || {
            primitives::arc_edged_extrusion(
                "mixed-curved-host".into(),
                ground(),
                band(3.0, 2.7, 0.8, -1.6),
                3.0,
                fine(),
            )
        }),
        built("arc-edged-extrusion.wide", || {
            primitives::arc_edged_extrusion(
                "wide-arc".into(),
                Frame3::IDENTITY,
                wide_arc(),
                3.0,
                fine(),
            )
        }),
        built("arc-edged-extrusion.polygon-only", || {
            let outer = polygon(&[[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]]);
            primitives::arc_edged_extrusion(
                "polygon".into(),
                Frame3::IDENTITY,
                outer,
                1.0,
                standard(),
            )
        }),
        built("arc-edged-extrusion.open-profile", || {
            let outer = vec![line([0.0, 0.0], [1.0, 0.0])];
            primitives::arc_edged_extrusion("open".into(), Frame3::IDENTITY, outer, 1.0, standard())
        }),
    ]
}

fn curved_with_holes() -> Vec<Case> {
    vec![
        built("arc-edged-extrusion-with-holes.oracle", || {
            let name = "arc-edged-extrusion-with-holes".into();
            primitives::arc_edged_extrusion_with_holes(
                name,
                ground(),
                rounded_outer(),
                vec![square_hole()],
                2.0,
                standard(),
            )
        }),
        built("arc-edged-extrusion-with-holes.round-hole", || {
            let outer = polygon(&rectangle([-2.0, -2.0], [2.0, 2.0]));
            let hole = band(1.0, 0.5, 0.0, PI);
            primitives::arc_edged_extrusion_with_holes(
                "round-hole".into(),
                Frame3::IDENTITY,
                outer,
                vec![hole],
                2.0,
                standard(),
            )
        }),
    ]
}

pub(crate) fn cases() -> Vec<Case> {
    [wires(), arcs(), linear(), curved(), curved_with_holes()]
        .into_iter()
        .flatten()
        .collect()
}
