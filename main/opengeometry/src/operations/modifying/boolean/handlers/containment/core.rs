use crate::brep::{Accuracy, BrepEnvelope, FaceRole, GeometryError, Surface};
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

pub(crate) fn boundary_side(
    brep: &BrepEnvelope,
    other: &BrepEnvelope,
) -> Result<PointClassification, GeometryError> {
    let mut sides = Vec::new();
    for shell in &brep.topology.shells {
        sides.push(shell_side(brep, &shell.faces, other)?);
    }
    Ok(match sides.split_first() {
        Some((first, rest)) if rest.iter().all(|side| side == first) => *first,
        _ => PointClassification::Unknown,
    })
}

fn shell_side(
    brep: &BrepEnvelope,
    faces: &[u32],
    other: &BrepEnvelope,
) -> Result<PointClassification, GeometryError> {
    let samples = [0.5, 0.25, 0.75, 0.125, 0.875];
    for face in faces
        .iter()
        .map(|&face| &brep.topology.faces[face as usize])
    {
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
                let place = classify_point(other, surface.point_at(uv)?)?;
                if matches!(
                    place,
                    PointClassification::Inside | PointClassification::Outside
                ) {
                    return Ok(place);
                }
            }
        }
    }
    Ok(PointClassification::Unknown)
}
