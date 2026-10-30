use super::support::{along_axis, assert_cut, sloped, Cut, Cyl, Expected, HALF_SPACE};
use crate::brep::unit;
use crate::math::{add, scale};

fn horizontal() -> Cyl {
    Cyl {
        origin: [0.0, 0.0, 1.0],
        axis: [1.0, 0.0, 0.0],
        height: 6.0,
    }
}

fn vertical() -> Cyl {
    Cyl {
        origin: [0.0, 0.0, 0.0],
        axis: [0.0, 0.0, 1.0],
        height: 6.0,
    }
}

fn cut(point: [f64; 3], normal: [f64; 3], reference: [f64; 3]) -> Cut {
    Cut {
        point,
        normal,
        reference,
    }
}

fn half_space(faces: [usize; 2]) -> Expected {
    Expected {
        faces,
        handlers: HALF_SPACE,
    }
}

#[test]
fn horizontal_cylinder_across_keeps_the_perpendicular_cut() {
    assert_cut(
        horizontal(),
        cut([2.5, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        half_space([3, 3]),
    );
}

#[test]
fn horizontal_cylinder_along_keeps_the_arc_extrusion_and_prismatic_cuts() {
    assert_cut(
        horizontal(),
        cut([0.0, 0.2, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        Expected {
            faces: [6, 4],
            handlers: [
                "subtract_vertical_arc_extrusion",
                "prismatic_curved_boolean",
            ],
        },
    );
}

#[test]
fn horizontal_cylinder_oblique_cut_is_an_exact_ellipse_whole_or_clipped_by_a_cap() {
    let a = 30f64.to_radians();
    let normal = [-a.sin(), a.cos(), 0.0];
    for (x, faces) in [(3.0, [3, 3]), (5.6, [3, 4]), (0.4, [4, 3])] {
        assert_cut(
            horizontal(),
            cut([x, 0.0, 1.0], normal, [0.0, 0.0, 1.0]),
            half_space(faces),
        );
    }
}

#[test]
fn sloped_cylinder_across_oblique_and_parallel_cuts_build_exactly() {
    let b = 30f64.to_radians();
    for degrees in [15.0, 40.0] {
        let c = sloped(degrees, 6.0);
        let mid = along_axis(&c, 3.0);
        assert_cut(
            sloped(degrees, 6.0),
            cut(mid, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            half_space([3, 3]),
        );
        assert_cut(
            sloped(degrees, 6.0),
            cut(mid, [b.cos(), b.sin(), 0.0], [0.0, 0.0, 1.0]),
            half_space([3, 3]),
        );
        assert_cut(
            sloped(degrees, 6.0),
            cut([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
            half_space([4, 4]),
        );
    }
}

#[test]
fn sloped_cylinder_ellipse_clipped_by_the_low_cap_the_high_cap_and_both_caps() {
    let c = sloped(40.0, 6.0);
    for (t, faces) in [(0.2, [3, 4]), (5.8, [4, 3])] {
        assert_cut(
            sloped(40.0, 6.0),
            cut(along_axis(&c, t), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            half_space(faces),
        );
    }
    let short = sloped(40.0, 0.3);
    assert_cut(
        sloped(40.0, 0.3),
        cut(along_axis(&short, 0.15), [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        half_space([4, 4]),
    );
}

#[test]
fn near_perpendicular_cut_keeps_an_orthonormal_chart() {
    let c = sloped(40.0, 6.0);
    let axis = unit(c.axis).unwrap();
    for eps in [3e-5, 1e-5, 3e-6] {
        let normal = unit(add(axis, scale([0.0, 1.0, 0.0], eps))).unwrap();
        assert_cut(
            sloped(40.0, 6.0),
            cut(along_axis(&c, 3.0), normal, [0.0, 1.0, 0.0]),
            half_space([3, 3]),
        );
    }
}

#[test]
fn vertical_cylinder_cuts_keep_their_handlers() {
    assert_cut(
        vertical(),
        cut([0.0, 0.0, 2.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
        half_space([3, 3]),
    );
    assert_cut(
        vertical(),
        cut([0.2, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        Expected {
            faces: [5, 5],
            handlers: [
                "subtract_vertical_arc_extrusion",
                "prismatic_curved_boolean",
            ],
        },
    );
}
