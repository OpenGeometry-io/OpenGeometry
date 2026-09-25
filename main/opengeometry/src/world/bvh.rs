use super::graph::Bounds3;

const LEAF_SIZE: usize = 4;

enum BvhNode {
    Leaf {
        bounds: Bounds3,
        start: usize,
        count: usize,
    },
    Inner {
        bounds: Bounds3,
        left: usize,
        right: usize,
    },
}

pub(super) struct Bvh {
    nodes: Vec<BvhNode>,
    items: Vec<usize>,
    boxes: Vec<Bounds3>,
}

impl Bvh {
    pub(super) fn build(boxes: Vec<Bounds3>) -> Self {
        let mut bvh = Self {
            nodes: Vec::new(),
            items: (0..boxes.len()).collect(),
            boxes,
        };
        if !bvh.boxes.is_empty() {
            bvh.split(0, bvh.items.len());
        }
        bvh
    }

    pub(super) fn query(&self, target: &Bounds3, hits: &mut Vec<usize>) {
        if self.nodes.is_empty() {
            return;
        }
        let mut pending = vec![0];
        while let Some(index) = pending.pop() {
            match &self.nodes[index] {
                BvhNode::Leaf {
                    bounds,
                    start,
                    count,
                } => {
                    if overlaps(bounds, target) {
                        hits.extend(
                            self.items[*start..*start + *count]
                                .iter()
                                .copied()
                                .filter(|item| overlaps(&self.boxes[*item], target)),
                        );
                    }
                }
                BvhNode::Inner {
                    bounds,
                    left,
                    right,
                } => {
                    if overlaps(bounds, target) {
                        pending.push(*left);
                        pending.push(*right);
                    }
                }
            }
        }
    }

    fn split(&mut self, start: usize, end: usize) -> usize {
        let bounds = self.items[start..end]
            .iter()
            .map(|item| self.boxes[*item])
            .reduce(union)
            .unwrap_or([0.0; 6]);
        let index = self.nodes.len();
        if end - start <= LEAF_SIZE {
            self.nodes.push(BvhNode::Leaf {
                bounds,
                start,
                count: end - start,
            });
            return index;
        }
        let axis = (0..3)
            .max_by(|a, b| (bounds[a + 3] - bounds[*a]).total_cmp(&(bounds[b + 3] - bounds[*b])))
            .unwrap_or(0);
        let boxes = &self.boxes;
        self.items[start..end]
            .sort_by(|a, b| center(&boxes[*a], axis).total_cmp(&center(&boxes[*b], axis)));
        self.nodes.push(BvhNode::Leaf {
            bounds,
            start,
            count: 0,
        });
        let middle = start + (end - start) / 2;
        let left = self.split(start, middle);
        let right = self.split(middle, end);
        self.nodes[index] = BvhNode::Inner {
            bounds,
            left,
            right,
        };
        index
    }
}

pub(super) fn overlaps(a: &Bounds3, b: &Bounds3) -> bool {
    (0..3).all(|axis| a[axis] <= b[axis + 3] && b[axis] <= a[axis + 3])
}

fn union(a: Bounds3, b: Bounds3) -> Bounds3 {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].min(b[2]),
        a[3].max(b[3]),
        a[4].max(b[4]),
        a[5].max(b[5]),
    ]
}

fn center(bounds: &Bounds3, axis: usize) -> f64 {
    bounds[axis] / 2.0 + bounds[axis + 3] / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pseudo_random_boxes(count: usize) -> Vec<Bounds3> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % 10_000) as f64 / 100.0
        };
        (0..count)
            .map(|_| {
                let [x, y, z, w, h, d] = [
                    next(),
                    next(),
                    next(),
                    next() / 10.0,
                    next() / 10.0,
                    next() / 10.0,
                ];
                [x, y, z, x + w, y + h, z + d]
            })
            .collect()
    }

    #[test]
    fn query_matches_brute_force() {
        let boxes = pseudo_random_boxes(500);
        let bvh = Bvh::build(boxes.clone());
        for target in pseudo_random_boxes(60) {
            let mut hits = Vec::new();
            bvh.query(&target, &mut hits);
            hits.sort_unstable();
            let expected: Vec<usize> = (0..boxes.len())
                .filter(|index| overlaps(&boxes[*index], &target))
                .collect();
            assert_eq!(hits, expected);
        }
    }

    #[test]
    fn empty_tree_returns_nothing() {
        let mut hits = Vec::new();
        Bvh::build(Vec::new()).query(&[0.0; 6], &mut hits);
        assert!(hits.is_empty());
    }
}
