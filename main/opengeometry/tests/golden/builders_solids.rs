use crate::builder_case::built;
use crate::fixtures::{
    at, fine, ground, moved, scaled_accuracy, standard, tilted_about_y, y_axis_frame,
};
use crate::kernel::{placed, primitives, Frame3, GeometryError};
use crate::runner::Case;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn skewed() -> Result<Frame3, GeometryError> {
    Frame3::from_axis([1.0, -2.0, 0.5], [1.0, 1.0, 1.0], [1.0, 0.0, 0.0])
}

fn cuboids() -> Vec<Case> {
    vec![
        built("cuboid.oracle", || {
            primitives::cuboid("cuboid".into(), ground(), [2.0, 1.0, 3.0], standard())
        }),
        built("cuboid.identity", || {
            primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], standard())
        }),
        built("cuboid.translated", || {
            let frame = at([10.0, -7.0, 4.0]);
            primitives::cuboid("translated".into(), frame, [2.0, 3.0, 4.0], standard())
        }),
        built("cuboid.skewed", || {
            primitives::cuboid("skewed".into(), skewed()?, [1.0, 2.0, 0.5], fine())
        }),
        built("cuboid.zero-size", || {
            primitives::cuboid("zero".into(), Frame3::IDENTITY, [0.0, 1.0, 1.0], standard())
        }),
    ]
}

fn cylinders() -> Vec<Case> {
    vec![
        built("cylinder.oracle", || {
            primitives::cylinder("cylinder".into(), ground(), 1.0, 2.0, standard())
        }),
        built("cylinder.identity-fine", || {
            primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, fine())
        }),
        built("cylinder.skewed", || {
            primitives::cylinder("skewed".into(), skewed()?, 0.6, 4.0, fine())
        }),
        built("cylinder.zero-radius", || {
            primitives::cylinder("zero".into(), Frame3::IDENTITY, 0.0, 2.0, standard())
        }),
        built("cylinder.negative-height", || {
            primitives::cylinder("negative".into(), Frame3::IDENTITY, 1.0, -2.0, standard())
        }),
        built("cylinder-sector.quarter", || {
            let frame = Frame3::IDENTITY;
            primitives::cylinder_sector("sector".into(), frame, 1.0, 2.0, 0.0, FRAC_PI_2, fine())
        }),
        built("cylinder-sector.ground", || {
            primitives::cylinder_sector("sector".into(), ground(), 2.0, 1.0, 0.3, 4.0, standard())
        }),
        built("cylinder-sector.full-turn", || {
            let frame = Frame3::IDENTITY;
            primitives::cylinder_sector("sector".into(), frame, 1.0, 1.0, 0.0, TAU, standard())
        }),
        built("cylinder-sector.zero-sweep", || {
            let frame = Frame3::IDENTITY;
            primitives::cylinder_sector("sector".into(), frame, 1.0, 1.0, 0.0, 0.0, standard())
        }),
    ]
}

fn annular() -> Vec<Case> {
    vec![
        built("annular-cylinder.oracle", || {
            let name = "annular-cylinder".into();
            primitives::annular_cylinder(name, ground(), 0.4, 1.0, 2.0, standard())
        }),
        built("annular-cylinder.thin-ring", || {
            let name = "ring-host".into();
            primitives::annular_cylinder(name, Frame3::IDENTITY, 1.9, 2.1, 3.0, fine())
        }),
        built("annular-cylinder.inverted-radii", || {
            let name = "inverted".into();
            primitives::annular_cylinder(name, Frame3::IDENTITY, 1.0, 0.5, 1.0, standard())
        }),
        built("cylinder-with-hole.oracle", || {
            let frame = ground();
            let name = "cylinder-with-hole".into();
            primitives::cylinder_with_circular_hole(name, frame, frame, 0.4, 1.0, 2.0, standard())
        }),
        built("cylinder-with-hole.off-centre", || {
            let frame = ground();
            let inner = moved(frame, frame.point([0.3, 0.0, 0.0]));
            let name = "off-centre".into();
            primitives::cylinder_with_circular_hole(name, frame, inner, 0.4, 1.0, 2.0, standard())
        }),
        built("cylinder-with-hole.breaching", || {
            let frame = Frame3::IDENTITY;
            let inner = at([0.8, 0.0, 0.0]);
            let name = "breaching".into();
            primitives::cylinder_with_circular_hole(name, frame, inner, 0.4, 1.0, 2.0, standard())
        }),
    ]
}

fn conics() -> Vec<Case> {
    vec![
        built("cone.oracle", || {
            primitives::cone("cone".into(), ground(), 1.0, 2.0, standard())
        }),
        built("cone.identity", || {
            primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 2.0, fine())
        }),
        built("cone.zero-height", || {
            primitives::cone("cone".into(), Frame3::IDENTITY, 1.0, 0.0, standard())
        }),
        built("frustum.oracle", || {
            primitives::frustum("frustum".into(), ground(), 1.0, 0.4, 2.0, standard())
        }),
        built("frustum.expanding", || {
            primitives::frustum("frustum".into(), Frame3::IDENTITY, 3.0, 4.0, 4.0, fine())
        }),
        built("frustum.shell-family", || {
            primitives::frustum("frustum".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, fine())
        }),
        built("frustum.negative-radius", || {
            primitives::frustum(
                "frustum".into(),
                Frame3::IDENTITY,
                -1.0,
                1.0,
                1.0,
                standard(),
            )
        }),
    ]
}

fn rounds() -> Vec<Case> {
    vec![
        built("sphere.oracle", || {
            primitives::sphere("sphere".into(), ground(), 1.0, standard())
        }),
        built("sphere.offset", || {
            primitives::sphere("sphere".into(), at([5.0, -3.0, 8.0]), 2.0, fine())
        }),
        built("sphere.micro", || {
            primitives::sphere(
                "micro".into(),
                Frame3::IDENTITY,
                1e-6,
                scaled_accuracy(1e-6),
            )
        }),
        built("sphere.zero-radius", || {
            primitives::sphere("zero".into(), Frame3::IDENTITY, 0.0, standard())
        }),
        built("torus.oracle", || {
            primitives::torus("torus".into(), ground(), 2.0, 0.5, standard())
        }),
        built("torus.ring", || {
            primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, fine())
        }),
        built("torus.self-intersecting", || {
            primitives::torus("torus".into(), Frame3::IDENTITY, 1.0, 1.5, standard())
        }),
    ]
}

fn placements() -> Vec<Case> {
    vec![
        built("placed.cuboid-scaled", || {
            let source = primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], standard())?;
            placed(&source, y_axis_frame([5.0, 6.0, 7.0])?, 1.25)
        }),
        built("placed.cylinder-rotated", || {
            let source = primitives::cylinder(
                "c".into(),
                moved(ground(), [0.0, 1.2, 0.0]),
                1.0,
                2.0,
                fine(),
            )?;
            placed(&source, tilted_about_y([0.2, 0.1, 0.2], PI / 7.0), 1.25)
        }),
        built("placed.zero-scale", || {
            let source = primitives::sphere("s".into(), Frame3::IDENTITY, 1.0, standard())?;
            placed(&source, Frame3::IDENTITY, 0.0)
        }),
    ]
}

pub(crate) fn cases() -> Vec<Case> {
    [
        cuboids(),
        cylinders(),
        annular(),
        conics(),
        rounds(),
        placements(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
