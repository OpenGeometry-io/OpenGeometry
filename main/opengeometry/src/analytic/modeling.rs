use super::{
    geometry::{cross, dot, norm, scale, sub, unit},
    primitives::{self, boundary, uv_line, Builder},
    topology::{Accuracy, BrepEnvelope, Orientation, PcurveGeometry},
    CurveGeometry, Frame3, GeometryError, Point3, SurfaceGeometry,
};
use crate::math::interval::Interval;

#[derive(Clone, Copy, Debug)]
pub enum PolygonLoftAlignment {
    Auto,
    Indexed {
        upper_start: usize,
        reverse_upper: bool,
    },
}

fn polygon_normal(points: &[Point3]) -> Result<Point3, GeometryError> {
    let mut normal = [0.0; 3];
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    unit(normal).map_err(|_| GeometryError::InvalidGeometry("loft profile has zero area".into()))
}

fn polygon_centroid(points: &[Point3]) -> Point3 {
    let count = points.len() as f64;
    std::array::from_fn(|axis| points.iter().map(|point| point[axis]).sum::<f64>() / count)
}

fn validate_loft_profile(
    points: &[Point3],
    accuracy: Accuracy,
    label: &str,
) -> Result<Point3, GeometryError> {
    use crate::geometry::poly2d::{self_intersects2, Pt2};

    if points.len() < 3 {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} loft profile requires at least three vertices"
        )));
    }
    if points
        .iter()
        .flatten()
        .any(|coordinate| !coordinate.is_finite())
    {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} loft profile contains a non-finite coordinate"
        )));
    }
    let resolution = 4.0 * accuracy.geometric;
    if points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .any(|(a, b)| norm(sub(*b, *a)) <= resolution)
    {
        return Err(GeometryError::UnresolvedIntersection(format!(
            "{label} loft profile contains an edge below geometric resolution"
        )));
    }
    let normal = polygon_normal(points)?;
    let origin = points[0];
    let reference = sub(points[1], origin);
    let frame = Frame3::from_axis(origin, normal, reference)?;
    let mut projected = Vec::with_capacity(points.len());
    for point in points {
        let local = frame.local(*point);
        if local[2].abs() > resolution {
            return Err(GeometryError::InvalidGeometry(format!(
                "{label} loft profile is not planar"
            )));
        }
        projected.push(Pt2::new(local[0], local[1]));
    }
    if self_intersects2(&projected, accuracy.geometric) {
        return Err(GeometryError::InvalidGeometry(format!(
            "{label} loft profile self-intersects"
        )));
    }
    Ok(normal)
}

fn rotate_profile(points: &[Point3], start: usize, reverse: bool) -> Vec<Point3> {
    let count = points.len();
    (0..count)
        .map(|offset| {
            let index = if reverse {
                (start + count - (offset % count)) % count
            } else {
                (start + offset) % count
            };
            points[index]
        })
        .collect()
}

fn connector_score(lower: &[Point3], upper: &[Point3]) -> f64 {
    lower
        .iter()
        .zip(upper)
        .map(|(a, b)| {
            let delta = sub(*b, *a);
            dot(delta, delta)
        })
        .sum()
}

fn align_loft_profiles(
    lower: Vec<Point3>,
    upper: Vec<Point3>,
    alignment: PolygonLoftAlignment,
    accuracy: Accuracy,
) -> Result<(Vec<Point3>, Vec<Point3>), GeometryError> {
    let separation = sub(polygon_centroid(&upper), polygon_centroid(&lower));
    if norm(separation) <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "loft profile planes are not separated".into(),
        ));
    }
    let lower_normal = polygon_normal(&lower)?;
    let mut lower = lower;
    if dot(lower_normal, separation) < 0.0 {
        lower.reverse();
    }

    let upper = match alignment {
        PolygonLoftAlignment::Indexed {
            upper_start,
            reverse_upper,
        } => {
            if upper_start >= upper.len() {
                return Err(GeometryError::InvalidGeometry(
                    "loft upper_start is outside the upper profile".into(),
                ));
            }
            rotate_profile(&upper, upper_start, reverse_upper)
        }
        PolygonLoftAlignment::Auto => {
            let mut candidates = Vec::with_capacity(upper.len() * 2);
            for reverse in [false, true] {
                for start in 0..upper.len() {
                    let candidate = rotate_profile(&upper, start, reverse);
                    let candidate_normal = polygon_normal(&candidate)?;
                    if dot(candidate_normal, separation) <= 0.0 {
                        continue;
                    }
                    candidates.push((
                        connector_score(&lower, &candidate),
                        start,
                        reverse,
                        candidate,
                    ));
                }
            }
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            let Some(best) = candidates.first() else {
                return Err(GeometryError::InvalidGeometry(
                    "loft profiles do not have compatible winding".into(),
                ));
            };
            if let Some(second) = candidates.get(1) {
                let score_tolerance = accuracy.geometric * accuracy.geometric * lower.len() as f64;
                if (second.0 - best.0).abs() <= score_tolerance {
                    return Err(GeometryError::AmbiguousProfileAlignment(
                        "automatic loft correspondence has multiple equally good solutions; provide an indexed alignment".into(),
                    ));
                }
            }
            best.3.clone()
        }
    };
    Ok((lower, upper))
}

pub fn polygon_loft(
    id: String,
    lower: Vec<Point3>,
    upper: Vec<Point3>,
    alignment: PolygonLoftAlignment,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    accuracy.validate()?;
    if lower.len() != upper.len() {
        return Err(GeometryError::UnsupportedGeometry(
            "polygon loft profiles must have the same vertex count".into(),
        ));
    }
    let lower_normal = validate_loft_profile(&lower, accuracy, "lower")?;
    let upper_normal = validate_loft_profile(&upper, accuracy, "upper")?;
    let centroid_delta = sub(polygon_centroid(&upper), polygon_centroid(&lower));
    if dot(lower_normal, centroid_delta).abs() <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "loft profile planes have no resolved normal separation".into(),
        ));
    }
    let angular_tolerance =
        (accuracy.geometric / norm(centroid_delta).max(accuracy.geometric)).clamp(1.0e-12, 1.0e-6);
    if dot(lower_normal, upper_normal).abs() < 1.0 - angular_tolerance {
        return Err(GeometryError::UnsupportedGeometry(
            "polygon loft currently requires parallel profile planes".into(),
        ));
    }
    let (lower, upper) = align_loft_profiles(lower, upper, alignment, accuracy)?;
    let count = lower.len();
    let mut vertices = lower.clone();
    vertices.extend_from_slice(&upper);

    let mut faces = Vec::with_capacity(count + 2);
    faces.push((0..count as u32).rev().collect());
    faces.push(
        (0..count as u32)
            .map(|index| index + count as u32)
            .collect(),
    );
    for index in 0..count {
        let next = (index + 1) % count;
        let quad = [lower[index], lower[next], upper[next], upper[index]];
        let base_normal = cross(sub(quad[1], quad[0]), sub(quad[3], quad[0]));
        let denominator = norm(base_normal);
        if denominator <= 4.0 * accuracy.geometric {
            return Err(GeometryError::UnresolvedIntersection(
                "loft side face collapses below geometric resolution".into(),
            ));
        }
        let distance = dot(base_normal, sub(quad[2], quad[0])).abs() / denominator;
        if distance > 4.0 * accuracy.geometric {
            return Err(GeometryError::UnsupportedGeometry(
                "polygon loft side is warped and requires a ruled surface outside the current analytic surface set".into(),
            ));
        }
        faces.push(vec![
            index as u32,
            next as u32,
            (next + count) as u32,
            (index + count) as u32,
        ]);
    }
    primitives::planar_polyhedron(id, vertices, faces, accuracy)
}

pub fn coaxial_circle_loft(
    id: String,
    frame: Frame3,
    lower_radius: f64,
    upper_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    if !lower_radius.is_finite()
        || !upper_radius.is_finite()
        || lower_radius < 0.0
        || upper_radius < 0.0
        || (lower_radius == 0.0 && upper_radius == 0.0)
    {
        return Err(GeometryError::InvalidGeometry(
            "coaxial circle loft requires at least one positive finite radius".into(),
        ));
    }
    if lower_radius == 0.0 {
        let reversed = Frame3 {
            origin: frame.point([0.0, 0.0, height]),
            x: frame.x,
            y: [-frame.y[0], -frame.y[1], -frame.y[2]],
            z: [-frame.z[0], -frame.z[1], -frame.z[2]],
        };
        return primitives::cone(id, reversed, upper_radius, height, accuracy);
    }
    primitives::frustum(id, frame, lower_radius, upper_radius, height, accuracy)
}

pub fn revolve_rectangle(
    id: String,
    frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    if !inner_radius.is_finite() || inner_radius < 0.0 {
        return Err(GeometryError::InvalidGeometry(
            "revolve inner radius must be finite and nonnegative".into(),
        ));
    }
    if inner_radius == 0.0 {
        primitives::cylinder(id, frame, outer_radius, height, accuracy)
    } else {
        primitives::annular_cylinder(id, frame, inner_radius, outer_radius, height, accuracy)
    }
}

pub fn chamfered_cuboid(
    id: String,
    frame: Frame3,
    size: [f64; 3],
    chamfer: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    if size
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
        || !chamfer.is_finite()
        || chamfer <= 0.0
    {
        return Err(GeometryError::InvalidGeometry(
            "chamfered cuboid dimensions must be positive and finite".into(),
        ));
    }
    let [width, depth, height] = size;
    if 2.0 * chamfer >= width.min(depth) {
        return Err(GeometryError::InvalidGeometry(
            "chamfer must be smaller than half the plan dimensions".into(),
        ));
    }
    if chamfer <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "chamfer is below geometric resolution".into(),
        ));
    }
    primitives::linear_extrusion(
        id,
        frame,
        vec![
            [chamfer, 0.0],
            [width - chamfer, 0.0],
            [width, chamfer],
            [width, depth - chamfer],
            [width - chamfer, depth],
            [chamfer, depth],
            [0.0, depth - chamfer],
            [0.0, chamfer],
        ],
        Vec::new(),
        height,
        accuracy,
    )
}

pub fn filleted_cylinder(
    id: String,
    frame: Frame3,
    radius: f64,
    height: f64,
    fillet_radius: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    accuracy.validate()?;
    if [radius, height, fillet_radius]
        .into_iter()
        .any(|value| !value.is_finite() || value <= 0.0)
    {
        return Err(GeometryError::InvalidGeometry(
            "filleted cylinder dimensions must be positive and finite".into(),
        ));
    }
    if fillet_radius * 2.0 >= radius || fillet_radius * 2.0 >= height {
        return Err(GeometryError::UnsupportedGeometry(
            "filleted cylinder requires a ring-torus fillet smaller than half its radius and height".into(),
        ));
    }
    if fillet_radius <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "fillet radius is below geometric resolution".into(),
        ));
    }

    let mut builder = Builder::new(id, accuracy)?;
    let tau = std::f64::consts::TAU;
    let quarter = std::f64::consts::FRAC_PI_2;
    let major = radius - fillet_radius;
    let levels = [0.0, fillet_radius, height - fillet_radius, height];
    let radii = [major, radius, radius, major];
    let vertices: [u32; 4] = std::array::from_fn(|index| {
        builder.vertex(frame.point([radii[index], 0.0, levels[index]]))
    });
    let mut ring_frames = [frame; 4];
    for (index, ring_frame) in ring_frames.iter_mut().enumerate() {
        ring_frame.origin = frame.point([0.0, 0.0, levels[index]]);
    }
    let circle_range = Interval::new(0.0, tau)?;
    let mut rings = Vec::with_capacity(4);
    for index in 0..4 {
        rings.push(builder.edge(
            CurveGeometry::Circle {
                frame: ring_frames[index],
                radius: radii[index],
            },
            circle_range,
            false,
        ));
    }
    let tube_frame = |center_height| Frame3 {
        origin: frame.point([major, 0.0, center_height]),
        x: frame.x,
        y: frame.z,
        z: scale(frame.y, -1.0),
    };
    let lower_seam = builder.edge(
        CurveGeometry::Circle {
            frame: tube_frame(fillet_radius),
            radius: fillet_radius,
        },
        Interval::new(-quarter, 0.0)?,
        true,
    );
    let upper_seam = builder.edge(
        CurveGeometry::Circle {
            frame: tube_frame(height - fillet_radius),
            radius: fillet_radius,
        },
        Interval::new(0.0, quarter)?,
        true,
    );
    let cylinder_seam = builder.edge(
        CurveGeometry::Line {
            origin: frame.point([radius, 0.0, fillet_radius]),
            direction: frame.z,
        },
        Interval::new(0.0, height - 2.0 * fillet_radius)?,
        true,
    );

    use Orientation::{Forward as F, Reverse as R};
    let mut lower_frame = frame;
    lower_frame.origin = frame.point([0.0, 0.0, fillet_radius]);
    builder.face(
        "lower_fillet",
        SurfaceGeometry::Torus {
            frame: lower_frame,
            major_radius: major,
            minor_radius: fillet_radius,
        },
        [[0.0, tau], [-quarter, 0.0]],
        vec![
            boundary(
                rings[0],
                vertices[0],
                vertices[0],
                F,
                uv_line([0.0, -quarter], [1.0, 0.0]),
            ),
            boundary(
                lower_seam,
                vertices[0],
                vertices[1],
                F,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                rings[1],
                vertices[1],
                vertices[1],
                R,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                lower_seam,
                vertices[1],
                vertices[0],
                R,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    builder.face(
        "lateral",
        SurfaceGeometry::Cylinder { frame, radius },
        [[0.0, tau], [fillet_radius, height - fillet_radius]],
        vec![
            boundary(
                rings[1],
                vertices[1],
                vertices[1],
                F,
                uv_line([0.0, fillet_radius], [1.0, 0.0]),
            ),
            boundary(
                cylinder_seam,
                vertices[1],
                vertices[2],
                F,
                uv_line([tau, fillet_radius], [0.0, 1.0]),
            ),
            boundary(
                rings[2],
                vertices[2],
                vertices[2],
                R,
                uv_line([0.0, height - fillet_radius], [1.0, 0.0]),
            ),
            boundary(
                cylinder_seam,
                vertices[2],
                vertices[1],
                R,
                uv_line([0.0, fillet_radius], [0.0, 1.0]),
            ),
        ],
    )?;
    let mut upper_frame = frame;
    upper_frame.origin = frame.point([0.0, 0.0, height - fillet_radius]);
    builder.face(
        "upper_fillet",
        SurfaceGeometry::Torus {
            frame: upper_frame,
            major_radius: major,
            minor_radius: fillet_radius,
        },
        [[0.0, tau], [0.0, quarter]],
        vec![
            boundary(
                rings[2],
                vertices[2],
                vertices[2],
                F,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                upper_seam,
                vertices[2],
                vertices[3],
                F,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                rings[3],
                vertices[3],
                vertices[3],
                R,
                uv_line([0.0, quarter], [1.0, 0.0]),
            ),
            boundary(
                upper_seam,
                vertices[3],
                vertices[2],
                R,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    let bottom_frame = Frame3 {
        y: scale(frame.y, -1.0),
        z: scale(frame.z, -1.0),
        ..frame
    };
    builder.face(
        "lower_cap",
        SurfaceGeometry::Plane {
            frame: bottom_frame,
        },
        [[-major, major]; 2],
        vec![boundary(
            rings[0],
            vertices[0],
            vertices[0],
            R,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [major, 0.0],
                axis_b: [0.0, -major],
            },
        )],
    )?;
    let mut cap_frame = frame;
    cap_frame.origin = frame.point([0.0, 0.0, height]);
    builder.face(
        "upper_cap",
        SurfaceGeometry::Plane { frame: cap_frame },
        [[-major, major]; 2],
        vec![boundary(
            rings[3],
            vertices[3],
            vertices[3],
            F,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [major, 0.0],
                axis_b: [0.0, major],
            },
        )],
    )?;
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::SurfaceGeometry;

    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-5,
        }
    }

    #[test]
    fn coaxial_circle_loft_closes_over_cylinder_cone_and_frustum() {
        for (lower, upper, expected_faces) in [(1.0, 1.0, 3), (1.0, 0.0, 2), (1.0, 0.5, 3)] {
            let result = coaxial_circle_loft(
                format!("loft-{lower}-{upper}"),
                Frame3::IDENTITY,
                lower,
                upper,
                2.0,
                accuracy(),
            )
            .unwrap();
            assert_eq!(result.topology.faces.len(), expected_faces);
            assert!(matches!(
                result.geometry.surfaces[0],
                SurfaceGeometry::Cylinder { .. } | SurfaceGeometry::Cone { .. }
            ));
            result.validate().unwrap();
        }
        let apex_first =
            coaxial_circle_loft("apex".into(), Frame3::IDENTITY, 0.0, 1.0, 2.0, accuracy())
                .unwrap();
        apex_first.validate().unwrap();
    }

    #[test]
    fn polygon_loft_matches_reversed_shifted_sections_and_builds_shared_planar_faces() {
        let lower = vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let upper = vec![
            [0.6, 0.6, 2.0],
            [0.6, -0.6, 2.0],
            [-0.6, -0.6, 2.0],
            [-0.6, 0.6, 2.0],
        ];
        let result = polygon_loft(
            "polygon-loft".into(),
            lower,
            upper,
            PolygonLoftAlignment::Auto,
            accuracy(),
        )
        .unwrap();
        assert_eq!(result.topology.vertices.len(), 8);
        assert_eq!(result.topology.edges.len(), 12);
        assert_eq!(result.topology.faces.len(), 6);
        assert!(result
            .geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. })));
        result.validate().unwrap();
        crate::analytic::tessellation::tessellate(&result, 0.01, 2_000_000).unwrap();
    }

    #[test]
    fn polygon_loft_rejects_unequal_or_warped_sections() {
        let lower = vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ];
        assert!(matches!(
            polygon_loft(
                "count".into(),
                lower.clone(),
                vec![[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
                PolygonLoftAlignment::Auto,
                accuracy(),
            ),
            Err(GeometryError::UnsupportedGeometry(_))
        ));
        assert!(matches!(
            polygon_loft(
                "warped".into(),
                lower,
                vec![
                    [0.0, 0.0, 1.0],
                    [2.0, 0.0, 1.0],
                    [1.5, 2.0, 1.0],
                    [0.0, 2.0, 1.0],
                ],
                PolygonLoftAlignment::Indexed {
                    upper_start: 0,
                    reverse_upper: false,
                },
                accuracy(),
            ),
            Err(GeometryError::UnsupportedGeometry(_))
        ));
        assert!(matches!(
            polygon_loft(
                "coplanar".into(),
                vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
                vec![
                    [2.0, 0.0, 0.0],
                    [3.0, 0.0, 0.0],
                    [3.0, 1.0, 0.0],
                    [2.0, 1.0, 0.0],
                ],
                PolygonLoftAlignment::Auto,
                accuracy(),
            ),
            Err(GeometryError::UnresolvedIntersection(_))
        ));
    }

    #[test]
    fn rectangular_revolve_keeps_cylindrical_hole_analytic() {
        let result =
            revolve_rectangle("ring".into(), Frame3::IDENTITY, 0.5, 1.0, 2.0, accuracy()).unwrap();
        assert_eq!(result.topology.faces.len(), 4);
        assert!(matches!(
            result.geometry.surfaces[0],
            SurfaceGeometry::Cylinder { radius: 1.0, .. }
        ));
        assert!(matches!(
            result.geometry.surfaces[1],
            SurfaceGeometry::Cylinder { radius: 0.5, .. }
        ));
    }

    #[test]
    fn planar_chamfer_rebuilds_a_valid_all_plane_solid() {
        let result = chamfered_cuboid(
            "chamfer".into(),
            Frame3::IDENTITY,
            [4.0, 3.0, 2.0],
            0.25,
            accuracy(),
        )
        .unwrap();
        assert_eq!(result.topology.faces.len(), 10);
        assert!(result
            .geometry
            .surfaces
            .iter()
            .all(|surface| matches!(surface, SurfaceGeometry::Plane { .. })));
        result.validate().unwrap();
    }

    #[test]
    fn circular_edge_fillet_uses_torus_quarters_and_shared_circles() {
        let result =
            filleted_cylinder("fillet".into(), Frame3::IDENTITY, 2.0, 4.0, 0.3, accuracy())
                .unwrap();
        assert_eq!(result.topology.faces.len(), 5);
        assert_eq!(
            result
                .geometry
                .surfaces
                .iter()
                .filter(|surface| matches!(surface, SurfaceGeometry::Torus { .. }))
                .count(),
            2
        );
        result.validate().unwrap();
        crate::analytic::tessellation::tessellate(&result, 0.02, 2_000_000).unwrap();
    }
}
