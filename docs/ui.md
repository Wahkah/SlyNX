# The `ui` crate

`crates/ui` draws Sly 1's user interface:
- the fonts and their glyph sheet;
- text with the game's markup;
- the HUD and the binocucom overlay;
- the menus: title, pause, options, save/load/new, prompts, hideout select and map screen.

It produces a **backend-neutral draw list** that the renderer's overlay pass draws. It is written from
[`specs/sly1/ui-and-menus.md`](../specs/sly1/ui-and-menus.md), [`hud.md`](../specs/sly1/hud.md) and
[`textures.md`](../specs/sly1/textures.md), plus observation of the disc's data.

- `#![no_std]` with `alloc`, no third-party crates, no `unsafe`. It depends on `formats` (disc, world, bitmap
  decoding, SHA-256) and `platform-api` (controller types).
- The `std` feature (default) only turns on `formats/std`. The Switch build uses `--no-default-features`.

## Quick start

```rust
use ui::{Canvas, DrawList, FontSet, Hud, HudValues, Menus, MenuInput, TextTables};
use ui::tables::read_placements;

// Loading. Read the tables only from a supported executable: they check its SHA-256.
let world = formats::world::World::load(&level_bytes)?;
let fonts = FontSet::from_world(&world)?;            // or .merged(&hideout_fonts) for font 4
let tables = TextTables::read(&disc.executable)?;
let mut hud = Hud::new(read_placements(&disc.executable)?, HudValues { lives: 3, ..Default::default() });
let mut menus = Menus::default();
// Upload fonts.sheet (RGBA8, width x height, rows top to bottom) as one texture.

// Every frame:
hud.set_values(values);                               // counters that change slide in for 2.5 s
hud.update(dt, clock_running);
let actions = menus.update(&MenuInput::from_pad(&pad, &Default::default()), dt, &tables);
let mut list = DrawList::new(Canvas::for_display(width, height));
hud.draw(&fonts, &tables, &mut list);
menus.draw(&fonts, &tables, &mut list);
// Hand list.quads to the renderer's overlay pass.
```

## What the renderer needs to provide (overlay pass)

`render` is not changed by this crate. To draw a `DrawList`, its overlay pass needs to do the following.

1. **One extra texture: the glyph sheet.**
   - `FontSet::sheet` is RGBA8 with straight alpha, rows top to bottom (row 0 is v = 0), and power-of-two sizes.
     It is 1024 × 1024 for the game's fonts.
   - It changes only when the fonts are reloaded (a world change).
   - Sample it with **bilinear filtering and clamp-to-edge**, no mipmaps. The glyphs have one-texel gaps around
     them.
2. **Quads in canvas space.** Each `Quad` has these fields:
   - `rect`: x, y, w, h in canvas units, with y pointing down from the top left.
   - `uv`: `[u0, v0, u1, v1]`, the top-left and bottom-right of the texture area.
   - `texture`: `Texture::Glyphs` (the sheet) or `Texture::None`.
   - `color`: RGBA 0..1, multiplied with the texel. For `Texture::None` it is the flat colour.

   Map canvas units to pixels with `Canvas::pixels_per_unit(display_width)`: the canvas is 640 units wide and
   `640 × height / width` tall. `DrawList::to_pixels` does the scaling. Snapping glyph quads to whole pixels is
   welcome but not required.
3. **Order and state.**
   - Draw in list order.
   - Use alpha blending `SrcAlpha, OneMinusSrcAlpha`, with no depth test or write and no culling.
   - The colour goes to the framebuffer as given (no gamma conversion), like the debug draw.
4. **Batching.** A frame holds up to about a thousand quads (the binocucom vignette is the largest user, a few hundred bands).
   All of them can be one draw: a flat quad samples a white texel or skips sampling by its texture field, and
   the per-quad data is 4 floats of rect + 4 of uv + 4 of colour + 1 flag.
5. **Reference output.** `ui::raster::rasterize` is a CPU version of exactly this pass: bilinear sampling at
   texel centres and straight-alpha blending. A GPU result should match it within the usual image-comparison
   tolerance.

## API

| Module | Main items |
|---|---|
| `font` | `FontSet` (`from_world`, `pack`, `merged`, `glyph_uv`, `fonts`, `sheet`), `Font` (`glyph`, `metrics`, `scale`), `Glyph`, `FontMetrics`, `GlyphSheet`, `read_font`, `FontSource` |
| `text` | `tokenize` / `Token`, `markup_font`, `layout` → `TextLayout` (`glyphs`, `width`, `height`, `lines`, `draw`, `caret`), `TextStyle` (`font`, `scale`, `color`, `align`, `wrap`, `units_per_texel`), `Align`, `Typewriter` |
| `tables` | `TextTables::read` (`prompts`, `responses`, `two_state`, `movie_titles`, `episodes`, `levels`, title strings, `slot_format`, timer formats, hideout strings; `response`, `slot_description`, `timer_text`), `read_placements` → `Placement`, `Edge`, `sprintf`, `SlotInfo`, the `resp`, `prompt`, `pair` and `va` constants |
| `hud` | `Hud` (`set_values`, `flash_counters`, `set_paused`, `set_timer`, `set_saving`, `update`, `draw`, `counter_text`), `HudValues`, `Slide`, `Counter`, `place`, `place_phases`, `slide_rect`, `element` numbers |
| `binoc` | `Binocucom` (`open`, `close`, `update`, `draw`, `set_radio`, `heading`, `zoom`, `reticle`, `filter`, `markers`), `Target`, `Marker` |
| `nav` | `MenuInput` (`from_pad` with a `ButtonMap`), `Repeat`, `Dir` |
| `menu` | `Menus` (`open_title`, `open_pause`, `open_prompt`, `open_hideout`, `open_map`, `close`, `update` → `Vec<Action>`, `draw`, `screen`, `cursor`, `entries`), `PauseContext`, `Settings`, `Screen`, `SlotMode`, `Confirm`, `Action`, `MenuSound`, `Episode`, `MapLevel`, `map_neighbour`, `prompt_box` |
| `draw` | `DrawList`, `Quad`, `Rect`, `Canvas`, `Texture` |
| `raster` | `rasterize` |

### Behaviour

- **Fonts.** A font record holds 4 metric bytes: cell width, space width, line height, and the font's own number.
  Each glyph record is 8 bytes: `u16` code, then `s16` x, y (the cell's top-left) and width.
  - The bitmaps are stored bottom row first, but the glyph rows count from the top of the upright picture.
    `read_font` turns each bitmap upright, so the records apply as they are.
  - Every glyph is one line height tall.
  - The fonts are drawn at half a canvas unit per texel, so the 41-texel text line is 20.5 units, 41 pixels at
    1280 × 720.
- **Markup.** `&` digit switches font: `&0` font 0, `&1` font 2, `&2` font 1, `&3` font 3, `&4` font 4.
  - `&.` resets the font and colour.
  - `^dd` multiplies the scale by d.d until the matching `^.`.
  - `~RRGGBB` sets a colour, halved to 0..128 so `ff` is full intensity. `~.` resets it.
  - `&&`, `^^` and `~~` print the character. A newline breaks the line.
  - A malformed sequence prints its first character, then resets font and colour.
  - A switched font is scaled to the current line height. Smaller glyphs sit on the line's bottom.
- **Layout.**
  - Greedy word wrap at `wrap` units. Words longer than a line break between glyphs, and trailing spaces do not
    count.
  - Lines are aligned left, centre or right within the widest line.
  - `Typewriter` and the `visible` argument of `TextLayout::draw` type text out.
- **HUD.**
  - **Placement.** Records from the executable's table (37 × 20 bytes at `0x26C3E0`). x/y hints are measured
    from the edges, and 0 centres.
  - **Placed against another element.** The edge says on which side of the element being placed the other one
    lies: `Top`, it sits below the other; `Bottom`, above; `Left`, to its right; `Right`, to its left. If the other
    is not showing, its own entry is tried down the chain; a disappearing element counts only for one that is
    disappearing too.
  - **Slides.** 0.25 s in and out along a smoothstep, from fully past the slide edge. An element with no slide
    edge fades.
  - **Counters.**
    - They count toward the value at 6000 per second, with a 0.25 s "smack" pulse (scale up to 1.3) on arrival.
    - They appear on change and stay 2.5 s of running game clock. All enabled counters stay up while paused.
    - Texts in font 2: lives `L`n; coins `&3C&.`n; keys n`&3K&.`; bottles n`/`total then `?`, or `&3!&.` when
      complete; percent n`%`.
  - **Timer.** The executable's format, `m:ss`, or `0.t` below one second.
  - **Auto-save icon.** `^15&2a&.^.`, bottom left.
  - **Auto-save notice.** Starting a new game in a slot sets `Menus::autosave_notice_pending`. Before each
    auto-save the game calls `Menus::before_autosave()`: the first time it opens prompt 20 (the notice, answered
    with Continue as `Action::Answer { prompt: 20, response: 2 }`) and returns true, and the game saves once it is
    answered (with `Hud::set_saving` showing the icon); afterwards it returns false and the game saves at once. For
    a loaded slot that has never saved, set `autosave_notice_pending` from the save data.
  - Every HUD text gets a half-alpha black shadow one unit down-right.
- **Binocucom.** Markers are refreshed every 0.25 s for targets within 4000 units: scan points first, then nearest
  first, up to 32. The filter highlights one kind. The radio text types out at 30 characters/s with a white
  blinking `|`. The vignette, opening, reticle, compass strip and zoom readout are our own design; the spec
  leaves the layout open.
- **Navigation.**
  - The direction pad, or the stick past 0.8, gives a move at once.
  - Holding repeats after 1/3 s, then each interval is 2/3 of the last, never below 0.1 s.
  - The stick keeps a held direction while it stays past 0.25.
  - Default buttons (`ButtonMap::default`), the Switch convention: A accepts, B backs out, Y erases, + is Start,
    − is Select.
- **Menus.**
  - Pause entries follow the ui-and-menus.md 4 rules: restart race, sprint or level, view map, exit level or exit
    to hideout, and Quit Game or Save and Quit. Options shows New Game only from the title and Save Game only
    from in game.
  - A slot list holds the three slots and Back. A used slot asks to overwrite (saving, new game); Erase asks to
    erase. An empty slot cannot be loaded (refuse sound).
  - Yes/no questions default to No.
  - Any table prompt opens with `open_prompt`; its answer comes back as `Action::Answer`.
  - The hideout select stops at the first and last unlocked episode and wraps only when all are unlocked. Accept
    enters a visited episode or plays the newest one's introduction. The totals panel shows "Bentley" on the
    prologue and "Murray" on an unvisited episode.
  - On the map, a direction moves to the nearest unlocked level within 60° of it.
  - Slot descriptions use the executable's `%d%% - %s - %d:%02d` and `Empty`.
  - Menu sounds come out as `Action::Sound(MenuSound::…)`; the game maps them to sound IDs.

## Not yet done

- **Menu art.** The prompt box, highlight, map markers and hideout layout are plain shapes of our own. The real
  map pictures (class 103) and hideout panels (class 105) need their data layouts (open question 3 of
  ui-and-menus.md).
- **Movie menu, credits, letterbox, television panels, minigame counters (elements 8 to 15) and the sprint
  note.** The placement table and the slide machinery cover them, but nothing fills them yet.
- **Center Screen** only stores an offset in `Settings::center`; the game applies it.
- **The attract loop and the logo/legal screens.**

## Tests

- **Unit tests** (`cargo test -p ui`) use a synthetic five-font set. They cover:
  - markup tokens, including malformed sequences;
  - layout: advance, scale, switched fonts, colour, newlines, wrap, long words, alignment, the typewriter;
  - glyph-sheet packing and lookup;
  - `sprintf`, slot and timer texts;
  - slide motion, placement (including the against rule and shrinking), counter counting, time-out and pause;
  - HUD drawing and binocucom markers and radio;
  - repeat timing (1/3, 2/9, 4/27, then 0.1 s) and stick thresholds;
  - pause entry rules, options from the title, the save / overwrite / erase flow, quitting, prompt answers,
    hideout bounds and wrap, map neighbours, drawing every screen;
  - the CPU rasterizer.
- **Disc tests** (`tests/disc.rs`, skipped without `SLYNX_DISC_SLY1`):
  - Fonts: every world's fonts (sizes, metrics, scale, glyph counts and codes), the SHA-256 of each decoded
    font picture, the glyph sheet of Paris, and the spec's `&2X&. : New Game` layout.
  - Text tables: all three, with check values and SHA-256 of each string group.
  - Placements: the spec's selected placement records.
- **Captures.** The example `ui_capture` is a harness app that rasterizes on the CPU with the disc's own fonts and
  texts. Views: `hud`, `binoc`, `title`, `pause`, `options`, `slots`, `hideout`, `map`, `sheet`. Write the
  captures outside the repository:

  ```sh
  SLYNX_DISC_SLY1=<disc.iso> cargo run --release -p ui --example ui_capture -- run --app ui --frames 40 \
      --capture 39 --out <folder outside the repo> --opt view=hud --width 1280 --height 720
  ```

## Observations for the spec

Settled by the specs (textures.md 2.6, ui-and-menus.md 1.1 and 2, hud.md 2) and now followed by the code:

1. **Glyph records:** `u16` code, then `s16` x, y and width; (x, y) is the top-left of the cell, from the top of the
   upright picture; bitmaps are stored bottom row first; metric byte 3 is the font number (`font::parse_glyph`).
2. **Fifth font** in 13 worlds; **font 3** has six pictures, one per part of the game, so it always comes from the
   world being played (`FontSet::merged` keeps a set's own fonts and only adds missing ones).
3. **Placement:** the edge field says on which side of the element being placed the other lies, so element 17 (the
   note) sits above the timer. If the other element is not showing, its own "placed against" entry is tried down
   the chain (the sprint prize falls back from the keys to the clue counter). A disappearing element counts only
   while the element being placed is disappearing too (`hud::place_phases`).
4. **Movie titles:** the 16 strings at `0x24B438..0x24B578` are the movie menu's titles in movie order, read
   through the pointer table at `0x2699C0` (`TextTables::movie_titles`); the episode names are slots 4, 6, 8, 10
   and 12.
5. **Prompt layout:** the prompt record's fourth word is the response layout (`Prompt::layout`, `vertical()`): 1 a
   vertical list chosen with up / down, 0 side by side chosen with left / right (the yes/no questions and the sprint
   result). `prompt_box` draws either. Back on a prompt that offers No answers No; on others it dismisses (answer
   -1), the port's choice.

Other strings the UI reads, by address: the title text `0x24CEF0`; "Press START…" `0x24CF18` and `0x24CF38`;
"Press SELECT…" `0x24CF60`; the slot format `0x24C418`.
