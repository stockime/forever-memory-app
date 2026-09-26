//! Forever Memory: a local armory and memory explorer for WoW Forever,
//! reading the forever-memory archive, the native logs and the game's art.

mod art;
mod claude;
mod data;
mod theme;
mod ui;

use data::{Model, Paths};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// `forever-memory diary <character> [YYYY-MM-DD]` writes one diary entry
/// without opening the window (the day defaults to the latest one played).
fn diary_cli(args: &[String]) -> ! {
    let paths = Paths::from_env();
    let model = data::load(&paths);
    let who = args.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let Some(c) = model
        .memory
        .characters
        .iter()
        .find(|c| c.slug == who || c.name.to_lowercase() == who)
    else {
        eprintln!(
            "no character {who:?}; have: {}",
            model
                .memory
                .characters
                .iter()
                .map(|c| c.slug.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    };
    let day = args
        .get(1)
        .cloned()
        .or_else(|| data::diary::days(c).first().map(|(d, _)| d.clone()))
        .unwrap_or_default();
    let facts = data::diary::facts(&model, c, &day);
    if facts.is_empty() {
        eprintln!("nothing recorded on {day}");
        std::process::exit(2);
    }
    let previous = data::diary::previous(&paths.repo, c, &day);
    let prompt = data::diary::prompt(
        c,
        &day,
        &facts,
        previous.as_ref().map(|(d, t)| (d.as_str(), t.as_str())),
    );
    match claude::write(data::diary::SYSTEM, &prompt).and_then(|entry| {
        data::diary::store(&paths.repo, c, &day, &entry, &facts)?;
        Ok(entry)
    }) {
        Ok(entry) => {
            println!("{entry}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("diary") {
        diary_cli(&args[1..]);
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Forever Memory")
            .with_app_id("forever-memory")
            .with_inner_size([1480.0, 940.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Forever Memory",
        options,
        Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Armory,
    Journal,
    Diary,
    Quests,
    Map,
    Combat,
    Economy,
    Players,
}

impl Page {
    const ALL: [Page; 9] = [
        Page::Overview,
        Page::Armory,
        Page::Journal,
        Page::Diary,
        Page::Quests,
        Page::Map,
        Page::Combat,
        Page::Economy,
        Page::Players,
    ];
    fn label(self) -> &'static str {
        match self {
            Page::Overview => "Overview",
            Page::Armory => "Armory",
            Page::Journal => "Journal",
            Page::Diary => "Diary",
            Page::Quests => "Quests",
            Page::Map => "Map",
            Page::Combat => "Combat",
            Page::Economy => "Gold & loot",
            Page::Players => "Players",
        }
    }
    /// Game icons (file IDs) for the navigation.
    fn icon(self) -> i64 {
        use ui::widgets::icons;
        match self {
            Page::Overview => icons::SPIRIT,
            Page::Armory => icons::CHEST,
            Page::Journal => icons::BOOK,
            Page::Diary => icons::SCROLL,
            Page::Quests => icons::NOTE,
            Page::Map => icons::MAP,
            Page::Combat => icons::SWORDS,
            Page::Economy => icons::COIN,
            Page::Players => icons::GROUP,
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
    pub session_sort: usize,
    pub repo: std::path::PathBuf,
    pub reload_now: bool,
    pub note_edit: Option<(String, String)>,
    pub diary_day: Option<String>,
    pub diary_job: Option<ui::diary::Job>,
    pub diary_queue: Vec<String>,
    pub diary_error: Option<String>,
    pub diary_status: Option<String>,
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
        let repo = paths.repo.clone();
        let mut app = App {
            paths,
            model: None,
            loading: None,
            last_check: Instant::now(),
            last_load: Instant::now(),
            art,
            page: Page::Overview,
            state: State {
                repo,
                ..Default::default()
            },
            shot: std::env::var("FM_SHOT")
                .ok()
                .map(|p| (p.into(), Instant::now(), false)),
        };
        if let Ok(p) = std::env::var("FM_PAGE") {
            app.page = Page::ALL
                .into_iter()
                .find(|x| x.label().to_lowercase().starts_with(&p.to_lowercase()))
                .unwrap_or(Page::Overview);
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
                if data::stamp(&self.paths) != m.stamp
                    && self.last_load.elapsed() > Duration::from_secs(30)
                {
                    self.reload(ctx);
                }
            }
        }
        ctx.request_repaint_after(Duration::from_secs(5));
    }
}

impl App {
    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some((path, started, asked)) = &mut self.shot else {
            return;
        };
        for e in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = e {
                let img = image::RgbaImage::from_raw(
                    image.width() as u32,
                    image.height() as u32,
                    image.as_raw().to_vec(),
                );
                if let Some(img) = img {
                    img.save(&*path).ok();
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        let wait = std::env::var("FM_SHOT_WAIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(6.0);
        if !*asked && self.model.is_some() && started.elapsed().as_secs_f64() > wait {
            *asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if std::env::var("FM_HOVER").is_err() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

impl eframe::App for App {
    /// FM_HOVER=x,y places the pointer there (with FM_SHOT, to check tooltips).
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Ok(v) = std::env::var("FM_HOVER") {
            if let Some((x, y)) = v
                .split_once(',')
                .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
            {
                if self
                    .shot
                    .as_ref()
                    .is_some_and(|(_, t, _)| t.elapsed().as_secs_f64() < 1.5)
                {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x, y)));
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.watch(&ctx);
        self.screenshot(&ctx);
        let Some(model) = self.model.clone() else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("Reading your memories…")
                            .font(theme::display_font(28.0))
                            .color(theme::MUTED),
                    );
                });
            });
            return;
        };
        if self.state.character >= model.memory.characters.len() {
            self.state.character = 0;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) || std::mem::take(&mut self.state.reload_now)
        {
            self.reload(&ctx);
        }

        egui::Panel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(theme::NIGHT)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ui, |ui| {
                ui::top_bar(
                    ui,
                    &model,
                    &mut self.state,
                    &mut self.page,
                    &mut self.art,
                    self.loading.is_some(),
                );
            });

        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(196.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::NIGHT)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 12,
                        top: 8,
                        bottom: 12,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let class = model
                    .memory
                    .characters
                    .get(self.state.character)
                    .map(|c| c.class_file.to_lowercase())
                    .unwrap_or_default();
                for p in Page::ALL {
                    let selected = self.page == p;
                    // The active page is marked by background and colour only.
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 38.0),
                        egui::Sense::click(),
                    );
                    let painter = ui.painter();
                    if selected || resp.hovered() {
                        painter.rect_filled(
                            rect,
                            7.0,
                            if selected {
                                theme::RAISED
                            } else {
                                egui::Color32::from_rgb(0x12, 0x18, 0x33)
                            },
                        );
                    }
                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.left() + 20.0, rect.center().y),
                        egui::vec2(24.0, 24.0),
                    );
                    let tex = if p == Page::Overview {
                        self.art.get(&ctx, &format!("class-{class}.png"))
                    } else {
                        self.art.icon(&ctx, Some(p.icon()))
                    };
                    if let Some(t) = tex {
                        let tint = if selected {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_gray(185)
                        };
                        painter.image(
                            t.id(),
                            icon_rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.07, 0.07),
                                egui::pos2(0.93, 0.93),
                            ),
                            tint,
                        );
                        painter.rect_stroke(
                            icon_rect,
                            4.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(0x6b, 0x5a, 0x2e)),
                            egui::StrokeKind::Outside,
                        );
                    }
                    painter.text(
                        egui::pos2(rect.left() + 44.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        p.label(),
                        egui::FontId::proportional(16.5),
                        if selected { theme::GOLD } else { theme::INK },
                    );
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.page = p;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    let m = &model;
                    ui.label(
                        egui::RichText::new(format!(
                            "{} events\n{} combat log lines\n{} chat lines\n{} players met",
                            theme::thousands(
                                m.memory
                                    .characters
                                    .iter()
                                    .map(|c| c.events.len() as i64)
                                    .sum()
                            ),
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
                    Page::Diary => ui::diary::show(ui, &model, st, art),
                    Page::Quests => ui::quests::show(ui, &model, st, art),
                    Page::Map => ui::map::show(ui, &model, st, art),
                    Page::Combat => ui::combat::show(ui, &model, st, art),
                    Page::Economy => ui::economy::show(ui, &model, st, art),
                    Page::Players => ui::players::show(ui, &model, st, art),
                }
            });
    }
}
