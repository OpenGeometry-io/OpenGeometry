use super::entities::{
    Edge, EdgeGeometry, Face, HalfEdge, HalfEdgeGeometryUse, Loop, TrimRegion, Vertex,
};
use super::envelope::BrepEnvelope;
use super::orientation::Orientation;
use super::provenance::{FaceProvenance, FaceRole, FaceSource};
use crate::brep::accuracy::Accuracy;
use crate::brep::frame::Frame3;
use crate::brep::geometry::{CurveGeometry, SurfaceGeometry};
use crate::brep::pcurve::PcurveGeometry;
use crate::math::Interval;

pub(crate) fn circular_face() -> BrepEnvelope {
    let mut b = BrepEnvelope::new("circle".into(), fine_accuracy()).unwrap();
    b.geometry.surfaces.push(SurfaceGeometry::Plane {
        frame: Frame3::IDENTITY,
    });
    b.geometry.curves.push(CurveGeometry::Circle {
        frame: Frame3::IDENTITY,
        radius: 1.0,
    });
    b.geometry.pcurves.push(PcurveGeometry::Conic2 {
        origin: [0.0; 2],
        axis_a: [1.0, 0.0],
        axis_b: [0.0, 1.0],
    });
    b.topology.vertices.push(Vertex {
        id: 0,
        position: [1.0, 0.0, 0.0],
        outgoing_halfedge: Some(0),
        tolerance: 1e-9,
    });
    b.topology.edges.push(Edge {
        id: 0,
        geometry: EdgeGeometry::Curve {
            curve: 0,
            range: Interval::new(0.0, std::f64::consts::TAU).unwrap(),
        },
        halfedge: 0,
        twin_halfedge: None,
        tolerance: 1e-9,
        chart_seam: false,
    });
    b.topology.halfedges.push(HalfEdge {
        id: 0,
        from: 0,
        to: 0,
        twin: None,
        next: Some(0),
        prev: Some(0),
        edge: 0,
        face: Some(0),
        loop_ref: Some(0),
        wire_ref: None,
        geometry_use: HalfEdgeGeometryUse {
            sense: Orientation::Forward,
            pcurve: Some(0),
            periodic_lift: [0; 2],
        },
    });
    b.topology.loops.push(Loop {
        id: 0,
        start_halfedge: 0,
        face_ref: 0,
        is_hole: false,
    });
    b.topology.faces.push(Face {
        id: 0,
        key: "disc".into(),
        surface: 0,
        sense: Orientation::Forward,
        trim: TrimRegion {
            chart: 0,
            uv_bounds: [Interval::new(-1.0, 1.0).unwrap(); 2],
            outer: 0,
            holes: Vec::new(),
        },
        shell_ref: None,
        provenance: FaceProvenance {
            sources: vec![FaceSource {
                entity: "circle".into(),
                body: "circle".into(),
                key: "disc".into(),
                face: 0,
            }],
            role: FaceRole::Authored,
            reversed: false,
        },
    });
    b
}

pub(crate) fn fine_accuracy() -> Accuracy {
    Accuracy {
        geometric: 1e-9,
        intersection: 1e-10,
        tessellation: 1e-3,
        exchange: 1e-5,
    }
}

pub(crate) fn coarse_accuracy(exchange: f64) -> Accuracy {
    Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange,
    }
}
