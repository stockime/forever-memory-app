//! Settings, and the welcome for a first start: where the game is, the
//! addon, the language, who writes the diary, the narration key, where the
//! archive lives and an optional backup to object storage.

use super::{card, heading, label};
use crate::config::{S3, Settings, Writer};
use crate::i18n::Lang;
use crate::theme::{self, DANGER, GOLD, GOOD, INK, MUTED};
use crate::{State, addon, platform, sync, tr, writer};
use egui::{RichText, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

/// What was found on this machine, looked up in the background.
#[derive(Clone, Default)]
struct Found {
    installs: Vec<PathBuf>,
    clis: Vec<(String, String, PathBuf)>,
    apis: Vec<writer::Api>,
}

#[derive(Default)]
pub struct Page {
    draft: Option<Settings>,
    found: Option<Found>,
    finding: Option<Receiver<Found>>,
    /// A running check: which one, and its answer.
    job: Option<(&'static str, Receiver<Result<String, String>>)>,
    results: std::collections::HashMap<&'static str, Result<String, String>>,
    models: Vec<String>,
    models_job: Option<Receiver<Result<Vec<String>, String>>>,
}

fn find() -> Receiver<Found> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        tx.send(Found {
            installs: platform::find_installs(),
            clis: writer::detect_clis()
                .into_iter()
                .map(|(p, bin)| (p.id.to_string(), p.name.to_string(), bin))
                .collect(),
            apis: writer::detect_apis(),
        })
        .ok();
    });
    rx
}

fn job(
    name: &'static str,
    f: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> (&'static str, Receiver<Result<String, String>>) {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        tx.send(f()).ok();
    });
    (name, rx)
}

pub fn show(ui: &mut Ui, st: &mut State, characters: usize) {
    let page = &mut st.settings_page;
    let saved = crate::config::get();
    let draft = page.draft.get_or_insert_with(|| saved.clone());
    if page.found.is_none() && page.finding.is_none() {
        page.finding = Some(find());
    }
    if let Some(rx) = &page.finding {
        if let Ok(f) = rx.try_recv() {
            page.found = Some(f);
            page.finding = None;
        }
    }
    if let Some((name, rx)) = &page.job {
        if let Ok(r) = rx.try_recv() {
            page.results.insert(name, r);
            page.job = None;
        }
    }
    if let Some(rx) = &page.models_job {
        if let Ok(r) = rx.try_recv() {
            match r {
                Ok(m) => page.models = m,
                Err(e) => {
                    page.results.insert("writer", Err(e));
                }
            }
            page.models_job = None;
        }
    }
    if page.finding.is_some() || page.job.is_some() || page.models_job.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(150));
    }
    let found = page.found.clone().unwrap_or_default();

    egui::ScrollArea::vertical()
        .id_salt("settings")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.set_max_width(780.0);
            if !draft.onboarded || characters == 0 {
                welcome(ui, draft, characters);
                ui.add_space(14.0);
            } else {
                heading(ui, tr!("Settings"));
                ui.add_space(8.0);
            }
            game(ui, draft, &found, page.finding.is_some());
            ui.add_space(12.0);
            language(ui, draft);
            ui.add_space(12.0);
            writer_card(
                ui,
                draft,
                &found,
                &mut page.job,
                &page.results,
                &mut page.models,
                &mut page.models_job,
            );
            ui.add_space(12.0);
            narration(ui, draft);
            ui.add_space(12.0);
            archive(ui, draft, &st.sync_status);
            ui.add_space(12.0);
            backup(ui, draft, &mut page.job, &page.results);
            ui.add_space(24.0);
        });

    // Text fields are saved when they lose focus, everything else at once.
    let editing = ui.ctx().memory(|m| m.focused().is_some());
    if *draft != saved && !editing {
        let paths = (
            draft.game_dir.clone(),
            draft.flavor.clone(),
            draft.archive.clone(),
            draft.raw_logs.clone(),
        );
        let old = (
            saved.game_dir.clone(),
            saved.flavor.clone(),
            saved.archive.clone(),
            saved.raw_logs.clone(),
        );
        let d = draft.clone();
        match crate::config::update(|s| *s = d) {
            Ok(()) => {
                crate::i18n::set(crate::config::get().lang());
                if paths != old {
                    st.paths_changed = true;
                }
            }
            Err(e) => {
                page.results.insert("save", Err(e));
            }
        }
    }
    if let Some(Err(e)) = page.results.get("save") {
        ui.label(RichText::new(e).color(DANGER));
    }
}

/// A checkbox that shows its box on the dark cards.
fn check(ui: &mut Ui, value: &mut bool, text: &str) {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0x4a, 0x55, 0x8a));
        ui.checkbox(value, text);
    });
}

fn ok(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(format!("✔ {text}")).color(GOOD));
}

fn warn(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(DANGER));
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(MUTED));
}

fn result(ui: &mut Ui, r: Option<&Result<String, String>>) {
    match r {
        Some(Ok(t)) => ok(ui, t),
        Some(Err(e)) => warn(ui, e),
        None => {}
    }
}

/// A folder field with a picker and a button that opens it.
fn folder(ui: &mut Ui, value: &mut Option<PathBuf>, default: PathBuf, id: &str) {
    let mut text = value
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id_salt(id)
                .hint_text(default.to_string_lossy())
                .desired_width((ui.available_width() - 190.0).max(200.0)),
        );
        if r.changed() {
            *value = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
        }
        if ui.button(tr!("Choose…")).clicked() {
            if let Some(p) = rfd::FileDialog::new()
                .set_directory(value.clone().unwrap_or(default.clone()))
                .pick_folder()
            {
                *value = Some(p);
            }
        }
        if ui.button(tr!("Open")).clicked() {
            platform::reveal(&value.clone().unwrap_or(default.clone()));
        }
    });
}

fn welcome(ui: &mut Ui, s: &mut Settings, characters: usize) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(tr!("Welcome to Forever Memory"))
                .font(theme::display_font(30.0))
                .color(GOLD),
        );
        ui.label(RichText::new(tr!("Your characters' armory, every step on the map, their fights, quests and the people they met, and a diary they write themselves. Three steps and you're set:")).color(INK));
        ui.add_space(10.0);
        let install = s.install();
        let flavor = install.as_ref().and_then(|i| s.flavor_in(i));
        let step = |ui: &mut Ui, done: bool, text: &str| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if done { "✔" } else { "○" })
                        .color(if done { GOOD } else { MUTED })
                        .size(18.0),
                );
                ui.label(RichText::new(text).color(if done { MUTED } else { INK }));
            });
        };
        step(
            ui,
            install.is_some(),
            tr!("Find World of Warcraft (below, if it wasn't found on its own)."),
        );
        let installed = match (&install, &flavor) {
            (Some(i), Some(f)) => addon::state(&addon::dir(i, f)) != addon::State::Missing,
            _ => false,
        };
        step(
            ui,
            installed,
            tr!("Install the armory addon. Restart the game if it is running."),
        );
        step(
            ui,
            characters > 0,
            tr!(
                "Log in with a character, then type /reload or log out, so the game saves what the addon recorded."
            ),
        );
        ui.add_space(8.0);
        if characters > 0
            && ui
                .button(RichText::new(tr!("Done, show my characters")).color(GOLD))
                .clicked()
        {
            s.onboarded = true;
        }
    });
}

fn game(ui: &mut Ui, s: &mut Settings, found: &Found, finding: bool) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("World of Warcraft"));
        let install = s.install();
        ui.horizontal(|ui| {
            let current = install
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| tr!("Not found").to_string());
            egui::ComboBox::from_id_salt("install")
                .width((ui.available_width() - 190.0).max(200.0))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for p in &found.installs {
                        let text = p.to_string_lossy().to_string();
                        if ui
                            .selectable_label(install.as_ref() == Some(p), text)
                            .clicked()
                        {
                            s.game_dir = Some(p.clone());
                            s.flavor.clear();
                        }
                    }
                });
            if ui.button(tr!("Choose…")).clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    s.game_dir = Some(p);
                    s.flavor.clear();
                }
            }
            if finding {
                ui.spinner();
            }
        });
        if let Some(d) = &s.game_dir {
            if !platform::is_install(d) {
                warn(
                    ui,
                    tr!(
                        "That folder isn't a World of Warcraft install: pick the one with Data and .build.info in it."
                    ),
                );
            }
        }
        let Some(install) = install else {
            note(
                ui,
                tr!(
                    "Pick the World of Warcraft folder, the one that holds _classic_ or _retail_ and the Data folder."
                ),
            );
            return;
        };
        let flavors = platform::flavors(&install);
        let flavor = s.flavor_in(&install);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("Game client")).color(MUTED));
            egui::ComboBox::from_id_salt("flavor")
                .selected_text(flavor.clone().unwrap_or_default())
                .show_ui(ui, |ui| {
                    for f in &flavors {
                        let saves = !platform::saved_variables(&install, f).is_empty();
                        let text = if saves {
                            format!("{f}  ✔")
                        } else {
                            f.clone()
                        };
                        if ui
                            .selectable_label(flavor.as_deref() == Some(f), text)
                            .clicked()
                        {
                            s.flavor = f.clone();
                        }
                    }
                });
            if let Some(l) = flavor
                .as_deref()
                .and_then(|f| platform::game_locale(&install, f))
            {
                ui.label(
                    RichText::new(tr!("game language: {locale}", locale = l))
                        .small()
                        .color(MUTED),
                );
            }
        });
        let Some(flavor) = flavor else { return };
        ui.add_space(10.0);
        label(ui, tr!("The armory addon"));
        let dir = addon::dir(&install, &flavor);
        let state = addon::state(&dir);
        let bundled = addon::bundled_version();
        match &state {
            addon::State::Missing => note(
                ui,
                tr!(
                    "It records what your characters live through into the game's saved variables. It changes nothing in the game and sends nothing anywhere."
                ),
            ),
            addon::State::Linked(t) => ok(ui, &tr!("Linked to {path}", path = t.display())),
            addon::State::Installed { version } if addon::outdated(&state) => warn(
                ui,
                &tr!(
                    "Version {old} is installed; {new} is here.",
                    old = version,
                    new = bundled
                ),
            ),
            addon::State::Installed { version } => ok(
                ui,
                &tr!("Version {version} is installed.", version = version),
            ),
        }
        ui.horizontal(|ui| {
            let text = match &state {
                addon::State::Missing => Some(tr!("Install the addon").to_string()),
                _ if addon::outdated(&state) => Some(tr!("Update the addon").to_string()),
                _ => None,
            };
            if let Some(text) = text {
                if ui.button(RichText::new(text).color(GOLD)).clicked() {
                    match addon::install(&dir) {
                        Ok(()) => {
                            if platform::game_running() {
                                ui.ctx().data_mut(|d| {
                                    d.insert_temp(egui::Id::new("addon-restart"), true)
                                });
                            }
                        }
                        Err(e) => {
                            ui.ctx()
                                .data_mut(|d| d.insert_temp(egui::Id::new("addon-error"), e));
                        }
                    }
                }
            }
            if dir.exists() && ui.button(tr!("Open")).clicked() {
                platform::reveal(&dir);
            }
        });
        if ui
            .ctx()
            .data(|d| d.get_temp::<bool>(egui::Id::new("addon-restart")))
            .unwrap_or(false)
        {
            warn(
                ui,
                tr!(
                    "Installed. The game is running: restart it (a /reload isn't enough) to load the addon."
                ),
            );
        }
        if let Some(e) = ui
            .ctx()
            .data(|d| d.get_temp::<String>(egui::Id::new("addon-error")))
        {
            warn(ui, &e);
        }
    });
}

fn language(ui: &mut Ui, s: &mut Settings) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("Language"));
        let auto = {
            let mut probe = s.clone();
            probe.language.clear();
            probe.lang()
        };
        let chosen = Lang::from_code(&s.language);
        let auto_text = tr!("Automatic ({language})", language = auto.native_name());
        egui::ComboBox::from_id_salt("language")
            .selected_text(
                chosen
                    .map(|l| l.native_name().to_string())
                    .unwrap_or(auto_text.clone()),
            )
            .show_ui(ui, |ui| {
                if ui.selectable_label(chosen.is_none(), auto_text).clicked() {
                    s.language.clear();
                }
                for l in Lang::ALL {
                    if ui
                        .selectable_label(chosen == Some(l), l.native_name())
                        .clicked()
                    {
                        s.language = l.code().into();
                    }
                }
            });
        note(
            ui,
            tr!(
                "Automatic follows the game's language, then the system's. Diary entries are written and read aloud in it."
            ),
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn writer_card(
    ui: &mut Ui,
    s: &mut Settings,
    found: &Found,
    job: &mut Option<(&'static str, Receiver<Result<String, String>>)>,
    results: &std::collections::HashMap<&'static str, Result<String, String>>,
    models: &mut Vec<String>,
    models_job: &mut Option<Receiver<Result<Vec<String>, String>>>,
) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("Who writes the diary"));
        note(
            ui,
            tr!(
                "An agent CLI you're already signed in to, or any OpenAI-compatible API, hosted or on this computer."
            ),
        );
        ui.add_space(4.0);
        let kind = match &s.writer {
            Writer::Auto => 0,
            Writer::Cli { .. } => 1,
            Writer::Api { .. } => 2,
            Writer::Off => 3,
        };
        ui.horizontal_wrapped(|ui| {
            for (i, text) in [
                tr!("Automatic"),
                tr!("Agent CLI"),
                tr!("API"),
                tr!("Nobody"),
            ]
            .into_iter()
            .enumerate()
            {
                if ui.selectable_label(kind == i, text).clicked() && kind != i {
                    s.writer = match i {
                        0 => Writer::Auto,
                        1 => Writer::Cli {
                            command: found
                                .clis
                                .first()
                                .map(|c| c.0.clone())
                                .unwrap_or_else(|| "claude".into()),
                        },
                        2 => match found.apis.first() {
                            Some(a) => Writer::Api {
                                base_url: a.base_url.clone(),
                                api_key: a.api_key.clone(),
                                model: a.model.clone(),
                            },
                            None => Writer::Api {
                                base_url: "https://api.openai.com/v1".into(),
                                api_key: String::new(),
                                model: String::new(),
                            },
                        },
                        _ => Writer::Off,
                    };
                }
            }
        });
        ui.add_space(6.0);
        match &mut s.writer {
            Writer::Auto => match writer::resolve(&Writer::Auto) {
                Some(r) => ok(ui, &tr!("Uses {writer}.", writer = r.name())),
                None => warn(
                    ui,
                    tr!(
                        "Nothing found: install Claude Code, Codex or Gemini CLI, set OPENAI_API_KEY, or run Ollama or LM Studio."
                    ),
                ),
            },
            Writer::Cli { command } => {
                ui.horizontal_wrapped(|ui| {
                    for (id, name, _) in &found.clis {
                        if ui.selectable_label(command == id, name).clicked() {
                            *command = id.clone();
                        }
                    }
                });
                let preset = writer::PRESETS.iter().any(|p| p.id == command.as_str());
                let mut custom = if preset {
                    String::new()
                } else {
                    command.clone()
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr!("Or a command")).color(MUTED));
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut custom)
                                .hint_text("opencode run")
                                .desired_width(f32::INFINITY),
                        )
                        .changed()
                    {
                        *command = custom.clone();
                    }
                });
                note(
                    ui,
                    tr!(
                        "The prompt goes to the command on stdin and the entry is read from its output. {system} in the command stands for the instructions, {out} for a file it writes the answer to."
                    ),
                );
                match writer::resolve(&Writer::Cli {
                    command: command.clone(),
                }) {
                    Some(r) => ok(ui, &tr!("Uses {writer}.", writer = r.name())),
                    None => warn(ui, tr!("That command isn't installed here.")),
                }
            }
            Writer::Api {
                base_url,
                api_key,
                model,
            } => {
                if !found.apis.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(tr!("Found:")).color(MUTED));
                        for a in &found.apis {
                            if ui
                                .selectable_label(base_url == &a.base_url, &a.name)
                                .clicked()
                            {
                                *base_url = a.base_url.clone();
                                *api_key = a.api_key.clone();
                                *model = a.model.clone();
                            }
                        }
                    });
                }
                egui::Grid::new("api")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new(tr!("Base URL")).color(MUTED));
                        ui.add(
                            egui::TextEdit::singleline(base_url)
                                .hint_text("https://api.openai.com/v1")
                                .desired_width(420.0),
                        );
                        ui.end_row();
                        ui.label(RichText::new(tr!("API key")).color(MUTED));
                        ui.add(
                            egui::TextEdit::singleline(api_key)
                                .password(true)
                                .hint_text(tr!("not needed for local servers"))
                                .desired_width(420.0),
                        );
                        ui.end_row();
                        ui.label(RichText::new(tr!("Model")).color(MUTED));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(model).desired_width(260.0));
                            if !models.is_empty() {
                                egui::ComboBox::from_id_salt("models")
                                    .selected_text(tr!("Pick"))
                                    .width(120.0)
                                    .show_ui(ui, |ui| {
                                        for m in models.iter() {
                                            if ui.selectable_label(model == m, m).clicked() {
                                                *model = m.clone();
                                            }
                                        }
                                    });
                            }
                            if ui
                                .add_enabled(
                                    models_job.is_none(),
                                    egui::Button::new(tr!("List models")),
                                )
                                .clicked()
                            {
                                let (b, k) = (base_url.clone(), api_key.clone());
                                let (tx, rx) = channel();
                                std::thread::spawn(move || {
                                    tx.send(writer::list_models(
                                        &b,
                                        &k,
                                        std::time::Duration::from_secs(10),
                                    ))
                                    .ok();
                                });
                                *models_job = Some(rx);
                            }
                        });
                        ui.end_row();
                    });
            }
            Writer::Off => note(
                ui,
                tr!("Days are listed with what happened, but no entries are written."),
            ),
        }
        if !matches!(s.writer, Writer::Off) {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let busy = job.as_ref().is_some_and(|j| j.0 == "writer");
                if ui.add_enabled(!busy && job.is_none(), egui::Button::new(tr!("Try it"))).clicked() {
                    let w = s.writer.clone();
                    let lang = crate::i18n::current().english_name();
                    *job = Some(self::job("writer", move || {
                        let r = writer::resolve(&w).ok_or_else(|| tr!("Nothing to run.").to_string())?;
                        let answer = writer::write(
                            &r,
                            "You are a Forsaken paladin writing one line in your journal.",
                            &format!("In one short sentence in {lang}, write how today felt. No preamble."),
                        )?;
                        Ok(format!("{}: “{}”", r.name(), answer.lines().next().unwrap_or("").trim()))
                    }));
                }
                if busy {
                    ui.spinner();
                }
            });
            result(ui, results.get("writer"));
        }
    });
}

fn narration(ui: &mut Ui, s: &mut Settings) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("Reading the diary aloud"));
        note(
            ui,
            tr!(
                "Optional. ElevenLabs gives every character a voice of their own, in the style of their race in the game. Create a key at elevenlabs.io under Developers → API keys."
            ),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("ElevenLabs API key")).color(MUTED));
            ui.add(
                egui::TextEdit::singleline(&mut s.elevenlabs_key)
                    .password(true)
                    .desired_width(360.0),
            );
        });
        if std::env::var("ELEVENLABS_API_KEY").is_ok() {
            note(
                ui,
                tr!("ELEVENLABS_API_KEY is set in the environment and is used instead."),
            );
        }
    });
}

fn archive(ui: &mut Ui, s: &mut Settings, status: &sync::SharedStatus) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("The archive"));
        note(
            ui,
            tr!(
                "Everything the addon records is kept here for good (the game only keeps 30 days)."
            ),
        );
        folder(
            ui,
            &mut s.archive,
            platform::data_dir().join("archive"),
            "archive",
        );
        check(
            ui,
            &mut s.record,
            tr!("Record each save of the game into the archive"),
        );
        check(
            ui,
            &mut s.archive_logs,
            tr!("Move finished chat and combat logs here between sessions"),
        );
        if s.archive_logs {
            folder(
                ui,
                &mut s.raw_logs,
                platform::data_dir().join("logs"),
                "raw-logs",
            );
        }
        if sync::git_available() {
            note(
                ui,
                tr!("Each save is a git commit, so the archive has a full history."),
            );
        } else {
            note(
                ui,
                tr!("Install git to keep a full history of the archive."),
            );
        }
        let st = status.lock().map(|s| s.clone()).unwrap_or_default();
        if let Some((t, msg)) = &st.last {
            let secs = t
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            ui.label(
                RichText::new(format!("{} · {msg}", theme::ago(secs)))
                    .small()
                    .color(MUTED),
            );
        }
        if let Some(e) = &st.error {
            warn(ui, e);
        }
    });
}

fn backup(
    ui: &mut Ui,
    s: &mut Settings,
    job: &mut Option<(&'static str, Receiver<Result<String, String>>)>,
    results: &std::collections::HashMap<&'static str, Result<String, String>>,
) {
    card(ui, |ui| {
        ui.set_width(ui.available_width());
        label(ui, tr!("Backup to object storage"));
        check(
            ui,
            &mut s.s3.enabled,
            tr!("Back up the archive and the native logs to S3-compatible storage"),
        );
        if !s.s3.enabled {
            note(
                ui,
                tr!(
                    "Optional. Works with AWS S3, Hetzner, Backblaze B2, Cloudflare R2, MinIO and others."
                ),
            );
            return;
        }
        let S3 {
            endpoint,
            region,
            bucket,
            access_key,
            secret_key,
            ..
        } = &mut s.s3;
        egui::Grid::new("s3")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                for (text, value, hint, secret) in [
                    (
                        tr!("Endpoint"),
                        endpoint,
                        "fsn1.your-objectstorage.com",
                        false,
                    ),
                    (tr!("Region"), region, "fsn1", false),
                    (tr!("Bucket"), bucket, "forever-memory", false),
                    (tr!("Access key"), access_key, "", true),
                    (tr!("Secret key"), secret_key, "", true),
                ] {
                    ui.label(RichText::new(text).color(MUTED));
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .password(secret)
                            .hint_text(hint)
                            .desired_width(360.0),
                    );
                    ui.end_row();
                }
            });
        note(
            ui,
            tr!(
                "New archive history goes up as git bundles after each save; finished logs are compressed and uploaded before they are moved. Turn on object lock for the bucket to make the backup tamper-proof."
            ),
        );
        ui.horizontal(|ui| {
            let busy = job.as_ref().is_some_and(|j| j.0 == "s3");
            if ui
                .add_enabled(
                    job.is_none() && s.s3.ready(),
                    egui::Button::new(tr!("Check the connection")),
                )
                .clicked()
            {
                let cfg = s.s3.clone();
                *job = Some(self::job("s3", move || {
                    crate::s3::check(&cfg).map(|()| {
                        tr!("Connected: a test file was written and read back.").to_string()
                    })
                }));
            }
            if busy {
                ui.spinner();
            }
        });
        result(ui, results.get("s3"));
    });
}
