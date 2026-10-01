use super::error::{ErrorCode, GraphError};
use super::graph::WorldGraph;
use crate::brep::PatchBounds;
use crate::math::Point3;

type Bounds3 = [f64; 6];

fn corners(bounds: PatchBounds) -> [Point3; 8] {
    std::array::from_fn(|index| {
        std::array::from_fn(|axis| {
            if index & (1 << axis) == 0 {
                bounds.axes[axis].lo
            } else {
                bounds.axes[axis].hi
            }
        })
    })
}

fn include(bounds: Option<Bounds3>, point: Point3) -> Bounds3 {
    match bounds {
        Some(bounds) => [
            bounds[0].min(point[0]),
            bounds[1].min(point[1]),
            bounds[2].min(point[2]),
            bounds[3].max(point[0]),
            bounds[4].max(point[1]),
            bounds[5].max(point[2]),
        ],
        None => [point[0], point[1], point[2], point[0], point[1], point[2]],
    }
}

impl WorldGraph {
    pub fn bounds(&self, og_id: &str) -> Result<Option<Bounds3>, GraphError> {
        let mut stack = vec![og_id.to_string()];
        let mut bounds = None;
        while let Some(current) = stack.pop() {
            let node = self.node(&current)?;
            stack.extend(node.children.iter().rev().cloned());
            if let Some(shape_id) = &node.shape {
                let shape = self.shapes.shapes.get(shape_id).ok_or_else(|| {
                    GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
                })?;
                if let Some(local_bounds) = shape.brep.bounds()? {
                    let placement = self.world_placement(&current)?;
                    for corner in corners(local_bounds) {
                        bounds = Some(include(bounds, placement.apply_point(corner)));
                    }
                }
            }
        }
        Ok(bounds)
    }

    pub fn local_bounds(&self, og_id: &str) -> Result<Option<Bounds3>, GraphError> {
        let Some(shape_id) = &self.node(og_id)?.shape else {
            return Ok(None);
        };
        let shape = self.shapes.shapes.get(shape_id).ok_or_else(|| {
            GraphError::code(ErrorCode::InvalidTopology, "shape reference is missing")
        })?;
        Ok(shape.brep.bounds()?.map(|bounds| {
            [
                bounds.axes[0].lo,
                bounds.axes[1].lo,
                bounds.axes[2].lo,
                bounds.axes[0].hi,
                bounds.axes[1].hi,
                bounds.axes[2].hi,
            ]
        }))
    }
}
