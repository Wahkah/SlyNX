//! Alarms (class 47) and their sensors: lasers (49), searchlights (50) and pressure plates (51)
//! (alarms-sensors.md).
//!
//! An alarm is disabled, armed or triggered. Its sensors (option 290) run the cycle of section 3.2: switching on,
//! on, tripped, switching off, off, in "detect" mode (a detection trips the alarm) or, once the alarm went off,
//! "damage" mode (a detection hurts the player). Alarm boxes (option 289, breakables of class 34) disable the alarm
//! for good when all are broken. Guard groups trigger, re-arm and disable their alarms (guards.md 2).

use alloc::vec::Vec;

use super::{ClassBehavior, Cx, dist_sq};
use crate::math::{dot, length, scale, sub};
use crate::object::class::id;
use crate::object::{Arg, Handle, OptionValue};
use crate::world::shared::{INTENSITY_ALARM, damage};
use crate::world::{Notice, World};

/// Alarm and sensor options (alarms-sensors.md).
pub mod opt {
    /// Alarm boxes (OIDs).
    pub const BOXES: u16 = 289;
    /// Linked sensors (OIDs).
    pub const SENSORS: u16 = 290;
    /// Silent alarm.
    pub const SILENT: u16 = 292;
    /// Reset time.
    pub const RESET: u16 = 293;
    /// Script call: trigger.
    pub const TRIGGER: u16 = 296;
    /// State-machine goals (typed arguments: on-trigger flag, root, machine, goal).
    pub const GOALS: u16 = 297;
    /// Objects that trigger a sensor (OIDs).
    pub const TRIGGER_OBJECTS: u16 = 299;
    /// Objects that never trigger it (OIDs).
    pub const IGNORE_OBJECTS: u16 = 300;
    /// Classes that trigger it.
    pub const TRIGGER_CLASSES: u16 = 301;
    /// Classes that never trigger it.
    pub const IGNORE_CLASSES: u16 = 302;
    /// Anything triggers it.
    pub const ANYTHING: u16 = 303;
    /// Switch-on time.
    pub const ON_TIME: u16 = 304;
    /// Switch-off time.
    pub const OFF_TIME: u16 = 305;
    /// Starting state (8: off).
    pub const START_STATE: u16 = 306;
    /// Scripts: switch on, off.
    pub const SWITCH_ON: u16 = 311;
    /// See [`SWITCH_ON`].
    pub const SWITCH_OFF: u16 = 312;
    /// Scripts: extend and retract a laser's beams (the argument: seconds to do it).
    pub const EXTEND: u16 = 315;
    /// See [`EXTEND`].
    pub const RETRACT: u16 = 316;
    /// Laser kind: 0 moving, 1 fixed.
    pub const LASER_KIND: u16 = 313;
    /// Searchlight focus, zap and unfocus times (defaults [`super::FOCUS_TIME`], [`super::ZAP_TIME`],
    /// [`super::UNFOCUS_TIME`]).
    pub const FOCUS: u16 = 317;
    /// See [`FOCUS`].
    pub const ZAP: u16 = 318;
    /// See [`FOCUS`].
    pub const UNFOCUS: u16 = 319;
    /// Pressure plate "stay on" and "stay off" times.
    pub const STAY_ON: u16 = 335;
    /// See [`STAY_ON`].
    pub const STAY_OFF: u16 = 336;
}

/// Searchlight focus time without option 317 (alarms-sensors.md 5): 0.25 s.
pub const FOCUS_TIME: f32 = 0.25;
/// Searchlight zap time without option 318: 1 s.
pub const ZAP_TIME: f32 = 1.0;
/// Searchlight unfocus time without option 319: 0.75 s.
pub const UNFOCUS_TIME: f32 = 0.75;

/// Alarm state-machine goals (alarms-sensors.md 2.2): fixed constants, not options.
pub const GOAL_READY: i16 = 538;
/// Goal when triggered.
pub const GOAL_TRIGGERED: i16 = 543;
/// Goal when disabled.
pub const GOAL_DISABLED: i16 = 544;
/// Siren sound.
pub const SIREN: i32 = 21;
/// Laser hum.
pub const LASER_SOUND: i32 = 26;
/// Searchlight zap.
pub const ZAP_SOUND: i32 = 27;
/// Laser tests per second.
pub const LASER_RATE: f32 = 30.0;
/// No sensor uses a player radius (alarms-sensors.md): lasers test their beams against the player's mesh,
/// searchlights the player's origin, plates real contacts. Without a player mesh (the probe, or no collision
/// world), the player's stand-in sphere is used ([`super::hazard::STAND_IN`]).
/// A sensor switched off within this time of its alarm being disabled stays off.
pub const STAY_OFF_WINDOW: f32 = 0.2;

/// An alarm's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlarmState {
    /// Off for good (or until scripts re-arm it).
    Disabled,
    /// Watching.
    Armed,
    /// Ringing.
    Triggered,
}

/// An alarm.
#[derive(Clone, Debug, PartialEq)]
pub struct Alarm {
    /// State.
    pub state: AlarmState,
    /// Linked sensors.
    pub sensors: Vec<Handle>,
    /// Alarm boxes.
    pub boxes: Vec<Handle>,
    /// Time in the triggered state.
    pub triggered_for: f32,
    /// Seconds since it was disabled.
    pub disabled_for: f32,
    changed: bool,
}

/// A sensor's mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorMode {
    /// A detection trips the alarm.
    Detect,
    /// A detection hurts the player.
    Damage,
}

/// A sensor's phase (alarms-sensors.md 3.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Switching on, for this long.
    SwitchingOn(f32),
    /// On.
    On,
    /// Just detected (detect mode).
    Tripped,
    /// Switching off, for this long.
    SwitchingOff(f32),
    /// Off, for this long.
    Off(f32),
}

/// Sensor kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensorKind {
    /// Laser (class 49).
    Laser,
    /// Searchlight (class 50).
    Searchlight,
    /// Pressure plate (class 51).
    Plate,
}

/// Searchlight damage cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zap {
    /// Watching.
    Watch,
    /// Focusing, for this long.
    Focus(f32),
    /// Zapping, for this long.
    Zap(f32),
    /// Unfocusing, for this long.
    Unfocus(f32),
}

/// A sensor.
#[derive(Clone, Debug, PartialEq)]
pub struct Sensor {
    /// Kind.
    pub kind: SensorKind,
    /// Mode.
    pub mode: SensorMode,
    /// Phase.
    pub phase: Phase,
    /// Its alarm.
    pub alarm: Option<Handle>,
    /// Off for good (switched off with its alarm, or a laser switched off).
    pub stay_off: bool,
    /// Laser test accumulator.
    pub test_clock: f32,
    /// Searchlight damage cycle.
    pub zap: Zap,
    /// Pressure plate cycle: time in the current on/off half.
    pub cycle: f32,
    /// Detected this frame.
    pub detecting: bool,
    /// How far a laser's beams reach along their curves, 0 (retracted) to 1 (extended).
    pub extension: f32,
    /// Where the extension is going and how fast (per second).
    extend_to: (f32, f32),
    hum: bool,
}

/// Changes an alarm's state, with the effects of alarms-sensors.md 2.2.
pub fn set_alarm(world: &mut World, h: Handle, state: AlarmState) {
    let Some(s) = world.slot(h) else { return };
    let mut b = world.take_behavior(s);
    if let super::Behavior::Alarm(a) = &mut b {
        a.set(world, h, state);
    }
    world.put_behavior(s, b);
}

/// Applies the alarm's state-machine goals (option 297) for the trigger (`on_trigger`) or the reset.
fn apply_goals(world: &mut World, me: Handle, on_trigger: bool) {
    let goals: Vec<(bool, i16, i16)> = world
        .options(me)
        .map(|o| {
            o.all(opt::GOALS)
                .filter_map(|v| {
                    let OptionValue::Args(a) = v else { return None };
                    let flag = matches!(a.first(), Some(Arg::Bool(true)));
                    let shorts: Vec<i16> =
                        a.iter().filter_map(|x| if let Arg::Short(_, n) = x { Some(*n) } else { None }).collect();
                    Some((flag, *shorts.get(1)?, *shorts.get(2)?))
                })
                .collect()
        })
        .unwrap_or_default();
    for (flag, machine, goal) in goals {
        if flag == on_trigger
            && let Some(m) = world.find_nearest(machine, Some(me))
        {
            world.set_option(m, 516, OptionValue::Short(goal));
        }
    }
}

fn set_own_goal(world: &mut World, me: Handle, goal: i16) {
    for m in world.children_of_class(me, id::STATE_MACHINE) {
        world.set_option(m, 516, OptionValue::Short(goal));
    }
}

impl Alarm {
    /// An armed alarm (links are found after load).
    pub fn new() -> Alarm {
        Alarm {
            state: AlarmState::Armed,
            sensors: Vec::new(),
            boxes: Vec::new(),
            triggered_for: 0.0,
            disabled_for: 0.0,
            changed: false,
        }
    }

    fn set(&mut self, world: &mut World, me: Handle, state: AlarmState) {
        if state == self.state {
            return;
        }
        let old = self.state;
        self.state = state;
        self.changed = true;
        if old == AlarmState::Triggered {
            world.shared.intensity.remove(&me.index());
            apply_goals(world, me, false);
        }
        match state {
            AlarmState::Armed => {
                for &s in &self.sensors {
                    sensor_switch(world, s, true, SensorMode::Detect);
                }
                set_own_goal(world, me, GOAL_READY);
            }
            AlarmState::Triggered => {
                self.triggered_for = 0.0;
                if !world.options(me).is_some_and(|o| o.bool_or(opt::SILENT, false)) {
                    world.play_sound(SIREN, Some(me));
                }
                world.shared.intensity.insert(me.index(), INTENSITY_ALARM);
                for &s in &self.sensors {
                    sensor_switch(world, s, true, SensorMode::Damage);
                }
                apply_goals(world, me, true);
                world.raise(me, script::event::TRIGGERED, Vec::new());
                world.shared.alarms_triggered += 1;
                set_own_goal(world, me, GOAL_TRIGGERED);
            }
            AlarmState::Disabled => {
                self.disabled_for = 0.0;
                for &s in &self.sensors {
                    sensor_switch(world, s, false, SensorMode::Detect);
                }
                world.raise(me, script::event::UNTRIGGERED, Vec::new());
                set_own_goal(world, me, GOAL_DISABLED);
            }
        }
    }
}

impl Default for Alarm {
    fn default() -> Self {
        Alarm::new()
    }
}

impl ClassBehavior for Alarm {
    fn ticks(&self) -> bool {
        true
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        self.boxes = o.shorts(opt::BOXES).take(4).filter_map(|b| cx.world.find_nearest(b, Some(cx.me))).collect();
        self.sensors = o.shorts(opt::SENSORS).take(16).filter_map(|b| cx.world.find_nearest(b, Some(cx.me))).collect();
        for &s in &self.sensors {
            if let Some(super::Behavior::Sensor(sn)) = cx.world.behavior_mut(s) {
                sn.alarm = Some(cx.me);
            }
        }
        let world_id = cx.world.info.id;
        let disabled = cx
            .world
            .persist_key(cx.me)
            .is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.pressed.contains(&k)));
        if disabled {
            self.state = AlarmState::Armed;
            self.set(cx.world, cx.me, AlarmState::Disabled);
            cx.world.events.retain(|e| e.object != cx.me.to_script() || e.event != script::event::UNTRIGGERED);
            cx.raise(script::event::SAVED_TRIGGER, Vec::new());
        } else {
            self.state = AlarmState::Disabled;
            self.set(cx.world, cx.me, AlarmState::Armed);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.changed {
            self.changed = false;
            if self.state == AlarmState::Disabled
                && let Some(k) = cx.world.persist_key(cx.me)
            {
                let world_id = cx.world.info.id;
                cx.progress.level(world_id).pressed.insert(k);
            }
        }
        match self.state {
            AlarmState::Triggered => {
                self.triggered_for += cx.dt;
                let reset = cx.world.options(cx.me).map_or(0.0, |o| o.f32_or(opt::RESET, 0.0));
                if reset > 0.0 && self.triggered_for >= reset {
                    self.set(cx.world, cx.me, AlarmState::Armed);
                }
            }
            AlarmState::Disabled => self.disabled_for += cx.dt,
            AlarmState::Armed => {}
        }
        // Every box broken: disabled for good.
        if self.state != AlarmState::Disabled
            && !self.boxes.is_empty()
            && self.boxes.iter().all(|&b| !cx.world.in_world(b) || super::breakable::is_broken(cx.world, b))
        {
            self.set(cx.world, cx.me, AlarmState::Disabled);
        }
    }

    fn action(&mut self, world: &mut World, me: Handle, id: u16, _args: &[Arg]) -> bool {
        if id == opt::TRIGGER {
            self.set(world, me, AlarmState::Triggered);
            return true;
        }
        false
    }
}

/// Switches a sensor on (in a mode) or off.
pub fn sensor_switch(world: &mut World, h: Handle, on: bool, mode: SensorMode) {
    let disabled_recently = {
        let Some(super::Behavior::Sensor(s)) = world.behavior(h) else { return };
        s.alarm.and_then(|a| match world.behavior(a) {
            Some(super::Behavior::Alarm(al)) => {
                Some(al.disabled_for <= STAY_OFF_WINDOW && al.state == AlarmState::Disabled)
            }
            _ => None,
        })
    };
    let Some(super::Behavior::Sensor(s)) = world.behavior_mut(h) else { return };
    if on {
        if s.stay_off && s.kind == SensorKind::Laser {
            return;
        }
        s.mode = mode;
        s.stay_off = false;
        if !matches!(s.phase, Phase::On) || mode == SensorMode::Damage {
            s.phase = Phase::SwitchingOn(0.0);
        }
    } else {
        s.phase = Phase::SwitchingOff(0.0);
        if disabled_recently == Some(true) {
            s.stay_off = true;
        }
    }
}

impl Sensor {
    /// A sensor of a class.
    pub fn new(world: &World, h: Handle) -> Sensor {
        let kind = match world.class(h) {
            Some(49) => SensorKind::Laser,
            Some(50) => SensorKind::Searchlight,
            _ => SensorKind::Plate,
        };
        let starts_off = world.options(h).and_then(|o| o.i32(opt::START_STATE)) == Some(8);
        Sensor {
            kind,
            mode: SensorMode::Detect,
            phase: if starts_off { Phase::Off(0.0) } else { Phase::SwitchingOn(0.0) },
            alarm: None,
            stay_off: false,
            test_clock: 0.0,
            zap: Zap::Watch,
            cycle: 0.0,
            detecting: false,
            extension: 1.0,
            extend_to: (1.0, 0.0),
            hum: false,
        }
    }

    /// Whether an object may trigger the sensor (alarms-sensors.md 3.1). `None` is the player's stand-in.
    fn triggers(cx: &Cx<'_>, who: Option<Handle>) -> bool {
        let o = cx.world.options(cx.me);
        let oid = who.and_then(|h| cx.world.oid(h)).unwrap_or(5);
        let class = who.and_then(|h| cx.world.class(h)).unwrap_or(id::PLAYER);
        let Some(o) = o else { return oid == 5 };
        if o.bool_or(opt::ANYTHING, false) {
            return true;
        }
        if o.shorts(opt::IGNORE_OBJECTS).any(|x| x == oid) || o.shorts(opt::IGNORE_CLASSES).any(|x| x == class) {
            return false;
        }
        o.shorts(opt::TRIGGER_OBJECTS).any(|x| x == oid) || o.shorts(opt::TRIGGER_CLASSES).any(|x| x == class)
    }

    fn player(cx: &Cx<'_>) -> Option<(Option<Handle>, [f32; 3])> {
        match cx.world.player_target() {
            Some(p) => Some((Some(p), cx.world.position(p)?)),
            None => cx.world.probe.map(|p| (None, p)),
        }
    }

    /// The laser's beams: the polylines of its curve-shape children, in world space.
    pub fn beams(world: &World, me: Handle) -> Vec<Vec<[f32; 3]>> {
        let mut out = Vec::new();
        for c in world.children(me) {
            if !world.class(c).is_some_and(|k| (110..=113).contains(&k)) {
                continue;
            }
            let pts = curve_points(world, c);
            if pts.len() >= 2 {
                out.push(pts);
            }
        }
        out
    }

    fn detect(&mut self, cx: &mut Cx<'_>) -> bool {
        let Some((who, p)) = Self::player(cx) else { return false };
        if !Self::triggers(cx, who) {
            return false;
        }
        match self.kind {
            SensorKind::Laser => {
                if cx.world.shared.player.hiding != crate::world::Hiding::None {
                    return false;
                }
                for beam in Self::beams(cx.world, cx.me) {
                    let beam = cut_polyline(&beam, self.extension);
                    for w in beam.windows(2) {
                        let (a, b) = (w[0], w[1]);
                        let ab = sub(b, a);
                        let l2 = dot(ab, ab);
                        let (r, up) = super::hazard::STAND_IN;
                        let c = [p[0], p[1], p[2] + up];
                        let t = if l2 > 0.0 { (dot(sub(c, a), ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
                        let q = crate::math::madd(a, ab, t);
                        let stand_in = dist_sq(c, q) <= r * r;
                        let hit = match who {
                            Some(h) => cx.collision.sphere_touches(cx.world, h, q, 1.0).unwrap_or(stand_in),
                            None => stand_in,
                        };
                        if hit {
                            let mut ignore = alloc::vec![cx.me];
                            ignore.extend(who);
                            if cx.collision.line_clear(cx.world, a, q, &ignore).unwrap_or(true) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            SensorKind::Searchlight => {
                if cx.world.shared.player.fire_immune {
                    return false;
                }
                let inside = cx.collision.point_in_solid(cx.world, cx.me, p).unwrap_or(false);
                if !inside {
                    return false;
                }
                let me = cx.my_position();
                if let Some(h) = who
                    && let Some(w) = cx.world.world_transform(h)
                    && super::guard::Guard::hidden_from(cx, &w, me)
                {
                    return false;
                }
                let mut ignore = alloc::vec![cx.me];
                ignore.extend(who);
                cx.collision.line_clear(cx.world, me, p, &ignore).unwrap_or(true)
            }
            SensorKind::Plate => {
                let touched =
                    cx.world.notices(cx.me).iter().any(|n| matches!(n, Notice::Touched(t) if Some(*t) == who));
                touched
                    || (who.is_none()
                        && cx.collision.ground(cx.world, p, 200.0).is_some_and(|(_, _, h)| h == Some(cx.me)))
            }
        }
    }

    fn damage_kind(&self) -> u8 {
        match self.kind {
            SensorKind::Plate => damage::ELECTRIC,
            _ => damage::FIRE,
        }
    }
}

/// The control points of a curve shape (classes/paths-and-maps.md 1, shared-structures.md "Curve") in world space.
/// Cubic curves give their control points (the curve through them is not sampled yet).
pub fn curve_points(world: &World, h: Handle) -> Vec<[f32; 3]> {
    let Some(rec) = world.record(h) else { return Vec::new() };
    let r = world.level.record(rec);
    let data = &world.level.bytes;
    let mut rd = formats::reader::Reader::at(data, r.offset + 6);
    let parsed = (|| -> formats::Result<Vec<[f32; 3]>> {
        let kind = rd.u8()?;
        let _closed = rd.u8()?;
        let n = rd.u8()?;
        let mut v = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let _param = rd.f32()?;
            v.push(rd.vec3()?);
            if kind == 1 {
                rd.skip(24)?;
            }
        }
        Ok(v)
    })();
    let w = world.world_transform(h).unwrap_or(formats::geometry::Affine::IDENTITY);
    parsed.unwrap_or_default().into_iter().map(|p| w.transform_point(p)).collect()
}

impl ClassBehavior for Sensor {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let (to, rate) = self.extend_to;
        if self.extension != to {
            let step = if rate > 0.0 { rate * cx.dt } else { 1.0 };
            self.extension =
                if self.extension < to { (self.extension + step).min(to) } else { (self.extension - step).max(to) };
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let on_time = o.f32_or(opt::ON_TIME, 0.0);
        let off_time = o.f32_or(opt::OFF_TIME, 0.0);
        self.detecting = false;
        // Pressure plates' timed cycle.
        if self.kind == SensorKind::Plate {
            let stay_on = o.f32_or(opt::STAY_ON, -1.0);
            if stay_on >= 0.0 {
                self.cycle += cx.dt;
                match self.phase {
                    Phase::On if self.cycle >= stay_on => {
                        self.phase = Phase::SwitchingOff(0.0);
                        self.cycle = 0.0;
                    }
                    Phase::Off(_) if self.cycle >= o.f32_or(opt::STAY_OFF, 2.0) && !self.stay_off => {
                        self.phase = Phase::SwitchingOn(0.0);
                        self.cycle = 0.0;
                    }
                    _ => {}
                }
            }
        }
        self.phase = match self.phase {
            Phase::SwitchingOn(t) => {
                let t = t + cx.dt;
                if self.kind == SensorKind::Laser && self.mode == SensorMode::Detect && self.detect(cx) {
                    Phase::Tripped
                } else if t >= on_time {
                    Phase::On
                } else {
                    Phase::SwitchingOn(t)
                }
            }
            Phase::On => {
                let run = match self.kind {
                    SensorKind::Laser => {
                        self.test_clock += cx.dt;
                        if self.test_clock >= 1.0 / LASER_RATE {
                            self.test_clock -= 1.0 / LASER_RATE;
                            true
                        } else {
                            false
                        }
                    }
                    _ => true,
                };
                let hit = run && self.detect(cx);
                self.detecting = hit;
                match (self.mode, hit) {
                    (SensorMode::Detect, true) => Phase::Tripped,
                    (SensorMode::Damage, true) => {
                        self.hurt(cx);
                        Phase::On
                    }
                    _ => {
                        if self.kind == SensorKind::Searchlight && self.mode == SensorMode::Damage {
                            self.zap_cycle(cx, false);
                        }
                        Phase::On
                    }
                }
            }
            Phase::Tripped => {
                if let Some(a) = self.alarm {
                    set_alarm(cx.world, a, AlarmState::Triggered);
                }
                cx.raise(script::event::TRIGGERED, Vec::new());
                Phase::SwitchingOff(0.0)
            }
            Phase::SwitchingOff(t) => {
                let t = t + cx.dt;
                if t >= off_time {
                    if self.mode == SensorMode::Damage && !self.stay_off && self.alarm_triggered(cx) {
                        Phase::SwitchingOn(0.0)
                    } else {
                        Phase::Off(0.0)
                    }
                } else {
                    Phase::SwitchingOff(t)
                }
            }
            Phase::Off(t) => {
                let t = t + cx.dt;
                // Searchlights switch themselves back on unless switched off with their alarm.
                if self.kind == SensorKind::Searchlight && !self.stay_off && !self.alarm_disabled(cx) {
                    Phase::SwitchingOn(0.0)
                } else {
                    Phase::Off(t)
                }
            }
        };
        let on = matches!(self.phase, Phase::On | Phase::SwitchingOn(_));
        if self.kind == SensorKind::Laser && on != self.hum {
            self.hum = on;
            if on {
                cx.world.play_sound(LASER_SOUND, Some(cx.me));
            }
        }
    }

    fn action(&mut self, world: &mut World, me: Handle, id: u16, _args: &[Arg]) -> bool {
        match id {
            opt::SWITCH_ON => {
                self.stay_off = false;
                let mode = self.mode;
                sensor_switch_inner(self, mode, true);
                let _ = (world, me);
                true
            }
            opt::SWITCH_OFF => {
                sensor_switch_inner(self, self.mode, false);
                if self.kind == SensorKind::Laser {
                    self.stay_off = true;
                }
                true
            }
            opt::EXTEND | opt::RETRACT => {
                let t = _args.iter().find_map(|a| match *a {
                    Arg::Float(f) => Some(f),
                    Arg::Int(n) => Some(n as f32),
                    _ => None,
                });
                let to = if id == opt::EXTEND { 1.0 } else { 0.0 };
                self.extend_to = (to, t.filter(|&t| t > 0.0).map_or(0.0, |t| 1.0 / t));
                true
            }
            _ => false,
        }
    }
}

/// The first `fraction` (0 to 1) of a polyline's length.
pub fn cut_polyline(points: &[[f32; 3]], fraction: f32) -> Vec<[f32; 3]> {
    if fraction >= 1.0 || points.len() < 2 {
        return points.to_vec();
    }
    let total: f32 = points.windows(2).map(|w| length(sub(w[1], w[0]))).sum();
    let mut left = total * fraction.max(0.0);
    let mut out = alloc::vec![points[0]];
    for w in points.windows(2) {
        let l = length(sub(w[1], w[0]));
        if l >= left {
            if l > 0.0 {
                out.push(crate::math::madd(w[0], sub(w[1], w[0]), left / l));
            }
            break;
        }
        left -= l;
        out.push(w[1]);
    }
    out
}

fn sensor_switch_inner(s: &mut Sensor, mode: SensorMode, on: bool) {
    s.mode = mode;
    s.phase = if on { Phase::SwitchingOn(0.0) } else { Phase::SwitchingOff(0.0) };
}

impl Sensor {
    fn alarm_state(&self, cx: &Cx<'_>) -> Option<AlarmState> {
        match cx.world.behavior(self.alarm?) {
            Some(super::Behavior::Alarm(a)) => Some(a.state),
            _ => None,
        }
    }

    fn alarm_triggered(&self, cx: &Cx<'_>) -> bool {
        self.alarm_state(cx) == Some(AlarmState::Triggered)
    }

    fn alarm_disabled(&self, cx: &Cx<'_>) -> bool {
        self.alarm_state(cx) == Some(AlarmState::Disabled)
    }

    fn hurt(&mut self, cx: &mut Cx<'_>) {
        if cx.world.shared.player.invulnerable {
            return;
        }
        match self.kind {
            SensorKind::Searchlight => self.zap_cycle(cx, true),
            _ => cx.world.hurt_player(cx.me, self.damage_kind(), None),
        }
    }

    /// The searchlight's damage cycle (alarms-sensors.md 5): focus, zap, unfocus.
    fn zap_cycle(&mut self, cx: &mut Cx<'_>, detected: bool) {
        let o = cx.world.options(cx.me);
        let (focus, zap, unfocus) = o.map_or((FOCUS_TIME, ZAP_TIME, UNFOCUS_TIME), |o| {
            (o.f32_or(opt::FOCUS, FOCUS_TIME), o.f32_or(opt::ZAP, ZAP_TIME), o.f32_or(opt::UNFOCUS, UNFOCUS_TIME))
        });
        self.zap = match self.zap {
            Zap::Watch if detected => Zap::Focus(0.0),
            Zap::Watch => Zap::Watch,
            Zap::Focus(_) if cx.world.shared.player.invulnerable => Zap::Unfocus(0.0),
            Zap::Focus(t) if t + cx.dt >= focus => {
                cx.world.play_sound(ZAP_SOUND, Some(cx.me));
                cx.world.hurt_player(cx.me, damage::FIRE, None);
                Zap::Zap(0.0)
            }
            Zap::Focus(t) => Zap::Focus(t + cx.dt),
            Zap::Zap(t) if t + cx.dt >= zap => Zap::Unfocus(0.0),
            Zap::Zap(t) => Zap::Zap(t + cx.dt),
            Zap::Unfocus(t) if t + cx.dt >= unfocus => Zap::Watch,
            Zap::Unfocus(t) => Zap::Unfocus(t + cx.dt),
        };
        let _ = (length, scale);
    }
}
