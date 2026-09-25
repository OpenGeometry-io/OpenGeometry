use super::bvh::Bvh;
use super::graph::{corners, include, Bounds3, WorldError, WorldGraph};
use super::transform::Similarity3;
use std::collections::{HashMap, HashSet};

pub(super) struct Leaf {
    pub(super) item: usize,
    pub(super) node: String,
    pub(super) definition: String,
    pub(super) world: Similarity3,
    pub(super) bounds: Bounds3,
}

pub(super) struct ItemPairs {
    pub(super) items: Vec<String>,
    pub(super) leaves: Vec<Leaf>,
    pub(super) pairs: Vec<(usize, usize)>,
}

impl ItemPairs {
    pub(super) fn item_key(&self, leaf_a: usize, leaf_b: usize) -> (usize, usize) {
        let (a, b) = (self.leaves[leaf_a].item, self.leaves[leaf_b].item);
        (a.min(b), a.max(b))
    }
}

impl WorldGraph {
    pub(super) fn candidate_pairs(
        &self,
        items_a: &[String],
        items_b: &[String],
        reach: f64,
    ) -> Result<ItemPairs, WorldError> {
        let mut seen = HashSet::new();
        let items: Vec<String> = items_a
            .iter()
            .chain(items_b)
            .filter(|id| seen.insert(id.as_str()))
            .cloned()
            .collect();
        let item_index: HashMap<&str, usize> = items
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        let mut leaves = Vec::new();
        for (index, item) in items.iter().enumerate() {
            self.collect_leaves(index, item, &mut leaves)?;
        }
        let side = |ids: &[String]| -> HashSet<usize> {
            ids.iter().map(|id| item_index[id.as_str()]).collect()
        };
        let (side_a, side_b) = (side(items_a), side(items_b));
        let bvh = Bvh::build(leaves.iter().map(|leaf| leaf.bounds).collect());

        let mut pairs = Vec::new();
        let mut hits = Vec::new();
        for (index_a, leaf_a) in leaves.iter().enumerate() {
            if !side_a.contains(&leaf_a.item) {
                continue;
            }
            hits.clear();
            bvh.query(&inflate(leaf_a.bounds, reach), &mut hits);
            for &index_b in &hits {
                let leaf_b = &leaves[index_b];
                let mirrored = leaf_a.item > leaf_b.item
                    && side_a.contains(&leaf_b.item)
                    && side_b.contains(&leaf_a.item);
                if leaf_a.item == leaf_b.item
                    || !side_b.contains(&leaf_b.item)
                    || mirrored
                    || self.nested(&items[leaf_a.item], &items[leaf_b.item])?
                {
                    continue;
                }
                pairs.push((index_a, index_b));
            }
        }
        Ok(ItemPairs {
            items,
            leaves,
            pairs,
        })
    }

    pub(super) fn leaves_of(&self, items: &[String]) -> Result<Vec<Leaf>, WorldError> {
        let mut leaves = Vec::new();
        for (index, item) in items.iter().enumerate() {
            self.collect_leaves(index, item, &mut leaves)?;
        }
        let mut seen = HashSet::new();
        leaves.retain(|leaf| seen.insert(leaf.node.clone()));
        Ok(leaves)
    }

    fn collect_leaves(
        &self,
        item: usize,
        root: &str,
        leaves: &mut Vec<Leaf>,
    ) -> Result<(), WorldError> {
        let mut pending = vec![(root.to_string(), self.world(root)?)];
        while let Some((id, world)) = pending.pop() {
            let node = self.node(&id)?;
            if let Some(definition) = &node.definition {
                let local_bounds = self
                    .definitions
                    .get(definition)
                    .ok_or_else(|| WorldError::UnknownDefinition(definition.clone()))?
                    .local_bounds;
                if let Some(local_bounds) = local_bounds {
                    let bounds = corners(local_bounds)
                        .into_iter()
                        .fold(None, |bounds, corner| {
                            Some(include(bounds, world.apply_point(corner)))
                        })
                        .unwrap_or(local_bounds);
                    leaves.push(Leaf {
                        item,
                        node: id.clone(),
                        definition: definition.clone(),
                        world,
                        bounds,
                    });
                }
            }
            for child in &node.children {
                pending.push((child.clone(), world.compose(&self.node(child)?.local)));
            }
        }
        Ok(())
    }

    fn nested(&self, a: &str, b: &str) -> Result<bool, WorldError> {
        Ok(self.is_ancestor(a, b)? || self.is_ancestor(b, a)?)
    }

    fn is_ancestor(&self, ancestor: &str, id: &str) -> Result<bool, WorldError> {
        let mut cursor = self.node(id)?.parent.clone();
        while let Some(current) = cursor {
            if current == ancestor {
                return Ok(true);
            }
            cursor = self.node(&current)?.parent.clone();
        }
        Ok(false)
    }
}

pub(super) fn inflate(bounds: Bounds3, reach: f64) -> Bounds3 {
    [
        bounds[0] - reach,
        bounds[1] - reach,
        bounds[2] - reach,
        bounds[3] + reach,
        bounds[4] + reach,
        bounds[5] + reach,
    ]
}
