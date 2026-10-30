use crate::brep::{
    coverage_gap, BrepEnvelope, Edge, EdgeGeometry, Frame3, GeometryError, GeometryQuality,
    SurfaceGeometry,
};
use crate::math::{norm, sub};
use crate::primitives;

pub(crate) struct TorusInput<'a> {
    pub(crate) brep: &'a BrepEnvelope,
    pub(crate) frame: Frame3,
    pub(crate) major_radius: f64,
    pub(crate) minor_radius: f64,
}

pub(crate) fn full_torus(brep: &BrepEnvelope) -> Result<TorusInput<'_>, GeometryError> {
    brep.validate()?;
    let (frame, major_radius, minor_radius) = match brep.geometry.surfaces.as_slice() {
        [SurfaceGeometry::Torus {
            frame,
            major_radius,
            minor_radius,
        }] => (*frame, *major_radius, *minor_radius),
        _ => return Err(torus_gap()),
    };
    if !matches!(brep.quality, GeometryQuality::Analytic)
        || !brep.geometry.intersections.is_empty()
        || brep.topology.vertices.len() != 1
        || brep.topology.edges.len() != 2
        || brep.topology.halfedges.len() != 4
        || brep.topology.loops.len() != 1
        || brep.topology.faces.len() != 1
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.len() != 1
        || brep.solids.len() != 1
    {
        return Err(torus_gap());
    }
    let canonical = primitives::torus(
        brep.id.clone(),
        frame,
        major_radius,
        minor_radius,
        brep.accuracy,
    )?;
    let mut topology = brep.topology.clone();
    for (actual, expected) in topology
        .vertices
        .iter_mut()
        .zip(&canonical.topology.vertices)
    {
        if norm(sub(actual.position, expected.position)) > brep.accuracy.geometric {
            return Err(torus_gap());
        }
        actual.position = expected.position;
        actual.tolerance = expected.tolerance;
    }
    snap_torus_edges(
        &mut topology.edges,
        &canonical.topology.edges,
        brep.accuracy.geometric,
    )?;
    topology.faces[0].key = canonical.topology.faces[0].key.clone();
    topology.faces[0].provenance = canonical.topology.faces[0].provenance.clone();
    topology.faces[0].trim.uv_bounds = canonical.topology.faces[0].trim.uv_bounds;
    if brep.geometry != canonical.geometry
        || topology != canonical.topology
        || brep.solids != canonical.solids
    {
        return Err(torus_gap());
    }
    Ok(TorusInput {
        brep,
        frame,
        major_radius,
        minor_radius,
    })
}

fn torus_gap() -> GeometryError {
    coverage_gap("canonical full ring torus", "analytic solid")
}

fn snap_torus_edges(
    edges: &mut [Edge],
    expected_edges: &[Edge],
    geometric: f64,
) -> Result<(), GeometryError> {
    for (actual, expected) in edges.iter_mut().zip(expected_edges) {
        actual.tolerance = expected.tolerance;
        let (
            EdgeGeometry::Curve {
                curve: actual_curve,
                range: actual_range,
            },
            EdgeGeometry::Curve {
                curve: expected_curve,
                range: expected_range,
            },
        ) = (&mut actual.geometry, &expected.geometry)
        else {
            return Err(torus_gap());
        };
        if actual_curve != expected_curve
            || (actual_range.lo - expected_range.lo).abs() > geometric
            || (actual_range.hi - expected_range.hi).abs() > geometric
        {
            return Err(torus_gap());
        }
        *actual_range = *expected_range;
    }
    Ok(())
}
