use super::classification::PointClassification;
use super::face_trim::face_contains_uv;
use super::ray_roots::{ray_box_domain, support_roots};
use crate::brep::{unit, BrepEnvelope, Face, GeometryError, Surface};
use crate::math::{dot, norm, scale, sub, Point3};

fn classify_with_ray<'a>(
    brep: &BrepEnvelope,
    point: Point3,
    direction: Point3,
    faces: impl Iterator<Item = &'a Face>,
) -> Result<PointClassification, GeometryError> {
    let Some(domain) = ray_box_domain(brep, point, direction)? else {
        return Ok(PointClassification::Outside);
    };
    let mut hits: Vec<(f64, Point3)> = Vec::new();
    for face in faces {
        let surface = brep.geometry.surface(face.surface)?;
        let Some(roots) = support_roots(
            surface,
            point,
            direction,
            domain,
            brep.accuracy.intersection,
        )?
        else {
            return Ok(PointClassification::Unknown);
        };
        for parameter in roots {
            let position = [
                point[0] + parameter * direction[0],
                point[1] + parameter * direction[1],
                point[2] + parameter * direction[2],
            ];
            let uv = match surface.project(position, None) {
                Ok(uv) => uv,
                Err(GeometryError::SingularParameterization) => {
                    return Ok(PointClassification::Unknown)
                }
                Err(error) => return Err(error),
            };
            if norm(sub(surface.point_at(uv)?, position)) > brep.accuracy.intersection {
                continue;
            }
            match face_contains_uv(brep, face, uv)? {
                Some(true) => {}
                Some(false) => continue,
                None => return Ok(PointClassification::Unknown),
            }
            let normal = scale(surface.normal_at(uv)?, face.sense.multiplier());
            if dot(normal, direction).abs() <= 64.0 * f64::EPSILON {
                return Ok(PointClassification::Unknown);
            }
            if parameter <= brep.accuracy.geometric {
                return Ok(PointClassification::Boundary);
            }
            if hits
                .iter()
                .all(|(existing, _)| (existing - parameter).abs() > brep.accuracy.geometric)
            {
                hits.push((parameter, position));
            }
        }
    }
    Ok(if hits.len() % 2 == 1 {
        PointClassification::Inside
    } else {
        PointClassification::Outside
    })
}

pub fn classify_point(
    brep: &BrepEnvelope,
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    brep.validate()?;
    classify_point_validated(brep, point)
}

pub(crate) fn classify_point_validated(
    brep: &BrepEnvelope,
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    if point.into_iter().any(|value| !value.is_finite()) {
        return Err(GeometryError::InvalidGeometry(
            "point classification requires finite coordinates".into(),
        ));
    }
    let faces: Vec<u32> = brep.topology.faces.iter().map(|face| face.id).collect();
    classify_point_in_shell(brep, &faces, point)
}

pub(crate) fn classify_point_in_shell(
    brep: &BrepEnvelope,
    faces: &[u32],
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    let directions = [
        unit([1.0, 0.371, 0.127])?,
        unit([0.193, 1.0, 0.419])?,
        unit([0.311, 0.233, 1.0])?,
    ];
    for direction in directions {
        let result = classify_with_ray(
            brep,
            point,
            direction,
            faces
                .iter()
                .map(|face| &brep.topology.faces[*face as usize]),
        )?;
        if result != PointClassification::Unknown {
            return Ok(result);
        }
    }
    Ok(PointClassification::Unknown)
}
