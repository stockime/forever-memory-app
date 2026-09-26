//! The diary: a personality note per character, and for each day played an
//! entry the character writes themselves, from the recorded facts.

use super::{card, character, label};
use crate::State;
use crate::art::Art;
use crate::data::{Model, diary};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED};
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
        super::empty(ui, "No days played yet.");
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
                label(ui, &format!("Who {} is", c.name.split(' ').next().unwrap_or(&c.name)));
                ui.label(RichText::new("Voice, history, what they care about. The diary is written in this voice.").small().color(MUTED));
                let (_, text) = st.note_edit.as_mut().unwrap();
                ui.add(egui::TextEdit::multiline(text).desired_rows(8).desired_width(f32::INFINITY).hint_text("Died of the plague in Brill and came back wrong. Dry, gallows humour; still loves the Light, which does not love him back…"));
                let changed = *text != c.personality;
                ui.horizontal(|ui| {
                    if ui.add_enabled(changed, egui::Button::new("Save note")).clicked() {
                        st.diary_status = Some(match diary::save_personality(&st.repo, c, text) {
                            Ok(()) => "Note saved and committed.".into(),
                            Err(e) => format!("Could not save: {e}"),
                        });
                        st.reload_now = true;
                    }
                    if changed {
                        ui.label(RichText::new("unsaved").small().color(MUTED));
                    }
                });
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                super::heading(ui, "Days");
                let missing: Vec<String> = days.iter().filter(|(d, _)| !c.diary.contains_key(d)).map(|(d, _)| d.clone()).collect();
                if !missing.is_empty() && st.diary_job.is_none() {
                    if ui.button(format!("Write {} missing", missing.len())).on_hover_text("Writes every day without an entry, oldest first").clicked() {
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
                            let state = if busy { "being written…" } else if written { "written" } else { "not written yet" };
                            ui.label(RichText::new(format!("{} played, {state}", theme::duration(*secs as f64))).small().color(MUTED));
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
                ui.label(RichText::new(format!("{} is writing…", c.name.split(' ').next().unwrap_or(&c.name))).color(MUTED));
            });
        }
        ui.add_space(8.0);
        let facts = diary::facts(m, c, day);
        match c.diary.get(day) {
            Some(stored) => {
                let (prose, stored_facts) = diary::split(stored);
                super::widgets::parchment(ui, art, 720.0, |ui| render(ui, prose));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(st.diary_job.is_none(), egui::Button::new("Rewrite")).on_hover_text("Writes this day again from the facts as they are now").clicked() {
                        start(m, st, day.to_string());
                    }
                });
                ui.add_space(8.0);
                egui::CollapsingHeader::new(RichText::new("The facts behind this entry").color(MUTED)).id_salt(("facts", day)).show(ui, |ui| {
                    ui.label(RichText::new(stored_facts).small().color(MUTED));
                });
            }
            None => {
                ui.label(RichText::new(format!("No entry yet. {} can write one from what was recorded that day:", c.name.split(' ').next().unwrap_or(&c.name))).color(MUTED));
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
                        ui.label(RichText::new("Claude Code (claude) isn't installed or can't run, so entries can't be written.").color(theme::DANGER));
                        return;
                    }
                    if ui.add_enabled(st.diary_job.is_none() && !facts.is_empty(), egui::Button::new(RichText::new("Write this day's entry").color(GOLD))).clicked() {
                        start(m, st, day.to_string());
                    }
                    ui.label(RichText::new(format!("Written by {} (claude -p) from these facts and the note. Other players' chat stays out.", crate::claude::WRITER)).small().color(MUTED));
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
            st.diary_status = Some("Written and committed to the archive.".into());
            st.reload_now = true;
            start_next(m, st);
        }
        Err(e) => {
            st.diary_error = Some(e);
            st.diary_queue.clear();
        }
    }
}
