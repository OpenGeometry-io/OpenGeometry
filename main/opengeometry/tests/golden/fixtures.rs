use crate::kernel::{Accuracy, Frame3, GeometryError, Point3};

pub(crate) fn accuracy(
    geometric: f64,
    intersection: f64,
    tessellation: f64,
    exchange: f64,
) -> Accuracy {
    Accuracy {
        geometric,
        intersection,
        tessellation,
        exchange,
    }
}

pub(crate) fn standard() -> Accuracy {
    accuracy(1e-8, 1e-9, 0.01, 1e-6)
}

pub(crate) fn fine() -> Accuracy {
    accuracy(1e-9, 1e-10, 0.01, 1e-5)
}

pub(crate) fn scaled_accuracy(size: f64) -> Accuracy {
    accuracy(size * 1e-9, size * 1e-10, size * 0.01, size * 1e-5)
}

pub(crate) fn ground() -> Frame3 {
    Frame3 {
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, -1.0],
        z: [0.0, 1.0, 0.0],
        ..Frame3::IDENTITY
    }
}

pub(crate) fn upright(origin: Point3) -> Frame3 {
    Frame3 {
        origin,
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [0.0, -1.0, 0.0],
    }
}

pub(crate) fn at(origin: Point3) -> Frame3 {
    Frame3 {
        origin,
        ..Frame3::IDENTITY
    }
}

pub(crate) fn moved(frame: Frame3, origin: Point3) -> Frame3 {
    Frame3 { origin, ..frame }
}

pub(crate) fn tilted_about_y(origin: Point3, angle: f64) -> Frame3 {
    Frame3 {
        origin,
        x: [angle.cos(), 0.0, -angle.sin()],
        y: [0.0, 1.0, 0.0],
        z: [angle.sin(), 0.0, angle.cos()],
    }
}

pub(crate) fn tilted_about_x(origin: Point3, angle: f64) -> Frame3 {
    Frame3 {
        origin,
        x: [1.0, 0.0, 0.0],
        y: [0.0, angle.cos(), -angle.sin()],
        z: [0.0, angle.sin(), angle.cos()],
    }
}

pub(crate) fn turned_about_z(origin: Point3, angle: f64) -> Frame3 {
    Frame3 {
        origin,
        x: [angle.cos(), angle.sin(), 0.0],
        y: [-angle.sin(), angle.cos(), 0.0],
        z: [0.0, 0.0, 1.0],
    }
}

pub(crate) fn y_axis_frame(origin: Point3) -> Result<Frame3, GeometryError> {
    Frame3::from_axis(origin, [0.0, 1.0, 0.0], [0.8, 0.0, 0.6])
}

pub(crate) fn reversed_z(origin: Point3) -> Frame3 {
    Frame3 {
        origin,
        x: [1.0, 0.0, 0.0],
        y: [0.0, -1.0, 0.0],
        z: [0.0, 0.0, -1.0],
    }
}

pub(crate) fn rectangle(lo: [f64; 2], hi: [f64; 2]) -> Vec<[f64; 2]> {
    vec![
        [lo[0], lo[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
        [lo[0], hi[1]],
    ]
}

pub(crate) fn polar(radius: f64, angle: f64) -> [f64; 2] {
    [radius * angle.cos(), radius * angle.sin()]
}
