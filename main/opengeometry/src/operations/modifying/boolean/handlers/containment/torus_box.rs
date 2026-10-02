use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::dot;
use crate::operations::modifying::boolean::operands::{full_box, full_torus};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn torus_box_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (torus, box_, torus_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Torus { .. })
    ) {
        (full_torus(a)?, full_box(b)?, true)
    } else {
        (full_torus(b)?, full_box(a)?, false)
    };
    let clearance = 4.0
        * torus
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let center = box_.frame.local(torus.frame.origin);
    let torus_margin = (0..3).fold(f64::INFINITY, |margin, axis| {
        let axial_rate = dot(torus.frame.z, box_axes[axis]);
        let radial_rate = (1.0 - axial_rate * axial_rate).max(0.0).sqrt();
        let extent = torus.major_radius * radial_rate + torus.minor_radius;
        margin
            .min(center[axis] - extent)
            .min(box_.size[axis] - center[axis] - extent)
    });
    if torus_margin > clearance {
        let b_inside_a = !torus_is_a;
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if torus_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "torus/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["ring torus".into(), "cuboid".into()],
    })
}
