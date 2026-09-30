use super::error::{ErrorCode, ErrorContext, ErrorDetails, GraphError};
use super::graph::WorldGraph;
use super::node::NodeKind;
use super::placement::is_identity;
use crate::brep::{placed, BodyType, BrepEnvelope, Frame3, GeometryError, Similarity3};
use crate::exchange::{
    export_bodies, StepBodyInput, StepExportFailure, StepExportReport, StepSkipped,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct StepOptions {
    pub unit: String,
    pub up_axis: String,
    pub name: String,
    pub timestamp: String,
}

impl Default for StepOptions {
    fn default() -> Self {
        Self {
            unit: "millimetre".into(),
            up_axis: "Z".into(),
            name: "opengeometry-export".into(),
            timestamp: "1970-01-01T00:00:00".into(),
        }
    }
}

impl WorldGraph {
    fn export_world_placement(&self, og_id: &str) -> Result<Similarity3, GraphError> {
        let mut path = Vec::new();
        let mut cursor = Some(og_id);
        while let Some(current) = cursor {
            let node = self.node(current)?;
            path.push(node.local);
            cursor = node.parent.as_deref();
        }
        let mut world = Similarity3::IDENTITY;
        for local in path.iter().rev() {
            world = world.compose(local);
        }
        world.validate()?;
        Ok(world)
    }

    fn expand_export(
        &self,
        og_id: &str,
        direct: bool,
        seen: &mut BTreeSet<String>,
        selected: &mut Vec<String>,
        skipped: &mut Vec<StepSkipped>,
    ) -> Result<(), GraphError> {
        let node = self.node(og_id)?;
        if !seen.insert(node.og_id.clone()) {
            return Ok(());
        }
        if node.kind == NodeKind::SystemAssembly {
            for child in &node.children {
                self.expand_export(child, false, seen, selected, skipped)?;
            }
            return Ok(());
        }
        let shape = self.shape(og_id)?;
        match shape.brep.body_type()? {
            BodyType::Solid => selected.push(og_id.into()),
            BodyType::Wire if direct => {
                return Err(GraphError::code(
                    ErrorCode::InvalidOperand,
                    "a Wire cannot be exported directly",
                ))
            }
            BodyType::Wire => skipped.push(StepSkipped {
                og_id: og_id.into(),
                reason: "Wire".into(),
            }),
            BodyType::Sheet if direct => {
                return Err(GraphError::code(
                    ErrorCode::InvalidOperand,
                    "a Sheet cannot be exported directly",
                ))
            }
            BodyType::Sheet => skipped.push(StepSkipped {
                og_id: og_id.into(),
                reason: "Sheet".into(),
            }),
        }
        Ok(())
    }

    pub fn export_step(
        &self,
        nodes: &[String],
        options: &StepOptions,
    ) -> Result<(String, StepExportReport), GraphError> {
        self.try_export_step(nodes, options)
            .map_err(|error| error.in_context(ErrorContext::Export))
    }

    fn try_export_step(
        &self,
        nodes: &[String],
        options: &StepOptions,
    ) -> Result<(String, StepExportReport), GraphError> {
        if nodes.is_empty() {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "STEP export requires at least one node",
            ));
        }
        if !matches!(options.unit.as_str(), "metre" | "millimetre")
            || !matches!(options.up_axis.as_str(), "Y" | "Z")
            || !valid_timestamp(&options.timestamp)
            || options.name.is_empty()
            || options.name.len() > 4096
        {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "invalid STEP export options",
            ));
        }
        let (selected, skipped) = self.step_selection(nodes)?;
        if selected.is_empty() {
            return Err(GraphError::code(
                ErrorCode::EmptyResult,
                "STEP selection has no solids",
            ));
        }
        let conversion = if options.up_axis == "Z" {
            Similarity3 {
                frame: Frame3 {
                    origin: [0.0; 3],
                    x: [1.0, 0.0, 0.0],
                    y: [0.0, 0.0, 1.0],
                    z: [0.0, -1.0, 0.0],
                },
                scale: 1.0,
            }
        } else {
            Similarity3::IDENTITY
        };
        let owned = self.placed_bodies(selected, conversion)?;
        let inputs = owned
            .iter()
            .map(|(og_id, shape_id, revision, brep)| StepBodyInput {
                og_id,
                shape_id,
                shape_revision: *revision,
                brep,
            })
            .collect::<Vec<_>>();
        export_bodies(
            &inputs,
            &options.unit,
            &options.up_axis,
            &options.name,
            &options.timestamp,
            skipped,
        )
        .map_err(|failure| body_error(failure, &owned))
    }

    fn step_selection(
        &self,
        nodes: &[String],
    ) -> Result<(Vec<String>, Vec<StepSkipped>), GraphError> {
        let mut seen = BTreeSet::new();
        let mut selected = Vec::new();
        let mut skipped = Vec::new();
        for og_id in nodes {
            let node = self.node(og_id)?;
            if node.kind == NodeKind::Body
                && self.shape(og_id)?.brep.body_type()? != BodyType::Solid
            {
                return Err(GraphError::code(
                    ErrorCode::InvalidOperand,
                    "a non-solid body cannot be exported directly",
                ));
            }
            self.expand_export(og_id, true, &mut seen, &mut selected, &mut skipped)?;
        }
        Ok((selected, skipped))
    }

    fn placed_bodies(
        &self,
        selected: Vec<String>,
        conversion: Similarity3,
    ) -> Result<Vec<(String, String, u64, BrepEnvelope)>, GraphError> {
        let mut owned = Vec::<(String, String, u64, BrepEnvelope)>::with_capacity(selected.len());
        for og_id in selected {
            let node = self.node(&og_id)?;
            let shape_id = node
                .shape
                .as_ref()
                .ok_or_else(|| GraphError::code(ErrorCode::InvalidTopology, "body has no shape"))?;
            let shape = self.shape(&og_id)?;
            let placement = conversion.compose(&self.export_world_placement(&og_id)?);
            let mut brep = if is_identity(placement) {
                (*shape.brep).clone()
            } else {
                placed(&shape.brep, placement.frame, placement.scale).map_err(
                    |error| match error {
                        GeometryError::LimitExceeded(message) => GraphError::code(
                            ErrorCode::LimitExceeded,
                            format!("{og_id}: {message}"),
                        ),
                        other => GraphError::Geometry(other),
                    },
                )?
            };
            brep.id = og_id.clone();
            brep.revision = shape.revision;
            brep.validate()?;
            owned.push((og_id, shape_id.clone(), shape.revision, brep));
        }
        Ok(owned)
    }
}

fn body_error(
    failure: StepExportFailure,
    owned: &[(String, String, u64, BrepEnvelope)],
) -> GraphError {
    let og_id = failure
        .body_index
        .and_then(|index| owned.get(index))
        .map(|body| body.0.clone());
    let source = GraphError::Geometry(failure.error);
    match og_id {
        Some(og_id) => GraphError::with_details(
            source.error_code(),
            source.message(),
            ErrorDetails::Export { og_id },
        ),
        None => source,
    }
}

fn valid_timestamp(timestamp: &str) -> bool {
    let bytes = timestamp.as_bytes();
    if bytes.len() != 19
        || [4, 7, 10, 13, 16]
            .into_iter()
            .zip([b'-', b'-', b'T', b':', b':'])
            .any(|(index, separator)| bytes[index] != separator)
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| ![4, 7, 10, 13, 16].contains(&index) && !byte.is_ascii_digit())
    {
        return false;
    }
    let year = timestamp[0..4].parse::<u32>().ok();
    let month = timestamp[5..7].parse::<u32>().ok();
    let day = timestamp[8..10].parse::<u32>().ok();
    let hour = timestamp[11..13].parse::<u32>().ok();
    let minute = timestamp[14..16].parse::<u32>().ok();
    let second = timestamp[17..19].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) =
        (year, month, day, hour, minute, second)
    else {
        return false;
    };
    if year == 0 || month == 0 || month > 12 || hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day > 0 && day <= days[(month - 1) as usize]
}
