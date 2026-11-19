//! Sly 3 behaviours (specs/sly3 guards.md, alarms-sensors.md, interactables.md, pickpocket-and-pickups.md), attached
//! to Sly 3 worlds by [`attach`]. Sly 3 runs Sly 2's behaviour engine for these classes [R], so they reuse the Sly 2
//! behaviours (`sly2`, `sly2_sensors`, `sly2_props`, `sly2_pickups`) unchanged, with three small marked hooks there
//! (the playable characters' classes, the pocket row, object IDs).
//!
//! **Options.** A Sly 3 object's options have Sly 3 IDs. Before its behaviour is made, [`attach`] copies each option
//! whose meaning carries over (the "Sly 2 option" columns of the specs) to the Sly 2 ID the Sly 2 behaviour reads,
//! after clearing those Sly 2 IDs (a Sly 3 option may reuse a Sly 2 number with another meaning):
//!
//! | Sly 3 class (Sly 2 behaviour) | Options copied (Sly 3 → Sly 2) |
//! |---|---|
//! | 58 guards and swarmers (guard) | 937 → 254 hit points, 265 → 233 attacks, 309 → 292 near, 310 → 293 far, 329 → 312 sight flags, 330 → 313, 331 → 314 always / never see, 332 → 315 view volume, 344 → 327 home, 349 → 333 no surprise |
//! | 96 lasers (laser) | 508 → 455 beams |
//! | 15 detectors (detector) | 395 → 345 view volume |
//! | 93 contact triggers (touch pad) | 459 → 413 press kind, 468 → 419 re-arm time |
//! | 111 reactive props (prop) | 1125 → 1025, 1128 to 1143 → 1030 to 1045 |
//! | 21 trigger shapes (trigger volume) | 478 → 443 tracking, 491 → 441 command script; the lists 480 → 432 counted objects, 483 → 433 counted classes, 489 → 440 task-state requests |
//! | 103 coin, 104 task item, 105 medicine, 106 pocket item (pickups 91 to 94) | 964 → 902, 970 → 907 |
//! | 97 barriers (Sly 2 barrier rule, `sly2_world::barrier_blocks_characters`) | 513 → 458 mode; the lists 510 → 456 objects, 512 → 457 classes |
//! | 101 water (water) | 930 → 874 splash explosion [I: 930 to 932 name the splash and spray effects]; no contact value (876) |
//! | 139 noise templates (Sly 3 template, `devices`) | 1184 to 1188 → 1090 to 1094 |
//!
//! Typed lists (480, 483, 489) are kept by the Sly 3 loader as the level's actions; [`attach`] adds each list as
//! values of the Sly 2 option ([`list_aliases`]): integers for the object and class lists, the whole list for the
//! task-state requests. Option 488 (state-machine goals) has no Sly 2 trigger-volume counterpart and is not used yet.
//!
//! Barriers 97 (recording their contacts; the collision rule is Sly 2's through the copied options) and use points
//! 33 get Sly 3 behaviours ([`interactables`]); bouncers 94, safe controllers 80 (with their tumblers 121) and noise
//! templates 139 get theirs in [`devices`]. Water 101 runs Sly 2's water, its shape read through a marked hook. Not
//! attached: carry-use points 34, treasure stands 35, hiding spots 25 (shapes, not the Sly 2 barrels that are hit and
//! knocked over), set pieces 37, hooks 38, the compass 125 and treasure maps 153 (data in [`interactables`]) and
//! their marker factories 127. Option 469 on contact triggers and bouncers is probably a state-machine goal on
//! trigger (Sly 2's 424), not used yet.
//!
//! Other answers: a detector's option 390 links it to its spotlight gun (a class-47 object), not a watched target;
//! option 937 is the maximum hit points, and values of [`UNKILLABLE_HIT_POINTS`] and above are effectively
//! unkillable.

pub mod devices;
pub mod interactables;

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::Behavior;
use crate::object::{Handle, OptionValue};
use crate::world::World;

/// Hit points (option 937) from which a guard is effectively unkillable.
pub const UNKILLABLE_HIT_POINTS: f32 = 1000.0;

/// The playable characters' classes in Sly 3 (Sly, Murray, Bentley).
pub const CHARACTERS: [i16; 3] = [52, 53, 54];

/// Sly 3 classes with a reused Sly 2 behaviour.
pub mod class {
    /// Guards and swarming creatures.
    pub const GUARD: i16 = 58;
    /// Laser beams.
    pub const LASER: i16 = 96;
    /// Stand-alone detectors.
    pub const DETECTOR: i16 = 15;
    /// Contact triggers.
    pub const CONTACT: i16 = 93;
    /// Bouncers.
    pub const BOUNCER: i16 = 94;
    /// Reactive props and breakables.
    pub const PROP: i16 = 111;
    /// Trigger shapes.
    pub const TRIGGER: i16 = 21;
    /// Barriers.
    pub const BARRIER: i16 = 97;
    /// Water.
    pub const WATER: i16 = 101;
    /// Noise templates.
    pub const NOISE_TEMPLATE: i16 = 139;
    /// Coin.
    pub const COIN: i16 = 103;
    /// Task item.
    pub const TASK_ITEM: i16 = 104;
    /// Medicine.
    pub const MEDICINE: i16 = 105;
    /// Shown pocket prize.
    pub const LOOT: i16 = 106;
}

/// The Sly 3 → Sly 2 option IDs copied for a Sly 3 class.
pub fn aliases(class: i16) -> &'static [(u16, u16)] {
    match class {
        class::GUARD => &[
            (937, 254),
            (265, 233),
            (309, 292),
            (310, 293),
            (329, 312),
            (330, 313),
            (331, 314),
            (332, 315),
            (344, 327),
            (349, 333),
        ],
        class::LASER => &[(508, 455)],
        class::DETECTOR => &[(395, 345)],
        class::CONTACT => &[(459, 413), (468, 419)],
        class::PROP => &[
            (1125, 1025),
            (1128, 1030),
            (1129, 1031),
            (1130, 1032),
            (1131, 1033),
            (1132, 1034),
            (1133, 1035),
            (1134, 1036),
            (1135, 1037),
            (1136, 1038),
            (1137, 1039),
            (1138, 1040),
            (1139, 1041),
            (1140, 1042),
            (1141, 1043),
            (1142, 1044),
            (1143, 1045),
        ],
        class::COIN => &[(964, 902)],
        class::LOOT => &[(970, 907)],
        class::TRIGGER => &[(478, 443), (491, 441)],
        class::BARRIER => &[(513, 458)],
        class::WATER => &[(930, 874)],
        class::NOISE_TEMPLATE => &[(1184, 1090), (1185, 1091), (1186, 1092), (1187, 1093), (1188, 1094)],
        _ => &[],
    }
}

/// The Sly 3 typed lists copied to Sly 2 options for a class: (Sly 3 list, Sly 2 option, as integers). As integers,
/// each argument becomes one integer value; otherwise the whole list becomes one argument-list value.
pub fn list_aliases(class: i16) -> &'static [(u16, u16, bool)] {
    match class {
        class::TRIGGER => &[(480, 432, true), (483, 433, true), (489, 440, false)],
        class::BARRIER => &[(510, 456, true), (512, 457, true)],
        _ => &[],
    }
}

/// The playable characters' classes of the world's game (a marked hook in the Sly 2 behaviours).
pub fn character_classes(world: &World) -> [i16; 3] {
    if world.sly3.is_some() { CHARACTERS } else { super::sly2_sensors::CHARACTERS }
}

/// The pocket table row of an episode (a marked hook in the Sly 2 guard): Sly 3's row is episode − 2, an episode of
/// −1 counting as 3 (Venice, row 1); the training world (2) and Kaine Island (8) land on the empty rows 0 and 6
/// (guards.md 3) [X].
pub fn pocket_row(world: &World, episode: usize) -> usize {
    if world.sly3.is_some() { sly3_pocket_row(episode) } else { episode }
}

/// Sly 3's pocket row of an episode (see [`pocket_row`]); rows past the table read as empty.
pub fn sly3_pocket_row(episode: usize) -> usize {
    // The episode is stored unsigned, so −1 reads as 255.
    let e = if episode == usize::from(u8::MAX) { 3 } else { episode };
    e.wrapping_sub(2).min(crate::gang::sly3::POCKET_ROWS)
}

/// Copies the options of `h` whose meaning carries over to the Sly 2 IDs (see the module notes).
pub fn alias_options(world: &mut World, h: Handle, class: i16) {
    let pairs = aliases(class);
    let lists = list_aliases(class);
    if pairs.is_empty() && lists.is_empty() {
        return;
    }
    let mut values: Vec<(u16, Vec<OptionValue>)> = lists
        .iter()
        .map(|&(from, to, ints)| {
            let vs = interactables::lists(world, h, from)
                .flat_map(|a| -> Vec<OptionValue> {
                    if ints {
                        a.iter().filter_map(interactables::arg_i32).map(OptionValue::Int).collect()
                    } else {
                        alloc::vec![OptionValue::Args(a.into())]
                    }
                })
                .collect();
            (to, vs)
        })
        .collect();
    let Some(o) = world.options_mut(h) else { return };
    values.extend(pairs.iter().map(|&(from, to)| (to, o.all(from).cloned().collect())));
    for &(_, to) in pairs {
        o.clear(to);
    }
    for &(_, to, _) in lists {
        o.clear(to);
    }
    for (to, vs) in values {
        for v in vs {
            o.push(to, v);
        }
    }
}

/// The behaviour of a Sly 3 object, from its class.
pub fn for_class(world: &World, h: Handle) -> Behavior {
    use super::{sly2, sly2_pickups, sly2_props, sly2_sensors};
    match world.class(h) {
        Some(class::GUARD) => Behavior::Sly2Guard(Box::new(sly2::Sly2Guard::new(world, h))),
        Some(class::LASER) => Behavior::Sly2Laser(Box::new(sly2_sensors::Laser::new(world, h))),
        Some(class::DETECTOR) => Behavior::Sly2Detector(Box::new(sly2_sensors::Detector::new(world, h))),
        Some(class::CONTACT) => Behavior::Sly2Pad(Box::default()),
        Some(class::PROP) => Behavior::Sly2Prop(Box::new(sly2_props::Prop::new(world, h))),
        Some(class::TRIGGER) => Behavior::Sly2Volume(Box::default()),
        Some(c @ class::COIN..=class::LOOT) => sly2_pickups::Sly2Pickup::new(world, h, c - 12)
            .map_or(Behavior::None, |p| Behavior::Sly2Pickup(Box::new(p))),
        Some(class::BARRIER) => Behavior::Custom(Box::new(interactables::Barrier::new(world, h))),
        Some(class::WATER) => Behavior::Sly2Water(Box::default()),
        Some(interactables::class::USE_POINT) => Behavior::Custom(Box::new(interactables::UsePoint::new(world, h))),
        _ => devices::for_class(world, h),
    }
}

/// Attaches the Sly 3 behaviours to every object of a Sly 3 world.
pub fn attach(world: &mut World) {
    if world.sly3.is_none() {
        return;
    }
    let handles: Vec<Handle> = world.handles().collect();
    for h in handles {
        let Some(c) = world.class(h) else { continue };
        alias_options(world, h, c);
        let b = for_class(world, h);
        if !matches!(b, Behavior::None) {
            world.set_behavior(h, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_cover_the_spec_tables() {
        assert!(aliases(class::GUARD).contains(&(937, 254)));
        assert!(aliases(class::GUARD).contains(&(309, 292)) && aliases(class::GUARD).contains(&(310, 293)));
        assert_eq!(aliases(class::PROP).len(), 17);
        assert!(aliases(class::PROP).iter().all(|&(a, b)| a == 1125 && b == 1025 || a - b == 98));
        assert!(aliases(class::BOUNCER).is_empty());
        // The pickups: coin 103 → 91 … pocket item 106 → 94.
        assert_eq!(class::COIN - 12, super::super::sly2_pickups::class::COIN);
        assert_eq!(class::LOOT - 12, super::super::sly2_pickups::class::LOOT);
    }

    #[test]
    fn pocket_rows_follow_the_episode() {
        // Venice (3) row 1 … the pirates (7) row 5; −1 counts as 3; training (2) and Kaine Island (8) are empty.
        assert_eq!((3..=7).map(sly3_pocket_row).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(sly3_pocket_row(255), 1);
        assert_eq!(sly3_pocket_row(2), 0);
        assert_eq!(sly3_pocket_row(8), 6);
        assert_eq!(sly3_pocket_row(0), 6);
    }
}
