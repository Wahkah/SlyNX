//! Named apps the command-line runner can start, and the built-in `pattern` stub.
//!
//! The harness binary only knows its built-in apps. A binary that links the game registers the game's apps in its
//! own [`AppRegistry`] and calls [`crate::cli::main_with`]; see `docs/testing.md`.

use crate::app::{App, AppConfig, AppError, FrameContext};
use crate::hash::Fnv64;
use crate::image::Image;
use crate::input::Button;
use crate::perf::{DRAW_CALLS, TRIANGLES};

/// Creates an app from its configuration.
pub type AppFactory = fn(&AppConfig) -> Result<Box<dyn App>, AppError>;

/// A list of named apps.
#[derive(Clone, Default)]
pub struct AppRegistry {
    entries: Vec<(&'static str, &'static str, AppFactory)>,
}

impl AppRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The harness's built-in apps (`pattern`).
    pub fn builtin() -> Self {
        let mut r = Self::new();
        r.register("pattern", "software-rendered moving pattern; options: work=N (extra CPU work per frame)", |c| {
            Ok(Box::new(PatternApp::new(c)))
        });
        r
    }

    /// Adds an app. A later registration with the same name replaces the earlier one.
    pub fn register(&mut self, name: &'static str, description: &'static str, factory: AppFactory) {
        self.entries.retain(|(n, _, _)| *n != name);
        self.entries.push((name, description, factory));
    }

    /// Creates the app called `name`.
    pub fn create(&self, name: &str, config: &AppConfig) -> Result<Box<dyn App>, AppError> {
        match self.entries.iter().find(|(n, _, _)| *n == name) {
            Some((_, _, f)) => f(config),
            None => Err(AppError(format!("unknown app `{name}` (known: {})", self.names().join(", ")))),
        }
    }

    /// The registered names, in registration order.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|(n, _, _)| *n).collect()
    }

    /// `(name, description)` pairs.
    pub fn list(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.entries.iter().map(|(n, d, _)| (*n, *d))
    }
}

/// A tiny deterministic app for testing the harness itself: a square moved by the d-pad or left stick, sparkles
/// spawned from the run's RNG, and a scrolling background, rendered in software.
pub struct PatternApp {
    width: u32,
    height: u32,
    pos: [f32; 2],
    hue: u8,
    sparkles: Vec<Sparkle>,
    work: u32,
    work_sink: u64,
    frame: u64,
    target: Image,
}

#[derive(Clone, Copy)]
struct Sparkle {
    x: u32,
    y: u32,
    life: u8,
}

impl PatternApp {
    /// Pixels per second the square moves at full input.
    pub const SPEED: f32 = 60.0;

    /// Creates the app. Option `work=N` adds N rounds of hashing per update, to simulate CPU load.
    pub fn new(config: &AppConfig) -> Self {
        let (width, height) = (config.width.max(1), config.height.max(1));
        PatternApp {
            width,
            height,
            pos: [width as f32 / 2.0, height as f32 / 2.0],
            hue: (config.seed % 256) as u8,
            sparkles: Vec::new(),
            work: config.option("work").and_then(|v| v.parse().ok()).unwrap_or(0),
            work_sink: 0,
            frame: 0,
            target: Image::new(width, height, [0, 0, 0, 255]),
        }
    }
}

impl App for PatternApp {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let input = ctx.input;
        let mut dir = input.left_stick;
        dir[1] = -dir[1]; // stick +y is up; image +y is down
        for (b, d) in [
            (Button::Left, [-1.0, 0.0]),
            (Button::Right, [1.0, 0.0]),
            (Button::Up, [0.0, -1.0]),
            (Button::Down, [0.0, 1.0]),
        ] {
            if input.held(b) {
                dir[0] += d[0];
                dir[1] += d[1];
            }
        }
        let step = Self::SPEED * ctx.dt as f32;
        self.pos[0] = (self.pos[0] + dir[0] * step).clamp(0.0, self.width as f32 - 1.0);
        self.pos[1] = (self.pos[1] + dir[1] * step).clamp(0.0, self.height as f32 - 1.0);
        if input.just_pressed(Button::A) {
            self.hue = self.hue.wrapping_add(85);
        }
        if input.held(Button::B) {
            self.sparkles.clear();
        }
        self.sparkles.retain_mut(|s| {
            s.life -= 1;
            s.life > 0
        });
        if ctx.rng.chance(0.3) {
            let x = ctx.rng.below(self.width);
            let y = ctx.rng.below(self.height);
            self.sparkles.push(Sparkle { x, y, life: 20 + ctx.rng.below(20) as u8 });
        }
        if self.work > 0 {
            let mut h = Fnv64::new();
            h.write_u64(self.work_sink);
            for i in 0..self.work {
                h.write_u32(i);
            }
            self.work_sink = h.finish();
        }
        self.frame = ctx.frame + 1;
        Ok(())
    }

    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let (w, h) = (self.width as usize, self.height as usize);
        let shift = self.frame as usize;
        {
            let _bg = ctx.probe.scope("render.background");
            let px = self.target.pixels_mut();
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) * 4;
                    px[i] = ((x + shift) * 255 / w.max(1)) as u8;
                    px[i + 1] = (y * 255 / h.max(1)) as u8;
                    px[i + 2] = self.hue;
                    px[i + 3] = 255;
                }
            }
        }
        let size = (self.width.min(self.height) / 6).max(1);
        let half = (size / 2) as i32;
        let (cx, cy) = (self.pos[0].round() as i32, self.pos[1].round() as i32);
        self.target.fill_rect(cx - half, cy - half, size, size, [255, 255, 255, 255]);
        for s in &self.sparkles {
            let v = (s.life as u32 * 6).min(255) as u8;
            self.target.set(s.x, s.y, [v, v, 0, 255]);
        }
        let draws = 2 + self.sparkles.len() as u64;
        ctx.probe.count(DRAW_CALLS, draws);
        ctx.probe.count(TRIANGLES, draws * 2);
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        Ok(self.target.clone())
    }

    fn state_hash(&self) -> Option<u64> {
        let mut h = Fnv64::new();
        h.write_u64(self.frame).write_f32(self.pos[0]).write_f32(self.pos[1]).write(&[self.hue]);
        h.write_u64(self.work_sink);
        for s in &self.sparkles {
            h.write_u32(s.x).write_u32(s.y).write(&[s.life]);
        }
        Some(h.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_lookup() {
        let mut r = AppRegistry::builtin();
        assert_eq!(r.names(), vec!["pattern"]);
        let config = AppConfig { width: 8, height: 8, ..Default::default() };
        assert!(r.create("pattern", &config).is_ok());
        let e = r.create("nope", &config).err().unwrap();
        assert!(e.0.contains("pattern"));
        r.register("pattern", "replaced", |c| Ok(Box::new(PatternApp::new(c))));
        assert_eq!(r.list().count(), 1);
        assert_eq!(r.list().next().unwrap().1, "replaced");
    }
}
