use crate::support::export_matched;
use opengeometry::world_graph::{EditScope, Primitive, StepOptions};
use opengeometry_test_support::world_graph::{graph, named};

#[test]
fn a_shelled_cylinder_exports_one_brep_with_voids() {
    let mut world = graph();
    let tank = Primitive::Cylinder {
        radius: 0.5,
        height: 1.0,
    };
    world.create_primitive(tank, named("tank")).unwrap();
    world.shell("tank", 0.05, EditScope::Node).unwrap();
    let exported = export_matched(&world, &["tank"], &StepOptions::default());
    assert_eq!(exported.document.count("BREP_WITH_VOIDS"), 1);
    assert_eq!(exported.document.count("CYLINDRICAL_SURFACE"), 2);
    assert_eq!(exported.report["solids"], 1);
}
