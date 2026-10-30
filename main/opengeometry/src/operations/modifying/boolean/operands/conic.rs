use super::compare::{cylinder_geometry_matches, normalize_cylinder_topology};
use crate::brep::{
    coverage_gap, BrepEnvelope, Frame3, GeometryError, GeometryQuality, SurfaceGeometry,
};
use crate::math::{norm, scale, sub, Interval};
use crate::primitives;

pub(crate) struct ConicSectionInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) semi_angle: f64,
    pub(crate) axial_range: Interval,
    pub(crate) lower_radius: f64,
    pub(crate) upper_radius: f64,
}

pub(crate) fn full_conic_section(
    brep: &BrepEnvelope,
) -> Result<ConicSectionInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, semi_angle) = match brep.geometry.surfaces.first() {
        Some(SurfaceGeometry::Cone { frame, semi_angle }) => (*frame, *semi_angle),
        _ => return Err(conic_section_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
        || !brep.solids[0].cavity_shells.is_empty()
        || !matches!(brep.geometry.surfaces.len(), 2 | 3)
        || brep.geometry.surfaces[1..]
            .iter()
            .any(|surface| !matches!(surface, SurfaceGeometry::Plane { .. }))
        || brep.topology.faces.len() != brep.geometry.surfaces.len()
    {
        return Err(conic_section_gap());
    }
    let axial_range = brep.topology.faces[0].trim.uv_bounds[1];
    if axial_range.lo < 0.0
        || axial_range.hi <= axial_range.lo
        || semi_angle <= 0.0
        || semi_angle >= std::f64::consts::FRAC_PI_2
    {
        return Err(conic_section_gap());
    }
    let slope = semi_angle.tan();
    let lower_radius = axial_range.lo * slope;
    let upper_radius = axial_range.hi * slope;
    let canonical = canonical_conic_section(brep, frame, axial_range, lower_radius, upper_radius)?;
    let mut topology = brep.topology.clone();
    for (face, expected) in topology.faces.iter_mut().zip(&canonical.topology.faces) {
        face.key = expected.key.clone();
        face.provenance = expected.provenance.clone();
    }
    for vertex in &mut topology.vertices {
        vertex.tolerance = brep.accuracy.geometric;
        let expected = canonical
            .topology
            .vertices
            .get(vertex.id as usize)
            .ok_or_else(conic_section_gap)?
            .position;
        if norm(sub(vertex.position, expected)) > brep.accuracy.geometric {
            return Err(conic_section_gap());
        }
        vertex.position = expected;
    }
    for edge in &mut topology.edges {
        edge.tolerance = brep.accuracy.geometric;
    }
    if !normalize_cylinder_topology(&mut topology, &canonical.topology, brep.accuracy.geometric)
        || !cylinder_geometry_matches(&brep.geometry, &canonical.geometry, brep.accuracy.geometric)
        || topology != canonical.topology
        || brep.solids != canonical.solids
    {
        return Err(conic_section_gap());
    }
    Ok(ConicSectionInput {
        brep,
        frame,
        semi_angle,
        axial_range,
        lower_radius,
        upper_radius,
    })
}

fn conic_section_gap() -> GeometryError {
    coverage_gap("canonical cone or frustum", "analytic solid")
}

fn canonical_conic_section(
    brep: &BrepEnvelope,
    frame: Frame3,
    axial_range: Interval,
    lower_radius: f64,
    upper_radius: f64,
) -> Result<BrepEnvelope, GeometryError> {
    if axial_range.lo <= brep.accuracy.geometric {
        if axial_range.lo != 0.0 || brep.geometry.surfaces.len() != 2 {
            return Err(conic_section_gap());
        }
        let base_frame = Frame3 {
            origin: frame.point([0.0, 0.0, axial_range.hi]),
            x: frame.x,
            y: scale(frame.y, -1.0),
            z: scale(frame.z, -1.0),
        };
        primitives::cone(
            brep.id.clone(),
            base_frame,
            upper_radius,
            axial_range.hi,
            brep.accuracy,
        )
    } else {
        if brep.geometry.surfaces.len() != 3 {
            return Err(conic_section_gap());
        }
        let base_frame = Frame3 {
            origin: frame.point([0.0, 0.0, axial_range.lo]),
            ..frame
        };
        primitives::frustum(
            brep.id.clone(),
            base_frame,
            lower_radius,
            upper_radius,
            axial_range.width(),
            brep.accuracy,
        )
    }
}

pub(crate) fn conic_radius_at(input: &ConicSectionInput<'_>, axial: f64) -> f64 {
    axial * input.semi_angle.tan()
}
