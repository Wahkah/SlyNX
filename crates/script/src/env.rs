//! Environments, kept in an arena and reclaimed by a mark-and-sweep pass.
//!
//! Procedures refer to their defining environment and environments hold procedures, so plain reference counting
//! would leak every closure stored where it was made. Environments therefore live in slots addressed by
//! [`EnvRef`] (index plus generation), and [`Heap::collect`] frees the ones no root can reach.

use alloc::collections::BTreeSet;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::value::Value;

/// A handle to an environment. A handle to a freed slot is detected through the generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvRef {
    index: u32,
    generation: u32,
}

struct Env {
    parent: Option<EnvRef>,
    bindings: Vec<(u32, Value)>,
}

struct Slot {
    generation: u32,
    mark: bool,
    env: Option<Env>,
}

/// The environment arena.
#[derive(Default)]
pub(crate) struct Heap {
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
    /// Environments made since the last collection.
    pub since_collect: usize,
}

impl Heap {
    /// A new environment with `parent`.
    pub fn alloc(&mut self, parent: Option<EnvRef>) -> EnvRef {
        self.live += 1;
        self.since_collect += 1;
        let env = Env { parent, bindings: Vec::new() };
        if let Some(index) = self.free.pop() {
            let s = &mut self.slots[index as usize];
            s.env = Some(env);
            EnvRef { index, generation: s.generation }
        } else {
            self.slots.push(Slot { generation: 0, mark: false, env: Some(env) });
            EnvRef { index: (self.slots.len() - 1) as u32, generation: 0 }
        }
    }

    fn get(&self, r: EnvRef) -> Option<&Env> {
        self.slots.get(r.index as usize).filter(|s| s.generation == r.generation).and_then(|s| s.env.as_ref())
    }

    fn get_mut(&mut self, r: EnvRef) -> Option<&mut Env> {
        self.slots.get_mut(r.index as usize).filter(|s| s.generation == r.generation).and_then(|s| s.env.as_mut())
    }

    /// Whether `r` still names a live environment.
    pub fn is_live(&self, r: EnvRef) -> bool {
        self.get(r).is_some()
    }

    /// Number of live environments.
    pub fn live(&self) -> usize {
        self.live
    }

    /// The binding of `sym`, searched from `r` outwards.
    pub fn lookup(&self, mut r: EnvRef, sym: u32) -> Option<Value> {
        loop {
            let e = self.get(r)?;
            if let Some((_, v)) = e.bindings.iter().find(|(s, _)| *s == sym) {
                return Some(v.clone());
            }
            r = e.parent?;
        }
    }

    /// Stores `v` in the nearest existing binding of `sym`; false when there is none.
    pub fn set(&mut self, mut r: EnvRef, sym: u32, v: Value) -> bool {
        loop {
            let Some(e) = self.get_mut(r) else { return false };
            if let Some(slot) = e.bindings.iter_mut().find(|(s, _)| *s == sym) {
                slot.1 = v;
                return true;
            }
            match e.parent {
                Some(p) => r = p,
                None => return false,
            }
        }
    }

    /// Binds `sym` in `r` (replacing a binding of `sym` already in `r`).
    pub fn define(&mut self, r: EnvRef, sym: u32, v: Value) -> bool {
        let Some(e) = self.get_mut(r) else { return false };
        match e.bindings.iter_mut().find(|(s, _)| *s == sym) {
            Some(slot) => slot.1 = v,
            None => e.bindings.push((sym, v)),
        }
        true
    }

    /// The parent of `r` (`None` for a root or a freed environment).
    pub fn parent(&self, r: EnvRef) -> Option<EnvRef> {
        self.get(r)?.parent
    }

    /// Makes `parent` the parent of `r`. Refused (false) when `r` is freed or `r` is `parent` or one of its
    /// ancestors (that would make a cycle).
    pub fn set_parent(&mut self, r: EnvRef, parent: EnvRef) -> bool {
        let mut at = Some(parent);
        while let Some(a) = at {
            if a == r {
                return false;
            }
            at = self.parent(a);
        }
        match self.get_mut(r) {
            Some(env) => {
                env.parent = Some(parent);
                true
            }
            None => false,
        }
    }

    /// The symbols bound in `r`, in binding order.
    pub fn symbols(&self, r: EnvRef) -> Vec<u32> {
        self.get(r).map(|e| e.bindings.iter().map(|(s, _)| *s).collect()).unwrap_or_default()
    }

    /// Frees every environment not reachable from `envs` or from the environments inside `values`.
    ///
    /// Each list cell and procedure is visited once (by address), so lists that share cells or contain themselves
    /// (operation 35 can store a list in its own element) are walked in time proportional to their size.
    pub fn collect<'a>(&mut self, envs: impl IntoIterator<Item = EnvRef>, values: impl IntoIterator<Item = &'a Value>) {
        let mut work_envs: Vec<EnvRef> = envs.into_iter().collect();
        let mut work_vals: Vec<Value> = values.into_iter().cloned().collect();
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        loop {
            if let Some(v) = work_vals.pop() {
                match v {
                    Value::Procedure(p) => {
                        if seen.insert(Rc::as_ptr(&p) as *const u8 as usize) {
                            work_envs.push(p.env);
                        }
                    }
                    Value::List(c) => {
                        if seen.insert(Rc::as_ptr(&c) as *const u8 as usize) {
                            work_vals.push(c.car.borrow().clone());
                            work_vals.push(c.cdr.borrow().clone());
                        }
                    }
                    _ => {}
                }
                continue;
            }
            let Some(r) = work_envs.pop() else { break };
            let Some(slot) = self.slots.get_mut(r.index as usize) else { continue };
            if slot.generation != r.generation || slot.mark {
                continue;
            }
            let Some(env) = slot.env.as_ref() else { continue };
            slot.mark = true;
            work_envs.extend(env.parent);
            work_vals.extend(env.bindings.iter().map(|(_, v)| v.clone()));
        }
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.mark {
                s.mark = false;
            } else if s.env.is_some() {
                s.env = None;
                s.generation = s.generation.wrapping_add(1);
                self.free.push(i as u32);
                self.live -= 1;
            }
        }
        self.since_collect = 0;
    }
}
