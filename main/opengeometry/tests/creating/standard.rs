use opengeometry::world_graph::{
    CreateOptions, ErrorCode, Plane, Primitive, StepOptions, WorldGraph,
};
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::scenes::bar::bar_handlers;
use opengeometry_test_support::world_graph::{graph, named};

fn created(primitive: Primitive, options: CreateOptions) -> WorldGraph {
    let mut world = graph();
    world.create_primitive(primitive, options).unwrap();
    world
}

fn assert_closed_with_volume(world: &WorldGraph, expected: f64) {
    let validity = world.validity("solid").unwrap();
    assert!(validity.valid && validity.closed);
    let measured = world.measure("solid", Some(1e-4)).unwrap();
    assert!(
        (measured.volume - expected).abs() <= measured.error_bound,
        "{} versus {expected} with bound {}",
        measured.volume,
        measured.error_bound
    );
}

pub(super) fn assert_exact_volume(primitive: Primitive, expected: f64) {
    assert_closed_with_volume(&created(primitive, named("solid")), expected);
}

pub(super) fn assert_refused(primitive: Primitive, message: &str) {
    let mut world = graph();
    let label = format!("{primitive:?}");
    let error = world
        .create_primitive(primitive, named("solid"))
        .expect_err(&label);
    assert_eq!(error.error_code(), ErrorCode::InvalidParameter, "{label}");
    assert_eq!(error.message(), message, "{label}");
    assert_eq!(world.node_count(), 0);
}

pub(super) fn assert_on_plane(primitive: Primitive, expected: f64, corners: &[[f64; 3]]) {
    let options = CreateOptions {
        plane: Some(Plane {
            origin: Some([1.0, 2.0, 3.0]),
            normal: Some([1.0, 0.0, 0.0]),
            x_direction: Some([0.0, 0.0, 1.0]),
        }),
        ..named("solid")
    };
    let world = created(primitive, options);
    assert_closed_with_volume(&world, expected);
    let placement = world.world_placement("solid").unwrap();
    let brep = world.brep("solid").unwrap();
    let points = brep
        .topology
        .vertices
        .iter()
        .map(|vertex| placement.apply_point(vertex.position))
        .collect::<Vec<_>>();
    let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|axis| (a[axis] - b[axis]).abs() <= 1e-9);
    for corner in corners {
        assert!(
            points.iter().any(|point| near(*point, *corner)),
            "{corner:?} in {points:?}"
        );
    }
}

pub(super) fn assert_bar(primitive: Primitive, rows: [&str; 6]) {
    assert_eq!(bar_handlers(&primitive), rows);
}

pub(super) fn assert_step_surfaces(primitive: Primitive, cylinders: usize, planes: usize) {
    let world = created(primitive, named("solid"));
    let (text, _) = world
        .export_step(&["solid".into()], &StepOptions::default())
        .unwrap();
    let document = Document::parse(&text).unwrap();
    assert_eq!(document.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(document.pcurveless_edges().unwrap(), 0);
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), cylinders);
    assert_eq!(document.count("PLANE"), planes);
}
