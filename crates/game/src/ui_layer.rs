//! Draws the `ui` crate's draw lists (HUD, menus, subtitles, wipes) through the renderer's 2D layer
//! (`render::ui2d`): quads in pixels, in list order, one draw command.

use alloc::vec::Vec;

use render::ui2d::{UiLayer as Layer, UiQuad};
use render::{Backend, RenderError, Renderer, TextureData};
use ui::draw::{DrawList, Texture};

/// Most quads a frame draws (the binocucom vignette is the largest user, a few hundred).
pub const MAX_QUADS: usize = 1536;

/// The overlay.
pub struct UiLayer {
    layer: Layer,
    quads: Vec<UiQuad>,
}

impl UiLayer {
    /// Adds the layer with the glyph sheet (RGBA8 width × height). Call while the level's resources are being added,
    /// before `Renderer::commit`.
    pub fn new<B: Backend>(r: &mut Renderer<B>, sheet: Option<(u32, u32, Vec<u8>)>) -> Result<UiLayer, RenderError> {
        let tex = match sheet {
            Some((w, h, px)) if w > 0 && h > 0 => Some(TextureData::rgba8(w, h, px)?),
            _ => None,
        };
        Ok(UiLayer { layer: Layer::new(r, tex, MAX_QUADS)?, quads: Vec::new() })
    }

    /// Shows a draw list (replacing the last one).
    pub fn draw<B: Backend>(&mut self, r: &mut Renderer<B>, list: &DrawList) -> Result<(), RenderError> {
        let (w, _) = r.backend().target_size();
        self.quads.clear();
        self.quads.extend(list.to_pixels(w.max(1)).iter().map(|q| UiQuad {
            rect: [q.rect.x, q.rect.y, q.rect.w, q.rect.h],
            uv: q.uv,
            textured: q.texture == Texture::Glyphs,
            color: q.color,
        }));
        self.layer.set_quads(r, &self.quads)
    }
}
