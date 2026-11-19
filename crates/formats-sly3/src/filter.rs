//! World filters and condition lists (world-file.md 6), which gate object records, meshes, palettes and bitmaps.

use alloc::vec::Vec;

use formats::error::Result;
use formats::reader::Reader;

/// A world filter (world slots) and a condition list (condition ids).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub slots: Vec<i16>,
    pub conditions: Vec<i32>,
}

/// What the filters are tested against: the current world slot and condition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// The current world slot; `None` matches every world filter.
    pub slot: Option<i16>,
    /// The current condition; −1 matches every condition.
    pub condition: i32,
    /// Extra conditions added by the option set.
    pub extra: Vec<i32>,
}

impl Context {
    /// Everything matches.
    pub fn any() -> Context {
        Context { slot: None, condition: -1, extra: Vec::new() }
    }
}

impl Filter {
    /// Reads `f` × `s16` world slots, then `c` × `s32` conditions.
    pub fn read(r: &mut Reader<'_>, f: usize, c: usize) -> Result<Filter> {
        let slots = (0..f).map(|_| r.i16()).collect::<Result<Vec<_>>>()?;
        let conditions = (0..c).map(|_| r.i32()).collect::<Result<Vec<_>>>()?;
        Ok(Filter { slots, conditions })
    }

    /// Whether there is nothing to test.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty() && self.conditions.is_empty()
    }

    /// The world filter matches when empty or holding the current slot; a condition matches when the current
    /// condition is −1, it is −1, it equals the current one or it is an extra one; the list matches when empty or
    /// any entry matches. Both must match.
    pub fn matches(&self, ctx: &Context) -> bool {
        let slot_ok = self.slots.is_empty() || ctx.slot.is_none_or(|s| self.slots.contains(&s));
        let cond_ok = self.conditions.is_empty()
            || self
                .conditions
                .iter()
                .any(|&c| ctx.condition == -1 || c == -1 || c == ctx.condition || ctx.extra.contains(&c));
        slot_ok && cond_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        let f = Filter { slots: alloc::vec![2, 5], conditions: alloc::vec![12] };
        assert!(f.matches(&Context::any()));
        let mut c = Context { slot: Some(5), condition: 12, extra: alloc::vec![] };
        assert!(f.matches(&c));
        c.slot = Some(3);
        assert!(!f.matches(&c));
        c.slot = Some(2);
        c.condition = 7;
        assert!(!f.matches(&c));
        c.extra.push(12);
        assert!(f.matches(&c));
        assert!(Filter { slots: alloc::vec![], conditions: alloc::vec![-1] }.matches(&c));
        assert!(Filter::default().matches(&c) && Filter::default().is_empty());
    }
}
