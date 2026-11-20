//! The gang's and the crew's clips on their poses, on the player's own discs (`SLYNX_DISC_SLY2`,
//! `SLYNX_DISC_SLY3`; each test skips when its disc is not set): the state → clip tables resolve to clips in each
//! character's own package or group, the pose versions change and the bones move over frames, transform 1 stays
//! put (root motion moves the object), and a clip's lens curve gives a camera object its field of view.

use std::sync::OnceLock;

use engine::anim::sly2_gang::{self, GangClips};
use engine::anim::sly2_pose::{self, Driven};
use engine::anim::sly3_pose::{self, CrewClips};
use engine::gang::{Character, Who, bentley, murray, sly};
use engine::object::Handle;
use engine::world::sly2::Sly2Options;
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats::geometry::Affine;
use harness::golden::{Game as Title, require_disc};

/// The nightclub hub (characters.md 1.1).
const HUB_WORLD_ID: u32 = 0x41;
const DT: f32 = 1.0 / 60.0;

struct Sly2Data {
    options: formats_sly2::exe::OptionTable,
    row: usize,
    bytes: Vec<u8>,
}

fn sly2(test: &str) -> Option<&'static Sly2Data> {
    static D: OnceLock<Sly2Data> = OnceLock::new();
    let path = require_disc(Title::Sly2, test)?;
    Some(D.get_or_init(|| {
        let mut d = formats_sly2::Disc::open_path(&path).expect("open disc");
        let row = d.find_world(HUB_WORLD_ID).expect("hub row");
        Sly2Data { options: d.options.clone(), row, bytes: d.read_world(row).expect("hub") }
    }))
}

fn hub(d: &Sly2Data) -> World {
    let info = WorldInfo { index: d.row as u32, id: HUB_WORLD_ID, task_mask: 0, identity: HUB_WORLD_ID };
    let opts = Sly2Options { row: Some(d.row as i16), info, ..Sly2Options::default() };
    World::load_sly2(d.bytes.clone(), &d.options, opts).expect("hub loads")
}

fn first(w: &World, class: i16) -> Handle {
    w.slots_of_class(class).first().and_then(|&s| w.handle(s)).expect("character")
}

/// The largest distance between the translations of two pose lists.
fn moved(a: &[Affine], b: &[Affine]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (0..3).map(|k| (x.t[k] - y.t[k]).powi(2)).sum::<f32>().sqrt()).fold(0.0, f32::max)
}

#[test]
fn sly2_gang_clips_resolve_and_pose() {
    let Some(d) = sly2("sly2_gang_clips_resolve_and_pose") else { return };
    let mut w = hub(d);
    let handles = [Some(first(&w, 53)), Some(first(&w, 54)), Some(first(&w, 55))];
    // characters.md 1.1, 1.2 and 9.1: every clip of the state table is a clip (109) in the character's package.
    use sly2_gang::clip as c;
    let tables: [(usize, Vec<i32>); 3] = [
        (
            0,
            [c::SLY_RUN.as_slice(), &c::SLY_MOVE, &c::SLY_AIR_ATTACK, &c::SLY_DODGE]
                .concat()
                .into_iter()
                .chain(c::TAKE_OFF[0])
                .chain(c::SECOND[0])
                .chain(c::SLY_SLIDE[..3].iter().copied())
                .chain(c::SLY_GLIDE)
                .chain([158, c::SLY_GLIDE_OPEN, c::SLY_DIVE, c::SLY_PICKPOCKET, c::SLY_JUGGLE, c::SLY_SWING])
                .collect(),
        ),
        (
            1,
            [
                c::MURRAY_RUN.as_slice(),
                &c::MURRAY_WALK,
                &c::MURRAY_CARRY,
                &c::MURRAY_PUNCHES,
                &c::MURRAY_CHARGE,
                &c::TAKE_OFF[1],
            ]
            .concat()
            .into_iter()
            .chain([590, 595, c::MURRAY_FLOP, c::MURRAY_THROW, c::MURRAY_SLAM, c::MURRAY_PICK_UP, c::MURRAY_TURNBUCKLE])
            .collect(),
        ),
        (
            2,
            [c::BENTLEY_RUN.as_slice(), &c::BENTLEY_WALK]
                .concat()
                .into_iter()
                .chain([c::BENTLEY_TIMED, 590, 595, 478, c::BENTLEY_HELD, c::BENTLEY_PICK_UP])
                .chain(560..572)
                .collect(),
        ),
    ];
    for (k, ids) in &tables {
        let h = handles[*k].unwrap();
        for &id in ids {
            let o = w.find_sly2_id(id, Some(h));
            assert_eq!(o.and_then(|o| w.class(o)), Some(sly2_gang::CLIP_CLASS), "character {k}: clip {id}");
            assert_eq!(o.and_then(|o| w.sly2_object(o)).map(|x| x.copy), w.sly2_object(h).map(|x| x.copy));
        }
    }
    // Sly runs at 832, Murray throws his first punch, Bentley stands.
    let mut members = [
        Character::new(Who::Sly, [0.0; 3], 0.0),
        Character::new(Who::Murray, [0.0; 3], 0.0),
        Character::new(Who::Bentley, [0.0; 3], 0.0),
    ];
    members[0].state = sly::RUN;
    members[0].velocity = [832.0, 0.0, 0.0];
    members[1].state = murray::PUNCH;
    members[2].state = bentley::STAND;
    let slot = |h: Option<Handle>| w.slot(h.unwrap()).unwrap();
    let slots = [slot(handles[0]), slot(handles[1]), slot(handles[2])];
    let loaded: Vec<Vec<Affine>> = slots.iter().map(|&s| w.sly2.as_deref().unwrap().pose_of(s).to_vec()).collect();
    let mut clips = GangClips::default();
    let mut history: Vec<[(u32, Vec<Affine>); 3]> = Vec::new();
    for _ in 0..40 {
        clips.drive(&mut w, &members, &handles, &[], DT);
        sly2_pose::step(&mut w);
        let l = w.sly2.as_deref().unwrap();
        history.push(slots.map(|s| (l.poser.version_of(s), l.pose_of(s).to_vec())));
    }
    for k in 0..2 {
        let (v0, p0) = &history[5][k];
        let (v1, p1) = &history[20][k];
        assert!(v1 > v0, "character {k}: pose version {v0} → {v1}");
        let m = moved(p0, p1);
        assert!(m > 1.0, "character {k}: bones moved {m}");
        // Transform 1 keeps its loaded horizontal place.
        for (a, b) in p1[1].t.iter().zip(&loaded[k][1].t).take(2) {
            assert!((a - b).abs() < 1e-2, "character {k}: root {:?} vs {:?}", p1[1].t, loaded[k][1].t);
        }
    }
    // Bentley stands: his idle machine animates him too.
    assert!(history[30][2].0 > history[5][2].0);
    assert!(moved(&history[39][2].1, &loaded[2]) > 1.0, "the idle pose differs from the loaded one");
}

#[test]
fn sly2_standing_runs_the_idle_machines_and_the_glider_shows_while_gliding() {
    let Some(d) = sly2("sly2_standing_runs_the_idle_machines_and_the_glider_shows_while_gliding") else { return };
    let mut w = hub(d);
    let handles = [Some(first(&w, 53)), Some(first(&w, 54)), Some(first(&w, 55))];
    let members = [
        Character::new(Who::Sly, [0.0; 3], 0.0),
        Character::new(Who::Murray, [0.0; 3], 0.0),
        Character::new(Who::Bentley, [0.0; 3], 0.0),
    ];
    let mut clips = GangClips::default();
    let mut played: [Vec<i32>; 3] = Default::default();
    // 40 s standing: characters.md 9.2's chains.
    for _ in 0..2400 {
        clips.drive(&mut w, &members, &handles, &[], DT);
        sly2_pose::step(&mut w);
        for k in 0..3 {
            if let Some(sly2_gang::Motion::At(c, _)) = clips.tracks[k].motion
                && played[k].last() != Some(&c)
            {
                played[k].push(c);
            }
        }
    }
    eprintln!("idle chains: {played:?}");
    assert!([4576, 4578, 4580].contains(&played[0][0]), "{:?}", played[0]);
    assert!([5539, 5540, 5541].contains(&played[0][1]), "{:?}", played[0]);
    assert!(played[0].iter().any(|c| (5542..=5555).contains(c)), "{:?}", played[0]);
    assert!([7238, 7240, 7242, 17779].contains(&played[1][0]), "{:?}", played[1]);
    assert!([17780, 7244].contains(&played[1][1]), "{:?}", played[1]);
    assert_eq!(&played[2][..2], [7608, 7610]);
    assert!(played[2][2..].iter().all(|c| [7602, 7604, 7606].contains(c)), "{:?}", played[2]);
    // The glider (9.3): shown only while Sly glides.
    let sly_slot = w.slot(handles[0].unwrap()).unwrap();
    assert!(!w.sly2.as_deref().unwrap().poser.gliding.contains(&sly_slot));
    let mut gliding = members.clone();
    gliding[0].state = sly::AIR;
    gliding[0].gliding = true;
    clips.drive(&mut w, &gliding, &handles, &[(Who::Sly, engine::gang::GangEvent::SpecialJump(19))], DT);
    assert!(w.sly2.as_deref().unwrap().poser.gliding.contains(&sly_slot));
    assert_eq!(clips.tracks[0].overlay.map(|o| o.0), Some(sly2_gang::Motion::Once(1518)));
}

#[test]
fn sly2_bentley_weapon_marker_is_on_his_left_at_rest() {
    let Some(d) = sly2("sly2_bentley_weapon_marker_is_on_his_left_at_rest") else { return };
    let w = hub(d);
    // characters.md 5.3: object 580 on Bentley's transform 52, at rest on his left: the pairs 562/563, 566/567, 570/571.
    let bentley = first(&w, 55);
    let m = w.find_sly2_id(sly2_gang::WEAPON_MARKER, Some(bentley)).expect("weapon marker");
    assert_eq!(w.sly2_object(m).and_then(|o| o.bone).map(|b| b.1), Some(52));
    assert_eq!(sly2_gang::weapon_side(&w, bentley), 1);
}

#[test]
fn sly2_lens_curve_sets_the_camera_field_of_view() {
    let Some(d) = sly2("sly2_lens_curve_sets_the_camera_field_of_view") else { return };
    let mut w = hub(d);
    // Clip 642's extra curve names camera object 4332 and runs from 15 at its start (camera.md 2.1).
    let cam = w.find_sly2_id(4332, None).expect("camera 4332");
    assert_eq!(w.class(cam), Some(sly2_pose::CAMERA_CLASS));
    let clip = w
        .slots_with_sly2_id(642)
        .into_iter()
        .filter_map(|s| w.handle(s))
        .find(|&h| w.class(h) == Some(sly2_gang::CLIP_CLASS))
        .expect("clip 642");
    w.sly2.as_deref_mut().unwrap().poser.driven.push(Driven {
        target: cam,
        clips: vec![(clip, 0.0, 1.0)],
        lock_root: false,
    });
    sly2_pose::step(&mut w);
    let s = w.slot(cam).unwrap();
    let fov = w.sly2.as_deref().unwrap().poser.fov_of(s).expect("a lens");
    let want = engine::camera::views::fov_of_focal_length(15.0);
    assert!((fov - want).abs() < 1e-4, "{fov} vs {want}");
    // The camera's view takes it.
    let shot = engine::camera::views::camera_object_view(&w, cam).expect("view");
    assert_eq!(shot.fov, Some(fov));
}

struct Sly3Data {
    options: formats_sly2::exe::OptionTable,
    multipliers: [f32; 3],
    slot: usize,
    bytes: Vec<u8>,
    exe: formats_sly3::exe::Executable,
}

fn sly3(test: &str) -> Option<&'static Sly3Data> {
    static D: OnceLock<Sly3Data> = OnceLock::new();
    let path = require_disc(Title::Sly3, test)?;
    Some(D.get_or_init(|| {
        let mut d = formats_sly3::Disc::open_path(&path).expect("open disc");
        let slot = d.find_world("v_apt").expect("v_apt");
        Sly3Data {
            options: d.options.clone(),
            multipliers: d.tangent_multipliers,
            slot,
            bytes: d.read_world(slot).expect("v_apt"),
            exe: d.executable.clone(),
        }
    }))
}

#[test]
fn sly3_crew_clips_resolve_and_pose() {
    let Some(d) = sly3("sly3_crew_clips_resolve_and_pose") else { return };
    // characters.md 12.2: the lookup tables hold the IDs the crew plays.
    use engine::gang::sly3::read_lookup;
    use sly3_pose::clip as c;
    let sly_t = read_lookup(&d.exe, 0x0034_5140, 106).unwrap();
    let sly_2 = read_lookup(&d.exe, 0x0034_5638, 11).unwrap();
    let murray_t = read_lookup(&d.exe, 0x0034_5FD8, 127).unwrap();
    let bentley_t = read_lookup(&d.exe, 0x0034_2890, 84).unwrap();
    let ids = |t: &[(u32, i32)], r: &[usize]| r.iter().map(|&i| t[i].1).collect::<Vec<_>>();
    assert_eq!(ids(&sly_t, &[0, 1, 2]), [c::IDLE[0], c::FULL[0][0], c::FULL[0][1]]);
    assert_eq!(ids(&sly_2, &[6, 7]), c::STICK[0]);
    assert_eq!(ids(&sly_t, &[15, 19, 18, 16, 17]), c::JUMP[0]);
    assert_eq!(ids(&sly_t, &[20, 24, 23, 21, 22]), c::FALL[0]);
    assert_eq!(ids(&sly_t, &[14, 80, 81]), [c::LEDGE_HANG[0], 312, 313]);
    assert_eq!(ids(&sly_t, &[54, 55, 56]), c::SWING);
    assert_eq!(ids(&bentley_t, &[2, 0, 1, 26, 27, 33, 38]), [451, 449, 447, 535, 534, 696, 707]);
    assert_eq!(ids(&murray_t, &[2, 0, 1, 33, 34, 17, 18, 53, 58]), [451, 449, 447, 535, 534, 528, 529, 696, 701]);

    let info = WorldInfo { index: d.slot as u32, ..WorldInfo::default() };
    let opts = Sly3Options { slot: Some(d.slot as i16), info, multipliers: d.multipliers, ..Sly3Options::default() };
    let mut w = World::load_sly3(d.bytes.clone(), &d.options, opts).expect("v_apt loads");
    let handles = [Some(first(&w, 52)), Some(first(&w, 53)), Some(first(&w, 54))];
    // Every clip of the table is a clip with data in the character's own group; the idle machine plays one of its
    // members.
    for (k, h) in handles.iter().enumerate() {
        let g = w.sly3_object(h.unwrap()).unwrap().group;
        let mut want: Vec<i32> = [c::FULL[k], c::STICK[k]].concat();
        // Sub-kind 0 must be there (the others fall back to it); the ledge hang where the character has one.
        want.extend([c::JUMP[k][0], c::FALL[k][0], c::LEDGE_HANG[k]].into_iter().filter(|&x| x >= 0));
        if k == 0 {
            want.extend([312, 313, c::GLIDE_OPEN]);
            want.extend(c::SWING);
            want.extend(c::GLIDE);
        }
        if k == 1 {
            want.extend(c::MURRAY_CARRY);
        }
        let mut p = std::mem::take(&mut w.sly3.as_deref_mut().unwrap().poser);
        for id in want {
            assert!(p.clip(&w, g, id).is_some_and(|x| x.duration() > 0.0), "character {k}: clip {id}");
        }
        let missing: Vec<i32> =
            c::JUMP[k].iter().chain(&c::FALL[k]).copied().filter(|&x| x >= 0 && p.clip(&w, g, x).is_none()).collect();
        eprintln!("character {k}: jump and fall clips not in the group (sub-kind 0 plays instead): {missing:?}");
        let idle = p.machine_clip(&w, g, c::IDLE[k]).expect("idle machine clip");
        assert!(p.clip(&w, g, idle).is_some(), "character {k}: idle clip {idle}");
        w.sly3.as_deref_mut().unwrap().poser = p;
    }
    // Murray's punches are his table's records 5 to 9 (characters.md 12.3), clips in his group.
    assert_eq!(ids(&murray_t, &[5, 6, 7, 8, 9]), c::MURRAY_PUNCHES);
    let murray = handles[1].unwrap();
    let g = w.sly3_object(murray).unwrap().group;
    let mut p = std::mem::take(&mut w.sly3.as_deref_mut().unwrap().poser);
    let lengths: Vec<f32> =
        c::MURRAY_PUNCHES.iter().map(|&id| p.clip(&w, g, id).expect("punch clip").duration()).collect();
    for (l, want) in lengths.iter().zip([2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 11.0 / 12.0]) {
        assert!((l - want).abs() < 1e-3, "{lengths:?}");
    }
    w.sly3.as_deref_mut().unwrap().poser = p;
    // Standing (the idle machine), then running with the stick; Murray walks and Bentley stands.
    let mut members = [
        Character::new(Who::Sly, [0.0; 3], 0.0),
        Character::new(Who::Murray, [0.0; 3], 0.0),
        Character::new(Who::Bentley, [0.0; 3], 0.0),
    ];
    members[1].state = murray::WALK;
    members[1].last_deflection = 0.6;
    let slots: Vec<u32> = handles.iter().map(|h| w.slot(h.unwrap()).unwrap()).collect();
    let loaded: Vec<Affine> = w.sly3.as_deref().unwrap().pose_of(slots[0]).to_vec();
    let mut clips = CrewClips::default();
    let mut history: Vec<Vec<(u32, Vec<Affine>)>> = Vec::new();
    for f in 0..80 {
        if f == 40 {
            members[0].state = sly::RUN;
            members[0].last_deflection = 1.0;
        }
        clips.drive(&mut w, &members, &handles, &[], DT);
        let l = w.sly3.as_deref().unwrap();
        history.push(slots.iter().map(|&s| (l.poser.version_of(s), l.pose_of(s).to_vec())).collect());
    }
    for (k, (a, b)) in [(0, (5, 30)), (0, (45, 70)), (1, (5, 30)), (2, (5, 30))] {
        let (v0, p0) = &history[a][k];
        let (v1, p1) = &history[b][k];
        assert!(v1 > v0, "character {k}, frames {a}..{b}: pose version {v0} → {v1}");
        let m = moved(p0, p1);
        assert!(m > 0.5, "character {k}, frames {a}..{b}: bones moved {m}");
    }
    let run = &history[70][0].1;
    for (a, b) in run[1].t.iter().zip(&loaded[1].t).take(2) {
        assert!((a - b).abs() < 1e-2, "Sly's root {:?} vs {:?}", run[1].t, loaded[1].t);
    }
    assert!(history.iter().all(|f| f.iter().all(|(_, p)| p.iter().all(|m| m.t.iter().all(|v| v.is_finite())))));
}
