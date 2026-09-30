use super::tunnel::{Tunnel, TunnelRims};
use crate::brep::{
    boundary, uv_line, Builder, FaceRole, Frame3, GeometryError, Orientation, PcurveGeometry,
    SurfaceGeometry,
};
use crate::math::scale;
use crate::operations::modifying::boolean::assembly::{
    circular_boundary, cylinder_band, cylinder_cap, cylinder_source, face_provenance, reverse_face,
    CylinderBand,
};
use crate::operations::modifying::boolean::operands::CylinderInput;

pub(super) fn add_tunnel_opening(
    builder: &mut Builder,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    rims: &TunnelRims,
) -> Result<(), GeometryError> {
    let (low_vertex, low_circle) = rims
        .low_boundary
        .ok_or_else(|| GeometryError::InvalidTopology("missing low cylinder boundary".into()))?;
    let (high_vertex, high_circle) = rims
        .high_boundary
        .ok_or_else(|| GeometryError::InvalidTopology("missing high cylinder boundary".into()))?;
    let seam = rims
        .seam
        .ok_or_else(|| GeometryError::InvalidTopology("missing cylinder seam boundary".into()))?;
    add_tunnel_face(
        builder,
        cylinder,
        tunnel,
        (low_vertex, low_circle),
        (high_vertex, high_circle),
        seam,
    )?;
    if tunnel.crosses_low != tunnel.crosses_high {
        add_pocket_cap(
            builder,
            cylinder,
            tunnel,
            rims.high_frame,
            (low_vertex, low_circle),
            (high_vertex, high_circle),
        )?;
    }
    Ok(())
}

fn add_tunnel_face(
    builder: &mut Builder,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    low: (u32, u32),
    high: (u32, u32),
    seam: u32,
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    let (low_vertex, low_circle) = low;
    let (high_vertex, high_circle) = high;
    let tunnel_face = builder.brep.topology.faces.len() as u32;
    builder.face(
        "cutter:tunnel",
        SurfaceGeometry::Cylinder {
            frame: tunnel.frame,
            radius: cylinder.radius,
        },
        [[0.0, tau], [0.0, tunnel.height]],
        vec![
            boundary(
                low_circle,
                low_vertex,
                low_vertex,
                Orientation::Forward,
                uv_line([0.0, 0.0], [1.0, 0.0]),
            ),
            boundary(
                seam,
                low_vertex,
                high_vertex,
                Orientation::Forward,
                uv_line([tau, 0.0], [0.0, 1.0]),
            ),
            boundary(
                high_circle,
                high_vertex,
                high_vertex,
                Orientation::Reverse,
                uv_line([0.0, tunnel.height], [1.0, 0.0]),
            ),
            boundary(
                seam,
                high_vertex,
                low_vertex,
                Orientation::Reverse,
                uv_line([0.0, 0.0], [0.0, 1.0]),
            ),
        ],
    )?;
    builder.brep.topology.faces[tunnel_face as usize].provenance =
        face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Cut, true);
    reverse_face(&mut builder.brep, tunnel_face);
    Ok(())
}

fn add_pocket_cap(
    builder: &mut Builder,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    high_frame: Frame3,
    low: (u32, u32),
    high: (u32, u32),
) -> Result<(), GeometryError> {
    let (low_vertex, low_circle) = low;
    let (high_vertex, high_circle) = high;
    let (key, circle, vertex, mut cap_frame, sense, axis_b, cylinder_cap) = if tunnel.crosses_low {
        (
            "cutter:pocket_cap",
            high_circle,
            high_vertex,
            high_frame,
            Orientation::Forward,
            [0.0, cylinder.radius],
            if tunnel.alignment > 0.0 { 2 } else { 1 },
        )
    } else {
        let mut cap_frame = tunnel.frame;
        cap_frame.y = scale(cap_frame.y, -1.0);
        cap_frame.z = scale(cap_frame.z, -1.0);
        (
            "cutter:pocket_cap",
            low_circle,
            low_vertex,
            cap_frame,
            Orientation::Reverse,
            [0.0, -cylinder.radius],
            if tunnel.alignment > 0.0 { 1 } else { 2 },
        )
    };
    let cap_face = builder.brep.topology.faces.len() as u32;
    cap_frame.origin = if tunnel.crosses_low {
        high_frame.origin
    } else {
        tunnel.frame.origin
    };
    builder.face(
        key,
        SurfaceGeometry::Plane { frame: cap_frame },
        [[-cylinder.radius, cylinder.radius]; 2],
        vec![boundary(
            circle,
            vertex,
            vertex,
            sense,
            PcurveGeometry::Conic2 {
                origin: [0.0; 2],
                axis_a: [cylinder.radius, 0.0],
                axis_b,
            },
        )],
    )?;
    builder.brep.topology.faces[cap_face as usize].provenance = face_provenance(
        vec![cylinder_source(cylinder, cylinder_cap)],
        FaceRole::Cut,
        true,
    );
    reverse_face(&mut builder.brep, cap_face);
    Ok(())
}

pub(super) fn add_protruding_bands(
    builder: &mut Builder,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    rims: &TunnelRims,
) -> Result<(), GeometryError> {
    if tunnel.crosses_low {
        let entry = rims.low_boundary.ok_or_else(|| {
            GeometryError::InvalidTopology("missing low cylinder boundary".into())
        })?;
        let outer_z = tunnel.axial_span[0] - tunnel.clipped_low;
        let outer = circular_boundary(builder, tunnel.frame, cylinder.radius, outer_z)?;
        cylinder_band(
            builder,
            &CylinderBand {
                frame: tunnel.frame,
                radius: cylinder.radius,
                z0: outer_z,
                z1: 0.0,
                low: outer,
                high: entry,
            },
            face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
            false,
        )?;
        cylinder_cap(
            builder,
            tunnel.frame,
            cylinder.radius,
            outer_z,
            outer,
            false,
            face_provenance(
                vec![cylinder_source(cylinder, tunnel.cylinder_low_face)],
                FaceRole::Preserved,
                false,
            ),
        )?;
    }
    if tunnel.crosses_high {
        let entry = rims.high_boundary.ok_or_else(|| {
            GeometryError::InvalidTopology("missing high cylinder boundary".into())
        })?;
        let outer_z = tunnel.axial_span[1] - tunnel.clipped_low;
        let outer = circular_boundary(builder, tunnel.frame, cylinder.radius, outer_z)?;
        cylinder_band(
            builder,
            &CylinderBand {
                frame: tunnel.frame,
                radius: cylinder.radius,
                z0: tunnel.height,
                z1: outer_z,
                low: entry,
                high: outer,
            },
            face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
            false,
        )?;
        cylinder_cap(
            builder,
            tunnel.frame,
            cylinder.radius,
            outer_z,
            outer,
            true,
            face_provenance(
                vec![cylinder_source(cylinder, tunnel.cylinder_high_face)],
                FaceRole::Preserved,
                false,
            ),
        )?;
    }
    Ok(())
}
