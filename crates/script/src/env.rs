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

    /// The binding of `sym`, searched from `r` outwards, passing over bindings whose value `skip` accepts.
    pub fn lookup_skipping(&self, mut r: EnvRef, sym: u32, skip: impl Fn(&Value) -> bool) -> Option<Value> {
        loop {
            let e = self.get(r)?;
            if let Some((_, v)) = e.bindings.iter().find(|(s, v)| *s == sym && !skip(v)) {
                return Some(v.clone());
            }
            r = e.parent?;
        }
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

    /// A clone of `r` (specs/sly2/scripts.md 1: a package copy's module frame): the same parent, the bindings copied
    /// (lists copied too), and every procedure closed over `r` re-created closed over the clone. `None` when `r` is
    /// freed.
    pub fn clone_env(&mut self, r: EnvRef) -> Option<EnvRef> {
        let (parent, bindings) = {
            let e = self.get(r)?;
            (e.parent, e.bindings.clone())
        };
        let new = self.alloc(parent);
        let bindings = bindings.into_iter().map(|(s, v)| (s, reclose(&v, r, new))).collect();
        if let Some(e) = self.get_mut(new) {
            e.bindings = bindings;
        }
        Some(new)
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

/// `v` with procedures closed over `from` re-created over `to`, and lists copied (cells are not shared with the
/// original); other values as they are.
///
/// A list can contain itself (operations 35 and 36 replace a cell's halves), so the copy keeps a map from each
/// original cell to its copy: a cell reached twice, or through a loop, gives the same copy, and shared cells stay
/// shared. The walk uses an explicit stack, not recursion, and copies at most [`MAX_LIST_ITEMS`] cells; cells past
/// that limit are shared with the original rather than copied.
fn reclose(v: &Value, from: EnvRef, to: EnvRef) -> Value {
    use crate::value::{Cons, MAX_LIST_ITEMS};
    use alloc::collections::BTreeMap;
    use core::cell::RefCell;
    let mut copies: BTreeMap<*const Cons, Rc<Cons>> = BTreeMap::new();
    let mut pending: Vec<(Rc<Cons>, Rc<Cons>)> = Vec::new();
    let map = |v: &Value, copies: &mut BTreeMap<*const Cons, Rc<Cons>>, pending: &mut Vec<(Rc<Cons>, Rc<Cons>)>| match v
    {
        Value::Procedure(p) if p.env == from => Value::Procedure(Rc::new(crate::value::Procedure {
            node: p.node,
            params: p.params.clone(),
            rest: p.rest,
            env: to,
        })),
        Value::List(c) => {
            if let Some(n) = copies.get(&Rc::as_ptr(c)) {
                return Value::List(n.clone());
            }
            if copies.len() >= MAX_LIST_ITEMS {
                return v.clone();
            }
            let n = Rc::new(Cons { car: RefCell::new(Value::Nil), cdr: RefCell::new(Value::Nil) });
            copies.insert(Rc::as_ptr(c), n.clone());
            pending.push((c.clone(), n.clone()));
            Value::List(n)
        }
        _ => v.clone(),
    };
    let out = map(v, &mut copies, &mut pending);
    while let Some((old, new)) = pending.pop() {
        let car = map(&old.car.borrow(), &mut copies, &mut pending);
        let cdr = map(&old.cdr.borrow(), &mut copies, &mut pending);
        *new.car.borrow_mut() = car;
        *new.cdr.borrow_mut() = cdr;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Procedure;

    #[test]
    fn a_clone_has_its_own_bindings_and_reclosed_procedures() {
        let mut h = Heap::default();
        let world = h.alloc(None);
        let m = h.alloc(Some(world));
        let proc_ = Value::Procedure(Rc::new(Procedure { node: 0, params: Vec::new(), rest: None, env: m }));
        h.define(m, 1, Value::Int(5));
        h.define(m, 2, proc_);
        h.define(m, 3, Value::list([Value::Int(1), Value::Int(2)]));
        let c = h.clone_env(m).unwrap();
        assert_eq!(h.parent(c), Some(world));
        // A `set` in the clone leaves the original as it was (each copy's event 0 lands in its own frame).
        h.set(c, 1, Value::Int(7));
        assert!(matches!(h.lookup(m, 1), Some(Value::Int(5))));
        assert!(matches!(h.lookup(c, 1), Some(Value::Int(7))));
        // Procedures closed over the module frame are closed over the clone.
        assert!(matches!(h.lookup(c, 2), Some(Value::Procedure(p)) if p.env == c));
        assert!(matches!(h.lookup(m, 2), Some(Value::Procedure(p)) if p.env == m));
        // Lists are copied, not shared.
        let (a, b) = (h.lookup(m, 3).unwrap(), h.lookup(c, 3).unwrap());
        assert!(!crate::value::same_value(&a, &b) && a.to_vec().len() == 2 && b.to_vec().len() == 2);
    }

    #[test]
    fn a_clone_copies_a_list_that_contains_itself() {
        let mut h = Heap::default();
        let world = h.alloc(None);
        let m = h.alloc(Some(world));
        let proc_ = Value::Procedure(Rc::new(Procedure { node: 0, params: Vec::new(), rest: None, env: m }));
        // (p . <itself>) in the car and a loop in the cdr: the list holds itself twice over.
        let l = Value::list([proc_, Value::Int(2)]);
        let Value::List(c) = &l else { unreachable!() };
        let second = match &*c.cdr.borrow() {
            Value::List(n) => n.clone(),
            _ => unreachable!(),
        };
        *second.car.borrow_mut() = l.clone();
        *second.cdr.borrow_mut() = l.clone();
        h.define(m, 1, l.clone());
        let k = h.clone_env(m).unwrap();
        let copy = h.lookup(k, 1).unwrap();
        assert!(!crate::value::same_value(&copy, &l));
        let Value::List(cc) = &copy else { panic!("not a list") };
        assert!(matches!(&*cc.car.borrow(), Value::Procedure(p) if p.env == k));
        // The copy loops back on itself, not on the original.
        let next = match &*cc.cdr.borrow() {
            Value::List(n) => n.clone(),
            _ => panic!("no second cell"),
        };
        assert!(crate::value::same_value(&next.car.borrow(), &copy));
        assert!(crate::value::same_value(&next.cdr.borrow(), &copy));
        // Break the loops so the test does not leak them.
        *second.cdr.borrow_mut() = Value::Nil;
        *second.car.borrow_mut() = Value::Nil;
        *next.cdr.borrow_mut() = Value::Nil;
        *next.car.borrow_mut() = Value::Nil;
    }
}
