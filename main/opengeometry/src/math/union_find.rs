pub(crate) fn find(parent: &mut [usize], value: usize) -> usize {
    let mut root = value;
    while parent[root] != root {
        root = parent[root];
    }
    let mut current = value;
    while parent[current] != current {
        let next = parent[current];
        parent[current] = root;
        current = next;
    }
    root
}

pub(crate) fn union(parent: &mut [usize], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}
