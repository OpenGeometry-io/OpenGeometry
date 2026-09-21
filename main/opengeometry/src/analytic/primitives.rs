use super::{
    face_intersection::{intersect_faces, FaceView, IntersectionBranch, IntersectionGraph},
    geometry::{add, cross, dot, norm, scale, sub, unit},
    intersection::{IntersectionDefinition, TraceAnchor},
    topology::*,
    Curve, CurveGeometry, Frame3, GeometryError, Point3, Surface, SurfaceGeometry,
};
use crate::math::interval::Interval;
use std::collections::BTreeMap;

pub(super) struct Use {
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
    pcurve: PcurveGeometry,
}
pub(super) struct Builder {
    pub(super) brep: BrepEnvelope,
}
impl Builder {
    pub(super) fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
        Ok(Self {
            brep: BrepEnvelope::new(id, accuracy)?,
        })
    }
    pub(super) fn vertex(&mut self, p: Point3) -> u32 {
        let id = self.brep.topology.vertices.len() as u32;
        self.brep.topology.vertices.push(Vertex {
            id,
            position: p,
            outgoing_halfedge: None,
            tolerance: self.brep.accuracy.geometric,
        });
        id
    }
    pub(super) fn edge(&mut self, curve: CurveGeometry, range: Interval, seam: bool) -> u32 {
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
    pub(super) fn edge_geometry(&mut self, geometry: EdgeGeometry, seam: bool) -> u32 {
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
    fn face_loop(
        &mut self,
        face: u32,
        is_hole: bool,
        uses: Vec<Use>,
    ) -> Result<u32, GeometryError> {
        let loop_id = self.brep.topology.loops.len() as u32;
        let start = self.brep.topology.halfedges.len() as u32;
        let count = uses.len() as u32;
        if count == 0 {
            return Err(GeometryError::InvalidTopology(
                "empty primitive face".into(),
            ));
        }
        for (i, u) in uses.into_iter().enumerate() {
            let id = start + i as u32;
            let pcurve = self.brep.geometry.pcurves.len() as u32;
            self.brep.geometry.pcurves.push(u.pcurve);
            let e = self
                .brep
                .topology
                .edges
                .get_mut(u.edge as usize)
                .ok_or_else(|| {
                    GeometryError::InvalidTopology("builder references missing edge".into())
                })?;
            let twin = if e.halfedge == u32::MAX {
                e.halfedge = id;
                None
            } else {
                if e.twin_halfedge.is_some() {
                    return Err(GeometryError::InvalidTopology(
                        "nonmanifold primitive edge".into(),
                    ));
                }
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
        Ok(loop_id)
    }
    pub(super) fn face(
        &mut self,
        key: &str,
        surface: SurfaceGeometry,
        bounds: [[f64; 2]; 2],
        uses: Vec<Use>,
    ) -> Result<(), GeometryError> {
        self.face_with_holes(key, surface, bounds, uses, Vec::new())
    }
    pub(super) fn face_with_holes(
        &mut self,
        key: &str,
        surface: SurfaceGeometry,
        bounds: [[f64; 2]; 2],
        outer_uses: Vec<Use>,
        hole_uses: Vec<Vec<Use>>,
    ) -> Result<(), GeometryError> {
        let surface_id = self.brep.geometry.surfaces.len() as u32;
        self.brep.geometry.surfaces.push(surface);
        let face = self.brep.topology.faces.len() as u32;
        let outer = self.face_loop(face, false, outer_uses)?;
        let mut holes = Vec::with_capacity(hole_uses.len());
        for uses in hole_uses {
            holes.push(self.face_loop(face, true, uses)?);
        }
        self.brep.topology.faces.push(Face {
            id: face,
            key: key.into(),
            surface: surface_id,
            sense: Orientation::Forward,
            trim: TrimRegion {
                chart: 0,
                uv_bounds: [
                    Interval::new(bounds[0][0], bounds[0][1])?,
                    Interval::new(bounds[1][0], bounds[1][1])?,
                ],
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
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<BrepEnvelope, GeometryError> {
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

pub(super) fn uv_line(origin: [f64; 2], direction: [f64; 2]) -> PcurveGeometry {
    PcurveGeometry::Line2 { origin, direction }
}
pub(super) fn boundary(
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
    pcurve: PcurveGeometry,
) -> Use {
    Use {
        edge,
        from,
        to,
        sense,
        pcurve,
    }
}
fn dimensions(values: &[f64]) -> Result<(), GeometryError> {
    if values.iter().any(|&x| !x.is_finite() || x <= 0.0) {
        return Err(GeometryError::InvalidGeometry(
            "primitive dimensions must be positive and finite".into(),
        ));
    }
    Ok(())
}

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

fn section(
    id: String,
    frame: Frame3,
    surface: SurfaceGeometry,
    r0: f64,
    r1: f64,
    z0: f64,
    z1: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    surface.validate()?;
    dimensions(&[r1, z1 - z0])?;
    if !r0.is_finite() || r0 < 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "negative lower radius".into(),
        ));
    }
    let mut b = Builder::new(id, accuracy)?;
    let low = b.vertex(frame.point([r0, 0.0, z0]));
    let high = b.vertex(frame.point([r1, 0.0, z1]));
    let tau = std::f64::consts::TAU;
    let mut f0 = frame;
    f0.origin = frame.point([0.0, 0.0, z0]);
    let mut f1 = frame;
    f1.origin = frame.point([0.0, 0.0, z1]);
    let bottom = if r0 == 0.0 {
        b.edge_geometry(EdgeGeometry::Collapsed { vertex: low }, true)
    } else {
        b.edge(
            CurveGeometry::Circle {
                frame: f0,
                radius: r0,
            },
            Interval::new(0.0, tau)?,
            false,
        )
    };
    let top = b.edge(
        CurveGeometry::Circle {
            frame: f1,
            radius: r1,
        },
        Interval::new(0.0, tau)?,
        false,
    );
    let p0 = frame.point([r0, 0.0, z0]);
    let p1 = frame.point([r1, 0.0, z1]);
    let length = norm(sub(p1, p0));
    let seam = b.edge(
        CurveGeometry::Line {
            origin: p0,
            direction: unit(sub(p1, p0))?,
        },
        Interval::new(0.0, length)?,
        true,
    );
    b.face(
        "lateral",
        surface,
        [[0.0, tau], [z0, z1]],
        vec![
            boundary(
                bottom,
                low,
                low,
                Orientation::Forward,
                if r0 == 0.0 {
                    uv_line([0.0, z0], [tau, 0.0])
                } else {
                    uv_line([0.0, z0], [1.0, 0.0])
                },
            ),
            boundary(
                seam,
                low,
                high,
                Orientation::Forward,
                uv_line([tau, z0], [0.0, (z1 - z0) / length]),
            ),
            boundary(
                top,
                high,
                high,
                Orientation::Reverse,
                uv_line([0.0, z1], [1.0, 0.0]),
            ),
            boundary(
                seam,
                high,
                low,
                Orientation::Reverse,
                uv_line([0.0, z0], [0.0, (z1 - z0) / length]),
            ),
        ],
    )?;
    if r0 > 0.0 {
        f0.y = scale(f0.y, -1.0);
        f0.z = scale(f0.z, -1.0);
        b.face(
            "lower_cap",
            SurfaceGeometry::Plane { frame: f0 },
            [[-r0, r0]; 2],
            vec![boundary(
                bottom,
                low,
                low,
                Orientation::Reverse,
                PcurveGeometry::Conic2 {
                    origin: [0.0; 2],
                    axis_a: [r0, 0.0],
                    axis_b: [0.0, -r0],
                },
            )],
        )?;
    }
    b.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: f1 },
        [[-r1, r1]; 2],
        vec![boundary(
            top,
            high,
            high,
            Orientation::Forward,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [r1, 0.0],
                axis_b: [0.0, r1],
            },
        )],
    )?;
    b.finish()
}

pub fn cylinder(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    dimensions(&[radius, height])?;
    section(
        id,
        frame,
        SurfaceGeometry::Cylinder { frame, radius },
        radius,
        radius,
        0.0,
        height,
        accuracy,
    )
}

pub fn cylinder_sector(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    start_angle: f64,
    sweep_angle: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[radius, height])?;
    if !start_angle.is_finite()
        || !sweep_angle.is_finite()
        || sweep_angle == 0.0
        || sweep_angle.abs() > std::f64::consts::TAU
    {
        return Err(GeometryError::InvalidGeometry(
            "cylinder-sector angles require a finite, nonzero sweep of at most one turn".into(),
        ));
    }
    if (sweep_angle.abs() - std::f64::consts::TAU).abs() <= 64.0 * f64::EPSILON {
        return cylinder(id, frame, radius, height, accuracy);
    }
    if radius * sweep_angle.abs() <= 4.0 * accuracy.geometric
        || radius <= 4.0 * accuracy.geometric
        || height <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::InvalidGeometry(
            "cylinder-sector features are below geometric resolution".into(),
        ));
    }

    let end_angle = start_angle + sweep_angle;
    let lo = start_angle.min(end_angle);
    let hi = start_angle.max(end_angle);
    let range = Interval::new(lo, hi)?;
    let point = |angle: f64, elevation: f64| {
        frame.point([radius * angle.cos(), radius * angle.sin(), elevation])
    };
    let radial = |angle: f64| frame.vector([angle.cos(), angle.sin(), 0.0]);
    let tangent = |angle: f64| frame.vector([-angle.sin(), angle.cos(), 0.0]);
    let mut b = Builder::new(id, accuracy)?;
    let bottom_center = b.vertex(frame.origin);
    let bottom_start = b.vertex(point(lo, 0.0));
    let bottom_end = b.vertex(point(hi, 0.0));
    let top_center = b.vertex(frame.point([0.0, 0.0, height]));
    let top_start = b.vertex(point(lo, height));
    let top_end = b.vertex(point(hi, height));

    let line_edge = |builder: &mut Builder,
                     origin: Point3,
                     direction: Point3,
                     length: f64|
     -> Result<u32, GeometryError> {
        Ok(builder.edge(
            CurveGeometry::Line { origin, direction },
            Interval::new(0.0, length)?,
            false,
        ))
    };
    let bottom_start_edge = line_edge(&mut b, frame.origin, radial(lo), radius)?;
    let bottom_end_edge = line_edge(&mut b, frame.origin, radial(hi), radius)?;
    let top_origin = frame.point([0.0, 0.0, height]);
    let top_start_edge = line_edge(&mut b, top_origin, radial(lo), radius)?;
    let top_end_edge = line_edge(&mut b, top_origin, radial(hi), radius)?;
    let center_edge = line_edge(&mut b, frame.origin, frame.z, height)?;
    let start_edge = line_edge(&mut b, point(lo, 0.0), frame.z, height)?;
    let end_edge = line_edge(&mut b, point(hi, 0.0), frame.z, height)?;
    let bottom_arc = b.edge(CurveGeometry::Circle { frame, radius }, range, false);
    let mut top_frame = frame;
    top_frame.origin = top_origin;
    let top_arc = b.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius,
        },
        range,
        false,
    );
    use Orientation::{Forward as F, Reverse as R};

    b.face(
        "lateral",
        SurfaceGeometry::Cylinder { frame, radius },
        [[lo, hi], [0.0, height]],
        vec![
            boundary(
                bottom_arc,
                bottom_start,
                bottom_end,
                F,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                end_edge,
                bottom_end,
                top_end,
                F,
                uv_line([hi, 0.0], [0.0, 1.0]),
            ),
            boundary(
                top_arc,
                top_end,
                top_start,
                R,
                uv_line([0.0, height], [1.0, 0.0]),
            ),
            boundary(
                start_edge,
                top_start,
                bottom_start,
                R,
                uv_line([lo, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;

    let bottom_frame = Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
    };
    let bottom_uses = [
        (bottom_end_edge, bottom_center, bottom_end, F),
        (bottom_arc, bottom_end, bottom_start, R),
        (bottom_start_edge, bottom_start, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(&b, bottom_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "lower_cap",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        [[-radius, radius]; 2],
        bottom_uses,
    )?;

    let top_uses = [
        (top_start_edge, top_center, top_start, F),
        (top_arc, top_start, top_end, F),
        (top_end_edge, top_end, top_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(&b, top_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: top_frame },
        [[-radius, radius]; 2],
        top_uses,
    )?;

    let start_frame = Frame3 {
        origin: frame.origin,
        x: radial(lo),
        y: frame.z,
        z: scale(tangent(lo), -1.0),
    };
    let start_uses = [
        (bottom_start_edge, bottom_center, bottom_start, F),
        (start_edge, bottom_start, top_start, F),
        (top_start_edge, top_start, top_center, R),
        (center_edge, top_center, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(&b, start_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "start_radial",
        SurfaceGeometry::Plane { frame: start_frame },
        [[0.0, radius], [0.0, height]],
        start_uses,
    )?;

    let end_frame = Frame3 {
        origin: frame.origin,
        x: frame.z,
        y: radial(hi),
        z: tangent(hi),
    };
    let end_uses = [
        (center_edge, bottom_center, top_center, F),
        (top_end_edge, top_center, top_end, F),
        (end_edge, top_end, bottom_end, R),
        (bottom_end_edge, bottom_end, bottom_center, R),
    ]
    .into_iter()
    .map(|(edge, from, to, sense)| plane_boundary(&b, end_frame, edge, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "end_radial",
        SurfaceGeometry::Plane { frame: end_frame },
        [[0.0, height], [0.0, radius]],
        end_uses,
    )?;
    b.finish()
}

pub fn annular_cylinder(
    id: String,
    frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    cylinder_with_circular_hole(
        id,
        frame,
        frame,
        inner_radius,
        outer_radius,
        height,
        accuracy,
    )
}

pub(super) fn cylinder_with_circular_hole(
    id: String,
    frame: Frame3,
    inner_frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    inner_frame.validate()?;
    accuracy.validate()?;
    dimensions(&[inner_radius, outer_radius, height])?;
    let inner_offset = sub(inner_frame.origin, frame.origin);
    let axial_offset = dot(inner_offset, frame.z);
    let radial_offset = sub(inner_offset, scale(frame.z, axial_offset));
    if norm(sub(inner_frame.z, frame.z)) > 1e-12 || axial_offset.abs() > accuracy.geometric {
        return Err(GeometryError::InvalidGeometry(
            "circular hole axis must share the outer cylinder span".into(),
        ));
    }
    if norm(radial_offset) + inner_radius >= outer_radius {
        return Err(GeometryError::InvalidGeometry(
            "circular hole must lie strictly inside the outer cylinder".into(),
        ));
    }
    if outer_radius - inner_radius - norm(radial_offset) <= 4.0 * accuracy.geometric
        || height <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::UnresolvedIntersection(
            "circular cylinder wall is below geometric resolution".into(),
        ));
    }

    let mut b = Builder::new(id, accuracy)?;
    let tau = std::f64::consts::TAU;
    let range = Interval::new(0.0, tau)?;
    let outer_low = b.vertex(frame.point([outer_radius, 0.0, 0.0]));
    let outer_high = b.vertex(frame.point([outer_radius, 0.0, height]));
    let inner_low = b.vertex(inner_frame.point([inner_radius, 0.0, 0.0]));
    let inner_high = b.vertex(inner_frame.point([inner_radius, 0.0, height]));
    let mut low_frame = frame;
    let mut high_frame = frame;
    high_frame.origin = frame.point([0.0, 0.0, height]);
    let mut inner_high_frame = inner_frame;
    inner_high_frame.origin = inner_frame.point([0.0, 0.0, height]);

    let outer_bottom = b.edge(
        CurveGeometry::Circle {
            frame: low_frame,
            radius: outer_radius,
        },
        range,
        false,
    );
    let outer_top = b.edge(
        CurveGeometry::Circle {
            frame: high_frame,
            radius: outer_radius,
        },
        range,
        false,
    );
    let inner_bottom = b.edge(
        CurveGeometry::Circle {
            frame: inner_frame,
            radius: inner_radius,
        },
        range,
        false,
    );
    let inner_top = b.edge(
        CurveGeometry::Circle {
            frame: inner_high_frame,
            radius: inner_radius,
        },
        range,
        false,
    );
    let outer_seam = b.edge(
        CurveGeometry::Line {
            origin: frame.point([outer_radius, 0.0, 0.0]),
            direction: frame.z,
        },
        Interval::new(0.0, height)?,
        true,
    );
    let inner_seam = b.edge(
        CurveGeometry::Line {
            origin: inner_frame.point([inner_radius, 0.0, 0.0]),
            direction: inner_frame.z,
        },
        Interval::new(0.0, height)?,
        true,
    );
    use Orientation::{Forward as F, Reverse as R};
    let lateral_uses = |bottom, top, seam, low, high| {
        vec![
            boundary(bottom, low, low, F, uv_line([0.0, 0.0], [1.0, 0.0])),
            boundary(seam, low, high, F, uv_line([tau, 0.0], [0.0, 1.0])),
            boundary(top, high, high, R, uv_line([0.0, height], [1.0, 0.0])),
            boundary(seam, high, low, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ]
    };
    b.face(
        "outer",
        SurfaceGeometry::Cylinder {
            frame,
            radius: outer_radius,
        },
        [[0.0, tau], [0.0, height]],
        lateral_uses(outer_bottom, outer_top, outer_seam, outer_low, outer_high),
    )?;
    b.face(
        "inner",
        SurfaceGeometry::Cylinder {
            frame: inner_frame,
            radius: inner_radius,
        },
        [[0.0, tau], [0.0, height]],
        lateral_uses(inner_bottom, inner_top, inner_seam, inner_low, inner_high),
    )?;
    b.brep.topology.faces[1].sense = R;

    low_frame.y = scale(low_frame.y, -1.0);
    low_frame.z = scale(low_frame.z, -1.0);
    let outer_lower = vec![plane_boundary(
        &b,
        low_frame,
        outer_bottom,
        outer_low,
        outer_low,
        R,
    )?];
    let inner_lower = vec![plane_boundary(
        &b,
        low_frame,
        inner_bottom,
        inner_low,
        inner_low,
        R,
    )?];
    b.face_with_holes(
        "lower_cap",
        SurfaceGeometry::Plane { frame: low_frame },
        [[-outer_radius, outer_radius]; 2],
        outer_lower,
        vec![inner_lower],
    )?;
    let outer_upper = vec![plane_boundary(
        &b, high_frame, outer_top, outer_high, outer_high, F,
    )?];
    let inner_upper = vec![plane_boundary(
        &b, high_frame, inner_top, inner_high, inner_high, F,
    )?];
    b.face_with_holes(
        "upper_cap",
        SurfaceGeometry::Plane { frame: high_frame },
        [[-outer_radius, outer_radius]; 2],
        outer_upper,
        vec![inner_upper],
    )?;
    b.finish()
}

pub fn cuboid(
    id: String,
    frame: Frame3,
    size: [f64; 3],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&size)?;
    if size.iter().any(|s| *s <= 4.0 * accuracy.geometric) {
        return Err(GeometryError::UnresolvedIntersection(
            "cuboid dimensions are below geometric resolution".into(),
        ));
    }
    let mut builder = Builder::new(id, accuracy)?;
    let points: Vec<_> = (0..8)
        .map(|i| {
            frame.point(std::array::from_fn(|axis| {
                if i & (1 << axis) == 0 {
                    0.0
                } else {
                    size[axis]
                }
            }))
        })
        .collect();
    for point in &points {
        builder.vertex(*point);
    }
    let axes = [frame.x, frame.y, frame.z];
    let mut edges = [[u32::MAX; 3]; 8];
    for i in 0..8 {
        for axis in 0..3 {
            if i & (1 << axis) == 0 {
                edges[i][axis] = builder.edge(
                    CurveGeometry::Line {
                        origin: points[i],
                        direction: axes[axis],
                    },
                    Interval::new(0.0, size[axis])?,
                    false,
                );
            }
        }
    }
    for (key, indices, normal) in [
        ("bottom", [0, 2, 3, 1], scale(frame.z, -1.0)),
        ("top", [4, 5, 7, 6], frame.z),
        ("left", [0, 4, 6, 2], scale(frame.x, -1.0)),
        ("right", [1, 3, 7, 5], frame.x),
        ("front", [0, 1, 5, 4], scale(frame.y, -1.0)),
        ("back", [2, 6, 7, 3], frame.y),
    ] {
        let face_frame = Frame3::from_axis(
            points[indices[0]],
            normal,
            sub(points[indices[1]], points[indices[0]]),
        )?;
        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        let mut uses = Vec::new();
        for i in 0..4 {
            let from = indices[i];
            let to = indices[(i + 1) % 4];
            let axis = (from ^ to).trailing_zeros() as usize;
            let edge = edges[from.min(to)][axis];
            uses.push(plane_boundary(
                &builder,
                face_frame,
                edge,
                from as u32,
                to as u32,
                if from < to {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                },
            )?);
            let uv = face_frame.local(points[from]);
            for j in 0..2 {
                bounds[j][0] = bounds[j][0].min(uv[j]);
                bounds[j][1] = bounds[j][1].max(uv[j]);
            }
        }
        for bound in &mut bounds {
            bound[0] -= accuracy.geometric;
            bound[1] += accuracy.geometric;
        }
        builder.face(
            key,
            SurfaceGeometry::Plane { frame: face_frame },
            bounds,
            uses,
        )?;
    }
    builder.finish()
}

/// Builds a closed, orientable solid whose boundary is described entirely by
/// planar polygon faces. Face loops must be ordered so their normals point out
/// of the material. Vertex and face identity are explicit; only the shared
/// edge uses are paired here.
pub fn planar_polyhedron(
    id: String,
    vertices: Vec<Point3>,
    faces: Vec<Vec<u32>>,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    accuracy.validate()?;
    if vertices.len() < 4 || faces.len() < 4 {
        return Err(GeometryError::InvalidGeometry(
            "planar polyhedron requires at least four vertices and four faces".into(),
        ));
    }
    if vertices
        .iter()
        .flat_map(|point| point.iter())
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(GeometryError::InvalidGeometry(
            "planar polyhedron vertices must be finite".into(),
        ));
    }

    let mut builder = Builder::new(id, accuracy)?;
    for point in &vertices {
        builder.vertex(*point);
    }

    struct SharedEdge {
        edge: u32,
        from: u32,
        to: u32,
        uses: u8,
    }
    let mut edges = BTreeMap::<(u32, u32), SharedEdge>::new();
    for face in &faces {
        if face.len() < 3 {
            return Err(GeometryError::InvalidGeometry(
                "planar polyhedron face requires at least three vertices".into(),
            ));
        }
        for index in 0..face.len() {
            let from = face[index];
            let to = face[(index + 1) % face.len()];
            if from == to || from as usize >= vertices.len() || to as usize >= vertices.len() {
                return Err(GeometryError::InvalidGeometry(
                    "planar polyhedron face contains an invalid edge".into(),
                ));
            }
            let key = (from.min(to), from.max(to));
            if let Some(shared) = edges.get_mut(&key) {
                if shared.uses != 1 || shared.from != to || shared.to != from {
                    return Err(GeometryError::InvalidTopology(
                        "planar polyhedron edge uses must form one opposite-oriented pair".into(),
                    ));
                }
                shared.uses = 2;
            } else {
                let delta = sub(vertices[to as usize], vertices[from as usize]);
                let length = norm(delta);
                if length <= 4.0 * accuracy.geometric {
                    return Err(GeometryError::UnresolvedIntersection(
                        "planar polyhedron edge is below geometric resolution".into(),
                    ));
                }
                let edge = builder.edge(
                    CurveGeometry::Line {
                        origin: vertices[from as usize],
                        direction: scale(delta, 1.0 / length),
                    },
                    Interval::new(0.0, length)?,
                    false,
                );
                edges.insert(
                    key,
                    SharedEdge {
                        edge,
                        from,
                        to,
                        uses: 1,
                    },
                );
            }
        }
    }
    if edges.values().any(|edge| edge.uses != 2) {
        return Err(GeometryError::InvalidTopology(
            "planar polyhedron boundary is open".into(),
        ));
    }

    for (face_index, face) in faces.iter().enumerate() {
        let origin = vertices[face[0] as usize];
        let mut normal = [0.0; 3];
        for index in 0..face.len() {
            let a = vertices[face[index] as usize];
            let b = vertices[face[(index + 1) % face.len()] as usize];
            normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
            normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
            normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let normal = unit(normal).map_err(|_| {
            GeometryError::InvalidGeometry("planar polyhedron face has zero area".into())
        })?;
        let first_edge = sub(vertices[face[1] as usize], origin);
        let frame = Frame3::from_axis(origin, normal, first_edge)?;
        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        let mut uses = Vec::with_capacity(face.len());
        for index in 0..face.len() {
            let from = face[index];
            let to = face[(index + 1) % face.len()];
            let point = vertices[from as usize];
            let local = frame.local(point);
            if local[2].abs() > 4.0 * accuracy.geometric {
                return Err(GeometryError::InvalidGeometry(
                    "planar polyhedron face vertices are not coplanar".into(),
                ));
            }
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(local[axis]);
                bounds[axis][1] = bounds[axis][1].max(local[axis]);
            }
            let shared = &edges[&(from.min(to), from.max(to))];
            uses.push(plane_boundary(
                &builder,
                frame,
                shared.edge,
                from,
                to,
                if shared.from == from {
                    Orientation::Forward
                } else {
                    Orientation::Reverse
                },
            )?);
        }
        for bound in &mut bounds {
            bound[0] -= accuracy.geometric;
            bound[1] += accuracy.geometric;
        }
        builder.face(
            &format!("face-{face_index}"),
            SurfaceGeometry::Plane { frame },
            bounds,
            uses,
        )?;
    }
    builder.finish()
}

fn signed_area(points: &[[f64; 2]]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        * 0.5
}

fn orient_profile_loop(mut points: Vec<[f64; 2]>, ccw: bool) -> Vec<[f64; 2]> {
    if (signed_area(&points) > 0.0) != ccw {
        points.reverse();
    }
    points
}

fn segment_distance(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let edge = [b[0] - a[0], b[1] - a[1]];
    let length_squared = edge[0].mul_add(edge[0], edge[1] * edge[1]);
    if length_squared == 0.0 {
        return (point[0] - a[0]).hypot(point[1] - a[1]);
    }
    let t = (((point[0] - a[0]) * edge[0] + (point[1] - a[1]) * edge[1]) / length_squared)
        .clamp(0.0, 1.0);
    (point[0] - (a[0] + t * edge[0])).hypot(point[1] - (a[1] + t * edge[1]))
}

fn loops_touch(a: &[[f64; 2]], b: &[[f64; 2]], tolerance: f64) -> bool {
    use crate::geometry::poly2d::{segments_cross2, Pt2};

    a.iter().any(|point| {
        b.iter()
            .zip(b.iter().cycle().skip(1))
            .any(|(from, to)| segment_distance(*point, *from, *to) <= tolerance)
    }) || b.iter().any(|point| {
        a.iter()
            .zip(a.iter().cycle().skip(1))
            .any(|(from, to)| segment_distance(*point, *from, *to) <= tolerance)
    }) || a.iter().zip(a.iter().cycle().skip(1)).any(|(a0, a1)| {
        b.iter().zip(b.iter().cycle().skip(1)).any(|(b0, b1)| {
            segments_cross2(
                Pt2::new(a0[0], a0[1]),
                Pt2::new(a1[0], a1[1]),
                Pt2::new(b0[0], b0[1]),
                Pt2::new(b1[0], b1[1]),
                tolerance,
            )
        })
    })
}

fn validate_profile_loop(
    points: &[[f64; 2]],
    tolerance: f64,
    label: &str,
) -> Result<(), GeometryError> {
    use crate::geometry::poly2d::{self_intersects2, Pt2};

    if points.len() < 3 {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} requires at least three vertices"
        )));
    }
    if points
        .iter()
        .flatten()
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} contains a non-finite coordinate"
        )));
    }
    let resolution = 4.0 * tolerance;
    if points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .any(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= resolution)
    {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "{label} contains an edge below geometric resolution"
        )));
    }
    if signed_area(points).abs() <= resolution * resolution {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "{label} area is below geometric resolution"
        )));
    }
    let planar = points
        .iter()
        .map(|point| Pt2::new(point[0], point[1]))
        .collect::<Vec<_>>();
    if self_intersects2(&planar, tolerance) {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} self-intersects"
        )));
    }
    Ok(())
}

fn profile_point_inside(point: [f64; 2], loop_points: &[[f64; 2]]) -> bool {
    use crate::geometry::poly2d::{point_in_ring2, Pt2};

    let ring = loop_points
        .iter()
        .map(|point| Pt2::new(point[0], point[1]))
        .collect::<Vec<_>>();
    point_in_ring2(Pt2::new(point[0], point[1]), &ring)
}

struct ExtrusionLoop {
    points: Vec<[f64; 2]>,
    bottom_vertices: Vec<u32>,
    top_vertices: Vec<u32>,
    bottom_edges: Vec<u32>,
    top_edges: Vec<u32>,
    vertical_edges: Vec<u32>,
}

pub fn linear_extrusion(
    id: String,
    frame: Frame3,
    outer: Vec<[f64; 2]>,
    holes: Vec<Vec<[f64; 2]>>,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[height])?;
    if height <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "extrusion height is below geometric resolution".into(),
        ));
    }
    validate_profile_loop(&outer, accuracy.geometric, "outer profile")?;
    for (index, hole) in holes.iter().enumerate() {
        validate_profile_loop(hole, accuracy.geometric, &format!("profile hole {index}"))?;
        if !profile_point_inside(hole[0], &outer) || loops_touch(&outer, hole, accuracy.geometric) {
            return Err(GeometryError::InvalidGeometry(format!(
                "profile hole {index} is not strictly inside the outer profile"
            )));
        }
    }
    for first in 0..holes.len() {
        for second in (first + 1)..holes.len() {
            if loops_touch(&holes[first], &holes[second], accuracy.geometric)
                || profile_point_inside(holes[first][0], &holes[second])
                || profile_point_inside(holes[second][0], &holes[first])
            {
                return Err(GeometryError::InvalidGeometry(format!(
                    "profile holes {first} and {second} overlap or nest"
                )));
            }
        }
    }

    let mut loops = Vec::with_capacity(holes.len() + 1);
    loops.push(orient_profile_loop(outer, true));
    loops.extend(
        holes
            .into_iter()
            .map(|hole| orient_profile_loop(hole, false)),
    );
    let mut builder = Builder::new(id, accuracy)?;
    let mut built = Vec::with_capacity(loops.len());
    for points in loops {
        let bottom_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], 0.0])))
            .collect::<Vec<_>>();
        let top_vertices = points
            .iter()
            .map(|point| builder.vertex(frame.point([point[0], point[1], height])))
            .collect::<Vec<_>>();
        let mut bottom_edges = Vec::with_capacity(points.len());
        let mut top_edges = Vec::with_capacity(points.len());
        let mut vertical_edges = Vec::with_capacity(points.len());
        for index in 0..points.len() {
            let next = (index + 1) % points.len();
            let direction = unit(sub(
                builder.brep.topology.vertices[bottom_vertices[next] as usize].position,
                builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
            ))?;
            let length = norm(sub(
                builder.brep.topology.vertices[bottom_vertices[next] as usize].position,
                builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
            ));
            bottom_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin:
                        builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
                    direction,
                },
                Interval::new(0.0, length)?,
                false,
            ));
            top_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin: builder.brep.topology.vertices[top_vertices[index] as usize].position,
                    direction,
                },
                Interval::new(0.0, length)?,
                false,
            ));
            vertical_edges.push(builder.edge(
                CurveGeometry::Line {
                    origin:
                        builder.brep.topology.vertices[bottom_vertices[index] as usize].position,
                    direction: frame.z,
                },
                Interval::new(0.0, height)?,
                false,
            ));
        }
        built.push(ExtrusionLoop {
            points,
            bottom_vertices,
            top_vertices,
            bottom_edges,
            top_edges,
            vertical_edges,
        });
    }

    let cap_bounds = |loops: &[ExtrusionLoop]| {
        let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        for point in loops.iter().flat_map(|profile| &profile.points) {
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(point[axis]);
                bounds[axis][1] = bounds[axis][1].max(point[axis]);
            }
        }
        for bound in &mut bounds {
            bound[0] -= accuracy.geometric;
            bound[1] += accuracy.geometric;
        }
        bounds
    };
    let top_bounds = cap_bounds(&built);
    let bottom_bounds = [top_bounds[0], [-top_bounds[1][1], -top_bounds[1][0]]];
    let mut top_frame = frame;
    top_frame.origin = frame.point([0.0, 0.0, height]);
    let bottom_frame = Frame3 {
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
        ..frame
    };
    let cap_uses = |builder: &Builder,
                    profile: &ExtrusionLoop,
                    top: bool|
     -> Result<Vec<Use>, GeometryError> {
        let count = profile.points.len();
        (0..count)
            .map(|offset| {
                let index = if top { offset } else { count - 1 - offset };
                let next = (index + 1) % count;
                if top {
                    plane_boundary(
                        builder,
                        top_frame,
                        profile.top_edges[index],
                        profile.top_vertices[index],
                        profile.top_vertices[next],
                        Orientation::Forward,
                    )
                } else {
                    plane_boundary(
                        builder,
                        bottom_frame,
                        profile.bottom_edges[index],
                        profile.bottom_vertices[next],
                        profile.bottom_vertices[index],
                        Orientation::Reverse,
                    )
                }
            })
            .collect()
    };
    let top_outer = cap_uses(&builder, &built[0], true)?;
    let top_holes = built[1..]
        .iter()
        .map(|profile| cap_uses(&builder, profile, true))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face_with_holes(
        "top",
        SurfaceGeometry::Plane { frame: top_frame },
        top_bounds,
        top_outer,
        top_holes,
    )?;
    let bottom_outer = cap_uses(&builder, &built[0], false)?;
    let bottom_holes = built[1..]
        .iter()
        .map(|profile| cap_uses(&builder, profile, false))
        .collect::<Result<Vec<_>, _>>()?;
    builder.face_with_holes(
        "bottom",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        bottom_bounds,
        bottom_outer,
        bottom_holes,
    )?;

    for (loop_index, profile) in built.iter().enumerate() {
        for index in 0..profile.points.len() {
            let next = (index + 1) % profile.points.len();
            let bottom_from = profile.bottom_vertices[index];
            let bottom_to = profile.bottom_vertices[next];
            let top_from = profile.top_vertices[index];
            let top_to = profile.top_vertices[next];
            let origin = builder.brep.topology.vertices[bottom_from as usize].position;
            let edge_direction = unit(sub(
                builder.brep.topology.vertices[bottom_to as usize].position,
                origin,
            ))?;
            let face_frame = Frame3::from_axis(
                origin,
                unit(cross(edge_direction, frame.z))?,
                edge_direction,
            )?;
            let uses = [
                (
                    profile.bottom_edges[index],
                    bottom_from,
                    bottom_to,
                    Orientation::Forward,
                ),
                (
                    profile.vertical_edges[next],
                    bottom_to,
                    top_to,
                    Orientation::Forward,
                ),
                (
                    profile.top_edges[index],
                    top_to,
                    top_from,
                    Orientation::Reverse,
                ),
                (
                    profile.vertical_edges[index],
                    top_from,
                    bottom_from,
                    Orientation::Reverse,
                ),
            ]
            .into_iter()
            .map(|(edge, from, to, sense)| {
                plane_boundary(&builder, face_frame, edge, from, to, sense)
            })
            .collect::<Result<Vec<_>, _>>()?;
            let edge_length = norm(sub(
                builder.brep.topology.vertices[bottom_to as usize].position,
                origin,
            ));
            let g = accuracy.geometric;
            builder.face(
                &format!("side-{loop_index}-{index}"),
                SurfaceGeometry::Plane { frame: face_frame },
                [[-g, edge_length + g], [-g, height + g]],
                uses,
            )?;
        }
    }
    builder.finish()
}

#[derive(Clone, Debug)]
pub struct StraightWallArchedOpening {
    pub id: String,
    pub station: f64,
    pub width: f64,
    pub bottom: f64,
    pub height: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn straight_wall_with_arched_opening(
    id: String,
    frame: Frame3,
    width: f64,
    depth: f64,
    height: f64,
    opening: StraightWallArchedOpening,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[width, depth, height, opening.width, opening.height])?;
    let radius = opening.width * 0.5;
    let spring = opening.bottom + opening.height - radius;
    if opening.id.is_empty()
        || !opening.station.is_finite()
        || !opening.bottom.is_finite()
        || spring <= opening.bottom + 4.0 * accuracy.geometric
        || opening.station - radius <= 4.0 * accuracy.geometric
        || opening.station + radius >= width - 4.0 * accuracy.geometric
        || opening.bottom < 0.0
        || opening.bottom + opening.height >= height - 4.0 * accuracy.geometric
    {
        return Err(GeometryError::InvalidGeometry(
            "arched wall opening must remain resolved and strictly inside the wall".into(),
        ));
    }
    let extrusion_frame = Frame3 {
        origin: frame.point([0.0, depth, 0.0]),
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let left = opening.station - radius;
    let right = opening.station + radius;
    let mut result = linear_extrusion(
        id,
        extrusion_frame,
        vec![[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]],
        vec![vec![
            [left, opening.bottom],
            [right, opening.bottom],
            [right, spring],
            [left, spring],
        ]],
        depth,
        accuracy,
    )?;
    let header = result
        .topology
        .faces
        .iter()
        .find(|face| face.key == "side-1-0")
        .map(|face| face.id)
        .ok_or_else(|| GeometryError::InvalidTopology("arched header face is missing".into()))?;
    let header_loop = result.topology.faces[header as usize].trim.outer;
    let header_uses = loop_halfedges_for_primitive(&result, header_loop)?;
    let mut arch_edges = Vec::new();
    for halfedge in &header_uses {
        let use_ = &result.topology.halfedges[*halfedge as usize];
        let endpoints = [use_.from, use_.to].map(|vertex| {
            extrusion_frame.local(result.topology.vertices[vertex as usize].position)
        });
        if endpoints
            .iter()
            .all(|point| (point[1] - spring).abs() <= 4.0 * accuracy.geometric)
            && (endpoints[0][2] - endpoints[1][2]).abs() <= 4.0 * accuracy.geometric
        {
            arch_edges.push(use_.edge);
        }
    }
    arch_edges.sort_unstable();
    arch_edges.dedup();
    if arch_edges.len() != 2 {
        return Err(GeometryError::InvalidTopology(
            "arched opening requires two shared spring edges".into(),
        ));
    }
    for edge_id in &arch_edges {
        let edge = &result.topology.edges[*edge_id as usize];
        let primary = &result.topology.halfedges[edge.halfedge as usize];
        let parameter_start = if primary.geometry_use.sense == Orientation::Forward {
            primary.from
        } else {
            primary.to
        };
        let start = result.topology.vertices[parameter_start as usize].position;
        let center = add(
            extrusion_frame.origin,
            add(
                scale(extrusion_frame.x, opening.station),
                add(
                    scale(extrusion_frame.y, spring),
                    scale(
                        extrusion_frame.z,
                        dot(sub(start, extrusion_frame.origin), extrusion_frame.z),
                    ),
                ),
            ),
        );
        let x = unit(sub(start, center))?;
        let circle_frame = Frame3 {
            origin: center,
            x,
            y: extrusion_frame.y,
            z: unit(cross(x, extrusion_frame.y))?,
        };
        circle_frame.validate()?;
        let curve = result.geometry.curves.len() as u32;
        result.geometry.curves.push(CurveGeometry::Circle {
            frame: circle_frame,
            radius,
        });
        result.topology.edges[*edge_id as usize].geometry = EdgeGeometry::Curve {
            curve,
            range: Interval::new(0.0, std::f64::consts::PI)?,
        };
    }
    let cylinder_frame = Frame3 {
        origin: extrusion_frame.point([opening.station, spring, 0.0]),
        x: scale(extrusion_frame.x, -1.0),
        y: extrusion_frame.y,
        z: scale(extrusion_frame.z, -1.0),
    };
    cylinder_frame.validate()?;
    let cylinder_surface = result.geometry.surfaces.len() as u32;
    result.geometry.surfaces.push(SurfaceGeometry::Cylinder {
        frame: cylinder_frame,
        radius,
    });
    {
        let face = &mut result.topology.faces[header as usize];
        face.key = format!("{}-arched-header", opening.id);
        face.surface = cylinder_surface;
        face.sense = Orientation::Reverse;
        face.trim.uv_bounds = [
            Interval::new(0.0, std::f64::consts::PI)?,
            Interval::new(-depth, 0.0)?,
        ];
        face.provenance.sources[0].key = face.key.clone();
        face.provenance.role = FaceRole::Cut;
        face.provenance.reversed = true;
    }
    let affected = result
        .topology
        .halfedges
        .iter()
        .filter(|halfedge| halfedge.face == Some(header) || arch_edges.contains(&halfedge.edge))
        .map(|halfedge| halfedge.id)
        .collect::<Vec<_>>();
    for halfedge_id in affected {
        let halfedge = &result.topology.halfedges[halfedge_id as usize];
        let face = halfedge.face.ok_or_else(|| {
            GeometryError::InvalidTopology("arched opening edge use has no face".into())
        })?;
        let surface = result.topology.faces[face as usize].surface;
        let (curve, range) = match result.topology.edges[halfedge.edge as usize].geometry {
            EdgeGeometry::Curve { curve, range } => (curve, range),
            EdgeGeometry::Collapsed { .. } => {
                return Err(GeometryError::InvalidTopology(
                    "arched opening contains a collapsed boundary".into(),
                ))
            }
        };
        let start = result.geometry.curve(curve)?.point_at(range.lo)?;
        let end = result.geometry.curve(curve)?.point_at(range.hi)?;
        let surface_geometry = result.geometry.surface(surface)?;
        let mut uv_hint = surface_geometry.project(start, None)?;
        let mut uv_end = surface_geometry.project(end, Some(uv_hint))?;
        if surface == cylinder_surface {
            for uv in [&mut uv_hint, &mut uv_end] {
                if uv[0] < -accuracy.intersection {
                    uv[0] += std::f64::consts::TAU;
                }
            }
        }
        let uv_rate = if range.width() > accuracy.geometric {
            std::array::from_fn(|axis| (uv_end[axis] - uv_hint[axis]) / range.width())
        } else {
            [0.0; 2]
        };
        let pcurve = result.geometry.pcurves.len() as u32;
        result
            .geometry
            .pcurves
            .push(PcurveGeometry::ProjectedCurve {
                curve,
                surface,
                chart: 0,
                uv_hint,
                uv_rate,
                parameter_origin: range.lo,
            });
        result.topology.halfedges[halfedge_id as usize]
            .geometry_use
            .pcurve = Some(pcurve);
    }
    result.validate()?;
    Ok(result)
}

pub fn cone(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    dimensions(&[radius, height])?;
    let semi_angle = radius.atan2(height);
    let frame = Frame3 {
        origin: frame.point([0.0, 0.0, height]),
        x: frame.x,
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
    };
    section(
        id,
        frame,
        SurfaceGeometry::Cone { frame, semi_angle },
        0.0,
        radius,
        0.0,
        height,
        accuracy,
    )
}

pub fn frustum(
    id: String,
    frame: Frame3,
    lower_radius: f64,
    upper_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&[lower_radius, height])?;
    if !upper_radius.is_finite() || upper_radius < 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "invalid frustum upper radius".into(),
        ));
    }
    if lower_radius == upper_radius {
        return cylinder(id, frame, lower_radius, height, accuracy);
    }
    let k = (upper_radius - lower_radius).abs() / height;
    let semi_angle = k.atan();
    let mut cone_frame = frame;
    let (r0, r1, z0, z1) = if upper_radius > lower_radius {
        cone_frame.origin = frame.point([0.0, 0.0, -lower_radius / k]);
        (
            lower_radius,
            upper_radius,
            lower_radius / k,
            upper_radius / k,
        )
    } else {
        cone_frame.origin = frame.point([0.0, 0.0, lower_radius / k]);
        cone_frame.y = scale(frame.y, -1.0);
        cone_frame.z = scale(frame.z, -1.0);
        (
            upper_radius,
            lower_radius,
            upper_radius / k,
            lower_radius / k,
        )
    };
    section(
        id,
        cone_frame,
        SurfaceGeometry::Cone {
            frame: cone_frame,
            semi_angle,
        },
        r0,
        r1,
        z0,
        z1,
        accuracy,
    )
}

pub(super) fn plane_boundary(
    builder: &Builder,
    frame: Frame3,
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
) -> Result<Use, GeometryError> {
    let curve = match builder.brep.topology.edges[edge as usize].geometry {
        EdgeGeometry::Curve { curve, .. } => &builder.brep.geometry.curves[curve as usize],
        _ => {
            return Err(GeometryError::InvalidGeometry(
                "collapsed planar boundary".into(),
            ))
        }
    };
    let local = |p: Point3| [dot(p, frame.x), dot(p, frame.y)];
    let pcurve = match curve {
        CurveGeometry::Line { origin, direction } => {
            uv_line(local(sub(*origin, frame.origin)), local(*direction))
        }
        CurveGeometry::Circle { frame: c, radius } => PcurveGeometry::Conic2 {
            origin: local(sub(c.origin, frame.origin)),
            axis_a: local(scale(c.x, *radius)),
            axis_b: local(scale(c.y, *radius)),
        },
        _ => {
            return Err(GeometryError::UnsupportedGeometry(
                "planar primitive boundary".into(),
            ))
        }
    };
    Ok(boundary(edge, from, to, sense, pcurve))
}

#[derive(Clone, Debug)]
pub struct CircularWallOpening {
    pub id: String,
    pub angle: f64,
    pub width: f64,
    pub bottom: f64,
    pub height: f64,
}

struct BuiltCircularWallOpening {
    source: String,
    outer_angles: [f64; 2],
    inner_angles: [f64; 2],
    elevations: [f64; 2],
    vertices: [u32; 8],
    lower_outer_arc: Option<u32>,
    upper_outer_arc: u32,
    lower_inner_arc: Option<u32>,
    upper_inner_arc: u32,
    outer_verticals: [u32; 2],
    inner_verticals: [u32; 2],
    connectors: [[u32; 2]; 2],
}

impl BuiltCircularWallOpening {
    fn touches_bottom(&self) -> bool {
        self.elevations[0] == 0.0
    }
}

pub fn circular_wall(
    id: String,
    frame: Frame3,
    radius: f64,
    thickness: f64,
    height: f64,
    start_angle: f64,
    sweep_angle: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    circular_wall_with_openings(
        id,
        frame,
        radius,
        thickness,
        height,
        start_angle,
        sweep_angle,
        Vec::new(),
        accuracy,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn circular_wall_with_openings(
    id: String,
    frame: Frame3,
    radius: f64,
    thickness: f64,
    height: f64,
    start_angle: f64,
    sweep_angle: f64,
    openings: Vec<CircularWallOpening>,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    dimensions(&[radius, thickness, height])?;
    let inner = radius - thickness * 0.5;
    let outer = radius + thickness * 0.5;
    if inner <= 0.0 || !outer.is_finite() {
        return Err(GeometryError::InvalidGeometry(
            "wall radius must exceed half thickness".into(),
        ));
    }
    if !start_angle.is_finite() || !sweep_angle.is_finite() || sweep_angle == 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "wall angles must be finite with a nonzero sweep".into(),
        ));
    }
    if sweep_angle.abs() >= std::f64::consts::TAU {
        return Err(GeometryError::UnsupportedGeometry(
            "circular-wall sweep must be smaller than one full turn".into(),
        ));
    }
    let end_angle = start_angle + sweep_angle;
    let lo = start_angle.min(end_angle);
    let hi = start_angle.max(end_angle);
    let range = Interval::new(lo, hi)?;
    if range.width() == 0.0
        || inner * (hi - lo) <= 4.0 * accuracy.geometric
        || thickness <= 4.0 * accuracy.geometric
        || height <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::InvalidGeometry(
            "wall features are below geometric resolution".into(),
        ));
    }
    let mut prepared_openings = Vec::with_capacity(openings.len());
    let mut opening_ids = std::collections::HashSet::new();
    for opening in openings {
        if opening.id.is_empty() || !opening_ids.insert(opening.id.clone()) {
            return Err(GeometryError::InvalidGeometry(
                "circular-wall opening identities must be nonempty and unique".into(),
            ));
        }
        if !opening.angle.is_finite()
            || !opening.width.is_finite()
            || !opening.bottom.is_finite()
            || !opening.height.is_finite()
            || opening.width <= 4.0 * accuracy.geometric
            || opening.height <= 4.0 * accuracy.geometric
        {
            return Err(GeometryError::InvalidGeometry(
                "circular-wall opening dimensions and station must be finite and resolved".into(),
            ));
        }
        let half_width = opening.width * 0.5;
        if half_width >= inner {
            return Err(GeometryError::UnsupportedGeometry(
                "circular-wall opening width reaches the wall axis".into(),
            ));
        }
        let outer_delta = (half_width / outer).asin();
        let inner_delta = (half_width / inner).asin();
        let outer_angles = [opening.angle - outer_delta, opening.angle + outer_delta];
        let inner_angles = [opening.angle - inner_delta, opening.angle + inner_delta];
        let mut elevations = [opening.bottom, opening.bottom + opening.height];
        let clearance = 4.0 * accuracy.geometric;
        if inner * (inner_angles[0] - lo) <= clearance
            || inner * (hi - inner_angles[1]) <= clearance
        {
            return Err(GeometryError::UnsupportedGeometry(
                "circular-wall opening must stay strictly inside both authored wall ends".into(),
            ));
        }
        if elevations[0] < 0.0 || elevations[1] >= height {
            return Err(GeometryError::InvalidGeometry(
                "circular-wall opening elevation lies outside the wall".into(),
            ));
        }
        if elevations[0] > 0.0 && elevations[0] <= clearance {
            return Err(GeometryError::UnresolvedIntersection(
                "circular-wall opening sill is below geometric resolution".into(),
            ));
        }
        if height - elevations[1] <= clearance {
            return Err(GeometryError::UnresolvedIntersection(
                "circular-wall opening head clearance is below geometric resolution".into(),
            ));
        }
        if elevations[0] == -0.0 {
            elevations[0] = 0.0;
        }
        prepared_openings.push((opening.id, outer_angles, inner_angles, elevations));
    }
    for first in 0..prepared_openings.len() {
        for second in (first + 1)..prepared_openings.len() {
            let a = &prepared_openings[first];
            let b = &prepared_openings[second];
            let angular_overlap = a.2[1].min(b.2[1]) - a.2[0].max(b.2[0]);
            let vertical_overlap = a.3[1].min(b.3[1]) - a.3[0].max(b.3[0]);
            if angular_overlap * inner > -4.0 * accuracy.geometric
                && vertical_overlap > -4.0 * accuracy.geometric
            {
                return Err(GeometryError::UnsupportedGeometry(
                    "circular-wall openings overlap or are below geometric separation".into(),
                ));
            }
        }
    }
    let mut b = Builder::new(id, accuracy)?;
    let angles = [lo, hi];
    let point = |r: f64, a: f64, z: f64| frame.point([r * a.cos(), r * a.sin(), z]);
    let mut vertices = [0u32; 8];
    for (i, vertex) in vertices.iter_mut().enumerate() {
        *vertex = b.vertex(point(
            if i % 4 < 2 { outer } else { inner },
            angles[i % 2],
            if i < 4 { 0.0 } else { height },
        ));
    }
    let [ob0, ob1, ib0, ib1, ot0, ot1, it0, it1] = vertices;
    let mut top_frame = frame;
    top_frame.origin = frame.point([0.0, 0.0, height]);
    let has_bottom_openings = prepared_openings.iter().any(|opening| opening.3[0] == 0.0);
    let ob = (!has_bottom_openings).then(|| {
        b.edge(
            CurveGeometry::Circle {
                frame,
                radius: outer,
            },
            range,
            false,
        )
    });
    let ot = b.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius: outer,
        },
        range,
        false,
    );
    let ib = (!has_bottom_openings).then(|| {
        b.edge(
            CurveGeometry::Circle {
                frame,
                radius: inner,
            },
            range,
            false,
        )
    });
    let it = b.edge(
        CurveGeometry::Circle {
            frame: top_frame,
            radius: inner,
        },
        range,
        false,
    );
    let mut radial = [0u32; 4];
    for (i, edge) in radial.iter_mut().enumerate() {
        let angle = angles[i % 2];
        *edge = b.edge(
            CurveGeometry::Line {
                origin: point(inner, angle, if i < 2 { 0.0 } else { height }),
                direction: frame.vector([angle.cos(), angle.sin(), 0.0]),
            },
            Interval::new(0.0, thickness)?,
            false,
        );
    }
    let [rb0, rb1, rt0, rt1] = radial;
    let mut vertical = [0u32; 4];
    for (i, edge) in vertical.iter_mut().enumerate() {
        *edge = b.edge(
            CurveGeometry::Line {
                origin: b.brep.topology.vertices[vertices[i] as usize].position,
                direction: frame.z,
            },
            Interval::new(0.0, height)?,
            false,
        );
    }
    let [vo0, vo1, vi0, vi1] = vertical;
    let mut built_openings = Vec::with_capacity(prepared_openings.len());
    for (source, outer_angles, inner_angles, elevations) in prepared_openings {
        let positions = [
            point(outer, outer_angles[0], elevations[0]),
            point(outer, outer_angles[1], elevations[0]),
            point(inner, inner_angles[0], elevations[0]),
            point(inner, inner_angles[1], elevations[0]),
            point(outer, outer_angles[0], elevations[1]),
            point(outer, outer_angles[1], elevations[1]),
            point(inner, inner_angles[0], elevations[1]),
            point(inner, inner_angles[1], elevations[1]),
        ];
        let vertices = positions.map(|position| b.vertex(position));
        let mut lower_frame = frame;
        lower_frame.origin = frame.point([0.0, 0.0, elevations[0]]);
        let mut upper_frame = frame;
        upper_frame.origin = frame.point([0.0, 0.0, elevations[1]]);
        let outer_range = Interval::new(outer_angles[0], outer_angles[1])?;
        let inner_range = Interval::new(inner_angles[0], inner_angles[1])?;
        let lower_outer_arc = (elevations[0] > 0.0).then(|| {
            b.edge(
                CurveGeometry::Circle {
                    frame: lower_frame,
                    radius: outer,
                },
                outer_range,
                false,
            )
        });
        let upper_outer_arc = b.edge(
            CurveGeometry::Circle {
                frame: upper_frame,
                radius: outer,
            },
            outer_range,
            false,
        );
        let lower_inner_arc = (elevations[0] > 0.0).then(|| {
            b.edge(
                CurveGeometry::Circle {
                    frame: lower_frame,
                    radius: inner,
                },
                inner_range,
                false,
            )
        });
        let upper_inner_arc = b.edge(
            CurveGeometry::Circle {
                frame: upper_frame,
                radius: inner,
            },
            inner_range,
            false,
        );
        let opening_height = elevations[1] - elevations[0];
        let outer_verticals = [
            b.edge(
                CurveGeometry::Line {
                    origin: positions[0],
                    direction: frame.z,
                },
                Interval::new(0.0, opening_height)?,
                false,
            ),
            b.edge(
                CurveGeometry::Line {
                    origin: positions[1],
                    direction: frame.z,
                },
                Interval::new(0.0, opening_height)?,
                false,
            ),
        ];
        let inner_verticals = [
            b.edge(
                CurveGeometry::Line {
                    origin: positions[2],
                    direction: frame.z,
                },
                Interval::new(0.0, opening_height)?,
                false,
            ),
            b.edge(
                CurveGeometry::Line {
                    origin: positions[3],
                    direction: frame.z,
                },
                Interval::new(0.0, opening_height)?,
                false,
            ),
        ];
        let mut connectors = [[0_u32; 2]; 2];
        for elevation in 0..2 {
            for side in 0..2 {
                let outer_index = elevation * 4 + side;
                let inner_index = elevation * 4 + 2 + side;
                let delta = sub(positions[outer_index], positions[inner_index]);
                connectors[elevation][side] = b.edge(
                    CurveGeometry::Line {
                        origin: positions[inner_index],
                        direction: unit(delta)?,
                    },
                    Interval::new(0.0, norm(delta))?,
                    false,
                );
            }
        }
        built_openings.push(BuiltCircularWallOpening {
            source,
            outer_angles,
            inner_angles,
            elevations,
            vertices,
            lower_outer_arc,
            upper_outer_arc,
            lower_inner_arc,
            upper_inner_arc,
            outer_verticals,
            inner_verticals,
            connectors,
        });
    }
    use Orientation::{Forward as F, Reverse as R};
    struct BottomSpan {
        outer_edge: u32,
        inner_edge: u32,
        outer_vertices: [u32; 2],
        inner_vertices: [u32; 2],
        low_connector: u32,
        high_connector: u32,
    }
    let mut bottom_openings = built_openings
        .iter()
        .enumerate()
        .filter(|(_, opening)| opening.touches_bottom())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    bottom_openings.sort_by(|&a, &b| {
        built_openings[a].outer_angles[0].total_cmp(&built_openings[b].outer_angles[0])
    });
    let mut bottom_spans = Vec::with_capacity(bottom_openings.len() + 1);
    if bottom_openings.is_empty() {
        bottom_spans.push(BottomSpan {
            outer_edge: ob.ok_or_else(|| {
                GeometryError::InvalidTopology("circular-wall bottom edge missing".into())
            })?,
            inner_edge: ib.ok_or_else(|| {
                GeometryError::InvalidTopology("circular-wall bottom edge missing".into())
            })?,
            outer_vertices: [ob0, ob1],
            inner_vertices: [ib0, ib1],
            low_connector: rb0,
            high_connector: rb1,
        });
    } else {
        for span_index in 0..=bottom_openings.len() {
            let low = span_index
                .checked_sub(1)
                .map(|index| &built_openings[bottom_openings[index]]);
            let high = bottom_openings
                .get(span_index)
                .map(|&index| &built_openings[index]);
            let outer_angles = [
                low.map_or(lo, |opening| opening.outer_angles[1]),
                high.map_or(hi, |opening| opening.outer_angles[0]),
            ];
            let inner_angles = [
                low.map_or(lo, |opening| opening.inner_angles[1]),
                high.map_or(hi, |opening| opening.inner_angles[0]),
            ];
            let outer_vertices = [
                low.map_or(ob0, |opening| opening.vertices[1]),
                high.map_or(ob1, |opening| opening.vertices[0]),
            ];
            let inner_vertices = [
                low.map_or(ib0, |opening| opening.vertices[3]),
                high.map_or(ib1, |opening| opening.vertices[2]),
            ];
            bottom_spans.push(BottomSpan {
                outer_edge: b.edge(
                    CurveGeometry::Circle {
                        frame,
                        radius: outer,
                    },
                    Interval::new(outer_angles[0], outer_angles[1])?,
                    false,
                ),
                inner_edge: b.edge(
                    CurveGeometry::Circle {
                        frame,
                        radius: inner,
                    },
                    Interval::new(inner_angles[0], inner_angles[1])?,
                    false,
                ),
                outer_vertices,
                inner_vertices,
                low_connector: low.map_or(rb0, |opening| opening.connectors[0][1]),
                high_connector: high.map_or(rb1, |opening| opening.connectors[0][0]),
            });
        }
    }
    let outer_holes = built_openings
        .iter()
        .filter(|opening| !opening.touches_bottom())
        .map(|opening| {
            let [obl, obr, _, _, otl, otr, _, _] = opening.vertices;
            Ok(vec![
                boundary(
                    opening.lower_outer_arc.ok_or_else(|| {
                        GeometryError::InvalidTopology("opening sill edge missing".into())
                    })?,
                    obr,
                    obl,
                    R,
                    uv_line([0.0, opening.elevations[0]], [1.0, 0.0]),
                ),
                boundary(
                    opening.outer_verticals[0],
                    obl,
                    otl,
                    F,
                    uv_line([opening.outer_angles[0], opening.elevations[0]], [0.0, 1.0]),
                ),
                boundary(
                    opening.upper_outer_arc,
                    otl,
                    otr,
                    F,
                    uv_line([0.0, opening.elevations[1]], [1.0, 0.0]),
                ),
                boundary(
                    opening.outer_verticals[1],
                    otr,
                    obr,
                    R,
                    uv_line([opening.outer_angles[1], opening.elevations[0]], [0.0, 1.0]),
                ),
            ])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut outer_boundary = Vec::new();
    for (span_index, span) in bottom_spans.iter().enumerate() {
        outer_boundary.push(boundary(
            span.outer_edge,
            span.outer_vertices[0],
            span.outer_vertices[1],
            F,
            uv_line([0.0, 0.0], [1.0, 0.0]),
        ));
        if let Some(&opening_index) = bottom_openings.get(span_index) {
            let opening = &built_openings[opening_index];
            let [obl, obr, _, _, otl, otr, _, _] = opening.vertices;
            outer_boundary.extend([
                boundary(
                    opening.outer_verticals[0],
                    obl,
                    otl,
                    F,
                    uv_line([opening.outer_angles[0], 0.0], [0.0, 1.0]),
                ),
                boundary(
                    opening.upper_outer_arc,
                    otl,
                    otr,
                    F,
                    uv_line([0.0, opening.elevations[1]], [1.0, 0.0]),
                ),
                boundary(
                    opening.outer_verticals[1],
                    otr,
                    obr,
                    R,
                    uv_line([opening.outer_angles[1], 0.0], [0.0, 1.0]),
                ),
            ]);
        }
    }
    outer_boundary.extend([
        boundary(vo1, ob1, ot1, F, uv_line([hi, 0.0], [0.0, 1.0])),
        boundary(ot, ot1, ot0, R, uv_line([0.0, height], [1.0, 0.0])),
        boundary(vo0, ot0, ob0, R, uv_line([lo, 0.0], [0.0, 1.0])),
    ]);
    b.face_with_holes(
        "outer",
        SurfaceGeometry::Cylinder {
            frame,
            radius: outer,
        },
        [[lo, hi], [0.0, height]],
        outer_boundary,
        outer_holes,
    )?;
    let inner_holes = built_openings
        .iter()
        .filter(|opening| !opening.touches_bottom())
        .map(|opening| {
            let [_, _, ibl, ibr, _, _, itl, itr] = opening.vertices;
            Ok(vec![
                boundary(
                    opening.lower_inner_arc.ok_or_else(|| {
                        GeometryError::InvalidTopology("opening sill edge missing".into())
                    })?,
                    ibl,
                    ibr,
                    F,
                    uv_line([0.0, opening.elevations[0]], [1.0, 0.0]),
                ),
                boundary(
                    opening.inner_verticals[1],
                    ibr,
                    itr,
                    F,
                    uv_line([opening.inner_angles[1], opening.elevations[0]], [0.0, 1.0]),
                ),
                boundary(
                    opening.upper_inner_arc,
                    itr,
                    itl,
                    R,
                    uv_line([0.0, opening.elevations[1]], [1.0, 0.0]),
                ),
                boundary(
                    opening.inner_verticals[0],
                    itl,
                    ibl,
                    R,
                    uv_line([opening.inner_angles[0], opening.elevations[0]], [0.0, 1.0]),
                ),
            ])
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut inner_boundary = Vec::new();
    for span_index in (0..bottom_spans.len()).rev() {
        let span = &bottom_spans[span_index];
        inner_boundary.push(boundary(
            span.inner_edge,
            span.inner_vertices[1],
            span.inner_vertices[0],
            R,
            uv_line([0.0, 0.0], [1.0, 0.0]),
        ));
        if span_index > 0 {
            let opening = &built_openings[bottom_openings[span_index - 1]];
            let [_, _, ibl, ibr, _, _, itl, itr] = opening.vertices;
            inner_boundary.extend([
                boundary(
                    opening.inner_verticals[1],
                    ibr,
                    itr,
                    F,
                    uv_line([opening.inner_angles[1], 0.0], [0.0, 1.0]),
                ),
                boundary(
                    opening.upper_inner_arc,
                    itr,
                    itl,
                    R,
                    uv_line([0.0, opening.elevations[1]], [1.0, 0.0]),
                ),
                boundary(
                    opening.inner_verticals[0],
                    itl,
                    ibl,
                    R,
                    uv_line([opening.inner_angles[0], 0.0], [0.0, 1.0]),
                ),
            ]);
        }
    }
    inner_boundary.extend([
        boundary(vi0, ib0, it0, F, uv_line([lo, 0.0], [0.0, 1.0])),
        boundary(it, it0, it1, F, uv_line([0.0, height], [1.0, 0.0])),
        boundary(vi1, it1, ib1, R, uv_line([hi, 0.0], [0.0, 1.0])),
    ]);
    b.face_with_holes(
        "inner",
        SurfaceGeometry::Cylinder {
            frame,
            radius: inner,
        },
        [[lo, hi], [0.0, height]],
        inner_boundary,
        inner_holes,
    )?;
    b.brep.topology.faces[1].sense = R;
    let bottom_frame = Frame3 {
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
        ..frame
    };
    for (span_index, span) in bottom_spans.iter().enumerate() {
        let bottom_uses = [
            (
                span.outer_edge,
                span.outer_vertices[1],
                span.outer_vertices[0],
                R,
            ),
            (
                span.low_connector,
                span.outer_vertices[0],
                span.inner_vertices[0],
                R,
            ),
            (
                span.inner_edge,
                span.inner_vertices[0],
                span.inner_vertices[1],
                F,
            ),
            (
                span.high_connector,
                span.inner_vertices[1],
                span.outer_vertices[1],
                F,
            ),
        ]
        .into_iter()
        .map(|(edge, from, to, sense)| plane_boundary(&b, bottom_frame, edge, from, to, sense))
        .collect::<Result<Vec<_>, _>>()?;
        let key = if bottom_spans.len() == 1 {
            "bottom".into()
        } else {
            format!("bottom-{span_index}")
        };
        b.face(
            &key,
            SurfaceGeometry::Plane {
                frame: bottom_frame,
            },
            [[-outer - accuracy.geometric, outer + accuracy.geometric]; 2],
            bottom_uses,
        )?;
    }
    let top_uses = [
        (ot, ot0, ot1, F),
        (rt1, ot1, it1, R),
        (it, it1, it0, R),
        (rt0, it0, ot0, F),
    ]
    .into_iter()
    .map(|(e, from, to, sense)| plane_boundary(&b, top_frame, e, from, to, sense))
    .collect::<Result<Vec<_>, _>>()?;
    b.face(
        "top",
        SurfaceGeometry::Plane { frame: top_frame },
        [[-outer - accuracy.geometric, outer + accuracy.geometric]; 2],
        top_uses,
    )?;
    for i in 0..2 {
        let angle = angles[i];
        let radial_direction = frame.vector([angle.cos(), angle.sin(), 0.0]);
        let normal = frame.vector([angle.sin(), -angle.cos(), 0.0]);
        let end_frame = Frame3::from_axis(
            frame.origin,
            scale(normal, if i == 0 { 1.0 } else { -1.0 }),
            radial_direction,
        )?;
        let pairs = if i == 0 {
            [
                (rb0, ib0, ob0, F),
                (vo0, ob0, ot0, F),
                (rt0, ot0, it0, R),
                (vi0, it0, ib0, R),
            ]
        } else {
            [
                (rb1, ob1, ib1, R),
                (vi1, ib1, it1, F),
                (rt1, it1, ot1, F),
                (vo1, ot1, ob1, R),
            ]
        };
        let uses = pairs
            .into_iter()
            .map(|(e, from, to, sense)| plane_boundary(&b, end_frame, e, from, to, sense))
            .collect::<Result<Vec<_>, _>>()?;
        let key = if (i == 0) == (sweep_angle > 0.0) {
            "axis-start"
        } else {
            "axis-end"
        };
        let g = accuracy.geometric;
        b.face(
            key,
            SurfaceGeometry::Plane { frame: end_frame },
            [
                [inner - g, outer + g],
                if i == 0 {
                    [-g, height + g]
                } else {
                    [-height - g, g]
                },
            ],
            uses,
        )?;
    }
    for (opening_index, opening) in built_openings.iter().enumerate() {
        let [obl, obr, ibl, ibr, otl, otr, itl, itr] = opening.vertices;
        let plane_bounds = |builder: &Builder, plane: Frame3, vertices: [u32; 4]| {
            let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
            for vertex in vertices {
                let local = plane.local(builder.brep.topology.vertices[vertex as usize].position);
                for axis in 0..2 {
                    bounds[axis][0] = bounds[axis][0].min(local[axis]);
                    bounds[axis][1] = bounds[axis][1].max(local[axis]);
                }
            }
            for bound in &mut bounds {
                bound[0] -= accuracy.geometric;
                bound[1] += accuracy.geometric;
            }
            bounds
        };
        let curved_plane_bounds = |builder: &Builder,
                                   plane: Frame3,
                                   edges: [u32; 4]|
         -> Result<[[f64; 2]; 2], GeometryError> {
            let mut bounds = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
            for edge in edges {
                let edge = &builder.brep.topology.edges[edge as usize];
                let edge_bounds = match edge.geometry {
                    EdgeGeometry::Curve { curve, range } => {
                        builder.brep.geometry.curve(curve)?.enclose(range)?
                    }
                    EdgeGeometry::Collapsed { vertex } => {
                        let point = builder.brep.topology.vertices[vertex as usize].position;
                        super::geometry::PatchBounds {
                            axes: [
                                Interval::point(point[0])?,
                                Interval::point(point[1])?,
                                Interval::point(point[2])?,
                            ],
                        }
                    }
                };
                for corner in 0..8 {
                    let point = std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            edge_bounds.axes[axis].lo
                        } else {
                            edge_bounds.axes[axis].hi
                        }
                    });
                    let local = plane.local(point);
                    for axis in 0..2 {
                        bounds[axis][0] = bounds[axis][0].min(local[axis]);
                        bounds[axis][1] = bounds[axis][1].max(local[axis]);
                    }
                }
            }
            for bound in &mut bounds {
                bound[0] -= accuracy.geometric;
                bound[1] += accuracy.geometric;
            }
            Ok(bounds)
        };
        let mark_cut = |builder: &mut Builder, face: u32, local_face: u32, key: String| {
            builder.brep.topology.faces[face as usize].provenance = FaceProvenance {
                sources: vec![FaceSource {
                    entity: opening.source.clone(),
                    body: opening.source.clone(),
                    key,
                    face: local_face,
                }],
                role: FaceRole::Cut,
                reversed: true,
            };
        };

        if !opening.touches_bottom() {
            let lower_outer_arc = opening.lower_outer_arc.ok_or_else(|| {
                GeometryError::InvalidTopology("opening sill edge missing".into())
            })?;
            let lower_inner_arc = opening.lower_inner_arc.ok_or_else(|| {
                GeometryError::InvalidTopology("opening sill edge missing".into())
            })?;
            let mut sill_frame = frame;
            sill_frame.origin = frame.point([0.0, 0.0, opening.elevations[0]]);
            let sill_uses = [
                (lower_outer_arc, obl, obr, F),
                (opening.connectors[0][1], obr, ibr, R),
                (lower_inner_arc, ibr, ibl, R),
                (opening.connectors[0][0], ibl, obl, F),
            ]
            .into_iter()
            .map(|(edge, from, to, sense)| plane_boundary(&b, sill_frame, edge, from, to, sense))
            .collect::<Result<Vec<_>, _>>()?;
            let face = b.brep.topology.faces.len() as u32;
            b.face(
                &format!("opening-{opening_index}-sill"),
                SurfaceGeometry::Plane { frame: sill_frame },
                curved_plane_bounds(
                    &b,
                    sill_frame,
                    [
                        lower_outer_arc,
                        opening.connectors[0][1],
                        lower_inner_arc,
                        opening.connectors[0][0],
                    ],
                )?,
                sill_uses,
            )?;
            mark_cut(&mut b, face, 0, "sill".into());
        }

        let header_frame = Frame3 {
            origin: frame.point([0.0, 0.0, opening.elevations[1]]),
            y: scale(frame.y, -1.0),
            z: scale(frame.z, -1.0),
            ..frame
        };
        let header_uses = [
            (opening.upper_outer_arc, otr, otl, R),
            (opening.connectors[1][0], otl, itl, R),
            (opening.upper_inner_arc, itl, itr, F),
            (opening.connectors[1][1], itr, otr, F),
        ]
        .into_iter()
        .map(|(edge, from, to, sense)| plane_boundary(&b, header_frame, edge, from, to, sense))
        .collect::<Result<Vec<_>, _>>()?;
        let face = b.brep.topology.faces.len() as u32;
        b.face(
            &format!("opening-{opening_index}-header"),
            SurfaceGeometry::Plane {
                frame: header_frame,
            },
            curved_plane_bounds(
                &b,
                header_frame,
                [
                    opening.upper_outer_arc,
                    opening.connectors[1][0],
                    opening.upper_inner_arc,
                    opening.connectors[1][1],
                ],
            )?,
            header_uses,
        )?;
        mark_cut(&mut b, face, 1, "header".into());

        let center_tangent = frame.vector([
            -((opening.inner_angles[0] + opening.inner_angles[1]) * 0.5).sin(),
            ((opening.inner_angles[0] + opening.inner_angles[1]) * 0.5).cos(),
            0.0,
        ]);
        let left_frame = Frame3::from_axis(
            b.brep.topology.vertices[ibl as usize].position,
            center_tangent,
            sub(
                b.brep.topology.vertices[obl as usize].position,
                b.brep.topology.vertices[ibl as usize].position,
            ),
        )?;
        let left_uses = [
            (opening.connectors[0][0], obl, ibl, R),
            (opening.inner_verticals[0], ibl, itl, F),
            (opening.connectors[1][0], itl, otl, F),
            (opening.outer_verticals[0], otl, obl, R),
        ]
        .into_iter()
        .map(|(edge, from, to, sense)| plane_boundary(&b, left_frame, edge, from, to, sense))
        .collect::<Result<Vec<_>, _>>()?;
        let face = b.brep.topology.faces.len() as u32;
        b.face(
            &format!("opening-{opening_index}-jamb-lower"),
            SurfaceGeometry::Plane { frame: left_frame },
            plane_bounds(&b, left_frame, [obl, ibl, itl, otl]),
            left_uses,
        )?;
        mark_cut(&mut b, face, 2, "jamb-lower".into());

        let right_frame = Frame3::from_axis(
            b.brep.topology.vertices[ibr as usize].position,
            scale(center_tangent, -1.0),
            sub(
                b.brep.topology.vertices[obr as usize].position,
                b.brep.topology.vertices[ibr as usize].position,
            ),
        )?;
        let right_uses = [
            (opening.connectors[0][1], ibr, obr, F),
            (opening.outer_verticals[1], obr, otr, F),
            (opening.connectors[1][1], otr, itr, R),
            (opening.inner_verticals[1], itr, ibr, R),
        ]
        .into_iter()
        .map(|(edge, from, to, sense)| plane_boundary(&b, right_frame, edge, from, to, sense))
        .collect::<Result<Vec<_>, _>>()?;
        let face = b.brep.topology.faces.len() as u32;
        b.face(
            &format!("opening-{opening_index}-jamb-upper"),
            SurfaceGeometry::Plane { frame: right_frame },
            plane_bounds(&b, right_frame, [ibr, obr, otr, itr]),
            right_uses,
        )?;
        mark_cut(&mut b, face, 3, "jamb-upper".into());
    }
    if !built_openings.is_empty() {
        b.brep.topology.faces[0].provenance.role = FaceRole::Split;
        b.brep.topology.faces[1].provenance.role = FaceRole::Split;
    }
    b.finish()
}

fn loop_halfedges_for_primitive(
    brep: &BrepEnvelope,
    loop_id: u32,
) -> Result<Vec<u32>, GeometryError> {
    let start = brep.topology.loops[loop_id as usize].start_halfedge;
    let mut current = start;
    let mut result = Vec::new();
    for _ in 0..=brep.topology.halfedges.len() {
        result.push(current);
        current = brep.topology.halfedges[current as usize]
            .next
            .ok_or_else(|| GeometryError::InvalidTopology("open primitive loop".into()))?;
        if current == start {
            return Ok(result);
        }
    }
    Err(GeometryError::InvalidTopology(
        "primitive loop does not close".into(),
    ))
}

fn arched_opening_top_edge(
    brep: &BrepEnvelope,
    face: u32,
    frame: Frame3,
    elevation: f64,
) -> Result<u32, GeometryError> {
    let face = &brep.topology.faces[face as usize];
    let tolerance = brep.accuracy.geometric * 4.0;
    for loop_id in &face.trim.holes {
        for halfedge in loop_halfedges_for_primitive(brep, *loop_id)? {
            let use_ = &brep.topology.halfedges[halfedge as usize];
            let edge = &brep.topology.edges[use_.edge as usize];
            let EdgeGeometry::Curve { curve, .. } = edge.geometry else {
                continue;
            };
            if !matches!(
                brep.geometry.curves[curve as usize],
                CurveGeometry::Circle { .. }
            ) {
                continue;
            }
            let elevations = [use_.from, use_.to].map(|vertex| {
                dot(
                    sub(
                        brep.topology.vertices[vertex as usize].position,
                        frame.origin,
                    ),
                    frame.z,
                )
            });
            if elevations
                .into_iter()
                .all(|value| (value - elevation).abs() <= tolerance)
            {
                return Ok(edge.id);
            }
        }
    }
    Err(GeometryError::InvalidTopology(
        "arched opening header edge is missing".into(),
    ))
}

fn intersection_point_at(
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    parameter: f64,
) -> Result<super::intersection::IntersectionPoint, GeometryError> {
    let CurveGeometry::Intersection { definition } = graph.geometry.curves[branch.curve as usize]
    else {
        return Err(GeometryError::CoverageGap {
            families: ["arched opening".into(), "non-numerical SSI branch".into()],
        });
    };
    graph.geometry.intersections[definition as usize].evaluate(parameter, &graph.geometry)
}

fn upper_intersection_definition(
    graph: &IntersectionGraph,
    branch: &IntersectionBranch,
    vertical_origin: Point3,
    vertical: Point3,
    desired_start: Point3,
    desired_end: Point3,
    support_surfaces: [u32; 2],
    output_surfaces: [&SurfaceGeometry; 2],
    accuracy: Accuracy,
) -> Result<IntersectionDefinition, GeometryError> {
    let range = branch.range;
    let signed_height = |parameter| {
        intersection_point_at(graph, branch, parameter)
            .map(|point| dot(sub(point.point, vertical_origin), vertical))
    };
    let subdivisions = 512;
    let mut roots = Vec::new();
    let mut previous_parameter = range.lo;
    let mut previous_value = signed_height(previous_parameter)?;
    for index in 1..=subdivisions {
        let parameter = range.lo + range.width() * index as f64 / subdivisions as f64;
        let value = signed_height(parameter)?;
        if previous_value == 0.0 {
            roots.push(previous_parameter);
        } else if value == 0.0 || previous_value.signum() != value.signum() {
            let mut lo = previous_parameter;
            let mut hi = parameter;
            let mut lo_value = previous_value;
            for _ in 0..64 {
                let middle = 0.5 * (lo + hi);
                let middle_value = signed_height(middle)?;
                if middle_value == 0.0 || hi - lo <= f64::EPSILON * middle.abs().max(1.0) * 16.0 {
                    lo = middle;
                    hi = middle;
                    break;
                }
                if lo_value.signum() == middle_value.signum() {
                    lo = middle;
                    lo_value = middle_value;
                } else {
                    hi = middle;
                }
            }
            roots.push(0.5 * (lo + hi));
        }
        previous_parameter = parameter;
        previous_value = value;
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() <= accuracy.intersection);
    if roots.len() != 2 {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "arched opening expected two spring intersections, found {}",
            roots.len()
        )));
    }
    let direct_midpoint = 0.5 * (roots[0] + roots[1]);
    let pieces = if signed_height(direct_midpoint)? > 0.0 {
        vec![(roots[0], roots[1])]
    } else {
        vec![(roots[1], range.hi), (range.lo, roots[0])]
    };
    let total_parameter = pieces.iter().map(|(lo, hi)| hi - lo).sum::<f64>();
    if total_parameter <= accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "arched opening curve is below geometric resolution".into(),
        ));
    }
    let sample_count = 128usize;
    let mut source_parameters = Vec::with_capacity(sample_count + 1);
    for sample in 0..=sample_count {
        let mut distance = total_parameter * sample as f64 / sample_count as f64;
        let mut parameter = pieces.last().unwrap().1;
        for (lo, hi) in &pieces {
            let width = hi - lo;
            if distance <= width {
                parameter = lo + distance;
                break;
            }
            distance -= width;
        }
        source_parameters.push(parameter);
    }
    let periods = output_surfaces.map(|surface| surface.charts()[0].periods);
    let mut raw = Vec::with_capacity(source_parameters.len());
    for parameter in source_parameters {
        let point = intersection_point_at(graph, branch, parameter)?;
        raw.push((point.point, [point.uv_a, point.uv_b]));
    }
    if norm(sub(raw[0].0, desired_start)) > norm(sub(raw.last().unwrap().0, desired_start)) {
        raw.reverse();
    }
    for index in 0..raw.len() {
        let previous_uv = index.checked_sub(1).map(|previous| raw[previous].1);
        let (point, uv) = &mut raw[index];
        if index == 0 {
            *point = desired_start;
        } else if index + 1 == sample_count + 1 {
            *point = desired_end;
        }
        for side in 0..2 {
            uv[side] = output_surfaces[side].project(*point, Some(uv[side]))?;
            if let Some(previous_uv) = previous_uv {
                for axis in 0..2 {
                    if let Some(period) = periods[side][axis] {
                        let previous = previous_uv[side][axis];
                        uv[side][axis] += ((previous - uv[side][axis]) / period).round() * period;
                    }
                }
            }
        }
    }
    let mut anchors = Vec::with_capacity(raw.len());
    let mut parameter = 0.0;
    for (index, (point, uv)) in raw.iter().enumerate() {
        if index > 0 {
            parameter += norm(sub(*point, raw[index - 1].0));
        }
        anchors.push(TraceAnchor {
            parameter,
            point: *point,
            uv_a: uv[0],
            uv_b: uv[1],
        });
    }
    let uv_tubes = anchors
        .windows(2)
        .map(|pair| {
            [
                [pair[0].uv_a[0], pair[1].uv_a[0]],
                [pair[0].uv_a[1], pair[1].uv_a[1]],
                [pair[0].uv_b[0], pair[1].uv_b[0]],
                [pair[0].uv_b[1], pair[1].uv_b[1]],
            ]
            .map(|values| {
                let padding = (values[1] - values[0]).abs().max(accuracy.intersection) * 2.0;
                Interval::new(
                    values[0].min(values[1]) - padding,
                    values[0].max(values[1]) + padding,
                )
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| GeometryError::InvalidGeometry("arched opening UV tube".into()))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(IntersectionDefinition {
        surfaces: support_surfaces,
        anchors,
        uv_tubes,
        residual_tolerance: accuracy.intersection,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn circular_wall_with_arched_opening(
    id: String,
    frame: Frame3,
    radius: f64,
    thickness: f64,
    height: f64,
    start_angle: f64,
    sweep_angle: f64,
    opening: CircularWallOpening,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let arch_radius = opening.width * 0.5;
    let lower_height = opening.height - arch_radius;
    if lower_height <= 4.0 * accuracy.geometric {
        return Err(GeometryError::InvalidGeometry(
            "arched opening height must exceed its radius".into(),
        ));
    }
    let spring_elevation = opening.bottom + lower_height;
    let mut result = circular_wall_with_openings(
        id,
        frame,
        radius,
        thickness,
        height,
        start_angle,
        sweep_angle,
        vec![CircularWallOpening {
            height: lower_height,
            ..opening.clone()
        }],
        accuracy,
    )?;
    let plain = circular_wall(
        "arched-opening-support".into(),
        frame,
        radius,
        thickness,
        height,
        start_angle,
        sweep_angle,
        accuracy,
    )?;
    let (sin_angle, cos_angle) = opening.angle.sin_cos();
    let radial = frame.vector([cos_angle, sin_angle, 0.0]);
    let tangent = frame.vector([-sin_angle, cos_angle, 0.0]);
    let cutter_frame = Frame3 {
        origin: frame.point([
            (radius - thickness) * cos_angle,
            (radius - thickness) * sin_angle,
            spring_elevation,
        ]),
        x: tangent,
        y: frame.z,
        z: radial,
    };
    let cutter = cylinder(
        opening.id.clone(),
        cutter_frame,
        arch_radius,
        thickness * 2.0,
        accuracy,
    )?;
    let graphs = [0_u32, 1_u32]
        .map(|face| {
            intersect_faces(
                FaceView { brep: &plain, face },
                FaceView {
                    brep: &cutter,
                    face: 0,
                },
            )
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    if graphs.iter().any(|graph| graph.branches.len() != 1) {
        return Err(GeometryError::UnresolvedIntersection(
            "arched opening support produced multiple SSI branches".into(),
        ));
    }

    let outer_edge = arched_opening_top_edge(&result, 0, frame, spring_elevation)?;
    let inner_edge = arched_opening_top_edge(&result, 1, frame, spring_elevation)?;
    let header = result
        .topology
        .faces
        .iter()
        .find(|face| face.key == "opening-0-header")
        .map(|face| face.id)
        .ok_or_else(|| {
            GeometryError::InvalidTopology("arched opening header face missing".into())
        })?;
    let cutter_surface = result.geometry.surfaces.len() as u32;
    result
        .geometry
        .surfaces
        .push(cutter.geometry.surfaces[0].clone());
    let mut endpoint_uvs = Vec::new();
    for (side, edge_id) in [outer_edge, inner_edge].into_iter().enumerate() {
        let edge = &result.topology.edges[edge_id as usize];
        let primary = &result.topology.halfedges[edge.halfedge as usize];
        let desired_start_vertex = if primary.geometry_use.sense == Orientation::Forward {
            primary.from
        } else {
            primary.to
        };
        let desired_end_vertex = if primary.geometry_use.sense == Orientation::Forward {
            primary.to
        } else {
            primary.from
        };
        let desired_start = result.topology.vertices[desired_start_vertex as usize].position;
        let desired_end = result.topology.vertices[desired_end_vertex as usize].position;
        let definition_geometry = upper_intersection_definition(
            &graphs[side],
            &graphs[side].branches[0],
            cutter_frame.origin,
            cutter_frame.y,
            desired_start,
            desired_end,
            [result.topology.faces[side].surface, cutter_surface],
            [
                &result.geometry.surfaces[result.topology.faces[side].surface as usize],
                &result.geometry.surfaces[cutter_surface as usize],
            ],
            accuracy,
        )?;
        endpoint_uvs.extend([
            (
                definition_geometry.anchors[0].point,
                definition_geometry.anchors[0].uv_b,
            ),
            (
                definition_geometry.anchors.last().unwrap().point,
                definition_geometry.anchors.last().unwrap().uv_b,
            ),
        ]);
        let range = Interval::new(0.0, definition_geometry.anchors.last().unwrap().parameter)?;
        let definition = result.geometry.intersections.len() as u32;
        result.geometry.intersections.push(definition_geometry);
        let curve = result.geometry.curves.len() as u32;
        result
            .geometry
            .curves
            .push(CurveGeometry::Intersection { definition });
        result.topology.edges[edge_id as usize].geometry = EdgeGeometry::Curve { curve, range };
        let wall_pcurve = result.geometry.pcurves.len() as u32;
        result
            .geometry
            .pcurves
            .push(PcurveGeometry::IntersectionSide {
                definition,
                side: IntersectionSide::A,
            });
        let cutter_pcurve = result.geometry.pcurves.len() as u32;
        result
            .geometry
            .pcurves
            .push(PcurveGeometry::IntersectionSide {
                definition,
                side: IntersectionSide::B,
            });
        for halfedge in &mut result.topology.halfedges {
            if halfedge.edge == edge_id {
                halfedge.geometry_use.pcurve = Some(if halfedge.face == Some(side as u32) {
                    wall_pcurve
                } else if halfedge.face == Some(header) {
                    cutter_pcurve
                } else {
                    return Err(GeometryError::InvalidTopology(
                        "arched opening edge has an unexpected face use".into(),
                    ));
                });
            }
        }
    }

    let cutter_surface_geometry = result.geometry.surfaces[cutter_surface as usize].clone();
    for halfedge_id in
        loop_halfedges_for_primitive(&result, result.topology.faces[header as usize].trim.outer)?
    {
        let halfedge = result.topology.halfedges[halfedge_id as usize].clone();
        if halfedge.edge == outer_edge || halfedge.edge == inner_edge {
            continue;
        }
        let edge = &result.topology.edges[halfedge.edge as usize];
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            return Err(GeometryError::InvalidTopology(
                "arched opening connector is collapsed".into(),
            ));
        };
        let points: [Point3; 2] = [range.lo, range.hi]
            .map(|parameter| result.geometry.curve(curve)?.point_at(parameter))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| GeometryError::InvalidGeometry("arched connector points".into()))?;
        let mut coordinates: [[f64; 2]; 2] = points
            .map(|point| cutter_surface_geometry.project(point, None))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| GeometryError::InvalidGeometry("arched connector pcurve".into()))?;
        for (point, uv) in points.into_iter().zip(&mut coordinates) {
            if let Some((_, endpoint_uv)) = endpoint_uvs
                .iter()
                .min_by(|a, b| norm(sub(a.0, point)).total_cmp(&norm(sub(b.0, point))))
            {
                if norm(sub(
                    endpoint_uvs
                        .iter()
                        .min_by(|a, b| norm(sub(a.0, point)).total_cmp(&norm(sub(b.0, point))))
                        .unwrap()
                        .0,
                    point,
                )) <= accuracy.geometric * 4.0
                {
                    *uv = *endpoint_uv;
                }
            }
        }
        let direction = [
            (coordinates[1][0] - coordinates[0][0]) / range.width(),
            (coordinates[1][1] - coordinates[0][1]) / range.width(),
        ];
        let origin = [
            coordinates[0][0] - direction[0] * range.lo,
            coordinates[0][1] - direction[1] * range.lo,
        ];
        let pcurve = result.geometry.pcurves.len() as u32;
        result
            .geometry
            .pcurves
            .push(PcurveGeometry::Line2 { origin, direction });
        result.topology.halfedges[halfedge_id as usize]
            .geometry_use
            .pcurve = Some(pcurve);
    }
    result.topology.faces[header as usize].surface = cutter_surface;
    result.topology.faces[header as usize].sense = Orientation::Reverse;
    result.topology.faces[header as usize].key = "opening-0-arch".into();
    let mut bounds = [
        [f64::INFINITY, f64::NEG_INFINITY],
        [f64::INFINITY, f64::NEG_INFINITY],
    ];
    for halfedge_id in
        loop_halfedges_for_primitive(&result, result.topology.faces[header as usize].trim.outer)?
    {
        let halfedge = &result.topology.halfedges[halfedge_id as usize];
        let pcurve = halfedge.geometry_use.pcurve.unwrap();
        let range = result.topology.edges[halfedge.edge as usize]
            .geometry
            .range();
        for sample in 0..=64 {
            let parameter = range.lo + range.width() * sample as f64 / 64.0;
            let uv = result.geometry.pcurve_at(pcurve, parameter)?;
            for axis in 0..2 {
                bounds[axis][0] = bounds[axis][0].min(uv[axis]);
                bounds[axis][1] = bounds[axis][1].max(uv[axis]);
            }
        }
    }
    result.topology.faces[header as usize].trim.uv_bounds = [
        Interval::new(bounds[0][0], bounds[0][1])?,
        Interval::new(bounds[1][0], bounds[1][1])?,
    ];
    result.revision = result
        .revision
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("arched opening revision overflow".into()))?;
    result.validate()?;
    Ok(result)
}

pub fn sphere(
    id: String,
    frame: Frame3,
    radius: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&[radius])?;
    let mut b = Builder::new(id, accuracy)?;
    let pi2 = std::f64::consts::FRAC_PI_2;
    let tau = std::f64::consts::TAU;
    let south = b.vertex(frame.point([0.0, 0.0, -radius]));
    let north = b.vertex(frame.point([0.0, 0.0, radius]));
    let bottom = b.edge_geometry(EdgeGeometry::Collapsed { vertex: south }, true);
    let top = b.edge_geometry(EdgeGeometry::Collapsed { vertex: north }, true);
    let meridian_frame = Frame3 {
        origin: frame.origin,
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let seam = b.edge(
        CurveGeometry::Circle {
            frame: meridian_frame,
            radius,
        },
        Interval::new(-pi2, pi2)?,
        true,
    );
    b.face(
        "skin",
        SurfaceGeometry::Sphere { frame, radius },
        [[0.0, tau], [-pi2, pi2]],
        vec![
            boundary(
                bottom,
                south,
                south,
                Orientation::Forward,
                uv_line([0.0, -pi2], [tau, 0.0]),
            ),
            boundary(
                seam,
                south,
                north,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                top,
                north,
                north,
                Orientation::Forward,
                uv_line([tau, pi2], [-tau, 0.0]),
            ),
            boundary(
                seam,
                north,
                south,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    b.finish()
}

pub fn torus(
    id: String,
    frame: Frame3,
    major_radius: f64,
    minor_radius: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    let surface = SurfaceGeometry::Torus {
        frame,
        major_radius,
        minor_radius,
    };
    surface.validate()?;
    let mut b = Builder::new(id, accuracy)?;
    let vertex = b.vertex(frame.point([major_radius + minor_radius, 0.0, 0.0]));
    let tau = std::f64::consts::TAU;
    let ring = b.edge(
        CurveGeometry::Circle {
            frame,
            radius: major_radius + minor_radius,
        },
        Interval::new(0.0, tau)?,
        true,
    );
    let tube_frame = Frame3 {
        origin: frame.point([major_radius, 0.0, 0.0]),
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let tube = b.edge(
        CurveGeometry::Circle {
            frame: tube_frame,
            radius: minor_radius,
        },
        Interval::new(0.0, tau)?,
        true,
    );
    b.face(
        "skin",
        surface,
        [[0.0, tau]; 2],
        vec![
            boundary(
                ring,
                vertex,
                vertex,
                Orientation::Forward,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                tube,
                vertex,
                vertex,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                ring,
                vertex,
                vertex,
                Orientation::Reverse,
                uv_line([0.0, tau], [1.0, 0.0]),
            ),
            boundary(
                tube,
                vertex,
                vertex,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    b.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::Surface;
    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 1e-3,
            exchange: 1e-5,
        }
    }
    #[test]
    fn cuboid_edges_are_shared_and_all_face_normals_point_outward() {
        let frame = Frame3 {
            origin: [1.0, 2.0, 3.0],
            x: [0.0, 1.0, 0.0],
            y: [-1.0, 0.0, 0.0],
            ..Frame3::IDENTITY
        };
        let body = cuboid("box".into(), frame, [2.0, 3.0, 4.0], accuracy()).unwrap();
        assert_eq!(
            (
                body.topology.vertices.len(),
                body.topology.edges.len(),
                body.topology.faces.len()
            ),
            (8, 12, 6)
        );
        let center = frame.point([1.0, 1.5, 2.0]);
        for face in &body.topology.faces {
            let surface = body.geometry.surface(face.surface).unwrap();
            let uv = face.trim.uv_bounds.map(|range| (range.lo + range.hi) / 2.0);
            assert!(
                dot(
                    sub(surface.point_at(uv).unwrap(), center),
                    surface.normal_at(uv).unwrap()
                ) > 0.0
            );
        }
        let serialized = body.to_json().unwrap();
        assert_eq!(
            BrepEnvelope::from_json(&serialized)
                .unwrap()
                .to_json()
                .unwrap(),
            serialized
        );
        assert!(cuboid("bad".into(), frame, [0.0, 1.0, 1.0], accuracy()).is_err());
    }
    #[test]
    fn linear_extrusion_preserves_planar_holes_and_shared_edges() {
        let outer = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 4.0], [0.0, 4.0]];
        let hole = vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]];
        let body = linear_extrusion(
            "profile".into(),
            Frame3::IDENTITY,
            outer,
            vec![hole],
            2.5,
            accuracy(),
        )
        .unwrap();

        body.validate().unwrap();
        assert_eq!(body.topology.vertices.len(), 16);
        assert_eq!(body.topology.edges.len(), 24);
        assert_eq!(body.topology.faces.len(), 10);
        assert_eq!(body.topology.shells.len(), 1);
        assert_eq!(body.solids.len(), 1);
        assert_eq!(body.topology.faces[0].trim.holes.len(), 1);
        assert_eq!(body.topology.faces[1].trim.holes.len(), 1);
        assert!(body
            .topology
            .edges
            .iter()
            .all(|edge| edge.twin_halfedge.is_some()));
        let tessellation = crate::analytic::tessellation::tessellate(&body, 0.01, 100_000).unwrap();
        assert!(!tessellation.indices.is_empty());
        assert_eq!(
            tessellation.triangle_face_ids.len(),
            tessellation.indices.len() / 3
        );
    }

    #[test]
    fn linear_extrusion_rejects_invalid_profile_relationships() {
        let square = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
        let crossing = vec![[1.0, 1.0], [3.0, 3.0], [1.0, 3.0], [3.0, 1.0]];
        assert!(linear_extrusion(
            "crossing".into(),
            Frame3::IDENTITY,
            crossing,
            Vec::new(),
            1.0,
            accuracy(),
        )
        .is_err());
        let outside = vec![[3.0, 1.0], [5.0, 1.0], [5.0, 2.0], [3.0, 2.0]];
        assert!(linear_extrusion(
            "outside".into(),
            Frame3::IDENTITY,
            square.clone(),
            vec![outside],
            1.0,
            accuracy(),
        )
        .is_err());
        let first = vec![[0.5, 0.5], [2.5, 0.5], [2.5, 2.5], [0.5, 2.5]];
        let second = vec![[1.5, 1.5], [3.0, 1.5], [3.0, 3.0], [1.5, 3.0]];
        assert!(linear_extrusion(
            "overlap".into(),
            Frame3::IDENTITY,
            square,
            vec![first, second],
            1.0,
            accuracy(),
        )
        .is_err());
    }
    #[test]
    fn curved_topology_is_sparse_and_closed() {
        let shapes = [
            cylinder("c".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap(),
            cone("k".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy()).unwrap(),
            sphere("s".into(), Frame3::IDENTITY, 1.0, accuracy()).unwrap(),
            torus("t".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy()).unwrap(),
        ];
        for (b, faces) in shapes.into_iter().zip([3, 2, 1, 1]) {
            b.validate().unwrap();
            assert_eq!(b.topology.faces.len(), faces);
            assert!(b.topology.vertices.len() <= 2);
            assert_eq!(b.solids.len(), 1);
            let json = b.to_json().unwrap();
            BrepEnvelope::from_json(&json).unwrap();
        }
    }
    #[test]
    fn annular_cylinder_has_exact_inner_outer_supports_and_planar_holes() {
        let body =
            annular_cylinder("tube".into(), Frame3::IDENTITY, 0.5, 1.0, 2.0, accuracy()).unwrap();
        body.validate().unwrap();
        assert_eq!(body.topology.faces.len(), 4);
        assert_eq!(body.topology.shells.len(), 1);
        assert_eq!(body.solids.len(), 1);
        assert_eq!(body.topology.faces[0].sense, Orientation::Forward);
        assert_eq!(body.topology.faces[1].sense, Orientation::Reverse);
        assert_eq!(body.topology.faces[2].trim.holes.len(), 1);
        assert_eq!(body.topology.faces[3].trim.holes.len(), 1);
        let mesh = crate::analytic::tessellation::tessellate(&body, 0.01, 100_000).unwrap();
        assert!(mesh.indices.len() > 0);
        assert!(annular_cylinder(
            "thin".into(),
            Frame3::IDENTITY,
            1.0,
            1.0 + accuracy().geometric,
            2.0,
            accuracy(),
        )
        .is_err());
    }
    #[test]
    fn frusta_reduce_to_the_supported_surface_set() {
        for radii in [(1.0, 2.0), (2.0, 1.0), (2.0, 0.0), (1.0, 1.0)] {
            let b = frustum(
                "f".into(),
                Frame3::IDENTITY,
                radii.0,
                radii.1,
                3.0,
                accuracy(),
            )
            .unwrap();
            b.validate().unwrap();
            assert_eq!(b.topology.faces.len(), if radii.1 == 0.0 { 2 } else { 3 });
        }
    }
    #[test]
    fn ring_and_positive_dimensions_are_enforced() {
        assert!(torus("t".into(), Frame3::IDENTITY, 1.0, 1.0, accuracy()).is_err());
        assert!(cylinder("c".into(), Frame3::IDENTITY, 1.0, 0.0, accuracy()).is_err());
    }
    #[test]
    fn gallery_and_arbitrary_wall_parameters_round_trip_without_changing_geometry() {
        let frame = Frame3 {
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
            ..Frame3::IDENTITY
        };
        let mut bodies = vec![
            cylinder("c".into(), frame, 0.7, 1.8, accuracy()).unwrap(),
            sphere("s".into(), frame, 0.8, accuracy()).unwrap(),
            cone("cone".into(), frame, 0.7, 1.8, accuracy()).unwrap(),
            frustum("f".into(), frame, 0.8, 0.45, 1.8, accuracy()).unwrap(),
            torus("t".into(), frame, 0.75, 0.25, accuracy()).unwrap(),
        ];
        for start in [0.15, 0.7, -2.3] {
            for sweep in [1.8, -1.8, 0.33] {
                bodies.push(
                    circular_wall(
                        "wall".into(),
                        frame,
                        2.4,
                        0.3,
                        1.4,
                        start,
                        sweep,
                        accuracy(),
                    )
                    .unwrap(),
                );
            }
        }
        for body in bodies {
            let json = body.to_json().unwrap();
            let decoded =
                BrepEnvelope::from_json(&json).unwrap_or_else(|e| panic!("{}: {e}", body.id));
            assert_eq!(decoded.to_json().unwrap(), json);
        }
    }
    #[test]
    fn circular_walls_preserve_analytic_boundaries_and_authored_end_identity() {
        let frame = Frame3::from_axis([10.0, -4.0, 6.0], [1.0, 2.0, 3.0], [1.0, 0.0, 0.0]).unwrap();
        for sweep in [1.8, -1.8] {
            let b =
                circular_wall("wall".into(), frame, 3.0, 0.4, 2.5, 0.7, sweep, accuracy()).unwrap();
            assert_eq!(b.topology.faces.len(), 6);
            assert_eq!(b.topology.vertices.len(), 8);
            assert_eq!(b.topology.edges.len(), 12);
            assert_eq!(
                b.geometry
                    .curves
                    .iter()
                    .filter(|c| matches!(c, CurveGeometry::Circle { .. }))
                    .count(),
                4
            );
            let end = b
                .topology
                .faces
                .iter()
                .find(|f| f.key == "axis-start")
                .unwrap();
            let normal = b
                .geometry
                .surface(end.surface)
                .unwrap()
                .normal_at([0.0; 2])
                .unwrap();
            let tangent = frame.vector([-0.7_f64.sin(), 0.7_f64.cos(), 0.0]);
            assert!((dot(normal, tangent) + sweep.signum()).abs() < 1e-12);
            BrepEnvelope::from_json(&b.to_json().unwrap()).unwrap();
        }
        assert!(circular_wall("wall".into(), frame, 1.0, 2.0, 1.0, 0.0, 1.0, accuracy()).is_err());
        assert!(circular_wall(
            "wall".into(),
            frame,
            1.0,
            0.2,
            1.0,
            0.0,
            std::f64::consts::TAU,
            accuracy()
        )
        .is_err());
    }
    #[test]
    fn circular_wall_openings_are_exact_trimmed_cylinder_holes() {
        let openings = vec![
            CircularWallOpening {
                id: "window-a".into(),
                angle: 0.65,
                width: 0.9,
                bottom: 0.7,
                height: 1.2,
            },
            CircularWallOpening {
                id: "window-b".into(),
                angle: 1.45,
                width: 0.6,
                bottom: 1.4,
                height: 0.8,
            },
        ];
        let wall = circular_wall_with_openings(
            "wall".into(),
            Frame3::IDENTITY,
            3.0,
            0.4,
            3.0,
            0.0,
            2.1,
            openings,
            accuracy(),
        )
        .unwrap();
        wall.validate().unwrap();
        assert_eq!(wall.topology.faces.len(), 14);
        assert_eq!(wall.topology.faces[0].trim.holes.len(), 2);
        assert_eq!(wall.topology.faces[1].trim.holes.len(), 2);
        assert_eq!(wall.topology.faces[0].provenance.role, FaceRole::Split);
        assert_eq!(wall.topology.faces[1].provenance.role, FaceRole::Split);
        assert_eq!(
            wall.topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut)
                .count(),
            8
        );
        assert!(wall.geometry.surfaces.iter().all(|surface| matches!(
            surface,
            SurfaceGeometry::Cylinder { .. } | SurfaceGeometry::Plane { .. }
        )));
        let coarse = crate::analytic::tessellation::tessellate(&wall, 0.04, 100_000).unwrap();
        let mesh = crate::analytic::tessellation::tessellate(&wall, 0.01, 100_000).unwrap();
        assert!(mesh.indices.len() > coarse.indices.len());
        let positions = mesh
            .positions
            .chunks_exact(3)
            .map(|point| [point[0].to_bits(), point[1].to_bits(), point[2].to_bits()])
            .collect::<std::collections::HashSet<_>>();
        assert!(mesh
            .outline_positions
            .chunks_exact(3)
            .all(|point| positions.contains(&[
                point[0].to_bits(),
                point[1].to_bits(),
                point[2].to_bits(),
            ])));
        BrepEnvelope::from_json(&wall.to_json().unwrap()).unwrap();

        let door = circular_wall_with_openings(
            "wall".into(),
            Frame3::IDENTITY,
            3.0,
            0.4,
            3.0,
            0.0,
            2.1,
            vec![CircularWallOpening {
                id: "door".into(),
                angle: 1.0,
                width: 0.9,
                bottom: 0.0,
                height: 2.1,
            }],
            accuracy(),
        )
        .unwrap();
        door.validate().unwrap();
        assert_eq!(door.topology.faces[0].trim.holes.len(), 0);
        assert_eq!(door.topology.faces[1].trim.holes.len(), 0);
        assert_eq!(
            door.topology
                .faces
                .iter()
                .filter(|face| face.key.starts_with("bottom-"))
                .count(),
            2
        );
        assert_eq!(
            door.topology
                .faces
                .iter()
                .filter(|face| face.provenance.role == FaceRole::Cut)
                .count(),
            3
        );
        crate::analytic::tessellation::tessellate(&door, 0.01, 100_000).unwrap();
        BrepEnvelope::from_json(&door.to_json().unwrap()).unwrap();
    }

    #[test]
    fn circular_wall_horizontal_opening_faces_bound_curved_edges() {
        let sweep = -std::f64::consts::TAU / 3.0;
        let wall = circular_wall_with_openings(
            "example-wall".into(),
            Frame3::IDENTITY,
            2.4,
            0.3,
            1.4,
            0.0,
            sweep,
            vec![
                CircularWallOpening {
                    id: "door".into(),
                    angle: sweep * 0.22,
                    width: 0.7,
                    bottom: 0.0,
                    height: 1.05,
                },
                CircularWallOpening {
                    id: "window".into(),
                    angle: sweep * 0.76,
                    width: 0.7,
                    bottom: 0.35,
                    height: 0.7,
                },
            ],
            accuracy(),
        )
        .unwrap();
        wall.validate().unwrap();
        crate::analytic::tessellation::tessellate(&wall, 0.01, 100_000).unwrap();
    }
    #[test]
    fn body_bounds_cover_curved_extrema_absent_from_vertices() {
        let b = cylinder("c".into(), Frame3::IDENTITY, 2.0, 3.0, accuracy()).unwrap();
        assert!(b.topology.vertices.iter().all(|v| v.position[0] == 2.0));
        let bounds = b.bounds().unwrap().unwrap();
        for p in [[-2.0, 0.0, 1.5], [0.0, -2.0, 1.5], [0.0, 2.0, 3.0]] {
            assert!(bounds.contains(p));
        }
        let empty = BrepEnvelope::new("empty".into(), accuracy()).unwrap();
        assert!(empty.bounds().unwrap().is_none());
    }
    #[test]
    fn cylinder_sector_keeps_one_trimmed_cylinder_face_and_exact_radial_caps() {
        let sector = cylinder_sector(
            "sector".into(),
            Frame3::IDENTITY,
            2.0,
            3.0,
            -1.2,
            1.2,
            accuracy(),
        )
        .unwrap();
        sector.validate().unwrap();
        assert_eq!(sector.topology.faces.len(), 5);
        assert_eq!(sector.topology.edges.len(), 9);
        assert_eq!(sector.topology.vertices.len(), 6);
        assert!(matches!(
            sector.geometry.surfaces[sector.topology.faces[0].surface as usize],
            SurfaceGeometry::Cylinder { radius: 2.0, .. }
        ));
        assert!(sector.topology.faces[1..].iter().all(|face| matches!(
            sector.geometry.surfaces[face.surface as usize],
            SurfaceGeometry::Plane { .. }
        )));
        let coarse = crate::analytic::tessellation::tessellate(&sector, 0.05, 100_000).unwrap();
        let fine = crate::analytic::tessellation::tessellate(&sector, 0.005, 100_000).unwrap();
        assert!(fine.indices.len() > coarse.indices.len());
        BrepEnvelope::from_json(&sector.to_json().unwrap()).unwrap();

        let full = cylinder_sector(
            "full".into(),
            Frame3::IDENTITY,
            2.0,
            3.0,
            0.0,
            std::f64::consts::TAU,
            accuracy(),
        )
        .unwrap();
        assert_eq!(full.topology.faces.len(), 3);
    }
    #[test]
    fn planar_polyhedron_keeps_explicit_shared_edges_and_plane_authority() {
        let body = planar_polyhedron(
            "tetrahedron".into(),
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
            accuracy(),
        )
        .unwrap();
        body.validate().unwrap();
        assert_eq!(body.topology.vertices.len(), 4);
        assert_eq!(body.topology.edges.len(), 6);
        assert_eq!(body.topology.faces.len(), 4);
        assert!(body
            .geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. })));
        assert!(body
            .geometry
            .curves
            .iter()
            .all(|curve| matches!(curve, CurveGeometry::Line { .. })));
        crate::analytic::tessellation::tessellate(&body, 0.01, 100_000).unwrap();

        let open = planar_polyhedron(
            "open".into(),
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2]],
            accuracy(),
        );
        assert!(open.is_err());
    }
    #[test]
    fn analytic_arc_wires_retain_signed_endpoints_and_single_edge_authority() {
        let curve = CurveGeometry::Circle {
            frame: Frame3::IDENTITY,
            radius: 2.0,
        };
        for sweep in [1.4, -1.4, std::f64::consts::TAU, -std::f64::consts::TAU] {
            let brep = arc_wire("arc".into(), curve.clone(), 0.7, sweep, accuracy()).unwrap();
            let h = &brep.topology.halfedges[0];
            assert_eq!(h.geometry_use.sense.multiplier(), sweep.signum());
            assert!(
                norm(sub(
                    brep.topology.vertices[h.from as usize].position,
                    curve.elementary_point(0.7).unwrap()
                )) < 1e-10
            );
            assert_eq!(brep.topology.edges.len(), 1);
            assert!(brep.topology.faces.is_empty());
            assert!(h.geometry_use.pcurve.is_none());
            assert_eq!(
                brep.topology.wires[0].is_closed,
                sweep.abs() == std::f64::consts::TAU
            );
            let coarse = crate::analytic::tessellation::tessellate(&brep, 0.05, 100_000).unwrap();
            let fine = crate::analytic::tessellation::tessellate(&brep, 0.005, 100_000).unwrap();
            assert!(fine.outline_edge_ids.len() > coarse.outline_edge_ids.len());
            assert!(fine.outline_edge_ids.iter().all(|id| *id == 0));
            assert!(fine.indices.is_empty());
            BrepEnvelope::from_json(&brep.to_json().unwrap()).unwrap();
        }
    }
    #[test]
    fn elliptical_arc_sampling_meets_deflection_and_rejects_invalid_spans() {
        let curve = CurveGeometry::Ellipse {
            frame: Frame3::IDENTITY,
            major_radius: 3.0,
            minor_radius: 0.5,
        };
        let brep = arc_wire(
            "ellipse".into(),
            curve.clone(),
            0.0,
            std::f64::consts::TAU,
            accuracy(),
        )
        .unwrap();
        let mesh = crate::analytic::tessellation::tessellate(&brep, 0.01, 100_000).unwrap();
        let n = mesh.outline_edge_ids.len();
        for (i, pair) in mesh.outline_positions.chunks_exact(6).enumerate() {
            let midpoint = curve
                .elementary_point(std::f64::consts::TAU * (i as f64 + 0.5) / n as f64)
                .unwrap();
            let chord = std::array::from_fn(|j| (pair[j] + pair[j + 3]) / 2.0);
            assert!(norm(sub(midpoint, chord)) < 0.01);
        }
        assert!(arc_wire("bad".into(), curve.clone(), 0.0, 0.0, accuracy()).is_err());
        assert!(arc_wire("bad".into(), curve, 0.0, 7.0, accuracy()).is_err());
    }
}
