//! State machines (class 136, scripts.md 9), run with the script crate's `StateMachine` and `MachineRun`.
//!
//! A machine with option 513 starts by itself after load (event 17). Option 516 sets the goal state, option 519 the
//! playback speed of its transitions. A step with an animation plays it on the machine's root through the animation
//! system (owned animation `k` = the `k`-th animated transition); when it ends the machine enters the new state
//! (event 14 with the from- and to-state OIDs) and chooses again.

use alloc::vec;
use alloc::vec::Vec;

use script::machine::{MachineHost, OPTION_AUTO_START, OPTION_GOAL, OPTION_SPEED, Step};
use script::{MachineRun, StateMachine, Value};

use super::{ClassBehavior, Cx};
use crate::object::{Arg, Handle, OptionValue};
use crate::world::{AnimCommand, World};

/// A running state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    /// The parsed machine.
    pub machine: StateMachine,
    /// Its run.
    pub run: MachineRun,
    /// Playback speed of its animations.
    pub speed: f32,
    rng: u32,
}

/// What the run asks of the engine, collected while it runs.
struct Host<'a> {
    world: &'a mut World,
    me: Handle,
    machine: &'a StateMachine,
    speed: f32,
    rng: &'a mut u32,
}

impl MachineHost for Host<'_> {
    fn random(&mut self) -> f32 {
        // xorshift32: each machine has its own deterministic stream, seeded from its slot.
        let mut x = *self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *self.rng = x;
        (x >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    fn play(&mut self, step: Step) {
        let index = self.machine.animation_index(step.transition).unwrap_or(0) as u32;
        self.world.anim.push(AnimCommand::PlayOwned {
            owner: self.me,
            index,
            reverse: step.reverse,
            speed: self.speed,
            tag: step.transition as u32,
        });
    }

    fn stop(&mut self, step: Step) {
        self.world.anim.push(AnimCommand::Stop { owner: self.me, tag: step.transition as u32 });
    }

    fn entered(&mut self, from: i16, to: i16) {
        let me = Value::Object(Some(self.me.to_script()));
        self.world.raise(
            self.me,
            script::event::STATE_ENTERED,
            vec![me, Value::Int(from as i32), Value::Int(to as i32)],
        );
    }
}

impl Machine {
    /// Parses the machine record of `h`.
    pub fn new(world: &World, h: Handle) -> Option<Machine> {
        let rec = world.record(h)?;
        let r = world.level.record(rec);
        let machine = StateMachine::from_record(&world.level.bytes, r, &world.level.table).ok()?;
        let run = MachineRun::new(&machine);
        let o = world.options(h)?;
        let speed = o.f32_or(OPTION_SPEED as u16, 1.0);
        let mut m = Machine { machine, run, speed, rng: 0x9E37_79B9 ^ (h.index().wrapping_mul(0x85EB_CA6B) | 1) };
        if let Some(goal) = o.i16(OPTION_GOAL as u16) {
            m.run.goal = Some(goal);
        }
        Some(m)
    }

    fn with_host(
        &mut self,
        world: &mut World,
        me: Handle,
        f: impl FnOnce(&mut MachineRun, &StateMachine, &mut Host<'_>),
    ) {
        let mut host = Host { world, me, machine: &self.machine, speed: self.speed, rng: &mut self.rng };
        f(&mut self.run, &self.machine, &mut host);
    }

    /// Starts the machine (event 17).
    pub fn start(&mut self, world: &mut World, me: Handle) {
        world.raise(me, script::event::MACHINE_STARTED, vec![Value::Object(Some(me.to_script()))]);
        self.with_host(world, me, |run, m, host| run.start(m, host));
    }

    /// Starts the machine in `state` if it is not running, then sets the goal to that state (animation event 46).
    pub fn start_in(&mut self, world: &mut World, me: Handle, state: i16) {
        if !self.run.running {
            self.run.state = state;
            self.start(world, me);
        }
        self.with_host(world, me, |run, m, host| run.set_goal(m, host, Some(state)));
    }

    /// Stops the machine (event 18).
    pub fn stop(&mut self, world: &mut World, me: Handle) {
        self.with_host(world, me, |run, _, host| run.stop(host));
        world.raise(me, script::event::MACHINE_STOPPED, Vec::new());
    }
}

impl ClassBehavior for Machine {
    fn ticks(&self) -> bool {
        self.run.running
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        if self.machine.auto_start
            || cx.world.options(cx.me).is_some_and(|o| o.bool_or(OPTION_AUTO_START as u16, false))
        {
            self.start(cx.world, cx.me);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.with_host(cx.world, cx.me, |run, m, host| run.update(m, host));
    }

    fn option_changed(&mut self, world: &mut World, me: Handle, id: u16, value: &OptionValue) {
        if id == OPTION_GOAL as u16 {
            // Setting the goal never starts a stopped machine (scripts.md 9); a stopped one keeps the goal for when
            // it starts.
            let goal = value.as_i32().map(|g| g as i16).filter(|&g| g > 0);
            self.with_host(world, me, |run, m, host| run.set_goal(m, host, goal));
        } else if id == OPTION_SPEED as u16 {
            self.speed = value.as_f32().unwrap_or(1.0);
        }
    }

    fn action(&mut self, _world: &mut World, _me: Handle, _id: u16, _args: &[Arg]) -> bool {
        false
    }

    fn animation_finished(&mut self, world: &mut World, me: Handle, tag: u32) {
        if self.run.step.is_some_and(|s| s.transition as u32 == tag) {
            self.with_host(world, me, |run, m, host| run.animation_finished(m, host));
        }
    }
}
