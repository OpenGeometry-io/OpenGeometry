use super::wire::WireUse;
use super::Builder;
use crate::brep::accuracy::Accuracy;
use crate::brep::body_type::BodyType;
use crate::brep::geometry::CurveGeometry;
use crate::brep::topology::Orientation;
use crate::math::Interval;

#[test]
fn multi_edge_open_wire_validates() {
    let accuracy = Accuracy {
        geometric: 1e-6,
        intersection: 1e-7,
        tessellation: 1e-3,
        exchange: 1e-6,
    };
    let mut builder = Builder::new("wire".into(), accuracy).unwrap();
    let a = builder.vertex([0.0, 0.0, 0.0]);
    let b = builder.vertex([1.0, 0.0, 0.0]);
    let c = builder.vertex([1.0, 1.0, 0.0]);
    let first = builder.edge(
        CurveGeometry::Line {
            origin: [0.0, 0.0, 0.0],
            direction: [1.0, 0.0, 0.0],
        },
        Interval::new(0.0, 1.0).unwrap(),
        false,
    );
    let second = builder.edge(
        CurveGeometry::Line {
            origin: [1.0, 0.0, 0.0],
            direction: [0.0, 1.0, 0.0],
        },
        Interval::new(0.0, 1.0).unwrap(),
        false,
    );
    let shape = builder
        .finish_wire(
            vec![
                WireUse {
                    edge: first,
                    from: a,
                    to: b,
                    sense: Orientation::Forward,
                },
                WireUse {
                    edge: second,
                    from: b,
                    to: c,
                    sense: Orientation::Forward,
                },
            ],
            false,
        )
        .unwrap();
    assert_eq!(shape.body_type().unwrap(), BodyType::Wire);
    assert_eq!(shape.topology.halfedges[0].next, Some(1));
    assert_eq!(shape.topology.halfedges[1].prev, Some(0));
    assert_eq!(shape.topology.halfedges[1].next, None);
}
