use crate::brep::{Accuracy, BrepEnvelope, FaceRole, GeometryError, SolidRegion, Surface};
use crate::math::Point3;
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, finish_analytic_result, reverse_face,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::query::{
    classify_point, classify_point_in_shell, face_contains_uv, PointClassification,
};

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
            let solids = out.solids.len();
            append_analytic_input(&mut out, b)?;
            let cavities = out.solids.split_off(solids);
            if cavities.is_empty() || cavities.iter().any(|solid| !solid.cavity_shells.is_empty()) {
                return Err(GeometryError::UnresolvedIntersection(
                    "containment cavity of a hollow cutter is unresolved".into(),
                ));
            }
            for (cavity, solid) in cavities.into_iter().zip(&b.solids) {
                let faces = &b.topology.shells[solid.outer_shell as usize].faces;
                let home = enclosing_solid(a, b, faces)?;
                out.solids[home].cavity_shells.push(cavity.outer_shell);
            }
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
    for solid in &brep.solids {
        for shell in std::iter::once(&solid.outer_shell).chain(&solid.cavity_shells) {
            let faces = &brep.topology.shells[*shell as usize].faces;
            sides.push(shell_side(brep, faces, other)?);
        }
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
    let side = first_sample(brep, faces, |point| {
        let place = classify_point(other, point)?;
        Ok(matches!(
            place,
            PointClassification::Inside | PointClassification::Outside
        )
        .then_some(place))
    })?;
    Ok(side.unwrap_or(PointClassification::Unknown))
}

fn enclosing_solid(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    faces: &[u32],
) -> Result<usize, GeometryError> {
    let home = first_sample(b, faces, |point| {
        for (index, solid) in a.solids.iter().enumerate() {
            if solid_place(a, solid, point)? == PointClassification::Inside {
                return Ok(Some(index));
            }
        }
        Ok(None)
    })?;
    home.ok_or_else(|| {
        GeometryError::UnresolvedIntersection("containment cavity has no enclosing solid".into())
    })
}

fn solid_place(
    brep: &BrepEnvelope,
    solid: &SolidRegion,
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    let faces = |shell: u32| &brep.topology.shells[shell as usize].faces;
    let outer = classify_point_in_shell(brep, faces(solid.outer_shell), point)?;
    if outer != PointClassification::Inside {
        return Ok(outer);
    }
    for cavity in &solid.cavity_shells {
        match classify_point_in_shell(brep, faces(*cavity), point)? {
            PointClassification::Outside => {}
            PointClassification::Inside => return Ok(PointClassification::Outside),
            other => return Ok(other),
        }
    }
    Ok(PointClassification::Inside)
}

fn first_sample<T>(
    brep: &BrepEnvelope,
    faces: &[u32],
    mut found: impl FnMut(Point3) -> Result<Option<T>, GeometryError>,
) -> Result<Option<T>, GeometryError> {
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
                if let Some(value) = found(surface.point_at(uv)?)? {
                    return Ok(Some(value));
                }
            }
        }
    }
    Ok(None)
}
