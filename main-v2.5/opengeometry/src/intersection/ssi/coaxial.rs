use super::circles::{axial_circles, circle};
use crate::brep::{unit, CurveGeometry, Frame3, GeometryError};
use crate::intersection::ssi_result::GeometryResult;
use crate::math::{
    add, cross, dot, norm, quadratic, scale, sphere_sphere_relation, sub, Interval, QuadraticRoots,
    Sign,
};

pub(super) fn parallel_cylinders(
    a: Frame3,
    ra: f64,
    b: Frame3,
    rb: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(a.z, b.z)) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "cylinder".into()],
        });
    }
    let delta = sub(b.origin, a.origin);
    let radial = sub(delta, scale(a.z, dot(delta, a.z)));
    let distance = norm(radial);
    if distance == 0.0 {
        return Ok(if ra == rb {
            GeometryResult::Coincident
        } else {
            GeometryResult::Empty
        });
    }
    let projected_center = add(a.origin, radial);
    let [outer, inner] = sphere_sphere_relation(a.origin, ra, projected_center, rb)?;
    if outer == Sign::Positive || inner == Sign::Negative {
        return Ok(GeometryResult::Empty);
    }
    let toward_b = unit(radial)?;
    let along = 0.5 * (distance + (ra - rb) * (ra + rb) / distance);
    let base = add(a.origin, scale(toward_b, along));
    if outer == Sign::Zero || inner == Sign::Zero {
        return Ok(GeometryResult::Curves(vec![(
            CurveGeometry::Line {
                origin: base,
                direction: a.z,
            },
            None,
        )]));
    }
    let half_separation_squared = (ra - along.abs()) * (ra + along.abs());
    if half_separation_squared <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "parallel-cylinder branch separation cancellation".into(),
        ));
    }
    let lateral = unit(cross(a.z, toward_b))?;
    let half_separation = half_separation_squared.sqrt();
    Ok(GeometryResult::Curves(
        [-1.0, 1.0]
            .into_iter()
            .map(|sense| {
                (
                    CurveGeometry::Line {
                        origin: add(base, scale(lateral, sense * half_separation)),
                        direction: a.z,
                    },
                    None,
                )
            })
            .collect(),
    ))
}

pub(super) fn coaxial_cylinder_cone(
    cylinder: Frame3,
    radius: f64,
    cone: Frame3,
    semi_angle: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(cylinder.z, cone.z)) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "cone".into()],
        });
    }
    let delta = sub(cylinder.origin, cone.origin);
    if norm(sub(delta, scale(cone.z, dot(delta, cone.z)))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "cone".into()],
        });
    }
    let height = radius / semi_angle.tan();
    if !height.is_finite() || height <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "coaxial cylinder/cone section exceeds numerical range".into(),
        ));
    }
    circle(add(cone.origin, scale(cone.z, height)), cone.z, radius)
}

pub(super) fn coaxial_sphere_cylinder(
    sphere: Frame3,
    sphere_radius: f64,
    cylinder: Frame3,
    cylinder_radius: f64,
) -> Result<GeometryResult, GeometryError> {
    let delta = sub(sphere.origin, cylinder.origin);
    if norm(sub(delta, scale(cylinder.z, dot(delta, cylinder.z)))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "cylinder".into()],
        });
    }
    if cylinder_radius > sphere_radius {
        return Ok(GeometryResult::Empty);
    }
    if cylinder_radius == sphere_radius {
        return circle(sphere.origin, cylinder.z, cylinder_radius);
    }
    let half_height_squared = (sphere_radius - cylinder_radius) * (sphere_radius + cylinder_radius);
    if half_height_squared <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/cylinder branch separation cancellation".into(),
        ));
    }
    let half_height = half_height_squared.sqrt();
    let domain = Some(Interval::new(0.0, std::f64::consts::TAU)?);
    let mut curves = Vec::with_capacity(2);
    for sense in [-1.0, 1.0] {
        let center = add(sphere.origin, scale(cylinder.z, sense * half_height));
        let GeometryResult::Curves(mut circle) = circle(center, cylinder.z, cylinder_radius)?
        else {
            return Err(GeometryError::UnresolvedIntersection(
                "sphere/cylinder circle construction returned a non-curve".into(),
            ));
        };
        let mut curve = circle.pop().ok_or_else(|| {
            GeometryError::UnresolvedIntersection("empty sphere/cylinder circle".into())
        })?;
        curve.1 = domain;
        curves.push(curve);
    }
    Ok(GeometryResult::Curves(curves))
}

pub(super) fn coaxial_sphere_cone(
    sphere: Frame3,
    sphere_radius: f64,
    cone: Frame3,
    semi_angle: f64,
) -> Result<GeometryResult, GeometryError> {
    let delta = sub(sphere.origin, cone.origin);
    let center_height = dot(delta, cone.z);
    if norm(sub(delta, scale(cone.z, center_height))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "cone".into()],
        });
    }
    let slope = semi_angle.tan();
    let roots = quadratic(
        1.0 + slope * slope,
        -2.0 * center_height,
        center_height * center_height - sphere_radius * sphere_radius,
    )?;
    let roots: Vec<f64> = match roots {
        QuadraticRoots::Empty => Vec::new(),
        QuadraticRoots::One(root) => vec![root],
        QuadraticRoots::Two(roots) => roots.into(),
        QuadraticRoots::IdenticallyZero => {
            return Err(GeometryError::UnresolvedIntersection(
                "sphere/cone equation is unexpectedly indeterminate".into(),
            ))
        }
    };
    let apex_contact = roots.contains(&0.0);
    let circles = axial_circles(
        cone.z,
        roots
            .into_iter()
            .filter(|height| *height > 0.0)
            .map(|height| (add(cone.origin, scale(cone.z, height)), slope * height)),
    )?;
    if !apex_contact {
        return Ok(circles);
    }
    Ok(match circles {
        GeometryResult::Empty => GeometryResult::Contact(cone.origin),
        GeometryResult::Curves(curves) => GeometryResult::CurvesAndContacts {
            curves,
            contacts: vec![cone.origin],
        },
        _ => {
            return Err(GeometryError::UnresolvedIntersection(
                "sphere/cone apex classification produced an inconsistent result".into(),
            ))
        }
    })
}

pub(super) fn coaxial_cones(
    a: Frame3,
    angle_a: f64,
    b: Frame3,
    angle_b: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(a.z, b.z)) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cone".into(), "cone".into()],
        });
    }
    let delta = sub(b.origin, a.origin);
    let axial_offset = dot(delta, a.z);
    if norm(sub(delta, scale(a.z, axial_offset))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cone".into(), "cone".into()],
        });
    }
    let same_direction = dot(a.z, b.z) > 0.0;
    if axial_offset == 0.0 {
        return Ok(if same_direction && angle_a == angle_b {
            GeometryResult::Coincident
        } else {
            GeometryResult::Contact(a.origin)
        });
    }
    let ka = angle_a.tan();
    let kb = angle_b.tan();
    let height_a = if same_direction {
        if ka == kb {
            return Ok(GeometryResult::Empty);
        }
        kb * axial_offset / (kb - ka)
    } else {
        kb * axial_offset / (ka + kb)
    };
    let height_b = if same_direction {
        height_a - axial_offset
    } else {
        axial_offset - height_a
    };
    if !height_a.is_finite() || !height_b.is_finite() {
        return Err(GeometryError::UnresolvedIntersection(
            "coaxial cone section exceeds numerical range".into(),
        ));
    }
    if height_a < 0.0 || height_b < 0.0 {
        return Ok(GeometryResult::Empty);
    }
    if height_a == 0.0 || height_b == 0.0 {
        return Ok(GeometryResult::Contact(add(a.origin, scale(a.z, height_a))));
    }
    circle(add(a.origin, scale(a.z, height_a)), a.z, ka * height_a)
}

fn trig_roots(a: f64, b: f64, c: f64) -> Result<Vec<f64>, GeometryError> {
    let amplitude = a.hypot(b);
    if !amplitude.is_finite() || amplitude == 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "degenerate trigonometric intersection equation".into(),
        ));
    }
    let ratio = -c / amplitude;
    if !ratio.is_finite() {
        return Err(GeometryError::UnresolvedIntersection(
            "trigonometric intersection equation exceeds numerical range".into(),
        ));
    }
    if ratio.abs() > 1.0 {
        return Ok(Vec::new());
    }
    let phase = b.atan2(a);
    let delta = ratio.acos();
    Ok(if delta == 0.0 || delta == std::f64::consts::PI {
        vec![phase + delta]
    } else {
        vec![phase - delta, phase + delta]
    })
}

pub(super) fn coaxial_sphere_torus(
    sphere: Frame3,
    sphere_radius: f64,
    torus: Frame3,
    major_radius: f64,
    minor_radius: f64,
) -> Result<GeometryResult, GeometryError> {
    let local = torus.local(sphere.origin);
    if local[0] != 0.0 || local[1] != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["sphere".into(), "torus".into()],
        });
    }
    let roots = trig_roots(
        2.0 * major_radius * minor_radius,
        -2.0 * local[2] * minor_radius,
        major_radius * major_radius + minor_radius * minor_radius + local[2] * local[2]
            - sphere_radius * sphere_radius,
    )?;
    axial_circles(
        torus.z,
        roots.into_iter().map(|v| {
            let center = add(torus.origin, scale(torus.z, minor_radius * v.sin()));
            (center, major_radius + minor_radius * v.cos())
        }),
    )
}

pub(super) fn coaxial_cylinder_torus(
    cylinder: Frame3,
    cylinder_radius: f64,
    torus: Frame3,
    major_radius: f64,
    minor_radius: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(cylinder.z, torus.z)) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "torus".into()],
        });
    }
    let delta = sub(cylinder.origin, torus.origin);
    if norm(sub(delta, scale(torus.z, dot(delta, torus.z)))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "torus".into()],
        });
    }
    let roots = trig_roots(minor_radius, 0.0, major_radius - cylinder_radius)?;
    axial_circles(
        torus.z,
        roots.into_iter().map(|v| {
            (
                add(torus.origin, scale(torus.z, minor_radius * v.sin())),
                cylinder_radius,
            )
        }),
    )
}

pub(super) fn coaxial_cone_torus(
    cone: Frame3,
    semi_angle: f64,
    torus: Frame3,
    major_radius: f64,
    minor_radius: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(cone.z, torus.z)) != 0.0 || dot(cone.z, torus.z) < 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cone".into(), "torus".into()],
        });
    }
    let delta = sub(torus.origin, cone.origin);
    let height = dot(delta, cone.z);
    if norm(sub(delta, scale(cone.z, height))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["cone".into(), "torus".into()],
        });
    }
    let slope = semi_angle.tan();
    let roots = trig_roots(
        minor_radius,
        -slope * minor_radius,
        major_radius - slope * height,
    )?;
    axial_circles(
        torus.z,
        roots.into_iter().filter_map(|v| {
            let z = height + minor_radius * v.sin();
            (z >= 0.0).then(|| {
                (
                    add(torus.origin, scale(torus.z, minor_radius * v.sin())),
                    major_radius + minor_radius * v.cos(),
                )
            })
        }),
    )
}

pub(super) fn coaxial_tori(
    a: Frame3,
    major_a: f64,
    minor_a: f64,
    b: Frame3,
    major_b: f64,
    minor_b: f64,
) -> Result<GeometryResult, GeometryError> {
    if norm(cross(a.z, b.z)) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["torus".into(), "torus".into()],
        });
    }
    let delta = sub(b.origin, a.origin);
    let axial_offset = dot(delta, a.z);
    if norm(sub(delta, scale(a.z, axial_offset))) != 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["torus".into(), "torus".into()],
        });
    }
    if axial_offset == 0.0 && major_a == major_b && minor_a == minor_b {
        return Ok(GeometryResult::Coincident);
    }
    let radial_offset = major_b - major_a;
    let distance = radial_offset.hypot(axial_offset);
    if distance == 0.0 {
        return Ok(GeometryResult::Empty);
    }
    let [outer, inner] = sphere_sphere_relation(
        [major_a, 0.0, 0.0],
        minor_a,
        [major_b, 0.0, axial_offset],
        minor_b,
    )?;
    if outer == Sign::Positive || inner == Sign::Negative {
        return Ok(GeometryResult::Empty);
    }
    let er = radial_offset / distance;
    let ez = axial_offset / distance;
    let along = 0.5 * (distance + (minor_a - minor_b) * (minor_a + minor_b) / distance);
    let base_r = major_a + er * along;
    let base_z = ez * along;
    let sections = if outer == Sign::Zero || inner == Sign::Zero {
        vec![(base_r, base_z)]
    } else {
        let half_squared = (minor_a - along.abs()) * (minor_a + along.abs());
        if half_squared <= 0.0 {
            return Err(GeometryError::UnresolvedIntersection(
                "coaxial torus branch separation cancellation".into(),
            ));
        }
        let half = half_squared.sqrt();
        vec![
            (base_r - ez * half, base_z + er * half),
            (base_r + ez * half, base_z - er * half),
        ]
    };
    axial_circles(
        a.z,
        sections
            .into_iter()
            .map(|(radius, z)| (add(a.origin, scale(a.z, z)), radius)),
    )
}
