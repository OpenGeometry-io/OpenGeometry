mod build;
mod cut;
mod notch;
mod prism;

use crate::brep::{Accuracy, BrepEnvelope, GeometryError, GeometryQuality};
use crate::operations::modifying::boolean::assembly::analytic_face_mappings;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanReport, BooleanResult};
use build::build;
use cut::cut_cap;
use notch::notch;
use prism::prism;

pub(crate) fn subtract_half_space_from_arc_extrusion(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_half_spaces_from_arc_extrusion(a, &[b], id)
}

pub(crate) fn subtract_half_spaces_from_arc_extrusion(
    host: &BrepEnvelope,
    cutters: &[&BrepEnvelope],
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = cutters.iter().fold(host.accuracy, |accuracy, cutter| {
        Accuracy::combined(accuracy, cutter.accuracy)
    });
    let mut prism = prism(host)?;
    for cutter in cutters {
        let (replaced, cap) = cut_cap(&prism, cutter, 4.0 * accuracy.geometric)?;
        prism.caps[replaced] = cap;
    }
    finish(build(&prism, id, accuracy)?, host, cutters)
}

pub(crate) fn subtract_arc_extrusion_from_planar_solid(
    host: &BrepEnvelope,
    tool: &BrepEnvelope,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy::combined(host.accuracy, tool.accuracy);
    finish(notch(host, tool, id, accuracy)?, host, &[tool])
}

fn finish(
    mut out: BrepEnvelope,
    host: &BrepEnvelope,
    tools: &[&BrepEnvelope],
) -> Result<BooleanResult, GeometryError> {
    out.revision = tools
        .iter()
        .fold(host.revision, |revision, tool| revision.max(tool.revision))
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings =
        analytic_face_mappings(&out, std::iter::once(host).chain(tools.iter().copied()));
    Ok(BooleanResult {
        report: BooleanReport {
            operation: BooleanOp::Subtraction,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: false,
            face_mappings,
        },
        brep: out,
    })
}
