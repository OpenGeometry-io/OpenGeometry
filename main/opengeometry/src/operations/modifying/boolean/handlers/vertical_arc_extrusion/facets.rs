use super::section::{canonical_arc_start, coverage};
use crate::brep::{
    boundary, padded_uv_bounds, plane_boundary, unit, uv_line, Accuracy, Builder, CurveGeometry,
    FaceProvenance, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::geom2d::{reversed_ring, CurveEdge2, CurveRegion2, Pt2};
use crate::math::{add, cross, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::assembly::add_welded_vertex;
use std::collections::BTreeMap;

pub(super) struct Facets {
    pub(super) builder: Builder,
    vertices: Vec<(Point3, u32)>,
    edges: BTreeMap<(u32, u32, i64, i64, i64), (u32, u32, u32, bool)>,
    pub(super) accuracy: Accuracy,
    pub(super) base: Frame3,
}

struct SideLoop {
    lo: f64,
    hi: f64,
    lower_from: Point3,
    lower_to: Point3,
    bottom: (u32, u32, u32, Orientation),
    top: (u32, u32, u32, Orientation),
    end_segments: Vec<(u32, u32, u32, Orientation, f64, f64)>,
    start_segments: Vec<(u32, u32, u32, Orientation, f64, f64)>,
}

pub(super) struct SidePatch {
    pub(super) edge: CurveEdge2,
    pub(super) levels: Vec<f64>,
    pub(super) provenance: FaceProvenance,
}

pub(super) fn same_side_patch(a: &SidePatch, b: &SidePatch, tolerance: f64) -> bool {
    if std::mem::discriminant(&a.edge) != std::mem::discriminant(&b.edge)
        || a.provenance.role != b.provenance.role
        || a.provenance.reversed != b.provenance.reversed
        || a.provenance.sources.len() != b.provenance.sources.len()
        || !a
            .provenance
            .sources
            .iter()
            .zip(&b.provenance.sources)
            .all(|(a, b)| {
                a.entity == b.entity && a.body == b.body && a.key == b.key && a.face == b.face
            })
        || (a.edge.length() - b.edge.length()).abs() > tolerance
    {
        return false;
    }
    [0.0, 0.5, 1.0].into_iter().all(|station| {
        let a = a.edge.point(station);
        let b = b.edge.point(station);
        (a.x - b.x).hypot(a.z - b.z) <= tolerance
    })
}

impl Facets {
    pub(super) fn new(id: String, accuracy: Accuracy, base: Frame3) -> Result<Self, GeometryError> {
        Ok(Self {
            builder: Builder::new(id, accuracy)?,
            vertices: Vec::new(),
            edges: BTreeMap::new(),
            accuracy,
            base,
        })
    }

    fn vertex(&mut self, point: Point3) -> u32 {
        add_welded_vertex(&mut self.builder, &mut self.vertices, point, self.accuracy)
    }

    fn edge(
        &mut self,
        from: Point3,
        to: Point3,
        middle: Point3,
        circle: Option<([f64; 2], f64, f64, f64)>,
    ) -> Result<(u32, u32, u32, Orientation), GeometryError> {
        let from_id = self.vertex(from);
        let to_id = self.vertex(to);
        if from_id == to_id {
            return Err(coverage());
        }
        let step = self.accuracy.geometric / 4.0;
        let key = (
            from_id.min(to_id),
            from_id.max(to_id),
            (middle[0] / step).round() as i64,
            (middle[1] / step).round() as i64,
            (middle[2] / step).round() as i64,
        );
        if let Some(&(id, stored_from, stored_to, forward)) = self.edges.get(&key) {
            let same = from_id == stored_from && to_id == stored_to;
            let orientation = if same == forward {
                Orientation::Forward
            } else {
                Orientation::Reverse
            };
            return Ok((id, from_id, to_id, orientation));
        }
        let (curve, range, forward) = if let Some((centre, radius, start, sweep)) = circle {
            let origin = self
                .base
                .point([centre[0], centre[1], self.base.local(from)[2]]);
            let frame = Frame3 {
                origin,
                ..self.base
            };
            let start = canonical_arc_start(start, sweep);
            let end = start + sweep;
            (
                CurveGeometry::Circle { frame, radius },
                Interval::new(start.min(end), start.max(end))?,
                sweep > 0.0,
            )
        } else {
            let delta = sub(to, from);
            (
                CurveGeometry::Line {
                    origin: from,
                    direction: unit(delta)?,
                },
                Interval::new(0.0, norm(delta))?,
                true,
            )
        };
        let id = self.builder.edge(curve, range, false);
        self.edges.insert(key, (id, from_id, to_id, forward));
        Ok((
            id,
            from_id,
            to_id,
            if forward {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
        ))
    }

    fn edge_at(
        &mut self,
        edge: &CurveEdge2,
        level: f64,
    ) -> Result<(u32, u32, u32, Orientation), GeometryError> {
        let point = |t| {
            let p = edge.point(t);
            self.base.point([p.x, p.z, level])
        };
        let curve = match edge {
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => Some((*center, *radius, *start_angle, *sweep_angle)),
            CurveEdge2::Line { .. } => None,
        };
        self.edge(point(0.0), point(1.0), point(0.5), curve)
    }

    fn cap_uses(
        &mut self,
        ring: &[CurveEdge2],
        level: f64,
        frame: Frame3,
    ) -> Result<Vec<Use>, GeometryError> {
        let mut uses = Vec::new();
        for edge in ring {
            let (id, from, to, sense) = self.edge_at(edge, level)?;
            uses.push(plane_boundary(&self.builder, frame, id, from, to, sense)?);
        }
        Ok(uses)
    }

    pub(super) fn cap(
        &mut self,
        regions: &[CurveRegion2],
        level: f64,
        up: bool,
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        let origin = self.base.point([0.0, 0.0, level]);
        let frame = if up {
            Frame3 {
                origin,
                ..self.base
            }
        } else {
            Frame3 {
                origin,
                x: self.base.x,
                y: scale(self.base.y, -1.0),
                z: scale(self.base.z, -1.0),
            }
        };
        for region in regions {
            let outer = if up {
                region.outer.clone()
            } else {
                reversed_ring(&region.outer)
            };
            let holes = if up {
                region.holes.clone()
            } else {
                region
                    .holes
                    .iter()
                    .map(|ring| reversed_ring(ring))
                    .collect()
            };
            let bounds = self.cap_bounds(&outer, &holes, level, frame);
            let outer_uses = self.cap_uses(&outer, level, frame)?;
            let hole_uses = holes
                .iter()
                .map(|ring| self.cap_uses(ring, level, frame))
                .collect::<Result<Vec<_>, _>>()?;
            let id = self.builder.brep.topology.faces.len();
            self.builder.face_with_holes(
                &format!("{}-cap-{id}", if up { "upper" } else { "lower" }),
                SurfaceGeometry::Plane { frame },
                bounds,
                outer_uses,
                hole_uses,
            )?;
            self.builder.brep.topology.faces[id].provenance = provenance.clone();
        }
        Ok(())
    }

    fn cap_bounds(
        &self,
        outer: &[CurveEdge2],
        holes: &[Vec<CurveEdge2>],
        level: f64,
        frame: Frame3,
    ) -> [[f64; 2]; 2] {
        let points = outer.iter().chain(holes.iter().flatten()).flat_map(|edge| {
            let mut stations = vec![0.0, 0.5, 1.0];
            if let CurveEdge2::Arc { center, radius, .. } = edge {
                for angle in [
                    0.0,
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    3.0 * std::f64::consts::FRAC_PI_2,
                ] {
                    let point = Pt2::new(
                        center[0] + radius * angle.cos(),
                        center[1] + radius * angle.sin(),
                    );
                    if let Some(station) = edge.parameter(point, self.accuracy.intersection) {
                        stations.push(station);
                    }
                }
            }
            stations.into_iter().map(move |t| {
                let point = edge.point(t);
                frame.local(self.base.point([point.x, point.z, level]))
            })
        });
        padded_uv_bounds(points, self.accuracy.geometric)
    }

    pub(super) fn side(
        &mut self,
        edge: &CurveEdge2,
        levels: &[f64],
        provenance: FaceProvenance,
    ) -> Result<(), GeometryError> {
        let side_loop = self.side_loop(edge, levels)?;
        let id = self.builder.brep.topology.faces.len();
        match edge {
            CurveEdge2::Line { .. } => {
                self.add_plane_side(id, &side_loop)?;
            }
            CurveEdge2::Arc {
                center,
                radius,
                start_angle,
                sweep_angle,
            } => {
                self.add_cylinder_side(
                    id,
                    &side_loop,
                    *center,
                    *radius,
                    *start_angle,
                    *sweep_angle,
                )?;
            }
        }
        self.builder.brep.topology.faces[id].provenance = provenance;
        Ok(())
    }

    fn side_loop(&mut self, edge: &CurveEdge2, levels: &[f64]) -> Result<SideLoop, GeometryError> {
        if levels.len() < 2 || levels.windows(2).any(|span| span[1] <= span[0]) {
            return Err(coverage());
        }
        let lo = levels[0];
        let hi = *levels.last().ok_or_else(coverage)?;
        let base = self.base;
        let point = |t, level| {
            let p = edge.point(t);
            base.point([p.x, p.z, level])
        };
        let lower_from = point(0.0, lo);
        let lower_to = point(1.0, lo);
        let bottom = self.edge_at(edge, lo)?;
        let top = self.edge_at(edge, hi)?;
        let mut end_segments = Vec::with_capacity(levels.len() - 1);
        let mut start_segments = Vec::with_capacity(levels.len() - 1);
        for span in levels.windows(2) {
            let from = point(1.0, span[0]);
            let to = point(1.0, span[1]);
            let (edge, from_id, to_id, sense) =
                self.edge(from, to, scale(add(from, to), 0.5), None)?;
            end_segments.push((edge, from_id, to_id, sense, span[0], span[1]));
        }
        for span in levels.windows(2).rev() {
            let from = point(0.0, span[1]);
            let to = point(0.0, span[0]);
            let (edge, from_id, to_id, sense) =
                self.edge(from, to, scale(add(from, to), 0.5), None)?;
            start_segments.push((edge, from_id, to_id, sense, span[0], span[1]));
        }
        Ok(SideLoop {
            lo,
            hi,
            lower_from,
            lower_to,
            bottom,
            top,
            end_segments,
            start_segments,
        })
    }

    fn add_plane_side(&mut self, id: usize, side_loop: &SideLoop) -> Result<(), GeometryError> {
        let &SideLoop {
            lo,
            hi,
            lower_from,
            lower_to,
            bottom: (bottom, bf, bt, bottom_sense),
            top: (top, tf, tt, top_sense),
            ref end_segments,
            ref start_segments,
        } = side_loop;
        let edge_direction = unit(sub(lower_to, lower_from))?;
        let normal = unit(cross(edge_direction, self.base.z))?;
        let frame = Frame3::from_axis(lower_from, normal, edge_direction)?;
        let mut uses = Vec::with_capacity(2 + 2 * end_segments.len());
        uses.push(plane_boundary(
            &self.builder,
            frame,
            bottom,
            bf,
            bt,
            bottom_sense,
        )?);
        for &(edge, from, to, sense, _, _) in end_segments {
            uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
        }
        uses.push(plane_boundary(
            &self.builder,
            frame,
            top,
            tt,
            tf,
            if top_sense == Orientation::Forward {
                Orientation::Reverse
            } else {
                Orientation::Forward
            },
        )?);
        for &(edge, from, to, sense, _, _) in start_segments {
            uses.push(plane_boundary(&self.builder, frame, edge, from, to, sense)?);
        }
        self.builder.face(
            &format!("vertical-plane-{id}"),
            SurfaceGeometry::Plane { frame },
            [
                [
                    -self.accuracy.geometric,
                    norm(sub(lower_to, lower_from)) + self.accuracy.geometric,
                ],
                [-self.accuracy.geometric, hi - lo + self.accuracy.geometric],
            ],
            uses,
        )?;
        self.builder.brep.topology.faces[id].sense = Orientation::Reverse;
        Ok(())
    }

    fn add_cylinder_side(
        &mut self,
        id: usize,
        side_loop: &SideLoop,
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    ) -> Result<(), GeometryError> {
        let &SideLoop { lo, hi, .. } = side_loop;
        let origin = self.base.point([center[0], center[1], 0.0]);
        let frame = Frame3 {
            origin,
            ..self.base
        };
        let start_angle = canonical_arc_start(start_angle, sweep_angle);
        let end_angle = start_angle + sweep_angle;
        let uses = cylinder_side_uses(side_loop, start_angle, end_angle);
        self.builder.face(
            &format!("vertical-cylinder-{id}"),
            SurfaceGeometry::Cylinder { frame, radius },
            [
                [
                    start_angle.min(end_angle) - self.accuracy.geometric / radius,
                    start_angle.max(end_angle) + self.accuracy.geometric / radius,
                ],
                [lo - self.accuracy.geometric, hi + self.accuracy.geometric],
            ],
            uses,
        )?;
        if sweep_angle > 0.0 {
            self.builder.brep.topology.faces[id].sense = Orientation::Reverse;
        }
        Ok(())
    }
}

fn cylinder_side_uses(side_loop: &SideLoop, start_angle: f64, end_angle: f64) -> Vec<Use> {
    let &SideLoop {
        lo,
        hi,
        bottom: (bottom, bf, bt, bottom_sense),
        top: (top, tf, tt, top_sense),
        ref end_segments,
        ref start_segments,
        ..
    } = side_loop;
    let mut uses = Vec::with_capacity(2 + 2 * end_segments.len());
    uses.push(boundary(
        bottom,
        bf,
        bt,
        bottom_sense,
        uv_line([0.0, lo], [1.0, 0.0]),
    ));
    for &(edge, from, to, sense, start, end) in end_segments {
        uses.push(boundary(
            edge,
            from,
            to,
            sense,
            if sense == Orientation::Forward {
                uv_line([end_angle, start], [0.0, 1.0])
            } else {
                uv_line([end_angle, end], [0.0, -1.0])
            },
        ));
    }
    uses.push(boundary(
        top,
        tt,
        tf,
        if top_sense == Orientation::Forward {
            Orientation::Reverse
        } else {
            Orientation::Forward
        },
        uv_line([0.0, hi], [1.0, 0.0]),
    ));
    for &(edge, from, to, sense, start, end) in start_segments {
        uses.push(boundary(
            edge,
            from,
            to,
            sense,
            if sense == Orientation::Forward {
                uv_line([start_angle, end], [0.0, -1.0])
            } else {
                uv_line([start_angle, start], [0.0, 1.0])
            },
        ));
    }
    uses
}
