use super::brep_builder::Builder;
use crate::brep::error::GeometryError;
use crate::brep::topology::{BrepEnvelope, HalfEdge, HalfEdgeGeometryUse, Orientation, Wire};

pub(crate) struct WireUse {
    pub(crate) edge: u32,
    pub(crate) from: u32,
    pub(crate) to: u32,
    pub(crate) sense: Orientation,
}

impl Builder {
    pub(crate) fn finish_wire(
        mut self,
        uses: Vec<WireUse>,
        is_closed: bool,
    ) -> Result<BrepEnvelope, GeometryError> {
        if uses.is_empty() || !self.brep.topology.faces.is_empty() {
            return Err(GeometryError::InvalidTopology(
                "wire builder has no edges or contains faces".into(),
            ));
        }
        let start = self.brep.topology.halfedges.len() as u32;
        let count = uses.len() as u32;
        let wire_id = self.brep.topology.wires.len() as u32;
        for (index, use_edge) in uses.into_iter().enumerate() {
            let edge = self
                .brep
                .topology
                .edges
                .get_mut(use_edge.edge as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "edge".into(),
                    index: use_edge.edge,
                })?;
            if edge.halfedge != u32::MAX {
                return Err(GeometryError::InvalidTopology(
                    "wire edge already has a use".into(),
                ));
            }
            let id = start + index as u32;
            edge.halfedge = id;
            let vertex = self
                .brep
                .topology
                .vertices
                .get_mut(use_edge.from as usize)
                .ok_or_else(|| GeometryError::MissingReference {
                    kind: "vertex".into(),
                    index: use_edge.from,
                })?;
            if vertex.outgoing_halfedge.is_none() {
                vertex.outgoing_halfedge = Some(id);
            }
            self.brep.topology.halfedges.push(HalfEdge {
                id,
                from: use_edge.from,
                to: use_edge.to,
                twin: None,
                next: if index + 1 < count as usize {
                    Some(id + 1)
                } else if is_closed {
                    Some(start)
                } else {
                    None
                },
                prev: if index > 0 {
                    Some(id - 1)
                } else if is_closed {
                    Some(start + count - 1)
                } else {
                    None
                },
                edge: use_edge.edge,
                face: None,
                loop_ref: None,
                wire_ref: Some(wire_id),
                geometry_use: HalfEdgeGeometryUse {
                    sense: use_edge.sense,
                    pcurve: None,
                    periodic_lift: [0; 2],
                },
            });
        }
        self.brep.topology.wires.push(Wire {
            id: wire_id,
            start_halfedge: start,
            is_closed,
        });
        self.brep.validate()?;
        Ok(self.brep)
    }
}
