use crate::support::export_matched;
use opengeometry::world_graph::{Primitive, StepOptions};
use opengeometry_test_support::world_graph::{graph, named};
use serde_json::json;

#[test]
fn sphere_cone_and_torus_primitives_export_one_analytic_surface_each() {
    for (value, surface) in [
        (
            json!({"kind": "Sphere", "radius": 0.5}),
            "SPHERICAL_SURFACE",
        ),
        (
            json!({"kind": "Cone", "radiusBottom": 1.0, "radiusTop": 0.0, "height": 2.0}),
            "CONICAL_SURFACE",
        ),
        (
            json!({"kind": "Cone", "radiusBottom": 0.0, "radiusTop": 0.6, "height": 1.0}),
            "CONICAL_SURFACE",
        ),
        (
            json!({"kind": "Cone", "radiusBottom": 1.0, "radiusTop": 0.5, "height": 2.0}),
            "CONICAL_SURFACE",
        ),
        (
            json!({"kind": "Cone", "radiusBottom": 0.5, "radiusTop": 0.5, "height": 1.0}),
            "CYLINDRICAL_SURFACE",
        ),
        (
            json!({"kind": "Torus", "majorRadius": 1.0, "minorRadius": 0.25}),
            "TOROIDAL_SURFACE",
        ),
    ] {
        let label = value.to_string();
        let primitive: Primitive = serde_json::from_value(value).expect(&label);
        let mut world = graph();
        world.create_primitive(primitive, named("solid")).unwrap();
        let exported = export_matched(&world, &["solid"], &StepOptions::default());
        assert_eq!(exported.document.count(surface), 1, "{label}");
        assert_eq!(exported.document.count("MANIFOLD_SOLID_BREP"), 1, "{label}");
    }
}
