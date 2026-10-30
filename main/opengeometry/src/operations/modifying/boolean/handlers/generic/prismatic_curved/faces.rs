use super::levels::gap;
use super::regions::Caps;
use crate::brep::{
    Accuracy, BrepEnvelope, FaceProvenance, FaceRole, FaceSource, Frame3, GeometryError,
};
use crate::geom2d::{CurveEdge2, CurveRegion2};
use crate::operations::modifying::boolean::handlers::vertical_arc_extrusion::{
    add_sides, source_for_cap_region, source_for_side, Facets, SidePatch,
};
use crate::operations::modifying::boolean::operands::unique_sources;
use crate::operations::modifying::boolean::types::BooleanOp;

pub(super) struct Operands<'a> {
    pub(super) a: &'a BrepEnvelope,
    pub(super) b: &'a BrepEnvelope,
    pub(super) operation: BooleanOp,
    pub(super) base: Frame3,
    pub(super) accuracy: Accuracy,
}

pub(super) fn add_faces(
    facets: &mut Facets,
    operands: &Operands<'_>,
    layers: &[Vec<CurveRegion2>],
    caps: &[Caps],
    levels: &[f64],
) -> Result<(), GeometryError> {
    for (cap, &level) in caps.iter().zip(levels) {
        for (regions, up) in [(&cap.up, true), (&cap.down, false)] {
            for region in regions {
                let provenance = cap_provenance(operands, level, region)?;
                facets.cap(std::slice::from_ref(region), level, up, provenance)?;
            }
        }
    }
    let mut patches = Vec::new();
    for (regions, pair) in layers.iter().zip(levels.windows(2)) {
        for region in regions {
            for edge in std::iter::once(&region.outer)
                .chain(&region.holes)
                .flatten()
            {
                patches.push(side_patch(operands, edge, pair[0], pair[1])?);
            }
        }
    }
    add_sides(facets, &patches, operands.accuracy)
}

fn cap_provenance(
    operands: &Operands<'_>,
    level: f64,
    region: &CurveRegion2,
) -> Result<FaceProvenance, GeometryError> {
    let &Operands {
        a,
        b,
        operation,
        base,
        accuracy,
    } = operands;
    let tolerance = accuracy.intersection;
    provenance(
        source_for_cap_region(a, base, level, region, tolerance)?,
        source_for_cap_region(b, base, level, region, tolerance)?,
        operation,
    )
}

fn side_patch(
    operands: &Operands<'_>,
    edge: &CurveEdge2,
    lo: f64,
    hi: f64,
) -> Result<SidePatch, GeometryError> {
    let &Operands {
        a,
        b,
        operation,
        base,
        accuracy,
    } = operands;
    let tolerance = accuracy.intersection;
    let point = edge.point(0.5);
    let middle = base.point([point.x, point.z, (lo + hi) / 2.0]);
    Ok(SidePatch {
        edge: edge.clone(),
        levels: vec![lo, hi],
        provenance: provenance(
            source_for_side(a, middle, base.z, tolerance)?,
            source_for_side(b, middle, base.z, tolerance)?,
            operation,
        )?,
    })
}

fn provenance(
    from_a: Vec<FaceSource>,
    from_b: Vec<FaceSource>,
    operation: BooleanOp,
) -> Result<FaceProvenance, GeometryError> {
    if from_a.is_empty() && from_b.is_empty() {
        return Err(gap());
    }
    let reversed = operation == BooleanOp::Subtraction && from_a.is_empty();
    let role = if !from_a.is_empty() && !from_b.is_empty() {
        FaceRole::Coincident
    } else if reversed {
        FaceRole::Cut
    } else {
        FaceRole::Split
    };
    Ok(FaceProvenance {
        sources: unique_sources(from_a.into_iter().chain(from_b)),
        role,
        reversed,
    })
}
