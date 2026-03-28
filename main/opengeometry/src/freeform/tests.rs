use openmaths::Vector3;

use super::OGFreeformGeometry;
use crate::primitives::arc::OGArc;
use crate::primitives::cuboid::OGCuboid;
use crate::primitives::curve::OGCurve;
use crate::primitives::cylinder::OGCylinder;
use crate::primitives::line::OGLine;
use crate::primitives::polygon::OGPolygon;
use crate::primitives::polyline::OGPolyline;
use crate::primitives::rectangle::OGRectangle;
use crate::primitives::sphere::OGSphere;
use crate::primitives::sweep::OGSweep;
use crate::primitives::wedge::OGWedge;

#[test]
fn freeform_entity_can_be_created_from_all_native_local_breps() {
    let mut line = OGLine::new("line-source".to_string());
    line.set_config(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0))
        .expect("line config");

    let mut polyline = OGPolyline::new("polyline-source".to_string());
    polyline
        .set_config(vec![
            Vector3::new(-1.0, 0.0, -1.0),
            Vector3::new(0.0, 0.0, 0.5),
            Vector3::new(1.0, 0.0, 0.0),
        ])
        .expect("polyline config");

    let mut arc = OGArc::new("arc-source".to_string());
    arc.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        2.0,
        0.0,
        std::f64::consts::PI,
        24,
    )
    .expect("arc config");

    let mut curve = OGCurve::new("curve-source".to_string());
    curve
        .set_config(vec![
            Vector3::new(-1.0, 0.0, 0.0),
            Vector3::new(-0.2, 0.6, 0.2),
            Vector3::new(0.5, 0.3, 0.8),
            Vector3::new(1.0, 0.0, 0.0),
        ])
        .expect("curve config");

    let mut rectangle = OGRectangle::new("rectangle-source".to_string());
    rectangle
        .set_config(Vector3::new(0.0, 0.0, 0.0), 3.0, 2.0)
        .expect("rectangle config");

    let mut polygon = OGPolygon::new("polygon-source".to_string());
    polygon
        .set_config(vec![
            Vector3::new(-1.0, 0.0, -1.0),
            Vector3::new(1.0, 0.0, -1.0),
            Vector3::new(1.5, 0.0, 0.5),
            Vector3::new(-1.0, 0.0, 1.0),
        ])
        .expect("polygon config");

    let mut cuboid = OGCuboid::new("cuboid-source".to_string());
    cuboid
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 3.0, 4.0)
        .expect("cuboid config");

    let mut cylinder = OGCylinder::new("cylinder-source".to_string());
    cylinder
        .set_config(
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            2.0,
            std::f64::consts::TAU,
            24,
        )
        .expect("cylinder config");

    let mut sphere = OGSphere::new("sphere-source".to_string());
    sphere
        .set_config(Vector3::new(0.0, 0.0, 0.0), 1.25, 16, 12)
        .expect("sphere config");

    let mut wedge = OGWedge::new("wedge-source".to_string());
    wedge
        .set_config(Vector3::new(0.0, 0.0, 0.0), 2.0, 1.5, 1.0)
        .expect("wedge config");

    let mut sweep = OGSweep::new("sweep-source".to_string());
    sweep
        .set_config(
            vec![
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.8, 0.3),
                Vector3::new(1.0, 1.2, 0.0),
            ],
            vec![
                Vector3::new(-0.2, 0.0, -0.2),
                Vector3::new(0.2, 0.0, -0.2),
                Vector3::new(0.2, 0.0, 0.2),
                Vector3::new(-0.2, 0.0, 0.2),
            ],
        )
        .expect("sweep config");

    let sources = vec![
        ("line", line.get_local_brep_serialized()),
        ("polyline", polyline.get_local_brep_serialized()),
        ("arc", arc.get_local_brep_serialized()),
        ("curve", curve.get_local_brep_serialized()),
        ("rectangle", rectangle.get_local_brep_serialized()),
        ("polygon", polygon.get_local_brep_serialized()),
        ("cuboid", cuboid.get_local_brep_serialized()),
        ("cylinder", cylinder.get_local_brep_serialized()),
        ("sphere", sphere.get_local_brep_serialized()),
        ("wedge", wedge.get_local_brep_serialized()),
        ("sweep", sweep.get_local_brep_serialized()),
    ];

    for (label, local_brep) in sources {
        let entity = OGFreeformGeometry::new(format!("freeform-{}", label), local_brep)
            .unwrap_or_else(|_| panic!("{} should convert to freeform", label));

        assert!(!entity.get_local_brep_serialized().is_empty());
    }
}
