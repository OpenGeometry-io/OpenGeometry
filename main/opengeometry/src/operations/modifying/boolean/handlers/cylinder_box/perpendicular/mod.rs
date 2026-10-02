mod tunnel;
mod tunnel_faces;

use crate::brep::{
    boundary, Accuracy, BrepEnvelope, Builder, FaceRole, Frame3, GeometryError, Orientation,
    PcurveGeometry, Use,
};
use crate::math::dot;
use crate::operations::modifying::boolean::assembly::{
    add_box_faces, append_cylinder_segment, box_source, cylinder_source, face_provenance,
    finish_analytic_result,
};
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use crate::primitives;
use tunnel::{add_tunnel_rims, perpendicular_tunnel, Tunnel, TunnelRims};
use tunnel_faces::{add_protruding_bands, add_tunnel_opening};

pub(super) fn perpendicular_cylinder_box_boolean(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let tunnel = perpendicular_tunnel(box_, cylinder, box_axes, accuracy)?;
    if operation == BooleanOp::Intersection {
        return perpendicular_intersection_boolean(
            box_, cylinder, &tunnel, operation, id, accuracy,
        );
    }
    if operation == BooleanOp::Subtraction && cylinder_is_a {
        return cylinder_minus_box_boolean(
            box_,
            cylinder,
            &tunnel,
            cylinder_is_a,
            operation,
            id,
            accuracy,
        );
    }
    let mut builder = Builder::new(id, accuracy)?;
    let corners = primitives::add_box_corners(&mut builder, box_.frame, box_.size, box_axes)?;
    let rims = add_tunnel_rims(&mut builder, cylinder, &tunnel, operation)?;
    add_box_faces(
        &mut builder,
        box_,
        &corners,
        accuracy,
        |_, face_index, face_frame| {
            box_face_holes(face_index, face_frame, cylinder, &tunnel, &rims)
        },
        |face_index| {
            (face_index == tunnel.low_face && tunnel.crosses_low)
                || (face_index == tunnel.high_face && tunnel.crosses_high)
        },
    )?;
    if operation == BooleanOp::Subtraction {
        add_tunnel_opening(&mut builder, cylinder, &tunnel, &rims)?;
    } else {
        add_protruding_bands(&mut builder, cylinder, &tunnel, &rims)?;
    }
    let out = builder.finish_solid()?;
    finish_analytic_result(
        out,
        box_.brep,
        cylinder.brep,
        !cylinder_is_a,
        operation,
        false,
    )
}

fn perpendicular_intersection_boolean(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut out = primitives::cylinder(id, tunnel.frame, cylinder.radius, tunnel.height, accuracy)?;
    out.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false);
    out.topology.faces[1].provenance = face_provenance(
        vec![if tunnel.crosses_low {
            box_source(box_, tunnel.low_face)
        } else {
            cylinder_source(cylinder, tunnel.cylinder_low_face)
        }],
        FaceRole::Split,
        false,
    );
    out.topology.faces[2].provenance = face_provenance(
        vec![if tunnel.crosses_high {
            box_source(box_, tunnel.high_face)
        } else {
            cylinder_source(cylinder, tunnel.cylinder_high_face)
        }],
        FaceRole::Split,
        false,
    );
    finish_analytic_result(out, box_.brep, cylinder.brep, true, operation, false)
}

fn cylinder_minus_box_boolean(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    cylinder_is_a: bool,
    operation: BooleanOp,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let mut out = BrepEnvelope::new(id, accuracy)?;
    if tunnel.crosses_low {
        let mut frame = tunnel.frame;
        frame.origin = box_.frame.point({
            let mut point = tunnel.local_origin;
            point[tunnel.axis] = tunnel.axial_span[0];
            point
        });
        append_cylinder_segment(
            &mut out,
            frame,
            cylinder.radius,
            -tunnel.axial_span[0],
            [
                face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                face_provenance(
                    vec![cylinder_source(cylinder, tunnel.cylinder_low_face)],
                    FaceRole::Preserved,
                    false,
                ),
                face_provenance(vec![box_source(box_, tunnel.low_face)], FaceRole::Cut, true),
            ],
            0,
        )?;
    }
    if tunnel.crosses_high {
        let mut frame = tunnel.frame;
        frame.origin = box_.frame.point({
            let mut point = tunnel.local_origin;
            point[tunnel.axis] = box_.size[tunnel.axis];
            point
        });
        append_cylinder_segment(
            &mut out,
            frame,
            cylinder.radius,
            tunnel.axial_span[1] - box_.size[tunnel.axis],
            [
                face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
                face_provenance(
                    vec![box_source(box_, tunnel.high_face)],
                    FaceRole::Cut,
                    true,
                ),
                face_provenance(
                    vec![cylinder_source(cylinder, tunnel.cylinder_high_face)],
                    FaceRole::Preserved,
                    false,
                ),
            ],
            usize::from(tunnel.crosses_low),
        )?;
    }
    finish_analytic_result(
        out,
        box_.brep,
        cylinder.brep,
        !cylinder_is_a,
        operation,
        false,
    )
}

fn box_face_holes(
    face_index: usize,
    face_frame: Frame3,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    rims: &TunnelRims,
) -> Result<Vec<Vec<Use>>, GeometryError> {
    let holes = if (face_index == tunnel.low_face && tunnel.crosses_low)
        || (face_index == tunnel.high_face && tunnel.crosses_high)
    {
        let is_low = face_index == tunnel.low_face;
        let circle_frame = if is_low {
            tunnel.frame
        } else {
            rims.high_frame
        };
        let circle_center = face_frame.local(circle_frame.origin);
        let (vertex, circle) = if is_low {
            rims.low_boundary.ok_or_else(|| {
                GeometryError::InvalidTopology("missing low cylinder boundary".into())
            })?
        } else {
            rims.high_boundary.ok_or_else(|| {
                GeometryError::InvalidTopology("missing high cylinder boundary".into())
            })?
        };
        vec![vec![boundary(
            circle,
            vertex,
            vertex,
            if is_low {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
            PcurveGeometry::Conic2 {
                origin: [circle_center[0], circle_center[1]],
                axis_a: [
                    cylinder.radius * dot(circle_frame.x, face_frame.x),
                    cylinder.radius * dot(circle_frame.x, face_frame.y),
                ],
                axis_b: [
                    cylinder.radius * dot(circle_frame.y, face_frame.x),
                    cylinder.radius * dot(circle_frame.y, face_frame.y),
                ],
            },
        )]]
    } else {
        Vec::new()
    };
    Ok(holes)
}
