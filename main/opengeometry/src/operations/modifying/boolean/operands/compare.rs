use crate::brep::{
    CurveGeometry, EdgeGeometry, Frame3, GeometryError, GeometryStore, PcurveGeometry,
    SurfaceGeometry, Topology,
};

pub(crate) fn required_id(id: Option<u32>) -> Result<u32, GeometryError> {
    id.ok_or_else(|| GeometryError::InvalidTopology("boolean selection is missing".into()))
}

pub(super) fn cylinder_geometry_matches(
    actual: &GeometryStore,
    expected: &GeometryStore,
    geometric: f64,
) -> bool {
    if actual.surfaces.len() != expected.surfaces.len()
        || actual.curves.len() != expected.curves.len()
        || actual.pcurves.len() != expected.pcurves.len()
        || !actual.intersections.is_empty()
        || !expected.intersections.is_empty()
    {
        return false;
    }
    let scalar = geometric.max(1e-12);
    let surfaces = actual
        .surfaces
        .iter()
        .zip(&expected.surfaces)
        .all(|(actual, expected)| surface_matches(actual, expected, geometric));
    let curves = actual
        .curves
        .iter()
        .zip(&expected.curves)
        .all(|(actual, expected)| curve_matches(actual, expected, geometric));
    let pcurves = actual
        .pcurves
        .iter()
        .zip(&expected.pcurves)
        .all(|(actual, expected)| pcurve_matches(actual, expected, scalar));
    surfaces && curves && pcurves
}

fn surface_matches(actual: &SurfaceGeometry, expected: &SurfaceGeometry, geometric: f64) -> bool {
    match (actual, expected) {
        (SurfaceGeometry::Plane { frame: actual }, SurfaceGeometry::Plane { frame: expected }) => {
            frames_match(*actual, *expected, geometric)
        }
        (
            SurfaceGeometry::Cylinder {
                frame: actual_frame,
                radius: actual_radius,
            },
            SurfaceGeometry::Cylinder {
                frame: expected_frame,
                radius: expected_radius,
            },
        ) => {
            frames_match(*actual_frame, *expected_frame, geometric)
                && (actual_radius - expected_radius).abs() <= geometric
        }
        (
            SurfaceGeometry::Cone {
                frame: actual_frame,
                semi_angle: actual_angle,
            },
            SurfaceGeometry::Cone {
                frame: expected_frame,
                semi_angle: expected_angle,
            },
        ) => {
            frames_match(*actual_frame, *expected_frame, geometric)
                && (actual_angle - expected_angle).abs() <= 1.0e-12
        }
        _ => false,
    }
}

pub(super) fn frames_match(a: Frame3, b: Frame3, geometric: f64) -> bool {
    coordinates_match(a.origin, b.origin, geometric)
        && coordinates_match(a.x, b.x, 1e-12)
        && coordinates_match(a.y, b.y, 1e-12)
        && coordinates_match(a.z, b.z, 1e-12)
}

pub(super) fn coordinates_match<const N: usize>(a: [f64; N], b: [f64; N], tolerance: f64) -> bool {
    a.into_iter()
        .zip(b)
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn curve_matches(actual: &CurveGeometry, expected: &CurveGeometry, geometric: f64) -> bool {
    match (actual, expected) {
        (
            CurveGeometry::Circle {
                frame: actual_frame,
                radius: actual_radius,
            },
            CurveGeometry::Circle {
                frame: expected_frame,
                radius: expected_radius,
            },
        ) => {
            frames_match(*actual_frame, *expected_frame, geometric)
                && (actual_radius - expected_radius).abs() <= geometric
        }
        (
            CurveGeometry::Line {
                origin: actual_origin,
                direction: actual_direction,
            },
            CurveGeometry::Line {
                origin: expected_origin,
                direction: expected_direction,
            },
        ) => {
            coordinates_match(*actual_origin, *expected_origin, geometric)
                && coordinates_match(*actual_direction, *expected_direction, 1e-12)
        }
        _ => false,
    }
}

fn pcurve_matches(actual: &PcurveGeometry, expected: &PcurveGeometry, scalar: f64) -> bool {
    match (actual, expected) {
        (
            PcurveGeometry::Line2 {
                origin: actual_origin,
                direction: actual_direction,
            },
            PcurveGeometry::Line2 {
                origin: expected_origin,
                direction: expected_direction,
            },
        ) => {
            coordinates_match(*actual_origin, *expected_origin, scalar)
                && coordinates_match(*actual_direction, *expected_direction, 1e-12)
        }
        (
            PcurveGeometry::Conic2 {
                origin: actual_origin,
                axis_a: actual_a,
                axis_b: actual_b,
            },
            PcurveGeometry::Conic2 {
                origin: expected_origin,
                axis_a: expected_a,
                axis_b: expected_b,
            },
        ) => {
            coordinates_match(*actual_origin, *expected_origin, scalar)
                && coordinates_match(*actual_a, *expected_a, scalar)
                && coordinates_match(*actual_b, *expected_b, scalar)
        }
        _ => false,
    }
}

pub(super) fn normalize_cylinder_topology(
    actual: &mut Topology,
    expected: &Topology,
    geometric: f64,
) -> bool {
    if actual.vertices.len() != expected.vertices.len()
        || actual.edges.len() != expected.edges.len()
        || actual.faces.len() != expected.faces.len()
    {
        return false;
    }
    let scalar = geometric.max(1e-12);
    for (actual, expected) in actual.edges.iter_mut().zip(&expected.edges) {
        match (&mut actual.geometry, &expected.geometry) {
            (
                EdgeGeometry::Curve {
                    curve: actual_curve,
                    range: actual_range,
                },
                EdgeGeometry::Curve {
                    curve: expected_curve,
                    range: expected_range,
                },
            ) if actual_curve == expected_curve
                && (actual_range.lo - expected_range.lo).abs() <= scalar
                && (actual_range.hi - expected_range.hi).abs() <= scalar =>
            {
                *actual_range = *expected_range;
            }
            (
                EdgeGeometry::Collapsed { vertex: actual },
                EdgeGeometry::Collapsed { vertex: expected },
            ) if actual == expected => {}
            _ => return false,
        }
    }
    for (actual, expected) in actual.faces.iter_mut().zip(&expected.faces) {
        for axis in 0..2 {
            let a = &mut actual.trim.uv_bounds[axis];
            let e = expected.trim.uv_bounds[axis];
            if (a.lo - e.lo).abs() > scalar || (a.hi - e.hi).abs() > scalar {
                return false;
            }
            *a = e;
        }
    }
    true
}
