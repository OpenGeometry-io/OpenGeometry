use super::classification::PointClassification;
use super::face_trim::face_contains_uv;
use super::ray_roots::{ray_box_domain, support_roots};
use crate::brep::{
    unit, BrepEnvelope, Face, GeometryError, SolidRegion, Surface, SurfaceGeometry, UV,
};
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
    let mut answer = PointClassification::Outside;
    for solid in &brep.solids {
        match classify_point_in_solid(brep, solid, point)? {
            PointClassification::Inside => return Ok(PointClassification::Inside),
            PointClassification::Boundary => answer = PointClassification::Boundary,
            PointClassification::Unknown if answer == PointClassification::Outside => {
                answer = PointClassification::Unknown
            }
            _ => {}
        }
    }
    Ok(answer)
}

fn classify_point_in_solid(
    brep: &BrepEnvelope,
    solid: &SolidRegion,
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    let shell_faces = |shell: u32| &brep.topology.shells[shell as usize].faces;
    let outer = classify_point_in_shell(brep, shell_faces(solid.outer_shell), point)?;
    if outer != PointClassification::Inside {
        return Ok(outer);
    }
    for cavity in &solid.cavity_shells {
        match classify_point_in_shell(brep, shell_faces(*cavity), point)? {
            PointClassification::Outside => {}
            PointClassification::Inside => return Ok(PointClassification::Outside),
            other => return Ok(other),
        }
    }
    Ok(PointClassification::Inside)
}

pub(crate) fn classify_point_in_shell(
    brep: &BrepEnvelope,
    faces: &[u32],
    point: Point3,
) -> Result<PointClassification, GeometryError> {
    for face in faces {
        if on_face(brep, &brep.topology.faces[*face as usize], point)? {
            return Ok(PointClassification::Boundary);
        }
    }
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

fn on_face(brep: &BrepEnvelope, face: &Face, point: Point3) -> Result<bool, GeometryError> {
    let surface = brep.geometry.surface(face.surface)?;
    let uv = match surface.project(point, None) {
        Ok(uv) => uv,
        Err(GeometryError::SingularParameterization) => {
            return Ok(near_face_vertex(brep, face, point))
        }
        Err(error) => return Err(error),
    };
    if norm(sub(surface.point_at(uv)?, point)) > brep.accuracy.geometric {
        return Ok(false);
    }
    let uv = within_face_bounds(surface, face, uv, brep.accuracy.geometric)?;
    Ok(face_contains_uv(brep, face, uv)? != Some(false))
}

fn within_face_bounds(
    surface: &SurfaceGeometry,
    face: &Face,
    uv: UV,
    geometric: f64,
) -> Result<UV, GeometryError> {
    let jet = surface.derivatives(uv)?;
    let lengths = [norm(jet.du), norm(jet.dv)];
    let periods = surface.charts()[face.trim.chart as usize].periods;
    Ok(std::array::from_fn(|axis| {
        let bounds = face.trim.uv_bounds[axis];
        let value = periods[axis].map_or(uv[axis], |period| {
            uv[axis] + ((bounds.midpoint() - uv[axis]) / period).round() * period
        });
        let slack = geometric / lengths[axis];
        if value < bounds.lo && bounds.lo - value <= slack {
            bounds.lo
        } else if value > bounds.hi && value - bounds.hi <= slack {
            bounds.hi
        } else {
            value
        }
    }))
}

fn near_face_vertex(brep: &BrepEnvelope, face: &Face, point: Point3) -> bool {
    brep.topology
        .halfedges
        .iter()
        .filter(|halfedge| halfedge.face == Some(face.id))
        .any(|halfedge| {
            norm(sub(
                brep.topology.vertices[halfedge.from as usize].position,
                point,
            )) <= brep.accuracy.geometric
        })
}
