use crate::brep::error::{invalid, GeometryError};
use crate::brep::frame::UV;
use crate::brep::geometry::{Curve, CurveGeometry, Surface, SurfaceGeometry};
use crate::brep::pcurve::{IntersectionSide, PcurveGeometry};
use crate::brep::topology::{BrepEnvelope, Edge, EdgeGeometry, Face, HalfEdge, Orientation};
use crate::math::{add, norm, scale, sub, Point3};

impl BrepEnvelope {
    pub(super) fn validate_face_use(
        &self,
        h: &HalfEdge,
        face: &Face,
        edge: &Edge,
        tolerance: f64,
    ) -> Result<(), GeometryError> {
        let surface = self.geometry.surface(face.surface)?;
        let pcurve = h
            .geometry_use
            .pcurve
            .ok_or_else(|| invalid("missing face pcurve"))?;
        self.check_pcurve_reference(pcurve, face, edge)?;
        let range = edge.geometry.range();
        for i in 0..=8 {
            let t = (range.lo + (range.hi - range.lo) * i as f64 / 8.0).clamp(range.lo, range.hi);
            let mut uv = self.geometry.pcurve_at(pcurve, t)?;
            let chart = &surface.charts()[face.trim.chart as usize];
            for j in 0..2 {
                if let Some(period) = chart.periods[j] {
                    uv[j] += h.geometry_use.periodic_lift[j] as f64 * period;
                } else if h.geometry_use.periodic_lift[j] != 0 {
                    return Err(invalid("lift on nonperiodic chart axis"));
                }
            }
            let p = surface.point_at(uv)?;
            let expected = match edge.geometry {
                EdgeGeometry::Curve { curve, .. } => self.geometry.curve(curve)?.point_at(t)?,
                EdgeGeometry::Collapsed { vertex } => {
                    self.topology.vertices[vertex as usize].position
                }
            };
            if norm(sub(p, expected)) > tolerance {
                return Err(invalid("pcurve does not lie on edge support"));
            }
            check_uv_bounds(h, face, surface, uv, p, tolerance)?;
            if matches!(edge.geometry, EdgeGeometry::Collapsed { .. }) {
                let singular = match surface {
                    SurfaceGeometry::Sphere { .. } => uv[1].abs() == std::f64::consts::FRAC_PI_2,
                    SurfaceGeometry::Cone { .. } => uv[1] == 0.0,
                    _ => false,
                };
                if !singular {
                    return Err(invalid("collapsed edge is not at a surface singularity"));
                }
            }
        }
        Ok(())
    }

    fn check_pcurve_reference(
        &self,
        pcurve: u32,
        face: &Face,
        edge: &Edge,
    ) -> Result<(), GeometryError> {
        match &self.geometry.pcurves[pcurve as usize] {
            PcurveGeometry::ProjectedCurve {
                curve,
                surface: s,
                chart,
                ..
            } => {
                if *s != face.surface
                    || *chart != face.trim.chart
                    || !matches!(edge.geometry,EdgeGeometry::Curve{curve:c,..}if c==*curve)
                {
                    return Err(invalid("projected pcurve references another edge or face"));
                }
            }
            PcurveGeometry::IntersectionSide { definition, side } => {
                let d = &self.geometry.intersections[*definition as usize];
                let s = d.surfaces[if *side == IntersectionSide::A { 0 } else { 1 }];
                if s != face.surface
                    || !matches!(edge.geometry,EdgeGeometry::Curve{curve,..}if matches!(self.geometry.curves[curve as usize],CurveGeometry::Intersection{definition:df}if df==*definition))
                {
                    return Err(invalid(
                        "intersection pcurve references another face or curve",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn validate_uv_join(
        &self,
        a: &HalfEdge,
        b: &HalfEdge,
        face: &Face,
    ) -> Result<(), GeometryError> {
        let range_a = self.topology.edges[a.edge as usize].geometry.range();
        let range_b = self.topology.edges[b.edge as usize].geometry.range();
        let ta = if a.geometry_use.sense == Orientation::Forward {
            range_a.hi
        } else {
            range_a.lo
        };
        let tb = if b.geometry_use.sense == Orientation::Forward {
            range_b.lo
        } else {
            range_b.hi
        };
        let ua = self.geometry.pcurve_at(
            a.geometry_use
                .pcurve
                .ok_or_else(|| invalid("missing pcurve"))?,
            ta,
        )?;
        let ub = self.geometry.pcurve_at(
            b.geometry_use
                .pcurve
                .ok_or_else(|| invalid("missing pcurve"))?,
            tb,
        )?;
        let surface = self.geometry.surface(face.surface)?;
        let chart = &surface.charts()[face.trim.chart as usize];
        let jet = surface.derivatives(ua)?;
        let mut displacement = [0.0; 2];
        for j in 0..2 {
            displacement[j] = ua[j] - ub[j];
            if let Some(period) = chart.periods[j] {
                displacement[j] -= (displacement[j] / period).round() * period;
            }
        }
        let error = norm(add(
            scale(jet.du, displacement[0]),
            scale(jet.dv, displacement[1]),
        ));
        if error > self.accuracy.geometric {
            return Err(invalid("pcurve loop is discontinuous"));
        }
        Ok(())
    }
}

fn check_uv_bounds(
    h: &HalfEdge,
    face: &Face,
    surface: &SurfaceGeometry,
    uv: UV,
    p: Point3,
    tolerance: f64,
) -> Result<(), GeometryError> {
    if !face.trim.uv_bounds[0].contains(uv[0]) || !face.trim.uv_bounds[1].contains(uv[1]) {
        let clipped = std::array::from_fn(|axis| {
            uv[axis].clamp(face.trim.uv_bounds[axis].lo, face.trim.uv_bounds[axis].hi)
        });
        let jet = surface.derivatives(clipped)?;
        for axis in 0..2 {
            let excess = (uv[axis] - clipped[axis]).abs();
            let metric = norm(if axis == 0 { jet.du } else { jet.dv });
            let roundoff = 128.0 * f64::EPSILON * uv[axis].abs().max(clipped[axis].abs()).max(1.0);
            let allowed = roundoff
                + if metric > 0.0 {
                    tolerance / metric
                } else {
                    0.0
                };
            if excess > allowed {
                return Err(invalid(format!(
                    "pcurve on halfedge {} leaves face {} UV bounds at [{}, {}]",
                    h.id, face.id, uv[0], uv[1]
                )));
            }
        }
        if norm(sub(surface.point_at(clipped)?, p)) > tolerance {
            return Err(invalid(format!(
                "pcurve on halfedge {} leaves face {} UV bounds at [{}, {}]",
                h.id, face.id, uv[0], uv[1]
            )));
        }
    }
    Ok(())
}
