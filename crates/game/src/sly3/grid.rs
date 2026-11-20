//! The Sly 3 gadget assignment screen (screens 1062 to 1064; safehouse-and-shop.md 4): Sly 2's screen
//! (`ui::sly2::grid`, `engine::gang::gadgets::Assignment`) on Sly 3's grids (`engine::gang::sly3::grids_from_executable`)
//! and the crew's core characters (`gang::sly3::Crew`, reached through the player system's `gang_mut`).
//!
//! - **Select** (− on the Switch) opens the current character's grid during play; − or Circle closes it. The d-pad
//!   moves to the nearest owned cell, L, ZL and R put the selected item on L1, L2 and R2.
//! - **Ownership and slots** live in the progress block (`powerUps/mppukfAcquired`, `<character>/apukCur`; saving.md
//!   2): the crew is given them when a world loads and when the screen opens, and an assignment writes the slots
//!   back.
//! - **Messages** are the world's strings 65 ("Must purchase a powerup before assigning it to a button.") and 66
//!   ("This powerup does not need to be assigned to a button.").
//! - **Icons** come from the screens' own art, as Sly 2's: the grid screens' files (`formats_sly3::screen`, read into
//!   Sly 2's screen form) give each cell's sprite in its three labels, baked into the glyph sheet's third font with
//!   the fonts ([`bake`]), placed where the screen's idle label (after marker 4869) puts them. Without them the
//!   cells are plain squares at the executable's positions moved by (38, −14) to the screen's (the cell table is the
//!   screen position shifted by (−38, +14)).

use alloc::string::String;
use alloc::vec::Vec;

use engine::game::systems::FrameInput;
use engine::gang::Who;
use engine::gang::gadgets::{AssignMessage, Assignment, GridCell};
use engine::gang::sly3::{Member, Sly3ItemTable};
use formats::iso::ReadAt;
use platform_api::Buttons;
use render::Backend;
use save::sly2::SaveValue;
use ui::sly2::grid::{Cell, GridScreen, GridView};

use super::Sly3Session;

/// The grids' title string ("Gadget Grid").
pub const TITLE: i16 = 5337;
/// The refusal messages: not owned, not assignable.
pub const NOT_OWNED: i16 = 65;
pub const NOT_ASSIGNABLE: i16 = 66;
/// The cell table's offset from the screen position (spec writer's note).
pub const TABLE_SHIFT: [f32; 2] = [-38.0, 14.0];

/// The grids and the open screen.
#[derive(Default)]
pub struct GridState {
    /// (screen object, character, cells).
    pub grids: Vec<(i32, Member, Vec<GridCell>)>,
    pub items: Option<Sly3ItemTable>,
    pub open: Option<(Assignment, GridScreen)>,
    /// Each grid screen's art (icons in the sheet's third font, the title's place), once the fonts are read.
    pub art: Vec<(i32, crate::sly2::layer::GridArt)>,
}

/// The grid screens' external files in world `fw` (screens 1062 to 1064 keep their element data in a file).
pub fn screen_files(fw: &formats_sly3::World<'_>) -> Vec<(i32, formats_sly2::record::FileRecord)> {
    use formats_sly3::stream::{Body, Record, visit};
    let mut out: Vec<(i32, formats_sly2::record::FileRecord)> = Vec::new();
    for b in &fw.bodies {
        visit(&b.objects, &mut |r: &Record| {
            if engine::gang::sly3::GRIDS.iter().any(|g| g.0 == r.id)
                && !out.iter().any(|o| o.0 == r.id)
                && let Body::Screen(s) = &r.body
                && let Some(f) = s.external
            {
                out.push((r.id, f));
            }
        });
    }
    out
}

/// Bakes the grids' icons from their screens, as Sly 2's (`crate::sly2::layer::grid_art`): the glyphs of the icon
/// font and each screen's art. The icon codes of grid k start at k × 256.
pub fn bake(
    grids: &[(i32, Member, Vec<GridCell>)],
    screens: &[(i32, formats_sly2::screen::Screen)],
) -> (Vec<ui::sly2::font::GlyphShape>, Vec<(i32, crate::sly2::layer::GridArt)>) {
    let mut glyphs = Vec::new();
    let mut art = Vec::new();
    for (k, (id, _, cells)) in grids.iter().enumerate() {
        let Some((_, screen)) = screens.iter().find(|s| s.0 == *id) else { continue };
        let (g, a) = crate::sly2::layer::grid_art(screen, k, cells, i32::from(TITLE));
        glyphs.extend(g);
        art.push((*id, a));
    }
    (glyphs, art)
}

/// The progress group of a character's slots.
fn group(who: Who) -> &'static str {
    match who {
        Who::Sly => "sly",
        Who::Bentley => "bentley",
        Who::Murray => "murray",
    }
}

fn member(who: Who) -> Member {
    match who {
        Who::Sly => Member::Sly,
        Who::Bentley => Member::Bentley,
        Who::Murray => Member::Murray,
    }
}

impl<B: Backend, R: ReadAt> Sly3Session<B, R> {
    /// The executable's grids and item table (once, when the session is made).
    pub(super) fn grid_state(exe: Option<&formats_sly3::exe::Executable>) -> GridState {
        let items = exe.and_then(|e| Sly3ItemTable::from_executable(e).ok());
        let grids = match (exe, items.as_ref()) {
            (Some(e), Some(t)) => engine::gang::sly3::grids_from_executable(e, t).unwrap_or_default(),
            _ => Vec::new(),
        };
        GridState { grids, items, open: None, art: Vec::new() }
    }

    /// Whether the gadget grid is open.
    pub fn grid_open(&self) -> bool {
        self.ui.grid.open.is_some()
    }

    /// Gives the crew the progress block's owned items and slots.
    pub(super) fn sync_crew(&mut self) {
        let Some(p) = self.progress().cloned() else { return };
        let n = self.ui.grid.items.as_ref().map_or(0, |t| t.items.len());
        let Some(gang) = self.game.as_mut().and_then(|g| g.systems.player.gang_mut()) else { return };
        let mut owned = engine::gang::Owned::default();
        for i in 0..n {
            if save::sly3::owned(&p, i) {
                owned.add(i as u8);
            }
        }
        gang.shared.owned = owned;
        let items = gang.shared.items.clone();
        for c in &mut gang.members {
            for (k, slot) in c.slots.0.iter_mut().enumerate() {
                let v = p.get_named(&[group(c.who), "apukCur"], Some(k)).map_or(-1, SaveValue::as_i32);
                // Only an owned item of this character that goes on a button (a zeroed block reads item 0).
                *slot = u8::try_from(v).ok().filter(|&i| {
                    owned.has(i)
                        && items.get(i).is_some_and(|r| {
                            r.owner() == Some(c.who)
                                && matches!(
                                    r.button_use(),
                                    engine::gang::ButtonUse::Press | engine::gang::ButtonUse::Hold
                                )
                        })
                });
            }
        }
    }

    /// Marks item `item` owned or not in the progress block (as a purchase or a reward does) and tells the crew.
    pub fn set_owned(&mut self, item: usize, on: bool) {
        if let Some(p) = self.progress_mut() {
            p.set_named(&["powerUps", "mppukfAcquired"], Some(item), SaveValue::Flag(on));
        }
        self.sync_crew();
    }

    /// The slots of the character being played (item records).
    pub fn player_slots(&self) -> Option<[Option<u8>; 3]> {
        let gang = self.game.as_ref()?.systems.player.gang()?;
        gang.get(gang.player).map(|c| c.slots.0)
    }

    /// Writes `who`'s slots to the progress block.
    fn store_slots(&mut self, who: Who, slots: [Option<u8>; 3]) {
        let Some(p) = self.progress_mut() else { return };
        for (k, s) in slots.iter().enumerate() {
            let v = s.map_or(-1, i32::from);
            p.set_named(&[group(who), "apukCur"], Some(k), SaveValue::Int(v));
        }
    }

    fn open_grid(&mut self) {
        self.sync_crew();
        let Some(gang) = self.game.as_mut().and_then(|g| g.systems.player.gang_mut()) else { return };
        let who = gang.player;
        let Some((screen, _, cells)) = self.ui.grid.grids.iter().find(|g| g.1 == member(who)) else { return };
        let a = Assignment::open(who, cells.clone(), &gang.shared.owned);
        self.ui.grid.open = Some((a, GridScreen::open(*screen)));
    }

    /// The grid's frame; returns whether it is open (it takes the input).
    pub(super) fn update_grid(&mut self, input: &FrameInput, circle: bool, others_open: bool, dt: f32) -> bool {
        let pressed = |b: Buttons| input.pressed & b.0 != 0;
        if self.ui.grid.open.is_none() {
            if !others_open && pressed(Buttons::MINUS) && self.game.is_some() {
                self.open_grid();
            }
            return self.ui.grid.open.is_some();
        }
        let Some((mut a, mut screen)) = self.ui.grid.open.take() else { return false };
        screen.update(dt);
        if circle || pressed(Buttons::MINUS) {
            screen.screen.hide();
        }
        let mut message = None;
        let mut changed = None;
        if matches!(screen.screen.phase, ui::sly2::Phase::Idle(_))
            && let Some(gang) = self.game.as_mut().and_then(|g| g.systems.player.gang_mut())
        {
            let mut dir = [0.0f32, 0.0];
            for (b, d) in [
                (Buttons::LEFT, [-1.0, 0.0]),
                (Buttons::RIGHT, [1.0, 0.0]),
                (Buttons::UP, [0.0, -1.0]),
                (Buttons::DOWN, [0.0, 1.0]),
            ] {
                if pressed(b) {
                    dir = d;
                }
            }
            a.step(dir, &gang.shared.owned);
            for (b, slot) in [(Buttons::L, 0usize), (Buttons::ZL, 1), (Buttons::R, 2)] {
                if pressed(b) {
                    let (items, owned) = (gang.shared.items.clone(), gang.shared.owned);
                    if let Some(c) = gang.get_mut(a.who) {
                        match a.assign(slot, &mut c.slots, &items, &owned) {
                            AssignMessage::Assigned => changed = Some(c.slots.0),
                            AssignMessage::NotOwned => message = Some((NOT_OWNED, "Not owned")),
                            AssignMessage::NotAssignable => message = Some((NOT_ASSIGNABLE, "No button needed")),
                        }
                    }
                }
            }
        }
        if let Some(slots) = changed {
            self.store_slots(a.who, slots);
        }
        if let Some((id, fallback)) = message {
            screen.say(self.ui.strings.get(&id).cloned().unwrap_or_else(|| String::from(fallback)));
        }
        if screen.screen.visible() {
            self.ui.grid.open = Some((a, screen));
        }
        true
    }

    /// The open grid's view.
    pub fn grid_view(&self) -> Option<GridView> {
        let (a, _) = self.ui.grid.open.as_ref()?;
        let t = self.ui.grid.items.as_ref();
        let name = |item: i32| -> String {
            let id = t.and_then(|t| usize::try_from(item).ok().and_then(|n| t.items.get(n))).map_or(-1, |r| r.name);
            let s = i16::try_from(id).ok().and_then(|id| self.ui.strings.get(&id)).cloned().unwrap_or_default();
            super::layer::bytes(&s).into_iter().map(char::from).collect()
        };
        let gang = self.game.as_ref().and_then(|g| g.systems.player.gang())?;
        let slots = gang.get(a.who).map_or([None; 3], |c| c.slots.0);
        let screen = self.ui.grid.grids.iter().find(|g| g.1 == member(a.who)).map(|g| g.0);
        let art = screen.and_then(|s| self.ui.grid.art.iter().find(|x| x.0 == s)).map(|x| &x.1);
        let cells = a
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| Cell {
                position: [c.position[0] - TABLE_SHIFT[0], c.position[1] - TABLE_SHIFT[1]],
                owned: gang.shared.owned.has(c.item as u8),
                assignable: c.assignable(),
                name: name(c.item),
                slot: slots.iter().position(|s| *s == Some(c.item as u8)),
                icon: art.and_then(|g| g.icons.get(i).copied().flatten()),
            })
            .collect();
        let title = self.ui.strings.get(&TITLE).cloned().unwrap_or_else(|| String::from("Gadget Grid"));
        Some(GridView {
            title: super::layer::bytes(&title).into_iter().map(char::from).collect(),
            title_at: art.and_then(|g| g.title),
            cells,
            selected: a.selected,
            slots: slots.map(|s| s.map(|i| name(i32::from(i)))),
        })
    }
}
