use crate::brep::{
    Accuracy, BrepEnvelope, Builder, CurveGeometry, GeometryError, HalfEdge, HalfEdgeGeometryUse,
    Orientation, Wire,
};
use crate::math::Interval;

pub fn arc_wire(
    id: String,
    curve: CurveGeometry,
    start_angle: f64,
    sweep_angle: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    curve.validate()?;
    accuracy.validate()?;
    let radius = match &curve {
        CurveGeometry::Circle { radius, .. } => *radius,
        CurveGeometry::Ellipse { minor_radius, .. } => *minor_radius,
        _ => {
            return Err(GeometryError::UnsupportedGeometry(
                "arc wire requires a circle or ellipse".into(),
            ))
        }
    };
    if !start_angle.is_finite()
        || !sweep_angle.is_finite()
        || sweep_angle == 0.0
        || sweep_angle.abs() > std::f64::consts::TAU
    {
        return Err(GeometryError::InvalidGeometry(
            "arc angles require a finite, nonzero sweep of at most one turn".into(),
        ));
    }
    let end = start_angle + sweep_angle;
    let range = Interval::new(start_angle.min(end), start_angle.max(end))?;
    if range.width() * radius <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "arc span is below geometric resolution".into(),
        ));
    }
    let mut builder = Builder::new(id, accuracy)?;
    let closed = sweep_angle.abs() == std::f64::consts::TAU;
    let from = builder.vertex(curve.elementary_point(start_angle)?);
    let to = if closed {
        from
    } else {
        builder.vertex(curve.elementary_point(end)?)
    };
    let edge = builder.edge(curve, range, false);
    builder.brep.topology.edges[edge as usize].halfedge = 0;
    builder.brep.topology.vertices[from as usize].outgoing_halfedge = Some(0);
    builder.brep.topology.halfedges.push(HalfEdge {
        id: 0,
        from,
        to,
        twin: None,
        next: closed.then_some(0),
        prev: closed.then_some(0),
        edge,
        face: None,
        loop_ref: None,
        wire_ref: Some(0),
        geometry_use: HalfEdgeGeometryUse {
            sense: if sweep_angle > 0.0 {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
            pcurve: None,
            periodic_lift: [0; 2],
        },
    });
    builder.brep.topology.wires.push(Wire {
        id: 0,
        start_halfedge: 0,
        is_closed: closed,
    });
    builder.brep.validate()?;
    Ok(builder.brep)
}
