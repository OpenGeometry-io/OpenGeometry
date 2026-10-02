use crate::brep::{Accuracy, BrepEnvelope, FaceRole, FaceSource, GeometryError, GeometryQuality};
use crate::operations::modifying::boolean::assembly::{
    analytic_face_mappings, append_analytic_input,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanReport, BooleanResult};

pub(crate) fn coincident_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    let mut out = BrepEnvelope::new(id, accuracy)?;
    if operation != BooleanOp::Subtraction {
        append_analytic_input(&mut out, a)?;
        for face in &mut out.topology.faces {
            let counterpart = b.topology.faces.get(face.id as usize).ok_or_else(|| {
                GeometryError::InvalidTopology(
                    "coincident operands have incompatible face ownership".into(),
                )
            })?;
            face.provenance.sources.push(FaceSource {
                entity: b.id.clone(),
                body: b.id.clone(),
                key: counterpart.key.clone(),
                face: counterpart.id,
            });
            face.provenance.role = FaceRole::Coincident;
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident: true,
            face_mappings: analytic_face_mappings(&out, [a, b]),
        },
        brep: out,
    })
}
