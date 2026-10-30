use super::circles::{axial_circles, circle};
use crate::brep::{unit, Accuracy, CurveGeometry, Frame3, GeometryError};
use crate::intersection::ssi_result::GeometryResult;
use crate::math::{
    add, cross, dot, norm, plane_sphere_relation, scale, solve, sphere_sphere_relation, sub,
    Interval, Sign,
};

pub(super) fn plane_plane(
    a: Frame3,
    b: Frame3,
    accuracy: Accuracy,
) -> Result<GeometryResult, GeometryError> {
    let tangent = cross(a.z, b.z);
    let length = norm(tangent);
    let delta = sub(b.origin, a.origin);
    if length == 0.0 {
        return Ok(if dot(a.z, delta).abs() <= accuracy.intersection {
            GeometryResult::Coincident
        } else {
            GeometryResult::Empty
        });
    }
    let tangent = unit(tangent)?;
    let solution = solve([a.z, b.z, tangent], [0.0, dot(b.z, delta), 0.0])
        .map_err(|e| GeometryError::UnresolvedIntersection(format!("plane intersection: {e}")))?;
    Ok(GeometryResult::Curves(vec![(
        CurveGeometry::Line {
            origin: add(a.origin, solution.value),
            direction: tangent,
        },
        None,
    )]))
}

pub(super) fn plane_sphere(
    plane: Frame3,
    sphere: Frame3,
    radius: f64,
) -> Result<GeometryResult, GeometryError> {
    let relation = plane_sphere_relation(plane.origin, plane.z, sphere.origin, radius)?;
    if relation == Sign::Positive {
        return Ok(GeometryResult::Empty);
    }
    let n2 = dot(plane.z, plane.z);
    let distance = dot(sub(sphere.origin, plane.origin), plane.z);
    let center = sub(sphere.origin, scale(plane.z, distance / n2));
    if relation == Sign::Zero {
        return Ok(GeometryResult::Contact(center));
    }
    let d = distance.abs() / n2.sqrt();
    let r2 = (radius - d) * (radius + d);
    if r2 <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "plane/sphere radius cancellation".into(),
        ));
    }
    circle(center, plane.z, r2.sqrt())
}

pub(super) fn sphere_sphere(
    a: Frame3,
    ra: f64,
    b: Frame3,
    rb: f64,
) -> Result<GeometryResult, GeometryError> {
    let delta = sub(b.origin, a.origin);
    let distance = norm(delta);
    if distance == 0.0 {
        return Ok(if ra == rb {
            GeometryResult::Coincident
        } else {
            GeometryResult::Empty
        });
    }
    let [outer, inner] = sphere_sphere_relation(a.origin, ra, b.origin, rb)?;
    if outer == Sign::Positive || inner == Sign::Negative {
        return Ok(GeometryResult::Empty);
    }
    let axis = unit(delta)?;
    let x = 0.5 * (distance + (ra - rb) * (ra + rb) / distance);
    let center = add(a.origin, scale(axis, x));
    if outer == Sign::Zero || inner == Sign::Zero {
        return Ok(GeometryResult::Contact(center));
    }
    let r2 = (ra - x.abs()) * (ra + x.abs());
    if r2 <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "sphere/sphere radius cancellation".into(),
        ));
    }
    circle(center, axis, r2.sqrt())
}

pub(super) fn plane_cylinder(
    plane: Frame3,
    cylinder: Frame3,
    radius: f64,
) -> Result<GeometryResult, GeometryError> {
    let axial = dot(plane.z, cylinder.z);
    let offset = dot(sub(cylinder.origin, plane.origin), plane.z);
    if axial != 0.0 {
        let center = add(cylinder.origin, scale(cylinder.z, -offset / axial));
        if axial.abs() == 1.0 {
            return circle(center, plane.z, radius);
        }
        let major_direction = unit(sub(cylinder.z, scale(plane.z, axial)))?;
        let major_radius = radius / axial.abs();
        if !major_radius.is_finite() {
            return Err(GeometryError::UnresolvedIntersection(
                "plane/cylinder ellipse exceeds numerical range".into(),
            ));
        }
        let frame = Frame3::from_axis(center, plane.z, major_direction)?;
        return Ok(GeometryResult::Curves(vec![(
            CurveGeometry::Ellipse {
                frame,
                major_radius,
                minor_radius: radius,
            },
            Some(Interval::new(0.0, std::f64::consts::TAU)?),
        )]));
    }

    let relation = plane_sphere_relation(plane.origin, plane.z, cylinder.origin, radius)?;
    if relation == Sign::Positive {
        return Ok(GeometryResult::Empty);
    }
    let base = add(cylinder.origin, scale(plane.z, -offset));
    if relation == Sign::Zero {
        return Ok(GeometryResult::Curves(vec![(
            CurveGeometry::Line {
                origin: base,
                direction: cylinder.z,
            },
            None,
        )]));
    }
    let lateral = unit(cross(cylinder.z, plane.z))?;
    let half_separation_squared = (radius - offset.abs()) * (radius + offset.abs());
    if half_separation_squared <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "plane/cylinder generator separation cancellation".into(),
        ));
    }
    let half_separation = half_separation_squared.sqrt();
    Ok(GeometryResult::Curves(
        [-1.0, 1.0]
            .into_iter()
            .map(|sense| {
                (
                    CurveGeometry::Line {
                        origin: add(base, scale(lateral, sense * half_separation)),
                        direction: cylinder.z,
                    },
                    None,
                )
            })
            .collect(),
    ))
}

pub(super) fn plane_cone(
    plane: Frame3,
    cone: Frame3,
    semi_angle: f64,
) -> Result<GeometryResult, GeometryError> {
    let axial = dot(plane.z, cone.z);
    let tilt = norm(cross(plane.z, cone.z));
    if tilt == 0.0 {
        let height = dot(sub(plane.origin, cone.origin), cone.z);
        if height < 0.0 {
            return Ok(GeometryResult::Empty);
        }
        if height == 0.0 {
            return Ok(GeometryResult::Contact(cone.origin));
        }
        let radius = height * semi_angle.tan();
        if !radius.is_finite() || radius <= 0.0 {
            return Err(GeometryError::UnresolvedIntersection(
                "plane/cone circle exceeds numerical range".into(),
            ));
        }
        return circle(add(cone.origin, scale(cone.z, height)), plane.z, radius);
    }
    if axial == 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["plane".into(), "cone".into()],
        });
    }
    let height = dot(sub(plane.origin, cone.origin), plane.z) / axial;
    let slope = semi_angle.tan();
    let coefficient = axial * axial - slope * slope * tilt * tilt;
    if coefficient <= 0.0 {
        return Err(GeometryError::CoverageGap {
            families: ["plane".into(), "cone".into()],
        });
    }
    if height < 0.0 {
        return Ok(GeometryResult::Empty);
    }
    if height == 0.0 {
        return Ok(GeometryResult::Contact(cone.origin));
    }
    let cross_axis = unit(cross(plane.z, cone.z))?;
    let down_slope = unit(cross(plane.z, cross_axis))?;
    let center_shift = -slope * slope * height * tilt / coefficient;
    let center = add(
        add(cone.origin, scale(cone.z, height)),
        scale(down_slope, center_shift),
    );
    let minor_radius = slope * height.abs() * axial.abs() / coefficient.sqrt();
    let major_radius = minor_radius / coefficient.sqrt();
    if !major_radius.is_finite() || !minor_radius.is_finite() || minor_radius <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "plane/cone ellipse exceeds numerical range".into(),
        ));
    }
    let frame = Frame3::from_axis(center, plane.z, down_slope)?;
    Ok(GeometryResult::Curves(vec![(
        CurveGeometry::Ellipse {
            frame,
            major_radius,
            minor_radius,
        },
        Some(Interval::new(0.0, std::f64::consts::TAU)?),
    )]))
}

pub(super) fn plane_torus(
    plane: Frame3,
    torus: Frame3,
    major_radius: f64,
    minor_radius: f64,
) -> Result<GeometryResult, GeometryError> {
    let parallel = norm(cross(plane.z, torus.z)) == 0.0;
    if parallel {
        let height = dot(sub(plane.origin, torus.origin), torus.z);
        if height.abs() > minor_radius {
            return Ok(GeometryResult::Empty);
        }
        let center = add(torus.origin, scale(torus.z, height));
        if height.abs() == minor_radius {
            return axial_circles(plane.z, [(center, major_radius)]);
        }
        let radial = ((minor_radius - height.abs()) * (minor_radius + height.abs())).sqrt();
        return axial_circles(
            plane.z,
            [
                (center, major_radius - radial),
                (center, major_radius + radial),
            ],
        );
    }
    if dot(plane.z, torus.z) == 0.0 && dot(sub(torus.origin, plane.origin), plane.z) == 0.0 {
        let radial = unit(cross(plane.z, torus.z))?;
        return axial_circles(
            plane.z,
            [
                (add(torus.origin, scale(radial, major_radius)), minor_radius),
                (
                    add(torus.origin, scale(radial, -major_radius)),
                    minor_radius,
                ),
            ],
        );
    }
    Err(GeometryError::CoverageGap {
        families: ["plane".into(), "torus".into()],
    })
}
