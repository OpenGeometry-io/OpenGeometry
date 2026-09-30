use super::parts::{
    accuracy, at, band, door, fine, part, polar, prism, quarter_annulus, rectangle, round, slab,
    tilted_about_y, wall, Part, Shape,
};
use opengeometry::analytic::{topology::Accuracy, Frame3, Point3};
use std::f64::consts::{FRAC_PI_2, PI};

pub(super) struct Scene {
    pub(super) name: &'static str,
    pub(super) host: Part,
    pub(super) cutters: Vec<Part>,
    pub(super) ordered: bool,
}

pub(super) fn scenes(ground: Frame3, standard: Accuracy) -> Vec<Scene> {
    planar_scenes(ground, standard)
        .into_iter()
        .chain(prismatic_scenes(ground))
        .chain(arc_scenes(ground))
        .collect()
}

fn planar_scenes(ground: Frame3, standard: Accuracy) -> Vec<Scene> {
    let doors = |count: usize, prefix: &str, from: f64, pitch: f64, width: f64| {
        (0..count)
            .map(|index| {
                let left = from + index as f64 * pitch;
                door(&format!("{prefix}-{index}"), left, left + width)
            })
            .collect::<Vec<_>>()
    };
    let overlapping = vec![door("first", 2.0, 3.0), door("second", 2.5, 3.5)];
    let single = vec![door("lower_cutout", 2.0, 3.0)];
    vec![
        two_cuboids(standard),
        scene(
            "planar.ten-flush-openings",
            wall("host", 20.0),
            doors(10, "opening", 1.0, 1.8, 0.5),
        ),
        scene("planar.single-cutter", wall("host", 10.0), single),
        scene(
            "planar.single-oblique",
            wall("oblique-host", 10.0),
            oblique_cutters(1),
        ),
        scene(
            "planar.overlapping-cutters",
            wall("host", 10.0),
            overlapping,
        ),
        scene(
            "planar.oblique-cutters",
            wall("oblique-host", 10.0),
            oblique_cutters(2),
        ),
        scene("planar.no-cutters", wall("host", 10.0), Vec::new()),
        scene(
            "planar.too-many-cutters",
            wall("host", 10.0),
            doors(101, "crowded", 0.0, 0.05, 0.01),
        ),
        fifty_openings(ground, standard),
    ]
}

fn two_cuboids(standard: Accuracy) -> Scene {
    let block = |name: &str, x: f64| {
        part(
            name,
            at([x, 0.0, 0.0]),
            standard,
            Shape::Cuboid([1.0, 2.0, 2.0]),
        )
    };
    let host = part(
        "host",
        Frame3::IDENTITY,
        standard,
        Shape::Cuboid([6.0, 2.0, 2.0]),
    );
    scene(
        "planar.two-cuboids",
        host,
        vec![block("left", 1.0), block("right", 4.0)],
    )
}

fn oblique_cutters(count: usize) -> Vec<Part> {
    [("first-oblique", 2.5), ("second-oblique", 6.0)]
        .into_iter()
        .take(count)
        .map(|(name, station)| {
            let frame = tilted_about_y([station, 0.15, 0.0], PI / 6.0);
            prism(name, frame, rectangle([-0.5, -0.5], [0.5, 0.5]), 2.1)
        })
        .collect()
}

fn fifty_openings(ground: Frame3, standard: Accuracy) -> Scene {
    let outer = rectangle([-30.0, -0.1], [30.0, 0.1]);
    let host = part(
        "wall",
        ground,
        standard,
        Shape::Prism { outer, height: 3.0 },
    );
    let cutters = (0..50)
        .map(|index| {
            let origin = [-27.0 + index as f64 * 1.1 - 0.25, 0.0, 0.2];
            let frame = Frame3 { origin, ..ground };
            part(
                &format!("cut-{index}"),
                frame,
                standard,
                Shape::Cuboid([0.5, 0.4, 2.0]),
            )
        })
        .collect();
    scene("planar.fifty-openings", host, cutters)
}

fn prismatic_scenes(ground: Frame3) -> Vec<Scene> {
    let upright_round = |name: &str, origin: Point3, radius: f64, height: f64| {
        round(name, Frame3 { origin, ..ground }, radius, height)
    };
    let round_a = || upright_round("round-a", [1.5, 0.0, 1.5], 0.4, 0.3);
    let round_b = upright_round("round-b", [4.5, 0.0, 1.5], 0.4, 0.3);
    let disjoint = vec![
        upright_round("round", [1.5, -0.5, 1.5], 0.4, 1.3),
        slab("rectangle", 0.8, [3.5, 0.7], [4.5, 2.2], 1.3),
    ];
    let split = vec![
        upright_round("split-round", [1.5, -0.5, 1.5], 0.4, 1.3),
        slab("full-height", 0.3, [2.5, 0.0], [3.5, 3.0], 0.3),
    ];
    let flush = vec![
        upright_round("flush-round", [1.5, 0.0, 1.5], 0.4, 0.3),
        slab("flush-rectangle", 0.3, [3.5, 0.7], [4.5, 1.5], 0.3),
    ];
    let arched = vec![
        upright_round("arched-cap", [2.0, -0.5, 2.0], 0.5, 1.3),
        slab("arched-lower", 0.8, [1.5, 0.5], [2.5, 2.0], 1.3),
    ];
    vec![
        scene(
            "prismatic.single-round",
            wall("two-round-host", 6.0),
            vec![round_a()],
        ),
        scene("prismatic.arched", wall("arched-host", 4.0), arched),
        ordered(
            "prismatic.disjoint-mixed",
            wall("mixed-host", 6.0),
            disjoint,
        ),
        ordered(
            "prismatic.two-round",
            wall("two-round-host", 6.0),
            vec![round_a(), round_b],
        ),
        ordered(
            "prismatic.split-mixed",
            wall("split-mixed-host", 6.0),
            split,
        ),
        ordered(
            "prismatic.flush-mixed",
            wall("flush-mixed-host", 6.0),
            flush,
        ),
    ]
}

fn arc_scenes(ground: Frame3) -> Vec<Scene> {
    let arc_prism = |name: &str, frame: Frame3, accuracy: Accuracy, outer| {
        part(
            name,
            frame,
            accuracy,
            Shape::ArcPrism { outer, height: 3.0 },
        )
    };
    let opening = |name: &str, origin: Point3, size: [f64; 3]| {
        part(name, at(origin), fine(), Shape::Cuboid(size))
    };
    let arc_host = arc_prism(
        "arc-host",
        Frame3::IDENTITY,
        fine(),
        quarter_annulus(2.0, 1.5),
    );
    let arc_openings = vec![
        opening("opening", [1.1, 0.9, 0.5], [1.0, 0.35, 1.5]),
        opening("second-opening", [0.6, 1.3, 0.8], [0.45, 0.8, 1.2]),
    ];
    let wide_outer = band(2.1, 1.9, -FRAC_PI_2, 3.0 * FRAC_PI_2);
    let wide_host = arc_prism("wide-arc", Frame3::IDENTITY, fine(), wide_outer);
    let staged_host = arc_prism(
        "mixed-curved-host",
        ground,
        staged(),
        band(3.0, 2.7, 0.8, -1.6),
    );
    vec![
        scene("vertical-arc.arc-host", arc_host, arc_openings),
        scene(
            "vertical-arc.wide-opening",
            wide_host,
            (0..3).map(wide_sector).collect(),
        ),
        ordered(
            "staged.mixed-curved-openings",
            staged_host,
            staged_openings(),
        ),
    ]
}

fn scene(name: &'static str, host: Part, cutters: Vec<Part>) -> Scene {
    Scene {
        name,
        host,
        cutters,
        ordered: false,
    }
}

fn ordered(name: &'static str, host: Part, cutters: Vec<Part>) -> Scene {
    Scene {
        ordered: true,
        ..scene(name, host, cutters)
    }
}

fn wide_sector(index: usize) -> Part {
    let start = -FRAC_PI_2;
    let opening_start = start + 1.2 / 2.0;
    let opening_end = start + 8.2 / 2.0;
    let from = opening_start + (opening_end - opening_start) * index as f64 / 3.0;
    let to = opening_start + (opening_end - opening_start) * (index + 1) as f64 / 3.0;
    let reach = 2.1 / ((to - from) / 2.0).cos() + 0.2;
    let outer = vec![[0.0, 0.0], polar(reach, from), polar(reach, to)];
    prism(&format!("sector-{index}"), Frame3::IDENTITY, outer, 2.0)
}

fn staged() -> Accuracy {
    accuracy([6.3e-8, 1.575e-8, 0.01, 1e-5])
}

fn staged_openings() -> Vec<Part> {
    let frame = Frame3 {
        origin: [2.65, 1.5, 0.0],
        x: [0.0, 0.0, -1.0],
        y: [0.0, 1.0, 0.0],
        z: [1.0, 0.0, 0.0],
    };
    vec![
        part(
            "rectangular-opening",
            at([2.2, 1.0, 1.1]),
            staged(),
            Shape::Cuboid([1.0, 0.5, 0.4]),
        ),
        part(
            "round-opening",
            frame,
            staged(),
            Shape::Cylinder {
                radius: 0.5,
                height: 0.4,
            },
        ),
    ]
}
