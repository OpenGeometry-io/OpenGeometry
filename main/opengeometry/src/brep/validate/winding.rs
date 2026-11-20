use crate::brep::error::{invalid, GeometryError};
use crate::brep::frame::UV;
use crate::brep::geometry::Surface;
use crate::brep::pcurve::{period_lifted_uv, PcurveGeometry};
use crate::brep::topology::{BrepEnvelope, Face, Orientation};
use crate::math::{cross, dot};

const SAMPLES_PER_USE: usize = 16;

struct Winding {
    id: u32,
    hole: bool,
    area: f64,
    delta: UV,
    mean: UV,
}

impl BrepEnvelope {
    pub(crate) fn orient_loops(&mut self) -> Result<(), GeometryError> {
        let mut flips = Vec::new();
        for face in &self.topology.faces {
            flips.extend(self.misoriented_loops(face)?);
        }
        if flips.is_empty() {
            return Ok(());
        }
        self.reverse_loops(&flips)
    }

    pub(super) fn reverse_loops(&mut self, loops: &[u32]) -> Result<(), GeometryError> {
        for &loop_id in loops {
            let start = self.topology.loops[loop_id as usize].start_halfedge;
            let mut h = start;
            for _ in 0..=self.topology.halfedges.len() {
                let use_ = &mut self.topology.halfedges[h as usize];
                use_.reverse();
                h = use_.prev.ok_or_else(|| invalid("nonterminating loop"))?;
                if h == start {
                    break;
                }
            }
        }
        let mut outgoing = vec![None; self.topology.vertices.len()];
        for use_ in &self.topology.halfedges {
            outgoing[use_.from as usize].get_or_insert(use_.id);
        }
        let halfedges = &self.topology.halfedges;
        for vertex in &mut self.topology.vertices {
            if let Some(h) = vertex.outgoing_halfedge {
                if halfedges[h as usize].from != vertex.id {
                    vertex.outgoing_halfedge = outgoing[vertex.id as usize];
                }
            }
        }
        Ok(())
    }

    pub(super) fn check_loop_winding(&self) -> Result<(), GeometryError> {
        for face in &self.topology.faces {
            if !self.misoriented_loops(face)?.is_empty() {
                return Err(invalid(format!(
                    "face {} ('{}') has loops running against its material normal",
                    face.id, face.key
                )));
            }
        }
        Ok(())
    }

    fn misoriented_loops(&self, face: &Face) -> Result<Vec<u32>, GeometryError> {
        let Some(material) = self.material_sign(face)? else {
            return Ok(Vec::new());
        };
        let periods = self.face_periods(face)?;
        let mut windings = Vec::new();
        for &loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
            windings.push(self.winding(loop_id, periods)?);
        }
        let wraps = |w: &Winding, axis: usize| {
            periods[axis].is_some_and(|period| w.delta[axis].abs() > 0.5 * period)
        };
        let mut flips = Vec::new();
        let mut wrapping = Vec::new();
        for w in &windings {
            if wraps(w, 0) || wraps(w, 1) {
                wrapping.push(w);
            } else if (w.area * material > 0.0) == w.hole {
                flips.push(w.id);
            }
        }
        if let [a, b] = wrapping[..] {
            let axis = match (wraps(a, 0) && wraps(b, 0), wraps(a, 1) && wraps(b, 1)) {
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
            if let Some(axis) = axis {
                flips.extend(band_flips(a, b, axis, material));
            }
        }
        Ok(flips)
    }

    fn material_sign(&self, face: &Face) -> Result<Option<f64>, GeometryError> {
        let surface = self.geometry.surface(face.surface)?;
        let [u, v] = face.trim.uv_bounds;
        let centre = [(u.lo + u.hi) * 0.5, (v.lo + v.hi) * 0.5];
        let Ok(normal) = surface.normal_at(centre) else {
            return Ok(None);
        };
        let jet = surface.derivatives(centre)?;
        let chart = dot(cross(jet.du, jet.dv), normal);
        if chart == 0.0 {
            return Ok(None);
        }
        Ok(Some(chart.signum() * face.sense.multiplier()))
    }

    fn face_periods(&self, face: &Face) -> Result<[Option<f64>; 2], GeometryError> {
        let surface = self.geometry.surface(face.surface)?;
        Ok(surface.charts()[face.trim.chart as usize].periods)
    }

    fn winding(&self, loop_id: u32, periods: [Option<f64>; 2]) -> Result<Winding, GeometryError> {
        let points = self.loop_samples(loop_id, periods)?;
        let mut area = 0.0;
        let mut mean = [0.0; 2];
        let count = (points.len() - 1) as f64;
        for pair in points.windows(2) {
            area -= 0.5 * (pair[0][1] + pair[1][1]) * (pair[1][0] - pair[0][0]);
            for axis in 0..2 {
                mean[axis] += pair[0][axis] / count;
            }
        }
        let (first, last) = (points[0], points[points.len() - 1]);
        Ok(Winding {
            id: loop_id,
            hole: self.topology.loops[loop_id as usize].is_hole,
            area,
            delta: [last[0] - first[0], last[1] - first[1]],
            mean,
        })
    }

    fn loop_samples(
        &self,
        loop_id: u32,
        periods: [Option<f64>; 2],
    ) -> Result<Vec<UV>, GeometryError> {
        let start = self.topology.loops[loop_id as usize].start_halfedge;
        let mut points: Vec<UV> = Vec::new();
        let mut h = start;
        for _ in 0..=self.topology.halfedges.len() {
            let use_ = &self.topology.halfedges[h as usize];
            let range = self.topology.edges[use_.edge as usize].geometry.range();
            let pcurve = use_
                .geometry_use
                .pcurve
                .ok_or_else(|| invalid("missing face pcurve"))?;
            let straight = matches!(
                self.geometry.pcurves[pcurve as usize],
                PcurveGeometry::Line2 { .. }
            );
            let samples = if straight && periods == [None, None] {
                1
            } else {
                SAMPLES_PER_USE
            };
            for i in 0..=samples {
                let s = i as f64 / samples as f64;
                let s = match use_.geometry_use.sense {
                    Orientation::Forward => s,
                    Orientation::Reverse => 1.0 - s,
                };
                let uv = self
                    .geometry
                    .pcurve_at(pcurve, range.lo + range.width() * s)?;
                let mut uv = period_lifted_uv(uv, periods, use_.geometry_use.periodic_lift);
                if let Some(previous) = points.last() {
                    for axis in 0..2 {
                        if let Some(period) = periods[axis] {
                            uv[axis] += ((previous[axis] - uv[axis]) / period).round() * period;
                        }
                    }
                }
                if i > 0 || points.is_empty() {
                    points.push(uv);
                }
            }
            h = use_.next.ok_or_else(|| invalid("nonterminating loop"))?;
            if h == start {
                return Ok(points);
            }
        }
        Err(invalid("nonterminating loop"))
    }
}

fn band_flips(a: &Winding, b: &Winding, axis: usize, material: f64) -> Vec<u32> {
    let other = 1 - axis;
    let (low, high) = if a.mean[other] <= b.mean[other] {
        (a, b)
    } else {
        (b, a)
    };
    let forward = if axis == 0 { 1.0 } else { -1.0 };
    let mut flips = Vec::new();
    if low.delta[axis] * material * forward < 0.0 {
        flips.push(low.id);
    }
    if high.delta[axis] * material * forward > 0.0 {
        flips.push(high.id);
    }
    flips
}
