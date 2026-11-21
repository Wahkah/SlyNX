//! World-wide state that behaviours share: what the player is doing (written by the player system), damage dealt to
//! the player (read by it), guard alert groups, music intensity requests, and the screen effects.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::object::Handle;

/// How the player is hiding (guards.md 4.3).
/// A task method a script called (Sly 3; specs/sly2/missions.md 3.1 for the meanings), by the task's ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskCall {
    /// Request a state (−1 or 4: become current, 5: stop being current); `forced` skips the checks.
    Request { task: i32, state: i32, forced: bool },
    /// Force a state (0 to 3).
    Force { task: i32, state: i8 },
    /// Become current.
    MakeCurrent(i32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hiding {
    /// Not hiding.
    #[default]
    None,
    /// Flattened against a wall: hidden from guards on the wall side (behind the player's facing).
    Wall {
        /// The spot is marked detectable.
        detectable: bool,
    },
    /// Ducking in cover: hidden from guards in front of the player.
    Cover {
        /// The spot is marked detectable.
        detectable: bool,
    },
    /// In a hiding barrel or basket, standing still.
    Basket,
    /// The casual-hiding pose (hidden only with the power-up mask `0x12000`).
    Casual,
}

/// What the player system tells the world about the player each frame. Without a player system, the defaults
/// hold: visible, vulnerable, on the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerState {
    /// Hiding mode.
    pub hiding: Hiding,
    /// Cannot be hurt (being hurt already, a power-up).
    pub invulnerable: bool,
    /// Immune to fire (searchlight zaps).
    pub fire_immune: bool,
    /// In the air.
    pub airborne: bool,
    /// The base contact point, if known (else the position, or 70 below it while airborne).
    pub base: Option<[f32; 3]>,
    /// The game has taken control away (a cutscene, a confrontation dialogue).
    pub controls_locked: bool,
    /// Held by an enemy (never outside a leash).
    pub grabbed: bool,
    /// Velocity, for search directions.
    pub velocity: [f32; 3],
    /// Vault reward bits owned (power-ups).
    pub rewards: u32,
    /// Hit by an attack this frame (for attack-triggered objects).
    pub attacking: bool,
}

/// Damage kinds (interactables.md 5).
pub mod damage {
    /// Blunt.
    pub const BLUNT: u8 = 0;
    /// Electric.
    pub const ELECTRIC: u8 = 1;
    /// Fire.
    pub const FIRE: u8 = 2;
    /// Water.
    pub const WATER: u8 = 3;
    /// Crush.
    pub const CRUSH: u8 = 4;
    /// Falling into a pit.
    pub const PIT: u8 = 5;
    /// Thrown.
    pub const THROWN: u8 = 6;
}

/// Damage dealt to an object (the player, mostly), for the player system or the object's behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Damage {
    /// Who is hurt (`None`: the player, when there is no player object yet).
    pub target: Option<Handle>,
    /// Who hurt it (a guard, a hazard, a laser).
    pub source: Handle,
    /// Damage kind ([`damage`]).
    pub kind: u8,
    /// Where the target should be thrown to recover (hazards' recovery points), if anywhere.
    pub recover_to: Option<[f32; 3]>,
    /// The velocity the hit throws the target at (guards' attack effects, guards.md 6.2), if it throws.
    pub throw: Option<[f32; 3]>,
}

/// A guard group's shared alert level (guards.md 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    /// Nobody has seen the player.
    #[default]
    Patrol,
    /// A member sees the player now.
    Attack,
    /// Seen, then lost.
    Search,
    /// Every member is gone.
    Dead,
}

/// A guard group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlertGroup {
    /// The group object (class 137), if it has one.
    pub object: Option<Handle>,
    /// Members (guard slots).
    pub members: Vec<u32>,
    /// Alert level.
    pub level: AlertLevel,
    /// Members that detect the player (latest report).
    pub seeing: Vec<u32>,
    /// Members done searching.
    pub done_searching: Vec<u32>,
    /// A member asked to give up now.
    pub give_up: bool,
    /// Where the player was last seen, and its velocity then.
    pub last_seen: Option<([f32; 3], [f32; 3])>,
    /// Alarms linked to the group (slots).
    pub alarms: Vec<u32>,
    /// Bumped on every level change, so members notice transitions.
    pub serial: u32,
}

/// The music intensity requests of objects (audio.md 5.4): the highest active one wins.
pub const INTENSITY_ALARM: i32 = 107;
/// Quiet sneaking music near flashlight guards.
pub const INTENSITY_SNEAK: i32 = -20;

/// A full-screen colour (lightning flashes, effects.md 4), RGBA 0..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenTint {
    /// Colour and alpha.
    pub color: [f32; 4],
    /// Drawn in front of the world (else behind it).
    pub front: bool,
}

/// World-wide screen effects for the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenEffects {
    /// World darkening factor (effects.md 8): multiplies every mesh's colour except those flagged.
    pub darkening: f32,
    /// Target the darkening eases toward.
    pub darkening_target: f32,
    /// Lightning flashes showing now.
    pub tints: Vec<ScreenTint>,
    /// Letterbox bars shown (animation events 49 and 50, dialogue events 5 and 6).
    pub letterbox: bool,
}

impl Default for ScreenEffects {
    fn default() -> Self {
        ScreenEffects { darkening: 1.0, darkening_target: 1.0, tints: Vec::new(), letterbox: false }
    }
}

impl ScreenEffects {
    /// Eases the darkening toward its target (effects.md 8): 2.0 per second toward darker, 1.0 toward brighter,
    /// slowing linearly to 0 over the last 0.1 of travel.
    pub fn step(&mut self, dt: f32) {
        let d = self.darkening_target - self.darkening;
        if d == 0.0 {
            return;
        }
        let rate = if d < 0.0 { 2.0 } else { 1.0 };
        let dist = if d < 0.0 { -d } else { d };
        let speed = if dist < 0.1 { rate * dist / 0.1 } else { rate };
        let step = (speed * dt).max(1e-4).min(dist);
        self.darkening += if d < 0.0 { -step } else { step };
    }
}

/// Shared state, one per world.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared {
    /// The player's state (player system).
    pub player: PlayerState,
    /// Sly 2 noises (guards.md 4).
    pub noises: Vec<crate::behavior::sly2::Noise>,
    /// The Sly 2 guards' pocket table (guards.md 8, class 58 row set), set by the game before the world starts.
    pub pockets: Option<crate::gang::PocketTable>,
    /// Sly 2 episode of the world (selects the pocket table row).
    pub episode: u8,
    /// Coins spilled by knocked-out guards: where, how many (picked up by the pickup behaviours).
    pub coin_drops: Vec<([f32; 3], u32)>,
    /// Pocket entries the player took (the game launches them toward Sly, `gang::pocket::give`).
    pub picked: Vec<crate::gang::Entry>,
    /// Guards knocked out so far.
    pub guards_knocked_out: u32,
    /// Hits on the Sly 2 gang from the world (lasers, hazards), for the game's gang system.
    pub gang_hits: Vec<crate::behavior::sly2_sensors::GangHit>,
    /// Sly 2 medicine the played character touches this frame (the gang system takes it or not).
    pub medicine: Vec<Handle>,
    /// Sly 2 task items collected, for the job system.
    pub task_items: Vec<Handle>,
    /// Sly 2 task-state requests (task, state) from props and volumes, for the job system.
    pub task_requests: Vec<(i32, i32)>,
    /// Sly 3 hook (job system): the task methods scripts called (`TaskCall`), for the session's job system, which
    /// lives outside the world.
    pub task_calls: Vec<TaskCall>,
    /// Sly 3 hook (missions.md 4.2): how many holds player 0's control has (script command 0x1F, menus), set by the
    /// session each frame; the world object's method 1000 answers whether it is above 0.
    pub sly3_holds: u32,
    /// Sly 3 hook (job system): each task's state by object ID, mirrored by the session for scripts (task option
    /// 1415, missions.md 4.1a).
    pub sly3_task_states: BTreeMap<i32, i8>,
    /// Sly 3 hook: command scripts started by scripts (method 747 on class 135): (script, object ID or −1), for the
    /// session's runner.
    pub sly3_script_starts: Vec<(i32, i32)>,
    /// Sly 2 sequence starts, stops and alarms for the game's sequence player.
    pub sequence_requests: Vec<crate::behavior::sly2_commands::SequenceRequest>,
    /// Sly 2 meshes hidden by scripts (options 44 and 45): (object slot, mesh object ID).
    pub hidden_meshes: Vec<(u32, i32)>,
    /// Sly 2 music streams scripts queued (action 146), for the game's sound.
    pub music_requests: Vec<i16>,
    /// Sly 2 vault changes, for the session (sequences, lines, the dial's screen).
    pub vault_events: Vec<crate::behavior::sly2_vault::VaultEvent>,
    /// Sly 2: the safe house is the current player (no character takes damage; characters.md 2).
    pub safe_house: bool,
    /// Sly 2 / Sly 3: the object mask the camera's visibility volumes select (camera.md 7.1; `0xFF` in none).
    /// Objects whose zone mask shares no bit with it are not updated.
    pub visibility_mask: u32,
    /// Damage dealt this frame, in order.
    pub damage: Vec<Damage>,
    /// Guard groups.
    pub groups: Vec<AlertGroup>,
    /// Music intensity requests by object slot.
    pub intensity: BTreeMap<u32, i32>,
    /// Alarms triggered in this world (the game's alarm counter).
    pub alarms_triggered: u32,
    /// Screen effects.
    pub screen: ScreenEffects,
    /// The dialogue playing (one at a time).
    pub dialogue: Option<Handle>,
    /// The subtitle showing.
    pub subtitle: Option<crate::behavior::dialogue::Subtitle>,
    /// The dialogue picture panels: (speaker, replacement picture) left and right.
    pub panels: [Option<(i16, i16)>; 2],
    /// Voice lines to start (the sound engine reads and plays them).
    pub voice: Vec<formats::disc::Descriptor>,
    /// A voice line is playing (set by the game from the sound engine).
    pub voice_playing: bool,
    /// The confirm button went down this frame (dialogue lines that wait for a button).
    pub confirm_pressed: bool,
    /// The controller this frame.
    pub input: crate::game::systems::FrameInput,
    /// The camera's eye last frame (skies, emitters).
    pub camera_eye: [f32; 3],
    /// Game time at the start of this frame (for actions, which get no step).
    pub time: f32,
    /// The camera's vertical field of view last frame (emitters' draw-distance factor, effects.md 1.2).
    pub camera_fov: f32,
    /// The world's camera draw-distance ratio `k` (1 unless a script changes it).
    pub draw_distance_ratio: f32,
    /// The level's adaptive difficulty, 0 to 1 (world option 547, read by scripts as 634).
    pub difficulty: f32,
    /// The hiding basket the player is in (set by the player system).
    pub basket: Option<Handle>,
    /// Vehicles hook: the machine the player drives (`behavior::vehicles`); set by the session, read by the machines
    /// and by the gang players (which then get an idle pad).
    pub driving: Option<Handle>,
    /// Vehicles hook: the machines' projectiles and the job they follow (`behavior::vehicles::shots`).
    pub vehicles: crate::behavior::vehicles::shots::Shared,
    /// Live particles.
    pub particles: Vec<crate::behavior::effects::Particle>,
    /// Charms and lives, written by the game from the progress before the player decides.
    pub vitals: (i32, i32),
    /// The game-clock scale the player's power-ups ask for (1 normally).
    pub time_scale: f32,
    /// The player controller's events this frame, for the game to apply.
    pub player_events: Vec<crate::player::PlayerEvent>,
    /// The camera's heading last frame (camera-relative controls).
    pub camera_yaw: f32,
    /// The camera placement a cutscene enabled (animation event 12), until event 13.
    pub camera_placement: Option<Handle>,
    /// Camera hook: a driven machine the camera follows instead of the player (`camera::CameraSystem`).
    pub camera_follow: Option<Handle>,
    /// Camera hook: a mount the camera rides (a turret taking over the camera): the object and the eye's offset in
    /// its frame (forward, left, up).
    pub camera_mount: Option<(Handle, [f32; 3])>,
    /// Gang hook: each character's state parts (canes 316, 317, 318 and the throw marker 586), found once
    /// (`gang::parts`), by character slot.
    pub gang_parts: alloc::collections::BTreeMap<u32, [Option<Handle>; 4]>,
    /// Gang hook: the gadget copies out of their pools, with their carry states (`gang::pool`).
    pub gadget_copies: Vec<crate::gang::pool::GadgetCopy>,
    /// Camera hook: two characters to frame together (confrontations; camera.md 2, priority 6).
    pub camera_pair: Option<(Handle, Handle)>,
    /// Camera hook: the binocucom's focus while it is on, and whether its tilt is inverted (camera.md 6).
    pub camera_binocucom: Option<(Handle, bool)>,
    /// Camera hook: a timed move to run (camera.md 2); the camera starts it when this changes to a new move.
    pub camera_move: Option<crate::camera::views::TimedMove>,
    /// Sly 3 hook: launches by bouncers for the game to apply (`behavior::sly3::devices`).
    pub launches: Vec<crate::behavior::sly3::devices::Launch>,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            player: PlayerState::default(),
            noises: Vec::new(),
            pockets: None,
            episode: 0,
            coin_drops: Vec::new(),
            picked: Vec::new(),
            guards_knocked_out: 0,
            gang_hits: Vec::new(),
            medicine: Vec::new(),
            task_items: Vec::new(),
            task_requests: Vec::new(),
            task_calls: Vec::new(),
            sly3_holds: 0,
            sly3_task_states: BTreeMap::new(),
            sly3_script_starts: Vec::new(),
            sequence_requests: Vec::new(),
            hidden_meshes: Vec::new(),
            music_requests: Vec::new(),
            vault_events: Vec::new(),
            safe_house: false,
            visibility_mask: 0xFF,
            damage: Vec::new(),
            groups: Vec::new(),
            intensity: BTreeMap::new(),
            alarms_triggered: 0,
            screen: ScreenEffects::default(),
            dialogue: None,
            subtitle: None,
            panels: [None, None],
            voice: Vec::new(),
            voice_playing: false,
            confirm_pressed: false,
            input: Default::default(),
            camera_eye: [0.0; 3],
            camera_fov: 1.0,
            time: 0.0,
            draw_distance_ratio: 1.0,
            difficulty: 0.0,
            basket: None,
            driving: None,                // Vehicles hook
            vehicles: Default::default(), // Vehicles hook
            particles: Vec::new(),
            vitals: (0, 5),
            time_scale: 1.0,
            player_events: Vec::new(),
            camera_yaw: 0.0,
            camera_placement: None,
            launches: Vec::new(),           // Sly 3 hook
            camera_follow: None,            // camera hook
            camera_pair: None,              // camera hook
            gadget_copies: Vec::new(),      // gang hook
            gang_parts: Default::default(), // gang hook
            camera_binocucom: None,         // camera hook
            camera_move: None,              // camera hook
            camera_mount: None,             // camera hook
        }
    }
}

impl Shared {
    /// The highest active music intensity request (0 with none).
    pub fn intensity(&self) -> i32 {
        self.intensity.values().copied().fold(None, |m: Option<i32>, v| Some(m.map_or(v, |m| m.max(v)))).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darkening_eases_at_the_spec_rates() {
        let mut s = ScreenEffects { darkening_target: 0.3, ..Default::default() };
        s.step(0.1);
        assert!((s.darkening - 0.8).abs() < 1e-5, "{}", s.darkening);
        for _ in 0..100 {
            s.step(1.0 / 60.0);
        }
        assert!((s.darkening - 0.3).abs() < 1e-4);
        s.darkening_target = 1.0;
        s.step(0.1);
        assert!((s.darkening - 0.4).abs() < 1e-5);
        let mut sh = Shared::default();
        assert_eq!(sh.intensity(), 0);
        sh.intensity.insert(3, INTENSITY_SNEAK);
        assert_eq!(sh.intensity(), -20);
        sh.intensity.insert(4, INTENSITY_ALARM);
        assert_eq!(sh.intensity(), 107);
    }
}
