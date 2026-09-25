use super::{
    booleans::{boolean_brep, shell_brep, subtract_planar_cutters, BooleanOp, BooleanReport},
    primitives,
    query::{classify_point, PointClassification},
    tessellation::{Tessellation, TessellationCache},
    topology::{Accuracy, BrepEnvelope},
    CurveGeometry, Frame3, GeometryError,
};
use crate::export::projection::{project_analytic_brep_to_scene, CameraParameters, HlrOptions};
use serde::Deserialize;
use std::{cell::RefCell, sync::Arc};
use wasm_bindgen::prelude::*;

const DEFAULT_MAX_TRIANGLES: usize = 2_000_000;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TessellationOptions {
    max_triangles: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnnularSectorOpeningConfig {
    id: String,
    angle: f64,
    width: f64,
    bottom: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxArchedOpeningConfig {
    id: String,
    station: f64,
    width: f64,
    bottom: f64,
    height: f64,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PolygonLoftAlignmentConfig {
    Auto,
    Indexed {
        upper_start: usize,
        reverse_upper: bool,
    },
}

fn tessellation_limit(options_json: Option<String>) -> Result<usize, GeometryError> {
    let options = match options_json {
        Some(json) if !json.trim().is_empty() => serde_json::from_str::<TessellationOptions>(&json)
            .map_err(|error| GeometryError::InvalidGeometry(error.to_string()))?,
        _ => TessellationOptions::default(),
    };
    let limit = options.max_triangles.unwrap_or(DEFAULT_MAX_TRIANGLES);
    if limit == 0 || limit > DEFAULT_MAX_TRIANGLES {
        return Err(GeometryError::LimitExceeded(format!(
            "max_triangles must be between 1 and {DEFAULT_MAX_TRIANGLES}"
        )));
    }
    Ok(limit)
}

fn tessellate_cached(
    brep: &BrepEnvelope,
    deflection: f64,
    max_triangles: usize,
) -> Result<OGAnalyticTessellation, GeometryError> {
    let mesh = CACHE.with(|cache| {
        cache
            .try_borrow_mut()
            .map_err(|_| {
                GeometryError::LimitExceeded("tessellation cache is already in use".into())
            })?
            .tessellate(brep, deflection, max_triangles)
    })?;
    Ok(OGAnalyticTessellation { mesh })
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PrimitiveConfig {
    Cuboid {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        size: [f64; 3],
    },
    LinearExtrusion {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        outer: Vec<[f64; 2]>,
        holes: Vec<Vec<[f64; 2]>>,
        height: f64,
    },
    ArcEdgedExtrusion {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        outer: Vec<primitives::ProfileEdge>,
        #[serde(default)]
        holes: Vec<Vec<primitives::ProfileEdge>>,
        height: f64,
    },
    PolygonLoft {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        lower: Vec<[f64; 3]>,
        upper: Vec<[f64; 3]>,
        alignment: PolygonLoftAlignmentConfig,
    },
    BoxWithArchedOpening {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        width: f64,
        depth: f64,
        height: f64,
        opening: BoxArchedOpeningConfig,
    },
    PlanarPolyhedron {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        vertices: Vec<[f64; 3]>,
        faces: Vec<Vec<u32>>,
    },
    Arc {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    EllipticalArc {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        major_radius: f64,
        minor_radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    Cylinder {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        height: f64,
    },
    CylinderSector {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    Sphere {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
    },
    Cone {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        height: f64,
    },
    Frustum {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        lower_radius: f64,
        upper_radius: f64,
        height: f64,
    },
    Torus {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        major_radius: f64,
        minor_radius: f64,
    },
    AnnularSectorExtrusion {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        thickness: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    AnnularSectorExtrusionWithOpenings {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        thickness: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
        openings: Vec<AnnularSectorOpeningConfig>,
    },
    AnnularSectorExtrusionWithArchedOpening {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        thickness: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
        opening: AnnularSectorOpeningConfig,
    },
    CoaxialCircleLoft {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        lower_radius: f64,
        upper_radius: f64,
        height: f64,
    },
    RevolvedRectangle {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        inner_radius: f64,
        outer_radius: f64,
        height: f64,
    },
    ChamferedCuboid {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        size: [f64; 3],
        chamfer: f64,
    },
    FilletedCylinder {
        id: String,
        frame: Frame3,
        accuracy: Accuracy,
        radius: f64,
        height: f64,
        fillet_radius: f64,
    },
}
impl PrimitiveConfig {
    fn build(self) -> Result<BrepEnvelope, GeometryError> {
        match self {
            Self::Cuboid {
                id,
                frame,
                accuracy,
                size,
            } => primitives::cuboid(id, frame, size, accuracy),
            Self::LinearExtrusion {
                id,
                frame,
                accuracy,
                outer,
                holes,
                height,
            } => primitives::linear_extrusion(id, frame, outer, holes, height, accuracy),
            Self::ArcEdgedExtrusion {
                id,
                frame,
                accuracy,
                outer,
                holes,
                height,
            } => primitives::arc_edged_extrusion_with_holes(
                id, frame, outer, holes, height, accuracy,
            ),
            Self::PolygonLoft {
                id,
                frame,
                accuracy,
                lower,
                upper,
                alignment,
            } => super::modeling::polygon_loft(
                id,
                lower.into_iter().map(|point| frame.point(point)).collect(),
                upper.into_iter().map(|point| frame.point(point)).collect(),
                match alignment {
                    PolygonLoftAlignmentConfig::Auto => super::modeling::PolygonLoftAlignment::Auto,
                    PolygonLoftAlignmentConfig::Indexed {
                        upper_start,
                        reverse_upper,
                    } => super::modeling::PolygonLoftAlignment::Indexed {
                        upper_start,
                        reverse_upper,
                    },
                },
                accuracy,
            ),
            Self::BoxWithArchedOpening {
                id,
                frame,
                accuracy,
                width,
                depth,
                height,
                opening,
            } => primitives::box_with_arched_opening(
                id,
                frame,
                width,
                depth,
                height,
                primitives::BoxArchedOpening {
                    id: opening.id,
                    station: opening.station,
                    width: opening.width,
                    bottom: opening.bottom,
                    height: opening.height,
                },
                accuracy,
            ),
            Self::PlanarPolyhedron {
                id,
                frame,
                accuracy,
                vertices,
                faces,
            } => {
                frame.validate()?;
                primitives::planar_polyhedron(
                    id,
                    vertices
                        .into_iter()
                        .map(|point| frame.point(point))
                        .collect(),
                    faces,
                    accuracy,
                )
            }
            Self::Arc {
                id,
                frame,
                accuracy,
                radius,
                start_angle,
                sweep_angle,
            } => primitives::arc_wire(
                id,
                CurveGeometry::Circle { frame, radius },
                start_angle,
                sweep_angle,
                accuracy,
            ),
            Self::EllipticalArc {
                id,
                frame,
                accuracy,
                major_radius,
                minor_radius,
                start_angle,
                sweep_angle,
            } => primitives::arc_wire(
                id,
                CurveGeometry::Ellipse {
                    frame,
                    major_radius,
                    minor_radius,
                },
                start_angle,
                sweep_angle,
                accuracy,
            ),
            Self::Cylinder {
                id,
                frame,
                accuracy,
                radius,
                height,
            } => primitives::cylinder(id, frame, radius, height, accuracy),
            Self::CylinderSector {
                id,
                frame,
                accuracy,
                radius,
                height,
                start_angle,
                sweep_angle,
            } => primitives::cylinder_sector(
                id,
                frame,
                radius,
                height,
                start_angle,
                sweep_angle,
                accuracy,
            ),
            Self::Sphere {
                id,
                frame,
                accuracy,
                radius,
            } => primitives::sphere(id, frame, radius, accuracy),
            Self::Cone {
                id,
                frame,
                accuracy,
                radius,
                height,
            } => primitives::cone(id, frame, radius, height, accuracy),
            Self::Frustum {
                id,
                frame,
                accuracy,
                lower_radius,
                upper_radius,
                height,
            } => primitives::frustum(id, frame, lower_radius, upper_radius, height, accuracy),
            Self::Torus {
                id,
                frame,
                accuracy,
                major_radius,
                minor_radius,
            } => primitives::torus(id, frame, major_radius, minor_radius, accuracy),
            Self::AnnularSectorExtrusion {
                id,
                frame,
                accuracy,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
            } => primitives::annular_sector_extrusion(
                id,
                frame,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
                accuracy,
            ),
            Self::AnnularSectorExtrusionWithOpenings {
                id,
                frame,
                accuracy,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
                openings,
            } => primitives::annular_sector_extrusion_with_openings(
                id,
                frame,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
                openings
                    .into_iter()
                    .map(|opening| primitives::AnnularSectorOpening {
                        id: opening.id,
                        angle: opening.angle,
                        width: opening.width,
                        bottom: opening.bottom,
                        height: opening.height,
                    })
                    .collect(),
                accuracy,
            ),
            Self::AnnularSectorExtrusionWithArchedOpening {
                id,
                frame,
                accuracy,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
                opening,
            } => primitives::annular_sector_extrusion_with_arched_opening(
                id,
                frame,
                radius,
                thickness,
                height,
                start_angle,
                sweep_angle,
                primitives::AnnularSectorOpening {
                    id: opening.id,
                    angle: opening.angle,
                    width: opening.width,
                    bottom: opening.bottom,
                    height: opening.height,
                },
                accuracy,
            ),
            Self::CoaxialCircleLoft {
                id,
                frame,
                accuracy,
                lower_radius,
                upper_radius,
                height,
            } => super::modeling::coaxial_circle_loft(
                id,
                frame,
                lower_radius,
                upper_radius,
                height,
                accuracy,
            ),
            Self::RevolvedRectangle {
                id,
                frame,
                accuracy,
                inner_radius,
                outer_radius,
                height,
            } => super::modeling::revolve_rectangle(
                id,
                frame,
                inner_radius,
                outer_radius,
                height,
                accuracy,
            ),
            Self::ChamferedCuboid {
                id,
                frame,
                accuracy,
                size,
                chamfer,
            } => super::modeling::chamfered_cuboid(id, frame, size, chamfer, accuracy),
            Self::FilletedCylinder {
                id,
                frame,
                accuracy,
                radius,
                height,
                fillet_radius,
            } => super::modeling::filleted_cylinder(
                id,
                frame,
                radius,
                height,
                fillet_radius,
                accuracy,
            ),
        }
    }
}

fn js_error(error: GeometryError) -> JsValue {
    JsValue::from_str(
        &serde_json::to_string(&error)
            .unwrap_or_else(|_| "analytic geometry error could not be serialized".into()),
    )
}

#[wasm_bindgen(js_name = validateAnalyticBrep)]
pub fn validate_analytic_brep(serialized: &str) -> Result<String, JsValue> {
    serde_json::to_string(&super::diagnostics::validate_json(serialized))
        .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))
}

#[wasm_bindgen(js_name = tessellate_brep)]
pub fn tessellate_brep(
    serialized: &str,
    deflection: f64,
    options_json: Option<String>,
) -> Result<OGAnalyticTessellation, JsValue> {
    let brep = BrepEnvelope::from_json(serialized).map_err(js_error)?;
    let max_triangles = tessellation_limit(options_json).map_err(js_error)?;
    tessellate_cached(&brep, deflection, max_triangles).map_err(js_error)
}

#[wasm_bindgen]
pub struct OGAnalyticBrep {
    brep: BrepEnvelope,
    boolean_report: Option<BooleanReport>,
}
thread_local! {
    static CACHE: RefCell<TessellationCache> = RefCell::new(TessellationCache::new(64 * 1024 * 1024));
}
impl OGAnalyticBrep {
    fn from_brep(brep: BrepEnvelope) -> Self {
        Self {
            brep,
            boolean_report: None,
        }
    }
}
#[wasm_bindgen]
impl OGAnalyticBrep {
    #[wasm_bindgen(constructor)]
    pub fn new(serialized: &str) -> Result<OGAnalyticBrep, JsValue> {
        Ok(Self::from_brep(
            BrepEnvelope::from_json(serialized).map_err(js_error)?,
        ))
    }

    pub fn from_primitive(config_json: &str) -> Result<OGAnalyticBrep, JsValue> {
        if config_json.len() > 64 * 1024 {
            return Err(js_error(GeometryError::LimitExceeded(
                "primitive config exceeds 64 KiB".into(),
            )));
        }
        let config: PrimitiveConfig = serde_json::from_str(config_json)
            .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))?;
        Ok(Self::from_brep(config.build().map_err(js_error)?))
    }

    pub fn get_brep_serialized(&self) -> Result<String, JsValue> {
        self.brep.to_json().map_err(js_error)
    }

    pub fn export_step(&self, length_unit: &str) -> Result<String, JsValue> {
        let (text, report) =
            super::exchange::export_step(&self.brep, length_unit).map_err(js_error)?;
        serde_json::to_string(&serde_json::json!({ "text": text, "report": report }))
            .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))
    }

    pub fn placed(&self, frame_json: &str, scale: f64) -> Result<OGAnalyticBrep, JsValue> {
        if frame_json.len() > 4096 {
            return Err(js_error(GeometryError::LimitExceeded(
                "placement frame exceeds 4 KiB".into(),
            )));
        }
        let frame: Frame3 = serde_json::from_str(frame_json)
            .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))?;
        Ok(Self::from_brep(
            super::placement::placed(&self.brep, frame, scale).map_err(js_error)?,
        ))
    }

    pub fn prepare_ifc_exchange(&self) -> Result<String, JsValue> {
        let body = super::ifc_exchange::prepare_ifc_body(&self.brep).map_err(js_error)?;
        serde_json::to_string(&body)
            .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))
    }

    pub fn face_normal_at(&self, face_id: u32, point: &[f64]) -> Result<Vec<f64>, JsValue> {
        let point: [f64; 3] = point.try_into().map_err(|_| {
            js_error(GeometryError::InvalidGeometry(
                "face normal query requires three coordinates".into(),
            ))
        })?;
        self.brep
            .face_normal_at(face_id, point)
            .map(|normal| normal.to_vec())
            .map_err(js_error)
    }

    pub fn classify_point(&self, point: &[f64]) -> Result<String, JsValue> {
        let point: [f64; 3] = point.try_into().map_err(|_| {
            js_error(GeometryError::InvalidGeometry(
                "point classification requires three coordinates".into(),
            ))
        })?;
        classify_point(&self.brep, point)
            .map(|classification| match classification {
                PointClassification::Inside => "inside",
                PointClassification::Outside => "outside",
                PointClassification::Boundary => "boundary",
                PointClassification::Unknown => "unknown",
            })
            .map(str::to_owned)
            .map_err(js_error)
    }

    pub fn project_to_2d_lines(
        &self,
        camera_json: &str,
        hlr_json: &str,
        deflection: f64,
    ) -> Result<String, JsValue> {
        let camera: CameraParameters = serde_json::from_str(camera_json)
            .map_err(|error| js_error(GeometryError::InvalidGeometry(error.to_string())))?;
        let hlr: HlrOptions = serde_json::from_str(hlr_json)
            .map_err(|error| js_error(GeometryError::InvalidGeometry(error.to_string())))?;
        let scene = project_analytic_brep_to_scene(
            &self.brep,
            &camera,
            &hlr,
            deflection,
            DEFAULT_MAX_TRIANGLES,
        )
        .map_err(js_error)?;
        serde_json::to_string(&scene.to_lines())
            .map_err(|error| js_error(GeometryError::InvalidGeometry(error.to_string())))
    }

    pub fn boolean_brep(
        &self,
        other: &OGAnalyticBrep,
        operation: &str,
        id: &str,
    ) -> Result<OGAnalyticBrep, JsValue> {
        let operation = match operation {
            "union" => BooleanOp::Union,
            "intersection" => BooleanOp::Intersection,
            "subtraction" => BooleanOp::Subtraction,
            _ => {
                return Err(js_error(GeometryError::InvalidGeometry(
                    "unknown boolean operation".into(),
                )))
            }
        };
        let result =
            boolean_brep(&self.brep, &other.brep, operation, id.into()).map_err(js_error)?;
        Ok(Self {
            brep: result.brep,
            boolean_report: Some(result.report),
        })
    }

    pub fn subtract_planar_cutters(
        &self,
        cutters_json: &str,
        id: &str,
    ) -> Result<OGAnalyticBrep, JsValue> {
        if cutters_json.len() > 16 * 1024 * 1024 {
            return Err(js_error(GeometryError::LimitExceeded(
                "planar cutter payload exceeds 16 MiB".into(),
            )));
        }
        let cutters: Vec<BrepEnvelope> = serde_json::from_str(cutters_json)
            .map_err(|error| js_error(GeometryError::InvalidGeometry(error.to_string())))?;
        let result = subtract_planar_cutters(&self.brep, &cutters, id.into()).map_err(js_error)?;
        Ok(Self {
            brep: result.brep,
            boolean_report: Some(result.report),
        })
    }

    pub fn shell(&self, thickness: f64, id: &str) -> Result<OGAnalyticBrep, JsValue> {
        let result = shell_brep(&self.brep, thickness, id.into()).map_err(js_error)?;
        Ok(Self {
            brep: result.brep,
            boolean_report: Some(result.report),
        })
    }

    pub fn get_boolean_report_serialized(&self) -> Result<Option<String>, JsValue> {
        self.boolean_report
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| js_error(GeometryError::InvalidGeometry(e.to_string())))
    }

    pub fn tessellate(&mut self, deflection: f64) -> Result<OGAnalyticTessellation, JsValue> {
        tessellate_cached(&self.brep, deflection, DEFAULT_MAX_TRIANGLES).map_err(js_error)
    }

    pub fn get_face_count(&self) -> u32 {
        self.brep.topology.faces.len() as u32
    }
    pub fn get_edge_count(&self) -> u32 {
        self.brep.topology.edges.len() as u32
    }
    pub fn get_id(&self) -> String {
        self.brep.id.clone()
    }
    pub fn get_bounds(&self) -> Result<Vec<f64>, JsValue> {
        Ok(match self.brep.bounds().map_err(js_error)? {
            Some(bounds) => bounds
                .axes
                .iter()
                .map(|i| i.lo)
                .chain(bounds.axes.iter().map(|i| i.hi))
                .collect(),
            None => Vec::new(),
        })
    }
    pub fn get_vertex_count(&self) -> u32 {
        self.brep.topology.vertices.len() as u32
    }
    pub fn get_revision(&self) -> u64 {
        self.brep.revision
    }
}

#[wasm_bindgen]
pub struct OGAnalyticTessellation {
    mesh: Arc<Tessellation>,
}
#[wasm_bindgen]
impl OGAnalyticTessellation {
    pub fn positions(&self) -> Vec<f64> {
        self.mesh.positions.clone()
    }
    pub fn normals(&self) -> Vec<f32> {
        self.mesh.normals.clone()
    }
    pub fn indices(&self) -> Vec<u32> {
        self.mesh.indices.clone()
    }
    pub fn triangle_face_ids(&self) -> Vec<u32> {
        self.mesh.triangle_face_ids.clone()
    }
    pub fn outline_positions(&self) -> Vec<f64> {
        self.mesh.outline_positions.clone()
    }
    pub fn outline_edge_ids(&self) -> Vec<u32> {
        self.mesh.outline_edge_ids.clone()
    }
    pub fn revision(&self) -> u64 {
        self.mesh.revision
    }
    pub fn achieved_deflection(&self) -> f64 {
        self.mesh.achieved_deflection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primitive_config_requires_all_geometry_fields() {
        let config = serde_json::json!({"kind":"cylinder","id":"c","frame":{"origin":[0,0,0],"x":[1,0,0],"y":[0,1,0],"z":[0,0,1]},"accuracy":{"geometric":1e-9,"intersection":1e-10,"tessellation":0.001,"exchange":1e-5},"radius":1,"height":2});
        let c: PrimitiveConfig = serde_json::from_value(config.clone()).unwrap();
        assert_eq!(c.build().unwrap().topology.faces.len(), 3);
        let mut incomplete = config;
        incomplete.as_object_mut().unwrap().remove("frame");
        assert!(serde_json::from_value::<PrimitiveConfig>(incomplete).is_err());
    }

    #[test]
    fn linear_extrusion_config_builds_strict_v2_geometry() {
        let config = serde_json::json!({
            "kind": "linear_extrusion",
            "id": "profile",
            "frame": Frame3::IDENTITY,
            "accuracy": {
                "geometric": 1e-9,
                "intersection": 1e-10,
                "tessellation": 0.001,
                "exchange": 1e-5
            },
            "outer": [[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]],
            "holes": [],
            "height": 1.5
        });
        let body = serde_json::from_value::<PrimitiveConfig>(config.clone())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(body.schema_version, 2);
        assert_eq!(body.topology.faces.len(), 6);
        let mut incomplete = config;
        incomplete.as_object_mut().unwrap().remove("holes");
        assert!(serde_json::from_value::<PrimitiveConfig>(incomplete).is_err());
    }

    #[test]
    fn arc_edged_extrusion_config_preserves_circle_edges() {
        let config = serde_json::json!({
            "kind": "arc_edged_extrusion",
            "id": "curved-profile",
            "frame": Frame3::IDENTITY,
            "accuracy": {
                "geometric": 1e-9,
                "intersection": 1e-10,
                "tessellation": 0.001,
                "exchange": 1e-5
            },
            "outer": [
                {"kind":"arc","center":[0.0,0.0],"radius":2.0,"start_angle":0.0,"sweep_angle":1.5707963267948966},
                {"kind":"line","from":[0.0,2.0],"to":[0.0,1.5]},
                {"kind":"arc","center":[0.0,0.0],"radius":1.5,"start_angle":1.5707963267948966,"sweep_angle":-1.5707963267948966},
                {"kind":"line","from":[1.5,0.0],"to":[2.0,0.0]}
            ],
            "height": 3.0
        });
        let body = serde_json::from_value::<PrimitiveConfig>(config)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(body.topology.faces.len(), 6);
        assert_eq!(
            body.geometry
                .surfaces
                .iter()
                .filter(|surface| matches!(
                    surface,
                    crate::analytic::SurfaceGeometry::Cylinder { .. }
                ))
                .count(),
            2
        );
    }

    #[test]
    fn polygon_loft_config_builds_capped_planar_brep() {
        let config = serde_json::json!({
            "kind": "polygon_loft",
            "id": "loft",
            "frame": Frame3::IDENTITY,
            "accuracy": {
                "geometric": 1e-9,
                "intersection": 1e-10,
                "tessellation": 0.001,
                "exchange": 1e-5
            },
            "lower": [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
            "upper": [[0.5, -0.5, 2.0], [0.5, 0.5, 2.0], [-0.5, 0.5, 2.0], [-0.5, -0.5, 2.0]],
            "alignment": { "kind": "auto" }
        });
        let body = serde_json::from_value::<PrimitiveConfig>(config)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(body.topology.faces.len(), 6);
        body.validate().unwrap();
    }

    #[test]
    fn annular_sector_extrusion_opening_config_is_strict_and_analytic() {
        let config = serde_json::json!({
            "kind": "annular_sector_extrusion_with_openings",
            "id": "host",
            "frame": Frame3::IDENTITY,
            "accuracy": {
                "geometric": 1e-9,
                "intersection": 1e-10,
                "tessellation": 0.001,
                "exchange": 1e-5
            },
            "radius": 3.0,
            "thickness": 0.3,
            "height": 3.0,
            "start_angle": 0.0,
            "sweep_angle": 2.0,
            "openings": [{
                "id": "raised_cutout",
                "angle": 1.0,
                "width": 0.8,
                "bottom": 0.9,
                "height": 1.2
            }]
        });
        let body = serde_json::from_value::<PrimitiveConfig>(config.clone())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(body.schema_version, 2);
        assert_eq!(body.topology.faces.len(), 10);
        assert_eq!(body.topology.faces[0].trim.holes.len(), 1);
        let mut incomplete = config;
        incomplete.as_object_mut().unwrap().remove("openings");
        assert!(serde_json::from_value::<PrimitiveConfig>(incomplete).is_err());
    }

    #[test]
    fn tessellation_options_are_strict_and_bounded() {
        assert_eq!(tessellation_limit(None).unwrap(), DEFAULT_MAX_TRIANGLES);
        assert_eq!(
            tessellation_limit(Some(r#"{"max_triangles":123}"#.into())).unwrap(),
            123
        );
        assert!(tessellation_limit(Some(r#"{"unknown":1}"#.into())).is_err());
        assert!(tessellation_limit(Some(r#"{"max_triangles":0}"#.into())).is_err());
    }
}
