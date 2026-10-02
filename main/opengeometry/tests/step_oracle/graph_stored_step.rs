use crate::support::{add_cuboid, cylinder, export_matched, fixture_text};
use opengeometry::world_graph::{EditScope, ModifyingOperation, StepOptions};
use opengeometry_test_support::part21::normalise_step;
use opengeometry_test_support::world_graph::graph;

#[test]
fn graph_export_of_fixture_primitives_matches_stored_text_after_normalisation() {
    let mut world = graph();
    cylinder(&mut world, "cylinder");
    add_cuboid(&mut world, "cuboid", [2.0, 3.0, 1.0], [1.0, 0.0, -0.5]);
    add_cuboid(&mut world, "box-cut", [3.0; 3], [1.5, 0.0, -1.5]);
    add_cuboid(&mut world, "box-cutter", [3.0; 3], [3.0, 0.5, -2.0]);
    world
        .operate(
            "box-cut",
            ModifyingOperation::Subtract,
            &["box-cutter".into()],
            EditScope::Node,
        )
        .unwrap();
    for (unit, suffix) in [("metre", "m"), ("millimetre", "mm")] {
        let options = StepOptions {
            unit: unit.into(),
            up_axis: "Y".into(),
            ..StepOptions::default()
        };
        for name in ["cylinder", "cuboid", "box-cut"] {
            let exported = export_matched(&world, &[name], &options);
            let stored = fixture_text(&format!("{name}.step.{suffix}")).unwrap();
            assert_eq!(
                normalise_step(&exported.text),
                normalise_step(&stored),
                "{name} {unit}"
            );
        }
    }
}
