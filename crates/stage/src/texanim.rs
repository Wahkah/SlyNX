//! Texture animation (Sly 1 rendering.md 8, Sly 2 rendering.md 4): frame changes and UV motion of a material, played
//! on the game clock. Randomness comes from a generator seeded per animation, so runs are reproducible.
//! [`Animator`] plays a level's [`TexAnimDesc`]s on the renderer's materials.

use alloc::vec::Vec;

use formats::textures::AnimRecord;
use render::math::{cos, sin};
use render::{Backend, MaterialId, RenderError, Renderer, TextureId};

/// What an animation drives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    /// Kind 1: loop through the frames in `T` seconds (T random in [min, max]), pausing in between.
    Loop { t: [f32; 2], pause: [f32; 2], start: u16 },
    /// Kind 2: up through the frames and down again in `T`, pausing at frame 0.
    PingPong { t: [f32; 2], pause: [f32; 2], start: u16 },
    /// Kind 3: after each pause, jump forward by 1 to frames - 1.
    Random { pause: [f32; 2] },
    /// Kind 5: eyes open for a random time (or none, a double blink), then through all frames in `blink` seconds.
    Blink { blink: f32, open: [f32; 2], double: f32 },
    /// Kind 6: UV scroll, wrapped.
    Scroll { speed: [f32; 2], wrap: [f32; 2] },
    /// Kind 7: UV circle.
    Circle { w: f32, r: f32, centre: [f32; 2] },
    /// Kind 8 (eye look) and anything else: no motion without a look target.
    Still,
}

fn f(p: &[u8; 24], i: usize) -> f32 {
    f32::from_le_bytes([p[i * 4], p[i * 4 + 1], p[i * 4 + 2], p[i * 4 + 3]])
}

fn h(p: &[u8; 24], at: usize) -> u16 {
    u16::from_le_bytes([p[at], p[at + 1]])
}

impl Motion {
    /// The motion of an animation record, or `None` for "no animation".
    pub fn from_record(a: &AnimRecord) -> Option<Motion> {
        let p = &a.params;
        Some(match a.kind {
            1 => Motion::Loop { t: [f(p, 0), f(p, 1)], pause: [f(p, 2), f(p, 3)], start: h(p, 16) },
            2 => Motion::PingPong { t: [f(p, 0), f(p, 1)], pause: [f(p, 2), f(p, 3)], start: h(p, 16) },
            3 => Motion::Random { pause: [f(p, 0), f(p, 1)] },
            5 => Motion::Blink { blink: f(p, 0), open: [f(p, 1), f(p, 2)], double: f(p, 3) },
            6 => Motion::Scroll { speed: [f(p, 0), f(p, 1)], wrap: [f(p, 2), f(p, 3)] },
            7 => Motion::Circle { w: f(p, 0), r: f(p, 1), centre: [f(p, 2), f(p, 3)] },
            4 | 8 => Motion::Still,
            _ => return None,
        })
    }

    /// The motion of a Sly 2 node record (rendering.md 4): classes 139 to 144 behave as Sly 1's kinds 1, 2, 3, 5, 6
    /// and 7 with the same parameters (`params` = record +12 to +24, `extra` = +28, the first frame of 139 and 140).
    pub fn from_sly2(class: u16, params: [f32; 4], extra: i32) -> Option<Motion> {
        let p = params;
        let start = extra.clamp(0, u16::MAX as i32) as u16;
        Some(match class {
            139 => Motion::Loop { t: [p[0], p[1]], pause: [p[2], p[3]], start },
            140 => Motion::PingPong { t: [p[0], p[1]], pause: [p[2], p[3]], start },
            141 => Motion::Random { pause: [p[0], p[1]] },
            142 => Motion::Blink { blink: p[0], open: [p[1], p[2]], double: p[3] },
            143 => Motion::Scroll { speed: [p[0], p[1]], wrap: [p[2], p[3]] },
            144 => Motion::Circle { w: p[0], r: p[1], centre: [p[2], p[3]] },
            _ => return None,
        })
    }

    /// Whether it changes frames (as opposed to UVs).
    pub fn uses_frames(&self) -> bool {
        matches!(self, Motion::Loop { .. } | Motion::PingPong { .. } | Motion::Random { .. } | Motion::Blink { .. })
    }
}

/// A playing animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexAnim {
    pub motion: Motion,
    pub frames: u32,
    /// Seconds since the start.
    pub time: f32,
    /// Seconds left in the current pause (frame motions).
    pause: f32,
    /// Position in the current run, 0..1 (frame motions).
    phase: f32,
    /// Duration of the current run.
    period: f32,
    frame: u32,
    uv: [f32; 2],
    rng: u64,
}

impl TexAnim {
    pub fn new(motion: Motion, frames: u32, seed: u64) -> TexAnim {
        let mut a = TexAnim {
            motion,
            frames: frames.max(1),
            time: 0.0,
            pause: 0.0,
            phase: 0.0,
            period: 1.0,
            frame: 0,
            uv: [0.0; 2],
            rng: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
        };
        match motion {
            Motion::Loop { t, pause, start } | Motion::PingPong { t, pause, start } => {
                a.frame = (start as u32).min(a.frames - 1);
                a.period = a.uniform(t);
                a.pause = a.uniform(pause);
            }
            Motion::Random { pause } => a.pause = a.uniform(pause),
            Motion::Blink { open, .. } => a.pause = a.uniform(open),
            _ => {}
        }
        a
    }

    fn next(&mut self) -> f32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        ((self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32) / (1u64 << 24) as f32
    }

    fn uniform(&mut self, [a, b]: [f32; 2]) -> f32 {
        a + (b - a) * self.next()
    }

    /// The current frame (0 when the motion has none).
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// The current UV offset.
    pub fn uv(&self) -> [f32; 2] {
        self.uv
    }

    /// Advances by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        self.time += dt;
        let n = self.frames;
        match self.motion {
            Motion::Scroll { speed, wrap } => {
                for k in 0..2 {
                    let w = if wrap[k] != 0.0 { wrap[k] } else { 1.0 };
                    self.uv[k] = (self.uv[k] + speed[k] * dt) % w;
                }
            }
            Motion::Circle { w, r, centre } => {
                let a = w * self.time;
                self.uv = [sin(a) * r + centre[0], cos(a) * r + centre[1]];
            }
            Motion::Still => {}
            _ if n <= 1 => {}
            Motion::Loop { t, pause, .. } => {
                if self.pause > 0.0 {
                    self.pause -= dt;
                    return;
                }
                self.phase += dt / self.period.max(1e-3);
                if self.phase >= 1.0 {
                    self.phase = 0.0;
                    self.frame = 0;
                    self.period = self.uniform(t);
                    self.pause = self.uniform(pause);
                } else {
                    self.frame = ((self.phase * n as f32) as u32).min(n - 1);
                }
            }
            Motion::PingPong { t, pause, .. } => {
                if self.pause > 0.0 {
                    self.pause -= dt;
                    return;
                }
                self.phase += dt / self.period.max(1e-3);
                if self.phase >= 1.0 {
                    self.phase = 0.0;
                    self.frame = 0;
                    self.period = self.uniform(t);
                    self.pause = self.uniform(pause);
                } else {
                    // Up through all frames in the first half, down in the second.
                    let x = if self.phase < 0.5 { self.phase * 2.0 } else { (1.0 - self.phase) * 2.0 };
                    self.frame = ((x * n as f32) as u32).min(n - 1);
                }
            }
            Motion::Random { pause } => {
                self.pause -= dt;
                if self.pause <= 0.0 {
                    let jump = 1 + ((self.next() * (n - 1) as f32) as u32).min(n - 2);
                    self.frame = (self.frame + jump) % n;
                    self.pause = self.uniform(pause);
                }
            }
            Motion::Blink { blink, open, double } => {
                if self.pause > 0.0 {
                    self.pause -= dt;
                    if self.pause <= 0.0 {
                        self.phase = 0.0;
                    }
                    return;
                }
                self.phase += dt / blink.max(1e-3);
                if self.phase >= 1.0 {
                    self.frame = 0;
                    self.phase = 0.0;
                    self.pause = if self.next() < double { 1e-6 } else { self.uniform(open) };
                } else {
                    // Close and reopen through all frames.
                    let x = if self.phase < 0.5 { self.phase * 2.0 } else { (1.0 - self.phase) * 2.0 };
                    self.frame = ((x * n as f32) as u32).min(n - 1);
                }
            }
        }
    }
}

/// A texture animation driving one material: indices into a level's materials and textures.
#[derive(Clone, Debug, PartialEq)]
pub struct TexAnimDesc {
    pub material: u32,
    pub motion: Motion,
    /// Per frame: the texture and the palette images (for frame motions).
    pub frames: Vec<(u32, Option<[u32; 2]>)>,
}

/// Plays a level's texture animations on its uploaded materials.
#[derive(Clone, Debug, Default)]
pub struct Animator {
    anims: Vec<(TexAnim, TexAnimDesc, Option<u32>)>,
    materials: Vec<MaterialId>,
    textures: Vec<TextureId>,
}

impl Animator {
    /// The animations of a level whose materials and textures were uploaded as `materials` and `textures`.
    pub fn new(anims: &[TexAnimDesc], materials: &[MaterialId], textures: &[TextureId]) -> Animator {
        Animator {
            anims: anims
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let frames = a.frames.len().max(1) as u32;
                    (TexAnim::new(a.motion, frames, 0x5eed + i as u64), a.clone(), None)
                })
                .collect(),
            materials: materials.to_vec(),
            textures: textures.to_vec(),
        }
    }

    /// Advances every animation by `dt` seconds and writes the changes into the renderer's materials.
    pub fn animate<B: Backend>(&mut self, r: &mut Renderer<B>, dt: f32) -> Result<(), RenderError> {
        for (anim, desc, shown) in &mut self.anims {
            anim.advance(dt);
            let Some(&mat) = self.materials.get(desc.material as usize) else { continue };
            if desc.motion.uses_frames() {
                let f = anim.frame();
                if *shown != Some(f)
                    && let Some(&(t, p)) = desc.frames.get(f as usize)
                {
                    let tex = self.textures[t as usize];
                    let pal = p.map(|[a, b]| [self.textures[a as usize], self.textures[b as usize]]);
                    // Frames of another size land in another pool; the material then keeps its frame.
                    if r.set_material_images(
                        mat,
                        tex,
                        pal.filter(|_| r.material(mat).is_some_and(|m| m.palettes.is_some())),
                    )
                    .is_ok()
                    {
                        *shown = Some(f);
                    }
                }
            } else {
                r.set_material_uv_offset(mat, anim.uv())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_test_vector() {
        // Kind 6 with (0.2, -0.1, 1, 1) after 2.5 s: u 0.5, v -0.25 (the remainder keeps the sign).
        let mut a = TexAnim::new(Motion::Scroll { speed: [0.2, -0.1], wrap: [1.0, 1.0] }, 1, 1);
        for _ in 0..150 {
            a.advance(1.0 / 60.0);
        }
        assert!((a.uv()[0] - 0.5).abs() < 1e-3 && (a.uv()[1] + 0.25).abs() < 1e-3, "{:?}", a.uv());
    }

    #[test]
    fn loop_runs_through_every_frame_and_is_reproducible() {
        let m = Motion::Loop { t: [0.3, 0.3], pause: [0.0, 0.0], start: 0 };
        let mut a = TexAnim::new(m, 4, 7);
        let mut seen = [false; 4];
        for _ in 0..120 {
            a.advance(1.0 / 60.0);
            seen[a.frame() as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        let mut b = TexAnim::new(m, 4, 7);
        let mut c = TexAnim::new(m, 4, 7);
        for _ in 0..50 {
            b.advance(0.01);
            c.advance(0.01);
        }
        assert_eq!(b, c);
        // One frame: nothing moves.
        let mut one = TexAnim::new(Motion::Random { pause: [0.0, 0.01] }, 1, 3);
        one.advance(1.0);
        assert_eq!(one.frame(), 0);
    }
}
