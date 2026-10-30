use crate::support::{add_cuboid, export_matched};
use opengeometry::world_graph::{EditScope, ModifyingOperation, Primitive, StepOptions, Transform};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn a_cylinder_cut_by_a_tilted_cuboid_exports_one_solid_with_the_cut_ellipse() {
    let mut world = graph();
    world
        .create_primitive(
            Primitive::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
            named("cylinder"),
        )
        .unwrap();
    add_cuboid(&mut world, "cuboid", [10.0; 3], [0.0, 1.0, 0.0]);
    world
        .transform(
            "cuboid",
            Transform::Rotate {
                axis: [0.0, 0.0, 1.0],
                degrees: 45.0,
                pivot: Some([0.0, 1.0, 0.0]),
            },
        )
        .unwrap();
    world
        .operate(
            "cylinder",
            ModifyingOperation::Subtract,
            &["cuboid".to_string()],
            EditScope::Node,
        )
        .unwrap();
    let exported = export_matched(&world, &["cylinder"], &StepOptions::default());
    assert_eq!(exported.document.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(exported.document.count("ADVANCED_FACE"), 3);
    assert_eq!(exported.document.count("CYLINDRICAL_SURFACE"), 1);
    assert_eq!(exported.document.count("ELLIPSE"), 1);
    assert_eq!(exported.report["pcurvelessEdges"], 1);
}
