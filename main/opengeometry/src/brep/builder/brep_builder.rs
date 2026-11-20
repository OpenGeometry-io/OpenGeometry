use crate::brep::accuracy::Accuracy;
use crate::brep::error::{missing, GeometryError};
use crate::brep::geometry::{CurveGeometry, SurfaceGeometry};
use crate::brep::pcurve::PcurveGeometry;
use crate::brep::topology::{
    BrepEnvelope, Edge, EdgeGeometry, Face, FaceProvenance, FaceRole, FaceSource, HalfEdge,
    HalfEdgeGeometryUse, Loop, Orientation, Shell, SolidRegion, TrimRegion, Vertex,
};
use crate::math::{Interval, Point3};
use std::collections::BTreeMap;

pub struct Use {
    pub(super) edge: u32,
    pub(super) from: u32,
    pub(super) to: u32,
    pub(super) sense: Orientation,
    pub(super) pcurve: PcurveGeometry,
}

pub struct Builder {
    pub(crate) brep: BrepEnvelope,
}
impl Builder {
    pub fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
        Ok(Self {
            brep: BrepEnvelope::new(id, accuracy)?,
        })
    }
    pub fn brep(&self) -> &BrepEnvelope {
        &self.brep
    }
    pub fn vertex(&mut self, p: Point3) -> u32 {
        let id = self.brep.topology.vertices.len() as u32;
        self.brep.topology.vertices.push(Vertex {
            id,
            position: p,
            outgoing_halfedge: None,
            tolerance: self.brep.accuracy.geometric,
        });
        id
    }
    pub fn edge(&mut self, curve: CurveGeometry, range: Interval, seam: bool) -> u32 {
        let curve_id = self.brep.geometry.curves.len() as u32;
        self.brep.geometry.curves.push(curve);
        self.edge_geometry(
            EdgeGeometry::Curve {
                curve: curve_id,
                range,
            },
            seam,
        )
    }
    pub(crate) fn edge_geometry(&mut self, geometry: EdgeGeometry, seam: bool) -> u32 {
        let id = self.brep.topology.edges.len() as u32;
        self.brep.topology.edges.push(Edge {
            id,
            geometry,
            halfedge: u32::MAX,
            twin_halfedge: None,
            tolerance: self.brep.accuracy.geometric,
            chart_seam: seam,
        });
        id
    }
    pub(crate) fn face_loop(
        &mut self,
        face: u32,
        is_hole: bool,
        uses: Vec<Use>,
    ) -> Result<u32, GeometryError> {
        self.check_loops([&uses])?;
        Ok(self.write_loop(face, is_hole, uses))
    }
    fn check_loops<'a>(
        &self,
        loops: impl IntoIterator<Item = &'a Vec<Use>>,
    ) -> Result<(), GeometryError> {
        let topology = &self.brep.topology;
        let mut uses_of_edge = BTreeMap::new();
        for uses in loops {
            if uses.is_empty() {
                return Err(GeometryError::InvalidTopology(
                    "empty primitive face".into(),
                ));
            }
            for u in uses {
                let edge = topology
                    .edges
                    .get(u.edge as usize)
                    .ok_or_else(|| missing("edge", u.edge))?;
                if let Some(&vertex) = [u.from, u.to]
                    .iter()
                    .find(|&&vertex| vertex as usize >= topology.vertices.len())
                {
                    return Err(missing("vertex", vertex));
                }
                let uses = uses_of_edge.entry(u.edge).or_insert(
                    usize::from(edge.halfedge != u32::MAX)
                        + usize::from(edge.twin_halfedge.is_some()),
                );
                *uses += 1;
                if *uses > 2 {
                    return Err(GeometryError::InvalidTopology(
                        "nonmanifold primitive edge".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    fn write_loop(&mut self, face: u32, is_hole: bool, uses: Vec<Use>) -> u32 {
        let loop_id = self.brep.topology.loops.len() as u32;
        let start = self.brep.topology.halfedges.len() as u32;
        let count = uses.len() as u32;
        for (i, u) in uses.into_iter().enumerate() {
            let id = start + i as u32;
            let pcurve = self.brep.geometry.pcurves.len() as u32;
            self.brep.geometry.pcurves.push(u.pcurve);
            let e = &mut self.brep.topology.edges[u.edge as usize];
            let twin = if e.halfedge == u32::MAX {
                e.halfedge = id;
                None
            } else {
                e.twin_halfedge = Some(id);
                self.brep.topology.halfedges[e.halfedge as usize].twin = Some(id);
                Some(e.halfedge)
            };
            self.brep.topology.halfedges.push(HalfEdge {
                id,
                from: u.from,
                to: u.to,
                twin,
                next: Some(start + (i as u32 + 1) % count),
                prev: Some(start + (i as u32 + count - 1) % count),
                edge: u.edge,
                face: Some(face),
                loop_ref: Some(loop_id),
                wire_ref: None,
                geometry_use: HalfEdgeGeometryUse {
                    sense: u.sense,
                    pcurve: Some(pcurve),
                    periodic_lift: [0; 2],
                },
            });
            if self.brep.topology.vertices[u.from as usize]
                .outgoing_halfedge
                .is_none()
            {
                self.brep.topology.vertices[u.from as usize].outgoing_halfedge = Some(id);
            }
        }
        self.brep.topology.loops.push(Loop {
            id: loop_id,
            start_halfedge: start,
            face_ref: face,
            is_hole,
        });
        loop_id
    }
    pub fn face(
        &mut self,
        key: &str,
        surface: SurfaceGeometry,
        bounds: [[f64; 2]; 2],
        uses: Vec<Use>,
    ) -> Result<u32, GeometryError> {
        self.face_with_holes(key, surface, bounds, uses, Vec::new())
    }
    pub fn face_with_holes(
        &mut self,
        key: &str,
        surface: SurfaceGeometry,
        bounds: [[f64; 2]; 2],
        outer_uses: Vec<Use>,
        hole_uses: Vec<Vec<Use>>,
    ) -> Result<u32, GeometryError> {
        let uv_bounds = [
            Interval::new(bounds[0][0], bounds[0][1])?,
            Interval::new(bounds[1][0], bounds[1][1])?,
        ];
        self.check_loops(std::iter::once(&outer_uses).chain(&hole_uses))?;
        let surface_id = self.brep.geometry.surfaces.len() as u32;
        self.brep.geometry.surfaces.push(surface);
        let face = self.brep.topology.faces.len() as u32;
        let outer = self.write_loop(face, false, outer_uses);
        let holes = hole_uses
            .into_iter()
            .map(|uses| self.write_loop(face, true, uses))
            .collect();
        self.brep.topology.faces.push(Face {
            id: face,
            key: key.into(),
            surface: surface_id,
            sense: Orientation::Forward,
            trim: TrimRegion {
                chart: 0,
                uv_bounds,
                outer,
                holes,
            },
            shell_ref: Some(0),
            provenance: FaceProvenance {
                sources: vec![FaceSource {
                    entity: self.brep.id.clone(),
                    body: self.brep.id.clone(),
                    key: key.into(),
                    face,
                }],
                role: FaceRole::Authored,
                reversed: false,
            },
        });
        Ok(face)
    }
    pub fn face_sense(&self, face: u32) -> Result<Orientation, GeometryError> {
        self.brep
            .topology
            .faces
            .get(face as usize)
            .map(|face| face.sense)
            .ok_or_else(|| missing("face", face))
    }
    pub fn set_face_sense(&mut self, face: u32, sense: Orientation) -> Result<(), GeometryError> {
        self.brep
            .topology
            .faces
            .get_mut(face as usize)
            .ok_or_else(|| missing("face", face))?
            .sense = sense;
        Ok(())
    }
    pub fn finish_solid(mut self) -> Result<BrepEnvelope, GeometryError> {
        let faces = (0..self.brep.topology.faces.len() as u32).collect();
        self.brep.topology.shells.push(Shell {
            id: 0,
            faces,
            is_closed: true,
        });
        self.brep.solids.push(SolidRegion {
            outer_shell: 0,
            cavity_shells: Vec::new(),
        });
        self.brep.validate()?;
        Ok(self.brep)
    }
}
