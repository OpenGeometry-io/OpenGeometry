use crate::brep::{wrapped_sector, SECTOR};
use crate::math::Point3;
use crate::tessellation::tessellate::tessellate;

#[test]
fn a_wrapped_side_meshes_to_the_exact_volume() {
    let mesh = tessellate(&wrapped_sector(), 1e-3, 1_000_000).unwrap();
    let p = |i: u32| -> Point3 { std::array::from_fn(|j| mesh.positions[i as usize * 3 + j]) };
    let volume: f64 = mesh
        .indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum();
    let exact = SECTOR.volume();
    assert!(
        ((volume - exact) / exact).abs() < 1e-4,
        "{volume} vs {exact}"
    );
}
