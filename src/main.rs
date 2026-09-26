//! Forever Memory: a local armory and memory explorer for World of Warcraft:
//! Forever, reading what the armory addon records, the game's native logs
//! and its art.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod addon;
mod art;
mod claude;
mod config;
mod data;
mod gamedata;
mod i18n;
mod platform;
mod s3;
mod savedvars;
mod sync;
mod theme;
mod ui;
mod voice;
mod writer;

use data::{Model, Paths};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// `forever-memory diary <character> [YYYY-MM-DD]` writes one diary entry
/// without opening the window (the day defaults to the latest one played).
fn diary_cli(args: &[String]) -> ! {
    let settings = config::get();
    i18n::set(settings.lang());
    let paths = Paths::from_settings(&settings);
    let model = data::load(&paths, None);
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
    // `forever-memory record [armory.lua]`: archives the addon's saves once and exits.
    if args.first().map(String::as_str) == Some("record") {
        let settings = config::get();
        let files: Vec<std::path::PathBuf> = match args.get(1) {
            Some(f) => vec![f.into()],
            None => settings
                .install()
                .and_then(|i| Some(platform::saved_variables(&i, &settings.flavor_in(&i)?)))
                .unwrap_or_default(),
        };
        if files.is_empty() {
            eprintln!("no armory SavedVariables found");
            std::process::exit(2);
        }
        for f in files {
            match sync::record(&settings, &f) {
                Ok(msg) => println!("{}: {}", f.display(), msg.as_deref().unwrap_or("no change")),
                Err(e) => {
                    eprintln!("{}: {e}", f.display());
                    std::process::exit(1);
                }
            }
        }
        std::process::exit(0);
    }
    // `forever-memory check-s3`: checks the backup settings without writing.
    if args.first().map(String::as_str) == Some("check-s3") {
        match s3::check(&config::get().s3) {
            Ok(()) => {
                println!("ok");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    // `forever-memory sync`: the recorder without the window, e.g. as a
    // service that runs all the time. Picks up settings changes on its own.
    if args.first().map(String::as_str) == Some("sync") {
        let shared = config::shared();
        let status = sync::SharedStatus::default();
        sync::spawn(shared.clone(), status.clone(), || {});
        let file = config::file();
        let modified = || std::fs::metadata(&file).and_then(|m| m.modified()).ok();
        let mut seen = modified();
        let mut shown: (Option<std::time::SystemTime>, Option<String>, bool) = (None, None, false);
        loop {
            std::thread::sleep(Duration::from_secs(2));
            if modified() != seen {
                seen = modified();
                if let Ok(mut s) = shared.lock() {
                    *s = config::Settings::load();
                }
                println!("settings reloaded");
            }
            let st = status.lock().map(|s| s.clone()).unwrap_or_default();
            if let Some((t, msg)) = &st.last
                && shown.0 != Some(*t) {
                    println!("{msg}");
                }
            if st.error.is_some() && st.error != shown.1 {
                eprintln!("{}", st.error.clone().unwrap_or_default());
            }
            if st.elsewhere && !shown.2 {
                println!("another recorder has the archive; waiting");
            }
            shown = (st.last.map(|l| l.0), st.error, st.elsewhere);
        }
    }
    // `forever-memory play <file.mp3>`: plays a file through the narration player (a check).
    if args.first().map(String::as_str) == Some("play") {
        let path = std::path::PathBuf::from(args.get(1).cloned().unwrap_or_default());
        let speed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        match voice::Player::play(&path, "cli".into(), speed) {
            Ok(p) => {
                println!("length ~{:.1}s", p.length);
                while !p.finished() {
                    std::thread::sleep(Duration::from_millis(250));
                }
                println!("played to {:.1}s", p.position());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
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
    Settings,
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
            Page::Overview => tr!("Overview"),
            Page::Armory => tr!("Armory"),
            Page::Journal => tr!("Journal"),
            Page::Diary => tr!("Diary"),
            Page::Quests => tr!("Quests"),
            Page::Map => tr!("Map"),
            Page::Combat => tr!("Combat"),
            Page::Economy => tr!("Gold & loot"),
            Page::Players => tr!("Players"),
            Page::Settings => tr!("Settings"),
        }
    }
    /// A name that stays the same in every language (for FM_PAGE).
    fn id(self) -> String {
        match self {
            Page::Economy => "gold".into(),
            p => format!("{p:?}").to_lowercase(),
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
            Page::Settings => 134063, // a gear
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
    pub voice_job: Option<std::sync::mpsc::Receiver<ui::diary::VoiceMsg>>,
    pub voice_busy: String,
    pub voice_error: Option<String>,
    pub narrator: Option<voice::Player>,
    pub el_key_input: String,
    pub settings_page: ui::settings::Page,
    pub sync_status: sync::SharedStatus,
    /// The game or archive folder changed: find everything again.
    pub paths_changed: bool,
    /// A character to select once the archive is read.
    pub select_slug: Option<String>,
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
    progress: data::Shared,
    /// Set when the recorder wrote to the archive.
    recorded: Arc<std::sync::atomic::AtomicBool>,
    /// FM_SHOT=<file.png> [FM_PAGE=quests]: save a screenshot after loading and quit.
    shot: Option<(std::path::PathBuf, Instant, bool)>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let settings = config::get();
        i18n::set(settings.lang());
        let paths = Paths::from_settings(&settings);
        let art = art::Art::new(ctx, paths.art.clone());
        let repo = paths.repo.clone();
        let sync_status = sync::SharedStatus::default();
        let repaint = ctx.clone();
        let recorded = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = recorded.clone();
        sync::spawn(config::shared(), sync_status.clone(), move || {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            repaint.request_repaint();
        });
        let mut app = App {
            paths,
            model: None,
            loading: None,
            last_check: Instant::now(),
            last_load: Instant::now(),
            art,
            page: Page::Overview,
            progress: Default::default(),
            recorded,
            state: State {
                repo,
                sync_status,
                ..Default::default()
            },
            shot: std::env::var("FM_SHOT")
                .ok()
                .map(|p| (p.into(), Instant::now(), false)),
        };
        if !settings.onboarded {
            app.page = Page::Settings;
        }
        // FM_CHARACTER=<slug> picks the character (with FM_SHOT, for screenshots).
        if let Ok(slug) = std::env::var("FM_CHARACTER") {
            app.state.select_slug = Some(slug);
        }
        // FM_DIARY_DAY=YYYY-MM-DD opens the diary on that day.
        if let Ok(day) = std::env::var("FM_DIARY_DAY") {
            app.state.diary_day = Some(day);
        }
        if let Ok(p) = std::env::var("FM_PAGE") {
            app.page = Page::ALL
                .into_iter()
                .chain([Page::Settings])
                .find(|x| x.id().starts_with(&p.to_lowercase()))
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
        let progress = self.progress.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            tx.send(data::load(&paths, Some(&progress))).ok();
            ctx.request_repaint();
        });
        self.loading = Some(rx);
        self.last_load = Instant::now();
    }

    /// Reload when the archive commits or the live logs grow, at most every
    /// 30 seconds, since the game appends to its logs constantly.
    fn watch(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.loading
            && let Ok(m) = rx.try_recv() {
                self.model = Some(Arc::new(m));
                self.loading = None;
            }
        // A new save was recorded: show it now rather than on the next check.
        if self.loading.is_none() && self.recorded.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.reload(ctx);
        }
        if self.last_check.elapsed() > Duration::from_secs(5) {
            self.last_check = Instant::now();
            if let Some(m) = &self.model
                && data::stamp(&self.paths) != m.stamp
                    && self.last_load.elapsed() > Duration::from_secs(30)
                {
                    self.reload(ctx);
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
        let early = std::env::var("FM_SHOT_LOADING").is_ok(); // shoot the loading screen
        if !*asked && (self.model.is_some() || early) && started.elapsed().as_secs_f64() > wait {
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
        if let Ok(v) = std::env::var("FM_HOVER")
            && let Some((x, y)) = v
                .split_once(',')
                .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
                && self
                    .shot
                    .as_ref()
                    .is_some_and(|(_, t, _)| t.elapsed().as_secs_f64() < 1.5)
                {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x, y)));
                }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.watch(&ctx);
        self.screenshot(&ctx);
        let Some(model) = self.model.clone() else {
            loading_screen(ui, &self.progress, &mut self.art);
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        };
        if let Some(slug) = self.state.select_slug.take()
            && let Some(i) = model.memory.characters.iter().position(|c| c.slug == slug) {
                self.state.character = i;
            }
        if self.state.character >= model.memory.characters.len() {
            self.state.character = 0;
        }
        if model.memory.characters.is_empty() {
            self.page = Page::Settings; // the welcome
        }
        if std::mem::take(&mut self.state.paths_changed) {
            self.paths = Paths::from_settings(&config::get());
            self.state.repo = self.paths.repo.clone();
            self.art.retry();
            self.state.reload_now = true;
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
                for p in Page::ALL.into_iter().filter(|_| !model.memory.characters.is_empty()) {
                    if nav_item(ui, &mut self.art, p, self.page == p, &class) {
                        self.page = p;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    if nav_item(ui, &mut self.art, Page::Settings, self.page == Page::Settings, &class) {
                        self.page = Page::Settings;
                    }
                    ui.add_space(10.0);
                    let m = &model;
                    ui.label(
                        egui::RichText::new(tr!(
                            "{events} events\n{combat} combat log lines\n{chat} chat lines\n{players} players met",
                            events = theme::thousands(
                                m.memory
                                    .characters
                                    .iter()
                                    .map(|c| c.events.len() as i64)
                                    .sum()
                            ),
                            combat = theme::thousands(m.combat.lines as i64),
                            chat = theme::thousands(m.chat.len() as i64),
                            players = m.players.len()
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::NIGHT).inner_margin(egui::Margin { left: 8, right: 24, top: 8, bottom: 0 }))
            .show(ui, |ui| {
                let st = &mut self.state;
                if self.page == Page::Settings {
                    ui::settings::show(ui, st, model.memory.characters.len());
                    return;
                }
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
                    Page::Settings => {}
                }
            });
    }
}

/// One page in the navigation; returns whether it was clicked. The active
/// page is marked by background and colour only.
fn nav_item(ui: &mut egui::Ui, art: &mut art::Art, p: Page, selected: bool, class: &str) -> bool {
    let ctx = ui.ctx().clone();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::click());
    let painter = ui.painter();
    if selected || resp.hovered() {
        painter.rect_filled(
            rect,
            7.0,
            if selected { theme::RAISED } else { egui::Color32::from_rgb(0x12, 0x18, 0x33) },
        );
    }
    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), egui::vec2(24.0, 24.0));
    let tex = if p == Page::Overview && !class.is_empty() {
        art.get(&ctx, &format!("class-{class}.png"))
    } else {
        art.icon(&ctx, Some(p.icon()))
    };
    if let Some(t) = tex {
        let tint = if selected { egui::Color32::WHITE } else { egui::Color32::from_gray(185) };
        painter.image(t.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.07, 0.07), egui::pos2(0.93, 0.93)), tint);
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
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// The first read, drawn like the game's own loading screen: Forever's
/// continent art, a tip, and a framed bar that fills as the logs are read and
/// the art is painted. Nothing else shows until everything is ready.
fn loading_screen(ui: &mut egui::Ui, progress: &data::Shared, art: &mut art::Art) {
    use egui::{Align2, Color32, FontId, Rect, Stroke};
    let p = progress.lock().map(|g| g.clone()).unwrap_or_default();
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::from_rgb(4, 6, 14));
    let key = match p.continent {
        Some("kalimdor") => 7963779,
        _ => 7963776,
    };
    if let Some(tex) = art.icon(ui.ctx(), Some(key)) {
        // The art sits in a band from 16% to 84% of the texture's height.
        let band = Rect::from_min_max(egui::pos2(0.0, 0.158), egui::pos2(1.0, 0.842));
        let aspect = 2992.0 / 1152.0;
        let (w, h) = (rect.width(), rect.height());
        let uv = if w / h > aspect {
            let vh = band.height() * (h * aspect / w);
            Rect::from_min_max(
                egui::pos2(0.0, band.center().y - vh / 2.0),
                egui::pos2(1.0, band.center().y + vh / 2.0),
            )
        } else {
            let vw = w / (h * aspect);
            Rect::from_min_max(
                egui::pos2(0.5 - vw / 2.0, band.min.y),
                egui::pos2(0.5 + vw / 2.0, band.max.y),
            )
        };
        painter.image(tex.id(), rect, uv, Color32::from_gray(200));
    }
    // Darken towards the bottom where the bar sits.
    let mut mesh = egui::Mesh::default();
    let top = rect.top() + rect.height() * 0.55;
    mesh.colored_vertex(egui::pos2(rect.left(), top), Color32::from_black_alpha(0));
    mesh.colored_vertex(egui::pos2(rect.right(), top), Color32::from_black_alpha(0));
    mesh.colored_vertex(rect.right_bottom(), Color32::from_black_alpha(220));
    mesh.colored_vertex(rect.left_bottom(), Color32::from_black_alpha(220));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);

    let title = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.12);
    painter.text(
        title + egui::vec2(2.0, 3.0),
        Align2::CENTER_CENTER,
        "Forever Memory",
        theme::display_font(54.0),
        Color32::from_black_alpha(160),
    );
    painter.text(
        title,
        Align2::CENTER_CENTER,
        "Forever Memory",
        theme::display_font(54.0),
        theme::GOLD,
    );

    let bar_w = (rect.width() * 0.62).min(900.0);
    let bar = Rect::from_center_size(
        egui::pos2(rect.center().x, rect.bottom() - 90.0),
        egui::vec2(bar_w, 22.0),
    );
    if !p.tips.is_empty() {
        let secs = ui.input(|i| i.time) as usize / 6;
        let tip = &p.tips[secs % p.tips.len()];
        painter.text(
            bar.center_top() - egui::vec2(0.0, 40.0),
            Align2::CENTER_CENTER,
            tip,
            FontId::proportional(18.0),
            theme::INK,
        );
    }
    painter.rect_filled(bar.expand(3.0), 4.0, Color32::from_rgb(8, 8, 10));
    painter.rect_stroke(
        bar.expand(3.0),
        4.0,
        Stroke::new(2.0, Color32::from_rgb(0x8a, 0x6d, 0x2c)),
        egui::StrokeKind::Outside,
    );
    let mut fill = bar;
    fill.set_width(bar.width() * p.frac.clamp(0.0, 1.0));
    let mut m = egui::Mesh::default();
    let (a, b) = (
        Color32::from_rgb(0x9a, 0x6a, 0x08),
        Color32::from_rgb(0xf2, 0xc2, 0x3a),
    );
    m.colored_vertex(fill.left_top(), b);
    m.colored_vertex(fill.right_top(), b);
    m.colored_vertex(fill.right_bottom(), a);
    m.colored_vertex(fill.left_bottom(), a);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(m);
    painter.text(
        bar.center_bottom() + egui::vec2(0.0, 22.0),
        Align2::CENTER_CENTER,
        &p.stage,
        FontId::proportional(15.0),
        theme::MUTED,
    );
}
