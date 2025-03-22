/**
 * Linked List of Primitives or Geometry Objects, where each node is a primitive or geometry object
 * and the edges are the operations between the nodes.
 */

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

pub struct GraphScene {
  id: String,
  nodes: Vec<GraphNode>,
  edges: Vec<GraphEdge>
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct GraphNode {
  id: String,
  primitive: Option<Primitive>,
  geometry: Option<BasePolygon>
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct GraphEdge {
  id: String,
  operation: Operation,
  from: String,
  to: String
}

#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub enum Operation {
  Triangulate,
  Windingsort,
  Intersect
}