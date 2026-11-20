//! The Sly 2 session's 2D layer (`render::ui2d` through [`crate::ui_layer::UiLayer`], as Sly 1's): the fonts of
//! screen object 884 rasterised into the layer's glyph sheet once per session, then every frame the HUD, the
//! subtitle, the job screens and the pause menu (`ui::sly2`).

use alloc::string::String;
use alloc::vec::Vec;

use engine::gang::gadgets::GridCell;
use formats::iso::ReadAt;
use formats_sly2::screen::{ItemBody, Screen};
use formats_sly2::stream::{Record, visit};
use render::{Backend, RenderError};
use ui::draw::{Canvas, DrawList};
use ui::sly2::font::GlyphShape;

use super::Sly2Session;

/// The screen object holding the two fonts (user-interface.md 4.2).
pub const FONT_SCREEN: i32 = 884;

/// The glyphs of the fonts in world `fw`'s object 884: coordinates from sixteenths of a pixel to font units. The
/// text font's glyphs are drawn black on the disc and coloured by their screen's colour pair, so they go into the
/// sheet white (coverage only); the icon font keeps each triangle's first vertex colour (alpha on the 0x80 scale).
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
        .enumerate()
        .map(|(fi, f)| {
            f.chars
                .iter()
                .zip(&f.advances)
                .zip(&f.glyphs)
                .map(|((&code, &advance), g)| {
                    let triangles = match &g.body {
                        ItemBody::Shape(prims) => glyph_triangles(prims, fi == 0),
                        _ => Vec::new(),
                    };
                    GlyphShape { code, advance, triangles }
                })
                .collect()
        })
        .collect();
    (!fonts.is_empty()).then_some(fonts)
}

/// A glyph's triangles in font units with their colours: one colour per primitive (user-interface.md 3). The text
/// font is drawn black on the disc and tinted by its screen, so it goes into the sheet white (its transparent
/// helper primitives stay transparent); the icon font keeps its colours (alpha on the 0x80 scale).
pub fn glyph_triangles(prims: &[formats_sly2::screen::Primitive], text_font: bool) -> Vec<([[f32; 2]; 3], [f32; 4])> {
    let mut out = Vec::new();
    for p in prims {
        let c = p.colour();
        let col = if text_font {
            [1.0, 1.0, 1.0, if c[3] > 0 { 1.0 } else { 0.0 }]
        } else {
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, (c[3] as f32 / 128.0).min(1.0)]
        };
        for t in p.triangles() {
            out.push((t.map(|i| [p.vertices[i].x as f32 / 16.0, p.vertices[i].y as f32 / 16.0]), col));
        }
    }
    out
}

/// The screen movies of world `fw` with the given object IDs, streamed ones read from `disc`.
pub fn screens<R: ReadAt>(
    fw: &formats_sly2::World<'_>,
    disc: &mut formats_sly2::Disc<R>,
    ids: &[i32],
) -> Vec<(i32, Screen)> {
    let mut found = Vec::new();
    for b in &fw.bodies {
        visit(&b.objects, &mut |r: &Record| {
            if r.class == 126 && ids.contains(&r.id) && !found.iter().any(|f: &(i32, Screen)| f.0 == r.id) {
                let start = r.options().map(|o| o.end);
                let end = (r.body_offset + r.body_size) as usize;
                if let Some(s) = start.and_then(|s| Screen::parse(fw.data, s, end).ok()) {
                    found.push((r.id, s));
                }
            }
        });
    }
    for (_, s) in &mut found {
        if let Some(f) = s.file {
            let ok = disc.read_file_decompressed(&f).ok().is_some_and(|file| s.parse_streamed(&file).is_ok());
            if !ok {
                s.items.clear();
                s.sprites.clear();
            }
        }
    }
    found
}

/// A gadget grid's art: each cell's icon and where the title goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridArt {
    /// Per cell of the executable's grid.
    pub icons: Vec<Option<ui::sly2::grid::Icon>>,
    /// The title's place and size (640 × 480).
    pub title: Option<([f32; 2], f32)>,
}

/// The glyph code of grid `grid`'s cell `cell` in state `state` (open, owned, selected) in the icon font.
pub fn icon_code(grid: usize, cell: usize, state: usize) -> u32 {
    (grid * 256 + cell * 4 + state) as u32
}

/// Bakes a gadget grid screen's art (safehouse-and-shop.md 6): each cell's sprite (the one holding the cell's three
/// screen-part labels) in each label, as icon-font glyphs centred on the sprite's origin; the cell's place from the
/// screen's idle label; the title's from its text item with `title` (string 3151).
pub fn grid_art(screen: &Screen, grid: usize, cells: &[GridCell], title: i32) -> (Vec<GlyphShape>, GridArt) {
    use super::movie::{IDENTITY, Tint, idle_label, label, scale_of, still};
    let mut glyphs = Vec::new();
    let mut art = GridArt::default();
    let root = idle_label(screen, 0).map(|l| still(screen, l, f32::MAX, &IDENTITY, &Tint::NONE));
    for (ci, c) in cells.iter().enumerate() {
        let sprite = screen.label(c.parts[0]).map(|(s, _)| s);
        let placed = sprite.and_then(|s| root.as_ref()?.sprites.iter().find(|p| p.0 == s).map(|p| p.1));
        let (Some(sprite), Some(m)) = (sprite, placed) else {
            art.icons.push(None);
            continue;
        };
        let mut codes = [0u32; 3];
        for (state, &part) in c.parts.iter().enumerate() {
            let Some(l) = label(screen, sprite, part) else { continue };
            let s = still(screen, l, f32::MAX, &IDENTITY, &Tint::NONE);
            codes[state] = icon_code(grid, ci, state);
            // The sheet keeps the first triangle to cover a sample: topmost first.
            let triangles = s
                .triangles
                .iter()
                .rev()
                .map(|(t, col)| {
                    (t.map(|p| [p[0] * ui::sly2::grid::ICON_SCALE, p[1] * ui::sly2::grid::ICON_SCALE]), *col)
                })
                .collect();
            glyphs.push(GlyphShape { code: codes[state], advance: 0.0, triangles });
        }
        art.icons.push(Some(ui::sly2::grid::Icon { codes, at: [m[4], m[5]], scale: scale_of(&m) }));
    }
    art.title = root
        .as_ref()
        .and_then(|r| r.texts.iter().find(|t| t.string == Some(title)))
        .map(|t| (t.at, t.floats[0] * t.scale));
    (glyphs, art)
}

/// Latin-1 bytes of a string.
pub fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// The glyph sheet of the Sly 2 fonts, once a world with them has loaded.
    pub fn font_sheet(&self) -> Option<&ui::sly2::font::FontSheet> {
        self.font.as_ref()
    }

    /// Whether the 2D layer exists (made with the level's resources).
    pub fn has_ui_layer(&self) -> bool {
        self.ui_layer.is_some()
    }

    /// The frame's UI draw list for a display of `width` × `height` pixels.
    pub fn ui_draw_list(&self, width: u32, height: u32) -> DrawList {
        let mut list = DrawList::new(Canvas::for_display(width, height));
        let font = self.font.as_ref();
        // Sly 2 hook (front end): a movie shows alone, with black around it; the title and the episode menu show
        // over the world without the HUD.
        if self.movie_playing() {
            crate::movie::movie_bars(&mut list, width, height, self.front.movies.rect());
            return list;
        }
        if let Some(m) = self.front.menu.as_ref().filter(|m| m.visible()) {
            m.draw(&mut list, font);
            return list;
        }
        let text = |id: i16| self.sound.as_ref().and_then(|s| s.string(id));
        let t = self.game.as_ref().map_or(0.0, |g| g.clock.game_time);
        let subtitle = self.sound.as_ref().and_then(|s| s.subtitle(t));
        // The HUD hides while a full screen (gadget grid, shop, options) is open, as the game's do.
        if !self.menus.open() {
            self.hud.draw(&mut list, font, subtitle.as_ref().map(|s| (s.text.as_str(), s.colour)));
        }
        self.screens.draw(&mut list, font, &text);
        self.pause.draw(&mut list, font, &|e| text(e.string()).map_or_else(|| latin1(e.fallback()), |s| latin1(&s)));
        // The menus over everything.
        let t = |id: i16, f: &str| latin1(&text(id).unwrap_or_else(|| alloc::string::String::from(f)));
        if let (Some(g), Some(view)) = (self.menus.assignment.as_ref(), self.grid_view()) {
            g.1.draw(&mut list, font, &view);
        }
        if let Some(s) = self.menus.shop.as_ref() {
            let labels = [
                t(3147, "ThiefNet"),
                t(3145, "Buy Gadgets"),
                t(3146, "Sell Loot"),
                latin1("Buy it? (X)"),
                latin1("Coins"),
            ];
            s.draw(&mut list, font, [&labels[0], &labels[1], &labels[2], &labels[3], &labels[4]]);
        }
        if let Some(o) = self.menus.options.as_ref() {
            // String 72 is the whole entry ("Vibrations: On"); the value is drawn on its own.
            let vibration = text(72).and_then(|v| v.split(':').next().map(String::from));
            let vibration = latin1(vibration.as_deref().unwrap_or("Vibrations"));
            let labels = [t(103, "Music Volume"), t(104, "Effects Volume"), vibration, latin1("On"), latin1("Off")];
            o.draw(&mut list, font, [&labels[0], &labels[1], &labels[2], &labels[3], &labels[4]]);
        }
        list
    }

    /// Draws the UI through the 2D layer (before the frame is rendered).
    pub(super) fn draw_ui(&mut self) -> Result<(), RenderError> {
        self.front.movies.present(&mut self.renderer)?; // Sly 2 hook: the movie's picture.
        let Some(mut layer) = self.ui_layer.take() else { return Ok(()) };
        let (w, h) = self.renderer.backend().target_size();
        let list = self.ui_draw_list(w, h);
        let r = layer.draw(&mut self.renderer, &list);
        self.ui_layer = Some(layer);
        r
    }
}
