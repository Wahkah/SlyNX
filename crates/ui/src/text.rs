//! Text markup (ui-and-menus.md 1.2) and layout: font switches, scale, colour, word wrap, alignment and text that
//! types itself out.

use alloc::vec::Vec;

use crate::draw::{DrawList, Rect, Texture};
use crate::font::{FontSet, Glyph};

/// Canvas units per font texel at scale 1 (the fonts are drawn at half their texel size on the 640-wide frame).
pub const UNITS_PER_TEXEL: f32 = 0.5;

/// One markup token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Token {
    /// A character to draw.
    Char(u16),
    /// Switch to font index `n` of the font set.
    Font(u8),
    /// `&.`: back to the base font and colour.
    FontReset,
    /// `^dd`: multiply the scale by d.d.
    ScalePush(f32),
    /// `^.`: undo the last scale.
    ScalePop,
    /// `~RRGGBB`: colour, in the console's 0..128 range per channel.
    Color([u8; 3]),
    /// `~.`: back to the base colour.
    ColorReset,
    /// A line break.
    Newline,
}

/// The font a `&` digit selects: `&0` font 0, `&1` font 2 (numerals), `&2` font 1 (buttons), `&3` font 3, `&4`
/// font 4.
pub fn markup_font(digit: u8) -> Option<u8> {
    match digit {
        b'0' => Some(0),
        b'1' => Some(2),
        b'2' => Some(1),
        b'3' => Some(3),
        b'4' => Some(4),
        _ => None,
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Halves an 8-bit channel to the 0..128 range (`ff` gives 128, full intensity).
pub fn halve(v: u8) -> u8 {
    (v as u16).div_ceil(2) as u8
}

/// Splits a string (bytes; codes above 127 are the font's accented letters) into tokens. A malformed sequence
/// prints its first character and then resets font and colour.
pub fn tokenize(text: &[u8]) -> Vec<Token> {
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    let malformed = |out: &mut Vec<Token>, c: u8| {
        out.push(Token::Char(c as u16));
        out.push(Token::FontReset);
    };
    while i < text.len() {
        let c = text[i];
        let next = text.get(i + 1).copied();
        match c {
            b'\n' => {
                out.push(Token::Newline);
                i += 1;
            }
            b'&' => match next {
                Some(b'&') => {
                    out.push(Token::Char(b'&' as u16));
                    i += 2;
                }
                Some(b'.') => {
                    out.push(Token::FontReset);
                    i += 2;
                }
                Some(d) if markup_font(d).is_some() => {
                    out.push(Token::Font(markup_font(d).unwrap_or(0)));
                    i += 2;
                }
                _ => {
                    malformed(&mut out, c);
                    i += 1;
                }
            },
            b'^' => match (next, text.get(i + 2).copied()) {
                (Some(b'^'), _) => {
                    out.push(Token::Char(b'^' as u16));
                    i += 2;
                }
                (Some(b'.'), _) => {
                    out.push(Token::ScalePop);
                    i += 2;
                }
                (Some(a @ b'0'..=b'9'), Some(b @ b'0'..=b'9')) => {
                    out.push(Token::ScalePush((a - b'0') as f32 + (b - b'0') as f32 / 10.0));
                    i += 3;
                }
                _ => {
                    malformed(&mut out, c);
                    i += 1;
                }
            },
            b'~' => match next {
                Some(b'~') => {
                    out.push(Token::Char(b'~' as u16));
                    i += 2;
                }
                Some(b'.') => {
                    out.push(Token::ColorReset);
                    i += 2;
                }
                _ => {
                    let digits: Option<Vec<u8>> =
                        text.get(i + 1..i + 7).map(|s| s.iter().filter_map(|&h| hex(h)).collect());
                    match digits {
                        Some(d) if d.len() == 6 => {
                            let ch = |k: usize| halve(d[k] * 16 + d[k + 1]);
                            out.push(Token::Color([ch(0), ch(2), ch(4)]));
                            i += 7;
                        }
                        _ => {
                            malformed(&mut out, c);
                            i += 1;
                        }
                    }
                }
            },
            _ => {
                out.push(Token::Char(c as u16));
                i += 1;
            }
        }
    }
    out
}

/// Horizontal alignment of lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Lines start at the block's left.
    #[default]
    Left,
    /// Lines are centred in the block.
    Center,
    /// Lines end at the block's right.
    Right,
}

/// How to lay text out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Base font (index in the font set).
    pub font: usize,
    /// Text scale.
    pub scale: f32,
    /// Base colour (RGBA 0..1).
    pub color: [f32; 4],
    /// Line alignment.
    pub align: Align,
    /// Wrap at this width (canvas units), if any.
    pub wrap: Option<f32>,
    /// Canvas units per texel at scale 1.
    pub units_per_texel: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            font: 0,
            scale: 1.0,
            color: [1.0; 4],
            align: Align::Left,
            wrap: None,
            units_per_texel: UNITS_PER_TEXEL,
        }
    }
}

impl TextStyle {
    /// This style with another scale.
    pub fn scaled(self, scale: f32) -> TextStyle {
        TextStyle { scale, ..self }
    }

    /// This style with another alignment.
    pub fn aligned(self, align: Align) -> TextStyle {
        TextStyle { align, ..self }
    }

    /// This style wrapping at `width`.
    pub fn wrapped(self, width: f32) -> TextStyle {
        TextStyle { wrap: Some(width), ..self }
    }
}

/// A glyph placed by the layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedGlyph {
    /// Font index.
    pub font: usize,
    /// The glyph.
    pub glyph: Glyph,
    /// Where, relative to the block's top-left.
    pub rect: Rect,
    /// Colour.
    pub color: [f32; 4],
}

/// Laid-out text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    /// Glyphs in reading order.
    pub glyphs: Vec<PlacedGlyph>,
    /// Block width (the widest line).
    pub width: f32,
    /// Block height.
    pub height: f32,
    /// Number of lines.
    pub lines: usize,
}

#[derive(Clone, Copy, Debug)]
enum Atom {
    Glyph(PlacedGlyph, f32),
    Space(f32, f32),
    Break(f32),
}

impl TextLayout {
    /// Draws the first `visible` glyphs (all with `None`) with the block's top-left at (x, y).
    pub fn draw(&self, fonts: &FontSet, list: &mut DrawList, x: f32, y: f32, visible: Option<usize>, alpha: f32) {
        let n = visible.unwrap_or(usize::MAX);
        for g in self.glyphs.iter().take(n) {
            let mut color = g.color;
            color[3] *= alpha;
            list.textured(g.rect.offset(x, y), Texture::Glyphs, fonts.glyph_uv(g.font, &g.glyph), color);
        }
    }

    /// The right edge of glyph `n - 1` relative to the block (for a typing cursor), or 0.
    pub fn caret(&self, n: usize) -> (f32, f32) {
        match n.checked_sub(1).and_then(|i| self.glyphs.get(i)) {
            Some(g) => (g.rect.right(), g.rect.y),
            None => (0.0, 0.0),
        }
    }
}

/// Lays out `text` (with markup) in `fonts` with `style`.
pub fn layout(fonts: &FontSet, text: &[u8], style: &TextStyle) -> TextLayout {
    if fonts.fonts.is_empty() {
        return TextLayout::default();
    }
    let base = style.font.min(fonts.fonts.len() - 1);
    let base_font = &fonts.fonts[base];
    let base_line = base_font.line_height() * base_font.scale;
    let mut font = base;
    let mut color = style.color;
    let mut scales: Vec<f32> = Vec::new();
    let mut atoms: Vec<Atom> = Vec::new();
    for tok in tokenize(text) {
        let scale = style.scale * scales.iter().product::<f32>();
        // Height of a line at the current scale, in canvas units.
        let line_h = base_line * scale * style.units_per_texel;
        match tok {
            Token::Font(f) => font = (f as usize).min(fonts.fonts.len() - 1),
            Token::FontReset => {
                font = base;
                color = style.color;
            }
            Token::ScalePush(s) => scales.push(s),
            Token::ScalePop => {
                scales.pop();
            }
            Token::Color(c) => {
                color = [
                    (c[0] as f32 / 128.0).min(1.0),
                    (c[1] as f32 / 128.0).min(1.0),
                    (c[2] as f32 / 128.0).min(1.0),
                    style.color[3],
                ];
            }
            Token::ColorReset => color = style.color,
            Token::Newline => atoms.push(Atom::Break(line_h)),
            Token::Char(code) => {
                let f = &fonts.fonts[font];
                // The base font draws at its own scale; a switched font is sized to the current line height.
                let k = line_h / f.line_height().max(1.0);
                if code == b' ' as u16 {
                    atoms.push(Atom::Space(f.metrics.space_width as f32 * k, line_h));
                } else if let Some(g) = f.glyph(code) {
                    let rect = Rect::new(0.0, 0.0, g.w as f32 * k, line_h);
                    atoms.push(Atom::Glyph(PlacedGlyph { font, glyph: *g, rect, color }, line_h));
                } else {
                    atoms.push(Atom::Space(f.metrics.cell_width as f32 * k, line_h));
                }
            }
        }
    }

    // Break into lines: (start, end) atom ranges.
    let mut lines: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    let mut width = 0.0f32;
    let mut i = 0;
    let wrap = style.wrap.unwrap_or(f32::INFINITY);
    while i < atoms.len() {
        match atoms[i] {
            Atom::Break(_) => {
                lines.push((start, i));
                start = i + 1;
                width = 0.0;
                i += 1;
            }
            Atom::Space(w, _) => {
                width += w;
                i += 1;
            }
            Atom::Glyph(..) => {
                // A word: glyphs up to the next space or break.
                let mut j = i;
                let mut ww = 0.0;
                while let Some(Atom::Glyph(g, _)) = atoms.get(j) {
                    ww += g.rect.w;
                    j += 1;
                }
                if width + ww > wrap && width > 0.0 {
                    // Start a new line at this word, dropping the spaces before it.
                    let mut end = i;
                    while end > start && matches!(atoms[end - 1], Atom::Space(..)) {
                        end -= 1;
                    }
                    lines.push((start, end));
                    start = i;
                    width = 0.0;
                }
                if ww > wrap && width == 0.0 {
                    // A word wider than the line: break it between glyphs.
                    let mut k = i;
                    let mut lw = 0.0;
                    while k < j {
                        let Atom::Glyph(g, _) = atoms[k] else { break };
                        if lw + g.rect.w > wrap && lw > 0.0 {
                            lines.push((start, k));
                            start = k;
                            lw = 0.0;
                        }
                        lw += g.rect.w;
                        k += 1;
                    }
                    width = lw;
                } else {
                    width += ww;
                }
                i = j;
            }
        }
    }
    lines.push((start, atoms.len()));

    let default_h = base_line * style.scale * style.units_per_texel;
    let mut out = TextLayout { lines: lines.len(), ..TextLayout::default() };
    let mut placed: Vec<(usize, f32, f32)> = Vec::new(); // (first glyph index, line width, line y)
    let mut y = 0.0;
    for &(a, b) in &lines {
        // Trailing spaces do not count.
        let mut b2 = b;
        while b2 > a && matches!(atoms[b2 - 1], Atom::Space(..)) {
            b2 -= 1;
        }
        let line_h = atoms[a..b]
            .iter()
            .map(|t| match *t {
                Atom::Glyph(_, h) | Atom::Space(_, h) | Atom::Break(h) => h,
            })
            .fold(0.0f32, f32::max);
        let line_h = if line_h > 0.0 { line_h } else { default_h };
        let first = out.glyphs.len();
        let mut x = 0.0;
        for t in &atoms[a..b2] {
            match *t {
                Atom::Glyph(mut g, h) => {
                    // Smaller glyphs sit on the line's bottom.
                    g.rect.x = x;
                    g.rect.y = y + line_h - h;
                    x += g.rect.w;
                    out.glyphs.push(g);
                }
                Atom::Space(w, _) => x += w,
                Atom::Break(_) => {}
            }
        }
        placed.push((first, x, y));
        out.width = out.width.max(x);
        y += line_h;
    }
    out.height = y;
    let block = out.width;
    for (li, &(first, w, _)) in placed.iter().enumerate() {
        let end = placed.get(li + 1).map_or(out.glyphs.len(), |p| p.0);
        let dx = match style.align {
            Align::Left => 0.0,
            Align::Center => (block - w) * 0.5,
            Align::Right => block - w,
        };
        for g in &mut out.glyphs[first..end] {
            g.rect.x += dx;
        }
    }
    out
}

/// Text that types itself out at a fixed rate (the binocucom radio text, briefings).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typewriter {
    /// Characters per second.
    pub rate: f32,
    elapsed: f32,
}

impl Typewriter {
    /// Starts typing at `rate` characters per second.
    pub fn new(rate: f32) -> Typewriter {
        Typewriter { rate, elapsed: 0.0 }
    }

    /// Advances time.
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;
    }

    /// Shows everything at once.
    pub fn finish(&mut self) {
        self.elapsed = f32::INFINITY;
    }

    /// Restarts from nothing.
    pub fn restart(&mut self) {
        self.elapsed = 0.0;
    }

    /// Glyphs visible now.
    pub fn visible(&self) -> usize {
        let n = self.elapsed * self.rate;
        if n.is_finite() { n.max(0.0) as usize } else { usize::MAX }
    }

    /// Whether all `total` glyphs show.
    pub fn done(&self, total: usize) -> bool {
        self.visible() >= total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::tests::test_set;

    #[test]
    fn tokens() {
        use Token::*;
        assert_eq!(
            tokenize(b"&2X&. : New"),
            [
                Font(1),
                Char(b'X' as u16),
                FontReset,
                Char(b' ' as u16),
                Char(b':' as u16),
                Char(b' ' as u16),
                Char(78),
                Char(101),
                Char(119)
            ]
        );
        assert_eq!(tokenize(b"^15a^."), [ScalePush(1.5), Char(b'a' as u16), ScalePop]);
        assert_eq!(tokenize(b"~ffffffA~."), [Color([128, 128, 128]), Char(65), ColorReset]);
        assert_eq!(tokenize(b"~b2a83d"), [Color([89, 84, 31])]);
        assert_eq!(tokenize(b"&&^^~~"), [Char(b'&' as u16), Char(b'^' as u16), Char(b'~' as u16)]);
        assert_eq!(tokenize(b"a\nb"), [Char(97), Newline, Char(98)]);
        // Malformed: the first character prints, then font and colour reset.
        assert_eq!(tokenize(b"&9"), [Char(b'&' as u16), FontReset, Char(b'9' as u16)]);
        assert_eq!(tokenize(b"~12"), [Char(b'~' as u16), FontReset, Char(b'1' as u16), Char(b'2' as u16)]);
        assert_eq!(tokenize(b"^1"), [Char(b'^' as u16), FontReset, Char(b'1' as u16)]);
        assert_eq!(markup_font(b'1'), Some(2));
        assert_eq!(markup_font(b'2'), Some(1));
    }

    #[test]
    fn layout_basics() {
        let f = test_set();
        let s = TextStyle::default();
        let l = layout(&f, b"AB C", &s);
        // Font 0: glyphs 10 texels wide, line 20, half a unit per texel; space 5 texels.
        assert_eq!(l.glyphs.len(), 3);
        assert_eq!(l.lines, 1);
        assert_eq!(l.glyphs[1].rect.x, 5.0);
        assert_eq!(l.glyphs[2].rect.x, 12.5);
        assert_eq!(l.width, 17.5);
        assert_eq!(l.height, 10.0);
        // Scale and a switched font sized to the line.
        let l = layout(&f, b"^20A^.&2X", &s);
        assert_eq!(l.glyphs[0].rect.h, 20.0);
        assert_eq!(l.glyphs[1].font, 1);
        assert_eq!(l.glyphs[1].rect.h, 10.0);
        assert_eq!(l.glyphs[1].rect.y, 10.0, "the smaller glyph sits on the line's bottom");
        // Colour.
        let l = layout(&f, b"~ff0000A~.B", &s);
        assert_eq!(l.glyphs[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(l.glyphs[1].color, [1.0; 4]);
        // Newlines.
        let l = layout(&f, b"A\nB\n", &s);
        assert_eq!(l.lines, 3);
        assert_eq!(l.glyphs[1].rect.y, 10.0);
    }

    #[test]
    fn wrap_and_align() {
        let f = test_set();
        // Words of 3 glyphs = 15 units; spaces 2.5; wrap at 35 fits two words.
        let s = TextStyle::default().wrapped(35.0);
        let l = layout(&f, b"AAA BBB CCC", &s);
        assert_eq!(l.lines, 2);
        assert_eq!(l.glyphs[6].rect.x, 0.0);
        assert_eq!(l.glyphs[6].rect.y, 10.0);
        assert_eq!(l.width, 32.5);
        // A long word breaks between glyphs.
        let l = layout(&f, b"AAAAAAAAAA", &TextStyle::default().wrapped(20.0));
        assert_eq!(l.lines, 3);
        // Centre and right alignment use the widest line.
        let l = layout(&f, b"AAAA\nA", &TextStyle::default().aligned(Align::Center));
        assert_eq!(l.glyphs[4].rect.x, 7.5);
        let l = layout(&f, b"AAAA\nA", &TextStyle::default().aligned(Align::Right));
        assert_eq!(l.glyphs[4].rect.x, 15.0);
    }

    #[test]
    fn typewriter() {
        let f = test_set();
        let l = layout(&f, b"ABCDEF", &TextStyle::default());
        let mut t = Typewriter::new(20.0);
        t.update(0.1);
        assert_eq!(t.visible(), 2);
        let mut d = DrawList::default();
        l.draw(&f, &mut d, 0.0, 0.0, Some(t.visible()), 1.0);
        assert_eq!(d.quads.len(), 2);
        assert_eq!(l.caret(2).0, 10.0);
        t.finish();
        assert!(t.done(6));
    }
}
