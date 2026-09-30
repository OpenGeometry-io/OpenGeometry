use super::{
    geometry::{norm, sub, PatchBounds, Surface, UVBox},
    intersection::IntersectionDefinition,
    Curve, CurveGeometry, GeometryError, Point3, SurfaceGeometry, UV,
};
use crate::math::{finite, interval::Interval};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Orientation {
    Forward,
    Reverse,
}
impl Orientation {
    pub fn multiplier(self) -> f64 {
        if self == Self::Forward {
            1.0
        } else {
            -1.0
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accuracy {
    pub geometric: f64,
    pub intersection: f64,
    pub tessellation: f64,
    pub exchange: f64,
}
impl Accuracy {
    pub fn validate(self) -> Result<(), GeometryError> {
        for value in [
            self.geometric,
            self.intersection,
            self.tessellation,
            self.exchange,
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(GeometryError::InvalidGeometry(
                    "accuracy budgets must be positive and finite".into(),
                ));
            }
        }
        if self.intersection > self.geometric {
            return Err(GeometryError::InvalidGeometry(
                "intersection budget exceeds geometric tolerance".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum GeometryQuality {
    Analytic,
    Approximate { max_error: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceSource {
    pub entity: String,
    pub body: String,
    pub key: String,
    pub face: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FaceRole {
    Authored,
    Preserved,
    Split,
    Cut,
    Coincident,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceProvenance {
    pub sources: Vec<FaceSource>,
    pub role: FaceRole,
    pub reversed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PcurveGeometry {
    Line2 {
        origin: UV,
        direction: UV,
    },
    Conic2 {
        origin: UV,
        axis_a: UV,
        axis_b: UV,
    },
    ProjectedCurve {
        curve: u32,
        surface: u32,
        chart: u32,
        uv_hint: UV,
        uv_rate: UV,
        parameter_origin: f64,
    },
    IntersectionSide {
        definition: u32,
        side: IntersectionSide,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum IntersectionSide {
    A,
    B,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryStore {
    pub surfaces: Vec<SurfaceGeometry>,
    pub curves: Vec<CurveGeometry>,
    pub pcurves: Vec<PcurveGeometry>,
    pub intersections: Vec<IntersectionDefinition>,
}
impl GeometryStore {
    pub fn new() -> Self {
        Self {
            surfaces: Vec::new(),
            curves: Vec::new(),
            pcurves: Vec::new(),
            intersections: Vec::new(),
        }
    }
    pub fn surface(&self, id: u32) -> Result<&SurfaceGeometry, GeometryError> {
        self.surfaces
            .get(id as usize)
            .ok_or_else(|| missing("surface", id))
    }
    pub fn curve(&self, id: u32) -> Result<CurveView<'_>, GeometryError> {
        let geometry = self
            .curves
            .get(id as usize)
            .ok_or_else(|| missing("curve", id))?;
        Ok(CurveView {
            geometry,
            store: self,
        })
    }
    pub fn pcurve_at(&self, id: u32, t: f64) -> Result<UV, GeometryError> {
        finite(t)?;
        let uv = match self
            .pcurves
            .get(id as usize)
            .ok_or_else(|| missing("pcurve", id))?
        {
            PcurveGeometry::Line2 { origin, direction } => {
                std::array::from_fn(|i| origin[i] + direction[i] * t)
            }
            PcurveGeometry::Conic2 {
                origin,
                axis_a,
                axis_b,
            } => std::array::from_fn(|i| origin[i] + axis_a[i] * t.cos() + axis_b[i] * t.sin()),
            PcurveGeometry::ProjectedCurve {
                curve,
                surface,
                chart,
                uv_hint,
                uv_rate,
                parameter_origin,
            } => {
                let surface = self.surface(*surface)?;
                if *chart as usize >= surface.charts().len() {
                    return Err(missing("chart", *chart));
                }
                let hint =
                    std::array::from_fn(|i| uv_hint[i] + uv_rate[i] * (t - parameter_origin));
                surface.project(self.curve(*curve)?.point_at(t)?, Some(hint))?
            }
            PcurveGeometry::IntersectionSide { definition, side } => {
                let point = self
                    .intersections
                    .get(*definition as usize)
                    .ok_or_else(|| missing("intersection", *definition))?
                    .evaluate(t, self)?;
                if *side == IntersectionSide::A {
                    point.uv_a
                } else {
                    point.uv_b
                }
            }
        };
        for v in uv {
            finite(v)?;
        }
        Ok(uv)
    }
}
impl Default for GeometryStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CurveView<'a> {
    pub geometry: &'a CurveGeometry,
    pub store: &'a GeometryStore,
}
impl Curve for CurveView<'_> {
    fn point_at(&self, t: f64) -> Result<Point3, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => Ok(self
                .store
                .intersections
                .get(*definition as usize)
                .ok_or_else(|| missing("intersection", *definition))?
                .evaluate(t, self.store)?
                .point),
            curve => curve.elementary_point(t),
        }
    }
    fn tangent_at(&self, t: f64) -> Result<Point3, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => Ok(self
                .store
                .intersections
                .get(*definition as usize)
                .ok_or_else(|| missing("intersection", *definition))?
                .evaluate(t, self.store)?
                .tangent),
            curve => curve.elementary_tangent(t),
        }
    }
    fn enclose(&self, range: Interval) -> Result<PatchBounds, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => self
                .store
                .intersections
                .get(*definition as usize)
                .ok_or_else(|| missing("intersection", *definition))?
                .enclose(range, self.store),
            curve => curve.elementary_bounds(range),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum EdgeGeometry {
    Curve { curve: u32, range: Interval },
    Collapsed { vertex: u32 },
}
impl EdgeGeometry {
    pub fn range(&self) -> Interval {
        match self {
            Self::Curve { range, .. } => *range,
            Self::Collapsed { .. } => Interval { lo: 0.0, hi: 1.0 },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vertex {
    pub id: u32,
    pub position: Point3,
    pub outgoing_halfedge: Option<u32>,
    pub tolerance: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub id: u32,
    pub geometry: EdgeGeometry,
    pub halfedge: u32,
    pub twin_halfedge: Option<u32>,
    pub tolerance: f64,
    pub chart_seam: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfEdgeGeometryUse {
    pub sense: Orientation,
    pub pcurve: Option<u32>,
    pub periodic_lift: [i32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfEdge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub twin: Option<u32>,
    pub next: Option<u32>,
    pub prev: Option<u32>,
    pub edge: u32,
    pub face: Option<u32>,
    pub loop_ref: Option<u32>,
    pub wire_ref: Option<u32>,
    pub geometry_use: HalfEdgeGeometryUse,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    pub id: u32,
    pub start_halfedge: u32,
    pub face_ref: u32,
    pub is_hole: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimRegion {
    pub chart: u32,
    pub uv_bounds: UVBox,
    pub outer: u32,
    pub holes: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    pub id: u32,
    pub key: String,
    pub surface: u32,
    pub sense: Orientation,
    pub trim: TrimRegion,
    pub shell_ref: Option<u32>,
    pub provenance: FaceProvenance,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub id: u32,
    pub start_halfedge: u32,
    pub is_closed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shell {
    pub id: u32,
    pub faces: Vec<u32>,
    pub is_closed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolidRegion {
    pub outer_shell: u32,
    pub cavity_shells: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topology {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub halfedges: Vec<HalfEdge>,
    pub loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub wires: Vec<Wire>,
    pub shells: Vec<Shell>,
}
impl Topology {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            halfedges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            wires: Vec::new(),
            shells: Vec::new(),
        }
    }
}
impl Default for Topology {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepEnvelope {
    pub schema_version: u32,
    pub id: String,
    pub revision: u64,
    pub geometry: GeometryStore,
    pub topology: Topology,
    pub solids: Vec<SolidRegion>,
    pub accuracy: Accuracy,
    pub quality: GeometryQuality,
}

fn missing(kind: &str, index: u32) -> GeometryError {
    GeometryError::MissingReference {
        kind: kind.into(),
        index,
    }
}
fn invalid(message: impl Into<String>) -> GeometryError {
    GeometryError::InvalidTopology(message.into())
}
fn positive(value: f64) -> Result<(), GeometryError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "entity tolerance must be finite and positive".into(),
        ));
    }
    Ok(())
}

impl BrepEnvelope {
    pub fn face_normal_at(&self, face_id: u32, point: Point3) -> Result<Point3, GeometryError> {
        let face = self
            .topology
            .faces
            .get(face_id as usize)
            .ok_or_else(|| missing("face", face_id))?;
        let surface = self.geometry.surface(face.surface)?;
        let uv = surface.project(point, None)?;
        Ok(super::geometry::unit(surface.normal_at(uv)?)?.map(|v| v * face.sense.multiplier()))
    }

    pub fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
        accuracy.validate()?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            id,
            revision: 0,
            geometry: GeometryStore::new(),
            topology: Topology::new(),
            solids: Vec::new(),
            accuracy,
            quality: GeometryQuality::Analytic,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, GeometryError> {
        if json.len() > 64 * 1024 * 1024 {
            return Err(GeometryError::LimitExceeded(
                "BRep JSON exceeds 64 MiB".into(),
            ));
        }
        let brep: Self = serde_json::from_str(json)
            .map_err(|e| GeometryError::InvalidGeometry(format!("invalid BRep v2 JSON: {e}")))?;
        brep.validate()?;
        Ok(brep)
    }

    pub fn to_json(&self) -> Result<String, GeometryError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|e| GeometryError::InvalidGeometry(e.to_string()))
    }

    pub fn bounds(&self) -> Result<Option<PatchBounds>, GeometryError> {
        self.validate()?;
        self.bounds_unchecked()
    }

    pub(crate) fn bounds_unchecked(&self) -> Result<Option<PatchBounds>, GeometryError> {
        let mut result: Option<PatchBounds> = None;
        let mut include = |bounds: PatchBounds| {
            result = Some(match result {
                Some(current) => PatchBounds {
                    axes: std::array::from_fn(|i| current.axes[i].hull(bounds.axes[i])),
                },
                None => bounds,
            });
        };
        for face in &self.topology.faces {
            include(
                self.geometry
                    .surface(face.surface)?
                    .enclose(face.trim.uv_bounds)?,
            );
        }
        for edge in &self.topology.edges {
            if let EdgeGeometry::Curve { curve, range } = edge.geometry {
                include(self.geometry.curve(curve)?.enclose(range)?);
            }
        }
        for vertex in &self.topology.vertices {
            include(PatchBounds {
                axes: [
                    Interval::point(vertex.position[0])?,
                    Interval::point(vertex.position[1])?,
                    Interval::point(vertex.position[2])?,
                ],
            });
        }
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), GeometryError> {
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
        for s in &self.geometry.surfaces {
            s.validate()?;
        }
        for c in &self.geometry.curves {
            c.validate()?;
            if let CurveGeometry::Intersection { definition } = c {
                self.geometry
                    .intersections
                    .get(*definition as usize)
                    .ok_or_else(|| missing("intersection", *definition))?;
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
                    self.geometry
                        .intersections
                        .get(*definition as usize)
                        .ok_or_else(|| missing("intersection", *definition))?;
                }
            }
        }
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
        }
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
            for &id in list {
                let h = &t.halfedges[id as usize];
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
            }
        }
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
        if t.halfedges
            .iter()
            .any(|h| h.face.is_some() && !loop_uses.contains(&h.id))
        {
            return Err(invalid("orphan face halfedge"));
        }
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
        if t.halfedges
            .iter()
            .any(|h| h.wire_ref.is_some() && !wire_uses.contains(&h.id))
        {
            return Err(invalid("orphan wire halfedge"));
        }
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

    fn validate_face_use(
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
            if !face.trim.uv_bounds[0].contains(uv[0]) || !face.trim.uv_bounds[1].contains(uv[1]) {
                let clipped = std::array::from_fn(|axis| {
                    uv[axis].clamp(face.trim.uv_bounds[axis].lo, face.trim.uv_bounds[axis].hi)
                });
                let jet = surface.derivatives(clipped)?;
                for axis in 0..2 {
                    let excess = (uv[axis] - clipped[axis]).abs();
                    let metric = norm(if axis == 0 { jet.du } else { jet.dv });
                    let roundoff =
                        128.0 * f64::EPSILON * uv[axis].abs().max(clipped[axis].abs()).max(1.0);
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

    fn validate_uv_join(
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
        let error = norm(super::geometry::add(
            super::geometry::scale(jet.du, displacement[0]),
            super::geometry::scale(jet.dv, displacement[1]),
        ));
        if error > self.accuracy.geometric {
            return Err(invalid("pcurve loop is discontinuous"));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    #[test]
    fn picked_support_normals_follow_curvature_reversal_and_singularities() {
        use crate::analytic::{
            booleans::{boolean_spheres, BooleanOp},
            geometry::{dot, sub, unit},
            primitives, Frame3,
        };
        let accuracy = Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        };
        let frame = Frame3::from_axis([3.0, -2.0, 1.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap();
        let sphere = primitives::sphere("host".into(), frame.clone(), 2.0, accuracy).unwrap();
        let point = sphere.geometry.surfaces[0].point_at([0.7, 0.3]).unwrap();
        let inward_mesh_point =
            std::array::from_fn(|i| frame.origin[i] + (point[i] - frame.origin[i]) * 0.99);
        let normal = sphere.face_normal_at(0, inward_mesh_point).unwrap();
        assert!(dot(normal, unit(sub(point, frame.origin)).unwrap()) > 1.0 - 1e-12);
        let cutter = primitives::sphere("cutter".into(), frame.clone(), 0.5, accuracy).unwrap();
        let cavity = boolean_spheres(&sphere, &cutter, BooleanOp::Subtraction, "cavity".into())
            .unwrap()
            .brep;
        let face = cavity
            .topology
            .faces
            .iter()
            .find(|face| face.provenance.role == FaceRole::Cut)
            .unwrap();
        let point = cavity
            .geometry
            .surface(face.surface)
            .unwrap()
            .point_at([0.7, 0.3])
            .unwrap();
        assert!(
            dot(
                cavity.face_normal_at(face.id, point).unwrap(),
                unit(sub(point, frame.origin)).unwrap()
            ) < -1.0 + 1e-12
        );
        assert!(matches!(
            sphere.face_normal_at(u32::MAX, point),
            Err(GeometryError::MissingReference { .. })
        ));
        assert!(sphere.face_normal_at(0, [f64::NAN, 0.0, 0.0]).is_err());
        let cone = primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy).unwrap();
        let face = cone
            .topology
            .faces
            .iter()
            .find(|face| {
                matches!(
                    cone.geometry.surfaces[face.surface as usize],
                    SurfaceGeometry::Cone { .. }
                )
            })
            .unwrap();
        let apex = cone.geometry.surface(face.surface).unwrap().frame().origin;
        assert!(matches!(
            cone.face_normal_at(face.id, apex),
            Err(GeometryError::SingularParameterization)
        ));
    }
    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-9,
            intersection: 1e-10,
            tessellation: 1e-3,
            exchange: 1e-5,
        }
    }
    pub fn circular_face() -> BrepEnvelope {
        let mut b = BrepEnvelope::new("circle".into(), accuracy()).unwrap();
        b.geometry.surfaces.push(SurfaceGeometry::Plane {
            frame: super::super::Frame3::IDENTITY,
        });
        b.geometry.curves.push(CurveGeometry::Circle {
            frame: super::super::Frame3::IDENTITY,
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
    #[test]
    fn full_circle_loop_is_valid_and_strictly_versioned() {
        let b = circular_face();
        b.validate().unwrap();
        let json = b.to_json().unwrap();
        BrepEnvelope::from_json(&json).unwrap();
        let mut bad: serde_json::Value = serde_json::from_str(&json).unwrap();
        bad["schema_version"] = serde_json::json!(1);
        assert!(matches!(
            BrepEnvelope::from_json(&bad.to_string()),
            Err(GeometryError::UnsupportedSchema { found: 1 })
        ));
        bad.as_object_mut().unwrap().remove("schema_version");
        assert!(BrepEnvelope::from_json(&bad.to_string()).is_err());
    }
    #[test]
    fn missing_pcurve_and_malformed_cycle_are_rejected() {
        let mut b = circular_face();
        b.topology.halfedges[0].geometry_use.pcurve = None;
        assert!(b.validate().is_err());
        let mut b = circular_face();
        b.topology.halfedges[0].next = Some(10);
        assert!(b.validate().is_err());
        let mut b = circular_face();
        b.geometry.pcurves[0] = PcurveGeometry::Line2 {
            origin: [0.0; 2],
            direction: [1.0, 0.0],
        };
        assert!(b.validate().is_err());
    }
    #[test]
    fn pcurve_outside_uv_bounds_beyond_geometric_tolerance_is_rejected() {
        let mut b = circular_face();
        b.topology.faces[0].trim.uv_bounds[0] = Interval::new(-0.999, 0.999).unwrap();
        assert!(matches!(
            b.validate(),
            Err(GeometryError::InvalidTopology(message)) if message.contains("leaves face 0 UV bounds")
        ));
    }
    #[test]
    fn old_json_and_unknown_fields_are_not_accepted() {
        assert!(BrepEnvelope::from_json("{\"vertices\":[],\"faces\":[]}").is_err());
        let mut json: serde_json::Value =
            serde_json::from_str(&circular_face().to_json().unwrap()).unwrap();
        json["legacy"] = serde_json::json!(true);
        assert!(BrepEnvelope::from_json(&json.to_string()).is_err());
    }
    #[test]
    fn malformed_charts_and_undeclared_degenerate_edges_are_rejected() {
        let mut b = circular_face();
        b.topology.faces[0].trim.chart = u32::MAX;
        assert!(
            matches!(b.validate(), Err(GeometryError::MissingReference { kind, .. }) if kind == "chart")
        );
        let mut b = circular_face();
        b.geometry.curves[0] = CurveGeometry::Line {
            origin: [1.0, 0.0, 0.0],
            direction: [1.0, 0.0, 0.0],
        };
        b.topology.edges[0].geometry = EdgeGeometry::Curve {
            curve: 0,
            range: Interval::new(0.0, 1e-12).unwrap(),
        };
        b.geometry.pcurves[0] = PcurveGeometry::Line2 {
            origin: [1.0, 0.0],
            direction: [1.0, 0.0],
        };
        assert!(
            matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("regular line"))
        );
        let mut b = circular_face();
        b.accuracy.geometric = 10.0;
        b.topology.edges[0].geometry = EdgeGeometry::Collapsed { vertex: 0 };
        b.geometry.pcurves[0] = PcurveGeometry::Line2 {
            origin: [1.0, 0.0],
            direction: [0.0; 2],
        };
        assert!(
            matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("surface singularity"))
        );
        let mut b = circular_face();
        b.topology.vertices[0].tolerance = 1e-18;
        b.topology.edges[0].tolerance = 1e-18;
        assert!(
            matches!(b.validate(), Err(GeometryError::InvalidTopology(message)) if message.contains("edge endpoints"))
        );
    }
}
