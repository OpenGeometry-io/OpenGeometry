use super::entities::{enumeration, real, reals, reference, references, Entities, IfcValue};
use crate::brep::{BrepEnvelope, CurveGeometry, EdgeGeometry, GeometryError, Orientation};
use crate::exchange::fit_curve::{fit_intersection_curve, FittedIntersectionCurve};
use crate::math::{norm, Point3};
use std::f64::consts::TAU;

pub(super) struct EdgeEntities {
    pub(super) vertices: Vec<u32>,
    pub(super) edges: Vec<Option<u32>>,
    pub(super) fitting_error: f64,
}

struct CurveCache {
    curves: Vec<Option<u32>>,
    fitting_budget: f64,
    fitting_error: f64,
}

pub(super) fn add_edges(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    fitting_budget: f64,
) -> Result<EdgeEntities, GeometryError> {
    let vertices: Vec<_> = brep
        .topology
        .vertices
        .iter()
        .map(|vertex| {
            let point = entities.point(&vertex.position);
            entities.add("IfcVertexPoint", vec![reference(point)])
        })
        .collect();
    let mut edges = vec![None; brep.topology.edges.len()];
    let mut cache = CurveCache {
        curves: vec![None; brep.geometry.curves.len()],
        fitting_budget,
        fitting_error: 0.0,
    };
    for edge in &brep.topology.edges {
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            continue;
        };
        let geometry = &brep.geometry.curves[curve as usize];
        if matches!(
            geometry,
            CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. }
        ) && range.width() > TAU
        {
            return Err(GeometryError::UnsupportedGeometry(
                "IFC conic edges cannot exceed one period".into(),
            ));
        }
        let curve_id = add_cached_curve(entities, brep, &mut cache, curve)?;
        let use_ = &brep.topology.halfedges[edge.halfedge as usize];
        let (from, to) = if use_.geometry_use.sense == Orientation::Forward {
            (use_.from, use_.to)
        } else {
            (use_.to, use_.from)
        };
        edges[edge.id as usize] = Some(entities.add(
            "IfcEdgeCurve",
            vec![
                reference(vertices[from as usize]),
                reference(vertices[to as usize]),
                reference(curve_id),
                IfcValue::Bool { value: true },
            ],
        ));
    }
    Ok(EdgeEntities {
        vertices,
        edges,
        fitting_error: cache.fitting_error,
    })
}

fn add_cached_curve(
    entities: &mut Entities,
    brep: &BrepEnvelope,
    cache: &mut CurveCache,
    curve: u32,
) -> Result<u32, GeometryError> {
    if let Some(id) = cache.curves[curve as usize] {
        return Ok(id);
    }
    let id = match &brep.geometry.curves[curve as usize] {
        CurveGeometry::Intersection { definition } => {
            if cache.fitting_budget <= 0.0 {
                return Err(GeometryError::LimitExceeded(
                    "IFC exchange budget leaves no room for numerical curve fitting".into(),
                ));
            }
            let fitted =
                fit_intersection_curve(&brep.geometry, *definition, cache.fitting_budget, 20_000)?;
            cache.fitting_error = cache.fitting_error.max(fitted.error_bound);
            add_fitted_curve(entities, &fitted)?
        }
        geometry => add_curve(entities, geometry)?,
    };
    cache.curves[curve as usize] = Some(id);
    Ok(id)
}

fn add_curve(entities: &mut Entities, geometry: &CurveGeometry) -> Result<u32, GeometryError> {
    Ok(match geometry {
        CurveGeometry::Line { origin, direction } => {
            let p = entities.point(origin);
            let d = entities.direction(direction);
            let v = entities.add("IfcVector", vec![reference(d), real(norm(*direction))]);
            entities.add("IfcLine", vec![reference(p), reference(v)])
        }
        CurveGeometry::Circle { frame, radius } => {
            let position = entities.placement(*frame);
            entities.add("IfcCircle", vec![reference(position), real(*radius)])
        }
        CurveGeometry::Ellipse {
            frame,
            major_radius,
            minor_radius,
        } => {
            let position = entities.placement(*frame);
            entities.add(
                "IfcEllipse",
                vec![
                    reference(position),
                    real(*major_radius),
                    real(*minor_radius),
                ],
            )
        }
        CurveGeometry::Intersection { .. } => {
            return Err(GeometryError::UnsupportedGeometry(
                "IFC numerical intersection edges require bounded export-only fitting".into(),
            ))
        }
    })
}

fn add_fitted_curve(
    entities: &mut Entities,
    fitted: &FittedIntersectionCurve,
) -> Result<u32, GeometryError> {
    if fitted.controls.len() < 4 || fitted.knots.len() != fitted.multiplicities.len() {
        return Err(GeometryError::InvalidGeometry(
            "invalid IFC fitted cubic curve shape".into(),
        ));
    }
    let points = fitted
        .controls
        .iter()
        .map(|point: &Point3| entities.point(point))
        .collect::<Vec<_>>();
    let multiplicities = fitted
        .multiplicities
        .iter()
        .map(|value| {
            i64::try_from(*value)
                .map(|value| IfcValue::Integer { value })
                .map_err(|_| GeometryError::LimitExceeded("IFC knot multiplicity".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entities.add(
        "IfcBSplineCurveWithKnots",
        vec![
            IfcValue::Integer { value: 3 },
            references(points),
            enumeration("UNSPECIFIED"),
            IfcValue::Bool { value: false },
            IfcValue::Bool { value: false },
            IfcValue::List {
                values: multiplicities,
            },
            reals(&fitted.knots),
            enumeration("UNSPECIFIED"),
        ],
    ))
}
