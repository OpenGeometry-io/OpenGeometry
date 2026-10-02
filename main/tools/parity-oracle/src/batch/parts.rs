use opengeometry::analytic::{
    primitives::{self, ProfileEdge},
    topology::{Accuracy, BrepEnvelope},
    Frame3, GeometryError, Point3,
};
use std::f64::consts::FRAC_PI_2;

pub(super) enum Shape {
    Cuboid([f64; 3]),
    Cylinder {
        radius: f64,
        height: f64,
    },
    Prism {
        outer: Vec<[f64; 2]>,
        height: f64,
    },
    ArcPrism {
        outer: Vec<ProfileEdge>,
        height: f64,
    },
}

pub(super) struct Part {
    name: String,
    frame: Frame3,
    accuracy: Accuracy,
    shape: Shape,
}

pub(super) fn body(part: Part) -> Result<BrepEnvelope, GeometryError> {
    let Part {
        name,
        frame,
        accuracy,
        shape,
    } = part;
    match shape {
        Shape::Cuboid(size) => primitives::cuboid(name, frame, size, accuracy),
        Shape::Cylinder { radius, height } => {
            primitives::cylinder(name, frame, radius, height, accuracy)
        }
        Shape::Prism { outer, height } => {
            primitives::linear_extrusion(name, frame, outer, Vec::new(), height, accuracy)
        }
        Shape::ArcPrism { outer, height } => {
            primitives::arc_edged_extrusion(name, frame, outer, height, accuracy)
        }
    }
}

pub(super) fn accuracy([geometric, intersection, tessellation, exchange]: [f64; 4]) -> Accuracy {
    Accuracy {
        geometric,
        intersection,
        tessellation,
        exchange,
    }
}

pub(super) fn fine() -> Accuracy {
    accuracy([1e-9, 1e-10, 0.01, 1e-5])
}

pub(super) fn at(origin: Point3) -> Frame3 {
    Frame3 {
        origin,
        ..Frame3::IDENTITY
    }
}

fn upright(origin: Point3) -> Frame3 {
    Frame3 {
        origin,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [0.0, -1.0, 0.0],
    }
}

pub(super) fn tilted_about_y(origin: Point3, angle: f64) -> Frame3 {
    Frame3 {
        origin,
        x: [angle.cos(), 0.0, -angle.sin()],
        y: [0.0, 1.0, 0.0],
        z: [angle.sin(), 0.0, angle.cos()],
    }
}

pub(super) fn rectangle(lo: [f64; 2], hi: [f64; 2]) -> Vec<[f64; 2]> {
    vec![
        [lo[0], lo[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
        [lo[0], hi[1]],
    ]
}

pub(super) fn polar(radius: f64, angle: f64) -> [f64; 2] {
    [radius * angle.cos(), radius * angle.sin()]
}

pub(super) fn part(name: &str, frame: Frame3, accuracy: Accuracy, shape: Shape) -> Part {
    Part {
        name: name.into(),
        frame,
        accuracy,
        shape,
    }
}

pub(super) fn prism(name: &str, frame: Frame3, outer: Vec<[f64; 2]>, height: f64) -> Part {
    part(name, frame, fine(), Shape::Prism { outer, height })
}

pub(super) fn wall(name: &str, length: f64) -> Part {
    prism(
        name,
        at([0.0; 3]),
        rectangle([0.0, 0.0], [length, 0.3]),
        3.0,
    )
}

pub(super) fn door(name: &str, from: f64, to: f64) -> Part {
    prism(name, at([0.0; 3]), rectangle([from, 0.0], [to, 0.3]), 2.1)
}

pub(super) fn slab(name: &str, depth: f64, lo: [f64; 2], hi: [f64; 2], height: f64) -> Part {
    prism(name, upright([0.0, depth, 0.0]), rectangle(lo, hi), height)
}

pub(super) fn round(name: &str, frame: Frame3, radius: f64, height: f64) -> Part {
    part(name, frame, fine(), Shape::Cylinder { radius, height })
}

fn arc(radius: f64, start_angle: f64, sweep_angle: f64) -> ProfileEdge {
    ProfileEdge::Arc {
        center: [0.0, 0.0],
        radius,
        start_angle,
        sweep_angle,
    }
}

pub(super) fn quarter_annulus(outer: f64, inner: f64) -> Vec<ProfileEdge> {
    vec![
        arc(outer, 0.0, FRAC_PI_2),
        ProfileEdge::Line {
            from: [0.0, outer],
            to: [0.0, inner],
        },
        arc(inner, FRAC_PI_2, -FRAC_PI_2),
        ProfileEdge::Line {
            from: [inner, 0.0],
            to: [outer, 0.0],
        },
    ]
}

pub(super) fn band(outer: f64, inner: f64, start: f64, sweep: f64) -> Vec<ProfileEdge> {
    vec![
        arc(outer, start, sweep),
        ProfileEdge::Line {
            from: polar(outer, start + sweep),
            to: polar(inner, start + sweep),
        },
        arc(inner, start + sweep, -sweep),
        ProfileEdge::Line {
            from: polar(inner, start),
            to: polar(outer, start),
        },
    ]
}
