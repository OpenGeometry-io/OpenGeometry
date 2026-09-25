use super::graph::{Geometry, WorldError, WorldGraph};
use super::projection::legacy_in_world;
use crate::analytic::GeometryError;
use crate::brep::Brep;
use crate::export::ifc::{
    export_scene_entities_to_ifc_text, IfcEntityInput, IfcExportConfig, IfcExportReport,
};
use crate::export::step::{export_breps_to_step_text, StepExportConfig, StepExportReport};
use crate::export::stl::{export_breps_to_stl_bytes, StlExportConfig, StlExportReport};

#[cfg(not(target_arch = "wasm32"))]
use crate::export::ifc::export_scene_entities_to_ifc_file;
#[cfg(not(target_arch = "wasm32"))]
use crate::export::pdf::{export_scene_to_pdf_with_config, PdfExportConfig};
#[cfg(not(target_arch = "wasm32"))]
use crate::export::projection::{CameraParameters, HlrOptions};
#[cfg(not(target_arch = "wasm32"))]
use crate::export::step::export_breps_to_step_file;
#[cfg(not(target_arch = "wasm32"))]
use crate::export::stl::export_breps_to_stl_file;

struct LegacyBody {
    node: String,
    kind: String,
    brep: Brep,
}

fn export_error(error: impl std::fmt::Display) -> WorldError {
    WorldError::Export(error.to_string())
}

impl WorldGraph {
    fn legacy_bodies(&self, items: &[String]) -> Result<Vec<LegacyBody>, WorldError> {
        let mut bodies = Vec::new();
        for leaf in self.leaves_of(items)? {
            let definition = self
                .definitions
                .get(&leaf.definition)
                .ok_or_else(|| WorldError::UnknownDefinition(leaf.definition.clone()))?;
            let Geometry::Legacy(brep) = &definition.geometry else {
                return Err(WorldError::Geometry(GeometryError::UnsupportedGeometry(
                    format!(
                        "{} is analytic; export it with the analytic STEP/STL/IFC exporters",
                        leaf.node
                    ),
                )));
            };
            bodies.push(LegacyBody {
                kind: self.kind_of(&leaf.node)?.unwrap_or("brep").to_string(),
                brep: legacy_in_world(brep, &leaf.world),
                node: leaf.node,
            });
        }
        Ok(bodies)
    }

    pub fn export_stl(
        &self,
        items: &[String],
        config: &StlExportConfig,
    ) -> Result<(Vec<u8>, StlExportReport), WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_breps_to_stl_bytes(bodies.iter().map(|body| &body.brep), config)
            .map_err(export_error)
    }

    pub fn export_step(
        &self,
        items: &[String],
        config: &StepExportConfig,
    ) -> Result<(String, StepExportReport), WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_breps_to_step_text(bodies.iter().map(|body| &body.brep), config)
            .map_err(export_error)
    }

    pub fn export_ifc(
        &self,
        items: &[String],
        config: &IfcExportConfig,
    ) -> Result<(String, IfcExportReport), WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_scene_entities_to_ifc_text(ifc_entities(&bodies), config).map_err(export_error)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_stl_file(
        &self,
        items: &[String],
        file_path: &str,
        config: &StlExportConfig,
    ) -> Result<StlExportReport, WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_breps_to_stl_file(bodies.iter().map(|body| &body.brep), file_path, config)
            .map_err(export_error)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_step_file(
        &self,
        items: &[String],
        file_path: &str,
        config: &StepExportConfig,
    ) -> Result<StepExportReport, WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_breps_to_step_file(bodies.iter().map(|body| &body.brep), file_path, config)
            .map_err(export_error)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_ifc_file(
        &self,
        items: &[String],
        file_path: &str,
        config: &IfcExportConfig,
    ) -> Result<IfcExportReport, WorldError> {
        let bodies = self.legacy_bodies(items)?;
        export_scene_entities_to_ifc_file(ifc_entities(&bodies), file_path, config)
            .map_err(export_error)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn project_to_pdf(
        &self,
        items: &[String],
        camera: &CameraParameters,
        hlr: &HlrOptions,
        deflection: f64,
        file_path: &str,
        config: &PdfExportConfig,
    ) -> Result<(), WorldError> {
        let scene = self.project(items, camera, hlr, deflection)?;
        export_scene_to_pdf_with_config(&scene, file_path, config).map_err(export_error)
    }
}

fn ifc_entities(bodies: &[LegacyBody]) -> impl Iterator<Item = IfcEntityInput<'_>> {
    bodies.iter().map(|body| IfcEntityInput {
        entity_id: body.node.as_str(),
        kind: body.kind.as_str(),
        brep: &body.brep,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::primitives;
    use crate::analytic::topology::Accuracy;
    use crate::analytic::Frame3;
    use crate::brep::BrepBuilder;
    use crate::world::{NodeSpec, Similarity3};
    use openmaths::Vector3;
    use uuid::Uuid;

    fn tetrahedron() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, 0.8660254, 0.0),
            Vector3::new(0.5, 0.2886751, 0.8164966),
        ]);
        builder.add_face(&[0, 2, 1], &[]).unwrap();
        builder.add_face(&[0, 1, 3], &[]).unwrap();
        builder.add_face(&[1, 2, 3], &[]).unwrap();
        builder.add_face(&[2, 0, 3], &[]).unwrap();
        builder.build().unwrap()
    }

    fn triangle() -> Brep {
        let mut builder = BrepBuilder::new(Uuid::new_v4());
        builder.add_vertices(&[
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ]);
        builder.add_face(&[0, 1, 2], &[]).unwrap();
        builder.build().unwrap()
    }

    fn graph_with(id: &str, brep: Brep, kind: Option<&str>, origin: [f64; 3]) -> WorldGraph {
        let mut graph = WorldGraph::new();
        graph.define_legacy(id, brep).unwrap();
        graph
            .add_node(NodeSpec {
                id: id.into(),
                parent: None,
                definition: Some(id.into()),
                local: Similarity3 {
                    frame: Frame3 {
                        origin,
                        ..Frame3::IDENTITY
                    },
                    scale: 1.0,
                },
                kind: kind.map(str::to_string),
            })
            .unwrap();
        graph
    }

    #[test]
    fn stl_export_writes_legacy_triangles_in_world_space() {
        let graph = graph_with("tri", triangle(), None, [2.0, 0.0, 0.0]);
        let (bytes, report) = graph
            .export_stl(&["tri".into()], &StlExportConfig::default())
            .unwrap();
        assert_eq!(report.exported_triangles, 1);
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        assert_eq!(count, 1);
        let xs: Vec<f32> = (0..3)
            .map(|vertex| {
                let offset = 84 + 12 + vertex * 12;
                f32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ])
            })
            .collect();
        assert!(xs.iter().all(|x| *x >= 2.0 - 1e-6), "{xs:?}");
    }

    #[test]
    fn step_export_writes_a_manifold_solid() {
        let graph = graph_with("tetra", tetrahedron(), None, [0.0; 3]);
        let (text, report) = graph
            .export_step(&["tetra".into()], &StepExportConfig::default())
            .unwrap();
        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("MANIFOLD_SOLID_BREP"));
        assert_eq!(report.exported_solids, 1);
    }

    #[test]
    fn ifc_export_writes_one_element_per_legacy_node() {
        let graph = graph_with("tetra", tetrahedron(), Some("wall"), [0.0; 3]);
        let (text, report) = graph
            .export_ifc(&["tetra".into()], &IfcExportConfig::default())
            .unwrap();
        assert!(text.contains("FILE_SCHEMA(('IFC4'));"));
        assert!(text.contains("IFCTRIANGULATEDFACESET("));
        assert_eq!(report.exported_elements, 1);
    }

    #[test]
    fn analytic_items_are_refused_by_the_legacy_exporters() {
        let accuracy = Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        };
        let mut graph = WorldGraph::new();
        graph
            .define(
                "box",
                primitives::cuboid("box".into(), Frame3::IDENTITY, [1.0; 3], accuracy).unwrap(),
            )
            .unwrap();
        graph
            .add_node(NodeSpec {
                id: "box".into(),
                parent: None,
                definition: Some("box".into()),
                local: Similarity3::IDENTITY,
                kind: None,
            })
            .unwrap();
        assert!(matches!(
            graph.export_stl(&["box".into()], &StlExportConfig::default()),
            Err(WorldError::Geometry(GeometryError::UnsupportedGeometry(_)))
        ));
    }
}
