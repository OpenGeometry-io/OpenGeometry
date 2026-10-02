use crate::brep::error::{invalid, missing, positive, GeometryError};
use crate::brep::geometry::{Curve, CurveGeometry};
use crate::brep::topology::{BrepEnvelope, Edge, EdgeGeometry, HalfEdge, Orientation};
use crate::math::{norm, sub, Interval, Point3};

impl BrepEnvelope {
    pub(super) fn incident_halfedges(&self) -> Result<Vec<Vec<u32>>, GeometryError> {
        let t = &self.topology;
        let mut incident = vec![Vec::new(); t.edges.len()];
        for (i, h) in t.halfedges.iter().enumerate() {
            if h.id as usize != i {
                return Err(invalid("halfedge IDs must be dense"));
            }
            t.vertices
                .get(h.from as usize)
                .ok_or_else(|| missing("vertex", h.from))?;
            t.vertices
                .get(h.to as usize)
                .ok_or_else(|| missing("vertex", h.to))?;
            t.edges
                .get(h.edge as usize)
                .ok_or_else(|| missing("edge", h.edge))?;
            incident[h.edge as usize].push(h.id);
            self.check_halfedge_links(h)?;
            self.check_halfedge_owner(h)?;
        }
        Ok(incident)
    }

    fn check_halfedge_links(&self, h: &HalfEdge) -> Result<(), GeometryError> {
        let t = &self.topology;
        if let Some(twin) = h.twin {
            let twin = t
                .halfedges
                .get(twin as usize)
                .ok_or_else(|| missing("halfedge", twin))?;
            if twin.id == h.id
                || twin.twin != Some(h.id)
                || twin.edge != h.edge
                || twin.from != h.to
                || twin.to != h.from
                || twin.geometry_use.sense == h.geometry_use.sense
            {
                return Err(invalid("inconsistent halfedge twins"));
            }
        }
        if let Some(next) = h.next {
            let n = t
                .halfedges
                .get(next as usize)
                .ok_or_else(|| missing("halfedge", next))?;
            if n.prev != Some(h.id)
                || n.from != h.to
                || n.face != h.face
                || n.wire_ref != h.wire_ref
                || n.loop_ref != h.loop_ref
            {
                return Err(invalid("inconsistent next link"));
            }
        }
        if let Some(prev) = h.prev {
            if t.halfedges
                .get(prev as usize)
                .ok_or_else(|| missing("halfedge", prev))?
                .next
                != Some(h.id)
            {
                return Err(invalid("inconsistent previous link"));
            }
        }
        Ok(())
    }

    fn check_halfedge_owner(&self, h: &HalfEdge) -> Result<(), GeometryError> {
        let t = &self.topology;
        match (h.face, h.wire_ref) {
            (Some(face), None) => {
                t.faces
                    .get(face as usize)
                    .ok_or_else(|| missing("face", face))?;
                let p = h
                    .geometry_use
                    .pcurve
                    .ok_or_else(|| invalid("face halfedge has no pcurve"))?;
                self.geometry
                    .pcurves
                    .get(p as usize)
                    .ok_or_else(|| missing("pcurve", p))?;
                if h.loop_ref.is_none() || h.next.is_none() || h.prev.is_none() {
                    return Err(invalid("face halfedge has incomplete loop links"));
                }
            }
            (None, Some(wire)) => {
                t.wires
                    .get(wire as usize)
                    .ok_or_else(|| missing("wire", wire))?;
                if h.geometry_use.pcurve.is_some() || h.loop_ref.is_some() {
                    return Err(invalid("wire use carries face geometry"));
                }
            }
            _ => return Err(invalid("halfedge must belong to one face or wire")),
        }
        Ok(())
    }

    pub(super) fn check_edges(&self, incident: &[Vec<u32>]) -> Result<(), GeometryError> {
        let t = &self.topology;
        for (i, e) in t.edges.iter().enumerate() {
            if e.id as usize != i {
                return Err(invalid("edge IDs must be dense"));
            }
            positive(e.tolerance)?;
            if e.tolerance > self.accuracy.geometric {
                return Err(invalid("edge tolerance exceeds the model accuracy budget"));
            }
            let list = &incident[i];
            if list.is_empty() || list.len() > 2 || !list.contains(&e.halfedge) {
                return Err(invalid("invalid edge incidence"));
            }
            if list.len() == 2 {
                let twin = e
                    .twin_halfedge
                    .ok_or_else(|| invalid("two-use edge lacks twin halfedge"))?;
                if !list.contains(&twin)
                    || twin == e.halfedge
                    || t.halfedges[e.halfedge as usize].twin != Some(twin)
                {
                    return Err(invalid("edge incidence does not match twins"));
                }
            } else if e.twin_halfedge.is_some() || t.halfedges[e.halfedge as usize].twin.is_some() {
                return Err(invalid("one-use edge has an extra twin"));
            }
            let range = e.geometry.range();
            Interval::new(range.lo, range.hi)?;
            if range.lo == range.hi {
                return Err(invalid("regular curve range must be nonzero"));
            }
            let points = self.edge_endpoints(e, range)?;
            for &id in list {
                let h = &t.halfedges[id as usize];
                self.check_edge_use(e, h, points)?;
            }
        }
        Ok(())
    }

    fn edge_endpoints(&self, e: &Edge, range: Interval) -> Result<[Point3; 2], GeometryError> {
        let t = &self.topology;
        let points = match e.geometry {
            EdgeGeometry::Curve { curve, .. } => [
                self.geometry.curve(curve)?.point_at(range.lo)?,
                self.geometry.curve(curve)?.point_at(range.hi)?,
            ],
            EdgeGeometry::Collapsed { vertex } => {
                let p = t
                    .vertices
                    .get(vertex as usize)
                    .ok_or_else(|| missing("vertex", vertex))?
                    .position;
                [p, p]
            }
        };
        Ok(points)
    }

    fn check_edge_use(
        &self,
        e: &Edge,
        h: &HalfEdge,
        points: [Point3; 2],
    ) -> Result<(), GeometryError> {
        let t = &self.topology;
        let endpoints = if h.geometry_use.sense == Orientation::Forward {
            points
        } else {
            [points[1], points[0]]
        };
        let tolerance = e
            .tolerance
            .max(t.vertices[h.from as usize].tolerance)
            .max(t.vertices[h.to as usize].tolerance);
        if norm(sub(endpoints[0], t.vertices[h.from as usize].position)) > tolerance
            || norm(sub(endpoints[1], t.vertices[h.to as usize].position)) > tolerance
        {
            return Err(invalid("edge endpoints disagree with vertices"));
        }
        if h.from == h.to
            && matches!(e.geometry, EdgeGeometry::Curve { .. })
            && norm(sub(points[0], points[1])) > tolerance
        {
            return Err(invalid("self-loop curve is not closed"));
        }
        if h.from == h.to
            && matches!(e.geometry, EdgeGeometry::Curve { curve, .. } if matches!(self.geometry.curves[curve as usize], CurveGeometry::Line { .. }))
        {
            return Err(invalid("a regular line cannot form a closed edge"));
        }
        if matches!(e.geometry, EdgeGeometry::Collapsed { .. })
            && (h.from != h.to || h.face.is_none())
        {
            return Err(invalid("collapsed edge must be a singular face use"));
        }
        if let Some(face) = h.face {
            self.validate_face_use(h, &t.faces[face as usize], e, tolerance)?;
        }
        Ok(())
    }
}
