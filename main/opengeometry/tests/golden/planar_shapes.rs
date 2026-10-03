use crate::fixtures::{at, fine, rectangle};
use crate::kernel::{primitives, BrepEnvelope, Frame3, GeometryError, Point3};
use std::f64::consts::PI;

pub(crate) type Shape = Result<BrepEnvelope, GeometryError>;

pub(crate) fn span(name: &str, bottom: f64, height: f64, outer: Vec<[f64; 2]>) -> Shape {
    holed(name, bottom, height, outer, Vec::new())
}

pub(crate) fn holed(
    name: &str,
    bottom: f64,
    height: f64,
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
) -> Shape {
    let frame = at([0.0, 0.0, bottom]);
    primitives::linear_extrusion(name.into(), frame, outer, holes, height, fine())
}

pub(crate) fn profile_shell() -> Shape {
    let outer = rectangle([0.0, 0.0], [4.0, 4.0]);
    let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
    holed("profile-shell", 0.0, 3.0, outer, vec![hole])
}

pub(crate) fn extrusion(name: &str, outer: Vec<[f64; 2]>) -> Shape {
    span(name, 0.0, 3.0, outer)
}

pub(crate) fn wall(name: &str, length: f64) -> Shape {
    extrusion(name, rectangle([0.0, 0.0], [length, 0.3]))
}

pub(crate) fn angled(name: &str) -> Shape {
    let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 0.3], [0.3, 0.3], [0.0, 0.1]];
    extrusion(name, outer)
}

pub(crate) fn opening(name: &str, bottom: f64, height: f64, from: f64, to: f64) -> Shape {
    span(name, bottom, height, rectangle([from, 0.0], [to, 0.3]))
}

pub(crate) fn square(half: f64) -> Vec<[f64; 2]> {
    rectangle([-half, -half], [half, half])
}

pub(crate) fn framed(name: &str, frame: Frame3, outer: Vec<[f64; 2]>, height: f64) -> Shape {
    primitives::linear_extrusion(name.into(), frame, outer, Vec::new(), height, fine())
}

pub(crate) fn oblique(name: &str, origin: Point3, sign: f64, outer: Vec<[f64; 2]>) -> Shape {
    let angle = PI / 12.0;
    let frame = Frame3 {
        origin,
        x: [angle.cos(), 0.0, -sign * angle.sin()],
        y: [0.0, 1.0, 0.0],
        z: [sign * angle.sin(), 0.0, angle.cos()],
    };
    framed(name, frame, outer, 2.1)
}
