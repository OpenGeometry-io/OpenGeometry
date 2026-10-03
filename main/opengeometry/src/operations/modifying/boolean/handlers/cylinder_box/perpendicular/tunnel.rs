use crate::brep::{Accuracy, Builder, CurveGeometry, Frame3, GeometryError};
use crate::math::{dot, Interval, Point3};
use crate::operations::modifying::boolean::assembly::circular_boundary;
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};
use crate::operations::modifying::boolean::types::BooleanOp;

pub(super) struct Tunnel {
    pub(super) axis: usize,
    pub(super) alignment: f64,
    pub(super) local_origin: Point3,
    pub(super) axial_span: [f64; 2],
    pub(super) crosses_low: bool,
    pub(super) crosses_high: bool,
    pub(super) clipped_low: f64,
    pub(super) frame: Frame3,
    pub(super) height: f64,
    pub(super) low_face: usize,
    pub(super) high_face: usize,
    pub(super) cylinder_low_face: u32,
    pub(super) cylinder_high_face: u32,
}

pub(super) struct TunnelRims {
    pub(super) high_frame: Frame3,
    pub(super) low_boundary: Option<(u32, u32)>,
    pub(super) high_boundary: Option<(u32, u32)>,
    pub(super) seam: Option<u32>,
}

pub(super) fn perpendicular_tunnel(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    box_axes: [Point3; 3],
    accuracy: Accuracy,
) -> Result<Tunnel, GeometryError> {
    let (axis, alignment) = aligned_box_axis(cylinder, box_axes)?;
    let local_origin = box_.frame.local(cylinder.frame.origin);
    let other_axes = match axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    check_radial_margins(box_, cylinder, local_origin, other_axes, accuracy)?;
    let axial_end = local_origin[axis] + alignment * cylinder.height;
    let axial_span = [
        local_origin[axis].min(axial_end),
        local_origin[axis].max(axial_end),
    ];
    let low_clearance = -axial_span[0];
    let high_clearance = axial_span[1] - box_.size[axis];
    if low_clearance.abs() <= 4.0 * accuracy.geometric
        || high_clearance.abs() <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder cap meets a cuboid boundary within geometric resolution".into(),
        ));
    }
    let crosses_low = low_clearance > 4.0 * accuracy.geometric;
    let crosses_high = high_clearance > 4.0 * accuracy.geometric;
    let clipped_low = axial_span[0].max(0.0);
    let clipped_high = axial_span[1].min(box_.size[axis]);
    if clipped_high - clipped_low <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cuboid overlap is below geometric resolution".into(),
        ));
    }
    if !crosses_low && !crosses_high {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "contained cuboid intersection".into()],
        });
    }

    let mut center = local_origin;
    center[axis] = clipped_low;
    let low_center = box_.frame.point(center);
    let tunnel_frame = Frame3::from_axis(low_center, box_axes[axis], box_axes[other_axes[0]])?;
    let tunnel_height = clipped_high - clipped_low;
    let low_face = match axis {
        0 => 2,
        1 => 4,
        _ => 0,
    };
    let high_face = low_face + 1;
    let cylinder_low_face = if alignment > 0.0 { 1 } else { 2 };
    let cylinder_high_face = if alignment > 0.0 { 2 } else { 1 };
    Ok(Tunnel {
        axis,
        alignment,
        local_origin,
        axial_span,
        crosses_low,
        crosses_high,
        clipped_low,
        frame: tunnel_frame,
        height: tunnel_height,
        low_face,
        high_face,
        cylinder_low_face,
        cylinder_high_face,
    })
}

fn aligned_box_axis(
    cylinder: &CylinderInput<'_>,
    box_axes: [Point3; 3],
) -> Result<(usize, f64), GeometryError> {
    let (axis, alignment) = box_axes
        .iter()
        .enumerate()
        .map(|(axis, direction)| (axis, dot(cylinder.frame.z, *direction)))
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
        .ok_or_else(|| GeometryError::InvalidGeometry("cuboid has no frame axes".into()))?;
    if 1.0 - alignment.abs() > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "oblique cuboid intersection".into()],
        });
    }
    Ok((axis, alignment))
}

fn check_radial_margins(
    box_: &BoxInput<'_>,
    cylinder: &CylinderInput<'_>,
    local_origin: Point3,
    other_axes: [usize; 2],
    accuracy: Accuracy,
) -> Result<(), GeometryError> {
    let radial_margins = other_axes.map(|candidate| {
        (local_origin[candidate] - cylinder.radius)
            .min(box_.size[candidate] - local_origin[candidate] - cylinder.radius)
    });
    if radial_margins
        .iter()
        .any(|margin| margin.abs() <= 4.0 * accuracy.geometric)
    {
        return Err(GeometryError::UnresolvedIntersection(
            "cylindrical opening is tangent to a cuboid side within geometric resolution".into(),
        ));
    }
    if radial_margins.iter().any(|margin| *margin < 0.0) {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "side-intersecting cuboid".into()],
        });
    }
    Ok(())
}

pub(super) fn add_tunnel_rims(
    builder: &mut Builder,
    cylinder: &CylinderInput<'_>,
    tunnel: &Tunnel,
    operation: BooleanOp,
) -> Result<TunnelRims, GeometryError> {
    let high_frame = Frame3 {
        origin: tunnel.frame.point([0.0, 0.0, tunnel.height]),
        ..tunnel.frame
    };
    let low_boundary = if operation == BooleanOp::Subtraction || tunnel.crosses_low {
        Some(circular_boundary(
            builder,
            tunnel.frame,
            cylinder.radius,
            0.0,
        )?)
    } else {
        None
    };
    let high_boundary = if operation == BooleanOp::Subtraction || tunnel.crosses_high {
        Some(circular_boundary(
            builder,
            tunnel.frame,
            cylinder.radius,
            tunnel.height,
        )?)
    } else {
        None
    };
    let seam = if operation == BooleanOp::Subtraction {
        Some(builder.edge(
            CurveGeometry::Line {
                origin: tunnel.frame.point([cylinder.radius, 0.0, 0.0]),
                direction: tunnel.frame.z,
            },
            Interval::new(0.0, tunnel.height)?,
            true,
        ))
    } else {
        None
    };
    Ok(TunnelRims {
        high_frame,
        low_boundary,
        high_boundary,
        seam,
    })
}
