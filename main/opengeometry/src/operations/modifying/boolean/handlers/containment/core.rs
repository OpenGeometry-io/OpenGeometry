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
            for face in face_offset..out.topology.faces.len() as u32 {
                reverse_face(&mut out, face);
                out.topology.faces[face as usize].provenance.role = FaceRole::Cut;
                out.topology.faces[face as usize].provenance.reversed = true;
            }
            let cutters = out.solids.split_off(solids);
            let outers = out
                .solids
                .iter()
                .map(|solid| solid.outer_shell)
                .chain(cutters.iter().flat_map(|solid| solid.cavity_shells.clone()))
                .collect::<Vec<_>>();
            let holes = out
                .solids
                .iter()
                .flat_map(|solid| solid.cavity_shells.clone())
                .chain(cutters.iter().map(|solid| solid.outer_shell))
                .collect::<Vec<_>>();
            out.solids = nest_shells(&out, &outers, holes)?;
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

fn nest_shells(
    brep: &BrepEnvelope,
    outers: &[u32],
    holes: Vec<u32>,
) -> Result<Vec<SolidRegion>, GeometryError> {
    let depths = outers
        .iter()
        .map(|&shell| Ok(enclosing_outers(brep, outers, shell)?.len()))
        .collect::<Result<Vec<_>, GeometryError>>()?;
    let mut solids = outers
        .iter()
        .map(|&outer_shell| SolidRegion {
            outer_shell,
            cavity_shells: Vec::new(),
        })
        .collect::<Vec<_>>();
    for hole in holes {
        let home = enclosing_outers(brep, outers, hole)?
            .into_iter()
            .max_by_key(|&index| depths[index])
            .ok_or_else(|| {
                GeometryError::UnresolvedIntersection(
                    "containment cavity has no enclosing solid".into(),
                )
            })?;
        solids[home].cavity_shells.push(hole);
    }
    Ok(solids)
}

fn enclosing_outers(
    brep: &BrepEnvelope,
    outers: &[u32],
    shell: u32,
) -> Result<Vec<usize>, GeometryError> {
    let faces = |shell: u32| &brep.topology.shells[shell as usize].faces;
    let found = first_sample(brep, faces(shell), |point| {
        let mut enclosing = Vec::new();
        for (index, &outer) in outers.iter().enumerate() {
            if outer == shell {
                continue;
            }
            match classify_point_in_shell(brep, faces(outer), point)? {
                PointClassification::Inside => enclosing.push(index),
                PointClassification::Outside => {}
                _ => return Ok(None),
            }
        }
        Ok(Some(enclosing))
    })?;
    found.ok_or_else(|| {
        GeometryError::UnresolvedIntersection("containment shell nesting is unresolved".into())
    })
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
