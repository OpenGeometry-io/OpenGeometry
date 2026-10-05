use crate::brep::{Accuracy, BrepEnvelope, FaceRole, FaceSource, GeometryError};
use crate::operations::modifying::boolean::assembly::{
    append_analytic_input, append_annular_cylinder, cylinder_source, face_provenance,
    finish_cylinder_result,
};
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives;

pub(super) fn coextensive_cylinder_radii(
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    alignment: f64,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let a_lateral = cylinder_source(a, 0);
    let b_lateral = cylinder_source(b, 0);
    let a_caps = [cylinder_source(a, 1), cylinder_source(a, 2)];
    let b_caps = if alignment > 0.0 {
        [cylinder_source(b, 1), cylinder_source(b, 2)]
    } else {
        [cylinder_source(b, 2), cylinder_source(b, 1)]
    };
    let a_is_larger = a.radius > b.radius;
    let mut out = BrepEnvelope::new(id, accuracy)?;
    match operation {
        BooleanOp::Union | BooleanOp::Intersection => {
            let choose_a = if operation == BooleanOp::Union {
                a_is_larger
            } else {
                !a_is_larger
            };
            let (radius, lateral) = if choose_a {
                (a.radius, a_lateral.clone())
            } else {
                (b.radius, b_lateral.clone())
            };
            add_coextensive_cylinder(&mut out, a, radius, lateral, &a_caps, &b_caps, accuracy)?;
        }
        BooleanOp::Subtraction if a_is_larger => {
            append_annular_cylinder(
                &mut out,
                a.frame,
                b.radius,
                a.radius,
                a.height,
                [
                    face_provenance(vec![a_lateral], FaceRole::Preserved, false),
                    face_provenance(vec![b_lateral], FaceRole::Cut, true),
                    face_provenance(vec![a_caps[0].clone()], FaceRole::Split, false),
                    face_provenance(vec![a_caps[1].clone()], FaceRole::Split, false),
                ],
            )?;
        }
        BooleanOp::Subtraction => {}
    }
    finish_cylinder_result(out, a, b, operation, true)
}

fn add_coextensive_cylinder(
    out: &mut BrepEnvelope,
    a: &CylinderInput<'_>,
    radius: f64,
    lateral: FaceSource,
    a_caps: &[FaceSource; 2],
    b_caps: &[FaceSource; 2],
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let part = primitives::cylinder(
        format!("{}:radial", out.id),
        a.frame,
        radius,
        a.height,
        accuracy,
    )?;
    append_analytic_input(out, &part)?;
    out.topology.faces[0].provenance = face_provenance(vec![lateral], FaceRole::Preserved, false);
    for (face, sources) in [
        (1, vec![a_caps[0].clone(), b_caps[0].clone()]),
        (2, vec![a_caps[1].clone(), b_caps[1].clone()]),
    ] {
        out.topology.faces[face].provenance = face_provenance(sources, FaceRole::Split, false);
    }
    Ok(())
}
