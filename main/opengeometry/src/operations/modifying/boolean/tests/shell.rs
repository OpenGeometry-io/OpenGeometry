use super::support::{extrusion, sphere};
use crate::brep::{FaceRole, Frame3, GeometryError};
use crate::operations::modifying::boolean::shell::shell_brep;
use crate::primitives;
use crate::tessellation::tessellate;

#[test]
fn restricted_shell_offsets_supported_closed_families() {
    let accuracy = sphere("accuracy", [0.0; 3], 1.0).accuracy;
    let inputs = [
        (
            primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], accuracy).unwrap(),
            12,
        ),
        (
            primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, accuracy).unwrap(),
            6,
        ),
        (
            primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, accuracy).unwrap(),
            2,
        ),
        (
            primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, accuracy).unwrap(),
            2,
        ),
        (
            primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 3.0, accuracy).unwrap(),
            4,
        ),
        (
            primitives::frustum("frustum".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, accuracy)
                .unwrap(),
            6,
        ),
        (
            extrusion(
                "profile-shell",
                vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
                vec![vec![[1.0, 1.0], [1.0, 3.0], [3.0, 3.0], [3.0, 1.0]]],
            ),
            20,
        ),
    ];
    for (input, expected_faces) in &inputs {
        let result = shell_brep(input, 0.2, format!("{}-shell", input.id)).unwrap();
        result.brep.validate().unwrap();
        assert_eq!(result.brep.solids.len(), 1);
        assert_eq!(result.brep.solids[0].cavity_shells.len(), 1);
        assert_eq!(result.brep.topology.faces.len(), *expected_faces);
        assert_eq!(
            result.report.face_mappings.len(),
            input.topology.faces.len()
        );
        assert!(result.brep.topology.faces.iter().any(|face| {
            face.provenance.role == FaceRole::Cut
                && face.provenance.reversed
                && face
                    .provenance
                    .sources
                    .iter()
                    .all(|source| source.entity == input.id)
        }));
        tessellate(&result.brep, 0.02, 2_000_000).unwrap();
    }
    assert!(matches!(
        shell_brep(&inputs[2].0, 1.0, "invalid".into()),
        Err(GeometryError::UnresolvedIntersection(_))
    ));
}
