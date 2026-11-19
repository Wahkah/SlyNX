//! The viewer itself, independent of the platform: the renderer, the current world, the loader, the free camera
//! and the stats overlay.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use formats::iso::ReadAt;
use render::overlay::TextOverlay;
use render::ui2d::UiLayer;
use render::{Backend, FrameStats, InstanceId, RenderError, Renderer};

use crate::anim::PlayerAnim;
use crate::camera::{Controls, FreeCamera, Spawn};
use crate::hud::{HudData, HudView, UI_QUADS};
use crate::level::{LevelStats, upload, upload_with_ui};
use crate::live::LevelMotion;
use crate::loader::{GameDisc, Host, Loader};

/// Glyph instances reserved for the overlay.
pub const OVERLAY_CHARS: usize = 400;
/// Clear colour before a world is loaded.
pub const CLEAR: [f32; 4] = [0.30, 0.38, 0.52, 1.0];

/// The world on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Current {
    pub index: usize,
    pub name: String,
    pub stats: LevelStats,
    pub spawn: Spawn,
    pub pools: u32,
    pub materials: u32,
}

/// Exponentially smoothed frame numbers for the overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Smoothed {
    pub frame_ms: f64,
    pub cpu_ms: f64,
    pub build_ms: f64,
    pub submit_ms: f64,
}

impl Smoothed {
    fn add(&mut self, frame_ms: f64, s: &FrameStats) {
        let k = if self.frame_ms == 0.0 { 1.0 } else { 0.05 };
        let mix = |a: &mut f64, b: f64| *a += (b - *a) * k;
        mix(&mut self.frame_ms, frame_ms);
        mix(&mut self.cpu_ms, s.cpu_ms());
        mix(&mut self.build_ms, s.cpu_build_ns as f64 * 1e-6);
        mix(&mut self.submit_ms, s.cpu_submit_ns as f64 * 1e-6);
    }
}

/// A viewer session.
pub struct Session<B: Backend, R: ReadAt + Send + 'static> {
    pub renderer: Renderer<B>,
    overlay: Option<TextOverlay>,
    /// The instance of each draw of the current world.
    instances: Vec<InstanceId>,
    /// The player character's animation in the current world.
    player: Option<PlayerAnim>,
    /// Backdrops, billboards and texture animation of the current world.
    motion: LevelMotion,
    /// The HUD and its UI layer.
    hud: Option<(HudView, UiLayer)>,
    /// Draw the HUD (on by default).
    pub show_hud: bool,
    /// The current world's clear colour.
    clear: [f32; 4],
    /// Play the player character's animation (on by default).
    pub animate: bool,
    loader: Option<Loader<R>>,
    names: Vec<String>,
    current: Option<Current>,
    /// A world asked for while another was loading.
    queued: Option<usize>,
    pub camera: FreeCamera,
    /// An error or notice shown on screen (no disc, a world that failed to load).
    pub message: Option<String>,
    pub show_overlay: bool,
    last: FrameStats,
    smooth: Smoothed,
}

impl<B: Backend, R: ReadAt + Send + 'static> Session<B, R> {
    /// A session on `disc`, or one that only shows the error when there is no disc. Nothing is loaded yet (see
    /// [`Session::request_world`]).
    pub fn new(backend: B, disc: Result<GameDisc<R>, String>) -> Result<Session<B, R>, RenderError> {
        let mut s = Session {
            renderer: Renderer::new(backend),
            overlay: None,
            instances: Vec::new(),
            player: None,
            motion: LevelMotion::default(),
            hud: None,
            show_hud: true,
            clear: CLEAR,
            animate: true,
            loader: None,
            names: Vec::new(),
            current: None,
            queued: None,
            camera: FreeCamera::default(),
            message: None,
            show_overlay: true,
            last: FrameStats::default(),
            smooth: Smoothed::default(),
        };
        match disc {
            Ok(d) => {
                s.names = d.world_names();
                s.loader = Some(Loader::new(d));
            }
            Err(e) => s.message = Some(e),
        }
        // An empty level with the overlay, so there is something to show until the first world is in.
        s.overlay = Some(upload(&mut s.renderer, None, OVERLAY_CHARS)?.overlay);
        Ok(s)
    }

    /// World names from the disc (empty without a disc).
    pub fn world_names(&self) -> &[String] {
        &self.names
    }

    pub fn current(&self) -> Option<&Current> {
        self.current.as_ref()
    }

    /// The world being loaded.
    pub fn loading(&self) -> Option<usize> {
        self.loader.as_ref().and_then(Loader::loading)
    }

    /// Statistics of the last rendered frame.
    pub fn last_stats(&self) -> &FrameStats {
        &self.last
    }

    pub fn smoothed(&self) -> &Smoothed {
        &self.smooth
    }

    /// Starts loading world `index` (wrapped into range). While another world loads, the request waits and the
    /// latest one wins.
    pub fn request_world(&mut self, host: &mut dyn Host, index: usize) {
        let Some(loader) = self.loader.as_mut() else { return };
        let index = index % self.names.len().max(1);
        match loader.start(host, index) {
            Ok(true) => {}
            Ok(false) => self.queued = Some(index),
            Err(e) => self.message = Some(format!("cannot start the loader: {e}")),
        }
    }

    /// Steps the world by `delta` from the current (or loading, or queued) one.
    fn step_world(&mut self, host: &mut dyn Host, delta: isize) {
        let n = self.names.len() as isize;
        if n == 0 {
            return;
        }
        let from = self.queued.or(self.loading()).or(self.current.as_ref().map(|c| c.index)).unwrap_or(0);
        let to = (from as isize + delta).rem_euclid(n) as usize;
        self.request_world(host, to);
    }

    /// Takes a finished load, uploads it and starts a queued one. Returns true when a new world went up.
    pub fn poll_loader(&mut self, host: &mut dyn Host) -> Result<bool, RenderError> {
        let Some(loader) = self.loader.as_mut() else { return Ok(false) };
        let Some((index, result)) = loader.poll(host) else { return Ok(false) };
        let mut changed = false;
        match result {
            Ok(mut level) => {
                let sheet = level.hud.as_ref().and_then(HudData::sheet);
                let quads = if sheet.is_some() { UI_QUADS } else { 0 };
                let mut up = upload_with_ui(&mut self.renderer, Some(&level), OVERLAY_CHARS, sheet, quads)?;
                self.hud = match (level.hud.take(), up.ui.take()) {
                    (Some(data), Some(layer)) => Some((HudView::new(data), layer)),
                    _ => None,
                };
                self.motion = LevelMotion::new(&level, &up);
                self.clear = level.look.clear;
                self.overlay = Some(up.overlay);
                self.instances = up.instances;
                self.player = level.player.take();
                self.camera.place(level.spawn);
                self.current = Some(Current {
                    index,
                    name: level.name.clone(),
                    stats: level.stats,
                    spawn: level.spawn,
                    pools: up.pools,
                    materials: up.materials.len() as u32,
                });
                self.message = None;
                changed = true;
            }
            Err(e) => self.message = Some(format!("world {index} failed to load: {e}")),
        }
        if let Some(next) = self.queued.take()
            && Some(next) != self.current.as_ref().map(|c| c.index)
        {
            self.request_world(host, next);
        }
        Ok(changed)
    }

    /// Loads world `index` and waits for it (harness and start-up).
    pub fn load_now(&mut self, host: &mut dyn Host, index: usize) -> Result<(), String> {
        self.request_world(host, index);
        loop {
            match self.poll_loader(host) {
                Ok(true) => return Ok(()),
                Ok(false) if self.loading().is_none() => {
                    return Err(self.message.clone().unwrap_or_else(|| String::from("nothing to load")));
                }
                Ok(false) => core::hint::spin_loop(),
                Err(e) => return Err(format!("{e}")),
            }
        }
    }

    /// One update step: controls, camera, world switching, finished loads. Returns false when the user quits.
    pub fn update(&mut self, host: &mut dyn Host, c: &Controls, dt: f32) -> Result<bool, RenderError> {
        if c.quit {
            return Ok(false);
        }
        if c.toggle_overlay {
            self.show_overlay = !self.show_overlay;
        }
        if c.reset
            && let Some(cur) = &self.current
        {
            self.camera.place(cur.spawn);
        }
        if c.next_world {
            self.step_world(host, 1);
        }
        if c.prev_world {
            self.step_world(host, -1);
        }
        self.camera.update(c, dt);
        if self.animate
            && let Some(p) = self.player.as_mut()
        {
            p.advance(dt);
            p.apply(&mut self.renderer, &self.instances)?;
        }
        if self.animate {
            self.motion.animate(&mut self.renderer, dt)?;
        }
        if let Some((view, _)) = self.hud.as_mut() {
            if c.toggle_overlay {
                view.flash();
            }
            view.update(dt);
        }
        self.poll_loader(host)?;
        Ok(true)
    }

    /// The overlay's text lines.
    pub fn overlay_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(m) = &self.message {
            // Long messages wrap at 60 characters.
            let mut rest = m.as_str();
            while !rest.is_empty() {
                let cut =
                    if rest.len() <= 60 { rest.len() } else { rest[..60].rfind([' ', ',', '/']).map_or(60, |i| i + 1) };
                let cut = (0..=cut).rev().find(|&i| rest.is_char_boundary(i)).unwrap_or(rest.len());
                lines.push(String::from(rest[..cut].trim()));
                rest = &rest[cut..];
            }
        }
        if !self.show_overlay {
            return lines;
        }
        let total = self.names.len();
        match &self.current {
            Some(c) => lines.push(format!("{} ({}/{})", c.name, c.index + 1, total)),
            None if self.loader.is_some() => lines.push(String::from("SlyNX viewer")),
            None => {}
        }
        if let Some(i) = self.loading() {
            lines.push(format!("loading {}...", self.names.get(i).map_or("", String::as_str)));
        }
        let s = &self.last;
        let m = &self.smooth;
        let fps = if m.frame_ms > 0.0 { 1000.0 / m.frame_ms } else { 0.0 };
        lines.push(format!("FPS {fps:.1}  CPU {:.2} ms (build {:.2} submit {:.2})", m.cpu_ms, m.build_ms, m.submit_ms));
        lines.push(format!(
            "draws {}  calls {}  batches {}  inst {}/{}  tris {}k",
            s.draw_commands,
            s.api_draw_calls,
            s.batches,
            s.visible,
            s.instances,
            s.triangles / 1000
        ));
        lines
    }

    /// Draws a frame. `frame_ns` is the time since the previous frame (for the FPS counter).
    pub fn render(&mut self, frame_ns: u64) -> Result<FrameStats, RenderError> {
        let lines = self.overlay_lines();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let (_, h) = self.renderer.backend().target_size();
        let scale = if h >= 1000 { 3 } else { 2 };
        if let Some(o) = self.overlay.as_mut() {
            o.set_text(&mut self.renderer, &refs, 8 * scale, 8 * scale, scale, [255, 255, 255, 255])?;
        }
        self.motion.follow_camera(&mut self.renderer, &self.instances, &self.camera)?;
        if let Some((view, layer)) = self.hud.as_mut() {
            if self.show_hud {
                let (w, h) = self.renderer.backend().target_size();
                view.draw(&mut self.renderer, layer, w, h)?;
            } else {
                layer.set_quads(&mut self.renderer, &[])?;
            }
        }
        let stats = self.renderer.render(&self.camera.camera(), self.clear)?;
        self.smooth.add(frame_ns as f64 * 1e-6, &stats);
        self.last = stats;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::Blocking;
    use render::NullBackend;

    type NoDisc = Session<NullBackend, &'static [u8]>;

    #[test]
    fn without_a_disc_the_message_is_shown() {
        let mut s: NoDisc =
            Session::new(NullBackend::new(1280, 720), Err("no disc image found: sdmc:/x.iso".into())).unwrap();
        assert!(s.world_names().is_empty());
        // Requests and world switches do nothing without a disc.
        s.request_world(&mut Blocking, 3);
        let c = Controls { next_world: true, ..Default::default() };
        assert!(s.update(&mut Blocking, &c, 1.0 / 60.0).unwrap());
        assert!(s.loading().is_none() && s.current().is_none());
        let lines = s.overlay_lines();
        assert_eq!(lines[0], "no disc image found: sdmc:/x.iso");
        let stats = s.render(16_666_667).unwrap();
        // The message and the stats lines are drawn as overlay glyphs, and nothing else.
        let chars: usize = lines.iter().map(|l| l.chars().filter(|c| *c != ' ').count()).sum();
        assert_eq!(stats.visible as usize, chars);
        assert!(!s.update(&mut Blocking, &Controls { quit: true, ..Default::default() }, 0.0).unwrap());
    }

    #[test]
    fn long_messages_wrap() {
        let long = "world 12 failed to load: corrupt data at offset 0x1234: a very long explanation of what went wrong";
        let mut s: NoDisc = Session::new(NullBackend::new(640, 480), Err(long.into())).unwrap();
        s.show_overlay = false;
        let lines = s.overlay_lines();
        assert!(lines.len() >= 2 && lines.iter().all(|l| l.len() <= 60), "{lines:?}");
        assert_eq!(lines.join(" ").replace("  ", " "), long);
    }
}
