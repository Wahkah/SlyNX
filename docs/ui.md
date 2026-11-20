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

## Sly 2 (`ui::sly2`)

A note for the crate's owner: Sly 2's interface lives in its own module, which uses only the crate's `DrawList`,
`nav` and `raster`, so nothing of Sly 1 changed. The hook is `pub mod sly2` in `lib.rs`. It is written from
`specs/sly2/user-interface.md`.

| Module | What it does |
|---|---|
| `font` | The two fonts of screen object 884 rasterised into one RGBA glyph sheet (`FontSheet`) |
| `hud` | Character gauge, coin counter, prompt with its one-slot queue, subtitle |
| `pause` | The pause menu (object 880), with the entries that apply |
| `jobs` | Job complete (1258's screen), saving (1492), job help (1248) |

- **Font sheet.** The game's glyphs are vector shapes in sixteenths of a pixel, baseline 0, drawn at text size /
  51.2.
  - `FontSheet::build` rasterises them with 4 × 4 supersampling into a 16-column sheet. That is about 1,072 ×
    1,344 for the two fonts; the size is computed from the glyphs.
  - The text font is stored white so a quad's colour tints it. The icon font keeps its colours.
  - Text layout knows the game's escapes `&x`, `&.`, `&2c` and `\n`, and wraps at spaces.
  - The sheet goes to the 2D layer as its glyph texture (`Texture::Glyphs`), as Sly 1's sheet does.
- **Layout.** The screens' own vector movies (`formats_sly2::screen`: shapes, sprites, labels, placements) are read
  but not played, because the placement fields' meanings are open. Panels, positions and sizes are SlyNX's; texts,
  fonts, timings (show, idle, hide phases on the real clock; 5 s counter idle; 1 s saving linger) and the menu
  entries are the game's.
- **Shapes** (user-interface.md 3, as corrected):
  - A primitive's topology (1 fan, 2 strip, 3 line strip) is the second of its three setting bytes. The first
    gives the draw layer, which matters only for the movies, and those are not played.
  - In a strip, a vertex byte of 255 starts a new strip (`formats_sly2::screen::Primitive::triangles`).
  - A primitive has one colour, its first vertex's.
  - Icons drawn after `&2` keep their own colours.
  - The placement colour pair and the text tint rules apply to the movies' own text, which is not drawn.
- **Tests:** unit tests of the sheet and layout, the gauge easing, the coin idle, the prompt queue, the pause
  entries and navigation, and the job screens.
  - `crates/game/tests/sly2_ui.rs` loads the hub on the null backend and checks the fonts (224 and 19 glyphs) and
    the layer. It opens the pause menu with +, checks the pause and the text, and closes it with Triangle.
  - With `SLYNX_TEST_OUT` the test rasterises the frame with `ui::raster` to `sly2_ui_pause.png`.

- **More screens** (`grid`, `shop`, `options`), shared by both games:
  - The gadget grid draws a view of `engine::gang::gadgets::Assignment`: each cell's icon, the selection, the slots
    and a message. The icons are the screen's own art: each cell's sprite (the one holding the cell record's three
    screen-part labels: open, owned, selected) is flattened per label by `game::sly2::movie` and baked into the
    glyph sheet as a third font (`grid::ICON_FONT`). It is placed where the screen's idle label (the one after the
    5572 marker) puts the sprite, and the title where it puts the 3151 text. The 4:3 canvas is scaled to fit and
    centred. Not-owned cells show the dimmed "open" label, the selected cell the label with the highlight; the
    spec gives no mark for assignable cells. Without the art the cells are plain squares.
  - The HUD hides while the grid, the shop or the options are open.
  - The shop has a Buy page and, in Sly 2 only, a Sell page switched with Triangle. Cross asks, and a second Cross
    buys. Circle cancels the question or closes the shop. A refusal message stays 5 s; six rows show at a time.
  - The options page has two volume sliders (Left and Right) and vibration on or off.
  - A screen takes input only once its show label has played (0.6 s).
- **Sly 2 session** (`crates/game/src/sly2/menus.rs`):
  - Select (− on the Switch, which no longer quits; Quit Game does) opens the current character's grid. The grid
    comes from the executable and is run by `Assignment`; L, ZL and R are L1, L2 and R2.
  - The gang is reached through a marked `PlayerSystem::gang_mut` hook.
  - The pause menu offers the shop in a hub, until the safe house's computer is played (our choice), and the
    options.
  - Purchases pay from the progress block, set its owned flag and give the item to the gang. Sales add the loot's
    value.
  - Options are stored in the block's `flags` (`uMusicVolume`, `uEffectsVolume`, `fNoVibration`). Both volumes 0
    means never set. The music volume goes to the sound engine; the effects volume and vibration are stored only.
- **Strips:** a 255 vertex byte is the GS's "no kick" vertex. It draws no triangle, and two in a row start a new
  strip. Reading it as "restart here" tore the Sly 2 text font.

### Front end (`ui::sly2::front`, both games)

`FrontMenu` is the title, the main page, the three-slot pages, the episode menu and its movie list. The sessions
hand it the texts (world strings as font codes) and the rows, and act on its `FrontAction`s.

- **Title:** the game's name and "Press START button for New Game" / "... to Resume Game" (Sly 2 strings 5 and 6,
  Sly 3 13 and 14), blinking. Start or Cross opens the main page.
- **Main page:** Continue (the newest slot), New Game, Load Game (when a slot is used).
- **Slots:** three rows, "Empty" when unused. A new game over a used slot asks "Do you want to overwrite this
  game?" first; Cross again confirms, Triangle or Circle cancels.
- **Episode menu:** the episodes the session allows, then "Watch a Movie" when there are movies.
- **Sly 2 session** (`crates/game/src/sly2/front.rs`):
  - `start_front` loads the boot world (row 0) as the backdrop, plays the logo movie `0x808` and opens the title.
  - New game, continue and load use the slots of the platform's store (`set_slots`, a `SlotAccess`); the chosen
    slot takes the autosaves and Save Game.
  - Loading the menu world (row 1) opens the episode menu. Episodes up to the highest selectable one (level-flow.md
    4) are listed with their titles (109 + n). A never-played episode plays its opening movie first, then its hub
    loads at point 393. The movie list has an opening and the episode's own movie per open episode, cut by 2 or 1
    by the spec's rule; the strings 119 to 128 are not matched to movies, so rows show the episode title and a
    number.
- **Sly 3 session** (`crates/game/src/sly3/front.rs`):
  - `start_front` loads `dvd_menu` (slot 0), plays the logos 338 and 339 and opens the title. The disc goes from
    the logos to the prologue before the title [O]; SlyNX shows the title first and plays the prologue as the new
    game's start (our choice).
  - New game is episode 1 with the opening not played (`m_ext`, task 1798). Continue and load start the saved
    `episodeCurrent` by `flow::episode_start`, with the values it reads taken from the save.
  - Loading `dvd_menu` opens the episode menu: the Hazard Room (171) and episodes 3 to 8 as "Episode 1" to
    "Episode 6" (5558 to 5563) with titles 164 to 169, up to `episodeBest` (at least episode 3; our choice). No
    movie list (unknown for Sly 3).
- **Movies** (`crate::movie::MovieScreen`, both sessions): Sly 2's file-table movies and Sly 3's movie table become
  `Clip`s of `Movies`; the picture goes to the level's video layer, black bars around it, and + skips. While a movie
  plays the world waits.
- **Switch:** both programs start at the title; a `world =` line in the settings file still starts directly in that
  world. The Sly 3 program no longer quits on −.
- **Tests:** `ui::sly2::front` unit tests (the pages, the overwrite question, continue, load, episodes and movies);
  disc tests `crates/game/tests/front_sly2.rs` and `front_sly3.rs` run the start-up, a new game, a save, continue or
  load, and the episode menu, rasterising each page with `SLYNX_TEST_OUT`.

### Sly 3 (`crates/game/src/sly3/layer.rs`)

Sly 3 runs Sly 2's screen runtime (`specs/sly3/user-interface.md`), so its session uses `ui::sly2` as it is.

- **Fonts** come from screen object 1061 (`stock_objects`), read through the offsets of `formats_sly3`'s screen
  elements. A Sly 3 font element is a `u8`, a `u16` count, and per character a `u16` code, an `f32` advance and a
  nested glyph element. A glyph is an `s32` followed by Sly 2's primitives. v_apt has three fonts: 92, 19 and 81
  glyphs.
- **What shows:**
  - the coins (screen 1048);
  - the subtitle (1060): the world string of a script's play-line command, until its clear-subtitle command or
    6 s. That limit is our choice: Sly 3 lines don't go through the sound engine here, so their duration is not
    at hand.
  - job complete (1066), from the job system's notices;
  - job help (1389) and the pause menu (1056: Keep Playing, Job Help, Abandon Job, Quit Game), with + (Start).
- **Text:** the 8-bit font codes take typographic quotes and dashes in their plain forms.
- **Also:**
  - the shop (1065) over the safe house's `safehouse::Shop` (no Sell page), buying through `Sly3Session::buy`;
  - the options page, with Sly 3's strings 119 and 120;
  - job failed (1067) when a script shows screen 1067, until the script hides it or Cross;
  - subtitles in the speaker's colour, from the class of the play-line command's object;
  - pause entries that show the world string whose text matches the entry, else English.
  - The game gets no input while a screen holds it, so the safe house ignores Cross while the shop is open.
- **Not yet:**
  - the prompt (1058) and the timer (1053): nothing in the Sly 3 session gives their text or time yet;
- **Gadget grids** (`crates/game/src/sly3/grid.rs`, screens 1062 to 1064):
  - The tables were not in the spec. They sit just before the slot table: three tables of 32-byte cells in Sly 2's
    layout, at 0x348BE0 (15 cells, screen 1062), 0x348DC0 (11, 1063) and 0x348F20 (8, 1064). They were found by
    the cells' labels, which are the labels of the grid screens' cell groups (for example 1174, 1173, 1175 for
    item 25). Each grid's character is its items' owner (`engine::gang::sly3::grids_from_executable`).
  - Select (−) opens the current character's grid through the crew's `gang_mut`; the d-pad moves, L, ZL and R
    assign, − or Circle closes. The crew is given the Sly 3 item table, and the owned items and slots from the
    progress block (`powerUps/mppukfAcquired`, `<character>/apukCur`) on every load and when the grid opens; an
    assignment writes the slots back.
  - Messages are strings 65 and 66 (Sly 2 now uses its 50 and 51 the same way).
  - **Icons:** `formats_sly3::screen` reads a Sly 3 screen's element set and group tree into Sly 2's screen form
    (elements as items: shapes, texts, group references as sprite values; groups as sprites, parts as labels; a
    key's items as placements: element, slot, flag and colour index, 127 for none; the key's trailing bytes as the
    slots it empties). The grid screens' files (1062 to 1064) then go through the same still renderer and baking as
    Sly 2's (`game::sly2::movie`, `layer::grid_art`), with Sly 3's idle marker 4869, into the sheet's third font, and
    the title is placed from the screen's 5337 text. Without the art the cells are plain squares at the table's
    positions moved by (38, −14) (the table is the screen position shifted by (−38, +14), used for navigation).
- **Test:** `crates/game/tests/sly3_ui.rs` mirrors `sly2_ui.rs`. It rasterises to `sly3_ui_pause.png` with
  `SLYNX_TEST_OUT`.

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
