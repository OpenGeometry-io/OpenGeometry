use super::handler_id::HandlerId;
use super::trace::{record_handler, record_optional};
use crate::brep::{BrepEnvelope, GeometryError, SurfaceGeometry};
use crate::operations::modifying::boolean::handlers::{
    boolean_boxes, boolean_cylinders, boolean_planar_extrusions, boolean_rectilinear,
    boolean_spheres, conic_box_containment, conic_containment_boolean, cylinder_box_boolean,
    cylinder_conic_containment, generic_multiple_closed_loops_boolean,
    generic_non_intersecting_boolean, generic_single_closed_loop_boolean,
    generic_two_sided_cutter_band_subtraction, sphere_box_boolean, sphere_conic_containment,
    sphere_cylinder_boolean, sphere_torus_containment, subtract_layered_extrusions,
    subtract_planar_polyhedra, subtract_vertical_arc_extrusion, torus_box_containment,
    torus_conic_containment, torus_containment_boolean, torus_cylinder_containment,
};
use crate::operations::modifying::boolean::operands::all_planar;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperandFamily {
    Sphere,
    Cylinder,
    Torus,
    Cone,
    Planar,
    Other,
}

fn operand_family(body: &BrepEnvelope) -> OperandFamily {
    match body.geometry.surfaces.first() {
        Some(SurfaceGeometry::Sphere { .. }) => OperandFamily::Sphere,
        Some(SurfaceGeometry::Cylinder { .. }) => OperandFamily::Cylinder,
        Some(SurfaceGeometry::Torus { .. }) => OperandFamily::Torus,
        Some(SurfaceGeometry::Cone { .. }) => OperandFamily::Cone,
        Some(_) if all_planar(body) => OperandFamily::Planar,
        _ => OperandFamily::Other,
    }
}

type BooleanHandler =
    fn(&BrepEnvelope, &BrepEnvelope, BooleanOp, String) -> Result<BooleanResult, GeometryError>;

struct FamilyHandler {
    pair: [OperandFamily; 2],
    symmetric: bool,
    name: HandlerId,
    call: BooleanHandler,
}

const FAMILY_HANDLERS: &[FamilyHandler] = &[
    FamilyHandler {
        pair: [OperandFamily::Sphere, OperandFamily::Cylinder],
        symmetric: true,
        name: HandlerId::SphereCylinderBoolean,
        call: sphere_cylinder_boolean,
    },
    FamilyHandler {
        pair: [OperandFamily::Sphere, OperandFamily::Cone],
        symmetric: true,
        name: HandlerId::SphereConicContainment,
        call: sphere_conic_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Cylinder, OperandFamily::Cone],
        symmetric: true,
        name: HandlerId::CylinderConicContainment,
        call: cylinder_conic_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Cone, OperandFamily::Planar],
        symmetric: true,
        name: HandlerId::ConicBoxContainment,
        call: conic_box_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Sphere, OperandFamily::Torus],
        symmetric: true,
        name: HandlerId::SphereTorusContainment,
        call: sphere_torus_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Cylinder, OperandFamily::Torus],
        symmetric: true,
        name: HandlerId::TorusCylinderContainment,
        call: torus_cylinder_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Planar, OperandFamily::Torus],
        symmetric: true,
        name: HandlerId::TorusBoxContainment,
        call: torus_box_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Cone, OperandFamily::Torus],
        symmetric: true,
        name: HandlerId::TorusConicContainment,
        call: torus_conic_containment,
    },
    FamilyHandler {
        pair: [OperandFamily::Torus, OperandFamily::Torus],
        symmetric: false,
        name: HandlerId::TorusContainmentBoolean,
        call: torus_containment_boolean,
    },
    FamilyHandler {
        pair: [OperandFamily::Cone, OperandFamily::Cone],
        symmetric: false,
        name: HandlerId::ConicContainmentBoolean,
        call: conic_containment_boolean,
    },
    FamilyHandler {
        pair: [OperandFamily::Sphere, OperandFamily::Planar],
        symmetric: true,
        name: HandlerId::SphereBoxBoolean,
        call: sphere_box_boolean,
    },
    FamilyHandler {
        pair: [OperandFamily::Cylinder, OperandFamily::Planar],
        symmetric: true,
        name: HandlerId::CylinderBoxBoolean,
        call: cylinder_box_boolean,
    },
    FamilyHandler {
        pair: [OperandFamily::Cylinder, OperandFamily::Cylinder],
        symmetric: false,
        name: HandlerId::BooleanCylinders,
        call: boolean_cylinders,
    },
];

struct BoxHandler {
    name: HandlerId,
    subtract_only: bool,
    call: BooleanHandler,
}

fn dispatch_boxes(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    boolean_boxes(a, b, operation, id)
}

fn dispatch_rectilinear(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    boolean_rectilinear(a, b, operation, id)
}

fn dispatch_layered(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    _: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_layered_extrusions(a, b, id)
}

fn dispatch_polyhedra(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    _: BooleanOp,
    id: String,
) -> Result<BooleanResult, GeometryError> {
    subtract_planar_polyhedra(a, b, id)
}

const BOX_HANDLERS: &[BoxHandler] = &[
    BoxHandler {
        name: HandlerId::BooleanBoxes,
        subtract_only: false,
        call: dispatch_boxes,
    },
    BoxHandler {
        name: HandlerId::BooleanPlanarExtrusions,
        subtract_only: false,
        call: boolean_planar_extrusions,
    },
    BoxHandler {
        name: HandlerId::BooleanRectilinear,
        subtract_only: false,
        call: dispatch_rectilinear,
    },
    BoxHandler {
        name: HandlerId::SubtractLayeredExtrusions,
        subtract_only: true,
        call: dispatch_layered,
    },
    BoxHandler {
        name: HandlerId::SubtractPlanarPolyhedra,
        subtract_only: true,
        call: dispatch_polyhedra,
    },
];

pub(super) fn specialized_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    let pair = [operand_family(a), operand_family(b)];
    for entry in FAMILY_HANDLERS {
        if pair == entry.pair || (entry.symmetric && pair == [entry.pair[1], entry.pair[0]]) {
            return record_handler(handlers, entry.name, || (entry.call)(a, b, operation, id));
        }
    }
    if pair == [OperandFamily::Planar, OperandFamily::Planar] {
        let mut gap = None;
        for entry in BOX_HANDLERS {
            if entry.subtract_only && operation != BooleanOp::Subtraction {
                break;
            }
            match record_handler(handlers, entry.name, || {
                (entry.call)(a, b, operation, id.clone())
            }) {
                Ok(result) => return Ok(result),
                Err(error @ GeometryError::CoverageGap { .. }) => gap = Some(error),
                Err(error) => return Err(error),
            }
        }
        return Err(gap.unwrap_or_else(|| GeometryError::CoverageGap {
            families: ["planar solid".into(), "planar solid".into()],
        }));
    }
    record_handler(handlers, HandlerId::BooleanSpheres, || {
        boolean_spheres(a, b, operation, id)
    })
}

type OptionalBooleanHandler = fn(
    &BrepEnvelope,
    &BrepEnvelope,
    BooleanOp,
    String,
) -> Result<Option<BooleanResult>, GeometryError>;

struct GenericHandler {
    name: HandlerId,
    call: OptionalBooleanHandler,
}

fn maybe_vertical_arc_extrusion(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
) -> Result<Option<BooleanResult>, GeometryError> {
    if operation != BooleanOp::Subtraction
        || !all_planar(b)
        || !a
            .geometry
            .surfaces
            .iter()
            .any(|surface| matches!(surface, SurfaceGeometry::Cylinder { .. }))
    {
        return Ok(None);
    }
    match subtract_vertical_arc_extrusion(a, b, id) {
        Ok(result) => Ok(Some(result)),
        Err(GeometryError::CoverageGap { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

const GENERIC_HANDLERS: &[GenericHandler] = &[
    GenericHandler {
        name: HandlerId::SubtractVerticalArcExtrusion,
        call: maybe_vertical_arc_extrusion,
    },
    GenericHandler {
        name: HandlerId::GenericTwoSidedCutterBandSubtraction,
        call: generic_two_sided_cutter_band_subtraction,
    },
    GenericHandler {
        name: HandlerId::GenericMultipleClosedLoopsBoolean,
        call: generic_multiple_closed_loops_boolean,
    },
    GenericHandler {
        name: HandlerId::GenericSingleClosedLoopBoolean,
        call: generic_single_closed_loop_boolean,
    },
    GenericHandler {
        name: HandlerId::GenericNonIntersectingBoolean,
        call: generic_non_intersecting_boolean,
    },
];

pub(super) fn generic_boolean(
    a: &BrepEnvelope,
    b: &BrepEnvelope,
    operation: BooleanOp,
    id: String,
    handlers: &mut Vec<String>,
) -> Result<BooleanResult, GeometryError> {
    for entry in GENERIC_HANDLERS {
        if let Some(result) = record_optional(handlers, entry.name, || {
            (entry.call)(a, b, operation, id.clone())
        })? {
            return Ok(result);
        }
    }
    Err(GeometryError::CoverageGap {
        families: [
            "generic analytic solid".into(),
            "intersecting analytic solid".into(),
        ],
    })
}
