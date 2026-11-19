//! The game's HUD (`crates/ui`) drawn through the renderer's UI layer, as a demo of the overlay pass.

use alloc::vec::Vec;

use formats::disc::Executable;
use formats::world::World;
use render::ui2d::{UiLayer, UiQuad};
use render::{Backend, RenderError, Renderer, TextureData};
use ui::FontSet;
use ui::draw::{Canvas, DrawList, Texture};
use ui::hud::{Hud, HudValues};
use ui::tables::{TextTables, read_placements};

/// Quads reserved for the UI layer (the HUD uses a few dozen; the binocucom up to a few hundred).
pub const UI_QUADS: usize = 1024;

/// What the HUD needs from the world and the executable, read on the loader thread.
#[derive(Clone, Debug)]
pub struct HudData {
    pub fonts: FontSet,
    pub tables: TextTables,
    pub placements: Vec<ui::tables::Placement>,
}

impl HudData {
    /// The fonts of `world` and the tables of the executable; `None` when either does not read.
    pub fn read(world: &World<'_>, exe: &Executable) -> Option<HudData> {
        Some(HudData {
            fonts: FontSet::from_world(world).ok()?,
            tables: TextTables::read(exe).ok()?,
            placements: read_placements(exe).ok()?,
        })
    }

    /// The glyph sheet as a renderer texture (clamp-to-edge, no mipmaps).
    pub fn sheet(&self) -> Option<TextureData> {
        let s = &self.fonts.sheet;
        TextureData::rgba8(s.width, s.height, s.pixels.clone()).ok().map(TextureData::clamped)
    }
}

/// A running HUD.
pub struct HudView {
    data: HudData,
    hud: Hud,
    list: DrawList,
    quads: Vec<UiQuad>,
}

impl HudView {
    /// A HUD with demo values (3 lives, no counters running), shown at start.
    pub fn new(data: HudData) -> HudView {
        let mut hud = Hud::new(data.placements.clone(), HudValues { lives: 3, ..Default::default() });
        hud.flash_counters();
        HudView { data, hud, list: DrawList::new(Canvas::for_display(1280, 720)), quads: Vec::new() }
    }

    /// Shows the counters again (they slide out after a few seconds).
    pub fn flash(&mut self) {
        self.hud.flash_counters();
    }

    pub fn update(&mut self, dt: f32) {
        self.hud.update(dt, true);
    }

    /// Draws into `layer` for a `width` x `height` target.
    pub fn draw<B: Backend>(
        &mut self,
        r: &mut Renderer<B>,
        layer: &mut UiLayer,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        self.list = DrawList::new(Canvas::for_display(width.max(1), height.max(1)));
        self.hud.draw(&self.data.fonts, &self.data.tables, &mut self.list);
        self.quads.clear();
        self.quads.extend(self.list.to_pixels(width.max(1)).iter().map(|q| UiQuad {
            rect: [q.rect.x, q.rect.y, q.rect.w, q.rect.h],
            uv: q.uv,
            textured: q.texture == Texture::Glyphs,
            color: q.color,
        }));
        layer.set_quads(r, &self.quads)
    }
}
