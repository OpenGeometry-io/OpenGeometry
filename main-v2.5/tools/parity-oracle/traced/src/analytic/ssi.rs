use super::{
    geometry::{add, cross, dot, norm, scale, sub, unit, PatchBounds, Surface},
    topology::{Accuracy, GeometryStore, PcurveGeometry},
    CurveGeometry, Frame3, GeometryError, Point3, SurfaceGeometry,
};
use crate::math::{
    interval::Interval,
    predicates::{plane_sphere_relation, sphere_sphere_relation, Sign},
    roots::{quadratic, QuadraticRoots},
    solve::solve,
};

#[derive(Clone, Debug)]
pub struct SsiCurve {
    pub curve: u32,
    pub pcurves: [u32; 2],
    pub domain: Option<Interval>,
}
#[derive(Clone, Debug, Default)]
pub struct SsiResult {
    pub curves: Vec<SsiCurve>,
    pub contacts: Vec<Point3>,
    pub coincident: bool,
}

enum GeometryResult {
    Empty,
    Curves(Vec<(CurveGeometry, Option<Interval>)>),
    CurvesAndContacts {
        curves: Vec<(CurveGeometry, Option<Interval>)>,
        contacts: Vec<Point3>,
    },
    Contact(Point3),
    Coincident,
}

fn circle(center: Point3, normal: Point3, radius: f64) -> Result<GeometryResult, GeometryError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(GeometryError::UnresolvedIntersection(
            "positive intersection radius is not representable".into(),
        ));
    }
    let reference = if normal[0].abs() < 0.8 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let frame = Frame3::from_axis(center, normal, reference)?;
    Ok(GeometryResult::Curves(vec![(
        CurveGeometry::Circle { frame, radius },
        Some(Interval::new(0.0, std::f64::consts::TAU)?),
    )]))
}

fn plane_plane(a: Frame3, b: Frame3, accuracy: Accuracy) -> Result<GeometryResult, GeometryError> {
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

fn plane_sphere(
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

fn sphere_sphere(a: Frame3, ra: f64, b: Frame3, rb: f64) -> Result<GeometryResult, GeometryError> {
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

fn plane_cylinder(
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

fn plane_cone(
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

fn parallel_cylinders(
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

fn coaxial_cylinder_cone(
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

fn coaxial_sphere_cylinder(
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

fn coaxial_sphere_cone(
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

fn coaxial_cones(
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

fn axial_circles(
    normal: Point3,
    sections: impl IntoIterator<Item = (Point3, f64)>,
) -> Result<GeometryResult, GeometryError> {
    let mut curves = Vec::new();
    for (center, radius) in sections {
        let GeometryResult::Curves(mut section) = circle(center, normal, radius)? else {
            return Err(GeometryError::UnresolvedIntersection(
                "axial circle construction returned a non-curve".into(),
            ));
        };
        curves.append(&mut section);
    }
    Ok(if curves.is_empty() {
        GeometryResult::Empty
    } else {
        GeometryResult::Curves(curves)
    })
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

fn plane_torus(
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

fn coaxial_sphere_torus(
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

fn coaxial_cylinder_torus(
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

fn coaxial_cone_torus(
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

fn coaxial_tori(
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

fn chart_domain(surface: &SurfaceGeometry) -> Result<[Interval; 2], GeometryError> {
    let chart = surface
        .charts()
        .first()
        .ok_or_else(|| GeometryError::InvalidGeometry("surface has no parameter chart".into()))?;
    Ok([
        Interval::new(chart.domain[0][0], chart.domain[0][1])?,
        Interval::new(chart.domain[1][0], chart.domain[1][1])?,
    ])
}

fn finite_target_domain(
    surface: &SurfaceGeometry,
    source_bounds: PatchBounds,
    margin: f64,
) -> Result<[Interval; 2], GeometryError> {
    if matches!(
        surface,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        return chart_domain(surface);
    }
    let mut local_min = [f64::INFINITY; 3];
    let mut local_max = [f64::NEG_INFINITY; 3];
    let mut maximum_radius = 0.0_f64;
    for mask in 0..8 {
        let point = std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                source_bounds.axes[axis].lo
            } else {
                source_bounds.axes[axis].hi
            }
        });
        let local = surface.frame().local(point);
        for axis in 0..3 {
            local_min[axis] = local_min[axis].min(local[axis]);
            local_max[axis] = local_max[axis].max(local[axis]);
        }
        maximum_radius = maximum_radius.max(norm(local));
    }
    let expanded = |lo: f64, hi: f64| Interval::new(lo - margin, hi + margin);
    match surface {
        SurfaceGeometry::Plane { .. } => Ok([
            expanded(local_min[0], local_max[0])?,
            expanded(local_min[1], local_max[1])?,
        ]),
        SurfaceGeometry::Cylinder { .. } => Ok([
            Interval::new(0.0, std::f64::consts::TAU)?,
            expanded(local_min[2], local_max[2])?,
        ]),
        SurfaceGeometry::Cone { .. } => Ok([
            Interval::new(0.0, std::f64::consts::TAU)?,
            Interval::new(0.0, maximum_radius + margin)?,
        ]),
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. } => unreachable!(),
    }
}

fn universal_fallback(
    store: &mut GeometryStore,
    a: u32,
    b: u32,
    sa: &SurfaceGeometry,
    sb: &SurfaceGeometry,
    accuracy: Accuracy,
    original: GeometryError,
) -> Result<SsiResult, GeometryError> {
    let source_side = if matches!(
        sa,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        0
    } else if matches!(
        sb,
        SurfaceGeometry::Sphere { .. } | SurfaceGeometry::Torus { .. }
    ) {
        1
    } else {
        return Err(original);
    };
    let mut domains = [chart_domain(sa)?, chart_domain(sb)?];
    let source_bounds = store
        .surface([a, b][source_side])?
        .enclose(domains[source_side])?;
    domains[1 - source_side] = finite_target_domain(
        store.surface([a, b][1 - source_side])?,
        source_bounds,
        accuracy.intersection,
    )?;
    super::universal_ssi::intersect_patches_from(
        store,
        [a, b],
        domains,
        source_side,
        accuracy,
        super::intersection::SsiBudget::default(),
    )
}

pub fn intersect_surfaces(
    store: &mut GeometryStore,
    a: u32,
    b: u32,
    accuracy: Accuracy,
) -> Result<SsiResult, GeometryError> {
    accuracy.validate()?;
    let sa = store.surface(a)?.clone();
    let sb = store.surface(b)?.clone();
    sa.validate()?;
    sb.validate()?;
    let fast_intersection = (|| -> Result<GeometryResult, GeometryError> {
        Ok(match (&sa, &sb) {
            (SurfaceGeometry::Plane { frame: a }, SurfaceGeometry::Plane { frame: b }) => {
                plane_plane(*a, *b, accuracy)?
            }
            (SurfaceGeometry::Plane { frame: p }, SurfaceGeometry::Sphere { frame: s, radius })
            | (SurfaceGeometry::Sphere { frame: s, radius }, SurfaceGeometry::Plane { frame: p }) => {
                plane_sphere(*p, *s, *radius)?
            }
            (
                SurfaceGeometry::Plane { frame: p },
                SurfaceGeometry::Cylinder { frame: c, radius },
            )
            | (
                SurfaceGeometry::Cylinder { frame: c, radius },
                SurfaceGeometry::Plane { frame: p },
            ) => plane_cylinder(*p, *c, *radius)?,
            (
                SurfaceGeometry::Plane { frame: p },
                SurfaceGeometry::Cone {
                    frame: c,
                    semi_angle,
                },
            )
            | (
                SurfaceGeometry::Cone {
                    frame: c,
                    semi_angle,
                },
                SurfaceGeometry::Plane { frame: p },
            ) => plane_cone(*p, *c, *semi_angle)?,
            (
                SurfaceGeometry::Sphere {
                    frame: a,
                    radius: ra,
                },
                SurfaceGeometry::Sphere {
                    frame: b,
                    radius: rb,
                },
            ) => sphere_sphere(*a, *ra, *b, *rb)?,
            (
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius: cylinder_radius,
                },
            )
            | (
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius: cylinder_radius,
                },
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
            ) => coaxial_sphere_cylinder(*sphere, *sphere_radius, *cylinder, *cylinder_radius)?,
            (
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
            )
            | (
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
            ) => coaxial_sphere_cone(*sphere, *sphere_radius, *cone, *semi_angle)?,
            (
                SurfaceGeometry::Cylinder {
                    frame: a,
                    radius: ra,
                },
                SurfaceGeometry::Cylinder {
                    frame: b,
                    radius: rb,
                },
            ) => parallel_cylinders(*a, *ra, *b, *rb)?,
            (
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius,
                },
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
            )
            | (
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius,
                },
            ) => coaxial_cylinder_cone(*cylinder, *radius, *cone, *semi_angle)?,
            (
                SurfaceGeometry::Cone {
                    frame: a,
                    semi_angle: angle_a,
                },
                SurfaceGeometry::Cone {
                    frame: b,
                    semi_angle: angle_b,
                },
            ) => coaxial_cones(*a, *angle_a, *b, *angle_b)?,
            (
                SurfaceGeometry::Plane { frame: plane },
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
            )
            | (
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
                SurfaceGeometry::Plane { frame: plane },
            ) => plane_torus(*plane, *torus, *major_radius, *minor_radius)?,
            (
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
            )
            | (
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
                SurfaceGeometry::Sphere {
                    frame: sphere,
                    radius: sphere_radius,
                },
            ) => coaxial_sphere_torus(
                *sphere,
                *sphere_radius,
                *torus,
                *major_radius,
                *minor_radius,
            )?,
            (
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius: cylinder_radius,
                },
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
            )
            | (
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
                SurfaceGeometry::Cylinder {
                    frame: cylinder,
                    radius: cylinder_radius,
                },
            ) => coaxial_cylinder_torus(
                *cylinder,
                *cylinder_radius,
                *torus,
                *major_radius,
                *minor_radius,
            )?,
            (
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
            )
            | (
                SurfaceGeometry::Torus {
                    frame: torus,
                    major_radius,
                    minor_radius,
                },
                SurfaceGeometry::Cone {
                    frame: cone,
                    semi_angle,
                },
            ) => coaxial_cone_torus(*cone, *semi_angle, *torus, *major_radius, *minor_radius)?,
            (
                SurfaceGeometry::Torus {
                    frame: a,
                    major_radius: major_a,
                    minor_radius: minor_a,
                },
                SurfaceGeometry::Torus {
                    frame: b,
                    major_radius: major_b,
                    minor_radius: minor_b,
                },
            ) => coaxial_tori(*a, *major_a, *minor_a, *b, *major_b, *minor_b)?,
        })
    })();
    let intersection = match fast_intersection {
        Ok(intersection) => intersection,
        Err(error @ GeometryError::CoverageGap { .. }) => {
            return universal_fallback(store, a, b, &sa, &sb, accuracy, error)
        }
        Err(error) => return Err(error),
    };
    let (intersection, pending_contacts) = match intersection {
        GeometryResult::CurvesAndContacts { curves, contacts } => {
            (GeometryResult::Curves(curves), contacts)
        }
        intersection => (intersection, Vec::new()),
    };
    let mut result = SsiResult::default();
    match intersection {
        GeometryResult::Empty => {}
        GeometryResult::Coincident => result.coincident = true,
        GeometryResult::Contact(point) => {
            for surface in [&sa, &sb] {
                let uv = match surface.project(point, None) {
                    Ok(uv) => uv,
                    Err(GeometryError::SingularParameterization) if matches!(surface, SurfaceGeometry::Cone { frame, .. } if norm(sub(frame.origin, point)) <= accuracy.intersection) => {
                        [0.0, 0.0]
                    }
                    Err(error) => return Err(error),
                };
                if norm(sub(surface.point_at(uv)?, point)) > accuracy.intersection {
                    return Err(GeometryError::UnresolvedIntersection(
                        "contact support residual exceeds budget".into(),
                    ));
                }
            }
            result.contacts.push(point);
        }
        GeometryResult::Curves(curves) => {
            for (curve, domain) in curves {
                let curve_id = u32::try_from(store.curves.len())
                    .map_err(|_| GeometryError::LimitExceeded("SSI curves".into()))?;
                let start = domain.map_or(0.0, |d| d.lo);
                let point = curve.elementary_point(start)?;
                let mut pcurves = [0; 2];
                let mut pending = Vec::new();
                for (i, (id, surface)) in [(a, &sa), (b, &sb)].into_iter().enumerate() {
                    let uv = surface.project(point, None)?;
                    let pcurve = if matches!(surface, SurfaceGeometry::Plane { .. })
                        && matches!(curve, CurveGeometry::Line { .. })
                    {
                        let direction = curve.elementary_tangent(start)?;
                        let frame = surface.frame();
                        PcurveGeometry::Line2 {
                            origin: uv,
                            direction: [dot(direction, frame.x), dot(direction, frame.y)],
                        }
                    } else {
                        let next =
                            surface.project(curve.elementary_point(start + 0.01)?, Some(uv))?;
                        let uv_rate = std::array::from_fn(|j| (next[j] - uv[j]) / 0.01);
                        PcurveGeometry::ProjectedCurve {
                            curve: curve_id,
                            surface: id,
                            chart: 0,
                            uv_hint: uv,
                            uv_rate,
                            parameter_origin: start,
                        }
                    };
                    pcurves[i] = u32::try_from(store.pcurves.len() + i)
                        .map_err(|_| GeometryError::LimitExceeded("SSI pcurves".into()))?;
                    pending.push(pcurve);
                }
                for t in [
                    start,
                    start + domain.map_or(1.0, |d| d.width()) * 0.25,
                    start + domain.map_or(1.0, |d| d.width()) * 0.5,
                ] {
                    let p = curve.elementary_point(t)?;
                    for surface in [&sa, &sb] {
                        let uv = surface.project(p, None)?;
                        if norm(sub(surface.point_at(uv)?, p)) > accuracy.intersection {
                            return Err(GeometryError::UnresolvedIntersection(
                                "intersection curve misses support".into(),
                            ));
                        }
                    }
                }
                store.curves.push(curve);
                store.pcurves.extend(pending);
                result.curves.push(SsiCurve {
                    curve: curve_id,
                    pcurves,
                    domain,
                });
            }
        }
        GeometryResult::CurvesAndContacts { .. } => {
            return Err(GeometryError::InvalidGeometry(
                "combined SSI result was not normalized".into(),
            ))
        }
    }
    for point in pending_contacts {
        for surface in [&sa, &sb] {
            let uv = match surface.project(point, None) {
                Ok(uv) => uv,
                Err(GeometryError::SingularParameterization) if matches!(surface, SurfaceGeometry::Cone { frame, .. } if norm(sub(frame.origin, point)) <= accuracy.intersection) => {
                    [0.0, 0.0]
                }
                Err(error) => return Err(error),
            };
            if norm(sub(surface.point_at(uv)?, point)) > accuracy.intersection {
                return Err(GeometryError::UnresolvedIntersection(
                    "contact support residual exceeds budget".into(),
                ));
            }
        }
        result.contacts.push(point);
    }
    Ok(result)
}
