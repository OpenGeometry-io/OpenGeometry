use super::candidate::Candidate;
use super::error::{ErrorCode, GraphError};
use super::node::Node;
use std::collections::BTreeMap;

pub(super) fn descendants(nodes: &BTreeMap<String, Node>, id: &str) -> Vec<String> {
    let mut stack = vec![id.to_string()];
    let mut result = Vec::new();
    while let Some(current) = stack.pop() {
        if let Some(node) = nodes.get(&current) {
            result.push(current);
            stack.extend(node.children.iter().rev().cloned());
        }
    }
    result
}

impl Candidate {
    pub(super) fn descendants(&self, id: &str) -> Result<Vec<String>, GraphError> {
        let mut stack = vec![id.to_string()];
        let mut result = Vec::new();
        while let Some(current) = stack.pop() {
            let node = self
                .nodes
                .get(&current)
                .ok_or_else(|| GraphError::code(ErrorCode::UnknownNode, &current))?;
            result.push(current);
            stack.extend(node.children.iter().rev().cloned());
        }
        Ok(result)
    }
}
