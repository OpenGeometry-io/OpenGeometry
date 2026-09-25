use super::{
    export_curve::{fit_intersection_curve, FittedIntersectionCurve},
    geometry::{cross, dot, norm, sub, unit},
    topology::{EdgeGeometry, GeometryQuality, Orientation, PcurveGeometry},
    BrepEnvelope, CurveGeometry, Frame3, GeometryError, SurfaceGeometry,
};
use crate::export::part21::{sanitize_string_literal, Part21Writer};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StepReport {
    pub schema_version: u32,
    pub revision: String,
    pub length_unit: String,
    pub quality: GeometryQuality,
    pub faces: usize,
    pub edges: usize,
    pub solids: usize,
    pub cavity_shells: usize,
    pub collapsed_chart_uses: usize,
    pub geometric_tolerance: f64,
    pub exchange_error_bound: f64,
    pub validation_level: &'static str,
}

fn real(value: f64) -> String {
    format!("{value:.17E}")
}
fn refs(ids: &[usize]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn sense(value: Orientation) -> &'static str {
    if value == Orientation::Forward {
        ".T."
    } else {
        ".F."
    }
}
fn frame_error(frame: &Frame3, bounds: super::geometry::PatchBounds) -> Result<f64, GeometryError> {
    let z = unit(frame.z)?;
    let x = unit(sub(frame.x, z.map(|v| v * dot(frame.x, z))))?;
    let y = cross(z, x);
    let deviation = norm(sub(x, frame.x))
        .max(norm(sub(y, frame.y)))
        .max(norm(sub(z, frame.z)));
    if deviation == 0.0 {
        return Ok(0.0);
    }
    let distance = norm(std::array::from_fn(|i| {
        (bounds.axes[i].lo - frame.origin[i])
            .abs()
            .max((bounds.axes[i].hi - frame.origin[i]).abs())
    }));
    Ok(4.0 * distance * deviation)
}
fn point(writer: &mut Part21Writer, value: &[f64]) -> usize {
    writer.add_entity(format!(
        "CARTESIAN_POINT('',({}))",
        value.iter().map(|v| real(*v)).collect::<Vec<_>>().join(",")
    ))
}
fn direction(writer: &mut Part21Writer, value: &[f64]) -> usize {
    writer.add_entity(format!(
        "DIRECTION('',({}))",
        value.iter().map(|v| real(*v)).collect::<Vec<_>>().join(",")
    ))
}
fn placement(writer: &mut Part21Writer, frame: &Frame3, scale: f64) -> usize {
    let p = point(writer, &frame.origin.map(|v| v * scale));
    let z = direction(writer, &frame.z);
    let x = direction(writer, &frame.x);
    writer.add_entity(format!("AXIS2_PLACEMENT_3D('',#{p},#{z},#{x})"))
}
fn line(
    writer: &mut Part21Writer,
    origin: &[f64],
    velocity: &[f64],
) -> Result<usize, GeometryError> {
    let length = velocity.iter().fold(0.0_f64, |n, v| n.hypot(*v));
    if !length.is_finite() || length == 0.0 {
        return Err(GeometryError::UnsupportedGeometry(
            "STEP pcurve has a singular line direction".into(),
        ));
    }
    let p = point(writer, origin);
    let d = direction(
        writer,
        &velocity.iter().map(|v| v / length).collect::<Vec<_>>(),
    );
    let v = writer.add_entity(format!("VECTOR('',#{d},{})", real(length)));
    Ok(writer.add_entity(format!("LINE('',#{p},#{v})")))
}

fn bspline_curve(
    writer: &mut Part21Writer,
    controls: &[Vec<f64>],
    knots: &[f64],
    multiplicities: &[usize],
) -> Result<usize, GeometryError> {
    if controls.len() < 4 || knots.len() != multiplicities.len() {
        return Err(GeometryError::InvalidGeometry(
            "invalid fitted cubic curve shape".into(),
        ));
    }
    let points = controls
        .iter()
        .map(|control| point(writer, control))
        .collect::<Vec<_>>();
    Ok(writer.add_entity(format!(
        "B_SPLINE_CURVE_WITH_KNOTS('',3,({}),.UNSPECIFIED.,.F.,.F.,({}),({}),.UNSPECIFIED.)",
        refs(&points),
        multiplicities
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        knots
            .iter()
            .map(|value| real(*value))
            .collect::<Vec<_>>()
            .join(",")
    )))
}

fn curve(
    writer: &mut Part21Writer,
    geometry: &CurveGeometry,
    scale: f64,
) -> Result<usize, GeometryError> {
    Ok(match geometry {
        CurveGeometry::Line { origin, direction } => line(
            writer,
            &origin.map(|v| v * scale),
            &direction.map(|v| v * scale),
        )?,
        CurveGeometry::Circle { frame, radius } => {
            let p = placement(writer, frame, scale);
            writer.add_entity(format!("CIRCLE('',#{p},{})", real(radius * scale)))
        }
        CurveGeometry::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => {
            let p = placement(writer, frame, scale);
            writer.add_entity(format!(
                "ELLIPSE('',#{p},{},{})",
                real(major_radius * scale),
                real(minor_radius * scale)
            ))
        }
        CurveGeometry::Intersection { .. } => {
            return Err(GeometryError::UnsupportedGeometry(
                "STEP numerical branches require bounded export-curve fitting".into(),
            ))
        }
    })
}

fn pcurve(
    writer: &mut Part21Writer,
    geometry: &PcurveGeometry,
    surface: &SurfaceGeometry,
    surface_id: usize,
    context: usize,
    scale: f64,
    lift: [i32; 2],
    fitted: Option<&FittedIntersectionCurve>,
) -> Result<usize, GeometryError> {
    // STEP cone v measures distance along a generator; the kernel uses axial height.
    let metric = match surface {
        SurfaceGeometry::Plane { .. } => [scale, scale],
        SurfaceGeometry::Cylinder { .. } => [1.0, scale],
        SurfaceGeometry::Cone { semi_angle, .. } => [1.0, scale / semi_angle.cos()],
        _ => [1.0, 1.0],
    };
    let period = match surface {
        SurfaceGeometry::Plane { .. } => [0.0, 0.0],
        SurfaceGeometry::Torus { .. } => [std::f64::consts::TAU; 2],
        _ => [std::f64::consts::TAU, 0.0],
    };
    let map_origin = |origin: [f64; 2]| {
        std::array::from_fn::<_, 2, _>(|i| (origin[i] + f64::from(lift[i]) * period[i]) * metric[i])
    };
    let geometry_id = match geometry {
        PcurveGeometry::Line2 { origin, direction } => line(
            writer,
            &map_origin(*origin),
            &std::array::from_fn::<_, 2, _>(|i| direction[i] * metric[i]),
        )?,
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => {
            let a = [axis_a[0] * metric[0], axis_a[1] * metric[1], 0.0];
            let b = [axis_b[0] * metric[0], axis_b[1] * metric[1], 0.0];
            let ra = norm(a);
            let rb = norm(b);
            if !ra.is_finite()
                || !rb.is_finite()
                || ra == 0.0
                || rb == 0.0
                || dot(a.map(|v| v / ra), b.map(|v| v / rb)).abs() > 64.0 * f64::EPSILON
            {
                return Err(GeometryError::UnsupportedGeometry(
                    "STEP requires an orthogonal conic pcurve parameterization".into(),
                ));
            }
            let axis = if ra >= rb { a } else { b };
            let p = point(writer, &map_origin(*origin));
            let d = direction(writer, &axis[..2]);
            let frame = writer.add_entity(format!("AXIS2_PLACEMENT_2D('',#{p},#{d})"));
            if ra == rb {
                writer.add_entity(format!("CIRCLE('',#{frame},{})", real(ra)))
            } else {
                writer.add_entity(format!(
                    "ELLIPSE('',#{frame},{},{})",
                    real(ra.max(rb)),
                    real(ra.min(rb))
                ))
            }
        }
        PcurveGeometry::IntersectionSide { side, .. } => {
            let fitted = fitted.ok_or_else(|| {
                GeometryError::InvalidGeometry(
                    "STEP intersection pcurve is missing its certified fit".into(),
                )
            })?;
            let controls = fitted
                .pcurve(*side)
                .iter()
                .map(|control| map_origin(*control).to_vec())
                .collect::<Vec<_>>();
            bspline_curve(writer, &controls, &fitted.knots, &fitted.multiplicities)?
        }
        PcurveGeometry::ProjectedCurve { .. } => {
            return Err(GeometryError::UnsupportedGeometry(
                "STEP projected pcurves require a certified exchange representation".into(),
            ))
        }
    };
    let definition = writer.add_entity(format!(
        "DEFINITIONAL_REPRESENTATION('',(#{geometry_id}),#{context})"
    ));
    Ok(writer.add_entity(format!("PCURVE('',#{surface_id},#{definition})")))
}

pub(super) fn preflight(brep: &BrepEnvelope, scale: f64) -> Result<f64, GeometryError> {
    brep.validate()?;
    if brep.solids.is_empty()
        || !brep.topology.wires.is_empty()
        || brep.topology.shells.iter().any(|shell| !shell.is_closed)
    {
        return Err(GeometryError::UnsupportedGeometry(
            "analytic analytic exchange export requires nonempty closed solid regions without wires".into(),
        ));
    }
    if brep.topology.vertices.len()
        + brep.topology.halfedges.len()
        + brep.topology.faces.len()
        + brep.geometry.curves.len()
        + brep.geometry.surfaces.len()
        + brep.geometry.pcurves.len()
        + brep.geometry.intersections.len()
        + brep.topology.edges.len()
        + brep.topology.loops.len()
        + brep.topology.shells.len()
        + brep.solids.len()
        > 100_000
    {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange body exceeds 100,000 geometry/topology items".into(),
        ));
    }
    if brep.id.len() > 4096 || brep.topology.faces.iter().any(|face| face.key.len() > 4096) {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange body and face labels must be at most 4 KiB".into(),
        ));
    }
    let bounds = brep.bounds()?.ok_or_else(|| {
        GeometryError::InvalidTopology("analytic exchange solid has no bounds".into())
    })?;
    let mut magnitude = bounds
        .axes
        .iter()
        .flat_map(|axis| [axis.lo.abs(), axis.hi.abs()])
        .fold(0.0_f64, f64::max);
    let mut frame_bound = 0.0_f64;
    for face in &brep.topology.faces {
        frame_bound = frame_bound.max(frame_error(
            brep.geometry.surface(face.surface)?.frame(),
            bounds,
        )?);
        magnitude = brep
            .geometry
            .surface(face.surface)?
            .frame()
            .origin
            .iter()
            .fold(magnitude, |m, v| m.max(v.abs()));
    }
    for edge in &brep.topology.edges {
        if let EdgeGeometry::Curve { curve, .. } = edge.geometry {
            let origin = match &brep.geometry.curves[curve as usize] {
                CurveGeometry::Line { origin, .. } => origin,
                CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } => {
                    &frame.origin
                }
                CurveGeometry::Intersection { .. } => continue,
            };
            magnitude = origin.iter().fold(magnitude, |m, v| m.max(v.abs()));
            if let CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } =
                &brep.geometry.curves[curve as usize]
            {
                frame_bound = frame_bound.max(frame_error(frame, bounds)?);
            }
        }
    }
    let exchange_bound =
        brep.accuracy.geometric + 2.0 * frame_bound + 64.0 * f64::EPSILON * magnitude;
    if !exchange_bound.is_finite()
        || exchange_bound > brep.accuracy.exchange
        || !(exchange_bound * scale).is_finite()
    {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange exchange budget is below geometric tolerance or coordinate precision".into(),
        ));
    }
    let encoded =
        serde_json::to_value(brep).map_err(|e| GeometryError::InvalidGeometry(e.to_string()))?;
    fn scalable(value: &serde_json::Value, scale: f64) -> bool {
        match value {
            serde_json::Value::Number(v) => v.as_f64().is_some_and(|v| (v * scale).is_finite()),
            serde_json::Value::Array(v) => v.iter().all(|v| scalable(v, scale)),
            serde_json::Value::Object(v) => v.values().all(|v| scalable(v, scale)),
            _ => true,
        }
    }
    if !scalable(&encoded, scale) {
        return Err(GeometryError::LimitExceeded(
            "analytic exchange unit conversion exceeds floating-point range".into(),
        ));
    }
    Ok(exchange_bound)
}

/// Coordinates in the v2 body are metres. Conversion to the selected output unit occurs here once.
pub fn export_step(brep: &BrepEnvelope, unit: &str) -> Result<(String, StepReport), GeometryError> {
    brep.validate()?;
    let scale = match unit {
        "metre" => 1.0,
        "millimetre" => 1000.0,
        _ => {
            return Err(GeometryError::InvalidGeometry(
                "STEP length unit must be metre or millimetre".into(),
            ))
        }
    };
    let mut exchange_bound = preflight(brep, scale)?;
    let fitting_budget = brep.accuracy.exchange - exchange_bound;
    let mut writer = Part21Writer::new("AUTOMOTIVE_DESIGN");
    writer.set_file_name(&brep.id);
    writer.set_description("OpenGeometry authoritative analytic BRep v2");
    let context2 =
        writer.add_entity("(GEOMETRIC_REPRESENTATION_CONTEXT(2) REPRESENTATION_CONTEXT('',''))");
    let mut surfaces = vec![None; brep.geometry.surfaces.len()];
    for face in &brep.topology.faces {
        if surfaces[face.surface as usize].is_some() {
            continue;
        }
        let surface = brep.geometry.surface(face.surface)?;
        let frame = placement(&mut writer, surface.frame(), scale);
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
    let vertices: Vec<_> = brep
        .topology
        .vertices
        .iter()
        .map(|v| {
            let p = point(&mut writer, &v.position.map(|v| v * scale));
            writer.add_entity(format!("VERTEX_POINT('',#{p})"))
        })
        .collect();
    let mut edges = vec![None; brep.topology.edges.len()];
    let mut curves = vec![None; brep.geometry.curves.len()];
    let mut fitted_curves = vec![None; brep.geometry.curves.len()];
    let mut fitting_error: f64 = 0.0;
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
                        if fitting_budget <= 0.0 {
                            return Err(GeometryError::LimitExceeded(
                                "STEP exchange budget leaves no room for numerical curve fitting"
                                    .into(),
                            ));
                        }
                        let fitted = fit_intersection_curve(
                            &brep.geometry,
                            *definition,
                            fitting_budget,
                            20_000,
                        )?;
                        fitting_error = fitting_error.max(fitted.error_bound);
                        let controls = fitted
                            .controls
                            .iter()
                            .map(|control| control.map(|value| value * scale).to_vec())
                            .collect::<Vec<_>>();
                        let id = bspline_curve(
                            &mut writer,
                            &controls,
                            &fitted.knots,
                            &fitted.multiplicities,
                        )?;
                        fitted_curves[curve_id as usize] = Some(fitted);
                        id
                    }
                    _ => curve(&mut writer, geometry, scale)?,
                };
                curves[curve_id as usize] = Some(id);
                id
            }
        };
        let mut pcurves = Vec::new();
        for use_id in [Some(edge.halfedge), edge.twin_halfedge]
            .into_iter()
            .flatten()
        {
            let use_ = &brep.topology.halfedges[use_id as usize];
            let face = &brep.topology.faces[use_.face.ok_or_else(|| {
                GeometryError::InvalidTopology("STEP solid edge lacks a face use".into())
            })? as usize];
            let pc = use_.geometry_use.pcurve.ok_or_else(|| {
                GeometryError::InvalidTopology("STEP face use lacks a pcurve".into())
            })?;
            let surface_id = surfaces[face.surface as usize].ok_or_else(|| {
                GeometryError::InvalidTopology("STEP surface was not emitted".into())
            })?;
            pcurves.push(pcurve(
                &mut writer,
                &brep.geometry.pcurves[pc as usize],
                brep.geometry.surface(face.surface)?,
                surface_id,
                context2,
                scale,
                use_.geometry_use.periodic_lift,
                fitted_curves[curve_id as usize].as_ref(),
            )?);
        }
        let kind = if edge.chart_seam {
            "SEAM_CURVE"
        } else {
            "SURFACE_CURVE"
        };
        let geometry_id = writer.add_entity(format!(
            "{kind}('',#{geometry_id},({}),.CURVE_3D.)",
            refs(&pcurves)
        ));
        let use_ = &brep.topology.halfedges[edge.halfedge as usize];
        let (from, to) = if use_.geometry_use.sense == Orientation::Forward {
            (use_.from, use_.to)
        } else {
            (use_.to, use_.from)
        };
        edges[edge.id as usize] = Some(writer.add_entity(format!(
            "EDGE_CURVE('',#{},#{},#{geometry_id},.T.)",
            vertices[from as usize], vertices[to as usize]
        )));
    }
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
                    // A collapsed chart interval contributes only its vertex to the 3D boundary.
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
    exchange_bound += fitting_error;
    if exchange_bound > brep.accuracy.exchange {
        return Err(GeometryError::LimitExceeded(
            "STEP fitted curves exceed the exchange error budget".into(),
        ));
    }
    let shells: Vec<_> = brep
        .topology
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
        .collect();
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
    let prefix = if unit == "millimetre" { ".MILLI." } else { "$" };
    let length = writer.add_entity(format!(
        "(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT({prefix},.METRE.))"
    ));
    let angle = writer.add_entity("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    let solid_angle =
        writer.add_entity("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())");
    let uncertainty = writer.add_entity(format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{length},'distance_accuracy_value','exchange error bound')", real(exchange_bound * scale)));
    let context = writer.add_entity(format!("(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('',''))"));
    let representation = writer.add_entity(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('',({}),#{context})",
        refs(&solids)
    ));
    let app = writer.add_entity("APPLICATION_CONTEXT('automotive design')");
    writer.add_entity(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
    ));
    let product_context = writer.add_entity(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let name = sanitize_string_literal(&brep.id);
    let product = writer.add_entity(format!(
        "PRODUCT('{name}','{name}','',(#{product_context}))"
    ));
    let formation = writer.add_entity(format!(
        "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('1','',#{product},.NOT_KNOWN.)"
    ));
    let definition_context = writer.add_entity(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
    ));
    let definition = writer.add_entity(format!(
        "PRODUCT_DEFINITION('','',#{formation},#{definition_context})"
    ));
    let shape = writer.add_entity(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
    writer.add_entity(format!(
        "SHAPE_DEFINITION_REPRESENTATION(#{shape},#{representation})"
    ));
    let text = writer.build().map_err(GeometryError::InvalidGeometry)?;
    let report = StepReport {
        schema_version: 2, revision: brep.revision.to_string(), length_unit: unit.into(), quality: brep.quality.clone(),
        faces: faces.len(), edges: edges.iter().flatten().count(), solids: solids.len(),
        cavity_shells: brep.solids.iter().map(|s| s.cavity_shells.len()).sum(), collapsed_chart_uses,
        geometric_tolerance: brep.accuracy.geometric * scale,
        exchange_error_bound: exchange_bound * scale,
        validation_level: "v2 structure, sampled residuals and Part-21 references; external schema/import validation pending",
    };
    Ok((text, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{
        booleans::{boolean_spheres, BooleanOp},
        primitives,
        topology::Accuracy,
        Curve, Surface,
    };
    use crate::math::interval::Interval;
    fn accuracy() -> Accuracy {
        Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        }
    }
    fn frame() -> Frame3 {
        Frame3::from_axis([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]).unwrap()
    }

    fn numerical_edge_box() -> BrepEnvelope {
        let mut brep = primitives::cuboid(
            "numerical-edge".into(),
            Frame3::IDENTITY,
            [1.0, 2.0, 3.0],
            accuracy(),
        )
        .unwrap();
        let edge = brep.topology.edges[0].clone();
        let super::EdgeGeometry::Curve { curve, range } = edge.geometry else {
            panic!("cuboid edge must be regular")
        };
        let use_a = edge.halfedge;
        let use_b = edge.twin_halfedge.unwrap();
        let face_a = brep.topology.halfedges[use_a as usize].face.unwrap();
        let face_b = brep.topology.halfedges[use_b as usize].face.unwrap();
        let surface_ids = [
            brep.topology.faces[face_a as usize].surface,
            brep.topology.faces[face_b as usize].surface,
        ];
        let evaluator = brep.geometry.curve(curve).unwrap();
        let points = [
            evaluator.point_at(range.lo).unwrap(),
            evaluator.point_at(range.hi).unwrap(),
        ];
        let uv_a = points.map(|point| {
            brep.geometry.surfaces[surface_ids[0] as usize]
                .project(point, None)
                .unwrap()
        });
        let uv_b = points.map(|point| {
            brep.geometry.surfaces[surface_ids[1] as usize]
                .project(point, None)
                .unwrap()
        });
        let padding = 1e-10;
        let tubes = std::array::from_fn(|axis| {
            let values = if axis < 2 {
                [uv_a[0][axis], uv_a[1][axis]]
            } else {
                [uv_b[0][axis - 2], uv_b[1][axis - 2]]
            };
            Interval::new(
                values[0].min(values[1]) - padding,
                values[0].max(values[1]) + padding,
            )
            .unwrap()
        });
        let definition = brep.geometry.intersections.len() as u32;
        brep.geometry
            .intersections
            .push(crate::analytic::intersection::IntersectionDefinition {
                surfaces: surface_ids,
                anchors: vec![
                    crate::analytic::intersection::TraceAnchor {
                        parameter: range.lo,
                        point: points[0],
                        uv_a: uv_a[0],
                        uv_b: uv_b[0],
                    },
                    crate::analytic::intersection::TraceAnchor {
                        parameter: range.hi,
                        point: points[1],
                        uv_a: uv_a[1],
                        uv_b: uv_b[1],
                    },
                ],
                uv_tubes: vec![tubes],
                residual_tolerance: accuracy().intersection,
            });
        brep.geometry.curves[curve as usize] = CurveGeometry::Intersection { definition };
        let pcurve_a = brep.geometry.pcurves.len() as u32;
        brep.geometry
            .pcurves
            .push(PcurveGeometry::IntersectionSide {
                definition,
                side: crate::analytic::topology::IntersectionSide::A,
            });
        let pcurve_b = brep.geometry.pcurves.len() as u32;
        brep.geometry
            .pcurves
            .push(PcurveGeometry::IntersectionSide {
                definition,
                side: crate::analytic::topology::IntersectionSide::B,
            });
        brep.topology.halfedges[use_a as usize].geometry_use.pcurve = Some(pcurve_a);
        brep.topology.halfedges[use_b as usize].geometry_use.pcurve = Some(pcurve_b);
        brep.validate().unwrap();
        brep
    }
    #[test]
    fn exports_all_families_shared_edges_seams_and_explicit_poles() {
        let bodies = [
            primitives::cylinder("c#999's".into(), frame(), 1.0, 2.0, accuracy()).unwrap(),
            primitives::sphere("s".into(), frame(), 1.0, accuracy()).unwrap(),
            primitives::cone("c".into(), frame(), 1.0, 2.0, accuracy()).unwrap(),
            primitives::frustum("f".into(), frame(), 1.0, 0.4, 2.0, accuracy()).unwrap(),
            primitives::torus("t".into(), frame(), 2.0, 0.5, accuracy()).unwrap(),
            primitives::annular_sector_extrusion(
                "w".into(),
                frame(),
                2.0,
                0.2,
                1.5,
                0.3,
                -2.1,
                accuracy(),
            )
            .unwrap(),
        ];
        for brep in bodies {
            let original = brep.to_json().unwrap();
            let (metres, report) = export_step(&brep, "metre").unwrap();
            assert_eq!(report.faces, brep.topology.faces.len());
            assert_eq!(metres.matches("=EDGE_CURVE(").count(), report.edges);
            assert!(!metres.contains("POLY_LOOP"));
            assert!(metres.contains("PCURVE("));
            if brep.topology.edges.iter().any(|e| e.chart_seam) {
                assert!(metres.contains("SEAM_CURVE("));
            }
            let (mm, _) = export_step(&brep, "millimetre").unwrap();
            assert!(mm.contains("SI_UNIT(.MILLI.,.METRE.)"));
            assert_ne!(metres, mm);
            assert_eq!(original, brep.to_json().unwrap());
        }
    }

    #[test]
    fn numerical_intersection_edges_export_as_bounded_cubic_curves() {
        let brep = numerical_edge_box();
        let (step, report) = export_step(&brep, "metre").unwrap();
        assert!(step.contains("B_SPLINE_CURVE_WITH_KNOTS"));
        assert!(report.exchange_error_bound <= brep.accuracy.exchange);
        let ifc = crate::analytic::ifc_exchange::prepare_ifc_body(&brep).unwrap();
        assert!(ifc
            .entities
            .iter()
            .any(|entity| entity.entity_type == "IfcBSplineCurveWithKnots"));
        assert!(ifc.exchange_error_bound <= brep.accuracy.exchange);
    }
    #[test]
    fn preserves_reversed_cut_faces_and_cavity_orientation() {
        let a = primitives::sphere("host".into(), frame(), 2.0, accuracy()).unwrap();
        let b = primitives::sphere("cut".into(), frame(), 0.5, accuracy()).unwrap();
        let cavity = boolean_spheres(&a, &b, BooleanOp::Subtraction, "cavity".into())
            .unwrap()
            .brep;
        let (text, report) = export_step(&cavity, "metre").unwrap();
        assert_eq!(report.cavity_shells, 1);
        assert!(text.contains("BREP_WITH_VOIDS("));
        assert!(text
            .lines()
            .any(|line| line.contains("=ADVANCED_FACE(") && line.ends_with(",.F.);")));
        let mut shifted = frame();
        shifted.origin[0] += 1.7;
        let cutter = primitives::sphere("cutter".into(), shifted, 1.0, accuracy()).unwrap();
        let cut = boolean_spheres(&a, &cutter, BooleanOp::Subtraction, "cut".into())
            .unwrap()
            .brep;
        assert!(export_step(&cut, "metre").is_ok());
        let mut invalid = cut;
        invalid.topology.halfedges[0].next = None;
        assert!(matches!(
            export_step(&invalid, "metre"),
            Err(GeometryError::InvalidTopology(_))
        ));
    }

    #[test]
    fn rejects_unrepresentable_exchange_and_preserves_large_revisions() {
        let mut brep = primitives::sphere("budget".into(), frame(), 1.0, accuracy()).unwrap();
        brep.revision = u64::MAX;
        let (_, report) = export_step(&brep, "metre").unwrap();
        assert_eq!(report.revision, u64::MAX.to_string());
        brep.accuracy.exchange = 1e-10;
        assert!(matches!(
            export_step(&brep, "metre"),
            Err(GeometryError::LimitExceeded(_))
        ));
        brep.accuracy.exchange = 1e-6;
        brep.id = "x".repeat(4097);
        assert!(matches!(
            export_step(&brep, "metre"),
            Err(GeometryError::LimitExceeded(_))
        ));
        assert!(matches!(
            export_step(&brep, "inch"),
            Err(GeometryError::InvalidGeometry(_))
        ));
    }
}
