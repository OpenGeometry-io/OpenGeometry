use super::provenance::cylinder_face_mappings;
use crate::brep::{
    BrepEnvelope, CurveGeometry, EdgeGeometry, FaceProvenance, FaceRole, FaceSource, GeometryError,
    GeometryQuality, PcurveGeometry,
};
use crate::operations::modifying::boolean::operands::CylinderInput;
use crate::operations::modifying::boolean::types::{
    BooleanOp, BooleanReport, BooleanResult, FaceMapping,
};

struct IdOffsets {
    surface: u32,
    curve: u32,
    pcurve: u32,
    intersection: u32,
    vertex: u32,
    edge: u32,
    halfedge: u32,
    loop_: u32,
    face: u32,
    wire: u32,
    shell: u32,
}

pub(crate) fn append_analytic_input(
    out: &mut BrepEnvelope,
    input: &BrepEnvelope,
) -> Result<Vec<FaceMapping>, GeometryError> {
    let offsets = id_offsets(out)?;
    add_input_geometry(out, input, &offsets);
    add_input_loop_topology(out, input, &offsets);
    let mappings = add_input_faces(out, input, &offsets);
    add_input_wires_shells_and_solids(out, input, &offsets);
    Ok(mappings)
}

fn id_offsets(out: &BrepEnvelope) -> Result<IdOffsets, GeometryError> {
    let offset = |value: usize, table: &str| {
        u32::try_from(value).map_err(|_| GeometryError::LimitExceeded(table.into()))
    };
    Ok(IdOffsets {
        surface: offset(out.geometry.surfaces.len(), "surface IDs")?,
        curve: offset(out.geometry.curves.len(), "curve IDs")?,
        pcurve: offset(out.geometry.pcurves.len(), "pcurve IDs")?,
        intersection: offset(out.geometry.intersections.len(), "intersection IDs")?,
        vertex: offset(out.topology.vertices.len(), "vertex IDs")?,
        edge: offset(out.topology.edges.len(), "edge IDs")?,
        halfedge: offset(out.topology.halfedges.len(), "halfedge IDs")?,
        loop_: offset(out.topology.loops.len(), "loop IDs")?,
        face: offset(out.topology.faces.len(), "face IDs")?,
        wire: offset(out.topology.wires.len(), "wire IDs")?,
        shell: offset(out.topology.shells.len(), "shell IDs")?,
    })
}

fn add_input_geometry(out: &mut BrepEnvelope, input: &BrepEnvelope, offsets: &IdOffsets) {
    out.geometry
        .surfaces
        .extend(input.geometry.surfaces.clone());
    out.geometry
        .intersections
        .extend(
            input
                .geometry
                .intersections
                .iter()
                .cloned()
                .map(|mut definition| {
                    definition.surfaces = definition.surfaces.map(|id| id + offsets.surface);
                    definition
                }),
        );
    out.geometry
        .curves
        .extend(input.geometry.curves.iter().cloned().map(|mut curve| {
            if let CurveGeometry::Intersection { definition } = &mut curve {
                *definition += offsets.intersection;
            }
            curve
        }));
    out.geometry
        .pcurves
        .extend(input.geometry.pcurves.iter().cloned().map(|mut pcurve| {
            match &mut pcurve {
                PcurveGeometry::ProjectedCurve { curve, surface, .. } => {
                    *curve += offsets.curve;
                    *surface += offsets.surface;
                }
                PcurveGeometry::IntersectionSide { definition, .. } => {
                    *definition += offsets.intersection;
                }
                PcurveGeometry::Line2 { .. } | PcurveGeometry::Conic2 { .. } => {}
            }
            pcurve
        }));
}

fn add_input_loop_topology(out: &mut BrepEnvelope, input: &BrepEnvelope, offsets: &IdOffsets) {
    out.topology
        .vertices
        .extend(input.topology.vertices.iter().cloned().map(|mut vertex| {
            vertex.id += offsets.vertex;
            vertex.outgoing_halfedge = vertex.outgoing_halfedge.map(|id| id + offsets.halfedge);
            vertex
        }));
    out.topology
        .edges
        .extend(input.topology.edges.iter().cloned().map(|mut edge| {
            edge.id += offsets.edge;
            edge.halfedge += offsets.halfedge;
            edge.twin_halfedge = edge.twin_halfedge.map(|id| id + offsets.halfedge);
            match &mut edge.geometry {
                EdgeGeometry::Curve { curve, .. } => *curve += offsets.curve,
                EdgeGeometry::Collapsed { vertex } => *vertex += offsets.vertex,
            }
            edge
        }));
    out.topology.halfedges.extend(
        input
            .topology
            .halfedges
            .iter()
            .cloned()
            .map(|mut halfedge| {
                halfedge.id += offsets.halfedge;
                halfedge.from += offsets.vertex;
                halfedge.to += offsets.vertex;
                halfedge.edge += offsets.edge;
                halfedge.twin = halfedge.twin.map(|id| id + offsets.halfedge);
                halfedge.next = halfedge.next.map(|id| id + offsets.halfedge);
                halfedge.prev = halfedge.prev.map(|id| id + offsets.halfedge);
                halfedge.face = halfedge.face.map(|id| id + offsets.face);
                halfedge.loop_ref = halfedge.loop_ref.map(|id| id + offsets.loop_);
                halfedge.wire_ref = halfedge.wire_ref.map(|id| id + offsets.wire);
                halfedge.geometry_use.pcurve =
                    halfedge.geometry_use.pcurve.map(|id| id + offsets.pcurve);
                halfedge
            }),
    );
    out.topology
        .loops
        .extend(input.topology.loops.iter().cloned().map(|mut loop_| {
            loop_.id += offsets.loop_;
            loop_.start_halfedge += offsets.halfedge;
            loop_.face_ref += offsets.face;
            loop_
        }));
}

fn add_input_faces(
    out: &mut BrepEnvelope,
    input: &BrepEnvelope,
    offsets: &IdOffsets,
) -> Vec<FaceMapping> {
    let mut mappings = Vec::with_capacity(input.topology.faces.len());
    out.topology
        .faces
        .extend(input.topology.faces.iter().cloned().map(|mut face| {
            let source = FaceSource {
                entity: input.id.clone(),
                body: input.id.clone(),
                key: face.key.clone(),
                face: face.id,
            };
            face.id += offsets.face;
            face.key = format!("{}:{}", input.id, face.key);
            face.surface += offsets.surface;
            face.trim.outer += offsets.loop_;
            for hole in &mut face.trim.holes {
                *hole += offsets.loop_;
            }
            face.shell_ref = face.shell_ref.map(|id| id + offsets.shell);
            face.provenance = FaceProvenance {
                sources: vec![source.clone()],
                role: FaceRole::Preserved,
                reversed: false,
            };
            mappings.push(FaceMapping {
                source,
                result_faces: vec![face.id],
            });
            face
        }));
    mappings
}

fn add_input_wires_shells_and_solids(
    out: &mut BrepEnvelope,
    input: &BrepEnvelope,
    offsets: &IdOffsets,
) {
    out.topology
        .wires
        .extend(input.topology.wires.iter().cloned().map(|mut wire| {
            wire.id += offsets.wire;
            wire.start_halfedge += offsets.halfedge;
            wire
        }));
    out.topology
        .shells
        .extend(input.topology.shells.iter().cloned().map(|mut shell| {
            shell.id += offsets.shell;
            for face in &mut shell.faces {
                *face += offsets.face;
            }
            shell
        }));
    out.solids
        .extend(input.solids.iter().cloned().map(|mut solid| {
            solid.outer_shell += offsets.shell;
            for shell in &mut solid.cavity_shells {
                *shell += offsets.shell;
            }
            solid
        }));
}

pub(crate) fn finish_analytic_result(
    mut out: BrepEnvelope,
    first: &BrepEnvelope,
    second: &BrepEnvelope,
    first_is_a: bool,
    operation: BooleanOp,
    coincident: bool,
) -> Result<BooleanResult, GeometryError> {
    out.revision = first
        .revision
        .max(second.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let inputs = if first_is_a {
        [first, second]
    } else {
        [second, first]
    };
    Ok(BooleanResult {
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident,
            face_mappings: analytic_face_mappings(&out, inputs),
        },
        brep: out,
    })
}

pub(crate) fn analytic_face_mappings<'a>(
    out: &BrepEnvelope,
    inputs: impl IntoIterator<Item = &'a BrepEnvelope>,
) -> Vec<FaceMapping> {
    inputs
        .into_iter()
        .flat_map(|input| {
            input.topology.faces.iter().map(|face| {
                let source = FaceSource {
                    entity: input.id.clone(),
                    body: input.id.clone(),
                    key: face.key.clone(),
                    face: face.id,
                };
                let result_faces = out
                    .topology
                    .faces
                    .iter()
                    .filter(|result| {
                        result.provenance.sources.iter().any(|candidate| {
                            candidate.entity == source.entity
                                && candidate.body == source.body
                                && candidate.key == source.key
                                && candidate.face == source.face
                        })
                    })
                    .map(|result| result.id)
                    .collect();
                FaceMapping {
                    source,
                    result_faces,
                }
            })
        })
        .collect()
}

pub(crate) fn finish_cylinder_result(
    mut out: BrepEnvelope,
    a: &CylinderInput<'_>,
    b: &CylinderInput<'_>,
    operation: BooleanOp,
    coincident: bool,
) -> Result<BooleanResult, GeometryError> {
    out.revision = a
        .brep
        .revision
        .max(b.brep.revision)
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("boolean revision overflow".into()))?;
    out.validate()?;
    let face_mappings = cylinder_face_mappings(&out, [a, b]);
    Ok(BooleanResult {
        brep: out,
        report: BooleanReport {
            operation,
            quality: GeometryQuality::Analytic,
            contacts: Vec::new(),
            coincident,
            face_mappings,
        },
    })
}
