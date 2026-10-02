use super::intersect_patches;
use crate::brep::{
    Accuracy, Curve, CurveGeometry, Frame3, GeometryStore, SsiBudget, Surface, SurfaceGeometry,
};
use crate::math::{norm, sub, Interval};

fn universal_ssi_accuracy() -> Accuracy {
    Accuracy {
        geometric: 1e-5,
        intersection: 2.5e-6,
        tessellation: 1e-3,
        exchange: 1e-5,
    }
}

#[test]
fn oblique_plane_torus_is_stored_as_corrected_numerical_branches() {
    let mut store = GeometryStore::new();
    let plane = Frame3::from_axis([0.0, 0.0, 0.15], [0.35, -0.2, 1.0], [1.0, 0.0, 0.0]).unwrap();
    store.surfaces = vec![
        SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 2.0,
            minor_radius: 0.5,
        },
        SurfaceGeometry::Plane { frame: plane },
    ];
    let tau = std::f64::consts::TAU;
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [Interval::new(0.0, tau).unwrap(); 2],
            [Interval::new(-4.0, 4.0).unwrap(); 2],
        ],
        universal_ssi_accuracy(),
        SsiBudget::default(),
    )
    .unwrap();
    assert!(!result.curves.is_empty());
    for branch in result.curves {
        assert!(matches!(
            store.curves[branch.curve as usize],
            CurveGeometry::Intersection { .. }
        ));
        let domain = branch.domain.unwrap();
        for index in 0..=16 {
            let t = domain.lo + domain.width() * index as f64 / 16.0;
            let point = store.curve(branch.curve).unwrap().point_at(t).unwrap();
            for surface in &store.surfaces {
                let uv = surface.project(point, None).unwrap();
                assert!(
                    norm(sub(surface.point_at(uv).unwrap(), point))
                        <= universal_ssi_accuracy().intersection
                );
            }
        }
    }
}

#[test]
fn disjoint_bounded_patches_return_empty_without_fabricating_a_branch() {
    let mut store = GeometryStore::new();
    store.surfaces = vec![
        SurfaceGeometry::Sphere {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        },
        SurfaceGeometry::Plane {
            frame: Frame3 {
                origin: [0.0, 0.0, 2.0],
                ..Frame3::IDENTITY
            },
        },
    ];
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [
                Interval::new(0.0, std::f64::consts::TAU).unwrap(),
                Interval::new(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2).unwrap(),
            ],
            [Interval::new(-2.0, 2.0).unwrap(); 2],
        ],
        universal_ssi_accuracy(),
        SsiBudget::default(),
    )
    .unwrap();
    assert!(result.curves.is_empty());
    assert!(result.contacts.is_empty());
}

#[test]
fn terminal_subdivision_reports_an_isolated_torus_plane_tangency() {
    let u: f64 = 0.37;
    let v: f64 = 0.41;
    let (sin_u, cos_u) = u.sin_cos();
    let (sin_v, cos_v) = v.sin_cos();
    let normal = [cos_v * cos_u, cos_v * sin_u, sin_v];
    let radial = 2.0 + 0.5 * cos_v;
    let contact = [radial * cos_u, radial * sin_u, 0.5 * sin_v];
    let mut store = GeometryStore::new();
    store.surfaces = vec![
        SurfaceGeometry::Torus {
            frame: Frame3::IDENTITY,
            major_radius: 2.0,
            minor_radius: 0.5,
        },
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis(contact, normal, [0.0, 0.0, 1.0]).unwrap(),
        },
    ];
    let tau = std::f64::consts::TAU;
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [Interval::new(0.0, tau).unwrap(); 2],
            [Interval::new(-3.0, 3.0).unwrap(); 2],
        ],
        universal_ssi_accuracy(),
        SsiBudget::default(),
    )
    .unwrap();
    assert!(result.curves.is_empty());
    assert_eq!(result.contacts.len(), 1);
    assert!(norm(sub(result.contacts[0], contact)) <= universal_ssi_accuracy().intersection);
}

#[test]
fn tangent_generator_is_not_collapsed_into_an_isolated_contact() {
    let mut store = GeometryStore::new();
    store.surfaces = vec![
        SurfaceGeometry::Cylinder {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        },
        SurfaceGeometry::Plane {
            frame: Frame3::from_axis([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
        },
    ];
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [
                Interval::new(-0.5, 0.5).unwrap(),
                Interval::new(-1.0, 1.0).unwrap(),
            ],
            [Interval::new(-2.0, 2.0).unwrap(); 2],
        ],
        universal_ssi_accuracy(),
        SsiBudget::default(),
    )
    .unwrap();
    assert!(!result.curves.is_empty());
    assert!(result.contacts.is_empty());
}

#[test]
fn target_patch_bounds_prune_intersections_on_the_unbounded_support() {
    let mut store = GeometryStore::new();
    store.surfaces = vec![
        SurfaceGeometry::Cylinder {
            frame: Frame3::IDENTITY,
            radius: 1.0,
        },
        SurfaceGeometry::Cylinder {
            frame: Frame3::from_axis([0.0, 0.0, 0.2], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap(),
            radius: 0.6,
        },
    ];
    let tau = std::f64::consts::TAU;
    let result = intersect_patches(
        &mut store,
        [0, 1],
        [
            [
                Interval::new(0.0, tau).unwrap(),
                Interval::new(-1.0, 1.0).unwrap(),
            ],
            [
                Interval::new(0.0, tau).unwrap(),
                Interval::new(10.0, 12.0).unwrap(),
            ],
        ],
        universal_ssi_accuracy(),
        SsiBudget::default(),
    )
    .unwrap();
    assert!(result.curves.is_empty());
    assert!(result.contacts.is_empty());
}
