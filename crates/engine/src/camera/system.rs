//! [`CameraSystem`]: the follow camera behind the game's camera seam (`game::systems::CameraSystem`).

use alloc::vec::Vec;

use formats::geometry::Affine;
use platform_api::Buttons;

use super::consts::{CameraConstants, CameraTunables};
use super::fade::{FadeSource, fade_sources};
use super::follow::{CameraFrame, CameraInput, CameraScene, FollowCamera, Lens, Mode, PanelSubject, Subject};
use super::zone::{CameraZone, zones_from_stream};
use crate::collision::CollisionWorld;
use crate::game::render_scene::{CameraView, world_opt};
use crate::game::systems::{CameraExtras, CameraSystem as CameraSeam, CollisionQueries, FrameInput};
use crate::math::atan2;
use crate::object::{Handle, OptionValue};
use crate::world::World;
use crate::world::shared::Hiding;

/// The camera of the game: zones, fade groups and a camera collision world read from the level at `configure`,
/// the player followed every frame.
pub struct CameraSystem {
    /// The camera.
    pub camera: FollowCamera,
    zones: Vec<CameraZone>,
    fades: Vec<(FadeSource, Affine)>,
    fade_objects: Vec<Option<Handle>>,
    collision: Option<CollisionWorld>,
    last: Option<CameraFrame>,
    conversation: bool,
}

impl Default for CameraSystem {
    fn default() -> Self {
        CameraSystem::new(CameraConstants::SPEC, CameraTunables::default())
    }
}

impl CameraSystem {
    /// A camera system with given constants (`CameraConstants::from_executable` for the player's disc).
    pub fn new(consts: CameraConstants, tun: CameraTunables) -> CameraSystem {
        CameraSystem {
            camera: FollowCamera::with(consts, tun),
            zones: Vec::new(),
            fades: Vec::new(),
            fade_objects: Vec::new(),
            collision: None,
            last: None,
            conversation: false,
        }
    }

    /// The last frame: view, lens, fog, faded meshes, letterbox and picture panels.
    pub fn frame(&self) -> Option<&CameraFrame> {
        self.last.as_ref()
    }

    /// The camera zones read at `configure`.
    pub fn zones(&self) -> &[CameraZone] {
        &self.zones
    }

    /// Switches mode: `Mode::Animation` for a camera placement driven by a cutscene (animation events 12 and 13
    /// enable and disable one), `Mode::Look` for the look-around view, `Mode::Follow` to return.
    pub fn set_mode(&mut self, mode: Mode) {
        self.camera.set_mode(mode);
    }

    fn player(world: &World) -> Option<(Handle, Affine)> {
        let h = world.player_target()?;
        Some((h, world.world_transform(h)?))
    }
}

fn lens_of(world: &World, c: &CameraConstants) -> Lens {
    // Sly 2/3 hook: the world option IDs below have Sly 1 meanings only; other games keep the constants' lens.
    let defaults = crate::object::OptionStore::new();
    let o = if world.sly1_meanings() { &world.world_options } else { &defaults };
    Lens {
        fov: o.f32_or(world_opt::FOV, c.fov),
        near: o.f32_or(world_opt::NEAR, c.near),
        far: o.f32_or(world_opt::FAR, c.far),
        fog: [
            o.f32_or(world_opt::FOG_START, c.fog[0]),
            o.f32_or(world_opt::FOG_END, c.fog[1]),
            o.f32_or(world_opt::FOG_MAX, c.fog[2]),
        ],
        fog_color: match o.get(world_opt::FOG_COLOR) {
            Some(OptionValue::Color(c)) => Some(*c),
            _ => None,
        },
    }
}

impl CameraSeam for CameraSystem {
    fn update(
        &mut self,
        world: &World,
        input: &FrameInput,
        real_dt: f32,
        collision: &dyn CollisionQueries,
    ) -> CameraView {
        let player = Self::player(world);
        let subject = match player {
            Some((_, t)) => {
                let ps = &world.shared.player;
                let flattened = matches!(ps.hiding, Hiding::Wall { .. });
                Subject {
                    position: t.t,
                    velocity: ps.velocity,
                    yaw: atan2(t.cols[0][1], t.cols[0][0]),
                    airborne: ps.airborne,
                    ground: ps.base.filter(|_| !ps.airborne).map(|b| b[2]),
                    // Flattened against a wall the move sets its own focus point, 50 above the origin.
                    focus_point: flattened.then(|| crate::math::add(t.t, [0.0, 0.0, 50.0])),
                    flattened,
                    hidden: matches!(ps.hiding, Hiding::Basket),
                    // The world state carries only "airborne" and the hiding kind: the follow factor eases to 0 in
                    // the air and in the casual hiding kind.
                    follow_off: ps.airborne || matches!(ps.hiding, Hiding::Casual),
                    ..Subject::default()
                }
            }
            // No player: hold the focus where it is (no look-ahead).
            None => Subject {
                position: crate::math::sub(self.camera.focus, [0.0, 0.0, self.camera.consts.base_height]),
                hidden: true,
                ..Subject::default()
            },
        };
        // A confrontation dialogue with the controls locked: the conversation view at the dialogue's anchor.
        let talking = world.shared.dialogue.filter(|_| world.shared.player.controls_locked);
        match (talking.and_then(|d| world.world_transform(d)), self.conversation) {
            (Some(anchor), _) => {
                if !self.conversation || !matches!(self.camera.mode, Mode::Aligned { .. }) {
                    self.camera.set_mode(Mode::Aligned { anchor, offset: self.camera.tun.conversation_offset });
                }
                self.conversation = true;
            }
            (None, true) => {
                self.camera.set_mode(Mode::Follow);
                self.conversation = false;
            }
            _ => {}
        }
        // Move the fade sources with their objects.
        for ((_, t), h) in self.fades.iter_mut().zip(&self.fade_objects) {
            if let Some(w) = h.and_then(|h| world.world_transform(h)) {
                *t = w;
            }
        }
        let panel = |p: Option<(i16, i16)>| -> Option<PanelSubject> {
            let (oid, _) = p?;
            let h = world.find_nearest(oid, None)?;
            let t = world.world_transform(h)?;
            Some(PanelSubject {
                position: t.t,
                yaw: atan2(t.cols[0][1], t.cols[0][0]),
                fov: None,
                distance: None,
                screen: [0.0; 2],
                light_offset: [0.0; 3],
            })
        };
        let scene = CameraScene {
            zones: &self.zones,
            // The physics system's collision world has the solids where they are now; the camera's own (built at
            // configure) is the fallback.
            collision: collision.collision_world().or(self.collision.as_ref()),
            fades: &self.fades,
            panels: [panel(world.shared.panels[0]), panel(world.shared.panels[1])],
            letterbox: world.shared.screen.letterbox,
        };
        let cam_input = CameraInput {
            left: input.left,
            right: input.right,
            recentre: input.just_pressed(Buttons::RSTICK),
            invert_x: false,
        };
        let frame = self.camera.update(real_dt, &cam_input, &subject, &scene);
        let view = CameraView {
            eye: frame.eye,
            target: frame.target,
            up: frame.up,
            fov_y: frame.fov,
            near: frame.near,
            far: frame.far,
        };
        self.last = Some(frame);
        view
    }

    fn cut(&mut self, eye: [f32; 3], target: [f32; 3]) {
        self.camera.cut_to(eye, target);
    }

    fn extras(&self) -> CameraExtras {
        let Some(f) = &self.last else { return CameraExtras::default() };
        let panel = |p: &Option<super::follow::PanelView>| p.as_ref().map(|p| (p.eye, p.target, p.fov, p.screen));
        CameraExtras {
            faded: f.faded.iter().map(|m| (m.record, m.mesh)).collect(),
            panels: [panel(&f.panels[0]), panel(&f.panels[1])],
        }
    }

    fn set_placement(&mut self, placement: Option<Affine>) {
        match placement {
            Some(placement) => self.camera.set_mode(Mode::Animation { placement }),
            None => {
                if matches!(self.camera.mode, Mode::Animation { .. }) {
                    self.camera.set_mode(Mode::Follow);
                }
            }
        }
    }

    fn configure(&mut self, world: &World) {
        let level = &world.level;
        let lens = lens_of(world, &self.camera.consts);
        self.camera.set_lens(lens);
        self.zones = zones_from_stream(&level.bytes, &level.stream, &level.table).unwrap_or_default();
        let sources = fade_sources(&level.bytes, &level.stream).unwrap_or_default();
        let handle_of_record = |rec: u32| world.handles().find(|&h| world.record(h) == Some(rec));
        self.fade_objects = sources.iter().map(|s| handle_of_record(s.record)).collect();
        let transforms = level.stream.world_transforms(true);
        self.fades = sources
            .into_iter()
            .map(|s| {
                let t = transforms[s.record as usize];
                (s, t)
            })
            .collect();
        // The camera's collision world: the level's solids where they were placed.
        let fw = formats::world::World {
            data: &level.bytes,
            layout: level.layout.clone(),
            objects: level.stream.clone(),
            texture_base: 0,
        };
        self.collision = CollisionWorld::from_world(&fw, &level.table).ok();
        // The player's own solids do not block the view.
        if let Some(cw) = &mut self.collision
            && let Some(p) = level.stream.records.iter().position(|r| r.class == 7 && r.oid == 5)
        {
            let root = level.stream.records[p].parent.unwrap_or(p as u32);
            let range = root..level.stream.records[root as usize].subtree_end;
            let own: Vec<_> = (0..cw.bodies().len() as u32)
                .filter(|&i| range.contains(&cw.bodies()[i as usize].user))
                .map(crate::collision::BodyId)
                .collect();
            for b in own {
                cw.set_enabled(b, false);
            }
        }
    }
}
