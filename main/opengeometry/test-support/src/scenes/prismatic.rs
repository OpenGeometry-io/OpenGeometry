use crate::world_graph::{graph, named};
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, Plane, Primitive, Transform, WorldGraph,
};
use std::f64::consts::PI;

pub const LEG_RADIUS: f64 = 0.1;

pub const LEG_HEIGHT: f64 = 0.7;

pub const BOX_VOLUME: f64 = 0.5;

pub fn d_shape(width: f64, depth: f64, inset: f64) -> Primitive {
    let straight = width - depth / 2.0;
    let edge = |to: [f64; 2]| ProfileSegment { to, arc: None };
    Primitive::Profile {
        start: [inset, inset],
        segments: vec![
            edge([straight, inset]),
            ProfileSegment {
                to: [straight, depth - inset],
                arc: Some(SegmentArc {
                    center: [straight, depth / 2.0],
                    counter_clockwise: true,
                }),
            },
            edge([inset, depth - inset]),
            edge([inset, inset]),
        ],
        closed: true,
    }
}

pub fn d_shape_area(width: f64, depth: f64, inset: f64) -> f64 {
    let straight = width - depth / 2.0;
    let radius = depth / 2.0 - inset;
    (straight - inset) * (depth - 2.0 * inset) + PI * radius * radius / 2.0
}

pub fn extrude_profile(
    world: &mut WorldGraph,
    og_id: &str,
    profile: Primitive,
    origin: [f64; 3],
    distance: f64,
) {
    let profile_id = format!("{og_id}-profile");
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some(origin),
            normal: None,
            x_direction: None,
        }),
        ..named(&profile_id)
    };
    world.create_primitive(profile, options).unwrap();
    world
        .create_operation(
            CreatingOperation::Extrude {
                profile: profile_id,
                holes: Vec::new(),
                distance,
            },
            named(og_id),
        )
        .unwrap();
}

pub fn pan_and_inset(inset_height: f64) -> WorldGraph {
    let mut world = graph();
    extrude_profile(&mut world, "pan", d_shape(0.5, 0.35, 0.0), [0.0; 3], 0.4);
    extrude_profile(
        &mut world,
        "inset",
        d_shape(0.5, 0.35, 0.03),
        [0.0, 0.05, 0.0],
        inset_height,
    );
    world
}

pub fn pan_volume() -> f64 {
    d_shape_area(0.5, 0.35, 0.0) * 0.4 - d_shape_area(0.5, 0.35, 0.03) * 0.35
}

pub fn add_box(world: &mut WorldGraph) {
    world
        .create_primitive(
            Primitive::Cuboid {
                width: 1.0,
                height: 0.5,
                depth: 1.0,
            },
            named("box"),
        )
        .unwrap();
}

pub fn box_and_leg(degrees: f64, offset: [f64; 3]) -> WorldGraph {
    let mut world = graph();
    add_box(&mut world);
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: LEG_RADIUS,
                height: LEG_HEIGHT,
            },
            named("leg"),
        )
        .unwrap();
    world
        .transform(
            "leg",
            Transform::Rotate {
                axis: [1.0, 0.0, 0.0],
                degrees,
                pivot: Some([0.0, LEG_HEIGHT / 2.0, 0.0]),
            },
        )
        .unwrap();
    world
        .transform("leg", Transform::Translate { offset })
        .unwrap();
    world
}

pub fn box_and_pocket(width: f64) -> WorldGraph {
    let mut world = graph();
    add_box(&mut world);
    extrude_profile(
        &mut world,
        "pocket",
        d_shape(width, 0.35, 0.0),
        [-0.25, 0.3, 0.175],
        0.4,
    );
    world
}
