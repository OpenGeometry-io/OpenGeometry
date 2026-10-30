use opengeometry::brep::{Accuracy, FaceRole, GeometryError, Surface, SurfaceGeometry};
use opengeometry::{
    brep::Frame3,
    operations::modifying::boolean::{boolean_spheres, BooleanOp},
    primitives,
};
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(a: [f64; 3]) -> Result<[f64; 3], GeometryError> {
    let n = dot(a, a).sqrt();
    if n == 0.0 {
        return Err(GeometryError::SingularParameterization);
    }
    Ok(a.map(|v| v / n))
}
#[test]
fn picked_support_normals_follow_curvature_reversal_and_singularities() {
    let accuracy = Accuracy {
        geometric: 1e-8,
        intersection: 1e-9,
        tessellation: 0.01,
        exchange: 1e-6,
    };
    let frame = Frame3::from_axis([3.0, -2.0, 1.0], [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]).unwrap();
    let sphere = primitives::sphere("host".into(), frame, 2.0, accuracy).unwrap();
    let point = sphere.geometry.surfaces[0].point_at([0.7, 0.3]).unwrap();
    let inward_mesh_point =
        std::array::from_fn(|i| frame.origin[i] + (point[i] - frame.origin[i]) * 0.99);
    let normal = sphere.face_normal_at(0, inward_mesh_point).unwrap();
    assert!(dot(normal, unit(sub(point, frame.origin)).unwrap()) > 1.0 - 1e-12);
    let cutter = primitives::sphere("cutter".into(), frame, 0.5, accuracy).unwrap();
    let cavity = boolean_spheres(&sphere, &cutter, BooleanOp::Subtraction, "cavity".into())
        .unwrap()
        .brep;
    let face = cavity
        .topology
        .faces
        .iter()
        .find(|face| face.provenance.role == FaceRole::Cut)
        .unwrap();
    let point = cavity
        .geometry
        .surface(face.surface)
        .unwrap()
        .point_at([0.7, 0.3])
        .unwrap();
    assert!(
        dot(
            cavity.face_normal_at(face.id, point).unwrap(),
            unit(sub(point, frame.origin)).unwrap()
        ) < -1.0 + 1e-12
    );
    assert!(matches!(
        sphere.face_normal_at(u32::MAX, point),
        Err(GeometryError::MissingReference { .. })
    ));
    assert!(sphere.face_normal_at(0, [f64::NAN, 0.0, 0.0]).is_err());
    let cone = primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy).unwrap();
    let face = cone
        .topology
        .faces
        .iter()
        .find(|face| {
            matches!(
                cone.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cone { .. }
            )
        })
        .unwrap();
    let apex = cone.geometry.surface(face.surface).unwrap().frame().origin;
    assert!(matches!(
        cone.face_normal_at(face.id, apex),
        Err(GeometryError::SingularParameterization)
    ));
}
