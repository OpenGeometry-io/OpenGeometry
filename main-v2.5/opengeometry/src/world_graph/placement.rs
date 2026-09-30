use super::change_log::ChangeSet;
use super::error::{ErrorCode, ErrorContext, GraphError};
use super::graph::WorldGraph;
use super::node::PlaceInput;
use crate::brep::{Frame3, Similarity3, GROUND};
use crate::math::{add, scale, sub, Point3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub origin: Point3,
    pub x_direction: Point3,
    pub normal: Point3,
    pub scale: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Transform {
    Translate {
        offset: Point3,
    },
    Rotate {
        axis: Point3,
        degrees: f64,
        pivot: Option<Point3>,
    },
    Scale {
        factor: f64,
        pivot: Option<Point3>,
    },
    Place {
        origin: Option<Point3>,
        x_direction: Option<Point3>,
        normal: Option<Point3>,
        scale: Option<f64>,
    },
}

pub(super) fn is_identity(transform: Similarity3) -> bool {
    same_bits(transform, Similarity3::IDENTITY)
}

fn same_bits(a: Similarity3, b: Similarity3) -> bool {
    entries(a)
        .iter()
        .zip(entries(b))
        .all(|(a, b)| a.to_bits() == b.to_bits())
}

fn entries(transform: Similarity3) -> [f64; 13] {
    [
        transform.frame.origin[0],
        transform.frame.origin[1],
        transform.frame.origin[2],
        transform.frame.x[0],
        transform.frame.x[1],
        transform.frame.x[2],
        transform.frame.y[0],
        transform.frame.y[1],
        transform.frame.y[2],
        transform.frame.z[0],
        transform.frame.z[1],
        transform.frame.z[2],
        transform.scale,
    ]
}

impl Transform {
    fn apply(self, local: Similarity3) -> Result<Similarity3, GraphError> {
        local
            .validate()
            .map_err(|error| invalid_transform(format!("invalid current placement: {error}")))?;
        let result = match self {
            Self::Translate { offset } => translated_local(local, offset)?,
            Self::Rotate {
                axis,
                degrees,
                pivot,
            } => rotated_local(local, axis, degrees, pivot)?,
            Self::Scale { factor, pivot } => scaled_local(local, factor, pivot)?,
            Self::Place {
                origin,
                x_direction,
                normal,
                scale,
            } => from_place(place_input(origin, x_direction, normal, scale))?,
        };
        result
            .validate()
            .map_err(|error| invalid_transform(format!("invalid transform result: {error}")))?;
        Ok(result)
    }
}

fn invalid_transform(message: impl Into<String>) -> GraphError {
    GraphError::code(ErrorCode::InvalidTransform, message)
}

fn translated_local(local: Similarity3, offset: Point3) -> Result<Similarity3, GraphError> {
    if offset.iter().any(|value| !value.is_finite()) {
        return Err(invalid_transform("translation must be finite"));
    }
    Ok(Similarity3 {
        frame: Frame3 {
            origin: add(local.frame.origin, offset),
            ..local.frame
        },
        ..local
    })
}

fn rotated_local(
    local: Similarity3,
    axis: Point3,
    degrees: f64,
    pivot: Option<Point3>,
) -> Result<Similarity3, GraphError> {
    if !degrees.is_finite() {
        return Err(invalid_transform("rotation must be finite"));
    }
    let pivot = pivot.unwrap_or(local.frame.origin);
    if pivot.iter().any(|value| !value.is_finite()) {
        return Err(invalid_transform("pivot must be finite"));
    }
    let reduced = degrees % 360.0;
    let rotation = match quarter_turn(reduced) {
        Some((cos, sin)) => Similarity3::from_axis_cos_sin([0.0; 3], axis, cos, sin, 1.0),
        None => Similarity3::from_axis_angle([0.0; 3], axis, reduced.to_radians(), 1.0),
    }
    .map_err(|error| invalid_transform(format!("invalid rotation: {error}")))?;
    let origin = sub(pivot, rotation.apply_vector(pivot));
    Ok(Similarity3 {
        frame: Frame3 {
            origin,
            ..rotation.frame
        },
        ..rotation
    }
    .compose(&local))
}

fn quarter_turn(degrees: f64) -> Option<(f64, f64)> {
    [
        (0.0, (1.0, 0.0)),
        (90.0, (0.0, 1.0)),
        (180.0, (-1.0, 0.0)),
        (270.0, (0.0, -1.0)),
        (-90.0, (0.0, -1.0)),
        (-180.0, (-1.0, 0.0)),
        (-270.0, (0.0, 1.0)),
    ]
    .into_iter()
    .find(|(turn, _)| *turn == degrees)
    .map(|(_, cos_sin)| cos_sin)
}

fn scaled_local(
    local: Similarity3,
    factor: f64,
    pivot: Option<Point3>,
) -> Result<Similarity3, GraphError> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err(invalid_transform("scale must be finite and positive"));
    }
    let pivot = pivot.unwrap_or(local.frame.origin);
    if pivot.iter().any(|value| !value.is_finite()) {
        return Err(invalid_transform("pivot must be finite"));
    }
    Ok(Similarity3 {
        frame: Frame3 {
            origin: add(pivot, scale(sub(local.frame.origin, pivot), factor)),
            ..local.frame
        },
        scale: local.scale * factor,
    })
}

fn place_input(
    origin: Option<Point3>,
    x_direction: Option<Point3>,
    normal: Option<Point3>,
    scale: Option<f64>,
) -> PlaceInput {
    PlaceInput {
        origin: origin.unwrap_or([0.0; 3]),
        x_direction: x_direction.unwrap_or([1.0, 0.0, 0.0]),
        normal: normal.unwrap_or([0.0, 1.0, 0.0]),
        scale: scale.unwrap_or(1.0),
    }
}

fn from_place(input: PlaceInput) -> Result<Similarity3, GraphError> {
    let frame = Frame3::from_axis(input.origin, input.normal, input.x_direction)
        .map_err(|error| invalid_transform(format!("invalid placement frame: {error}")))?;
    let local = Similarity3 {
        frame,
        scale: input.scale,
    }
    .compose(
        &Similarity3 {
            frame: GROUND,
            scale: 1.0,
        }
        .inverse(),
    );
    local
        .validate()
        .map_err(|error| invalid_transform(format!("invalid placement: {error}")))?;
    Ok(local)
}

impl WorldGraph {
    pub fn transform(
        &mut self,
        og_id: &str,
        transform: Transform,
    ) -> Result<ChangeSet, GraphError> {
        self.mutate(|draft| {
            let descendants = draft.descendants(og_id)?;
            let node = draft
                .nodes
                .get_mut(og_id)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, og_id))?;
            node.local = transform.apply(node.local)?;
            if let Transform::Place {
                origin,
                x_direction,
                normal,
                scale,
            } = transform
            {
                node.place_input = Some(place_input(origin, x_direction, normal, scale));
            }
            draft.affected.extend(descendants);
            Ok(())
        })
        .map_err(|error| error.in_context(ErrorContext::Transform))
    }

    pub fn placement(&self, og_id: &str) -> Result<Placement, GraphError> {
        let node = self.node(og_id)?;
        if let Some(input) = node.place_input {
            if same_bits(from_place(input)?, node.local) {
                return Ok(Placement {
                    origin: input.origin,
                    x_direction: input.x_direction,
                    normal: input.normal,
                    scale: input.scale,
                });
            }
        }
        Ok(placement_form(node.local))
    }

    pub fn world_placement(&self, og_id: &str) -> Result<Similarity3, GraphError> {
        if let Some(cached) = self.world_cache.borrow().get(og_id) {
            return Ok(*cached);
        }
        let mut chain = Vec::new();
        let mut cursor = Some(og_id.to_string());
        let mut result = Similarity3::IDENTITY;
        while let Some(current) = cursor {
            if let Some(cached) = self.world_cache.borrow().get(&current) {
                result = *cached;
                break;
            }
            let node = self.node(&current)?;
            chain.push(current);
            cursor = node.parent.clone();
        }
        while let Some(current) = chain.pop() {
            let node = self.node(&current)?;
            result = result.compose(&node.local);
            result
                .validate()
                .map_err(|error| invalid_transform(format!("invalid world placement: {error}")))?;
            self.world_cache.borrow_mut().insert(current, result);
        }
        Ok(result)
    }

    pub fn world_matrix(&self, og_id: &str) -> Result<[f64; 16], GraphError> {
        Ok(self.world_placement(og_id)?.to_column_major())
    }

    pub fn relative_placement(
        &self,
        source: &str,
        target: &str,
    ) -> Result<Similarity3, GraphError> {
        let ancestors = |id: &str| -> Result<Vec<String>, GraphError> {
            let mut path = Vec::new();
            let mut current = Some(id.to_string());
            while let Some(og_id) = current {
                let node = self.node(&og_id)?;
                current = node.parent.clone();
                path.push(og_id);
            }
            Ok(path)
        };
        let mut a = ancestors(source)?;
        let mut b = ancestors(target)?;
        while a.last().is_some() && a.last() == b.last() {
            a.pop();
            b.pop();
        }
        let mut source_to_lca = Similarity3::IDENTITY;
        for id in a.iter().rev() {
            source_to_lca = source_to_lca.compose(&self.node(id)?.local);
        }
        let mut target_to_lca = Similarity3::IDENTITY;
        for id in b.iter().rev() {
            target_to_lca = target_to_lca.compose(&self.node(id)?.local);
        }
        let result = target_to_lca.inverse().compose(&source_to_lca);
        result
            .validate()
            .map_err(|error| invalid_transform(format!("invalid relative placement: {error}")))?;
        Ok(result)
    }
}

fn placement_form(transform: Similarity3) -> Placement {
    let frame = transform
        .compose(&Similarity3 {
            frame: GROUND,
            scale: 1.0,
        })
        .frame;
    Placement {
        origin: frame.origin,
        x_direction: frame.x,
        normal: frame.z,
        scale: transform.scale,
    }
}
