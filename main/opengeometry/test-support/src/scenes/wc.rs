use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3};
use opengeometry::operations::modifying::boolean::shell_brep;
use opengeometry::primitives::{arc_edged_extrusion, ProfileEdge};
use std::f64::consts::TAU;

pub const PAN_VOLUME: f64 = 0.010544;

pub const SEAT_VOLUME: f64 = 0.0017897;

fn ground(point: [f64; 2]) -> [f64; 2] {
    [point[0], -point[1]]
}

fn line(from: [f64; 2], to: [f64; 2]) -> ProfileEdge {
    ProfileEdge::Line {
        from: ground(from),
        to: ground(to),
    }
}

fn arc(from: [f64; 2], to: [f64; 2], center: [f64; 2], counter_clockwise: bool) -> ProfileEdge {
    let [from, to, center] = [from, to, center].map(ground);
    let radius = (from[0] - center[0]).hypot(from[1] - center[1]);
    let start_angle = (from[1] - center[1]).atan2(from[0] - center[0]);
    let mut sweep_angle = (to[1] - center[1]).atan2(to[0] - center[0]) - start_angle;
    if counter_clockwise && sweep_angle <= 1e-12 {
        sweep_angle += TAU;
    } else if !counter_clockwise && sweep_angle >= -1e-12 {
        sweep_angle -= TAU;
    }
    ProfileEdge::Arc {
        center,
        radius,
        start_angle,
        sweep_angle,
    }
}

fn extrusion(id: &str, base: f64, top: f64, profile: Vec<ProfileEdge>) -> BrepEnvelope {
    let frame = Frame3 {
        origin: [0.0, 0.0, base],
        ..Frame3::IDENTITY
    };
    arc_edged_extrusion(id.into(), frame, profile, top - base, Accuracy::STANDARD).unwrap()
}

pub fn pan_shell() -> BrepEnvelope {
    let rim = [0.1053574007, 0.2624440433];
    let profile = vec![
        line([-0.185, -0.325], [-0.185, -0.05]),
        arc(
            [-0.185, -0.05],
            [-rim[0], rim[1]],
            [0.4676923077, -0.05],
            true,
        ),
        arc([-rim[0], rim[1]], rim, [0.0, 0.205], true),
        arc(rim, [0.185, -0.05], [-0.4676923077, -0.05], true),
        line([0.185, -0.05], [0.185, -0.325]),
        line([0.185, -0.325], [-0.185, -0.325]),
    ];
    let solid = extrusion("panSolid", 0.18, 0.4, profile);
    shell_brep(&solid, 0.015, "panShell".into()).unwrap().brep
}

pub fn bowl_opening() -> BrepEnvelope {
    let points = [
        [0.12, 0.08],
        [0.054, 0.212],
        [-0.054, 0.212],
        [-0.12, 0.08],
        [-0.054, -0.052],
        [0.054, -0.052],
    ];
    let centers = [
        [-0.045, 0.08],
        [0.0, 0.14],
        [0.045, 0.08],
        [0.045, 0.08],
        [0.0, 0.02],
        [-0.045, 0.08],
    ];
    let profile = (0..6)
        .map(|index| {
            arc(
                points[index],
                points[(index + 1) % 6],
                centers[index],
                false,
            )
        })
        .collect();
    extrusion("bowlOpening", 0.35, 0.41, profile)
}

pub fn seat_solid() -> BrepEnvelope {
    let rim = [0.1053574007, 0.2624440433];
    let profile = vec![
        line([-0.185, -0.12], [0.185, -0.12]),
        line([0.185, -0.12], [0.185, -0.05]),
        arc([0.185, -0.05], rim, [-0.4676923077, -0.05], false),
        arc(rim, [-rim[0], rim[1]], [0.0, 0.205], false),
        arc(
            [-rim[0], rim[1]],
            [-0.185, -0.05],
            [0.4676923077, -0.05],
            false,
        ),
        line([-0.185, -0.05], [-0.185, -0.12]),
    ];
    extrusion("seatSolid", 0.4, 0.42, profile)
}

pub fn seat_hole() -> BrepEnvelope {
    let (low, high, center) = ([0.0, -0.04], [0.0, 0.2], [0.0, 0.08]);
    let profile = vec![arc(low, high, center, false), arc(high, low, center, false)];
    extrusion("seatHole", 0.39, 0.43, profile)
}
