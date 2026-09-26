//! The diary: a personality note per character, and for each day played an
//! entry the character writes themselves, from the recorded facts.

use super::{card, character, label};
use crate::State;
use crate::art::Art;
use crate::data::{Model, diary};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Color32, RichText, Ui};

pub struct Job {
    pub slug: String,
    pub day: String,
    pub facts: Vec<String>,
    pub rx: std::sync::mpsc::Receiver<Result<String, String>>,
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let c = character(m, st);
    finish_job(m, st);
    let days = diary::days(c);
    if days.is_empty() {
        super::empty(ui, tr!("No days played yet."));
        return;
    }
    if !st
        .diary_day
        .as_ref()
        .is_some_and(|d| days.iter().any(|(x, _)| x == d))
    {
        st.diary_day = days.first().map(|(d, _)| d.clone());
    }
    if st
        .note_edit
        .as_ref()
        .is_none_or(|(slug, _)| *slug != c.slug)
    {
        st.note_edit = Some((c.slug.clone(), c.personality.clone()));
    }

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(340.0);
            card(ui, |ui| {
                ui.set_width(ui.available_width());
                label(ui, &tr!("Who {name} is", name = c.name.split(' ').next().unwrap_or(&c.name)));
                ui.label(RichText::new(tr!("Voice, history, what they care about. The diary is written in this voice.")).small().color(MUTED));
                let (_, text) = st.note_edit.as_mut().unwrap();
                ui.add(egui::TextEdit::multiline(text).desired_rows(8).desired_width(f32::INFINITY).hint_text(tr!("Died of the plague in Brill and came back wrong. Dry, gallows humour; still loves the Light, which does not love him back…")));
                let changed = *text != c.personality;
                ui.horizontal(|ui| {
                    if ui.add_enabled(changed, egui::Button::new(tr!("Save note"))).clicked() {
                        st.diary_status = Some(match diary::save_personality(&st.repo, c, text) {
                            Ok(()) => tr!("Note saved and committed.").into(),
                            Err(e) => tr!("Could not save: {error}", error = e),
                        });
                        st.reload_now = true;
                    }
                    if changed {
                        ui.label(RichText::new(tr!("unsaved")).small().color(MUTED));
                    }
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                super::heading(ui, tr!("Days"));
                let missing: Vec<String> = days.iter().filter(|(d, _)| !c.diary.contains_key(d)).map(|(d, _)| d.clone()).collect();
                if !missing.is_empty() && st.diary_job.is_none() {
                    if ui.button(tr!("Write {n} missing", n = missing.len())).on_hover_text(tr!("Writes every day without an entry, oldest first")).clicked() {
                        st.diary_queue = missing.into_iter().rev().collect();
                        start_next(m, st);
                    }
                }
            });
            egui::ScrollArea::vertical().id_salt("days").auto_shrink(false).show(ui, |ui| {
                for (d, secs) in &days {
                    let selected = st.diary_day.as_deref() == Some(d.as_str());
                    let written = c.diary.contains_key(d);
                    let busy = st.diary_job.as_ref().is_some_and(|j| j.day == *d && j.slug == c.slug);
                    let r = egui::Frame::new()
                        .fill(if selected { RAISED } else { Color32::TRANSPARENT })
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 7))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(diary::pretty_day(d)).color(if selected { GOLD } else { INK }));
                            let state = if busy { tr!("being written…") } else if written { tr!("written") } else { tr!("not written yet") };
                            ui.label(RichText::new(tr!("{time} played, {state}", time = theme::duration(*secs as f64), state = state)).small().color(MUTED));
                        })
                        .response
                        .interact(egui::Sense::click());
                    if r.clicked() {
                        st.diary_day = Some(d.clone());
                    }
                }
            });
        });
        ui.add_space(18.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            let day = st.diary_day.clone().unwrap_or_default();
            entry(ui, m, st, &day, art);
        });
    });
    if st.diary_job.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
}

fn entry(ui: &mut Ui, m: &Model, st: &mut State, day: &str, art: &mut Art) {
    let c = character(m, st);
    egui::ScrollArea::vertical().id_salt("entry").auto_shrink(false).show(ui, |ui| {
        ui.set_width(ui.available_width().min(760.0));
        ui.label(RichText::new(diary::pretty_day(day)).font(theme::display_font(30.0)).color(INK));
        if let Some(s) = st.diary_status.clone() {
            ui.label(RichText::new(s).small().color(MUTED));
        }
        if let Some(e) = st.diary_error.clone() {
            ui.label(RichText::new(e).color(theme::DANGER));
        }
        let busy = st.diary_job.as_ref().is_some_and(|j| j.day == day && j.slug == c.slug);
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(tr!("{name} is writing…", name = c.name.split(' ').next().unwrap_or(&c.name))).color(MUTED));
            });
        }
        ui.add_space(8.0);
        let facts = diary::facts(m, c, day);
        match c.diary.get(day) {
            Some(stored) => {
                let (prose, stored_facts) = diary::split(stored);
                narration(ui, st, c, day, prose);
                ui.add_space(8.0);
                super::widgets::parchment(ui, art, 720.0, |ui| render(ui, prose));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(st.diary_job.is_none(), egui::Button::new(tr!("Rewrite"))).on_hover_text(tr!("Writes this day again from the facts as they are now")).clicked() {
                        start(m, st, day.to_string());
                    }
                });
                ui.add_space(8.0);
                egui::CollapsingHeader::new(RichText::new(tr!("The facts behind this entry")).color(MUTED)).id_salt(("facts", day)).show(ui, |ui| {
                    ui.label(RichText::new(stored_facts).small().color(MUTED));
                });
            }
            None => {
                ui.label(RichText::new(tr!("No entry yet. {name} can write one from what was recorded that day:", name = c.name.split(' ').next().unwrap_or(&c.name))).color(MUTED));
                ui.add_space(6.0);
                egui::Frame::new().stroke(egui::Stroke::new(1.0, EDGE)).corner_radius(6).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for f in &facts {
                        ui.label(RichText::new(f).small().color(MUTED));
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if !crate::claude::available() {
                        ui.label(RichText::new(tr!("Claude Code (claude) isn't installed or can't run, so entries can't be written.")).color(theme::DANGER));
                        return;
                    }
                    if ui.add_enabled(st.diary_job.is_none() && !facts.is_empty(), egui::Button::new(RichText::new(tr!("Write this day's entry")).color(GOLD))).clicked() {
                        start(m, st, day.to_string());
                    }
                    ui.label(RichText::new(tr!("Written by {writer} (claude -p) from these facts and the note. Other players' chat stays out.", writer = crate::claude::WRITER)).small().color(MUTED));
                });
            }
        }
        ui.add_space(24.0);
    });
}

/// Minimal Markdown: headings, paragraphs, and emphasis markers dropped.
fn render(ui: &mut Ui, md: &str) {
    for block in md.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        if let Some(h) = block.strip_prefix('#') {
            ui.label(
                RichText::new(h.trim_start_matches('#').trim())
                    .font(theme::display_font(24.0))
                    .color(super::widgets::INK_RED),
            );
            ui.add_space(4.0);
            continue;
        }
        let text = block
            .replace("**", "")
            .replace('*', "")
            .replace('_', " ")
            .replace('\n', " ");
        ui.label(
            RichText::new(text)
                .family(theme::italic())
                .size(18.5)
                .color(super::widgets::INK_BROWN),
        );
        ui.add_space(8.0);
    }
}

fn start(m: &Model, st: &mut State, day: String) {
    let c = character(m, st);
    let facts = diary::facts(m, c, &day);
    let previous = diary::previous(&st.repo, c, &day);
    let prompt = diary::prompt(
        c,
        &day,
        &facts,
        previous.as_ref().map(|(d, t)| (d.as_str(), t.as_str())),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(crate::claude::write(diary::SYSTEM, &prompt)).ok();
    });
    st.diary_error = None;
    st.diary_status = None;
    st.diary_job = Some(Job {
        slug: c.slug.clone(),
        day,
        facts,
        rx,
    });
}

fn start_next(m: &Model, st: &mut State) {
    if let Some(day) = st.diary_queue.pop() {
        st.diary_day = Some(day.clone());
        start(m, st, day);
    }
}

fn finish_job(m: &Model, st: &mut State) {
    let Some(job) = &st.diary_job else { return };
    let Ok(result) = job.rx.try_recv() else {
        return;
    };
    let job = st.diary_job.take().unwrap();
    let Some(c) = m.memory.characters.iter().find(|c| c.slug == job.slug) else {
        return;
    };
    match result.and_then(|entry| diary::store(&st.repo, c, &job.day, &entry, &job.facts)) {
        Ok(()) => {
            st.diary_status = Some(tr!("Written and committed to the archive.").into());
            st.reload_now = true;
            start_next(m, st);
        }
        Err(e) => {
            st.diary_error = Some(e);
            st.diary_queue.clear();
        }
    }
}

// ---- narration ----

pub enum VoiceMsg {
    Created,
    Spoken(std::path::PathBuf, String),
    Failed(String),
}

fn run(st: &mut State, busy: &str, work: impl FnOnce() -> VoiceMsg + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(work()).ok();
    });
    st.voice_job = Some(rx);
    st.voice_busy = busy.to_string();
    st.voice_error = None;
}

fn poll_voice(st: &mut State) {
    let Some(rx) = &st.voice_job else { return };
    let Ok(msg) = rx.try_recv() else { return };
    st.voice_job = None;
    st.voice_busy.clear();
    match msg {
        VoiceMsg::Created => st.reload_now = true,
        VoiceMsg::Spoken(path, key) => match crate::voice::Player::play(&path, key) {
            Ok(p) => st.narrator = Some(p),
            Err(e) => st.voice_error = Some(e),
        },
        VoiceMsg::Failed(e) => st.voice_error = Some(e),
    }
}

/// The voice bar above an entry: set up a key, give the character a voice,
/// then listen.
fn narration(
    ui: &mut Ui,
    st: &mut State,
    c: &crate::data::memory::Character,
    day: &str,
    prose: &str,
) {
    use crate::voice;
    poll_voice(st);
    if st.voice_job.is_some() || st.narrator.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    let first = c.name.split(' ').next().unwrap_or(&c.name).to_string();
    card(ui, |ui| {
        ui.set_width(ui.available_width().min(720.0));
        if let Some(e) = &st.voice_error {
            ui.label(RichText::new(e).color(theme::DANGER));
        }
        if !st.voice_busy.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(&st.voice_busy).color(MUTED));
            });
            return;
        }
        let Some(key) = voice::api_key() else {
            label(ui, tr!("Hear it read aloud"));
            ui.label(RichText::new(tr!("Narration uses ElevenLabs. Paste an API key (kept in {path}, readable only by you) or set ELEVENLABS_API_KEY.", path = voice::key_file().display())).small().color(MUTED));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut st.el_key_input)
                        .password(true)
                        .hint_text(tr!("ElevenLabs API key"))
                        .desired_width(320.0),
                );
                if ui
                    .add_enabled(
                        st.el_key_input.trim().len() > 10,
                        egui::Button::new(tr!("Save key")),
                    )
                    .clicked()
                {
                    st.voice_error = voice::save_key(&st.el_key_input).err();
                    st.el_key_input.clear();
                }
            });
            return;
        };
        let Some(v) = voice::load_voice(&st.repo, c) else {
            label(ui, &tr!("{name}'s voice", name = first));
            let woman = c.snapshot.get("sex").and_then(serde_json::Value::as_i64) == Some(3);
            let about = if woman {
                tr!(
                    "{name} doesn't have a voice yet. It's made once, in the style of the game's {race} women and shaped by the note above, then kept for every entry.",
                    name = first,
                    race = c.race
                )
            } else {
                tr!(
                    "{name} doesn't have a voice yet. It's made once, in the style of the game's {race} men and shaped by the note above, then kept for every entry.",
                    name = first,
                    race = c.race
                )
            };
            ui.label(RichText::new(about).small().color(MUTED));
            if ui
                .button(RichText::new(tr!("Give {name} a voice", name = first)).color(GOLD))
                .clicked()
            {
                let (desc, seed, name, repo, ch) = (
                    voice::describe(c),
                    voice::seed(c),
                    format!("{} (Forever Memory)", c.name),
                    st.repo.clone(),
                    c.clone(),
                );
                run(
                    st,
                    &tr!("Finding {name}'s voice…", name = first),
                    move || {
                        // One step: design in the race's in-game style and keep the first voice.
                        let made = voice::design(&key, &desc, seed).and_then(|previews| {
                            let (gid, _) = previews.into_iter().next().ok_or("no voice")?;
                            let voice_id = voice::create(&key, &name, &desc, &gid)?;
                            voice::save_voice(
                                &repo,
                                &ch,
                                &voice::Voice {
                                    voice_id,
                                    name,
                                    description: desc,
                                },
                            )
                        });
                        match made {
                            Ok(()) => VoiceMsg::Created,
                            Err(e) => VoiceMsg::Failed(e),
                        }
                    },
                );
            }
            return;
        };
        let text = voice::spoken_text(prose);
        let path = voice::audio_path(c, day, &v.voice_id, &text);
        let key_name = format!("entry-{}-{day}", c.slug);
        let playing = st.narrator.as_ref().filter(|p| p.key == key_name);
        match playing {
            Some(p) if !p.finished() => {
                let (pos, len, paused) = (p.position(), p.length, p.paused());
                let mut stop = false;
                ui.horizontal(|ui| {
                    if ui
                        .button(if paused {
                            format!("▶ {}", tr!("Resume"))
                        } else {
                            format!("⏸ {}", tr!("Pause"))
                        })
                        .clicked()
                    {
                        p.toggle();
                    }
                    if ui.button(format!("■ {}", tr!("Stop"))).clicked() {
                        stop = true;
                    }
                    let w = (ui.available_width() - 110.0).max(80.0);
                    super::widgets::bar(ui, (pos / len.max(1.0)) as f32, GOLD, w);
                    ui.label(
                        RichText::new(format!("{} / {}", clock(pos), clock(len)))
                            .small()
                            .color(MUTED),
                    );
                });
                if stop {
                    st.narrator = None;
                }
            }
            _ => {
                if st
                    .narrator
                    .as_ref()
                    .is_some_and(|p| p.key == key_name && p.finished())
                {
                    st.narrator = None;
                }
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new(format!("▶ {}", tr!("Listen to {name}", name = first)))
                                .color(GOLD),
                        )
                        .clicked()
                    {
                        if path.exists() {
                            match crate::voice::Player::play(&path, key_name.clone()) {
                                Ok(p) => st.narrator = Some(p),
                                Err(e) => st.voice_error = Some(e),
                            }
                        } else {
                            let (vid, t, p2, k2) = (
                                v.voice_id.clone(),
                                text.clone(),
                                path.clone(),
                                key_name.clone(),
                            );
                            run(
                                st,
                                &tr!("{name} is clearing their throat…", name = first),
                                move || match crate::voice::speak(&key, &vid, &t) {
                                    Ok(mp3) => {
                                        let _ = std::fs::create_dir_all(p2.parent().unwrap());
                                        match std::fs::write(&p2, mp3) {
                                            Ok(()) => VoiceMsg::Spoken(p2, k2),
                                            Err(e) => VoiceMsg::Failed(e.to_string()),
                                        }
                                    }
                                    Err(e) => VoiceMsg::Failed(e),
                                },
                            );
                        }
                    }
                    ui.label(
                        RichText::new(if path.exists() {
                            tr!("read aloud before; plays from the cache")
                        } else {
                            tr!("read aloud by ElevenLabs in their voice")
                        })
                        .small()
                        .color(MUTED),
                    );
                });
            }
        }
    });
}

fn clock(secs: f64) -> String {
    format!("{}:{:02}", secs as i64 / 60, secs as i64 % 60)
}
