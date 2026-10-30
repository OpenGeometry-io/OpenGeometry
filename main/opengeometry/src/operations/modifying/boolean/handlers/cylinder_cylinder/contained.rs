use crate::brep::{Accuracy, BrepEnvelope, FaceRole, GeometryError};
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, finish_cylinder_result, reverse_face,
};
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(super) fn contained_cylinders(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    b_inside_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    let (contained, enclosing) = if b_inside_a { (b, a) } else { (a, b) };
    match operation {
        BooleanOp::Union => {
            append_analytic_input(&mut out, enclosing.brep)?;
        }
        BooleanOp::Intersection => {
            append_analytic_input(&mut out, contained.brep)?;
        }
        BooleanOp::Subtraction if b_inside_a => {
            append_analytic_input(&mut out, a.brep)?;
            let face_offset = out.topology.faces.len() as u32;
            append_analytic_input(&mut out, b.brep)?;
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
    finish_cylinder_result(out, a, b, operation, false)
}
