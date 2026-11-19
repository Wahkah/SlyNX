//! Harness app that draws the HUD, the binocucom or a menu with the disc's own fonts and texts over a flat
//! background, rasterized on the CPU, so captures can be looked at and compared.
//!
//! ```sh
//! SLYNX_DISC_SLY1=<disc.iso> cargo run --release -p ui --example ui_capture -- run --app ui --frames 60 \
//!     --capture 59 --out <folder outside the repository> --opt view=hud
//! ```
//!
//! Views: `hud`, `binoc`, `title`, `pause`, `options`, `slots`, `hideout`, `map`, `sheet` (the glyph sheet itself).
//! Captures are game-derived: keep them out of the repository.

use formats::disc::Disc;
use formats::world::World;
use harness::app::{App, AppConfig, AppError, FrameContext};
use harness::image::Image;
use ui::binoc::{Binocucom, Target};
use ui::hud::Hud;
use ui::menu::{Episode, MapLevel, Menus, PauseContext, Screen};
use ui::raster::rasterize;
use ui::tables::{SlotInfo, TextTables, read_placements};
use ui::{Canvas, DrawList, FontSet, HudValues, MenuInput};

struct UiApp {
    view: String,
    fonts: FontSet,
    tables: TextTables,
    hud: Hud,
    binoc: Binocucom,
    menus: Menus,
    width: u32,
    height: u32,
    image: Image,
}

fn load(cfg: &AppConfig) -> Result<UiApp, String> {
    let path = std::env::var("SLYNX_DISC_SLY1").map_err(|_| "SLYNX_DISC_SLY1 is not set".to_string())?;
    let mut disc = Disc::open_path(path).map_err(|e| e.to_string())?;
    let fonts_of = |disc: &mut Disc<std::fs::File>, name: &str| -> Result<FontSet, String> {
        let i = disc.find_world(name).ok_or(format!("no world {name}"))?;
        let bytes = disc.read_world(i).map_err(|e| e.to_string())?;
        let world = World::load(&bytes).map_err(|e| e.to_string())?;
        FontSet::from_world(&world).map_err(|e| e.to_string())
    };
    let fonts = fonts_of(&mut disc, "Paris, France")?.merged(&fonts_of(&mut disc, "The Hideout")?);
    let tables = TextTables::read(&disc.executable).map_err(|e| e.to_string())?;
    let placements = read_placements(&disc.executable).map_err(|e| e.to_string())?;
    let view = cfg.option("view").unwrap_or("hud").to_string();
    let hud =
        Hud::new(placements, HudValues { lives: 3, coins: Some(37), keys: Some(2), bottles: (12, 30), percent: None });
    let mut menus = Menus::default();
    menus.slots[0] = Some(SlotInfo { percent: 42, world: tables.levels[1].clone(), minutes: 135 });
    let paris = tables.levels.get(1).cloned().unwrap_or_default();
    match view.as_str() {
        "title" => menus.open_title(),
        "pause" => menus.open_pause(PauseContext {
            world_name: paris,
            has_map: true,
            slot_in_use: true,
            ..PauseContext::default()
        }),
        "options" => {
            menus.open_pause(PauseContext::default());
            menus.update(&MenuInput { dpad: [false, true, false, false], ..MenuInput::default() }, 0.016, &tables);
            menus.update(&MenuInput { accept: true, ..MenuInput::default() }, 0.016, &tables);
        }
        "slots" => {
            menus.open_title();
            menus.update(&MenuInput { select: true, ..MenuInput::default() }, 0.016, &tables);
            menus.update(&MenuInput { dpad: [false, true, false, false], ..MenuInput::default() }, 0.016, &tables);
            menus.update(&MenuInput { accept: true, ..MenuInput::default() }, 0.016, &tables);
            assert_eq!(menus.screen(), Some(Screen::Slots(ui::menu::SlotMode::Load)));
        }
        "hideout" => {
            let mut eps =
                vec![Episode { name: b"The Hideout".to_vec(), unlocked: true, visited: true, ..Episode::default() }];
            for (i, n) in tables.episodes.iter().enumerate() {
                eps.push(Episode {
                    name: n.clone(),
                    unlocked: i < 2,
                    visited: i < 1,
                    keys: (3, 7),
                    vaults: (1, 1),
                    sprints: (0, 1),
                });
            }
            menus.open_hideout(eps, 1);
        }
        "map" => {
            let names = &tables.levels;
            let levels = (0..7)
                .map(|i| MapLevel {
                    name: names.get(3 + i).cloned().unwrap_or_default(),
                    at: (120.0 + (i % 4) as f32 * 120.0, 140.0 + (i / 4) as f32 * 140.0),
                    unlocked: i < 5,
                    done: [i < 3, i < 1, false],
                })
                .collect();
            menus.open_map(levels, 2);
        }
        _ => {}
    }
    let image = Image::new(cfg.width, cfg.height, [0, 0, 0, 255]);
    Ok(UiApp {
        view,
        fonts,
        tables,
        hud,
        binoc: Binocucom::default(),
        menus,
        width: cfg.width,
        height: cfg.height,
        image,
    })
}

impl App for UiApp {
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let dt = ctx.dt as f32;
        match self.view.as_str() {
            "hud" => {
                if ctx.frame == 0 {
                    self.hud.flash_counters();
                    self.hud.set_timer(Some(83.4));
                    self.hud.set_saving(true);
                }
                if ctx.frame == 10 {
                    // The auto-save icon shares the timer's corner: show it briefly.
                    self.hud.set_saving(false);
                }
                if ctx.frame == 20 {
                    self.hud.set_values(HudValues {
                        lives: 3,
                        coins: Some(52),
                        keys: Some(2),
                        bottles: (13, 30),
                        percent: None,
                    });
                }
                self.hud.set_timer(Some(83.4 - ctx.time() as f32));
                self.hud.update(dt, true);
            }
            "binoc" => {
                if ctx.frame == 0 {
                    self.binoc.open();
                    self.binoc.zoom = 2.5;
                    self.binoc.heading = 30.0;
                    self.binoc.filter = Some(1);
                    self.binoc.set_radio(&self.tables.prompts[24].title.clone());
                }
                let targets = [
                    Target { kind: 1, position: [300.0, 0.0, 0.0], scan_point: false },
                    Target { kind: 2, position: [-200.0, 50.0, 0.0], scan_point: false },
                    Target { kind: 0, position: [100.0, -80.0, 0.0], scan_point: true },
                ];
                self.binoc.update(dt, [0.0; 3], &targets, |p| Some((320.0 + p[0] * 0.5, 180.0 + p[1] * 0.5)));
            }
            _ => {
                self.menus.update(&MenuInput::default(), dt, &self.tables);
            }
        }
        Ok(())
    }

    fn render(&mut self, _ctx: &mut FrameContext<'_>) -> Result<(), AppError> {
        let mut list = DrawList::new(Canvas::for_display(self.width, self.height));
        match self.view.as_str() {
            "hud" => self.hud.draw(&self.fonts, &self.tables, &mut list),
            "binoc" => self.binoc.draw(&self.fonts, &mut list),
            "sheet" => {
                let c = list.canvas;
                let side = c.height.min(c.width);
                list.textured(ui::Rect::new(0.0, 0.0, side, side), ui::Texture::Glyphs, [0.0, 0.0, 1.0, 1.0], [1.0; 4]);
            }
            _ => self.menus.draw(&self.fonts, &self.tables, &mut list),
        }
        let mut img = Image::new(self.width, self.height, [70, 96, 120, 255]);
        rasterize(&list, &self.fonts.sheet, self.width, self.height, img.pixels_mut());
        self.image = img;
        Ok(())
    }

    fn capture(&mut self) -> Result<Image, AppError> {
        Ok(self.image.clone())
    }
}

fn main() {
    let mut apps = harness::apps::AppRegistry::builtin();
    apps.register("ui", "HUD, binocucom and menus from SLYNX_DISC_SLY1; options: view=NAME", |cfg| {
        Ok(Box::new(load(cfg).map_err(AppError)?))
    });
    std::process::exit(harness::cli::main_with(&apps));
}
