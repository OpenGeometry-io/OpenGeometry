use crate::brep::{Accuracy, BrepEnvelope, FaceSource, GeometryError, GeometryQuality};
use crate::math::Point3;
use crate::operations::modifying::boolean::assembly::append_analytic_input;
use crate::operations::modifying::boolean::types::{
    BooleanOp, BooleanReport, BooleanResult, FaceMapping,
};

pub(crate) fn disjoint_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    let (Some(a_bounds), Some(b_bounds)) = (a.bounds()?, b.bounds()?) else {
        return Ok(None);
    };
    let clearance = 4.0 * a.accuracy.geometric.max(b.accuracy.geometric);
    let separated = (0..3).any(|axis| {
        a_bounds.axes[axis].hi + clearance < b_bounds.axes[axis].lo
            || b_bounds.axes[axis].hi + clearance < a_bounds.axes[axis].lo
    });
    if !separated {
        return Ok(None);
    }
    Ok(Some(separate_boolean(a, b, operation, id, Vec::new())?))
}

pub(super) fn separate_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    contacts: Vec<Point3>,
) -> Result<BooleanResult, GeometryError> {
    if !matches!(a.quality, GeometryQuality::Analytic)
        || !matches!(b.quality, GeometryQuality::Analytic)
    {
        return Err(GeometryError::UnsupportedGeometry(
            "analytic boolean operands must have analytic quality".into(),
        ));
    }
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let mut mappings = match operation {
        BooleanOp::Union | BooleanOp::Subtraction => append_analytic_input(&mut out, a)?,
        BooleanOp::Intersection => a
            .topology
            .faces
            .iter()
            .map(|face| FaceMapping {
                source: FaceSource {
                    entity: a.id.clone(),
                    body: a.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                },
                result_faces: Vec::new(),
            })
            .collect(),
    };
    match operation {
        BooleanOp::Union => mappings.extend(append_analytic_input(&mut out, b)?),
        BooleanOp::Intersection | BooleanOp::Subtraction => {
            mappings.extend(b.topology.faces.iter().map(|face| FaceMapping {
                source: FaceSource {
                    entity: b.id.clone(),
                    body: b.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                },
                result_faces: Vec::new(),
            }))
        }
    }
    out.revision = a
        .revision
        .max(b.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts,
            coincident: false,
            face_mappings: mappings,
        },
    })
}
