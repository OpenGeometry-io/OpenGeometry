mod prismatic_profile;

use super::assembly::{analytic_face_mappings, append_analytic_input};
use super::dispatch::{boolean_brep_with_handlers, record_handler, HandlerId};
use super::handlers::{boolean_rectilinear, subtract_vertical_arc_extrusion_batch};
use super::operands::{all_planar, full_cylinder, unique_sources};
use super::types::{BooleanOp, BooleanResult};
use crate::brep::{Accuracy, BrepEnvelope, GeometryError, PatchBounds, SurfaceGeometry};
use prismatic_profile::subtract_prismatic_profile_batch;

pub fn subtract_planar_cutters(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_planar_cutters_with_handlers(host, cutters, id).map(|result| result.0)
}

pub fn subtract_planar_cutters_with_handlers(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> Result<(BooleanResult, Vec<String>), GeometryError> {
    let (result, handlers) = subtract_planar_cutters_outcome_with_handlers(host, cutters, id);
    result.map(|result| (result, handlers))
}

pub fn subtract_planar_cutters_outcome_with_handlers(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
) -> (Result<BooleanResult, GeometryError>, Vec<String>) {
    let mut handlers = Vec::new();
    let result = subtract_planar_cutters_inner(host, cutters, id, &mut handlers);
    (result, handlers)
}

fn subtract_planar_cutters_inner(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    let mut nested = Vec::new();
    let result = record_handler(handlers, HandlerId::SubtractPlanarCutters, || {
        planar_cutters_boolean(host, cutters, id, &mut nested)
    })?;
    handlers.append(&mut nested);
    Ok(result)
}

fn planar_cutters_boolean(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    if cutters.is_empty() || cutters.len() > 100 {
        return Err(GeometryError::InvalidGeometry(
            "planar batch subtraction requires between one and 100 cutters".into(),
        ));
    }
    if cutters.len() == 1 {
        let (result, nested) =
            boolean_brep_with_handlers(host, &cutters[0], BooleanOp::Subtraction, id)?;
        handlers.extend(nested);
        return Ok(result);
    }
    let (accuracy, bounds) = accuracy_and_bounds(host, cutters)?;
    let planar = cutters
        .iter()
        .filter(|cutter| all_planar(cutter))
        .collect::<Vec<_>>();
    let cylinders = if planar.len() == cutters.len() {
        Vec::new()
    } else {
        cutters
            .iter()
            .filter(|cutter| full_cylinder(cutter).is_ok())
            .collect::<Vec<_>>()
    };
    if !cylinders.is_empty() && planar.len() + cylinders.len() == cutters.len() {
        if let Some(result) = subtract_prismatic_profile_batch(host, cutters, id.clone())? {
            handlers.push(HandlerId::SubtractPrismaticProfileBatch.as_str().into());
            return Ok(result);
        }
    }
    if !planar.is_empty()
        && !cylinders.is_empty()
        && planar.len() + cylinders.len() == cutters.len()
    {
        return mixed_cutters_boolean(host, cutters, planar, cylinders, id, handlers);
    }
    if host
        .geometry
        .surfaces
        .iter()
        .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
        && cutters.iter().all(all_planar)
    {
        match subtract_vertical_arc_extrusion_batch(
            host,
            &cutters.iter().collect::<Vec<_>>(),
            id.clone(),
        ) {
            Ok(result) => {
                handlers.push(HandlerId::SubtractVerticalArcExtrusionBatch.as_str().into());
                return Ok(result);
            }
            Err(GeometryError::CoverageGap { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    check_disjoint_cutters(cutters, &bounds, accuracy)?;
    let mut combined = BrepEnvelope::new(format!("{id}:cutters"), accuracy)?;
    for cutter in cutters {
        append_analytic_input(&mut combined, cutter)?;
    }
    combined.validate()?;
    let result = boolean_rectilinear(host, &combined, BooleanOp::Subtraction, id)?;
    handlers.push(HandlerId::BooleanRectilinear.as_str().into());
    Ok(result)
}

fn accuracy_and_bounds(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
) -> Result<(Accuracy, Vec<PatchBounds>), GeometryError> {
    let mut accuracy = host.accuracy;
    let mut bounds = Vec::with_capacity(cutters.len());
    for cutter in cutters {
        cutter.validate()?;
        accuracy = Accuracy::combined(accuracy, cutter.accuracy);
        bounds.push(
            cutter.bounds()?.ok_or_else(|| {
                GeometryError::InvalidGeometry("planar cutter has no bounds".into())
            })?,
        );
    }
    Ok((accuracy, bounds))
}

fn mixed_cutters_boolean(
    host: &BrepEnvelope,
    cutters: &[BrepEnvelope],
    planar: Vec<&BrepEnvelope>,
    cylinders: Vec<&BrepEnvelope>,
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    let planar_cutters = planar.into_iter().cloned().collect::<Vec<_>>();
    let mut result =
        subtract_planar_cutters_inner(host, &planar_cutters, format!("{id}:planar"), handlers)
            .map_err(|error| match error {
                GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
                    families: ["mixed cutter batch".into(), "planar stage".into()],
                },
                other => other,
            })?;
    let count = cylinders.len();
    for (index, cylinder) in cylinders.into_iter().enumerate() {
        let prior = result;
        let (mut next, nested) = boolean_brep_with_handlers(
            &prior.brep,
            cylinder,
            BooleanOp::Subtraction,
            if index + 1 == count {
                id.clone()
            } else {
                format!("{id}:round-{index}")
            },
        )
        .map_err(|error| match error {
            GeometryError::CoverageGap { .. } => GeometryError::CoverageGap {
                families: ["mixed cutter batch".into(), "cylindrical stage".into()],
            },
            other => other,
        })?;
        handlers.extend(nested);
        set_lineage_provenance(&mut next.brep, &prior.brep)?;
        next.brep.validate()?;
        result = next;
    }
    result.report.face_mappings =
        analytic_face_mappings(&result.brep, std::iter::once(host).chain(cutters.iter()));
    Ok(result)
}

fn set_lineage_provenance(
    next: &mut BrepEnvelope,
    prior: &BrepEnvelope,
) -> Result<(), GeometryError> {
    for face in &mut next.topology.faces {
        let mut lineage = Vec::new();
        for source in std::mem::take(&mut face.provenance.sources) {
            if source.entity == prior.id && source.body == prior.id {
                let previous = prior
                    .topology
                    .faces
                    .get(source.face as usize)
                    .ok_or_else(|| {
                        GeometryError::InvalidTopology("mixed cut source face is missing".into())
                    })?;
                if previous.provenance.sources.is_empty() {
                    lineage.push(source);
                } else {
                    lineage.extend(previous.provenance.sources.iter().cloned());
                }
            } else {
                lineage.push(source);
            }
        }
        face.provenance.sources = unique_sources(lineage);
    }
    Ok(())
}

fn check_disjoint_cutters(
    cutters: &[BrepEnvelope],
    bounds: &[PatchBounds],
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for first in 0..cutters.len() {
        for second in first + 1..cutters.len() {
            if (0..3).all(|axis| {
                bounds[first].axes[axis]
                    .hi
                    .min(bounds[second].axes[axis].hi)
                    - bounds[first].axes[axis]
                        .lo
                        .max(bounds[second].axes[axis].lo)
                    > 4.0 * accuracy.geometric
            }) {
                return Err(GeometryError::CoverageGap {
                    families: [
                        "overlapping planar batch cutters".into(),
                        "overlapping planar batch cutters".into(),
                    ],
                });
            }
        }
    }
    Ok(())
}
