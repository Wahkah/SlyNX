//! Finding objects by OID and by class (object-stream.md 7.4, scripts.md 7 operations 103 to 110).
//!
//! OIDs are not unique; every lookup takes a scope. Candidates are tried in load order. "Below a context" means a
//! descendant of the context, or of an object the context placed when the context is a proxy (a proxy stays in the
//! world as a named anchor for what it placed, classes/proxy.md 2).

use alloc::vec::Vec;

use super::{NONE, World};

/// The proxy class.
const PROXY: i16 = crate::object::class::id::PROXY;
use crate::object::Handle;

impl World {
    /// Every live object with `oid`, in load order (slots).
    pub fn slots_with_oid(&self, oid: i16) -> &[u32] {
        self.by_oid.get(&oid).map_or(&[], |v| v.as_slice())
    }

    /// The candidates of an OID lookup, in load order (scripts.md 11.4): a proxy with the OID gives the first object
    /// it placed, and a source whose proxy has the same OID is found only through that proxy.
    pub fn oid_candidates(&self, oid: i16) -> impl Iterator<Item = u32> + '_ {
        let all = self.slots_with_oid(oid);
        // Sources placed by a proxy with the same OID are found only through that proxy.
        let through_proxy: Vec<u32> = all
            .iter()
            .filter(|&&s| self.class[s as usize] == PROXY)
            .flat_map(|s| self.placed_by.get(s).into_iter().flatten().copied())
            .collect();
        let mut seen: Vec<u32> = Vec::new();
        all.iter().copied().filter_map(move |s| {
            let c = if self.class[s as usize] == PROXY {
                self.placed_by.get(&s).and_then(|p| p.first().copied())?
            } else if through_proxy.contains(&s) {
                return None;
            } else {
                s
            };
            if seen.contains(&c) {
                return None;
            }
            seen.push(c);
            Some(c)
        })
    }

    /// Every live object of class `class`, in load order (slots).
    pub fn slots_of_class(&self, class: i16) -> &[u32] {
        self.by_class.get(class as usize).map_or(&[], |v| v.as_slice())
    }

    /// Whether `slot` is strictly below `ctx` (a descendant, or below what `ctx` placed as a proxy). The order must be
    /// current.
    fn is_below(&self, slot: u32, ctx: u32) -> bool {
        let r = self.subtree_range(ctx);
        let p = self.order_pos[slot as usize] as usize;
        if p > r.start && p < r.end {
            return true;
        }
        if let Some(placed) = self.placed_by.get(&ctx) {
            for &root in placed {
                if root == slot {
                    return true;
                }
                let r = self.subtree_range(root);
                if p > r.start && p < r.end {
                    return true;
                }
            }
        }
        false
    }

    /// Operation 103: the first object with `oid` in the whole world (`context` `None`) or below `context`.
    pub fn find_oid(&mut self, oid: i16, context: Option<Handle>) -> Option<Handle> {
        self.ensure_order();
        let ctx = match context {
            Some(c) => Some(self.slot(c)?),
            None => None,
        };
        let s = self.oid_candidates(oid).find(|&s| ctx.is_none_or(|c| self.is_below(s, c)))?;
        self.handle(s)
    }

    /// Operation 104: every object with `oid`, whole world or below `context`, in load order.
    pub fn find_all_oid(&mut self, oid: i16, context: Option<Handle>, out: &mut Vec<Handle>) {
        self.ensure_order();
        let ctx = match context {
            Some(c) => match self.slot(c) {
                Some(s) => Some(s),
                None => return,
            },
            None => None,
        };
        for s in self.oid_candidates(oid) {
            if ctx.is_none_or(|c| self.is_below(s, c))
                && let Some(h) = self.handle(s)
            {
                out.push(h);
            }
        }
    }

    /// The ancestor chain of a slot, itself first.
    fn chain(&self, mut s: u32) -> Vec<u32> {
        let mut v = Vec::with_capacity(16);
        while s != NONE {
            v.push(s);
            s = self.parent[s as usize];
        }
        v
    }

    /// How far up `context`'s chain the candidate's chain meets it (0: the candidate is the context or below it;
    /// the chain length: they meet only at the world).
    fn meeting_distance(&self, chain: &[u32], candidate: u32) -> usize {
        let mut s = candidate;
        while s != NONE {
            if let Some(k) = chain.iter().position(|&a| a == s) {
                return k;
            }
            s = self.parent[s as usize];
        }
        chain.len()
    }

    /// Operation 105: the object with `oid` nearest to `context` (object-stream.md 7.4): the candidate whose
    /// ancestor chain meets the context's closest to the context; ties go to the first in load order. Without a
    /// context, the first in load order.
    pub fn find_nearest(&self, oid: i16, context: Option<Handle>) -> Option<Handle> {
        let cands: Vec<u32> = self.oid_candidates(oid).collect();
        let ctx = match context.and_then(|c| self.slot(c)) {
            Some(c) => c,
            None => return cands.first().and_then(|&s| self.handle(s)),
        };
        let chain = self.chain(ctx);
        let mut best: Option<(usize, u32)> = None;
        for &s in &cands {
            let d = self.meeting_distance(&chain, s);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, s));
            }
        }
        best.and_then(|(_, s)| self.handle(s))
    }

    /// Operation 106: every object with `oid` at the smallest meeting distance from `context`, in load order.
    pub fn find_all_nearest(&self, oid: i16, context: Option<Handle>, out: &mut Vec<Handle>) {
        let cands: Vec<u32> = self.oid_candidates(oid).collect();
        let Some(ctx) = context.and_then(|c| self.slot(c)) else {
            out.extend(cands.iter().filter_map(|&s| self.handle(s)));
            return;
        };
        let chain = self.chain(ctx);
        let d: Vec<usize> = cands.iter().map(|&s| self.meeting_distance(&chain, s)).collect();
        let Some(&min) = d.iter().min() else { return };
        out.extend(cands.iter().zip(&d).filter(|&(_, &x)| x == min).filter_map(|(&s, _)| self.handle(s)));
    }

    /// Operation 110: every object of `class`, whole world or below `context`, in load order.
    pub fn find_class(&mut self, class: i16, context: Option<Handle>, out: &mut Vec<Handle>) {
        self.ensure_order();
        let ctx = match context {
            Some(c) => match self.slot(c) {
                Some(s) => Some(s),
                None => return,
            },
            None => None,
        };
        for &s in self.slots_of_class(class) {
            if ctx.is_none_or(|c| self.is_below(s, c))
                && let Some(h) = self.handle(s)
            {
                out.push(h);
            }
        }
    }

    /// The nearest ancestor of `h` with `oid`.
    pub fn find_ancestor(&self, h: Handle, oid: i16) -> Option<Handle> {
        let s = self.slot(h)?;
        self.ancestor_slot_with_oid(s, oid).and_then(|a| self.handle(a))
    }

    /// The direct children of `h` of class `class`, in order.
    pub fn children_of_class(&self, h: Handle, class: i16) -> Vec<Handle> {
        self.children(h).filter(|&c| self.class(c) == Some(class)).collect()
    }
}
