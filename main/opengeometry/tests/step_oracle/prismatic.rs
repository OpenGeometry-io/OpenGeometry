use crate::support::export_matched;
use opengeometry::world_graph::{EditScope, ModifyingOperation, StepOptions};
use opengeometry_test_support::scenes::prismatic::pan_and_inset;

#[test]
fn a_pan_cut_from_an_arc_edged_extrusion_exports_one_solid_with_both_arcs() {
    let mut world = pan_and_inset(0.4);
    world
        .operate(
            "pan",
            ModifyingOperation::Subtract,
            &["inset".to_string()],
            EditScope::Node,
        )
        .unwrap();
    let exported = export_matched(&world, &["pan"], &StepOptions::default());
    assert_eq!(exported.document.count("MANIFOLD_SOLID_BREP"), 1);
    assert_eq!(exported.document.count("CYLINDRICAL_SURFACE"), 2);
}
