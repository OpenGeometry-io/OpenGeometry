mod boundaries;
mod charts;
mod cutter;
mod host;

use crate::brep::{Accuracy, Builder, FaceRole, GeometryError, SurfaceGeometry};
use crate::intersection::{intersect_breps, IntersectionGraph};
use crate::operations::modifying::boolean::assembly::{
    cylinder_source, face_provenance, finish_cylinder_result, reverse_face,
};
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{BooleanOp, BooleanResult};
use boundaries::{add_numerical_boundaries, align_cutter_boundaries};
use charts::{chart_seam_rotation, rotate_cylinder_chart};
use cutter::{add_cutter_seam, cutter_face_bounds, cutter_face_uses};
use host::{add_host_caps, add_host_lateral_face, add_host_rims};

pub(super) fn transverse_cylinder_through_subtraction(
    host: &CylinderInput<'_>,
    cutter: &CylinderInput<'_>,
    id: String,
    accuracy: Accuracy,
) -> Result<BooleanResult, GeometryError> {
    let graph = lateral_intersection_graph(host, cutter)?;

    let seam_rotation = chart_seam_rotation(&graph);
    let host_frame = rotate_cylinder_chart(host.frame, seam_rotation);
    let mut builder = Builder::new(id, accuracy)?;
    let rims = add_host_rims(&mut builder, host, host_frame)?;

    let mut numerical = add_numerical_boundaries(&mut builder, &graph, seam_rotation, accuracy)?;
    align_cutter_boundaries(&mut builder, &mut numerical)?;
    let lower = &numerical[0];
    let upper = &numerical[1];
    if upper.cutter_middle[1] - lower.cutter_middle[1] <= 4.0 * accuracy.geometric {
        return Err(GeometryError::UnresolvedIntersection(
            "cylinder through-cut branches are below geometric resolution".into(),
        ));
    }

    add_host_lateral_face(&mut builder, host, host_frame, &rims, &numerical)?;
    add_host_caps(&mut builder, host, host_frame, &rims)?;

    let seam = add_cutter_seam(&mut builder, cutter, lower, upper, accuracy)?;
    let cutter_face = builder.brep.topology.faces.len() as u32;
    let cutter_bounds = cutter_face_bounds(&builder, &numerical, lower, upper);
    builder.face(
        "cutter:tunnel",
        SurfaceGeometry::Cylinder {
            frame: cutter.frame,
            radius: cutter.radius,
        },
        cutter_bounds,
        cutter_face_uses(lower, upper, &seam),
    )?;
    builder.brep.topology.faces[0].provenance =
        face_provenance(vec![cylinder_source(host, 0)], FaceRole::Split, false);
    builder.brep.topology.faces[1].provenance =
        face_provenance(vec![cylinder_source(host, 1)], FaceRole::Preserved, false);
    builder.brep.topology.faces[2].provenance =
        face_provenance(vec![cylinder_source(host, 2)], FaceRole::Preserved, false);
    builder.brep.topology.faces[cutter_face as usize].provenance =
        face_provenance(vec![cylinder_source(cutter, 0)], FaceRole::Cut, true);
    reverse_face(&mut builder.brep, cutter_face);
    let out = builder.finish_solid()?;
    finish_cylinder_result(out, host, cutter, BooleanOp::Subtraction, false)
}

fn lateral_intersection_graph(
    host: &CylinderInput<'_>,
    cutter: &CylinderInput<'_>,
) -> Result<IntersectionGraph, GeometryError> {
    let mut body_graph = intersect_breps(host.brep, cutter.brep)?;
    if body_graph.pairs.len() != 1 {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "cap-intersecting cylinder".into(),
            ],
        });
    }
    let pair = body_graph
        .pairs
        .pop()
        .ok_or_else(|| GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "missing lateral intersection graph".into(),
            ],
        })?;
    if pair.faces != [0, 0]
        || pair.graph.coincident
        || !pair.graph.contacts.is_empty()
        || pair.graph.branches.len() != 2
    {
        return Err(GeometryError::CoverageGap {
            families: [
                "cylinder through subtraction".into(),
                "non-transverse cylinder graph".into(),
            ],
        });
    }
    Ok(pair.graph)
}
