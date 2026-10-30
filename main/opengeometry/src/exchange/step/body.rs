use super::entities::{
    bspline_curve, curve, pcurve, placement, point, real, refs, sense, PcurveOnSurface,
};
use crate::brep::{
    BrepEnvelope, CurveGeometry, Edge, EdgeGeometry, GeometryError, Orientation, PcurveGeometry,
    SurfaceGeometry,
};
use crate::exchange::fit_curve::{fit_intersection_curve, FittedIntersectionCurve};
use crate::exchange::part21::{sanitize_string_literal, Part21Writer};

pub(super) struct BodyEmission {
    pub(super) solids: Vec<usize>,
    pub(super) faces: usize,
    pub(super) edges: usize,
    pub(super) cavity_shells: usize,
    pub(super) collapsed_chart_uses: usize,
    pub(super) fitted_curves: usize,
    pub(super) pcurveless_edges: usize,
    pub(super) exchange_bound: f64,
}

struct BodyEntities<'a> {
    brep: &'a BrepEnvelope,
    scale: f64,
    context2: usize,
    surfaces: &'a [Option<usize>],
    vertices: &'a [usize],
}

struct EdgeEmission {
    edges: Vec<Option<usize>>,
    fitted_curves: Vec<Option<FittedIntersectionCurve>>,
    fitting_error: f64,
    pcurveless_edges: usize,
}

pub(super) fn emit_body(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    scale: f64,
    context2: usize,
    mut exchange_bound: f64,
) -> Result<BodyEmission, GeometryError> {
    let fitting_budget = brep.accuracy.exchange - exchange_bound;
    let surfaces = add_surfaces(writer, brep, scale)?;
    let vertices = add_vertices(writer, brep, scale);
    let body_entities = BodyEntities {
        brep,
        scale,
        context2,
        surfaces: &surfaces,
        vertices: &vertices,
    };
    let EdgeEmission {
        edges,
        fitted_curves,
        fitting_error,
        pcurveless_edges,
    } = add_edges(writer, &body_entities, fitting_budget)?;
    let (faces, collapsed_chart_uses) = add_faces(writer, brep, &surfaces, &vertices, &edges)?;
    exchange_bound += fitting_error;
    if exchange_bound > brep.accuracy.exchange {
        return Err(GeometryError::LimitExceeded(
            "STEP fitted curves exceed the exchange error budget".into(),
        ));
    }
    let shells = add_shells(writer, brep, &faces);
    let solids = add_solids(writer, brep, &shells);
    Ok(BodyEmission {
        faces: faces.len(),
        edges: edges.iter().flatten().count(),
        cavity_shells: brep
            .solids
            .iter()
            .map(|solid| solid.cavity_shells.len())
            .sum(),
        collapsed_chart_uses,
        fitted_curves: fitted_curves.iter().flatten().count(),
        pcurveless_edges,
        solids,
        exchange_bound,
    })
}

fn add_surfaces(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    scale: f64,
) -> Result<Vec<Option<usize>>, GeometryError> {
    let mut surfaces = vec![None; brep.geometry.surfaces.len()];
    for face in &brep.topology.faces {
        if surfaces[face.surface as usize].is_some() {
            continue;
        }
        let surface = brep.geometry.surface(face.surface)?;
        let frame = placement(writer, *surface.frame(), scale);
        let expression = match surface {
            SurfaceGeometry::Plane { .. } => format!("PLANE('',#{frame})"),
            SurfaceGeometry::Sphere { radius, .. } => {
                format!("SPHERICAL_SURFACE('',#{frame},{})", real(radius * scale))
            }
            SurfaceGeometry::Cylinder { radius, .. } => {
                format!("CYLINDRICAL_SURFACE('',#{frame},{})", real(radius * scale))
            }
            SurfaceGeometry::Cone { semi_angle, .. } => {
                format!("CONICAL_SURFACE('',#{frame},0.,{})", real(*semi_angle))
            }
            SurfaceGeometry::Torus {
                major_radius,
                minor_radius,
                ..
            } => format!(
                "TOROIDAL_SURFACE('',#{frame},{},{})",
                real(major_radius * scale),
                real(minor_radius * scale)
            ),
        };
        surfaces[face.surface as usize] = Some(writer.add_entity(expression));
    }
    Ok(surfaces)
}

fn add_vertices(writer: &mut Part21Writer, brep: &BrepEnvelope, scale: f64) -> Vec<usize> {
    brep.topology
        .vertices
        .iter()
        .map(|v| {
            let p = point(writer, &v.position.map(|v| v * scale));
            writer.add_entity(format!("VERTEX_POINT('',#{p})"))
        })
        .collect()
}

fn add_edges(
    writer: &mut Part21Writer,
    body: &BodyEntities<'_>,
    fitting_budget: f64,
) -> Result<EdgeEmission, GeometryError> {
    let BodyEntities { brep, scale, .. } = *body;
    let mut edges = vec![None; brep.topology.edges.len()];
    let mut curves = vec![None; brep.geometry.curves.len()];
    let mut fitted_curves = vec![None; brep.geometry.curves.len()];
    let mut fitting_error: f64 = 0.0;
    let mut pcurveless_edges = 0;
    for edge in &brep.topology.edges {
        let EdgeGeometry::Curve {
            curve: curve_id,
            range,
        } = edge.geometry
        else {
            continue;
        };
        let geometry = &brep.geometry.curves[curve_id as usize];
        if matches!(
            geometry,
            CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. }
        ) && range.hi - range.lo > std::f64::consts::TAU
        {
            return Err(GeometryError::UnsupportedGeometry(
                "STEP edge spans more than one conic period".into(),
            ));
        }
        let geometry_id = match curves[curve_id as usize] {
            Some(id) => id,
            None => {
                let id = match geometry {
                    CurveGeometry::Intersection { definition } => {
                        let (id, fitted) = add_fitted_curve(
                            writer,
                            brep,
                            *definition,
                            fitting_budget,
                            scale,
                            &mut fitting_error,
                        )?;
                        fitted_curves[curve_id as usize] = Some(fitted);
                        id
                    }
                    _ => curve(writer, geometry, scale)?,
                };
                curves[curve_id as usize] = Some(id);
                id
            }
        };
        edges[edge.id as usize] = Some(add_edge_curve(
            writer,
            body,
            edge,
            curve_id,
            geometry_id,
            &fitted_curves,
            &mut pcurveless_edges,
        )?);
    }
    Ok(EdgeEmission {
        edges,
        fitted_curves,
        fitting_error,
        pcurveless_edges,
    })
}

fn add_fitted_curve(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    definition: u32,
    fitting_budget: f64,
    scale: f64,
    fitting_error: &mut f64,
) -> Result<(usize, FittedIntersectionCurve), GeometryError> {
    if fitting_budget <= 0.0 {
        return Err(GeometryError::LimitExceeded(
            "STEP exchange budget leaves no room for numerical curve fitting".into(),
        ));
    }
    let fitted = fit_intersection_curve(&brep.geometry, definition, fitting_budget, 20_000)?;
    *fitting_error = fitting_error.max(fitted.error_bound);
    let controls = fitted
        .controls
        .iter()
        .map(|control| control.map(|value| value * scale).to_vec())
        .collect::<Vec<_>>();
    let id = bspline_curve(writer, &controls, &fitted.knots, &fitted.multiplicities)?;
    Ok((id, fitted))
}

fn add_edge_curve(
    writer: &mut Part21Writer,
    body: &BodyEntities<'_>,
    edge: &Edge,
    curve_id: u32,
    geometry_id: usize,
    fitted_curves: &[Option<FittedIntersectionCurve>],
    pcurveless_edges: &mut usize,
) -> Result<usize, GeometryError> {
    let BodyEntities {
        brep,
        scale,
        context2,
        surfaces,
        vertices,
    } = *body;
    let uses = [Some(edge.halfedge), edge.twin_halfedge]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let projected = any_projected_pcurve(brep, &uses)?;
    if projected && edge.chart_seam {
        return Err(GeometryError::UnsupportedGeometry(
            "STEP seam edge has a projected pcurve".into(),
        ));
    }
    let mut pcurves = Vec::new();
    for use_id in uses.into_iter().filter(|_| !projected) {
        let use_ = &brep.topology.halfedges[use_id as usize];
        let face = &brep.topology.faces[use_.face.ok_or_else(|| {
            GeometryError::InvalidTopology("STEP solid edge lacks a face use".into())
        })? as usize];
        let pc = use_
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("STEP face use lacks a pcurve".into()))?;
        let surface_id = surfaces[face.surface as usize]
            .ok_or_else(|| GeometryError::InvalidTopology("STEP surface was not emitted".into()))?;
        pcurves.push(pcurve(
            writer,
            &PcurveOnSurface {
                geometry: &brep.geometry.pcurves[pc as usize],
                surface: brep.geometry.surface(face.surface)?,
                surface_id,
            },
            context2,
            scale,
            use_.geometry_use.periodic_lift,
            fitted_curves[curve_id as usize].as_ref(),
        )?);
    }
    let geometry_id = if projected {
        *pcurveless_edges += 1;
        geometry_id
    } else {
        let kind = if edge.chart_seam {
            "SEAM_CURVE"
        } else {
            "SURFACE_CURVE"
        };
        writer.add_entity(format!(
            "{kind}('',#{geometry_id},({}),.CURVE_3D.)",
            refs(&pcurves)
        ))
    };
    let use_ = &brep.topology.halfedges[edge.halfedge as usize];
    let (from, to) = if use_.geometry_use.sense == Orientation::Forward {
        (use_.from, use_.to)
    } else {
        (use_.to, use_.from)
    };
    Ok(writer.add_entity(format!(
        "EDGE_CURVE('',#{},#{},#{geometry_id},.T.)",
        vertices[from as usize], vertices[to as usize]
    )))
}

fn any_projected_pcurve(brep: &BrepEnvelope, uses: &[u32]) -> Result<bool, GeometryError> {
    let mut projected = false;
    for use_id in uses {
        let use_ = &brep.topology.halfedges[*use_id as usize];
        let pc = use_
            .geometry_use
            .pcurve
            .ok_or_else(|| GeometryError::InvalidTopology("STEP face use lacks a pcurve".into()))?;
        projected |= matches!(
            brep.geometry.pcurves[pc as usize],
            PcurveGeometry::ProjectedCurve { .. }
        );
    }
    Ok(projected)
}

fn add_faces(
    writer: &mut Part21Writer,
    brep: &BrepEnvelope,
    surfaces: &[Option<usize>],
    vertices: &[usize],
    edges: &[Option<usize>],
) -> Result<(Vec<usize>, usize), GeometryError> {
    let mut faces = Vec::new();
    let mut collapsed_chart_uses = 0;
    for face in &brep.topology.faces {
        let mut bounds = Vec::new();
        for loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
            let loop_ = &brep.topology.loops[*loop_id as usize];
            let start = loop_.start_halfedge;
            let mut current = start;
            let mut oriented = Vec::new();
            for _ in 0..=brep.topology.halfedges.len() {
                let use_ = &brep.topology.halfedges[current as usize];
                if let Some(edge) = edges[use_.edge as usize] {
                    oriented.push(writer.add_entity(format!(
                        "ORIENTED_EDGE('',*,*,#{edge},{})",
                        sense(use_.geometry_use.sense)
                    )));
                } else {
                    collapsed_chart_uses += 1;
                }
                current = use_.next.ok_or_else(|| {
                    GeometryError::InvalidTopology("STEP loop has no next use".into())
                })?;
                if current == start {
                    break;
                }
            }
            let loop_geometry = if oriented.is_empty() {
                writer.add_entity(format!(
                    "VERTEX_LOOP('',#{})",
                    vertices[brep.topology.halfedges[start as usize].from as usize]
                ))
            } else {
                writer.add_entity(format!("EDGE_LOOP('',({}))", refs(&oriented)))
            };
            let kind = if loop_.is_hole {
                "FACE_BOUND"
            } else {
                "FACE_OUTER_BOUND"
            };
            bounds.push(writer.add_entity(format!("{kind}('',#{loop_geometry},.T.)")));
        }
        let surface = surfaces[face.surface as usize]
            .ok_or_else(|| GeometryError::InvalidTopology("STEP face lacks its support".into()))?;
        faces.push(writer.add_entity(format!(
            "ADVANCED_FACE('{}',({}),#{surface},{})",
            sanitize_string_literal(&face.key),
            refs(&bounds),
            sense(face.sense)
        )));
    }
    Ok((faces, collapsed_chart_uses))
}

fn add_shells(writer: &mut Part21Writer, brep: &BrepEnvelope, faces: &[usize]) -> Vec<usize> {
    brep.topology
        .shells
        .iter()
        .map(|shell| {
            writer.add_entity(format!(
                "CLOSED_SHELL('',({}))",
                refs(
                    &shell
                        .faces
                        .iter()
                        .map(|id| faces[*id as usize])
                        .collect::<Vec<_>>()
                )
            ))
        })
        .collect()
}

fn add_solids(writer: &mut Part21Writer, brep: &BrepEnvelope, shells: &[usize]) -> Vec<usize> {
    let mut solids = Vec::new();
    for (i, solid) in brep.solids.iter().enumerate() {
        let outer = shells[solid.outer_shell as usize];
        let name = sanitize_string_literal(&format!("{}-{i}", brep.id));
        let expression = if solid.cavity_shells.is_empty() {
            format!("MANIFOLD_SOLID_BREP('{name}',#{outer})")
        } else {
            let voids: Vec<_> = solid
                .cavity_shells
                .iter()
                .map(|id| {
                    writer.add_entity(format!(
                        "ORIENTED_CLOSED_SHELL('',*,#{},.T.)",
                        shells[*id as usize]
                    ))
                })
                .collect();
            format!("BREP_WITH_VOIDS('{name}',#{outer},({}))", refs(&voids))
        };
        solids.push(writer.add_entity(expression));
    }
    solids
}
