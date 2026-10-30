use super::box_boundary::{
    add_box_corners, box_face_boundary, box_face_layout, BoxCorners, BoxFaceLayout,
};
use crate::brep::{
    dimensions, Accuracy, BrepEnvelope, Builder, Frame3, GeometryError, SurfaceGeometry,
};

pub fn cuboid(
    id: String,
    frame: Frame3,
    size: [f64; 3],
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    dimensions(&size)?;
    if size.iter().any(|s| *s <= 4.0 * accuracy.geometric) {
        return Err(GeometryError::UnresolvedIntersection(
            "cuboid dimensions are below geometric resolution".into(),
        ));
    }
    let mut builder = Builder::new(id, accuracy)?;
    let corners = add_box_corners(&mut builder, frame, size, [frame.x, frame.y, frame.z])?;
    add_cuboid_faces(&mut builder, frame, &corners, accuracy)?;
    builder.finish_solid()
}

fn add_cuboid_faces(
    builder: &mut Builder,
    frame: Frame3,
    corners: &BoxCorners,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for BoxFaceLayout {
        key,
        indices,
        normal,
    } in box_face_layout(frame)
    {
        let face = box_face_boundary(builder, corners, indices, normal, accuracy, |from| {
            from as u32
        })?;
        builder.face(
            key,
            SurfaceGeometry::Plane { frame: face.frame },
            face.bounds,
            face.uses,
        )?;
    }
    Ok(())
}
