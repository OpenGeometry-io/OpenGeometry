mod circles;
mod coaxial;
mod fallback;
mod planar;
#[cfg(test)]
mod tests;

use super::ssi_result::{GeometryResult, SsiCurve, SsiResult};
use crate::brep::{
    Accuracy, CurveGeometry, Frame3, GeometryError, GeometryStore, PcurveGeometry, Surface,
    SurfaceGeometry,
};
use crate::math::{dot, norm, sub, Interval, Point3};
use coaxial::{
    coaxial_cone_torus, coaxial_cones, coaxial_cylinder_cone, coaxial_cylinder_torus,
    coaxial_sphere_cone, coaxial_sphere_cylinder, coaxial_sphere_torus, coaxial_tori,
    parallel_cylinders,
};
use fallback::universal_fallback;
use planar::{plane_cone, plane_cylinder, plane_plane, plane_sphere, plane_torus, sphere_sphere};

pub(crate) fn intersect_surfaces(
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
    let fast_intersection = analytic_intersection(&sa, &sb, accuracy);
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
            check_contact(&sa, &sb, point, accuracy)?;
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
                    let pcurve = support_pcurve(surface, id, &curve, curve_id, start, point)?;
                    pcurves[i] = u32::try_from(store.pcurves.len() + i)
                        .map_err(|_| GeometryError::LimitExceeded("SSI pcurves".into()))?;
                    pending.push(pcurve);
                }
                check_curve_support(&curve, start, domain, &sa, &sb, accuracy)?;
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
        check_contact(&sa, &sb, point, accuracy)?;
        result.contacts.push(point);
    }
    Ok(result)
}

fn analytic_intersection(
    sa: &SurfaceGeometry,
    sb: &SurfaceGeometry,
    accuracy: Accuracy,
) -> Result<GeometryResult, GeometryError> {
    match sa {
        SurfaceGeometry::Plane { frame } => plane_pairs(*frame, sb, accuracy),
        SurfaceGeometry::Sphere { frame, radius } => sphere_pairs(*frame, *radius, sb),
        SurfaceGeometry::Cylinder { frame, radius } => cylinder_pairs(*frame, *radius, sb),
        SurfaceGeometry::Cone { frame, semi_angle } => cone_pairs(*frame, *semi_angle, sb),
        SurfaceGeometry::Torus {
            frame,
            major_radius,
            minor_radius,
        } => torus_pairs(*frame, *major_radius, *minor_radius, sb),
    }
}

fn plane_pairs(
    plane: Frame3,
    sb: &SurfaceGeometry,
    accuracy: Accuracy,
) -> Result<GeometryResult, GeometryError> {
    Ok(match sb {
        SurfaceGeometry::Plane { frame: other } => plane_plane(plane, *other, accuracy)?,
        SurfaceGeometry::Sphere {
            frame: sphere,
            radius: sphere_radius,
        } => plane_sphere(plane, *sphere, *sphere_radius)?,
        SurfaceGeometry::Cylinder {
            frame: cylinder,
            radius: cylinder_radius,
        } => plane_cylinder(plane, *cylinder, *cylinder_radius)?,
        SurfaceGeometry::Cone {
            frame: cone,
            semi_angle,
        } => plane_cone(plane, *cone, *semi_angle)?,
        SurfaceGeometry::Torus {
            frame: torus,
            major_radius,
            minor_radius,
        } => plane_torus(plane, *torus, *major_radius, *minor_radius)?,
    })
}

fn sphere_pairs(
    sphere: Frame3,
    sphere_radius: f64,
    sb: &SurfaceGeometry,
) -> Result<GeometryResult, GeometryError> {
    Ok(match sb {
        SurfaceGeometry::Plane { frame: plane } => plane_sphere(*plane, sphere, sphere_radius)?,
        SurfaceGeometry::Sphere {
            frame: other,
            radius: other_radius,
        } => sphere_sphere(sphere, sphere_radius, *other, *other_radius)?,
        SurfaceGeometry::Cylinder {
            frame: cylinder,
            radius: cylinder_radius,
        } => coaxial_sphere_cylinder(sphere, sphere_radius, *cylinder, *cylinder_radius)?,
        SurfaceGeometry::Cone {
            frame: cone,
            semi_angle,
        } => coaxial_sphere_cone(sphere, sphere_radius, *cone, *semi_angle)?,
        SurfaceGeometry::Torus {
            frame: torus,
            major_radius,
            minor_radius,
        } => coaxial_sphere_torus(sphere, sphere_radius, *torus, *major_radius, *minor_radius)?,
    })
}

fn cylinder_pairs(
    cylinder: Frame3,
    cylinder_radius: f64,
    sb: &SurfaceGeometry,
) -> Result<GeometryResult, GeometryError> {
    Ok(match sb {
        SurfaceGeometry::Plane { frame: plane } => {
            plane_cylinder(*plane, cylinder, cylinder_radius)?
        }
        SurfaceGeometry::Sphere {
            frame: sphere,
            radius: sphere_radius,
        } => coaxial_sphere_cylinder(*sphere, *sphere_radius, cylinder, cylinder_radius)?,
        SurfaceGeometry::Cylinder {
            frame: other,
            radius: other_radius,
        } => parallel_cylinders(cylinder, cylinder_radius, *other, *other_radius)?,
        SurfaceGeometry::Cone {
            frame: cone,
            semi_angle,
        } => coaxial_cylinder_cone(cylinder, cylinder_radius, *cone, *semi_angle)?,
        SurfaceGeometry::Torus {
            frame: torus,
            major_radius,
            minor_radius,
        } => coaxial_cylinder_torus(
            cylinder,
            cylinder_radius,
            *torus,
            *major_radius,
            *minor_radius,
        )?,
    })
}

fn cone_pairs(
    cone: Frame3,
    semi_angle: f64,
    sb: &SurfaceGeometry,
) -> Result<GeometryResult, GeometryError> {
    Ok(match sb {
        SurfaceGeometry::Plane { frame: plane } => plane_cone(*plane, cone, semi_angle)?,
        SurfaceGeometry::Sphere {
            frame: sphere,
            radius: sphere_radius,
        } => coaxial_sphere_cone(*sphere, *sphere_radius, cone, semi_angle)?,
        SurfaceGeometry::Cylinder {
            frame: cylinder,
            radius: cylinder_radius,
        } => coaxial_cylinder_cone(*cylinder, *cylinder_radius, cone, semi_angle)?,
        SurfaceGeometry::Cone {
            frame: other,
            semi_angle: other_angle,
        } => coaxial_cones(cone, semi_angle, *other, *other_angle)?,
        SurfaceGeometry::Torus {
            frame: torus,
            major_radius,
            minor_radius,
        } => coaxial_cone_torus(cone, semi_angle, *torus, *major_radius, *minor_radius)?,
    })
}

fn torus_pairs(
    torus: Frame3,
    major_radius: f64,
    minor_radius: f64,
    sb: &SurfaceGeometry,
) -> Result<GeometryResult, GeometryError> {
    Ok(match sb {
        SurfaceGeometry::Plane { frame: plane } => {
            plane_torus(*plane, torus, major_radius, minor_radius)?
        }
        SurfaceGeometry::Sphere {
            frame: sphere,
            radius: sphere_radius,
        } => coaxial_sphere_torus(*sphere, *sphere_radius, torus, major_radius, minor_radius)?,
        SurfaceGeometry::Cylinder {
            frame: cylinder,
            radius: cylinder_radius,
        } => coaxial_cylinder_torus(
            *cylinder,
            *cylinder_radius,
            torus,
            major_radius,
            minor_radius,
        )?,
        SurfaceGeometry::Cone {
            frame: cone,
            semi_angle,
        } => coaxial_cone_torus(*cone, *semi_angle, torus, major_radius, minor_radius)?,
        SurfaceGeometry::Torus {
            frame: other,
            major_radius: other_major,
            minor_radius: other_minor,
        } => coaxial_tori(
            torus,
            major_radius,
            minor_radius,
            *other,
            *other_major,
            *other_minor,
        )?,
    })
}

fn check_contact(
    sa: &SurfaceGeometry,
    sb: &SurfaceGeometry,
    point: Point3,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for surface in [sa, sb] {
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
    Ok(())
}

fn support_pcurve(
    surface: &SurfaceGeometry,
    id: u32,
    curve: &CurveGeometry,
    curve_id: u32,
    start: f64,
    point: Point3,
) -> Result<PcurveGeometry, GeometryError> {
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
        let next = surface.project(curve.elementary_point(start + 0.01)?, Some(uv))?;
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
    Ok(pcurve)
}

fn check_curve_support(
    curve: &CurveGeometry,
    start: f64,
    domain: Option<Interval>,
    sa: &SurfaceGeometry,
    sb: &SurfaceGeometry,
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    for t in [
        start,
        start + domain.map_or(1.0, |d| d.width()) * 0.25,
        start + domain.map_or(1.0, |d| d.width()) * 0.5,
    ] {
        let p = curve.elementary_point(t)?;
        for surface in [sa, sb] {
            let uv = surface.project(p, None)?;
            if norm(sub(surface.point_at(uv)?, p)) > accuracy.intersection {
                return Err(GeometryError::UnresolvedIntersection(
                    "intersection curve misses support".into(),
                ));
            }
        }
    }
    Ok(())
}
