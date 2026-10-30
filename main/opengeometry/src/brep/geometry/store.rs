use super::curve::{Curve, CurveGeometry};
use super::intersection_definition::IntersectionDefinition;
use super::surface::{Surface, SurfaceGeometry};
use crate::brep::bounds::PatchBounds;
use crate::brep::error::{missing, GeometryError};
use crate::brep::frame::UV;
use crate::brep::pcurve::{IntersectionSide, PcurveGeometry};
use crate::math::{finite, Interval, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryStore {
    pub surfaces: Vec<SurfaceGeometry>,
    pub(crate) curves: Vec<CurveGeometry>,
    pub(crate) pcurves: Vec<PcurveGeometry>,
    pub(crate) intersections: Vec<IntersectionDefinition>,
}

pub(crate) struct CurveView<'a> {
    pub(crate) geometry: &'a CurveGeometry,
    store: &'a GeometryStore,
}

impl GeometryStore {
    pub(crate) fn new() -> Self {
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
    pub(crate) fn curve(&self, id: u32) -> Result<CurveView<'_>, GeometryError> {
        let geometry = self
            .curves
            .get(id as usize)
            .ok_or_else(|| missing("curve", id))?;
        Ok(CurveView {
            geometry,
            store: self,
        })
    }
    pub(crate) fn pcurve_at(&self, id: u32, t: f64) -> Result<UV, GeometryError> {
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
                let point = self.intersection(*definition)?.evaluate(t, self)?;
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
    pub(crate) fn intersection(&self, id: u32) -> Result<&IntersectionDefinition, GeometryError> {
        self.intersections
            .get(id as usize)
            .ok_or_else(|| missing("intersection", id))
    }
}
impl Default for GeometryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Curve for CurveView<'_> {
    fn point_at(&self, t: f64) -> Result<Point3, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => Ok(self
                .store
                .intersection(*definition)?
                .evaluate(t, self.store)?
                .point),
            curve => curve.elementary_point(t),
        }
    }
    fn tangent_at(&self, t: f64) -> Result<Point3, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => Ok(self
                .store
                .intersection(*definition)?
                .evaluate(t, self.store)?
                .tangent),
            curve => curve.elementary_tangent(t),
        }
    }
    fn enclose(&self, range: Interval) -> Result<PatchBounds, GeometryError> {
        match self.geometry {
            CurveGeometry::Intersection { definition } => self
                .store
                .intersection(*definition)?
                .enclose(range, self.store),
            curve => curve.elementary_bounds(range),
        }
    }
}
