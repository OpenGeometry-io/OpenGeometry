use crate::batch_case::{batch_table, both_orders, BatchBuilt, BatchEntry};
use crate::fixtures::{
    at, fine, ground, moved, polar, rectangle, standard, tilted_about_y, upright,
};
use crate::kernel::{primitives, Frame3, Point3};
use crate::planar_shapes::{framed, opening, wall, Shape};
use crate::profiles::{quarter_annulus, wide_arc};
use crate::runner::Case;
use std::f64::consts::{FRAC_PI_2, PI};

fn ground_cylinder(name: &str, origin: Point3, radius: f64, height: f64) -> Shape {
    primitives::cylinder(name.into(), moved(ground(), origin), radius, height, fine())
}

fn slab(name: &str, depth: f64, lo: [f64; 2], hi: [f64; 2], height: f64) -> Shape {
    framed(name, upright([0.0, depth, 0.0]), rectangle(lo, hi), height)
}

fn door(name: &str, from: f64, to: f64) -> Shape {
    opening(name, 0.0, 2.1, from, to)
}

fn two_cuboids() -> BatchBuilt {
    let block = |name: &str, x: f64| {
        primitives::cuboid(name.into(), at([x, 0.0, 0.0]), [1.0, 2.0, 2.0], standard())
    };
    let host = primitives::cuboid("host".into(), Frame3::IDENTITY, [6.0, 2.0, 2.0], standard())?;
    Ok((host, vec![block("left", 1.0)?, block("right", 4.0)?]))
}

fn ten_openings() -> BatchBuilt {
    let cutters = (0..10)
        .map(|index| {
            let left = 1.0 + index as f64 * 1.8;
            door(&format!("opening-{index}"), left, left + 0.5)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((wall("host", 20.0)?, cutters))
}

fn single_cutter() -> BatchBuilt {
    Ok((wall("host", 10.0)?, vec![door("lower_cutout", 2.0, 3.0)?]))
}

fn overlapping() -> BatchBuilt {
    let cutters = vec![door("first", 2.0, 3.0)?, door("second", 2.5, 3.5)?];
    Ok((wall("host", 10.0)?, cutters))
}

fn oblique_pair() -> BatchBuilt {
    let cutter = |name: &str, station: f64| {
        let frame = tilted_about_y([station, 0.15, 0.0], PI / 6.0);
        framed(name, frame, rectangle([-0.5, -0.5], [0.5, 0.5]), 2.1)
    };
    let cutters = vec![
        cutter("first-oblique", 2.5)?,
        cutter("second-oblique", 6.0)?,
    ];
    Ok((wall("oblique-host", 10.0)?, cutters))
}

fn single_oblique() -> BatchBuilt {
    let (host, mut cutters) = oblique_pair()?;
    cutters.truncate(1);
    Ok((host, cutters))
}

fn no_cutters() -> BatchBuilt {
    Ok((wall("host", 10.0)?, Vec::new()))
}

fn too_many_cutters() -> BatchBuilt {
    let cutters = (0..101)
        .map(|index| {
            let from = 0.05 * index as f64;
            door(&format!("crowded-{index}"), from, from + 0.01)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((wall("host", 10.0)?, cutters))
}

fn fifty_openings() -> BatchBuilt {
    let frame = ground();
    let outer = rectangle([-30.0, -0.1], [30.0, 0.1]);
    let name = "wall".into();
    let host = primitives::linear_extrusion(name, frame, outer, Vec::new(), 3.0, standard())?;
    let cutters = (0..50)
        .map(|index| {
            let origin = [-27.0 + index as f64 * 1.1 - 0.25, 0.0, 0.2];
            let name = format!("cut-{index}");
            primitives::cuboid(name, moved(frame, origin), [0.5, 0.4, 2.0], standard())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((host, cutters))
}

fn disjoint_mixed() -> BatchBuilt {
    let cylinder = ground_cylinder("round", [1.5, -0.5, 1.5], 0.4, 1.3)?;
    let slab = slab("rectangle", 0.8, [3.5, 0.7], [4.5, 2.2], 1.3)?;
    Ok((wall("mixed-host", 6.0)?, vec![cylinder, slab]))
}

fn two_round() -> BatchBuilt {
    let first = ground_cylinder("round-a", [1.5, 0.0, 1.5], 0.4, 0.3)?;
    let second = ground_cylinder("round-b", [4.5, 0.0, 1.5], 0.4, 0.3)?;
    Ok((wall("two-round-host", 6.0)?, vec![first, second]))
}

fn single_round() -> BatchBuilt {
    let (host, mut cutters) = two_round()?;
    cutters.truncate(1);
    Ok((host, cutters))
}

fn split_mixed() -> BatchBuilt {
    let cylinder = ground_cylinder("split-round", [1.5, -0.5, 1.5], 0.4, 1.3)?;
    let slab = slab("full-height", 0.3, [2.5, 0.0], [3.5, 3.0], 0.3)?;
    Ok((wall("split-mixed-host", 6.0)?, vec![cylinder, slab]))
}

fn flush_mixed() -> BatchBuilt {
    let cylinder = ground_cylinder("flush-round", [1.5, 0.0, 1.5], 0.4, 0.3)?;
    let slab = slab("flush-rectangle", 0.3, [3.5, 0.7], [4.5, 1.5], 0.3)?;
    Ok((wall("flush-mixed-host", 6.0)?, vec![cylinder, slab]))
}

fn arched() -> BatchBuilt {
    let cap = ground_cylinder("arched-cap", [2.0, -0.5, 2.0], 0.5, 1.3)?;
    let lower = slab("arched-lower", 0.8, [1.5, 0.5], [2.5, 2.0], 1.3)?;
    Ok((wall("arched-host", 4.0)?, vec![cap, lower]))
}

fn arc_host() -> BatchBuilt {
    let outer = quarter_annulus(2.0, 1.5);
    let name = "arc-host".into();
    let host = primitives::arc_edged_extrusion(name, Frame3::IDENTITY, outer, 3.0, fine())?;
    let size = [1.0, 0.35, 1.5];
    let first = primitives::cuboid("opening".into(), at([1.1, 0.9, 0.5]), size, fine())?;
    let size = [0.45, 0.8, 1.2];
    let second = primitives::cuboid("second-opening".into(), at([0.6, 1.3, 0.8]), size, fine())?;
    Ok((host, vec![first, second]))
}

fn wide_sector(index: usize) -> Shape {
    let start = -FRAC_PI_2;
    let opening_start = start + 1.2 / 2.0;
    let opening_end = start + 8.2 / 2.0;
    let from = opening_start + (opening_end - opening_start) * index as f64 / 3.0;
    let to = opening_start + (opening_end - opening_start) * (index + 1) as f64 / 3.0;
    let reach = 2.1 / ((to - from) / 2.0).cos() + 0.2;
    let outer = vec![[0.0, 0.0], polar(reach, from), polar(reach, to)];
    framed(&format!("sector-{index}"), Frame3::IDENTITY, outer, 2.0)
}

fn wide_opening() -> BatchBuilt {
    let name = "wide-arc".into();
    let host = primitives::arc_edged_extrusion(name, Frame3::IDENTITY, wide_arc(), 3.0, fine())?;
    let cutters = (0..3).map(wide_sector).collect::<Result<Vec<_>, _>>()?;
    Ok((host, cutters))
}

const ENTRIES: [BatchEntry; 13] = [
    ("planar.two-cuboids", "two", two_cuboids),
    ("planar.ten-flush-openings", "batch", ten_openings),
    ("planar.single-cutter", "single", single_cutter),
    ("planar.single-oblique", "single-oblique", single_oblique),
    ("planar.overlapping-cutters", "overlap", overlapping),
    ("planar.oblique-cutters", "oblique", oblique_pair),
    ("planar.no-cutters", "empty", no_cutters),
    ("planar.too-many-cutters", "crowded", too_many_cutters),
    ("planar.fifty-openings", "result", fifty_openings),
    ("prismatic.single-round", "single-round", single_round),
    ("prismatic.arched", "arched-batch", arched),
    ("vertical-arc.arc-host", "arc-cut-batch", arc_host),
    ("vertical-arc.wide-opening", "wide-cut", wide_opening),
];

const ORDERED: [BatchEntry; 4] = [
    ("prismatic.disjoint-mixed", "mixed", disjoint_mixed),
    ("prismatic.two-round", "two-round", two_round),
    ("prismatic.split-mixed", "split-mixed", split_mixed),
    ("prismatic.flush-mixed", "flush-mixed", flush_mixed),
];

pub(crate) fn cases() -> Vec<Case> {
    let mut cases = batch_table(&ENTRIES);
    cases.extend(both_orders(&ORDERED));
    cases
}
