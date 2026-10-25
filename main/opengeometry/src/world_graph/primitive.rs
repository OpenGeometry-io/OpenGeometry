use super::candidate::IdKind;
use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, GraphError};
use super::graph::WorldGraph;
use super::node::EditScope;
use crate::brep::{
    Accuracy, BodyType, BrepEnvelope, CurveGeometry, Frame3, GeometryError, Similarity3, GROUND,
};
use crate::math::{norm, sub, Point3};
use crate::primitives::{self, ProfileEdge, ProfileSegment};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Primitive {
    Cuboid {
        width: f64,
        height: f64,
        depth: f64,
    },
    Cylinder {
        radius: f64,
        height: f64,
    },
    #[serde(rename_all = "camelCase")]
    CylinderSector {
        radius: f64,
        height: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
    Polyhedron {
        points: Vec<Point3>,
        faces: Vec<Vec<u32>>,
    },
    Rectangle {
        width: f64,
        breadth: f64,
    },
    Circle {
        radius: f64,
    },
    Polyline {
        points: Vec<Point3>,
        closed: bool,
    },
    Profile {
        start: [f64; 2],
        segments: Vec<ProfileSegment>,
        closed: bool,
    },
    #[serde(rename_all = "camelCase")]
    Arc {
        radius: f64,
        start_angle: f64,
        sweep_angle: f64,
    },
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plane {
    pub origin: Option<Point3>,
    pub normal: Option<Point3>,
    pub x_direction: Option<Point3>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateOptions {
    pub og_id: Option<String>,
    pub parent: Option<String>,
    pub plane: Option<Plane>,
    pub body_type: Option<BodyType>,
}

impl Primitive {
    fn body_type(&self) -> BodyType {
        match self {
            Self::Cuboid { .. }
            | Self::Cylinder { .. }
            | Self::CylinderSector { .. }
            | Self::Polyhedron { .. } => BodyType::Solid,
            Self::Rectangle { .. }
            | Self::Circle { .. }
            | Self::Polyline { .. }
            | Self::Profile { .. }
            | Self::Arc { .. } => BodyType::Wire,
        }
    }
}

impl WorldGraph {
    pub fn create_primitive(
        &mut self,
        primitive: Primitive,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        self.try_create_primitive(primitive, options)
            .map_err(|error| error.in_context(ErrorContext::Create))
    }

    fn try_create_primitive(
        &mut self,
        primitive: Primitive,
        options: CreateOptions,
    ) -> Result<(String, ChangeSet), GraphError> {
        let body_type = primitive.body_type();
        check_body_type(options.body_type, body_type)?;
        let kind = IdKind::of_body(body_type)?;
        let accuracy = self.accuracy;
        let local = initial_placement(options.plane)?;
        let reserved = self.reserve(1, &[])?;
        let mut created = None;
        let changes = self.mutate(|draft| {
            if let Some(parent) = &options.parent {
                if !draft.nodes.contains_key(parent) {
                    return Err(GraphError::code(ErrorCode::UnknownNode, parent));
                }
            }
            let id = draft.new_og_id(options.og_id.as_deref(), kind)?;
            let shape_id = draft.new_shape_id()?;
            if reserved.shape_ids.first() != Some(&shape_id) {
                return Err(GraphError::code(
                    ErrorCode::RevisionConflict,
                    "reserved shape id changed",
                ));
            }
            let (brep, edge_keys) = build_primitive(shape_id.clone(), primitive, accuracy)?;
            if brep.body_type()? != body_type {
                return Err(GraphError::code(
                    ErrorCode::BodyTypeMismatch,
                    "primitive produced the wrong body type",
                ));
            }
            draft.add_body(&id, shape_id, brep, edge_keys, &options.parent, local)?;
            created = Some(id);
            Ok(())
        })?;
        Ok((
            created.ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "created node is missing")
            })?,
            changes,
        ))
    }

    pub fn rebuild_primitive(
        &mut self,
        og_id: &str,
        primitive: Primitive,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        self.try_rebuild_primitive(og_id, primitive, scope)
            .map_err(|error| error.in_context(ErrorContext::Rebuild))
    }

    fn try_rebuild_primitive(
        &mut self,
        og_id: &str,
        primitive: Primitive,
        scope: EditScope,
    ) -> Result<ChangeSet, GraphError> {
        let previous = self.ensure_editable(og_id, scope)?;
        let expected_type = previous.brep.body_type()?;
        let previous_revision = previous.revision;
        let shape_id = self
            .node(og_id)?
            .shape
            .clone()
            .ok_or_else(|| GraphError::code(ErrorCode::InvalidOperand, "node is not a body"))?;
        let reserved = self.reserve(0, std::slice::from_ref(&shape_id))?;
        let next_revision = reserved.revisions[0].1;
        let accuracy = self.accuracy;
        let actual_type = primitive.body_type();
        self.mutate(|candidate| {
            let (mut brep, edge_keys) = build_primitive(shape_id.clone(), primitive, accuracy)?;
            if actual_type != expected_type {
                return Err(GraphError::code(
                    ErrorCode::BodyTypeMismatch,
                    "rebuild changed body type",
                ));
            }
            brep.revision = next_revision;
            brep.validate()?;
            let shape = candidate.shapes.get_mut(&shape_id).ok_or_else(|| {
                GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
            })?;
            if shape.revision != previous_revision {
                return Err(GraphError::code(
                    ErrorCode::RevisionConflict,
                    "shape revision changed before commit",
                ));
            }
            shape.brep = Arc::new(brep);
            shape.revision = next_revision;
            shape.edge_keys = edge_keys;
            shape.report = None;
            Ok(())
        })
    }
}

pub(super) fn check_body_type(
    expected: Option<BodyType>,
    actual: BodyType,
) -> Result<(), GraphError> {
    match expected {
        Some(expected) if expected != actual => Err(GraphError::code(
            ErrorCode::BodyTypeMismatch,
            format!("expected {expected:?}, got {actual:?}"),
        )),
        _ => Ok(()),
    }
}

pub(super) fn check_no_plane(plane: Option<Plane>) -> Result<(), GraphError> {
    if plane.is_some() {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "plane applies only to primitives",
        ));
    }
    Ok(())
}

fn initial_placement(plane: Option<Plane>) -> Result<Similarity3, GraphError> {
    let Some(plane) = plane else {
        return Ok(Similarity3::IDENTITY);
    };
    let origin = plane.origin.unwrap_or([0.0; 3]);
    let normal = plane.normal.unwrap_or([0.0, 1.0, 0.0]);
    let x_direction = plane.x_direction.unwrap_or([1.0, 0.0, 0.0]);
    let frame = Frame3::from_axis(origin, normal, x_direction).map_err(|error| {
        GraphError::code(
            ErrorCode::InvalidParameter,
            format!("invalid primitive plane: {error}"),
        )
    })?;
    let placement = Similarity3 { frame, scale: 1.0 }.compose(
        &Similarity3 {
            frame: GROUND,
            scale: 1.0,
        }
        .inverse(),
    );
    placement.validate().map_err(|error| {
        GraphError::code(
            ErrorCode::InvalidTransform,
            format!("invalid primitive plane placement: {error}"),
        )
    })?;
    Ok(placement)
}

fn build_primitive(
    shape_id: String,
    primitive: Primitive,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let result = match primitive {
        Primitive::Cuboid {
            width,
            height,
            depth,
        } => cuboid_body(shape_id.clone(), [width, height, depth], accuracy)?,
        Primitive::Cylinder { radius, height } => {
            required_size(radius, accuracy.geometric, "radius")?;
            required_size(height, accuracy.geometric, "height")?;
            (
                primitives::cylinder(shape_id.clone(), GROUND, radius, height, accuracy)?,
                Vec::new(),
            )
        }
        Primitive::CylinderSector {
            radius,
            height,
            start_angle,
            sweep_angle,
        } => sector_body(
            shape_id.clone(),
            [radius, height],
            [start_angle, sweep_angle],
            accuracy,
        )?,
        Primitive::Polyhedron { points, faces } => {
            polyhedron_body(shape_id.clone(), &points, &faces, accuracy)?
        }
        Primitive::Rectangle { width, breadth } => {
            required_size(width, accuracy.geometric, "width")?;
            required_size(breadth, accuracy.geometric, "breadth")?;
            let (brep, keys) = primitives::rectangle_with_keys(
                shape_id.clone(),
                GROUND,
                width,
                breadth,
                accuracy,
            )?;
            (brep, keys)
        }
        Primitive::Circle { radius } => {
            required_size(radius, accuracy.geometric, "radius")?;
            arc_body(shape_id.clone(), radius, [0.0, TAU], accuracy)?
        }
        Primitive::Arc {
            radius,
            start_angle,
            sweep_angle,
        } => {
            check_arc("arc", radius, [start_angle, sweep_angle], accuracy)?;
            arc_body(
                shape_id.clone(),
                radius,
                [start_angle, sweep_angle],
                accuracy,
            )?
        }
        Primitive::Polyline { points, closed } => {
            check_polyline(&points, closed, accuracy)?;
            let (brep, keys) =
                primitives::polyline_with_keys(shape_id.clone(), &points, closed, accuracy)?;
            (brep, keys)
        }
        Primitive::Profile {
            start,
            segments,
            closed,
        } => profile_body(shape_id.clone(), start, &segments, closed, accuracy)?,
    };
    Ok(result)
}

fn profile_body(
    shape_id: String,
    start: [f64; 2],
    segments: &[ProfileSegment],
    closed: bool,
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let edges = primitives::profile_segment_edges(start, segments, closed);
    check_profile(start, segments, &edges, closed, accuracy.geometric)?;
    Ok(primitives::profile_wire_with_keys(
        shape_id, start, segments, closed, accuracy,
    )?)
}

fn cuboid_body(
    shape_id: String,
    [width, height, depth]: [f64; 3],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    for (name, value) in [("width", width), ("height", height), ("depth", depth)] {
        required_size(value, accuracy.geometric, name)?;
    }
    let frame = Frame3 {
        origin: [-width / 2.0, 0.0, depth / 2.0],
        ..GROUND
    };
    Ok((
        primitives::cuboid(shape_id, frame, [width, depth, height], accuracy)?,
        Vec::new(),
    ))
}

fn polyhedron_body(
    shape_id: String,
    points: &[Point3],
    faces: &[Vec<u32>],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let brep =
        primitives::polyhedron(shape_id, points, faces, accuracy).map_err(|error| match error {
            GeometryError::InvalidGeometry(message)
            | GeometryError::UnresolvedIntersection(message) => {
                GraphError::code(ErrorCode::InvalidParameter, message)
            }
            GeometryError::Math(error) => GraphError::code(
                ErrorCode::InvalidParameter,
                format!("polyhedron points are out of range: {error:?}"),
            ),
            other => other.into(),
        })?;
    Ok((brep, Vec::new()))
}

fn sector_body(
    shape_id: String,
    [radius, height]: [f64; 2],
    angles: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    check_arc("sector", radius, angles, accuracy)?;
    required_size(height, accuracy.geometric, "height")?;
    let [start_angle, sweep_angle] = angles;
    Ok((
        primitives::cylinder_sector(
            shape_id,
            GROUND,
            radius,
            height,
            start_angle,
            sweep_angle,
            accuracy,
        )?,
        Vec::new(),
    ))
}

fn arc_body(
    shape_id: String,
    radius: f64,
    [start_angle, sweep_angle]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(BrepEnvelope, Vec<String>), GraphError> {
    let curve = CurveGeometry::Circle {
        frame: GROUND,
        radius,
    };
    Ok((
        primitives::arc_wire(shape_id, curve, start_angle, sweep_angle, accuracy)?,
        vec!["edge-0".into()],
    ))
}

fn required_size(value: f64, accuracy: f64, name: &str) -> Result<(), GraphError> {
    if !value.is_finite() || value <= 4.0 * accuracy {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            format!("{name} must exceed four times geometric tolerance"),
        ));
    }
    Ok(())
}

fn check_polyline(points: &[Point3], closed: bool, accuracy: Accuracy) -> Result<(), GraphError> {
    if points.len() < if closed { 3 } else { 2 } {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "polyline has too few points",
        ));
    }
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return Err(GraphError::code(
            ErrorCode::InvalidParameter,
            "polyline points must be finite",
        ));
    }
    for index in 0..if closed {
        points.len()
    } else {
        points.len() - 1
    } {
        let next = (index + 1) % points.len();
        if norm(sub(points[next], points[index])) <= 4.0 * accuracy.geometric {
            return Err(GraphError::code(
                ErrorCode::InvalidParameter,
                "polyline has a zero-length segment",
            ));
        }
    }
    Ok(())
}

fn check_arc(
    kind: &str,
    radius: f64,
    [start, sweep]: [f64; 2],
    accuracy: Accuracy,
) -> Result<(), GraphError> {
    required_size(radius, accuracy.geometric, "radius")?;
    if !start.is_finite() || !sweep.is_finite() || sweep == 0.0 || sweep.abs() >= TAU {
        return Err(parameter(&format!(
            "{kind} sweep must be nonzero and less than one turn"
        )));
    }
    if radius * sweep.abs() <= 4.0 * accuracy.geometric {
        return Err(parameter(&format!(
            "{kind} length must exceed four times geometric tolerance"
        )));
    }
    Ok(())
}

fn check_profile(
    start: [f64; 2],
    segments: &[ProfileSegment],
    edges: &[ProfileEdge],
    closed: bool,
    g: f64,
) -> Result<(), GraphError> {
    if segments.len() < if closed { 2 } else { 1 } {
        return Err(parameter("profile has too few segments"));
    }
    let values = segments.iter().flat_map(|segment| {
        segment
            .to
            .into_iter()
            .chain(segment.arc.into_iter().flat_map(|arc| arc.center))
    });
    if !start.into_iter().chain(values).all(f64::is_finite) {
        return Err(parameter("profile points must be finite"));
    }
    let last = segments[segments.len() - 1].to;
    if closed != ((last[0] - start[0]).hypot(last[1] - start[1]) <= 4.0 * g) {
        return Err(parameter(if closed {
            "closed profile must end at its start"
        } else {
            "open profile must not end at its start"
        }));
    }
    for (index, (edge, segment)) in edges.iter().zip(segments).enumerate() {
        let to = if closed && index + 1 == segments.len() {
            start
        } else {
            segment.to
        };
        check_profile_edge(edge, to, g)?;
    }
    if primitives::profile_self_intersects(edges, closed, g) {
        return Err(parameter("profile self-intersects"));
    }
    if closed && primitives::profile_area_twice(edges).abs() <= 16.0 * g * g {
        return Err(parameter("profile area is below geometric resolution"));
    }
    Ok(())
}

fn check_profile_edge(edge: &ProfileEdge, to: [f64; 2], g: f64) -> Result<(), GraphError> {
    let (from, _) = edge.endpoints();
    if (to[0] - from[0]).hypot(to[1] - from[1]) <= 4.0 * g {
        return Err(parameter("profile has a zero-length segment"));
    }
    if let ProfileEdge::Arc {
        center,
        radius,
        sweep_angle,
        ..
    } = *edge
    {
        if radius <= 4.0 * g
            || ((to[0] - center[0]).hypot(to[1] - center[1]) - radius).abs() > g / 4.0
        {
            return Err(parameter("profile arc does not end on its circle"));
        }
        if sweep_angle.abs() >= TAU || radius * sweep_angle.abs() <= 4.0 * g {
            return Err(parameter("profile arc is below geometric resolution"));
        }
    }
    Ok(())
}

fn parameter(message: &str) -> GraphError {
    GraphError::code(ErrorCode::InvalidParameter, message)
}
