use super::{
    exchange::preflight,
    export_curve::fit_intersection_curve,
    geometry::{norm, scale},
    placement::placed,
    topology::{EdgeGeometry, FaceProvenance, GeometryQuality, Orientation},
    BrepEnvelope, CurveGeometry, Frame3, GeometryError, SurfaceGeometry,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum IfcValue {
    Ref { id: u32 },
    List { values: Vec<IfcValue> },
    Real { value: f64 },
    Integer { value: i64 },
    Text { value: String },
    Enum { value: String },
    Bool { value: bool },
    Null,
    Derived,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IfcEntity {
    pub id: u32,
    pub entity_type: String,
    pub attributes: Vec<IfcValue>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeFace {
    pub face: u32,
    pub key: String,
    pub entity: u32,
    pub provenance: FaceProvenance,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyticExchangeBodyV2 {
    pub schema_version: u32,
    pub revision: String,
    pub length_unit: String,
    pub up_axis: String,
    pub coordinate_space: String,
    pub representation_type: String,
    pub quality: GeometryQuality,
    pub geometric_tolerance: f64,
    pub exchange_error_bound: f64,
    pub validation_level: String,
    pub entities: Vec<IfcEntity>,
    pub solids: Vec<u32>,
    pub faces: Vec<ExchangeFace>,
}

struct Entities(Vec<IfcEntity>);
impl Entities {
    fn add(&mut self, entity_type: &str, attributes: Vec<IfcValue>) -> u32 {
        let id = self.0.len() as u32 + 1;
        self.0.push(IfcEntity {
            id,
            entity_type: entity_type.into(),
            attributes,
        });
        id
    }
    fn point(&mut self, p: &[f64]) -> u32 {
        self.add(
            "IfcCartesianPoint",
            vec![IfcValue::List {
                values: p
                    .iter()
                    .map(|value| IfcValue::Real { value: *value })
                    .collect(),
            }],
        )
    }
    fn direction(&mut self, p: &[f64]) -> u32 {
        self.add(
            "IfcDirection",
            vec![IfcValue::List {
                values: p
                    .iter()
                    .map(|value| IfcValue::Real { value: *value })
                    .collect(),
            }],
        )
    }
    fn placement(&mut self, frame: Frame3) -> u32 {
        let origin = self.point(&frame.origin);
        let z = self.direction(&frame.z);
        let x = self.direction(&frame.x);
        self.add(
            "IfcAxis2Placement3D",
            vec![reference(origin), reference(z), reference(x)],
        )
    }
    fn curve(&mut self, geometry: &CurveGeometry) -> Result<u32, GeometryError> {
        Ok(match geometry {
            CurveGeometry::Line { origin, direction } => {
                let p = self.point(origin);
                let d = self.direction(direction);
                let v = self.add(
                    "IfcVector",
                    vec![
                        reference(d),
                        IfcValue::Real {
                            value: norm(*direction),
                        },
                    ],
                );
                self.add("IfcLine", vec![reference(p), reference(v)])
            }
            CurveGeometry::Circle { frame, radius } => {
                let position = self.placement(*frame);
                self.add(
                    "IfcCircle",
                    vec![reference(position), IfcValue::Real { value: *radius }],
                )
            }
            CurveGeometry::Ellipse {
                frame,
                major_radius,
                minor_radius,
            } => {
                let position = self.placement(*frame);
                self.add(
                    "IfcEllipse",
                    vec![
                        reference(position),
                        IfcValue::Real {
                            value: *major_radius,
                        },
                        IfcValue::Real {
                            value: *minor_radius,
                        },
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

    fn fitted_curve(
        &mut self,
        controls: &[super::Point3],
        knots: &[f64],
        multiplicities: &[usize],
    ) -> Result<u32, GeometryError> {
        if controls.len() < 4 || knots.len() != multiplicities.len() {
            return Err(GeometryError::InvalidGeometry(
                "invalid IFC fitted cubic curve shape".into(),
            ));
        }
        let points = controls
            .iter()
            .map(|point| self.point(point))
            .collect::<Vec<_>>();
        let multiplicities = multiplicities
            .iter()
            .map(|value| {
                i64::try_from(*value)
                    .map(|value| IfcValue::Integer { value })
                    .map_err(|_| GeometryError::LimitExceeded("IFC knot multiplicity".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.add(
            "IfcBSplineCurveWithKnots",
            vec![
                IfcValue::Integer { value: 3 },
                references(points),
                IfcValue::Enum {
                    value: "UNSPECIFIED".into(),
                },
                IfcValue::Bool { value: false },
                IfcValue::Bool { value: false },
                IfcValue::List {
                    values: multiplicities,
                },
                IfcValue::List {
                    values: knots
                        .iter()
                        .map(|value| IfcValue::Real { value: *value })
                        .collect(),
                },
                IfcValue::Enum {
                    value: "UNSPECIFIED".into(),
                },
            ],
        ))
    }
}
fn reference(id: u32) -> IfcValue {
    IfcValue::Ref { id }
}
fn references(ids: impl IntoIterator<Item = u32>) -> IfcValue {
    IfcValue::List {
        values: ids.into_iter().map(reference).collect(),
    }
}
fn boolean(sense: Orientation) -> IfcValue {
    IfcValue::Bool {
        value: sense == Orientation::Forward,
    }
}

/// Rust prepares geometry in IFC world metres and Z-up; the receiver only assembles entity definitions.
pub fn prepare_ifc_body(input: &BrepEnvelope) -> Result<AnalyticExchangeBodyV2, GeometryError> {
    let mut exchange_error_bound = preflight(input, 1.0)?;
    let fitting_budget = input.accuracy.exchange - exchange_error_bound;
    let estimated_entities = input.geometry.surfaces.len() * 20
        + input.geometry.curves.len() * 5
        + input.topology.vertices.len() * 2
        + input.topology.edges.len()
        + input.topology.halfedges.len()
        + input.topology.loops.len() * 2
        + input.topology.faces.len()
        + input.topology.shells.len()
        + input.solids.len() * 2
        + input
            .solids
            .iter()
            .map(|s| s.cavity_shells.len() * 2)
            .sum::<usize>();
    if estimated_entities > 100_000 {
        return Err(GeometryError::LimitExceeded(
            "IFC prepared entity limit exceeded".into(),
        ));
    }
    let conversion = Frame3 {
        origin: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [0.0, -1.0, 0.0],
    };
    let mut brep = placed(input, conversion, 1.0)?;
    let has_cavities = brep
        .solids
        .iter()
        .any(|solid| !solid.cavity_shells.is_empty());
    // IFC4's published VoidsHaveAdvancedFaces rule has an inverted inner test; emit valid CSG operands instead.
    if has_cavities {
        for shell in brep.solids.iter().flat_map(|solid| &solid.cavity_shells) {
            for face in &brep.topology.shells[*shell as usize].faces {
                let face_record = &mut brep.topology.faces[*face as usize];
                face_record.sense = if face_record.sense == Orientation::Forward {
                    Orientation::Reverse
                } else {
                    Orientation::Forward
                };
                for use_ in brep
                    .topology
                    .halfedges
                    .iter_mut()
                    .filter(|use_| use_.face == Some(*face))
                {
                    std::mem::swap(&mut use_.from, &mut use_.to);
                    std::mem::swap(&mut use_.next, &mut use_.prev);
                    use_.geometry_use.sense = if use_.geometry_use.sense == Orientation::Forward {
                        Orientation::Reverse
                    } else {
                        Orientation::Forward
                    };
                }
            }
        }
        for vertex in &mut brep.topology.vertices {
            vertex.outgoing_halfedge = None;
        }
        for use_ in &brep.topology.halfedges {
            brep.topology.vertices[use_.from as usize]
                .outgoing_halfedge
                .get_or_insert(use_.id);
        }
        brep.validate()?;
    }
    let mut entities = Entities(Vec::new());
    let mut surfaces = vec![None; brep.geometry.surfaces.len()];
    for face in &brep.topology.faces {
        if surfaces[face.surface as usize].is_some() {
            continue;
        }
        let surface = brep.geometry.surface(face.surface)?;
        let position = entities.placement(*surface.frame());
        let id = match surface {
            SurfaceGeometry::Plane { .. } => entities.add("IfcPlane", vec![reference(position)]),
            SurfaceGeometry::Sphere { radius, .. } => entities.add(
                "IfcSphericalSurface",
                vec![reference(position), IfcValue::Real { value: *radius }],
            ),
            SurfaceGeometry::Cylinder { radius, .. } => entities.add(
                "IfcCylindricalSurface",
                vec![reference(position), IfcValue::Real { value: *radius }],
            ),
            SurfaceGeometry::Torus {
                major_radius,
                minor_radius,
                ..
            } => entities.add(
                "IfcToroidalSurface",
                vec![
                    reference(position),
                    IfcValue::Real {
                        value: *major_radius,
                    },
                    IfcValue::Real {
                        value: *minor_radius,
                    },
                ],
            ),
            SurfaceGeometry::Cone { frame, semi_angle } => {
                let height = brep
                    .topology
                    .faces
                    .iter()
                    .filter(|f| f.surface == face.surface)
                    .map(|f| f.trim.uv_bounds[1].hi)
                    .fold(0.0_f64, f64::max);
                if !height.is_finite() || height <= 0.0 {
                    return Err(GeometryError::InvalidGeometry(
                        "IFC cone profile requires a positive finite height".into(),
                    ));
                }
                let p0 = entities.point(&[0.0, 0.0]);
                let p1 = entities.point(&[height * semi_angle.tan(), height]);
                let line = entities.add("IfcPolyline", vec![references([p0, p1])]);
                let profile = entities.add(
                    "IfcArbitraryOpenProfileDef",
                    vec![
                        IfcValue::Enum {
                            value: "CURVE".into(),
                        },
                        IfcValue::Null,
                        reference(line),
                    ],
                );
                let profile_frame = Frame3 {
                    y: frame.z,
                    z: scale(frame.y, -1.0),
                    ..*frame
                };
                let profile_position = entities.placement(profile_frame);
                let origin = entities.point(&[0.0; 3]);
                let axis = entities.direction(&[0.0, 1.0, 0.0]);
                let axis = entities.add(
                    "IfcAxis1Placement",
                    vec![reference(origin), reference(axis)],
                );
                entities.add(
                    "IfcSurfaceOfRevolution",
                    vec![
                        reference(profile),
                        reference(profile_position),
                        reference(axis),
                    ],
                )
            }
        };
        surfaces[face.surface as usize] = Some(id);
    }
    let vertices: Vec<_> = brep
        .topology
        .vertices
        .iter()
        .map(|v| {
            let p = entities.point(&v.position);
            entities.add("IfcVertexPoint", vec![reference(p)])
        })
        .collect();
    let mut edges = vec![None; brep.topology.edges.len()];
    let mut curves = vec![None; brep.geometry.curves.len()];
    let mut fitting_error: f64 = 0.0;
    for edge in &brep.topology.edges {
        let EdgeGeometry::Curve { curve, range } = edge.geometry else {
            continue;
        };
        let geometry = &brep.geometry.curves[curve as usize];
        if matches!(
            geometry,
            CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. }
        ) && range.width() > std::f64::consts::TAU
        {
            return Err(GeometryError::UnsupportedGeometry(
                "IFC conic edges cannot exceed one period".into(),
            ));
        }
        let curve_id = match curves[curve as usize] {
            Some(id) => id,
            None => {
                let id = match geometry {
                    CurveGeometry::Intersection { definition } => {
                        if fitting_budget <= 0.0 {
                            return Err(GeometryError::LimitExceeded(
                                "IFC exchange budget leaves no room for numerical curve fitting"
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
                        entities.fitted_curve(
                            &fitted.controls,
                            &fitted.knots,
                            &fitted.multiplicities,
                        )?
                    }
                    _ => entities.curve(geometry)?,
                };
                curves[curve as usize] = Some(id);
                id
            }
        };
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
    exchange_error_bound += fitting_error;
    if exchange_error_bound > input.accuracy.exchange {
        return Err(GeometryError::LimitExceeded(
            "IFC fitted curves exceed the exchange error budget".into(),
        ));
    }
    let mut faces = Vec::new();
    for face in &brep.topology.faces {
        let mut bounds = Vec::new();
        for loop_id in std::iter::once(&face.trim.outer).chain(&face.trim.holes) {
            let loop_ = &brep.topology.loops[*loop_id as usize];
            let start = loop_.start_halfedge;
            let mut current = start;
            let mut uses = Vec::new();
            for _ in 0..=brep.topology.halfedges.len() {
                let use_ = &brep.topology.halfedges[current as usize];
                if let Some(edge) = edges[use_.edge as usize] {
                    uses.push(entities.add(
                        "IfcOrientedEdge",
                        vec![
                            IfcValue::Derived,
                            IfcValue::Derived,
                            reference(edge),
                            boolean(use_.geometry_use.sense),
                        ],
                    ));
                }
                current = use_
                    .next
                    .ok_or_else(|| GeometryError::InvalidTopology("open IFC loop".into()))?;
                if current == start {
                    break;
                }
            }
            let loop_entity = if uses.is_empty() {
                entities.add(
                    "IfcVertexLoop",
                    vec![reference(
                        vertices[brep.topology.halfedges[start as usize].from as usize],
                    )],
                )
            } else {
                entities.add("IfcEdgeLoop", vec![references(uses)])
            };
            bounds.push(entities.add(
                if loop_.is_hole {
                    "IfcFaceBound"
                } else {
                    "IfcFaceOuterBound"
                },
                vec![reference(loop_entity), IfcValue::Bool { value: true }],
            ));
        }
        let surface = surfaces[face.surface as usize]
            .ok_or_else(|| GeometryError::InvalidTopology("missing prepared IFC surface".into()))?;
        faces.push(entities.add(
            "IfcAdvancedFace",
            vec![references(bounds), reference(surface), boolean(face.sense)],
        ));
    }
    let shells: Vec<_> = brep
        .topology
        .shells
        .iter()
        .map(|s| {
            entities.add(
                "IfcClosedShell",
                vec![references(s.faces.iter().map(|id| faces[*id as usize]))],
            )
        })
        .collect();
    let mut solids: Vec<_> = brep
        .solids
        .iter()
        .map(|solid| {
            let mut root = entities.add(
                "IfcAdvancedBrep",
                vec![reference(shells[solid.outer_shell as usize])],
            );
            for shell in &solid.cavity_shells {
                let cutter =
                    entities.add("IfcAdvancedBrep", vec![reference(shells[*shell as usize])]);
                root = entities.add(
                    "IfcBooleanResult",
                    vec![
                        IfcValue::Enum {
                            value: "DIFFERENCE".into(),
                        },
                        reference(root),
                        reference(cutter),
                    ],
                );
            }
            root
        })
        .collect();
    if has_cavities && solids.len() > 1 {
        let mut root = solids[0];
        for id in &solids[1..] {
            root = entities.add(
                "IfcBooleanResult",
                vec![
                    IfcValue::Enum {
                        value: "UNION".into(),
                    },
                    reference(root),
                    reference(*id),
                ],
            );
        }
        solids = vec![root];
    }
    let faces = brep
        .topology
        .faces
        .iter()
        .map(|face| ExchangeFace {
            face: face.id,
            key: face.key.clone(),
            entity: faces[face.id as usize],
            provenance: face.provenance.clone(),
        })
        .collect();
    Ok(AnalyticExchangeBodyV2 {
        schema_version: 2, revision: input.revision.to_string(), length_unit: "metre".into(), up_axis: "Z".into(), coordinate_space: "world".into(),
        representation_type: if has_cavities { "CSG" } else { "AdvancedBrep" }.into(),
        quality: input.quality.clone(), geometric_tolerance: input.accuracy.geometric, exchange_error_bound,
        validation_level: "v2 structure, sampled residuals and bounded analytic entity preparation; receiver schema validation required".into(),
        entities: entities.0, solids, faces,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytic::{primitives, topology::Accuracy};
    #[test]
    fn prepared_ifc_has_shared_geometry_backreferences_and_single_axis_conversion() {
        let accuracy = Accuracy {
            geometric: 1e-8,
            intersection: 1e-9,
            tessellation: 0.01,
            exchange: 1e-6,
        };
        let frame = Frame3 {
            origin: [1.0, 2.0, 3.0],
            y: [0.0, 0.0, -1.0],
            z: [0.0, 1.0, 0.0],
            ..Frame3::IDENTITY
        };
        let bodies = [
            primitives::cylinder("c".into(), frame, 1.0, 2.0, accuracy).unwrap(),
            primitives::sphere("s".into(), frame, 1.0, accuracy).unwrap(),
            primitives::cone("cone".into(), frame, 1.0, 2.0, accuracy).unwrap(),
            primitives::torus("t".into(), frame, 2.0, 0.5, accuracy).unwrap(),
        ];
        fn validate(value: &IfcValue, before: u32) {
            match value {
                IfcValue::Ref { id } => assert!(*id > 0 && *id < before),
                IfcValue::List { values } => {
                    for value in values {
                        validate(value, before);
                    }
                }
                _ => {}
            }
        }
        for input in bodies {
            let original = input.to_json().unwrap();
            let output = prepare_ifc_body(&input).unwrap();
            assert_eq!(output.faces.len(), input.topology.faces.len());
            assert_eq!(output.solids.len(), 1);
            assert_eq!(output.up_axis, "Z");
            assert_eq!(output.length_unit, "metre");
            for entity in &output.entities {
                for value in &entity.attributes {
                    validate(value, entity.id);
                }
            }
            assert!(output.entities.iter().any(|e| e.entity_type == "IfcCartesianPoint" && matches!(&e.attributes[0], IfcValue::List { values } if matches!(values.as_slice(), [IfcValue::Real { value: 1.0 }, IfcValue::Real { value: -3.0 }, IfcValue::Real { value: 2.0 }]))));
            assert!(!output
                .entities
                .iter()
                .any(|e| e.entity_type == "IfcSurfaceCurve"));
            assert_eq!(input.to_json().unwrap(), original);
            let serialized = serde_json::to_string(&output).unwrap();
            serde_json::from_str::<AnalyticExchangeBodyV2>(&serialized).unwrap();
        }
    }
}
