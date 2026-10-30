use super::core::analytic_containment_boolean;
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::math::{cross, dot, norm, scale, sub};
use crate::operations::modifying::boolean::operands::{
    conic_radius_at, full_conic_section, full_cylinder, ConicSectionInput, CylinderInput,
};
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

pub(crate) fn cylinder_conic_containment(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    let (cylinder, conic, cylinder_is_a) = if matches!(
        a.geometry.surfaces.first(),
        Some(SurfaceGeometry::Cylinder { .. })
    ) {
        (full_cylinder(a)?, full_conic_section(b)?, true)
    } else {
        (full_cylinder(b)?, full_conic_section(a)?, false)
    };
    let clearance = 4.0
        * cylinder
            .brep
            .accuracy
            .geometric
            .max(conic.brep.accuracy.geometric);
    let alignment = dot(cylinder.frame.z, conic.frame.z);
    if norm(cross(cylinder.frame.z, conic.frame.z)) > 1.0e-12 {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "noncoaxial cone or frustum".into()],
        });
    }
    let displacement = sub(conic.frame.origin, cylinder.frame.origin);
    let cylinder_axis_offset = dot(displacement, cylinder.frame.z);
    let radial_offset = norm(sub(
        displacement,
        scale(cylinder.frame.z, cylinder_axis_offset),
    ));
    if radial_offset > clearance {
        return Err(GeometryError::CoverageGap {
            families: ["cylinder".into(), "noncoaxial cone or frustum".into()],
        });
    }

    let conic_margin = conic_margin_in_cylinder(&cylinder, &conic, alignment, cylinder_axis_offset);
    let cylinder_margin = cylinder_margin_in_conic(&cylinder, &conic, alignment);

    let conic_inside_cylinder = conic_margin > clearance;
    let cylinder_inside_conic = cylinder_margin > clearance;
    if conic_inside_cylinder || cylinder_inside_conic {
        let b_inside_a = if cylinder_is_a {
            conic_inside_cylinder
        } else {
            cylinder_inside_conic
        };
        return analytic_containment_boolean(a, b, b_inside_a, operation, id);
    }
    if conic_margin.abs() <= clearance || cylinder_margin.abs() <= clearance {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder/cone containment boundary is below geometric resolution".into(),
        ));
    }
    Err(GeometryError::CoverageGap {
        families: ["cylinder".into(), "intersecting cone or frustum".into()],
    })
}

fn conic_margin_in_cylinder(
    cylinder: &CylinderInput<'_>,
    conic: &ConicSectionInput<'_>,
    alignment: f64,
    cylinder_axis_offset: f64,
) -> f64 {
    let conic_ends_in_cylinder = [
        cylinder_axis_offset + alignment * conic.axial_range.lo,
        cylinder_axis_offset + alignment * conic.axial_range.hi,
    ];
    let conic_axial_lo = conic_ends_in_cylinder[0].min(conic_ends_in_cylinder[1]);
    let conic_axial_hi = conic_ends_in_cylinder[0].max(conic_ends_in_cylinder[1]);
    conic_axial_lo
        .min(cylinder.height - conic_axial_hi)
        .min(cylinder.radius - conic.lower_radius.max(conic.upper_radius))
}

fn cylinder_margin_in_conic(
    cylinder: &CylinderInput<'_>,
    conic: &ConicSectionInput<'_>,
    alignment: f64,
) -> f64 {
    let cylinder_origin_in_conic = dot(
        sub(cylinder.frame.origin, conic.frame.origin),
        conic.frame.z,
    );
    let cylinder_ends_in_conic = [
        cylinder_origin_in_conic,
        cylinder_origin_in_conic + alignment * cylinder.height,
    ];
    let cylinder_axial_lo = cylinder_ends_in_conic[0].min(cylinder_ends_in_conic[1]);
    let cylinder_axial_hi = cylinder_ends_in_conic[0].max(cylinder_ends_in_conic[1]);
    (cylinder_axial_lo - conic.axial_range.lo)
        .min(conic.axial_range.hi - cylinder_axial_hi)
        .min(conic_radius_at(conic, cylinder_ends_in_conic[0]) - cylinder.radius)
        .min(conic_radius_at(conic, cylinder_ends_in_conic[1]) - cylinder.radius)
}
