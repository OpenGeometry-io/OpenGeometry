use super::support::assert_volume;
use opengeometry::primitives::{ProfileSegment, SegmentArc};
use opengeometry::query::PointClassification;
use opengeometry::world_graph::{
    CreateOptions, CreatingOperation, EditScope, ErrorCode, GraphError, ModifyingOperation, Plane,
    Primitive, WorldGraph,
};
use opengeometry_test_support::volume;
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

const THICKNESS: f64 = 0.3;
const WALL_HEIGHT: f64 = 3.0;
const MISSES_SUPPORT_12_0: &str =
    r#"UnresolvedIntersection("face pair 12:0: intersection curve misses support")"#;
const MISSES_SUPPORT_6_2: &str =
    r#"UnresolvedIntersection("face pair 6:2: intersection curve misses support")"#;
const TANGENT_GUIDE: &str =
    r#"UnresolvedIntersection("trace guide is tangent to correction plane")"#;
const ANALYTIC_GAP: &str =
    r#"CoverageGap { families: ["generic analytic solid", "intersecting analytic solid"] }"#;

#[derive(Clone, Copy)]
struct Opening {
    radius: f64,
    width: f64,
    bottom: f64,
    spring: f64,
}

#[derive(Clone, Copy)]
enum Order {
    OneOperate,
    RectangleFirst,
    CylinderFirst,
}

impl Opening {
    fn window(radius: f64, width: f64) -> Self {
        Self::arch(radius, width, 0.4 + 1.4 - width / 2.0)
    }

    fn arch(radius: f64, width: f64, spring: f64) -> Self {
        Self {
            radius,
            width,
            bottom: 0.4,
            spring,
        }
    }

    fn half(self) -> f64 {
        self.width / 2.0
    }

    fn near(self) -> f64 {
        self.radius - THICKNESS
    }

    fn far(self) -> f64 {
        self.radius + THICKNESS
    }
}

fn radial() -> [f64; 3] {
    [FRAC_PI_4.cos(), 0.0, -FRAC_PI_4.sin()]
}

fn line(to: [f64; 2]) -> ProfileSegment {
    ProfileSegment { to, arc: None }
}

fn arc(to: [f64; 2], center: [f64; 2], counter_clockwise: bool) -> ProfileSegment {
    let arc = SegmentArc {
        center,
        counter_clockwise,
    };
    ProfileSegment {
        arc: Some(arc),
        ..line(to)
    }
}

fn placed(id: &str, [origin, normal, x_direction]: [[f64; 3]; 3]) -> CreateOptions {
    let [origin, normal, x_direction] = [origin, normal, x_direction].map(Some);
    let plane = Plane {
        origin,
        normal,
        x_direction,
    };
    CreateOptions {
        plane: Some(plane),
        ..named(id)
    }
}

fn floor_plane(id: &str, height: f64) -> CreateOptions {
    placed(id, [[0.0, height, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]])
}

fn add_prism(
    world: &mut WorldGraph,
    id: &str,
    profile: (CreateOptions, [f64; 2], Vec<ProfileSegment>),
    distance: f64,
) {
    let (options, start, segments) = profile;
    let profile_id = options.og_id.clone().unwrap();
    let primitive = Primitive::Profile {
        start,
        segments,
        closed: true,
    };
    world.create_primitive(primitive, options).unwrap();
    let extrude = CreatingOperation::Extrude {
        profile: profile_id,
        holes: Vec::new(),
        distance,
    };
    world.create_operation(extrude, named(id)).unwrap();
}

fn curved_wall(radius: f64) -> WorldGraph {
    let point = |r: f64, angle: f64| [r * angle.cos(), r * angle.sin()];
    let (outer, inner) = (radius + THICKNESS / 2.0, radius - THICKNESS / 2.0);
    let segments = vec![
        arc(point(outer, FRAC_PI_2), [0.0, 0.0], true),
        line(point(inner, FRAC_PI_2)),
        arc(point(inner, 0.0), [0.0, 0.0], false),
        line(point(outer, 0.0)),
    ];
    let mut world = graph();
    let profile = (
        floor_plane("wall-profile", 0.0),
        point(outer, 0.0),
        segments,
    );
    add_prism(&mut world, "wall", profile, WALL_HEIGHT);
    world
}

fn add_cylinder(world: &mut WorldGraph, opening: Opening) {
    let [dx, _, dz] = radial();
    let near = opening.near();
    let origin = [dx * near, opening.spring, dz * near];
    let options = placed("cylinder", [origin, radial(), [dz, 0.0, -dx]]);
    let primitive = Primitive::Cylinder {
        radius: opening.half(),
        height: opening.far() - near,
    };
    world.create_primitive(primitive, options).unwrap();
}

fn add_rectangle(world: &mut WorldGraph, opening: Opening) {
    let [dx, _, dz] = radial();
    let (nx, nz) = (-dz, dx);
    let point = |along: f64, side: f64| [dx * along + nx * side, -(dz * along + nz * side)];
    let (near, far, half) = (opening.near(), opening.far(), opening.half());
    let segments = vec![
        line(point(far, -half)),
        line(point(far, half)),
        line(point(near, half)),
        line(point(near, -half)),
    ];
    let profile = (
        floor_plane("rectangle-profile", opening.bottom),
        point(near, -half),
        segments,
    );
    add_prism(world, "rectangle", profile, opening.spring - opening.bottom);
}

fn add_arch(world: &mut WorldGraph, opening: Opening) {
    let direction = radial();
    let (half, bottom, spring) = (opening.half(), opening.bottom, opening.spring);
    let segments = vec![
        line([half, bottom]),
        line([half, spring]),
        arc([-half, spring], [0.0, spring], true),
        line([-half, bottom]),
    ];
    let origin = direction.map(|value| value * opening.near());
    let x_direction = [direction[2], 0.0, -direction[0]];
    let options = placed("arch-profile", [origin, direction, x_direction]);
    let profile = (options, [-half, bottom], segments);
    add_prism(world, "arch", profile, opening.far() - opening.near());
}

fn subtract(world: &mut WorldGraph, tools: &[&str]) -> Result<(), GraphError> {
    let tools: Vec<String> = tools.iter().map(|tool| tool.to_string()).collect();
    world
        .operate(
            "wall",
            ModifyingOperation::Subtract,
            &tools,
            EditScope::Node,
        )
        .map(|_| ())
}

fn assert_refused(world: &mut WorldGraph, tools: &[&str], code: ErrorCode, message: &str) {
    let before = world.brep("wall").unwrap().to_json().unwrap();
    let revision = world.revision();
    let error = subtract(world, tools).unwrap_err();
    assert_eq!(error.error_code(), code, "{error:?}");
    assert_eq!(error.message(), message);
    assert_eq!(world.revision(), revision);
    assert_eq!(world.brep("wall").unwrap().to_json().unwrap(), before);
}

fn assert_rectangle_and_cylinder_refused(opening: Opening, order: Order, message: &str) {
    let mut world = curved_wall(opening.radius);
    add_rectangle(&mut world, opening);
    add_cylinder(&mut world, opening);
    let code = ErrorCode::UnresolvedIntersection;
    match order {
        Order::OneOperate => assert_refused(&mut world, &["rectangle", "cylinder"], code, message),
        Order::RectangleFirst => {
            subtract(&mut world, &["rectangle"]).unwrap();
            assert_refused(&mut world, &["cylinder"], code, message);
        }
        Order::CylinderFirst => {
            subtract(&mut world, &["cylinder"]).unwrap();
            assert_refused(&mut world, &["rectangle"], code, message);
        }
    }
}

fn assert_arch_refused(opening: Opening, code: ErrorCode, message: &str) {
    let mut world = curved_wall(opening.radius);
    add_arch(&mut world, opening);
    assert_refused(&mut world, &["arch"], code, message);
}

fn removed_volume(opening: Opening) -> f64 {
    let (outer, inner) = (
        opening.radius + THICKNESS / 2.0,
        opening.radius - THICKNESS / 2.0,
    );
    let (half, jamb) = (opening.half(), opening.spring - opening.bottom);
    let integrand = |angle: f64| {
        let (side, rise) = (half * angle.sin(), half * angle.cos());
        let depth = (outer * outer - side * side).sqrt() - (inner * inner - side * side).sqrt();
        depth * (jamb + rise) * rise
    };
    let steps = 2000;
    let step = PI / steps as f64;
    let sum: f64 = (0..=steps)
        .map(|index| {
            let weight = match index {
                0 => 1.0,
                index if index == steps => 1.0,
                index if index % 2 == 1 => 4.0,
                _ => 2.0,
            };
            weight * integrand(-FRAC_PI_2 + step * index as f64)
        })
        .sum();
    sum * step / 3.0
}

fn assert_arch_cut(opening: Opening) {
    let mut world = curved_wall(opening.radius);
    add_arch(&mut world, opening);
    subtract(&mut world, &["arch"]).unwrap();
    assert_eq!(
        world.report("wall").unwrap().unwrap()["handlers"],
        serde_json::json!(["generic_two_sided_cutter_band_subtraction"])
    );
    let body = world.brep("wall").unwrap();
    body.validate().unwrap();
    assert_eq!(body.topology.faces.len(), 10);
    let [dx, _, dz] = radial();
    let inside_opening = [dx * opening.radius, opening.spring, dz * opening.radius];
    assert_eq!(
        world.classify_point("wall", inside_opening).unwrap(),
        PointClassification::Outside
    );
    let wall = PI * opening.radius * THICKNESS * WALL_HEIGHT / 2.0;
    let expected = wall - removed_volume(opening);
    assert_volume(volume::estimate(&body, 1e-4), expected, "arch cut");
}

#[test]
fn radius_2_width_0_6_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(2.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_0_8_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(2.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_1_0_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(2.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_0_6_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(3.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_0_8_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(3.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_1_0_rectangle_and_cylinder_in_one_operate_are_refused() {
    let opening = Opening::window(3.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::OneOperate, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_0_6_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(2.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_0_8_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(2.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_1_0_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(2.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_0_6_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(3.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_0_8_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(3.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_3_width_1_0_cylinder_after_the_rectangle_is_refused() {
    let opening = Opening::window(3.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::RectangleFirst, MISSES_SUPPORT_12_0);
}

#[test]
fn radius_2_width_0_6_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(2.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_2_width_0_8_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(2.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_2_width_1_0_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(2.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_3_width_0_6_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(3.0, 0.6);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_3_width_0_8_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(3.0, 0.8);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_3_width_1_0_rectangle_after_the_cylinder_is_refused() {
    let opening = Opening::window(3.0, 1.0);
    assert_rectangle_and_cylinder_refused(opening, Order::CylinderFirst, MISSES_SUPPORT_6_2);
}

#[test]
fn radius_2_width_0_6_arch_prism_cuts_the_wall() {
    assert_arch_cut(Opening::window(2.0, 0.6));
}

#[test]
fn radius_2_width_0_8_arch_prism_cuts_the_wall() {
    assert_arch_cut(Opening::window(2.0, 0.8));
}

#[test]
fn radius_2_width_1_0_arch_prism_is_refused_with_a_coverage_gap() {
    let opening = Opening::window(2.0, 1.0);
    assert_arch_refused(opening, ErrorCode::CoverageGap, ANALYTIC_GAP);
}

#[test]
fn radius_3_width_0_6_arch_prism_cuts_the_wall() {
    assert_arch_cut(Opening::window(3.0, 0.6));
}

#[test]
fn radius_3_width_0_8_arch_prism_cuts_the_wall() {
    assert_arch_cut(Opening::window(3.0, 0.8));
}

#[test]
fn radius_3_width_1_0_arch_prism_is_refused_with_a_coverage_gap() {
    let opening = Opening::window(3.0, 1.0);
    assert_arch_refused(opening, ErrorCode::CoverageGap, ANALYTIC_GAP);
}

#[test]
fn radius_3_width_1_0_door_sized_arch_prism_cuts_the_wall() {
    assert_arch_cut(Opening {
        radius: 3.0,
        width: 1.0,
        bottom: 0.5,
        spring: 0.5 + 1.25 - 0.5,
    });
}

#[test]
fn radius_2_width_0_6_arch_prism_with_its_spring_exactly_at_1_5_is_refused() {
    let opening = Opening::arch(2.0, 0.6, 1.5);
    assert_arch_refused(opening, ErrorCode::UnresolvedIntersection, TANGENT_GUIDE);
}

#[test]
fn radius_2_width_0_8_arch_prism_with_its_spring_at_1_875_cuts_the_wall() {
    assert_arch_cut(Opening::arch(2.0, 0.8, 1.875));
}

#[test]
fn radius_2_width_0_5_arch_prism_with_its_spring_at_2_25_cuts_a_measurable_wall() {
    assert_arch_cut(Opening::arch(2.0, 0.5, 2.25));
}
