use super::chart::{Chart, HalfSpaceUses};
use crate::brep::{Builder, FaceProvenance, FaceRole, GeometryError, SurfaceGeometry, Use};
use crate::operations::modifying::boolean::assembly::{
    box_source, cylinder_source, face_provenance,
};
use crate::operations::modifying::boolean::operands::{BoxInput, CylinderInput};
use crate::operations::modifying::boolean::types::BooleanOp;

pub(super) struct Sources<'a, 'b> {
    pub(super) box_: &'a BoxInput<'b>,
    pub(super) cylinder: &'a CylinderInput<'b>,
    pub(super) operation: BooleanOp,
}

pub(super) fn add_half_space_faces(
    builder: &mut Builder,
    chart: &Chart,
    uses: HalfSpaceUses,
    sources: &Sources<'_, '_>,
) -> Result<(), GeometryError> {
    let pad = builder.brep.accuracy.geometric;
    let cylinder = sources.cylinder;
    let [around, along] = uses.side_bounds;
    add_face(
        builder,
        "cylinder-side",
        SurfaceGeometry::Cylinder {
            frame: chart.frame,
            radius: chart.radius,
        },
        [around, [along[0] - pad, along[1] + pad]],
        uses.side,
        face_provenance(vec![cylinder_source(cylinder, 0)], FaceRole::Split, false),
    )?;
    let disk = [[-chart.radius - pad, chart.radius + pad]; 2];
    let (bottom_face, top_face) = if chart.flipped { (2, 1) } else { (1, 2) };
    if let Some(bottom) = uses.bottom {
        add_face(
            builder,
            "cylinder-cap",
            SurfaceGeometry::Plane {
                frame: chart.cap_bottom(),
            },
            disk,
            bottom,
            face_provenance(
                vec![cylinder_source(cylinder, bottom_face)],
                FaceRole::Split,
                false,
            ),
        )?;
    }
    let top_role = if uses.top_partial {
        FaceRole::Split
    } else {
        FaceRole::Preserved
    };
    add_face(
        builder,
        "cylinder-cap",
        SurfaceGeometry::Plane {
            frame: chart.cap_top(),
        },
        disk,
        uses.top,
        face_provenance(vec![cylinder_source(cylinder, top_face)], top_role, false),
    )?;
    add_face(
        builder,
        "half-space-cut",
        SurfaceGeometry::Plane {
            frame: uses.cut_frame,
        },
        uses.cut_bounds,
        uses.cut,
        face_provenance(
            vec![box_source(sources.box_, chart.box_face)],
            FaceRole::Cut,
            sources.operation == BooleanOp::Subtraction,
        ),
    )
}

fn add_face(
    builder: &mut Builder,
    label: &str,
    surface: SurfaceGeometry,
    bounds: [[f64; 2]; 2],
    uses: Vec<Use>,
    provenance: FaceProvenance,
) -> Result<(), GeometryError> {
    let face = builder.brep.topology.faces.len();
    builder.face(&format!("{face}:{label}"), surface, bounds, uses)?;
    builder.brep.topology.faces[face].provenance = provenance;
    Ok(())
}
