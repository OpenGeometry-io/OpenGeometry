use crate::brep::{unit, Accuracy, BrepEnvelope, FaceRole, GeometryError, Surface};
use crate::math::{scale, sub, Point3};
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, finish_analytic_result, reverse_face,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::query::{classify_point, face_contains_uv, PointClassification};

pub(crate) fn analytic_containment_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    b_inside_a: bool,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let accuracy = Accuracy::combined(a.accuracy, b.accuracy);
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (contained, enclosing) = if b_inside_a { (b, a) } else { (a, b) };
    match operation {
        BooleanOp::Union => {
            append_analytic_input(&mut out, enclosing)?;
        }
        BooleanOp::Intersection => {
            append_analytic_input(&mut out, contained)?;
        }
        BooleanOp::Subtraction if b_inside_a => {
            append_analytic_input(&mut out, a)?;
            let face_offset = out.topology.faces.len() as u32;
            append_analytic_input(&mut out, b)?;
            let cavity = out
                .solids
                .pop()
                .ok_or_else(|| GeometryError::InvalidTopology("missing cutter solid".into()))?
                .outer_shell;
            out.solids[0].cavity_shells.push(cavity);
            for face in face_offset..out.topology.faces.len() as u32 {
                reverse_face(&mut out, face);
                out.topology.faces[face as usize].provenance.role = FaceRole::Cut;
                out.topology.faces[face as usize].provenance.reversed = true;
            }
        }
        BooleanOp::Subtraction => {}
    }
    finish_analytic_result(out, a, b, true, operation, false)
}

pub(crate) fn interior_sample(brep: &BrepEnvelope) -> Result<Point3, GeometryError> {
    let bounds = brep.bounds()?.ok_or_else(|| {
        GeometryError::InvalidTopology("cannot classify an empty solid boundary".into())
    })?;
    let model_scale = bounds
        .axes
        .iter()
        .map(|axis| axis.width())
        .fold(brep.accuracy.geometric, f64::max);
    let offsets = [
        (model_scale * 1.0e-6).max(32.0 * brep.accuracy.geometric),
        16.0 * brep.accuracy.geometric,
        8.0 * brep.accuracy.geometric,
        4.0 * brep.accuracy.geometric,
    ];
    let samples = [0.5, 0.25, 0.75, 0.125, 0.875];
    for face in &brep.topology.faces {
        let surface = brep.geometry.surface(face.surface)?;
        for u in samples {
            for v in samples {
                let uv = [
                    face.trim.uv_bounds[0].lo + u * face.trim.uv_bounds[0].width(),
                    face.trim.uv_bounds[1].lo + v * face.trim.uv_bounds[1].width(),
                ];
                if face_contains_uv(brep, face, uv)? != Some(true) {
                    continue;
                }
                let boundary = surface.point_at(uv)?;
                let outward = scale(unit(surface.normal_at(uv)?)?, face.sense.multiplier());
                for offset in offsets {
                    let candidate = sub(boundary, scale(outward, offset));
                    if classify_point(brep, candidate)? == PointClassification::Inside {
                        return Ok(candidate);
                    }
                }
            }
        }
    }
    Err(GeometryError::UnresolvedIntersection(
        "could not certify an interior classification sample".into(),
    ))
}
