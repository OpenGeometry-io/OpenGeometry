use super::provenance::{box_source, face_provenance};
use crate::brep::{Accuracy, Builder, FaceRole, Frame3, GeometryError, SurfaceGeometry, Use};
use crate::operations::modifying::boolean::operands::BoxInput;
use crate::primitives::{box_face_boundary, box_face_layout, BoxCorners, BoxFaceLayout};

pub(crate) fn add_box_faces(
    builder: &mut Builder,
    box_: &BoxInput<'_>,
    corners: &BoxCorners,
    accuracy: Accuracy,
    face_holes: impl Fn(&Builder, usize, Frame3) -> Result<Vec<Vec<Use>>, GeometryError>,
    is_split: impl Fn(usize) -> bool,
) -> Result<(), GeometryError> {
    for (
        face_index,
        BoxFaceLayout {
            key,
            indices,
            normal,
        },
    ) in box_face_layout(box_.frame).into_iter().enumerate()
    {
        let face = box_face_boundary(builder, corners, indices, normal, accuracy, |from| {
            corners.vertices[from]
        })?;
        let holes = face_holes(builder, face_index, face.frame)?;
        builder.face_with_holes(
            key,
            SurfaceGeometry::Plane { frame: face.frame },
            face.bounds,
            face.uses,
            holes,
        )?;
        builder.brep.topology.faces[face_index].provenance = face_provenance(
            vec![box_source(box_, face_index)],
            if is_split(face_index) {
                FaceRole::Split
            } else {
                FaceRole::Preserved
            },
            false,
        );
    }
    Ok(())
}
