use super::standard::assert_refused;
use super::support::extrude;
use opengeometry::brep::BodyType;
use opengeometry::world_graph::{CreateOptions, Plane, Primitive, StepOptions, WorldGraph};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::world_graph::{graph, named};
use std::f64::consts::TAU;

struct Faces {
    total: usize,
    planes: usize,
    cylinders: usize,
    volume: f64,
    bounds: [f64; 6],
}

fn polygon(sides: u32, radius: f64) -> Primitive {
    Primitive::RegularPolygon { sides, radius }
}

fn slot(length: f64, width: f64) -> Primitive {
    Primitive::Slot { length, width }
}

fn rounded(width: f64, breadth: f64, radius: f64) -> Primitive {
    Primitive::RoundedRectangle {
        width,
        breadth,
        radius,
    }
}

fn assert_closed_wire(primitive: Primitive, edges: usize) {
    let mut world = graph();
    world.create_primitive(primitive, named("wire")).unwrap();
    let validity = world.validity("wire").unwrap();
    assert!(validity.valid && validity.closed);
    assert_eq!(validity.body_type, Some(BodyType::Wire));
    assert_eq!(world.brep("wire").unwrap().topology.edges.len(), edges);
}

fn faces(world: &WorldGraph, og_id: &str) -> Faces {
    let (text, _) = world
        .export_step(&[og_id.into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    let measured = world.measure(og_id, Some(1e-4)).unwrap();
    assert!(world.validity(og_id).unwrap().closed);
    Faces {
        total: world.brep(og_id).unwrap().topology.faces.len(),
        planes: document.count("PLANE"),
        cylinders: document.count("CYLINDRICAL_SURFACE"),
        volume: measured.volume,
        bounds: world.bounds(og_id).unwrap().unwrap(),
    }
}

fn extruded(wire: Primitive) -> (WorldGraph, Faces) {
    let mut world = graph();
    world.create_primitive(wire, named("profile")).unwrap();
    world
        .create_operation(extrude("profile", 1.0), named("solid"))
        .unwrap();
    let found = faces(&world, "solid");
    (world, found)
}

fn assert_extrudes_like(wire: Primitive, solid: Primitive) {
    let (_, from_wire) = extruded(wire);
    let mut world = graph();
    world.create_primitive(solid, named("kind")).unwrap();
    let from_kind = faces(&world, "kind");
    let bound = world.measure("kind", Some(1e-4)).unwrap().error_bound;
    assert_eq!(
        [from_wire.total, from_wire.planes, from_wire.cylinders],
        [from_kind.total, from_kind.planes, from_kind.cylinders]
    );
    assert!((from_wire.volume - from_kind.volume).abs() <= 2.0 * bound);
    for axis in 0..6 {
        assert!((from_wire.bounds[axis] - from_kind.bounds[axis]).abs() <= 1e-9);
    }
}

fn assert_placed_corner(primitive: Primitive, corner: [f64; 3]) {
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([1.0, 2.0, 3.0]),
            normal: Some([1.0, 0.0, 0.0]),
            x_direction: Some([0.0, 0.0, 1.0]),
        }),
        ..named("wire")
    };
    let mut world = graph();
    world.create_primitive(primitive, options).unwrap();
    let placement = world.world_placement("wire").unwrap();
    let found = world
        .brep("wire")
        .unwrap()
        .topology
        .vertices
        .iter()
        .any(|vertex| {
            let point = placement.apply_point(vertex.position);
            (0..3).all(|axis| (point[axis] - corner[axis]).abs() <= 1e-9)
        });
    assert!(found, "{corner:?}");
}

#[test]
fn wire_kinds_are_closed_wires_with_their_edge_counts() {
    assert_closed_wire(polygon(6, 0.5), 6);
    assert_closed_wire(polygon(3, 2.0), 3);
    assert_closed_wire(slot(1.0, 0.5), 4);
    assert_closed_wire(rounded(1.2, 0.8, 0.1), 8);
}

#[test]
fn invalid_wire_kinds_are_invalid_parameter_naming_the_value() {
    let size = |name: &str| format!("{name} must exceed four times geometric tolerance");
    let sides = "regular polygon sides must be between 3 and 1024".to_string();
    let corner = "rounded rectangle radius must be below half its width and breadth".to_string();
    for (primitive, message) in [
        (polygon(2, 1.0), sides.clone()),
        (polygon(1025, 1.0), sides),
        (polygon(6, f64::NAN), size("radius")),
        (polygon(1024, 1e-7), size("regular polygon side")),
        (slot(0.0, 0.5), size("length")),
        (slot(1.0, 6e-8), size("width")),
        (rounded(1.0, -1.0, 0.1), size("breadth")),
        (rounded(1.0, 1.0, 0.0), size("radius")),
        (rounded(1.0, 0.8, 0.4), corner),
    ] {
        assert_refused(primitive, &message);
    }
}

#[test]
fn wire_kind_json_parses_its_fields() {
    for json in [
        r#"{"kind":"RegularPolygon","sides":6,"radius":0.5}"#,
        r#"{"kind":"Slot","length":1,"width":0.5}"#,
        r#"{"kind":"RoundedRectangle","width":1,"breadth":0.8,"radius":0.1}"#,
    ] {
        assert!(serde_json::from_str::<Primitive>(json).is_ok(), "{json}");
    }
    let fractional = r#"{"kind":"RegularPolygon","sides":5.5,"radius":0.5}"#;
    assert!(serde_json::from_str::<Primitive>(fractional).is_err());
}

#[test]
fn wire_kinds_on_a_plane_keep_u_along_the_x_direction_and_turn_counter_clockwise() {
    assert_placed_corner(polygon(4, 0.5), [1.0, 2.0, 3.5]);
    assert_placed_corner(polygon(4, 0.5), [1.0, 1.5, 3.0]);
    assert_placed_corner(slot(1.0, 0.5), [1.0, 2.25, 3.5]);
    assert_placed_corner(rounded(1.2, 0.8, 0.1), [1.0, 2.4, 3.5]);
}

#[test]
fn an_extruded_slot_has_the_faces_volume_and_bounds_of_the_slot_prism() {
    let prism = Primitive::SlotPrism {
        length: 1.0,
        width: 0.5,
        height: 1.0,
    };
    assert_extrudes_like(slot(1.0, 0.5), prism);
}

#[test]
fn an_extruded_rounded_rectangle_has_the_faces_volume_and_bounds_of_the_rounded_box() {
    let block = Primitive::RoundedBox {
        width: 1.2,
        height: 1.0,
        depth: 0.8,
        radius: 0.1,
    };
    assert_extrudes_like(rounded(1.2, 0.8, 0.1), block);
}

#[test]
fn an_extruded_regular_polygon_is_a_prism_with_the_exact_volume() {
    for sides in [3_u32, 6, 7] {
        let (_, prism) = extruded(polygon(sides, 0.5));
        let n = f64::from(sides);
        let exact = n / 2.0 * 0.25 * (TAU / n).sin();
        let count = sides as usize + 2;
        assert_eq!(
            [prism.total, prism.planes, prism.cylinders],
            [count, count, 0]
        );
        assert!(
            (prism.volume - exact).abs() <= 1e-9,
            "{sides}: {}",
            prism.volume
        );
        assert!((prism.bounds[3] - 0.5).abs() <= 1e-12, "{sides}");
    }
}
