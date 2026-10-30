use super::{assert_clearance, assert_exact, variable};
use crate::geom2d::poly2d::Pt2;
use crate::geom2d::tests::p;
use std::f64::consts::PI;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }

    fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next()
    }

    fn pick(&mut self, items: &[f64]) -> f64 {
        items[((self.next() * items.len() as f64) as usize).min(items.len() - 1)]
    }
}

fn noisy_ring(rng: &mut Rng) -> (Vec<Pt2>, f64) {
    let amplitude = rng.pick(&[3e-7, 1e-6, 2e-6, 3e-6, 1e-5, 1e-4]);
    let pitch = rng.range(0.2, 2.0);
    let count = 2 * (2 + (rng.next() * 15.0) as usize);
    let mut ring: Vec<Pt2> = Vec::new();
    for index in 0..=count {
        let z = if index % 2 == 1 {
            amplitude * rng.range(-1.0, 1.0)
        } else {
            0.0
        };
        ring.push(p(index as f64 * pitch, z));
    }
    ring.push(p(count as f64 * pitch, 10.0));
    ring.push(p(0.0, 10.0));
    (ring, rng.pick(&[0.25, 0.5, 1.0, 2.0]))
}

fn varied_ring(rng: &mut Rng, kind: usize) -> (Vec<Pt2>, f64) {
    let ring: Vec<Pt2> = match kind {
        0 => {
            let amplitude = rng.pick(&[3e-3, 1e-3, 3e-4, 1e-4, 3e-5, 1e-5, 3e-6, 1e-6, 3e-7]);
            let pitch = rng.range(0.1, 2.0);
            let count = 2 * (2 + (rng.next() * 15.0) as usize);
            let mut ring: Vec<Pt2> = (0..=count)
                .map(|index| {
                    p(
                        index as f64 * pitch,
                        amplitude * rng.range(-1.0, 1.0) * (index % 2) as f64,
                    )
                })
                .collect();
            ring.push(p(count as f64 * pitch, 10.0));
            ring.push(p(0.0, 10.0));
            ring
        }
        1 => {
            let n = 6 + (rng.next() * 20.0) as usize;
            (0..n)
                .map(|index| {
                    let angle = index as f64 / n as f64 * 2.0 * PI;
                    let radius = rng.range(5.0, 12.0);
                    p(radius * angle.cos(), radius * angle.sin())
                })
                .collect()
        }
        _ => {
            let n = 20 + (rng.next() * 60.0) as usize;
            (0..n)
                .map(|index| {
                    let angle = index as f64 / n as f64 * 2.0 * PI;
                    let radius = 10.0 + rng.pick(&[1e-3, 1e-4, 1e-5, 1e-6]) * rng.range(-1.0, 1.0);
                    p(radius * angle.cos(), radius * angle.sin())
                })
                .collect()
        }
    };
    (ring, rng.pick(&[0.25, 0.5, 1.0, 2.0]))
}

fn assert_insets_exactly(ring: &[Pt2], distance: f64, label: &str) {
    let distances = vec![distance; ring.len()];
    let regions = variable(ring, &distances).unwrap_or_else(|error| panic!("{label}: {error:?}"));
    assert_eq!(regions.len(), 1, "{label}");
    assert_exact(&regions, &[(ring, &distances)]);
    assert_clearance(&regions, &[(ring, &distances)]);
}

#[test]
fn noisy_near_flat_rings_inset_exactly() {
    let cases = [
        24, 51, 74, 106, 119, 160, 216, 222, 304, 314, 320, 333, 340, 348, 349, 376, 424, 521, 554,
    ];
    let mut rng = Rng(0xabad_cafe);
    for case in 0..=cases[cases.len() - 1] {
        let (ring, distance) = noisy_ring(&mut rng);
        if cases.contains(&case) {
            assert_insets_exactly(&ring, distance, &format!("noise{case}"));
        }
    }
}

#[test]
fn a_shallow_zigzag_ring_insets_exactly() {
    let mut rng = Rng(0xfeed_0042);
    for case in 0..=153 {
        let (ring, distance) = varied_ring(&mut rng, case % 3);
        if case == 153 {
            assert_insets_exactly(&ring, distance, "off153");
        }
    }
}
