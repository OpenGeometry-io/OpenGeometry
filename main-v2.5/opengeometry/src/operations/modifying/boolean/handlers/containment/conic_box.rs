use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::{dot, sub};
use crate::operations::modifying::boolean::operands::{
    conic_radius_at, full_box, full_conic_section,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn conic_box_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (conic, box_, conic_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cone { .. })
    ) {
        (full_conic_section(a)?, full_box(b)?, true)
    } else {
        (full_conic_section(b)?, full_box(a)?, false)
    };
    let clearance = 4.0
        * conic
            .brep
            .accuracy
            .geometric
            .max(box_.brep.accuracy.geometric);
    let box_axes = [box_.frame.x, box_.frame.y, box_.frame.z];
    let endpoint_centers = [
        conic.frame.point([0.0, 0.0, conic.axial_range.lo]),
        conic.frame.point([0.0, 0.0, conic.axial_range.hi]),
    ];
    let endpoint_radii = [conic.lower_radius, conic.upper_radius];
    let mut conic_margin = f64::INFINITY;
    for axis in 0..3 {
        let radial_extent_rate = (1.0 - dot(conic.frame.z, box_axes[axis]).powi(2))
            .max(0.0)
            .sqrt();
        let extents = [0, 1].map(|endpoint| {
            let center = dot(
                sub(endpoint_centers[endpoint], box_.frame.origin),
                box_axes[axis],
            );
            let radial = endpoint_radii[endpoint] * radial_extent_rate;
            [center - radial, center + radial]
        });
        let lo = extents[0][0].min(extents[1][0]);
        let hi = extents[0][1].max(extents[1][1]);
        conic_margin = conic_margin.min(lo).min(box_.size[axis] - hi);
    }

    let mut box_margin = f64::INFINITY;
    for corner in 0..8 {
        let point = box_.frame.point(std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                0.0
            } else {
                box_.size[axis]
            }
        }));
        let local = conic.frame.local(point);
        box_margin = box_margin
            .min(local[2] - conic.axial_range.lo)
            .min(conic.axial_range.hi - local[2])
            .min(conic_radius_at(&conic, local[2]) - local[0].hypot(local[1]));
    }
    let conic_inside_box = conic_margin > clearance;
    let box_inside_conic = box_margin > clearance;
    if conic_inside_box || box_inside_conic {
        let b_inside_a = if conic_is_a {
            box_inside_conic
        } else {
            conic_inside_box
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if conic_margin.abs() <= clearance || box_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cone/cuboid containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["cone or frustum".into(), "cuboid".into()],
    })
}
