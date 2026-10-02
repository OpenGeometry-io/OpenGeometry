use crate::brep::{coarse_accuracy, GROUND};
use crate::operations::creating::{sweep, ProfileLoop};
use crate::tessellation::tessellate;
use std::collections::HashMap;

#[test]
fn circular_sweep_midpoints_reuse_boundary_points_for_bitwise_watertight_seams() {
    let profile = ProfileLoop::Circle {
        frame: GROUND,
        radius: 0.5,
    };
    let path = vec![[0.0, 0.0, 0.0], [0.0, 3.0, 0.0], [3.0, 3.0, 0.0]];
    let body = sweep::build("solid".into(), profile, path, coarse_accuracy(1e-6)).unwrap();
    let mesh = tessellate(&body, 0.02, 1_000_000).unwrap();
    let bits = |index: u32| -> Vec<u64> {
        let start = index as usize * 3;
        mesh.positions[start..start + 3]
            .iter()
            .map(|value| value.to_bits())
            .collect()
    };
    let mut uses: HashMap<Vec<Vec<u64>>, usize> = HashMap::new();
    for corner in 0..mesh.indices.len() {
        let next = corner - corner % 3 + (corner + 1) % 3;
        let mut key = vec![bits(mesh.indices[corner]), bits(mesh.indices[next])];
        key.sort();
        *uses.entry(key).or_default() += 1;
    }
    assert!(!uses.is_empty());
    assert!(uses.values().all(|&count| count == 2));
}
