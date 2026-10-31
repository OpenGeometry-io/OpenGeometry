use opengeometry::brep::{Accuracy, BrepEnvelope, Frame3, GeometryError};
use opengeometry::operations::modifying::boolean::{boolean_brep_outcome_with_handlers, BooleanOp};
use opengeometry::primitives::{self, ProfileSegment, SegmentArc};
use opengeometry::tessellation::measure;
use opengeometry::world_graph::{
    CreatingOperation, EditScope, ModifyingOperation, Primitive, Transform, WorldGraph,
};
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::PI;

const FINE: Accuracy = Accuracy {
    geometric: 1e-9,
    intersection: 1e-10,
    tessellation: 1e-3,
    exchange: 1e-5,
};
const TILTS: [f64; 6] = [15.0, 30.0, 45.0, 60.0, 75.0, 85.0];
const STEPS: [f64; 3] = [1e-2, 1e-3, 1e-4];

#[derive(Clone, Copy)]
struct Cut {
    radius: f64,
    height: f64,
    tilt: f64,
    offset: f64,
}

struct Exact {
    volume: f64,
    area: f64,
}

impl Cut {
    fn normal(&self) -> [f64; 3] {
        let tilt = self.tilt.to_radians();
        [-tilt.sin(), tilt.cos(), 0.0]
    }

    fn point(&self) -> [f64; 3] {
        let n = self.normal();
        [
            n[0] * self.offset,
            self.height / 2.0 + n[1] * self.offset,
            0.0,
        ]
    }

    fn body(&self) -> Option<BrepEnvelope> {
        let axis = Frame3::from_axis([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        let host =
            primitives::cylinder("host".into(), axis, self.radius, self.height, FINE).unwrap();
        let side = 50.0 * self.height;
        let frame = Frame3::from_axis([0.0; 3], self.normal(), [0.0, 0.0, 1.0]).unwrap();
        let point = self.point();
        let origin = std::array::from_fn(|i| point[i] - (frame.x[i] + frame.y[i]) * side / 2.0);
        let size = [side, side, side / 2.0];
        let tool =
            primitives::cuboid("tool".into(), Frame3 { origin, ..frame }, size, FINE).unwrap();
        let outcome =
            boolean_brep_outcome_with_handlers(&host, &tool, BooleanOp::Subtraction, "cut".into());
        outcome.0.ok().map(|result| result.brep)
    }

    fn exact(&self) -> Exact {
        let (r, h) = (self.radius, self.height);
        let tilt = self.tilt.to_radians();
        let level = self.point()[1] * tilt.cos() - self.point()[0] * tilt.sin();
        let plane = |x: f64| (level + x * tilt.sin()) / tilt.cos();
        let kept = |x: f64| plane(x).clamp(0.0, h);
        let breaks: Vec<f64> = [0.0, h]
            .iter()
            .map(|y| (y * tilt.cos() - level) / tilt.sin())
            .filter(|x| x.abs() < r)
            .map(|x| (x / r).acos())
            .collect();
        let disk = |g: &dyn Fn(f64) -> f64| {
            pieces(
                &|phi: f64| 2.0 * r * r * phi.sin().powi(2) * g(r * phi.cos()),
                &breaks,
            )
        };
        let inside = |test: &dyn Fn(f64) -> bool| disk(&|x| f64::from(u8::from(test(x))));
        let lateral = 2.0 * pieces(&|phi: f64| r * kept(r * phi.cos()), &breaks);
        let caps = inside(&|x| plane(x) > 0.0) + inside(&|x| plane(x) >= h);
        let face = inside(&|x| plane(x) > 0.0 && plane(x) < h) / tilt.cos();
        Exact {
            volume: disk(&kept),
            area: lateral + caps + face,
        }
    }
}

fn pieces(f: &dyn Fn(f64) -> f64, breaks: &[f64]) -> f64 {
    let mut cuts = vec![0.0, PI];
    cuts.extend(breaks.iter().copied().filter(|x| *x > 0.0 && *x < PI));
    cuts.sort_by(f64::total_cmp);
    cuts.windows(2).map(|w| simpson(f, w[0], w[1])).sum()
}

fn simpson(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> f64 {
    let n = 20_000;
    let step = (b - a) / f64::from(n);
    let weight = |i: u32| match i {
        0 => 1.0,
        _ if i == n => 1.0,
        _ if i % 2 == 1 => 4.0,
        _ => 2.0,
    };
    (0..=n)
        .map(|i| weight(i) * f(a + f64::from(i) * step))
        .sum::<f64>()
        * step
        / 3.0
}

fn check_row(name: &str, body: &BrepEnvelope, deflection: f64, exact: &Exact) -> usize {
    let measured = match measure(body, deflection) {
        Ok(measured) => measured,
        Err(GeometryError::LimitExceeded(_) | GeometryError::UnresolvedTessellation(_)) => {
            return 0
        }
        Err(error) => panic!("{name} at {deflection:e}: {error:?}"),
    };
    assert!(
        measured.area_error_bound.is_finite(),
        "{name} at {deflection:e}: no finite area bound"
    );
    let volume_error = (measured.volume - exact.volume).abs();
    let area_error = (measured.area - exact.area).abs();
    assert!(
        volume_error <= measured.error_bound,
        "{name} at {deflection:e}: volume error {volume_error:e} above {:e}",
        measured.error_bound
    );
    assert!(
        area_error <= measured.area_error_bound,
        "{name} at {deflection:e}: area error {area_error:e} above {:e}",
        measured.area_error_bound
    );
    1
}

fn check_cut(cut: Cut, steps: &[f64]) -> usize {
    let name = format!(
        "cut r {} h {} tilt {} offset {}",
        cut.radius, cut.height, cut.tilt, cut.offset
    );
    let Some(body) = cut.body() else {
        return 0;
    };
    let exact = cut.exact();
    steps
        .iter()
        .map(|step| check_row(&name, &body, step * cut.radius, &exact))
        .sum()
}

fn create(world: &mut WorldGraph, id: &str, primitive: Primitive) {
    world.create_primitive(primitive, named(id)).unwrap();
}

fn extrusions(world: &mut WorldGraph, s: f64) {
    create(world, "circle", Primitive::Circle { radius: 2.0 * s });
    create(
        world,
        "outer",
        Primitive::Rectangle {
            width: 4.0 * s,
            breadth: 4.0 * s,
        },
    );
    create(world, "hole", Primitive::Circle { radius: s });
    for (id, profile, holes, distance) in [
        ("circle-extrusion", "circle", vec![], 3.0 * s),
        ("holed-extrusion", "outer", vec!["hole".into()], 2.0 * s),
    ] {
        let operation = CreatingOperation::Extrude {
            profile: profile.into(),
            holes,
            distance,
        };
        world.create_operation(operation, named(id)).unwrap();
    }
}

fn tube(world: &mut WorldGraph, s: f64) {
    create(
        world,
        "tube",
        Primitive::Cylinder {
            radius: s,
            height: 2.0 * s,
        },
    );
    create(
        world,
        "bore",
        Primitive::Cylinder {
            radius: 0.4 * s,
            height: 4.0 * s,
        },
    );
    let offset = Transform::Translate {
        offset: [0.0, -s, 0.0],
    };
    world.transform("bore", offset).unwrap();
    let bore = ["bore".into()];
    world
        .operate("tube", ModifyingOperation::Subtract, &bore, EditScope::Node)
        .unwrap();
}

fn solids(world: &mut WorldGraph, s: f64) -> Vec<(&'static str, f64, f64)> {
    let (s2, s3) = (s * s, s * s * s);
    let shapes = [
        (
            "cuboid",
            Primitive::Cuboid {
                width: 2.0 * s,
                height: 3.0 * s,
                depth: 4.0 * s,
            },
        ),
        (
            "cylinder",
            Primitive::Cylinder {
                radius: s,
                height: 2.0 * s,
            },
        ),
        ("sphere", Primitive::Sphere { radius: s }),
        (
            "cone",
            Primitive::Cone {
                radius_bottom: s,
                radius_top: 0.0,
                height: 2.0 * s,
            },
        ),
        (
            "frustum",
            Primitive::Cone {
                radius_bottom: s,
                radius_top: 0.4 * s,
                height: 2.0 * s,
            },
        ),
        (
            "torus",
            Primitive::Torus {
                major_radius: 2.0 * s,
                minor_radius: 0.5 * s,
            },
        ),
    ];
    for (id, primitive) in shapes {
        create(world, id, primitive);
    }
    extrusions(world, s);
    tube(world, s);
    vec![
        ("cuboid", 24.0 * s3, 52.0 * s2),
        ("cylinder", 2.0 * PI * s3, 6.0 * PI * s2),
        ("sphere", 4.0 * PI / 3.0 * s3, 4.0 * PI * s2),
        ("cone", 2.0 * PI / 3.0 * s3, PI * (1.0 + 5f64.sqrt()) * s2),
        (
            "frustum",
            2.0 * PI * 1.56 / 3.0 * s3,
            PI * (1.4 * 4.36f64.sqrt() + 1.16) * s2,
        ),
        ("torus", PI * PI * s3, 4.0 * PI * PI * s2),
        ("circle-extrusion", 12.0 * PI * s3, 20.0 * PI * s2),
        (
            "holed-extrusion",
            2.0 * (16.0 - PI) * s3,
            (64.0 + 2.0 * PI) * s2,
        ),
        ("tube", 1.68 * PI * s3, 7.28 * PI * s2),
    ]
}

fn check_solids(steps: &[f64]) -> usize {
    let mut measured = 0;
    for s in [1e-3, 1.0] {
        let mut world = graph();
        for (id, volume, area) in solids(&mut world, s) {
            let body = world.brep(id).unwrap();
            let exact = Exact { volume, area };
            for step in steps {
                measured += check_row(&format!("{id} at size {s}"), &body, step * s, &exact);
            }
        }
    }
    measured
}

fn profile(
    world: &mut WorldGraph,
    id: &str,
    start: [f64; 2],
    path: &[([f64; 2], Option<[f64; 2]>)],
) {
    let segments = path
        .iter()
        .map(|&(to, center)| ProfileSegment {
            to,
            arc: center.map(|center| SegmentArc {
                center,
                counter_clockwise: true,
            }),
        })
        .collect();
    let primitive = Primitive::Profile {
        start,
        segments,
        closed: true,
    };
    create(world, id, primitive);
    let operation = CreatingOperation::Extrude {
        profile: id.into(),
        holes: vec![],
        distance: 0.7,
    };
    world
        .create_operation(operation, named(&format!("{id}-solid")))
        .unwrap();
}

fn rounded_profiles(world: &mut WorldGraph) -> Vec<(&'static str, Exact)> {
    let slot = [
        ([1.0, -0.5], None),
        ([1.0, 0.5], Some([1.0, 0.0])),
        ([-1.0, 0.5], None),
        ([-1.0, -0.5], Some([-1.0, 0.0])),
    ];
    profile(world, "slot", [-1.0, -0.5], &slot);
    let rounded = [
        ([0.8, -0.5], None),
        ([1.0, -0.3], Some([0.8, -0.3])),
        ([1.0, 0.3], None),
        ([0.8, 0.5], Some([0.8, 0.3])),
        ([-0.8, 0.5], None),
        ([-1.0, 0.3], Some([-0.8, 0.3])),
        ([-1.0, -0.3], None),
        ([-0.8, -0.5], Some([-0.8, -0.3])),
    ];
    profile(world, "rounded", [-0.8, -0.5], &rounded);
    let corners = 2.0 - (4.0 - PI) * 0.04;
    vec![
        (
            "slot-solid",
            Exact {
                volume: 0.7 * (2.0 + PI / 4.0),
                area: 2.0 * (2.0 + PI / 4.0) + 0.7 * (4.0 + PI),
            },
        ),
        (
            "rounded-solid",
            Exact {
                volume: 0.7 * corners,
                area: 2.0 * corners + 0.7 * (4.4 + 0.4 * PI),
            },
        ),
    ]
}

#[test]
fn measured_area_of_a_steep_cut_through_a_millimetre_cylinder_lies_within_its_area_bound() {
    let cut = Cut {
        radius: 1e-3,
        height: 40e-3,
        tilt: 15.0,
        offset: 0.0,
    };
    let body = cut.body().unwrap();
    let measured = measure(&body, 1e-6).unwrap();
    let exact = cut.exact();
    assert!(
        (measured.area - exact.area).abs() <= measured.area_error_bound,
        "area {:e} versus {:e} with bound {:e}",
        measured.area,
        exact.area,
        measured.area_error_bound
    );
}

#[test]
fn measured_volumes_and_areas_of_solids_at_two_sizes_lie_within_their_bounds() {
    assert_eq!(check_solids(&STEPS[..2]), 36);
}

#[test]
fn slot_and_rounded_rectangle_extrusions_get_a_finite_area_bound_that_holds() {
    let mut world = graph();
    let mut measured = 0;
    for (id, exact) in rounded_profiles(&mut world) {
        let body = world.brep(id).unwrap();
        for deflection in [1e-2, 1e-3] {
            measured += check_row(id, &body, deflection, &exact);
        }
    }
    assert_eq!(measured, 4);
}

#[test]
#[ignore = "release corpus: cargo test --offline --release --test world_graph measure_bounds -- --ignored"]
fn measured_volumes_and_areas_of_the_cut_cylinder_corpus_lie_within_their_bounds() {
    let mut measured = check_solids(&STEPS);
    for radius in [1e-3, 1.0] {
        for height in [2.0, 4.0, 40.0] {
            for tilt in TILTS {
                for offset in [0.0, 2.5e-4] {
                    let cut = Cut {
                        radius,
                        height: height * radius,
                        tilt,
                        offset: offset * radius,
                    };
                    measured += check_cut(cut, &STEPS);
                }
            }
        }
    }
    assert!(measured >= 201, "{measured} rows measured");
}
