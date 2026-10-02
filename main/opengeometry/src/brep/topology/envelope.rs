use super::entities::{EdgeGeometry, SolidRegion, Topology};
use crate::brep::accuracy::Accuracy;
use crate::brep::bounds::PatchBounds;
use crate::brep::error::{missing, GeometryError};
use crate::brep::frame::unit;
use crate::brep::geometry::{Curve, GeometryStore, Surface};
use crate::math::{Interval, Point3};
use serde::{Deserialize, Serialize};

pub(crate) const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum GeometryQuality {
    Analytic,
    Approximate { max_error: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrepEnvelope {
    pub(crate) schema_version: u32,
    pub id: String,
    pub revision: u64,
    pub geometry: GeometryStore,
    pub topology: Topology,
    pub(crate) solids: Vec<SolidRegion>,
    pub accuracy: Accuracy,
    pub(crate) quality: GeometryQuality,
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
        Ok(unit(surface.normal_at(uv)?)?.map(|v| v * face.sense.multiplier()))
    }

    pub(crate) fn new(id: String, accuracy: Accuracy) -> Result<Self, GeometryError> {
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
            .map_err(|e| GeometryError::InvalidGeometry(format!("invalid BRep JSON: {e}")))?;
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
}
