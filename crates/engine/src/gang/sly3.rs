//! The Sly 3 crew (specs/sly3/gameplay-overview.md 3 and 4): the three core characters, who follow Sly 2's scheme
//! and run on [`super::Character`], and the five recruits, who are present in their episodes' worlds.
//!
//! specs/sly3/characters.md leaves the recruits' moves and health open (its open questions 1 and 2), so the
//! recruits have no moves here: a [`Crew`] places them, lists them as targets to the others and refuses to switch
//! to them. Their rigs and clips work through [`crate::anim::xform::XformRig`] like the core characters'. The
//! executable's item table, lookup tables and constants (characters.md 3) and the arrival rule (2) are here.

use alloc::vec::Vec;

use super::character::{Character, GangEvent, Shared};
use super::consts::{GangConstants, GangTunables};
use super::input::{GangInput, GangWorld, Target, TargetKind};
use super::items::{Item, ItemExtra, ItemTable, Owned, behaviour};
use super::pocket::{LootTable, PocketTable};
use super::{Gang, Who};
use crate::math::Vec3;
use formats_sly3::exe::Executable;

/// A playable member of the Sly 3 crew, by character class (gameplay-overview.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Member {
    /// Class 52.
    Sly,
    /// Class 53.
    Murray,
    /// Class 54.
    Bentley,
    /// Class 55, the Guru.
    Guru,
    /// Class 56.
    PandaKing,
    /// Class 57.
    Penelope,
    /// Class 59.
    Dimitri,
    /// Class 60.
    Carmelita,
}

impl Member {
    /// Every member, in class order.
    pub const ALL: [Member; 8] = [
        Member::Sly,
        Member::Murray,
        Member::Bentley,
        Member::Guru,
        Member::PandaKing,
        Member::Penelope,
        Member::Dimitri,
        Member::Carmelita,
    ];

    /// The character class.
    pub const fn class(self) -> i16 {
        match self {
            Member::Sly => 52,
            Member::Murray => 53,
            Member::Bentley => 54,
            Member::Guru => 55,
            Member::PandaKing => 56,
            Member::Penelope => 57,
            Member::Dimitri => 59,
            Member::Carmelita => 60,
        }
    }

    /// The member of a character class.
    pub fn from_class(class: i16) -> Option<Member> {
        Member::ALL.into_iter().find(|m| m.class() == class)
    }

    /// The core character it is, if it is one (they follow Sly 2's scheme, gameplay-overview.md 4).
    pub const fn core(self) -> Option<Who> {
        match self {
            Member::Sly => Some(Who::Sly),
            Member::Murray => Some(Who::Murray),
            Member::Bentley => Some(Who::Bentley),
            _ => None,
        }
    }

    /// Whether it is one of the five recruits.
    pub const fn recruit(self) -> bool {
        self.core().is_none()
    }

    /// The target id a recruit is listed under (core characters use [`Who::target_id`]).
    pub const fn target_id(self) -> u32 {
        match self.core() {
            Some(w) => w.target_id(),
            None => 0x8000_0100 | self.class() as u32,
        }
    }
}

/// A recruit present in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recruit {
    /// Which one.
    pub member: Member,
    /// Feet position.
    pub position: Vec3,
    /// Heading.
    pub yaw: f32,
}

/// The rules the Sly 3 crew shares. The Sly 3 executable's addresses for the gang constants and the item table
/// are not in the specs, so the core characters use Sly 2's constants and an empty item table until they are.
pub fn shared() -> Shared {
    Shared {
        consts: GangConstants::SPEC,
        tun: GangTunables::default(),
        items: ItemTable { items: Vec::new(), extra: Vec::new() },
        owned: Owned::default(),
    }
}

/// The crew of a world: the core characters as a [`Gang`] and the recruits present.
#[derive(Clone, Debug, PartialEq)]
pub struct Crew {
    /// The core characters.
    pub gang: Gang,
    /// The recruits present.
    pub recruits: Vec<Recruit>,
}

impl Crew {
    /// A crew from the members present (position and heading each); the first core member is the player.
    pub fn new(present: &[(Member, Vec3, f32)], shared: Shared) -> Crew {
        let mut members = Vec::new();
        let mut recruits = Vec::new();
        for &(m, p, yaw) in present {
            match m.core() {
                Some(w) => members.push(Character::new(w, p, yaw)),
                None => recruits.push(Recruit { member: m, position: p, yaw }),
            }
        }
        Crew { gang: Gang::new(members, shared), recruits }
    }

    /// The member being played.
    pub fn player(&self) -> Member {
        match self.gang.player {
            Who::Sly => Member::Sly,
            Who::Murray => Member::Murray,
            Who::Bentley => Member::Bentley,
        }
    }

    /// Switches to `member`. The recruits cannot be played until the Sly 3 characters spec gives their moves.
    pub fn switch_to(&mut self, member: Member) -> bool {
        member.core().is_some_and(|w| self.gang.switch_to(w))
    }

    /// One frame. The recruits are listed to the core characters as targets.
    pub fn update(&mut self, dt: f32, input: &GangInput, world: &GangWorld<'_>) -> Vec<(Who, GangEvent)> {
        let mut targets: Vec<Target> = world.targets.to_vec();
        targets.extend(self.recruits.iter().map(|r| Target {
            id: r.member.target_id(),
            position: r.position,
            yaw: r.yaw,
            kind: TargetKind::Interactable,
        }));
        let w = GangWorld { targets: &targets, ..*world };
        self.gang.update(dt, input, &w)
    }
}

// ---- The Sly 3 executable's tables (specs/sly3/characters.md 3) ------------------------------------------------

/// The Sly 3 item table: 67 records of 60 bytes [X].
pub const ITEM_TABLE_VA: u32 = 0x0034_3098;
/// Records in the Sly 3 item table.
pub const ITEM_COUNT: usize = 67;

/// One Sly 3 item record (fifteen `s32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sly3Item {
    /// Button use: 2 press, 1 hold, −1 passive.
    pub button: i32,
    /// Owner: 24 Sly, 25 Bentley, 26 Murray, 28 four unnamed records, −1 none.
    pub owner: i32,
    /// Price in coins.
    pub price: i32,
    /// Gadget-power cost.
    pub cost: i32,
    /// The held items' drain in power per second (field +16; 3, 2, 3, 5 on items 32 to 35, 0 elsewhere; the
    /// Treasure Map, 36, is held without cost) [X].
    pub cost2: i32,
    /// Episode from which it is sold (−1 not sold).
    pub episode: i32,
    /// 14 or 15 on sold items, −1 otherwise.
    pub tier: i32,
    /// Name string ID.
    pub name: i32,
    /// Description string ID.
    pub description: i32,
    /// The usage text's string ID (gadgets.md 1; −1 on the attack upgrades).
    pub usage: i32,
    /// An object or effect ID, or 0.
    pub effect: i32,
    /// Two (id, count) pairs (meaning open).
    pub pairs: [i32; 4],
}

impl Sly3Item {
    /// The owner as a core member.
    pub fn owner(&self) -> Option<Member> {
        match self.owner {
            24 => Some(Member::Sly),
            25 => Some(Member::Bentley),
            26 => Some(Member::Murray),
            _ => None,
        }
    }
}

/// The Sly 3 item table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sly3ItemTable {
    /// The records.
    pub items: Vec<Sly3Item>,
}

impl Sly3ItemTable {
    /// Reads it from the Sly 3 executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<Sly3ItemTable> {
        if !formats_sly3::exe::is_supported(exe) {
            return Err(formats::Error::UnknownExecutable);
        }
        Ok(Sly3ItemTable::from_bytes(exe.bytes_at(ITEM_TABLE_VA, ITEM_COUNT * 60)?))
    }

    /// From raw records.
    pub fn from_bytes(raw: &[u8]) -> Sly3ItemTable {
        let items = raw
            .as_chunks::<60>()
            .0
            .iter()
            .map(|c| {
                let f = |i: usize| i32::from_le_bytes([c[i * 4], c[i * 4 + 1], c[i * 4 + 2], c[i * 4 + 3]]);
                Sly3Item {
                    button: f(0),
                    owner: f(1),
                    price: f(2),
                    cost: f(3),
                    cost2: f(4),
                    episode: f(5),
                    tier: f(6),
                    name: f(7),
                    description: f(8),
                    usage: f(9),
                    effect: f(10),
                    pairs: [f(11), f(12), f(13), f(14)],
                }
            })
            .collect();
        Sly3ItemTable { items }
    }

    /// The table in the gang's form (owners 7 Sly, 8 Bentley, 9 Murray), for slots and costs, with each item's
    /// behaviour (the Sly 2 item whose rules carry over, or a Sly 3 behaviour) and the held items' drains (field
    /// +16 [I]).
    pub fn to_item_table(&self) -> ItemTable {
        let mut t = self.to_item_table_plain();
        t.extra = (0..self.items.len())
            .map(|n| ItemExtra {
                behaviour: behaviour_of(n as u8),
                // Held items (button use 1, items 32 to 36) drain field +16 per second; item 7 is a press item.
                drain: if self.items[n].button == 1 { self.items[n].cost2 as f32 } else { 0.0 },
            })
            .collect();
        t
    }

    fn to_item_table_plain(&self) -> ItemTable {
        let items = self
            .items
            .iter()
            .map(|r| Item {
                button: r.button,
                owner: match r.owner() {
                    Some(Member::Sly) => 7,
                    Some(Member::Bentley) => 8,
                    Some(Member::Murray) => 9,
                    _ => -1,
                },
                price: r.price,
                cost: r.cost,
                episode: r.episode,
                name: r.name,
                description: r.description,
                usage: r.usage,
            })
            .collect();
        ItemTable { items, extra: Vec::new() }
    }
}

/// The lookup (parts) tables each loader binds (characters.md 3.2): (what, address, records of 12 bytes).
pub const LOOKUP_TABLES: [(&str, u32, usize); 9] = [
    ("every actor", 0x0034_2DC0, 1),
    ("every playable actor", 0x0034_2DD0, 59),
    ("Sly", 0x0034_5140, 106),
    ("Sly (second)", 0x0034_5638, 11),
    ("Murray", 0x0034_5FD8, 127),
    ("Bentley", 0x0034_2890, 84),
    ("the Guru", 0x0034_9460, 43),
    ("the Panda King", 0x0034_6650, 49),
    ("Penelope", 0x0034_68A0, 53),
];

/// Reads a lookup table: (flags, OID) per record; the third word (an address of the original program) is skipped.
pub fn read_lookup(exe: &Executable, va: u32, count: usize) -> formats::Result<Vec<(u32, i32)>> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(formats::Error::UnknownExecutable);
    }
    let raw = exe.bytes_at(va, count * 12)?;
    Ok(raw
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| (u32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[4], c[5], c[6], c[7]])))
        .collect())
}

/// The gang constants for Sly 3 (characters.md 3.3): the ones found at their Sly 3 addresses (Murray's
/// knock-aside, the crush, the dart depths, the costume gravity) read from the executable; the rest are Sly 2's
/// until found.
pub fn constants_from_executable(exe: &Executable) -> formats::Result<GangConstants> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(formats::Error::UnknownExecutable);
    }
    let f = |va: u32| -> formats::Result<f32> {
        let b = exe.bytes_at(va, 4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let v = |va: u32| -> formats::Result<[f32; 3]> { Ok([f(va)?, f(va + 4)?, f(va + 8)?]) };
    Ok(GangConstants {
        murray_knock: [v(0x0032_3630)?, v(0x0032_3640)?],
        crush: v(0x0032_3670)?,
        dart_depth: [f(0x0031_FC88)?, f(0x0031_FC98)?],
        costume_gravity: v(0x0032_3560)?,
        ..GangConstants::SPEC
    })
}

/// The behaviour a Sly 3 item runs with (specs/sly3/gadgets.md): the Sly 2 item whose rules carry over, or one of
/// [`behaviour`] for the new items.
pub fn behaviour_of(n: u8) -> u8 {
    match n {
        6 => 7,                          // Trigger Bomb
        7 => behaviour::FISHING_POLE,    //
        8 => 26,                         // Alarm Clock (now Bentley's)
        9 => 10,                         // Adrenaline Burst
        10 => 11,                        // Health Extractor
        11 => 12,                        // Hover Pack (passive in Sly 3)
        12 => 34,                        // Insanity Strike (through the wheelchair)
        13 => behaviour::GRAPPLE_CAM,    //
        14 => 8,                         // Size Destabilizer
        15 => 37,                        // Rage Bomb (now Bentley's)
        16 => 13,                        // Reduction Bomb
        17 => behaviour::BE_THE_BALL,    //
        18 => 20,                        // Berserker Charge
        19 => 17,                        // Juggernaut Throw
        20 => 21,                        // Guttural Roar
        21 => 15,                        // Fists of Flame
        22 => 14,                        // Temporal Lock (now Murray's)
        23 => 22,                        // Raging Inferno Flop
        24 => 19,                        // Diablo Fire Slam
        25 => 23,                        // Smoke Bomb
        26 => 24,                        // Combat Dodge
        27 => 27,                        // Paraglide
        28 => 28,                        // Silent Obliteration
        29 => 30,                        // Feral Pounce
        30 => 31,                        // Mega Jump
        31 => 33,                        // Knockout Dive
        32 => 40,                        // Shadow Power level 1
        33 => 29,                        // Thief Reflexes
        34 => behaviour::SHADOW_POWER_2, //
        35 => behaviour::ROCKET_BOOTS,   //
        36 => behaviour::TREASURE_MAP,   //
        37 => behaviour::SHIELD,         //
        38..=40 => behaviour::DISGUISES[(n - 38) as usize],
        41..=49 => behaviour::ATTACK_UPGRADE,
        _ => behaviour::NONE,
    }
}

/// The gadget slot table (L1, L2, R2 as pad bits 4, 1, 2) [X 0x349040].
pub const SLOT_TABLE_VA: u32 = 0x0034_9040;
/// The other givers' pocket table [X].
pub const OTHER_POCKETS_VA: u32 = 0x0034_6BA8;
/// The guards' pocket table [X].
pub const GUARD_POCKETS_VA: u32 = 0x0034_6DC8;
/// Sly's costume parts for the three disguises (items 38, 39, 40), put on and taken off through his attachment
/// controller (gadgets.md 6.1) [X].
pub const COSTUME_PARTS: [i32; 3] = [1881, 1882, 1883];

/// The hit kinds of Bentley's chair melee and what they do to a guard (characters.md 10) [X].
pub mod hit {
    /// Size Destabilizer: the guard shrinks to half size (mode 14) for [`SHRINK_TIME`].
    pub const SHRINK: u32 = 0x40_0000;
    /// Insanity Strike: the guard attacks other guards (class 58; mode 12) for [`FRENZY_TIME`]; not Dimitri.
    pub const FRENZY: u32 = 0x80_0000;
    /// The shrunk guard's scale.
    pub const SHRINK_SCALE: f32 = 0.5;
    /// How long a guard stays shrunk.
    pub const SHRINK_TIME: f32 = 10.0;
    /// How long a guard attacks the others.
    pub const FRENZY_TIME: f32 = 30.0;
}

/// Bentley's chair melee by behaviour (Insanity Strike 34, item 12; Size Destabilizer 8, item 14): the clip, its
/// length and the hit kind (characters.md 10) [D] [X]. Power costs (10, 5) are the item records'.
pub fn chair_melee(behaviour: u8) -> Option<(i32, f32, u32)> {
    match behaviour {
        34 => Some((377, 1.0 / 3.0, hit::FRENZY)),
        8 => Some((374, 1.0 / 3.0, hit::SHRINK)),
        _ => None,
    }
}

/// Sly 3's Grapple-Cam controller (gadgets.md 6.9): phases 0 aim (turning toward the target, blending two clips
/// by the angle), 1 and 2 waiting for a turn, 3 the grab, 4 to 6 holding (5 and 6 turning), 7 the end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrappleCam {
    /// The phase.
    pub phase: u8,
    /// Blend weights of the aim (w = 0.5 + 0.5 × angle / threshold, and 1 − w).
    pub weights: [f32; 2],
}

/// What the Grapple-Cam controller reads in one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrappleCamInput {
    /// The angle between the camera body's heading and the direction to its target.
    pub angle: f32,
    /// The aim thresholds of phases 0 and 4 (fields +20, +24; values not given).
    pub thresholds: [f32; 2],
    /// The playback the phase waits for has ended.
    pub playback_done: bool,
    /// Field +200 is set and Cross was pressed within 0.25 s (phase 3).
    pub grab_press: bool,
    /// The target is the current player, in state 10.
    pub target_player_ready: bool,
    /// Sly is in state 20.
    pub sly_state_20: bool,
    /// The target carries option 113 with its +312 not −1.
    pub target_exits: bool,
    /// Cross or Square (mask 192) within 0.1 s.
    pub continue_press: bool,
    /// Triangle or Circle (mask 48) within 0.1 s.
    pub release_press: bool,
    /// Field +88 is set.
    pub keep: bool,
}

/// What the Grapple-Cam controller asks of its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrappleCamCue {
    /// Run the release routine (hand control back to the character [I]).
    Release,
    /// Call the owner's exit method.
    Exit,
    /// Release, then exit.
    ReleaseAndExit,
}

impl GrappleCam {
    /// Starts the grab (phase 3); the button is `GangTunables::grapple_grab` (not found yet).
    pub fn grab(&mut self) {
        self.phase = 3;
    }

    /// One step of the phase table.
    pub fn step(&mut self, i: &GrappleCamInput) -> Option<GrappleCamCue> {
        let aim = |t: f32| {
            if i.angle > t {
                Some(2)
            } else if i.angle < -t {
                Some(1)
            } else {
                None
            }
        };
        match self.phase {
            0 => match aim(i.thresholds[0]) {
                Some(p) => self.phase = p,
                None => {
                    let t = i.thresholds[0].max(1e-6);
                    let w = 0.5 + 0.5 * (i.angle / t);
                    self.weights = [1.0 - w, w];
                }
            },
            1 | 2 => {
                if i.playback_done {
                    self.phase = 0;
                }
            }
            3 => {
                if i.playback_done || i.grab_press {
                    if i.target_player_ready {
                        self.phase = 4;
                    } else if i.sly_state_20 {
                        self.phase = 7;
                    } else {
                        self.phase = 0;
                        return Some(GrappleCamCue::Release);
                    }
                }
            }
            4..=6 => {
                if i.target_exits {
                    return Some(GrappleCamCue::Exit);
                }
                if i.continue_press {
                    self.phase = 7;
                } else if i.release_press {
                    return Some(if i.keep { GrappleCamCue::Release } else { GrappleCamCue::ReleaseAndExit });
                } else if self.phase == 4 {
                    if let Some(p) = aim(i.thresholds[1]) {
                        self.phase = p + 4;
                    }
                } else if i.playback_done {
                    self.phase = 4;
                }
            }
            _ => {
                if i.playback_done {
                    if i.keep {
                        self.phase = 0;
                    } else {
                        return Some(GrappleCamCue::Exit);
                    }
                }
            }
        }
        None
    }
}

/// Pocket table rows: a zero row, then one per episode with a hub (Venice to pirates).
pub const POCKET_ROWS: usize = 6;
/// The loot table [X].
pub const LOOT_TABLE_VA: u32 = 0x0034_9070;
/// Sly 3's first prize kind: kind 1255 + i is loot record i (i = 0 to 29).
pub const FIRST_PRIZE_KIND: i32 = 1255;
/// The pickups' gravity vector (0, 0, −2250, 0) [X 0x346FD0].
pub const PICKUP_GRAVITY_VA: u32 = 0x0034_6FD0;

/// The gang's gadget object classes (gadgets.md 3): Grapple-Cam 86, its camera part 124, remote-controlled bodies
/// 119, thrown items 113 (Alarm Clock, the tossed camera), bombs 114, the Health Extractor 115.
pub mod gadget_class {
    /// The Grapple-Cam.
    pub const GRAPPLE_CAM: i16 = 86;
    /// Thrown items.
    pub const THROWN: i16 = 113;
    /// Bentley's bombs.
    pub const BOMB: i16 = 114;
    /// The Health Extractor.
    pub const EXTRACTOR: i16 = 115;
    /// Remote-controlled bodies.
    pub const REMOTE: i16 = 119;
    /// The Grapple-Cam's camera part.
    pub const CAMERA: i16 = 124;
}

/// Reads the slot table (pad bits of L1, L2, R2) after the hash check.
pub fn slot_bits(exe: &Executable) -> formats::Result<[i32; 3]> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(formats::Error::UnknownExecutable);
    }
    let b = exe.bytes_at(SLOT_TABLE_VA, 12)?;
    let i = |k: usize| i32::from_le_bytes([b[k * 4], b[k * 4 + 1], b[k * 4 + 2], b[k * 4 + 3]]);
    Ok([i(0), i(1), i(2)])
}

/// Reads a Sly 3 pocket table (`GUARD_POCKETS_VA` or `OTHER_POCKETS_VA`) after the hash check. Row `e − 2` is
/// episode `e` (3 Venice to 7 pirates) [I]; `PocketTable::row` takes the row index.
pub fn pocket_table(exe: &Executable, va: u32) -> formats::Result<PocketTable> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(formats::Error::UnknownExecutable);
    }
    Ok(PocketTable::from_bytes(exe.bytes_at(va, POCKET_ROWS * 60)?))
}

/// Reads the Sly 3 loot table (64 records of 12 bytes) after the hash check.
pub fn loot_table(exe: &Executable) -> formats::Result<LootTable> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(formats::Error::UnknownExecutable);
    }
    Ok(LootTable::from_bytes(exe.bytes_at(LOOT_TABLE_VA, 64 * 12)?))
}

/// The gang's shared rules for Sly 3 from its executable: the constants found at Sly 3 addresses, the item table
/// in the gang's form, and no items owned.
pub fn shared_from_executable(exe: &Executable) -> formats::Result<Shared> {
    Ok(Shared {
        consts: constants_from_executable(exe)?,
        tun: GangTunables::default(),
        items: Sly3ItemTable::from_executable(exe)?.to_item_table(),
        owned: Owned::default(),
    })
}

// ---- Arrival (characters.md 2) -----------------------------------------------------------------------------

/// The default arrival point (424; Sly 2 used 393).
pub const DEFAULT_ARRIVAL: i32 = 424;
/// The second arrival id checkpoints use.
pub const SECOND_ARRIVAL: i32 = 425;

/// A checkpoint (class 40 and its derived classes) as the arrival rule sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    /// Its OID.
    pub oid: i32,
    /// The ids its scripts added to the ones it answers to (they add 424 and 425).
    pub answers: Vec<i32>,
    /// Position.
    pub position: Vec3,
    /// Heading.
    pub yaw: f32,
    /// Inside the key object an exit or task names (true when none is named).
    pub in_key: bool,
}

/// The checkpoint an arrival id names: its OID equals the id, or it answers to it; only those inside the key
/// object count when one is named.
pub fn find_arrival(checkpoints: &[Checkpoint], id: i32) -> Option<usize> {
    checkpoints.iter().position(|c| c.in_key && (c.oid == id || c.answers.contains(&id)))
}

/// The current player when none is current: the saved "current player object" if present, otherwise Sly, then
/// Bentley, then Murray (the first present).
pub fn choose_player(saved: Option<Member>, present: &[Member]) -> Option<Member> {
    saved
        .filter(|m| present.contains(m))
        .or_else(|| [Member::Sly, Member::Bentley, Member::Murray].into_iter().find(|m| present.contains(m)))
}

impl Crew {
    /// The crew of a world on arrival (characters.md 2): the chosen player placed at the checkpoint (or, with no
    /// matching checkpoint, at its own placement); every other playable character is removed until a task enables
    /// it. `present` lists each member with its own placement.
    pub fn arrive(
        present: &[(Member, Vec3, f32)],
        checkpoint: Option<&Checkpoint>,
        saved: Option<Member>,
        shared: Shared,
    ) -> Option<Crew> {
        let members: Vec<Member> = present.iter().map(|p| p.0).collect();
        let player = choose_player(saved, &members)?;
        let (_, at, yaw) = present.iter().copied().find(|p| p.0 == player)?;
        let (at, yaw) = checkpoint.map_or((at, yaw), |c| (c.position, c.yaw));
        Some(Crew::new(&[(player, at, yaw)], shared))
    }

    /// A task enables a core member (placed at `at`); the recruits are refused until their moves are specified.
    pub fn enable(&mut self, member: Member, at: Vec3, yaw: f32) -> bool {
        let Some(w) = member.core() else { return false };
        if self.gang.get(w).is_some() {
            return true;
        }
        self.gang.members.push(Character::new(w, at, yaw));
        true
    }
}

#[cfg(test)]
mod arrival_tests {
    use super::*;

    #[test]
    fn arrival_and_player_choice() {
        let cp =
            |oid, answers: Vec<i32>, x| Checkpoint { oid, answers, position: [x, 0.0, 0.0], yaw: 0.0, in_key: true };
        let cps = [cp(139_910, alloc::vec![], 1.0), cp(7, alloc::vec![424, 425], 2.0), cp(424, alloc::vec![], 3.0)];
        assert_eq!(find_arrival(&cps, DEFAULT_ARRIVAL), Some(1));
        assert_eq!(find_arrival(&cps, 139_910), Some(0));
        assert_eq!(find_arrival(&cps, 5), None);
        assert_eq!(choose_player(None, &[Member::Murray, Member::Bentley]), Some(Member::Bentley));
        assert_eq!(choose_player(Some(Member::Murray), &[Member::Sly, Member::Murray]), Some(Member::Murray));
        assert_eq!(choose_player(Some(Member::Guru), &[Member::Sly]), Some(Member::Sly));
        let present = [(Member::Murray, [9.0; 3], 0.0), (Member::Sly, [8.0; 3], 0.0)];
        let mut crew = Crew::arrive(&present, Some(&cps[1]), None, shared()).unwrap();
        assert_eq!((crew.player(), crew.gang.members.len()), (Member::Sly, 1));
        assert_eq!(crew.gang.members[0].position, [2.0, 0.0, 0.0]);
        assert!(crew.enable(Member::Murray, [0.0; 3], 0.0) && crew.switch_to(Member::Murray));
        assert!(!crew.enable(Member::Penelope, [0.0; 3], 0.0));
        let mut raw = alloc::vec![0u8; 60];
        for (i, v) in [2i32, 25, 200, 5, 0, 3, 14, 217, 218, 299, 0, 0, 0, 0, 0].iter().enumerate() {
            raw[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let t = Sly3ItemTable::from_bytes(&raw);
        assert_eq!(t.items[0].owner(), Some(Member::Bentley));
        assert_eq!(t.to_item_table().items[0].owner, 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn members_classes_and_switching() {
        assert_eq!(Member::from_class(58), None, "58 is the guards' class");
        assert_eq!(Member::from_class(59), Some(Member::Dimitri));
        assert_eq!(Member::ALL.iter().filter(|m| m.recruit()).count(), 5);
        assert_eq!(Member::Bentley.core(), Some(Who::Bentley));
        let crew = &mut Crew::new(
            &[
                (Member::Sly, [0.0; 3], 0.0),
                (Member::Carmelita, [100.0, 0.0, 0.0], 0.0),
                (Member::Bentley, [0.0; 3], 0.0),
            ],
            shared(),
        );
        assert_eq!(crew.player(), Member::Sly);
        assert_eq!((crew.gang.members.len(), crew.recruits.len()), (2, 1));
        assert!(!crew.switch_to(Member::Carmelita), "recruits have no moves yet");
        assert!(!crew.switch_to(Member::Murray), "not present");
        assert!(crew.switch_to(Member::Bentley));
        assert_eq!(crew.player(), Member::Bentley);
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;
    use crate::collision::TriMesh;
    use crate::collision::world::{CollisionWorld, QueryFilter, Shape};
    use crate::gang::character::{GangEvent, sly};
    use crate::gang::input::button;
    use crate::gang::items::Power;
    use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle};
    use formats::geometry::Affine;

    fn floor() -> CollisionWorld {
        let e = 5000.0;
        let positions = alloc::vec![[-e, -e, 0.0], [e, -e, 0.0], [e, e, 0.0], [-e, e, 0.0]];
        let tri = |c: [u16; 3]| CollisionTriangle {
            corners: c,
            radius: 8000.0,
            center: [0.0; 3],
            first_edge: 0,
            edge_count: 0,
        };
        let mesh = CollisionMesh {
            radius: 8000.0,
            positions,
            triangles: alloc::vec![tri([0, 1, 2]), tri([0, 2, 3])],
            edges: Vec::new(),
        };
        let mut cw = CollisionWorld::new();
        let s = cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None });
        cw.add_body(s, Affine::IDENTITY, 0, 0, true);
        cw.rebuild();
        cw
    }

    /// A Sly 3 table with the records the tests use: (number, button, owner, cost, cost2).
    fn sly3_shared() -> Shared {
        let mut raw = alloc::vec![0u8; ITEM_COUNT * 60];
        for (n, rec) in [
            (7usize, [2i32, 25, 0, 0, 0]),
            (11, [-1, 25, 300, 0, 0]),
            (12, [2, 25, 0, 10, 0]),
            (14, [2, 25, 0, 5, 0]),
            (36, [1, 24, 0, 0, 0]),
            (17, [2, 26, 0, 0, 0]),
            (32, [1, 24, 1000, 1, 3]),
            (34, [1, 24, 1600, 1, 3]),
            (35, [1, 24, 600, 1, 5]),
            (38, [2, 24, 0, 0, 0]),
            (41, [-1, 24, 0, 0, 0]),
            (45, [-1, 24, 800, 0, 0]),
        ] {
            for (i, v) in rec.iter().enumerate() {
                raw[n * 60 + i * 4..n * 60 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        let mut s = shared();
        s.items = Sly3ItemTable::from_bytes(&raw).to_item_table();
        s
    }

    #[test]
    fn sly3_items_and_moves() {
        let cw = floor();
        let mut s = sly3_shared();
        for n in [7, 11, 17, 32, 34, 35, 38, 45] {
            s.owned.add(n);
        }
        assert_eq!(s.items.behaviour(32), 40);
        assert_eq!(s.items.actual(behaviour::ROCKET_BOOTS), Some(35));
        assert_eq!(s.items.drain(32), Some(3.0));
        let targets = [Target {
            id: 9,
            position: [400.0, 0.0, 0.0],
            yaw: 0.0,
            kind: TargetKind::Guard { robbable: true, launched: false },
        }];
        let world = GangWorld {
            cw: &cw,
            filter: QueryFilter::default(),
            targets: &targets,
            help: 0.0,
            safehouse: false,
            pools: &[],
        };
        let dt = 1.0 / 60.0;
        let press = |b: u32| GangInput { pressed: b, held: b, ..GangInput::default() };
        let mut out = Vec::new();
        // Sly: the cane combo. Two Triangles then Cross: the jump attack at level 2 (item 45 owned); one Triangle then
        // Circle: push level 1 not owned, nothing.
        let mut c = Character::new(Who::Sly, [-2000.0, 0.0, 0.0], 0.0);
        for b in [button::TRIANGLE, 0, button::TRIANGLE, 0, button::CROSS] {
            c.update(dt, &press(b), &world, &s, &mut out);
        }
        assert!(out.contains(&GangEvent::ComboFinisher { kind: 1, level: 2 }), "{out:?}");
        assert_ne!(c.state, sly::AIR, "the Cross finished the combo instead of jumping");
        out.clear();
        for b in [button::TRIANGLE, 0, button::CIRCLE] {
            c.update(dt, &press(b), &world, &s, &mut out);
        }
        assert!(!out.iter().any(|e| matches!(e, GangEvent::ComboFinisher { .. })));
        // Shadow Power level 2: invisible while held, draining field +16; a disguise toggles.
        c.slots.0 = [Some(34), Some(38), Some(35)];
        c.power = Power::new(50.0, 100.0);
        out.clear();
        c.update(dt, &press(button::L1), &world, &s, &mut out);
        assert_eq!(c.invisible, 2);
        for _ in 0..60 {
            c.update(dt, &GangInput { held: button::L1, ..GangInput::default() }, &world, &s, &mut out);
        }
        assert!((c.power.value - (50.0 - 1.0 - 3.0)).abs() <= 1.0, "{}", c.power.value);
        c.update(dt, &GangInput::default(), &world, &s, &mut out);
        assert_eq!(c.invisible, 0);
        c.update(dt, &press(button::L2), &world, &s, &mut out);
        assert_eq!(c.disguise, Some(0));
        c.update(dt, &GangInput::default(), &world, &s, &mut out);
        c.update(dt, &press(button::L2), &world, &s, &mut out);
        assert_eq!(c.disguise, None);
        // Bentley: the Fishing Pole takes from the guard at range; no crossbow; the Hover Pack on Cross in the air.
        let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
        b.slots.0 = [Some(7), None, None];
        out.clear();
        // Held: the guard 200 from the tip is hooked, then held 0.35 s later (gadgets.md 6.5).
        b.update(dt, &press(button::L1), &world, &s, &mut out);
        let hold = GangInput { held: button::L1, ..GangInput::default() };
        for _ in 0..19 {
            b.update(dt, &hold, &world, &s, &mut out);
        }
        assert_eq!(b.fishing.map(|f| f.hook), Some(1));
        assert!(!out.contains(&GangEvent::Pickpocket(9)));
        for _ in 0..3 {
            b.update(dt, &hold, &world, &s, &mut out);
        }
        assert!(out.contains(&GangEvent::Pickpocket(9)), "{:?}", b.fishing);
        assert_eq!(b.fishing.map(|f| f.hook), Some(2));
        b.update(dt, &GangInput::default(), &world, &s, &mut out);
        assert!(b.fishing.is_none() && out.contains(&GangEvent::ItemEnded(behaviour::FISHING_POLE)));
        out.clear();
        b.update(dt, &GangInput::default(), &world, &s, &mut out);
        b.update(dt, &press(button::SQUARE), &world, &s, &mut out);
        assert!(!out.iter().any(|e| matches!(e, GangEvent::Crossbow { .. })));
        b.update(dt, &press(button::CROSS), &world, &s, &mut out);
        for _ in 0..10 {
            b.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        b.update(dt, &press(button::CROSS), &world, &s, &mut out);
        assert!(out.contains(&GangEvent::SpecialJump(17)), "{out:?}");
        // Murray: Be The Ball toggles rolling.
        let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
        m.slots.0 = [Some(17), None, None];
        out.clear();
        m.update(dt, &press(button::L1), &world, &s, &mut out);
        assert!(m.rolling && out.contains(&GangEvent::Rolling(true)));
    }

    #[test]
    fn grapple_cam_phases() {
        let mut g = GrappleCam::default();
        let base = GrappleCamInput { thresholds: [0.5, 0.5], ..GrappleCamInput::default() };
        // Aiming: within the threshold the blend follows the angle; beyond it a turn (phase 2), then back.
        g.step(&GrappleCamInput { angle: 0.25, ..base });
        assert_eq!((g.phase, g.weights), (0, [0.25, 0.75]));
        g.step(&GrappleCamInput { angle: 0.8, ..base });
        assert_eq!(g.phase, 2);
        g.step(&GrappleCamInput { playback_done: true, ..base });
        assert_eq!(g.phase, 0);
        // The grab on the player in state 10 holds; Cross or Square continues to the end.
        g.grab();
        g.step(&GrappleCamInput { playback_done: true, target_player_ready: true, ..base });
        assert_eq!(g.phase, 4);
        assert_eq!(g.step(&GrappleCamInput { target_exits: true, ..base }), Some(GrappleCamCue::Exit));
        assert_eq!(g.step(&GrappleCamInput { release_press: true, ..base }), Some(GrappleCamCue::ReleaseAndExit));
        g.step(&GrappleCamInput { continue_press: true, ..base });
        assert_eq!(g.phase, 7);
        assert_eq!(g.step(&GrappleCamInput { playback_done: true, ..base }), Some(GrappleCamCue::Exit));
        // A grab on anything else releases.
        let mut g = GrappleCam::default();
        g.grab();
        assert_eq!(g.step(&GrappleCamInput { playback_done: true, ..base }), Some(GrappleCamCue::Release));
        assert_eq!(g.phase, 0);
    }

    #[test]
    fn sly3_item_findings() {
        let cw = floor();
        let mut s = sly3_shared();
        for n in [12, 14, 17, 35, 36] {
            s.owned.add(n);
        }
        let world = GangWorld {
            cw: &cw,
            filter: QueryFilter::default(),
            targets: &[],
            help: 0.0,
            safehouse: false,
            pools: &[],
        };
        let dt = 1.0 / 60.0;
        let press = |b: u32| GangInput { pressed: b, held: b, ..GangInput::default() };
        let mut out = Vec::new();
        // Be The Ball: Square kicks 56 × stick forward along the heading; Cross jumps to about 315; radius 75.
        let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
        m.slots.0 = [Some(17), None, None];
        for _ in 0..20 {
            m.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        m.update(dt, &press(button::L1), &world, &s, &mut out);
        assert!(m.rolling);
        let fwd =
            GangInput { stick: [0.0, 1.0], pressed: button::SQUARE, held: button::SQUARE, ..GangInput::default() };
        m.update(dt, &GangInput::default(), &world, &s, &mut out);
        let v0 = m.velocity[0];
        m.update(dt, &fwd, &world, &s, &mut out);
        assert!(m.velocity[0] - v0 > 50.0, "{:?}", m.velocity);
        for _ in 0..20 {
            m.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        m.update(dt, &press(button::CROSS), &world, &s, &mut out);
        let mut peak: f32 = 0.0;
        for _ in 0..60 {
            m.update(dt, &GangInput::default(), &world, &s, &mut out);
            peak = peak.max(m.position[2]);
        }
        assert!((peak - 315.0).abs() < 15.0, "peak {peak}");
        // Rocket Boots: at least 0.07 s of start-up (turning in place), then the run.
        let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
        c.slots.0 = [Some(35), Some(36), None];
        c.power = Power::new(100.0, 100.0);
        let run = |b: u32| GangInput { stick: [0.0, 1.0], held: b, pressed: b, ..GangInput::default() };
        let hold = |b: u32| GangInput { stick: [0.0, 1.0], held: b, ..GangInput::default() };
        for _ in 0..5 {
            c.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        c.update(dt, &run(button::L1), &world, &s, &mut out);
        assert!(c.rocket && c.rocket_start.is_some());
        let x0 = c.position;
        for _ in 0..3 {
            c.update(dt, &hold(button::L1), &world, &s, &mut out);
        }
        assert!(c.rocket_start.is_some() && crate::math::length(crate::math::sub(c.position, x0)) < 1.0);
        for _ in 0..10 {
            c.update(dt, &hold(button::L1), &world, &s, &mut out);
        }
        assert!(c.rocket_start.is_none() && crate::math::length(crate::math::sub(c.position, x0)) > 1.0);
        c.update(dt, &GangInput::default(), &world, &s, &mut out);
        // The treasure map: at least 0.5 s out, and the heading snaps to one of 8 directions.
        out.clear();
        c.update(
            dt,
            &GangInput {
                stick: [0.3, 1.0],
                pressed: button::L2,
                held: button::L2,
                camera_yaw: 0.2,
                ..GangInput::default()
            },
            &world,
            &s,
            &mut out,
        );
        assert!(out.contains(&GangEvent::TreasureMap(true)));
        c.update(
            dt,
            &GangInput { stick: [0.3, 1.0], held: button::L2, camera_yaw: 0.2, ..GangInput::default() },
            &world,
            &s,
            &mut out,
        );
        let step = core::f32::consts::TAU / 8.0;
        let k = c.yaw / step;
        assert!((k - (k + 0.5 - crate::math::rem_euclid(k + 0.5, 1.0))).abs() < 1e-4, "yaw {}", c.yaw);
        for _ in 0..10 {
            c.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        assert!(!out.contains(&GangEvent::TreasureMap(false)), "held at least 0.5 s");
        for _ in 0..30 {
            c.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        assert!(out.contains(&GangEvent::TreasureMap(false)));
        // Bentley's chair melee: Insanity Strike clip 377 (frenzy), Size Destabilizer clip 374 (shrink).
        let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
        b.slots.0 = [Some(12), Some(14), None];
        b.power = Power::new(100.0, 100.0);
        out.clear();
        b.update(dt, &press(button::L1), &world, &s, &mut out);
        assert!(out.contains(&GangEvent::ChairMelee { item: 34, clip: 377, hit: hit::FRENZY }), "{out:?}");
        for _ in 0..30 {
            b.update(dt, &GangInput::default(), &world, &s, &mut out);
        }
        assert!(!out.iter().any(|e| matches!(e, GangEvent::Overlay { .. })));
        b.update(dt, &press(button::L2), &world, &s, &mut out);
        assert!(out.contains(&GangEvent::ChairMelee { item: 8, clip: 374, hit: hit::SHRINK }));
        assert_eq!(chair_melee(8).map(|m| m.1), Some(1.0 / 3.0));
    }
}
