mod cavities;
mod curves;
mod entities;
mod surfaces;
mod topology;

use super::preflight::preflight;
use crate::brep::{placed, BrepEnvelope, FaceProvenance, Frame3, GeometryError, GeometryQuality};
use entities::{Entities, IfcEntity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExchangeFace {
    face: u32,
    key: String,
    entity: u32,
    provenance: FaceProvenance,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticExchangeBodyV2 {
    schema_version: u32,
    revision: String,
    length_unit: String,
    up_axis: String,
    coordinate_space: String,
    representation_type: String,
    quality: GeometryQuality,
    geometric_tolerance: f64,
    exchange_error_bound: f64,
    validation_level: String,
    entities: Vec<IfcEntity>,
    solids: Vec<u32>,
    faces: Vec<ExchangeFace>,
}

pub fn export_ifc_body(
    input: &BrepEnvelope,
    conversion: Frame3,
) -> Result<AnalyticExchangeBodyV2, GeometryError> {
    let preflight_bound = preflight(input, 1.0)?;
    check_entity_estimate(input)?;
    let mut brep = placed(input, conversion, 1.0)?;
    let has_cavities = cavities::reverse_cavities(&mut brep)?;
    let mut entities = Entities::default();
    let surfaces = surfaces::add_surfaces(&mut entities, &brep)?;
    let edges = curves::add_edges(
        &mut entities,
        &brep,
        input.accuracy.exchange - preflight_bound,
    )?;
    let exchange_error_bound = preflight_bound + edges.fitting_error;
    let faces = topology::add_faces(&mut entities, &brep, &surfaces, &edges)?;
    let solids = topology::add_solids(&mut entities, &brep, &faces, has_cavities);
    Ok(AnalyticExchangeBodyV2 {
        schema_version: 2,
        revision: input.revision.to_string(),
        length_unit: "metre".into(),
        up_axis: "Z".into(),
        coordinate_space: "world".into(),
        representation_type: if has_cavities { "CSG" } else { "AdvancedBrep" }.into(),
        quality: input.quality.clone(),
        geometric_tolerance: input.accuracy.geometric,
        exchange_error_bound,
        validation_level: "v2 structure, sampled residuals and bounded analytic entity preparation; receiver schema validation required".into(),
        entities: entities.0,
        solids,
        faces: exchange_faces(&brep, &faces),
    })
}

fn check_entity_estimate(input: &BrepEnvelope) -> Result<(), GeometryError> {
    let topology = &input.topology;
    let estimated_entities = input.geometry.surfaces.len() * 20
        + input.geometry.curves.len() * 5
        + topology.vertices.len() * 2
        + topology.edges.len()
        + topology.halfedges.len()
        + topology.loops.len() * 2
        + topology.faces.len()
        + topology.shells.len()
        + input.solids.len() * 2
        + input
            .solids
            .iter()
            .map(|solid| solid.cavity_shells.len() * 2)
            .sum::<usize>();
    if estimated_entities > 100_000 {
        return Err(GeometryError::LimitExceeded(
            "IFC prepared entity limit exceeded".into(),
        ));
    }
    Ok(())
}

fn exchange_faces(brep: &BrepEnvelope, faces: &[u32]) -> Vec<ExchangeFace> {
    brep.topology
        .faces
        .iter()
        .map(|face| ExchangeFace {
            face: face.id,
            key: face.key.clone(),
            entity: faces[face.id as usize],
            provenance: face.provenance.clone(),
        })
        .collect()
}
