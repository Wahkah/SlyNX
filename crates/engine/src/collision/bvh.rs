//! A bounding-volume hierarchy over boxes, built with a binned surface-area split.
//!
//! Used twice: over the triangles of each collision mesh (in the mesh's local space) and over the bodies of a
//! collision world (in world space, refitted when bodies move).

use alloc::vec::Vec;

use super::geom::Aabb;

/// Most items in a leaf.
pub const LEAF_SIZE: usize = 4;
/// Deepest tree the queries' fixed stacks accept; deeper builds stop splitting.
pub const MAX_DEPTH: usize = 48;
const BINS: usize = 12;

/// One node: an internal node (`count == 0`) has its children at `first` and `first + 1`; a leaf holds
/// `items[first .. first + count]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// Bounds.
    pub bounds: Aabb,
    /// First child or first item.
    pub first: u32,
    /// Item count (0 for an internal node).
    pub count: u32,
}

/// The hierarchy. Node 0 is the root (absent when there are no items).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bvh {
    /// Nodes; a child always has a higher index than its parent.
    pub nodes: Vec<Node>,
    /// Item indices, grouped by leaf.
    pub items: Vec<u32>,
}

impl Bvh {
    /// Builds a hierarchy over `boxes` (item `i` has box `boxes[i]`).
    pub fn build(boxes: &[Aabb]) -> Bvh {
        let mut bvh = Bvh { nodes: Vec::new(), items: (0..boxes.len() as u32).collect() };
        if boxes.is_empty() {
            return bvh;
        }
        let centers: Vec<[f32; 3]> = boxes.iter().map(Aabb::center).collect();
        bvh.nodes.push(Node { bounds: Aabb::EMPTY, first: 0, count: boxes.len() as u32 });
        // (node, depth) to split.
        let mut stack: Vec<(usize, usize)> = alloc::vec![(0, 0)];
        while let Some((ni, depth)) = stack.pop() {
            let Node { first, count, .. } = bvh.nodes[ni];
            let (s, e) = (first as usize, (first + count) as usize);
            let bounds = bvh.items[s..e].iter().fold(Aabb::EMPTY, |b, &i| b.union(boxes[i as usize]));
            bvh.nodes[ni].bounds = bounds;
            if e - s <= LEAF_SIZE || depth + 1 >= MAX_DEPTH {
                continue;
            }
            let Some(mid) = split(&mut bvh.items[s..e], boxes, &centers) else { continue };
            let left = bvh.nodes.len();
            bvh.nodes.push(Node { bounds: Aabb::EMPTY, first, count: mid as u32 });
            bvh.nodes.push(Node { bounds: Aabb::EMPTY, first: first + mid as u32, count: count - mid as u32 });
            bvh.nodes[ni] = Node { bounds, first: left as u32, count: 0 };
            stack.push((left, depth + 1));
            stack.push((left + 1, depth + 1));
        }
        bvh
    }

    /// Recomputes every node's bounds from the items' current boxes (children before parents).
    pub fn refit(&mut self, boxes: &[Aabb]) {
        for i in (0..self.nodes.len()).rev() {
            let n = self.nodes[i];
            self.nodes[i].bounds = if n.count > 0 {
                let (s, e) = (n.first as usize, (n.first + n.count) as usize);
                self.items[s..e].iter().fold(Aabb::EMPTY, |b, &k| b.union(boxes[k as usize]))
            } else {
                self.nodes[n.first as usize].bounds.union(self.nodes[n.first as usize + 1].bounds)
            };
        }
    }

    /// Calls `leaf` with the items of every leaf whose bounds pass `test`, skipping subtrees that fail it. `leaf`
    /// returns `false` to stop. `test` is re-evaluated as the walk goes, so it may depend on state `leaf` changes
    /// (for example a shrinking ray length held in a `Cell`).
    #[inline]
    pub fn walk(&self, mut test: impl FnMut(&Aabb) -> bool, mut leaf: impl FnMut(&[u32]) -> bool) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = [0u32; MAX_DEPTH + 2];
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let n = &self.nodes[stack[sp] as usize];
            if !test(&n.bounds) {
                continue;
            }
            if n.count > 0 {
                let (s, e) = (n.first as usize, (n.first + n.count) as usize);
                if !leaf(&self.items[s..e]) {
                    return;
                }
            } else {
                stack[sp] = n.first + 1;
                stack[sp + 1] = n.first;
                sp += 2;
            }
        }
    }
}

/// Splits items in place with a binned surface-area heuristic along the longest axis of their centres; returns
/// the size of the left part, or `None` when no split is better than a leaf.
fn split(items: &mut [u32], boxes: &[Aabb], centers: &[[f32; 3]]) -> Option<usize> {
    let cb = items.iter().fold(Aabb::EMPTY, |b, &i| b.with_point(centers[i as usize]));
    let ext = [cb.max[0] - cb.min[0], cb.max[1] - cb.min[1], cb.max[2] - cb.min[2]];
    let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] {
        0
    } else if ext[1] >= ext[2] {
        1
    } else {
        2
    };
    if ext[axis] <= 0.0 {
        // All centres coincide: split in the middle.
        let mid = items.len() / 2;
        return Some(mid);
    }
    let bin_of = |i: u32| {
        let f = (centers[i as usize][axis] - cb.min[axis]) / ext[axis] * BINS as f32;
        (f as usize).min(BINS - 1)
    };
    let mut bin_box = [Aabb::EMPTY; BINS];
    let mut bin_count = [0usize; BINS];
    for &i in items.iter() {
        let b = bin_of(i);
        bin_box[b] = bin_box[b].union(boxes[i as usize]);
        bin_count[b] += 1;
    }
    let mut best = (f32::INFINITY, 0usize);
    for cut in 1..BINS {
        let (mut lb, mut lc) = (Aabb::EMPTY, 0);
        for k in 0..cut {
            lb = lb.union(bin_box[k]);
            lc += bin_count[k];
        }
        let (mut rb, mut rc) = (Aabb::EMPTY, 0);
        for k in cut..BINS {
            rb = rb.union(bin_box[k]);
            rc += bin_count[k];
        }
        if lc == 0 || rc == 0 {
            continue;
        }
        let cost = lb.area() * lc as f32 + rb.area() * rc as f32;
        if cost < best.0 {
            best = (cost, cut);
        }
    }
    if !best.0.is_finite() {
        let mid = items.len() / 2;
        items.sort_unstable_by(|&a, &b| centers[a as usize][axis].total_cmp(&centers[b as usize][axis]));
        return Some(mid);
    }
    // Partition: bins below the cut go left.
    let mut l = 0;
    for k in 0..items.len() {
        if bin_of(items[k]) < best.1 {
            items.swap(l, k);
            l += 1;
        }
    }
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_every_overlapping_item() {
        let boxes: Vec<Aabb> = (0..500)
            .map(|i| {
                let x = (i % 23) as f32 * 3.0;
                let y = (i / 23) as f32 * 2.0;
                Aabb { min: [x, y, 0.0], max: [x + 1.5, y + 1.0, 1.0] }
            })
            .collect();
        let bvh = Bvh::build(&boxes);
        let q = Aabb { min: [10.0, 10.0, 0.0], max: [20.0, 14.0, 0.5] };
        let mut got = Vec::new();
        bvh.walk(
            |b| b.overlaps(&q),
            |items| {
                got.extend(items.iter().copied().filter(|&i| boxes[i as usize].overlaps(&q)));
                true
            },
        );
        got.sort_unstable();
        let want: Vec<u32> = (0..500).filter(|&i| boxes[i as usize].overlaps(&q)).collect();
        assert_eq!(got, want);
        let mut moved = boxes.clone();
        moved[0] = Aabb { min: [100.0; 3], max: [101.0; 3] };
        let mut b2 = bvh.clone();
        b2.refit(&moved);
        assert!(b2.nodes[0].bounds.max[0] >= 101.0);
        assert!(Bvh::build(&[]).nodes.is_empty());
    }
}
