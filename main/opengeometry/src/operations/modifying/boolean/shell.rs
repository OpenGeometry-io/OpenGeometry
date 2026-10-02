use super::handlers::analytic_containment_boolean;
use super::operands::{
    brep_face_source, full_box, full_conic_section, full_cylinder, full_planar_extrusion,
    full_sphere, full_torus, prismatic_gap, unique_sources, BoxInput,
};
use super::types::{BooleanOp, BooleanResult, FaceMapping};
use crate::brep::{BrepEnvelope, Frame3, GeometryError, SurfaceGeometry};
use crate::geom2d::{self_intersects2, signed_area2, Pt2};
use crate::math::{scale, Point3};
use crate::primitives;

pub fn shell_brep(
    input: &BrepEnvelope,
    thickness: f64,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    input.validate()?;
    if !thickness.is_finite() || thickness <= 4.0 * input.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness is below geometric resolution".into(),
        ));
    }
    let inner_id = format!("{id}:offset-cavity");
    let inner = offset_cavity(input, thickness, inner_id)?;
    let inner_identity = inner.id.clone();
    let mut result = analytic_containment_boolean(input, &inner, true, BooleanOp::Subtraction, id)?;
    set_cavity_provenance(&mut result.brep, input, &inner_identity)?;
    result.report.face_mappings = input
        .topology
        .faces
        .iter()
        .map(|face| {
            let source = brep_face_source(input, face.id);
            let result_faces = result
                .brep
                .topology
                .faces
                .iter()
                .filter(|candidate| {
                    candidate.provenance.sources.iter().any(|candidate| {
                        candidate.entity == source.entity
                            && candidate.body == source.body
                            && candidate.key == source.key
                            && candidate.face == source.face
                    })
                })
                .map(|candidate| candidate.id)
                .collect();
            FaceMapping {
                source,
                result_faces,
            }
        })
        .collect();
    result.brep.validate()?;
    Ok(result)
}

fn offset_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    match input.geometry.surfaces.first() {
        Some(SurfaceGeometry::Sphere { .. }) => sphere_cavity(input, thickness, inner_id),
        Some(SurfaceGeometry::Cylinder { .. }) => cylinder_cavity(input, thickness, inner_id),
        Some(SurfaceGeometry::Torus { .. }) => torus_cavity(input, thickness, inner_id),
        Some(SurfaceGeometry::Plane { .. }) => match full_box(input) {
            Ok(input) => cuboid_cavity(&input, thickness, inner_id),
            Err(GeometryError::CoverageGap { .. }) => extrusion_cavity(input, thickness, inner_id),
            Err(error) => Err(error),
        },
        Some(SurfaceGeometry::Cone { .. }) => cone_cavity(input, thickness, inner_id),
        None => Err(GeometryError::CoverageGap {
            families: ["restricted analytic shell".into(), "input solid".into()],
        }),
    }
}

fn sphere_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let input = full_sphere(input)?;
    let radius = input.radius - thickness;
    if radius <= 4.0 * input.brep.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness consumes the sphere interior".into(),
        ));
    }
    primitives::sphere(inner_id, input.frame, radius, input.brep.accuracy)
}

fn cylinder_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let input = full_cylinder(input)?;
    let radius = input.radius - thickness;
    let height = input.height - 2.0 * thickness;
    if radius <= 4.0 * input.brep.accuracy.geometric
        || height <= 4.0 * input.brep.accuracy.geometric
    {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness consumes the cylinder interior".into(),
        ));
    }
    let mut frame = input.frame;
    frame.origin = input.frame.point([0.0, 0.0, thickness]);
    primitives::cylinder(inner_id, frame, radius, height, input.brep.accuracy)
}

fn torus_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let input = full_torus(input)?;
    let minor_radius = input.minor_radius - thickness;
    if minor_radius <= 4.0 * input.brep.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness consumes the torus interior".into(),
        ));
    }
    primitives::torus(
        inner_id,
        input.frame,
        input.major_radius,
        minor_radius,
        input.brep.accuracy,
    )
}

fn cuboid_cavity(
    input: &BoxInput<'_>,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let size = input.size.map(|dimension| dimension - 2.0 * thickness);
    if size
        .iter()
        .any(|dimension| *dimension <= 4.0 * input.brep.accuracy.geometric)
    {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness consumes the cuboid interior".into(),
        ));
    }
    let mut frame = input.frame;
    frame.origin = input.frame.point([thickness, thickness, thickness]);
    primitives::cuboid(inner_id, frame, size, input.brep.accuracy)
}

fn extrusion_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let input = full_planar_extrusion(input)?;
    let height = input.height - 2.0 * thickness;
    if height <= 4.0 * input.brep.accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "shell thickness consumes the profile extrusion height".into(),
        ));
    }
    let inset = input
        .contours
        .iter()
        .map(|ring| {
            inset_prismatic_ring(ring, input.frame, thickness, input.brep.accuracy.geometric)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut frame = input.frame;
    frame.origin = input.frame.point([0.0, 0.0, thickness]);
    primitives::linear_extrusion(
        inner_id,
        frame,
        inset[0].clone(),
        inset[1..].to_vec(),
        height,
        input.brep.accuracy,
    )
    .map_err(|error| match error {
        GeometryError::InvalidGeometry(_) | GeometryError::UnresolvedIntersection(_) => {
            GeometryError::UnresolvedIntersection(
                "shell offset self-intersects or consumes the profile interior".into(),
            )
        }
        other => other,
    })
}

fn inset_prismatic_ring(
    ring: &[Point3],
    frame: Frame3,
    thickness: f64,
    tolerance: f64,
) -> Result<Vec<[f64; 2]>, GeometryError> {
    let points = ring
        .iter()
        .map(|point| {
            let local = frame.local(*point);
            Pt2::new(local[0], local[1])
        })
        .collect::<Vec<_>>();
    if points.len() < 3 {
        return Err(prismatic_gap());
    }
    let direction = |from: Pt2, to: Pt2| -> Result<Pt2, GeometryError> {
        let delta = Pt2::new(to.x - from.x, to.z - from.z);
        let length = delta.x.hypot(delta.z);
        if length <= 4.0 * tolerance {
            return Err(GeometryError::UnresolvedIntersection(
                "profile shell contains a sub-tolerance edge".into(),
            ));
        }
        Ok(Pt2::new(delta.x / length, delta.z / length))
    };
    let mut result = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let point = points[index];
        let incoming = direction(points[(index + points.len() - 1) % points.len()], point)?;
        let outgoing = direction(point, points[(index + 1) % points.len()])?;
        let offset = miter_offset(point, incoming, outgoing, thickness)?;
        if !offset.x.is_finite()
            || !offset.z.is_finite()
            || (offset.x - point.x).hypot(offset.z - point.z) > thickness * 1.0e6
        {
            return Err(GeometryError::UnresolvedIntersection(
                "profile shell has an unbounded miter".into(),
            ));
        }
        result.push(offset);
    }
    let original_area = signed_area2(&points);
    let result_area = signed_area2(&result);
    if result_area.abs() <= tolerance * tolerance
        || original_area.signum() != result_area.signum()
        || self_intersects2(&result, tolerance)
    {
        return Err(GeometryError::UnresolvedIntersection(
            "profile shell offset collapses or self-intersects".into(),
        ));
    }
    Ok(result.into_iter().map(|point| [point.x, point.z]).collect())
}

fn miter_offset(
    point: Pt2,
    incoming: Pt2,
    outgoing: Pt2,
    thickness: f64,
) -> Result<Pt2, GeometryError> {
    let cross2 = |a: Pt2, b: Pt2| a.x * b.z - a.z * b.x;
    let incoming_normal = Pt2::new(-incoming.z, incoming.x);
    let outgoing_normal = Pt2::new(-outgoing.z, outgoing.x);
    let incoming_origin = Pt2::new(
        point.x + thickness * incoming_normal.x,
        point.z + thickness * incoming_normal.z,
    );
    let outgoing_origin = Pt2::new(
        point.x + thickness * outgoing_normal.x,
        point.z + thickness * outgoing_normal.z,
    );
    let determinant = cross2(incoming, outgoing);
    let offset = if determinant.abs() <= 1.0e-12 {
        if incoming.x * outgoing.x + incoming.z * outgoing.z < 0.0 {
            return Err(GeometryError::UnresolvedIntersection(
                "profile shell has a reversing corner".into(),
            ));
        }
        incoming_origin
    } else {
        let delta = Pt2::new(
            outgoing_origin.x - incoming_origin.x,
            outgoing_origin.z - incoming_origin.z,
        );
        let parameter = cross2(delta, outgoing) / determinant;
        Pt2::new(
            incoming_origin.x + parameter * incoming.x,
            incoming_origin.z + parameter * incoming.z,
        )
    };
    Ok(offset)
}

fn cone_cavity(
    input: &BrepEnvelope,
    thickness: f64,
    inner_id: String,
) -> Result<BrepEnvelope, GeometryError> {
    let input = full_conic_section(input)?;
    let slope = input.semi_angle.tan();
    let normal_scale = (1.0 + slope * slope).sqrt();
    if input.lower_radius <= input.brep.accuracy.geometric {
        let apex = thickness * normal_scale / slope;
        let base = input.axial_range.hi - thickness;
        let height = base - apex;
        let radius = height * slope;
        if radius <= 4.0 * input.brep.accuracy.geometric
            || height <= 4.0 * input.brep.accuracy.geometric
        {
            return Err(GeometryError::UnresolvedIntersection(
                "shell thickness consumes the cone interior".into(),
            ));
        }
        let base_frame = Frame3 {
            origin: input.frame.point([0.0, 0.0, base]),
            x: input.frame.x,
            y: scale(input.frame.y, -1.0),
            z: scale(input.frame.z, -1.0),
        };
        primitives::cone(inner_id, base_frame, radius, height, input.brep.accuracy)
    } else {
        let height = input.axial_range.width() - 2.0 * thickness;
        let lower_radius = input.lower_radius + thickness * slope - thickness * normal_scale;
        let upper_radius = input.upper_radius - thickness * slope - thickness * normal_scale;
        if height <= 4.0 * input.brep.accuracy.geometric
            || lower_radius <= 4.0 * input.brep.accuracy.geometric
            || upper_radius <= 4.0 * input.brep.accuracy.geometric
        {
            return Err(GeometryError::UnresolvedIntersection(
                "shell thickness consumes the frustum interior".into(),
            ));
        }
        let frame = Frame3 {
            origin: input
                .frame
                .point([0.0, 0.0, input.axial_range.lo + thickness]),
            ..input.frame
        };
        primitives::frustum(
            inner_id,
            frame,
            lower_radius,
            upper_radius,
            height,
            input.brep.accuracy,
        )
    }
}

fn set_cavity_provenance(
    brep: &mut BrepEnvelope,
    input: &BrepEnvelope,
    inner_identity: &str,
) -> Result<(), GeometryError> {
    for face in &mut brep.topology.faces {
        for source in &mut face.provenance.sources {
            if source.entity == inner_identity && source.body == inner_identity {
                let original = input
                    .topology
                    .faces
                    .get(source.face as usize)
                    .ok_or_else(|| {
                        GeometryError::InvalidTopology(
                            "shell offset face has no source face".into(),
                        )
                    })?;
                *source = brep_face_source(input, original.id);
            }
        }
        face.provenance.sources = unique_sources(std::mem::take(&mut face.provenance.sources));
    }
    Ok(())
}
