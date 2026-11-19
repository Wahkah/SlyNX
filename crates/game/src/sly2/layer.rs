//! The Sly 2 session's 2D layer (`render::ui2d` through [`crate::ui_layer::UiLayer`], as Sly 1's): the fonts of
//! screen object 884 rasterised into the layer's glyph sheet once per session, then every frame the HUD, the
//! subtitle, the job screens and the pause menu (`ui::sly2`).

use alloc::vec::Vec;

use formats::iso::ReadAt;
use formats_sly2::screen::{ItemBody, Screen};
use formats_sly2::stream::{Record, visit};
use render::{Backend, RenderError};
use ui::draw::{Canvas, DrawList};
use ui::sly2::font::GlyphShape;

use super::Sly2Session;

/// The screen object holding the two fonts (user-interface.md 4.2).
pub const FONT_SCREEN: i32 = 884;

/// The glyphs of the fonts in world `fw`'s object 884: coordinates from sixteenths of a pixel to font units, each
/// triangle with its first vertex's colour (alpha on the 0x80 scale).
pub fn font_glyphs(fw: &formats_sly2::World<'_>) -> Option<Vec<Vec<GlyphShape>>> {
    let mut screen = None;
    for b in &fw.bodies {
        visit(&b.objects, &mut |r: &Record| {
            if screen.is_none() && r.class == 126 && r.id == FONT_SCREEN {
                let start = r.options().map(|o| o.end);
                screen = start.and_then(|s| Screen::parse(fw.data, s, (r.body_offset + r.body_size) as usize).ok());
            }
        });
    }
    let screen = screen?;
    let fonts: Vec<Vec<GlyphShape>> = screen
        .fonts()
        .map(|f| {
            f.chars
                .iter()
                .zip(&f.advances)
                .zip(&f.glyphs)
                .map(|((&code, &advance), g)| {
                    let mut triangles = Vec::new();
                    if let ItemBody::Shape(prims) = &g.body {
                        for p in prims {
                            for t in p.triangles() {
                                let v = t.map(|i| p.vertices[i]);
                                let c = v[0].rgba;
                                let col = [
                                    c[0] as f32 / 255.0,
                                    c[1] as f32 / 255.0,
                                    c[2] as f32 / 255.0,
                                    (c[3] as f32 / 128.0).min(1.0),
                                ];
                                triangles.push((v.map(|v| [v.x as f32 / 16.0, v.y as f32 / 16.0]), col));
                            }
                        }
                    }
                    GlyphShape { code, advance, triangles }
                })
                .collect()
        })
        .collect();
    (!fonts.is_empty()).then_some(fonts)
}

/// Latin-1 bytes of a string.
pub fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// The frame's UI draw list for a display of `width` × `height` pixels.
    pub fn ui_draw_list(&self, width: u32, height: u32) -> DrawList {
        let mut list = DrawList::new(Canvas::for_display(width, height));
        let font = self.font.as_ref();
        let text = |id: i16| self.sound.as_ref().and_then(|s| s.string(id));
        let t = self.game.as_ref().map_or(0.0, |g| g.clock.game_time);
        let subtitle = self.sound.as_ref().and_then(|s| s.subtitle(t));
        self.hud.draw(&mut list, font, subtitle.as_ref().map(|s| (s.text.as_str(), s.colour)));
        self.screens.draw(&mut list, font, &text);
        self.pause.draw(&mut list, font, &|e| text(e.string()).map_or_else(|| latin1(e.fallback()), |s| latin1(&s)));
        list
    }

    /// Draws the UI through the 2D layer (before the frame is rendered).
    pub(super) fn draw_ui(&mut self) -> Result<(), RenderError> {
        let Some(mut layer) = self.ui_layer.take() else { return Ok(()) };
        let (w, h) = self.renderer.backend().target_size();
        let list = self.ui_draw_list(w, h);
        let r = layer.draw(&mut self.renderer, &list);
        self.ui_layer = Some(layer);
        r
    }
}
