mod edges;
mod face_use;

use super::error::{invalid, missing, positive, GeometryError};
use super::geometry::{CurveGeometry, Surface};
use super::pcurve::PcurveGeometry;
use super::topology::{BrepEnvelope, EdgeGeometry, GeometryQuality, SCHEMA_VERSION};
use crate::math::{finite, Interval};
use std::collections::HashSet;

impl BrepEnvelope {
    pub fn validate(&self) -> Result<(), GeometryError> {
        self.check_header()?;
        self.check_geometry()?;
        self.check_pcurves()?;
        let t = &self.topology;
        self.check_table_sizes()?;
        self.check_face_domains()?;
        self.check_vertices()?;
        let incident = self.incident_halfedges()?;
        self.check_edges(&incident)?;
        let loop_uses = self.loop_uses()?;
        self.check_faces()?;
        if t.halfedges
            .iter()
            .any(|h| h.face.is_some() && !loop_uses.contains(&h.id))
        {
            return Err(invalid("orphan face halfedge"));
        }
        let wire_uses = self.wire_uses()?;
        if t.halfedges
            .iter()
            .any(|h| h.wire_ref.is_some() && !wire_uses.contains(&h.id))
        {
            return Err(invalid("orphan wire halfedge"));
        }
        self.check_shells()?;
        self.check_solids()
    }

    fn check_header(&self) -> Result<(), GeometryError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(GeometryError::UnsupportedSchema {
                found: self.schema_version,
            });
        }
        if self.id.is_empty() {
            return Err(invalid("BRep identity is empty"));
        }
        self.accuracy.validate()?;
        if let GeometryQuality::Approximate { max_error } = self.quality {
            positive(max_error)?;
        }
        Ok(())
    }

    fn check_geometry(&self) -> Result<(), GeometryError> {
        for s in &self.geometry.surfaces {
            s.validate()?;
        }
        for c in &self.geometry.curves {
            c.validate()?;
            if let CurveGeometry::Intersection { definition } = c {
                self.geometry.intersection(*definition)?;
            }
        }
        for definition in &self.geometry.intersections {
            if definition.residual_tolerance > self.accuracy.intersection {
                return Err(invalid(
                    "intersection residual budget exceeds model accuracy",
                ));
            }
            definition.validate(&self.geometry)?;
        }
        Ok(())
    }

    fn check_pcurves(&self) -> Result<(), GeometryError> {
        for pcurve in &self.geometry.pcurves {
            match pcurve {
                PcurveGeometry::Line2 { origin, direction } => {
                    for v in origin.iter().chain(direction) {
                        finite(*v)?;
                    }
                }
                PcurveGeometry::Conic2 {
                    origin,
                    axis_a,
                    axis_b,
                } => {
                    for v in origin.iter().chain(axis_a).chain(axis_b) {
                        finite(*v)?;
                    }
                }
                PcurveGeometry::ProjectedCurve {
                    curve,
                    surface,
                    chart,
                    uv_hint,
                    uv_rate,
                    parameter_origin,
                } => {
                    self.geometry.curve(*curve)?;
                    let surface = self.geometry.surface(*surface)?;
                    if *chart as usize >= surface.charts().len() {
                        return Err(missing("chart", *chart));
                    }
                    for v in uv_hint
                        .iter()
                        .chain(uv_rate)
                        .chain(std::iter::once(parameter_origin))
                    {
                        finite(*v)?;
                    }
                }
                PcurveGeometry::IntersectionSide { definition, .. } => {
                    self.geometry.intersection(*definition)?;
                }
            }
        }
        Ok(())
    }

    fn check_table_sizes(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        for length in [
            t.vertices.len(),
            t.edges.len(),
            t.halfedges.len(),
            t.loops.len(),
            t.faces.len(),
            t.wires.len(),
            t.shells.len(),
            self.geometry.surfaces.len(),
            self.geometry.curves.len(),
            self.geometry.pcurves.len(),
            self.geometry.intersections.len(),
        ] {
            if length > 2_000_000 {
                return Err(GeometryError::LimitExceeded(
                    "BRep table exceeds 2000000 records".into(),
                ));
            }
        }
        Ok(())
    }

    fn check_face_domains(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        for face in &t.faces {
            let surface = self.geometry.surface(face.surface)?;
            if face.trim.chart as usize >= surface.charts().len() {
                return Err(missing("chart", face.trim.chart));
            }
            for d in face.trim.uv_bounds {
                Interval::new(d.lo, d.hi)?;
                if d.lo == d.hi {
                    return Err(invalid("face UV bounds must have positive area"));
                }
            }
        }
        Ok(())
    }

    fn check_vertices(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        for (i, v) in t.vertices.iter().enumerate() {
            if v.id as usize != i {
                return Err(invalid("vertex IDs must be dense"));
            }
            positive(v.tolerance)?;
            if v.tolerance > self.accuracy.geometric {
                return Err(invalid(
                    "vertex tolerance exceeds the model accuracy budget",
                ));
            }
            for x in v.position {
                finite(x)?;
            }
            if let Some(h) = v.outgoing_halfedge {
                if t.halfedges
                    .get(h as usize)
                    .ok_or_else(|| missing("halfedge", h))?
                    .from
                    != v.id
                {
                    return Err(invalid("outgoing halfedge starts at another vertex"));
                }
            }
        }
        Ok(())
    }

    fn loop_uses(&self) -> Result<HashSet<u32>, GeometryError> {
        let t = &self.topology;
        let mut loop_uses = HashSet::new();
        for (i, l) in t.loops.iter().enumerate() {
            if l.id as usize != i {
                return Err(invalid("loop IDs must be dense"));
            }
            let face = t
                .faces
                .get(l.face_ref as usize)
                .ok_or_else(|| missing("face", l.face_ref))?;
            if (face.trim.outer == l.id && l.is_hole)
                || (!face.trim.holes.contains(&l.id) && face.trim.outer != l.id)
                || face.trim.holes.contains(&l.id) && !l.is_hole
            {
                return Err(invalid("loop face membership or hole flag is inconsistent"));
            }
            let mut current = l.start_halfedge;
            let mut seen = HashSet::new();
            loop {
                let h = t
                    .halfedges
                    .get(current as usize)
                    .ok_or_else(|| missing("halfedge", current))?;
                if h.loop_ref != Some(l.id)
                    || h.face != Some(face.id)
                    || !seen.insert(current)
                    || !loop_uses.insert(current)
                {
                    return Err(invalid("loop has inconsistent or repeated uses"));
                }
                let next = h.next.ok_or_else(|| invalid("open face loop"))?;
                self.validate_uv_join(h, &t.halfedges[next as usize], face)?;
                if next == l.start_halfedge {
                    break;
                }
                current = next;
                if seen.len() > t.halfedges.len() {
                    return Err(invalid("nonterminating loop"));
                }
            }
        }
        Ok(loop_uses)
    }

    fn check_faces(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        let mut face_keys = HashSet::new();
        for (i, f) in t.faces.iter().enumerate() {
            if f.id as usize != i || f.key.is_empty() || !face_keys.insert(&f.key) {
                return Err(invalid("face IDs or keys are invalid"));
            }
            let surface = self.geometry.surface(f.surface)?;
            if f.trim.chart as usize >= surface.charts().len() {
                return Err(missing("chart", f.trim.chart));
            }
            for d in f.trim.uv_bounds {
                Interval::new(d.lo, d.hi)?;
            }
            t.loops
                .get(f.trim.outer as usize)
                .filter(|l| l.face_ref == f.id && !l.is_hole)
                .ok_or_else(|| invalid("face outer loop is invalid"))?;
            let mut holes = HashSet::new();
            for &hole in &f.trim.holes {
                if !holes.insert(hole) || hole == f.trim.outer {
                    return Err(invalid("duplicate face hole"));
                }
                t.loops
                    .get(hole as usize)
                    .filter(|l| l.face_ref == f.id && l.is_hole)
                    .ok_or_else(|| invalid("face hole is invalid"))?;
            }
            if f.provenance.sources.is_empty()
                || f.provenance
                    .sources
                    .iter()
                    .any(|s| s.key.is_empty() || s.entity.is_empty() || s.body.is_empty())
            {
                return Err(invalid("face provenance is incomplete"));
            }
            if let Some(shell) = f.shell_ref {
                if !t
                    .shells
                    .get(shell as usize)
                    .ok_or_else(|| missing("shell", shell))?
                    .faces
                    .contains(&f.id)
                {
                    return Err(invalid("face shell membership is inconsistent"));
                }
            }
        }
        Ok(())
    }

    fn wire_uses(&self) -> Result<HashSet<u32>, GeometryError> {
        let t = &self.topology;
        let mut wire_uses = HashSet::new();
        for (i, w) in t.wires.iter().enumerate() {
            if w.id as usize != i {
                return Err(invalid("wire IDs must be dense"));
            }
            let mut current = w.start_halfedge;
            let mut seen = HashSet::new();
            loop {
                let h = t
                    .halfedges
                    .get(current as usize)
                    .ok_or_else(|| missing("halfedge", current))?;
                if h.wire_ref != Some(w.id) || !seen.insert(current) || !wire_uses.insert(current) {
                    return Err(invalid("invalid wire membership"));
                }
                match h.next {
                    Some(next) if next == w.start_halfedge => {
                        if !w.is_closed {
                            return Err(invalid("open wire forms a cycle"));
                        }
                        break;
                    }
                    Some(next) => current = next,
                    None => {
                        if w.is_closed {
                            return Err(invalid("closed wire is open"));
                        }
                        break;
                    }
                }
            }
        }
        Ok(wire_uses)
    }

    fn check_shells(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        for (i, s) in t.shells.iter().enumerate() {
            if s.id as usize != i || s.faces.is_empty() {
                return Err(invalid("invalid shell IDs or empty shell"));
            }
            let mut seen = HashSet::new();
            for &face in &s.faces {
                let f = t
                    .faces
                    .get(face as usize)
                    .ok_or_else(|| missing("face", face))?;
                if f.shell_ref != Some(s.id) || !seen.insert(face) {
                    return Err(invalid("inconsistent shell membership"));
                }
            }
            if s.is_closed {
                for h in t
                    .halfedges
                    .iter()
                    .filter(|h| h.face.is_some_and(|f| seen.contains(&f)))
                {
                    if matches!(
                        t.edges[h.edge as usize].geometry,
                        EdgeGeometry::Collapsed { .. }
                    ) {
                        continue;
                    }
                    let twin = h
                        .twin
                        .ok_or_else(|| invalid("closed shell has a free edge"))?;
                    if !t.halfedges[twin as usize]
                        .face
                        .is_some_and(|f| seen.contains(&f))
                    {
                        return Err(invalid("shell twin belongs to another shell"));
                    }
                }
            }
        }
        Ok(())
    }

    fn check_solids(&self) -> Result<(), GeometryError> {
        let t = &self.topology;
        let mut shells = HashSet::new();
        for solid in &self.solids {
            for shell in std::iter::once(&solid.outer_shell).chain(&solid.cavity_shells) {
                if !shells.insert(*shell)
                    || !t
                        .shells
                        .get(*shell as usize)
                        .ok_or_else(|| missing("shell", *shell))?
                        .is_closed
                {
                    return Err(invalid("solid shell is open or multiply assigned"));
                }
            }
        }
        if t.shells
            .iter()
            .any(|s| s.is_closed && !shells.contains(&s.id))
        {
            return Err(invalid("closed shell has no solid membership"));
        }
        Ok(())
    }
}
