use crate::brep::{fine_accuracy, Frame3, SurfaceGeometry};
use crate::math::{cross, norm, sub};
use crate::primitives::{self, ProfileEdge};
use crate::tessellation::tessellate;
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

const RADIUS: f64 = 0.05;
const STOREY_DEFLECTION: f64 = 0.1875;

fn half_circle(start_angle: f64) -> ProfileEdge {
    ProfileEdge::Arc {
        center: [0.0, 0.0],
        radius: RADIUS,
        start_angle,
        sweep_angle: PI,
    }
}

fn cap_points_and_smallest_area(profile: Vec<ProfileEdge>) -> Vec<(usize, f64)> {
    let body = primitives::arc_edged_extrusion(
        "rail".into(),
        Frame3::IDENTITY,
        profile,
        7.0,
        fine_accuracy(),
    )
    .unwrap();
    let mesh = tessellate(&body, STOREY_DEFLECTION, 1_000_000).unwrap();
    let mut caps = Vec::new();
    for face in &body.topology.faces {
        let SurfaceGeometry::Plane { frame } = &body.geometry.surfaces[face.surface as usize]
        else {
            continue;
        };
        if frame.z[2].abs() < 0.5 {
            continue;
        }
        let mut ids = BTreeSet::new();
        let mut smallest = f64::INFINITY;
        for (triangle, &owner) in mesh.indices.chunks_exact(3).zip(&mesh.triangle_face_ids) {
            if owner != face.id {
                continue;
            }
            ids.extend(triangle.iter().copied());
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|id| mesh.point(id));
            smallest = smallest.min(0.5 * norm(cross(sub(b, a), sub(c, a))));
        }
        caps.push((ids.len(), smallest));
    }
    caps
}

fn ear_area(step: f64) -> f64 {
    0.5 * RADIUS * RADIUS * (2.0 * step.sin() - (2.0 * step).sin())
}

#[test]
fn two_arc_planar_loop_tessellates_at_a_coarse_deflection() {
    let caps = cap_points_and_smallest_area(vec![half_circle(0.0), half_circle(PI)]);
    assert_eq!(caps.len(), 2);
    for (points, smallest) in caps {
        assert!(points >= 4, "cap has {points} boundary points");
        let bound = 0.5 * ear_area(TAU / points as f64);
        assert!(
            smallest > bound,
            "cap triangle area {smallest} below {bound}"
        );
    }
}

#[test]
fn short_arc_and_chord_planar_loop_tessellates_at_a_coarse_deflection() {
    let sweep = PI / 6.0;
    let caps = cap_points_and_smallest_area(vec![
        ProfileEdge::Arc {
            center: [0.0, 0.0],
            radius: RADIUS,
            start_angle: 0.0,
            sweep_angle: sweep,
        },
        ProfileEdge::Line {
            from: [RADIUS * sweep.cos(), RADIUS * sweep.sin()],
            to: [RADIUS, 0.0],
        },
    ]);
    assert_eq!(caps.len(), 2);
    for (points, smallest) in caps {
        assert!(points >= 3, "cap has {points} boundary points");
        let bound = 0.5 * ear_area(sweep / (points - 1) as f64);
        assert!(
            smallest > bound,
            "cap triangle area {smallest} below {bound}"
        );
    }
}
