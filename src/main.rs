//! Forever Memory: a local armory and memory explorer for WoW Forever,
//! reading the forever-memory archive, the native logs and the game's art.

mod art;
mod data;
mod theme;
mod ui;

use data::{Model, Paths};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Forever Memory")
            .with_app_id("forever-memory")
            .with_inner_size([1480.0, 940.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("Forever Memory", options, Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Armory,
    Journal,
    Quests,
    Map,
    Combat,
    Economy,
    Players,
}

impl Page {
    const ALL: [Page; 8] = [Page::Overview, Page::Armory, Page::Journal, Page::Quests, Page::Map, Page::Combat, Page::Economy, Page::Players];
    fn label(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Armory => "Armory",
            Page::Journal => "Journal",
            Page::Quests => "Quests",
            Page::Map => "Map",
            Page::Combat => "Combat",
            Page::Economy => "Gold & loot",
            Page::Players => "Players",
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Page::Overview => "🏠",
            Page::Armory => "⛨",
            Page::Journal => "📖",
            Page::Quests => "❗",
            Page::Map => "⌖",
            Page::Combat => "⚔",
            Page::Economy => "⛃",
            Page::Players => "☺",
        }
    }
}

/// UI state that survives page switches.
#[derive(Default)]
pub struct State {
    pub character: usize,
    pub quest_tab: usize,
    pub quest_search: String,
    pub quest: Option<i64>,
    pub player_search: String,
    pub player: Option<String>,
    pub session: Option<usize>,
    pub journal_hide: std::collections::HashSet<&'static str>,
    pub map_zone: Option<i64>,
    pub map_session: Option<usize>,
    pub map_time: f64,
    pub map_playing: bool,
    pub map_whole: bool,
    pub fight: Option<usize>,
    pub loot_search: String,
    pub spec: usize,
    pub search: String,
}

pub struct App {
    paths: Paths,
    model: Option<Arc<Model>>,
    loading: Option<Receiver<Model>>,
    last_check: Instant,
    last_load: Instant,
    art: art::Art,
    page: Page,
    state: State,
    /// FM_SHOT=<file.png> [FM_PAGE=quests]: save a screenshot after loading and quit.
    shot: Option<(std::path::PathBuf, Instant, bool)>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let paths = Paths::from_env();
        let art = art::Art::new(ctx, paths.art.clone(), paths.wowdata.clone());
        let mut app = App {
            paths,
            model: None,
            loading: None,
            last_check: Instant::now(),
            last_load: Instant::now(),
            art,
            page: Page::Overview,
            state: State::default(),
            shot: std::env::var("FM_SHOT").ok().map(|p| (p.into(), Instant::now(), false)),
        };
        if let Ok(p) = std::env::var("FM_PAGE") {
            app.page = Page::ALL.into_iter().find(|x| x.label().to_lowercase().starts_with(&p.to_lowercase())).unwrap_or(Page::Overview);
        }
        app.reload(ctx);
        app
    }

    fn reload(&mut self, ctx: &egui::Context) {
        if self.loading.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let paths = self.paths.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            tx.send(data::load(&paths)).ok();
            ctx.request_repaint();
        });
        self.loading = Some(rx);
        self.last_load = Instant::now();
    }

    /// Reload when the archive commits or the live logs grow, at most every
    /// 30 seconds, since the game appends to its logs constantly.
    fn watch(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.loading {
            if let Ok(m) = rx.try_recv() {
                self.model = Some(Arc::new(m));
                self.loading = None;
            }
        }
        if self.last_check.elapsed() > Duration::from_secs(5) {
            self.last_check = Instant::now();
            if let Some(m) = &self.model {
                if data::stamp(&self.paths) != m.stamp && self.last_load.elapsed() > Duration::from_secs(30) {
                    self.reload(ctx);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_secs(5));
    }
}

impl App {
    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, started, asked)) = &mut self.shot else { return };
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let img = image::RgbaImage::from_raw(image.width() as u32, image.height() as u32, image.as_raw().to_vec());
                if let Some(img) = img {
                    img.save(&*path).ok();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let wait = std::env::var("FM_SHOT_WAIT").ok().and_then(|s| s.parse().ok()).unwrap_or(6.0);
        if !*asked && self.model.is_some() && started.elapsed().as_secs_f64() > wait {
            *asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.watch(&ctx);
        self.screenshot(&ctx);
        let Some(model) = self.model.clone() else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Reading your memories…").font(theme::display_font(28.0)).color(theme::MUTED));
                });
            });
            return;
        };
        if self.state.character >= model.memory.characters.len() {
            self.state.character = 0;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.reload(&ctx);
        }

        egui::Panel::top("top")
            .frame(egui::Frame::new().fill(theme::NIGHT).inner_margin(egui::Margin::symmetric(18, 10)))
            .show(ui, |ui| {
                ui::top_bar(ui, &model, &mut self.state, &mut self.page, &mut self.art, self.loading.is_some());
            });

        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(196.0)
            .frame(egui::Frame::new().fill(theme::NIGHT).inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 12 }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for p in Page::ALL {
                    let selected = self.page == p;
                    // The active page is marked by background and colour only.
                    let text = egui::RichText::new(format!("{}   {}", p.glyph(), p.label()))
                        .size(16.5)
                        .color(if selected { theme::GOLD } else { theme::INK });
                    let button = egui::Button::new(text)
                        .fill(if selected { theme::RAISED } else { egui::Color32::TRANSPARENT })
                        .min_size(egui::vec2(ui.available_width(), 36.0));
                    if ui.add(button).clicked() {
                        self.page = p;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    let m = &model;
                    ui.label(
                        egui::RichText::new(format!(
                            "{} events\n{} combat log lines\n{} chat lines\n{} players met",
                            theme::thousands(m.memory.characters.iter().map(|c| c.events.len() as i64).sum()),
                            theme::thousands(m.combat.lines as i64),
                            theme::thousands(m.chat.len() as i64),
                            m.players.len()
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::NIGHT).inner_margin(egui::Margin { left: 8, right: 24, top: 8, bottom: 0 }))
            .show(ui, |ui| {
                if model.memory.characters.is_empty() {
                    ui::empty(ui, "No characters yet. Log in with the armory addon and let armory-sync archive a save.");
                    return;
                }
                let st = &mut self.state;
                let art = &mut self.art;
                match self.page {
                    Page::Overview => ui::overview::show(ui, &model, st, art, &mut self.page),
                    Page::Armory => ui::armory::show(ui, &model, st, art),
                    Page::Journal => ui::journal::show(ui, &model, st, art),
                    Page::Quests => ui::quests::show(ui, &model, st, art),
                    Page::Map => ui::map::show(ui, &model, st, art),
                    Page::Combat => ui::combat::show(ui, &model, st, art),
                    Page::Economy => ui::economy::show(ui, &model, st, art),
                    Page::Players => ui::players::show(ui, &model, st, art),
                }
            });
    }
}
