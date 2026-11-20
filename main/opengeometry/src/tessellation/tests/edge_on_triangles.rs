use super::super::mesh::Tessellation;
use super::super::trimmed_face::general_curved_trimmed_face;
use crate::brep::{fine_accuracy, Frame3, Surface, SurfaceGeometry};
use crate::primitives;
use std::collections::BTreeSet;

const SLIVER: [[f64; 2]; 6] = [
    [0.0, 0.4315522348698053],
    [-2.4066236448435014e-16, 0.0],
    [0.0970789271307487, 0.0],
    [0.1941578542615101, 2.2648549702353193e-14],
    [0.12598686852943008, 0.13848252444618137],
    [0.061224770648327764, 0.2824284296054027],
];

#[test]
fn an_ear_of_end_samples_rounded_off_their_line_turns_like_its_face() {
    let body =
        primitives::cylinder("c".into(), Frame3::IDENTITY, 3.35, 1.0, fine_accuracy()).unwrap();
    let face = body
        .topology
        .faces
        .iter()
        .find(|face| {
            matches!(
                body.geometry.surfaces[face.surface as usize],
                SurfaceGeometry::Cylinder { .. }
            )
        })
        .unwrap();
    let surface = body.geometry.surface(face.surface).unwrap();
    let ring = SLIVER
        .into_iter()
        .map(|uv| {
            let on_end = [uv[0], if uv[1].abs() < 1e-12 { 0.0 } else { uv[1] }];
            (uv, surface.point_at(on_end).unwrap())
        })
        .collect::<Vec<_>>();
    let mut mesh = Tessellation::new(0, 0.02);
    general_curved_trimmed_face(&body, face, [0, 0], &[ring], 0.02, &mut mesh, 1000).unwrap();
    let mut directed = BTreeSet::new();
    for triangle in mesh.indices.chunks_exact(3) {
        for edge in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            assert!(
                directed.insert(edge),
                "edge {edge:?} runs the same way twice"
            );
        }
    }
    assert!(mesh.indices.len() >= 12);
}
