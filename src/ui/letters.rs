//! Letters between a player's characters, in their own voices: pick who
//! writes to whom, read the correspondence on parchment, and let either
//! answer.

use super::{card, label};
use crate::State;
use crate::art::Art;
use crate::data::letters::{self, Facts, Letter};
use crate::data::memory::Character;
use crate::data::{Model, diary};
use crate::theme::{self, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Color32, RichText, Ui};

#[derive(Default)]
pub struct Letters {
    pub from: Option<String>,
    pub to: Option<String>,
    /// The archive's letters, read again when the model reloads or one is written.
    cache: Option<(std::time::SystemTime, Vec<Letter>)>,
    job: Option<Job>,
    error: Option<String>,
    status: Option<String>,
}

struct Job {
    from: String,
    to: String,
    facts: Facts,
    rx: std::sync::mpsc::Receiver<Result<String, String>>,
}

fn first(c: &Character) -> &str {
    c.name.split(' ').next().unwrap_or(&c.name)
}

fn find<'a>(m: &'a Model, slug: &str) -> Option<&'a Character> {
    m.memory.characters.iter().find(|c| c.slug == slug)
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let chars = &m.memory.characters;
    if chars.len() < 2 {
        let name = chars.first().map(first).unwrap_or_default();
        super::empty(
            ui,
            art,
            133468,
            tr!("A letter needs two"),
            &tr!(
                "{name} has nobody on this account to write to yet. Play another character with the addon, and they can start writing to each other.",
                name = name
            ),
        );
        return;
    }
    finish_job(m, st);
    if st.letters.cache.as_ref().is_none_or(|(t, _)| *t != m.loaded_at) {
        let slugs: Vec<&str> = chars.iter().map(|c| c.slug.as_str()).collect();
        st.letters.cache = Some((m.loaded_at, letters::load(&st.repo, &slugs)));
    }
    let current = super::character(m, st).slug.clone();
    let l = &mut st.letters;
    if l.from.as_deref().and_then(|s| find(m, s)).is_none() {
        l.from = Some(current);
    }
    let from = l.from.clone().unwrap_or_default();
    let all = l.cache.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
    if l.to.as_deref().is_none_or(|t| t == from || find(m, t).is_none()) {
        // Whoever they last wrote with, else the first of the others.
        let last = all.iter().rev().find_map(|x| {
            if x.from == from { Some(&x.to) } else if x.to == from { Some(&x.from) } else { None }
        });
        l.to = last
            .cloned()
            .or_else(|| chars.iter().find(|c| c.slug != from).map(|c| c.slug.clone()));
    }
    let to = l.to.clone().unwrap_or_default();

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(320.0);
            egui::ScrollArea::vertical().id_salt("letter-side").auto_shrink(false).show(ui, |ui| {
                card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    label(ui, tr!("Who writes"));
                    for c in chars {
                        if pick(ui, c, c.slug == from) {
                            st.letters.from = Some(c.slug.clone());
                        }
                    }
                    ui.add_space(10.0);
                    label(ui, tr!("To whom"));
                    for c in chars.iter().filter(|c| c.slug != from) {
                        if pick(ui, c, c.slug == to) {
                            st.letters.to = Some(c.slug.clone());
                        }
                    }
                });
                ui.add_space(12.0);
                correspondence(ui, m, st, &all);
                ui.add_space(12.0);
                household(ui, m, art);
            });
        });
        ui.add_space(18.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            if let (Some(a), Some(b)) = (find(m, &from), find(m, &to)) {
                thread(ui, m, st, art, a, b, &all);
            }
        });
    });
    if st.letters.job.is_some() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// One character as a row to pick; the chosen one is raised.
fn pick(ui: &mut Ui, c: &Character, selected: bool) -> bool {
    egui::Frame::new()
        .fill(if selected { RAISED } else { Color32::TRANSPARENT })
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(&c.name).color(if selected { GOLD } else { INK }));
            ui.label(
                RichText::new(tr!(
                    "Level {level} {race} {class}",
                    level = c.level,
                    race = c.race,
                    class = c.class
                ))
                .small()
                .color(MUTED),
            );
        })
        .response
        .interact(egui::Sense::click())
        .clicked()
}

/// Every pair that has written, newest exchange first.
fn correspondence(ui: &mut Ui, m: &Model, st: &mut State, all: &[Letter]) {
    let mut pairs: Vec<(&str, &str, usize, &Letter)> = vec![];
    for l in all.iter().rev() {
        let (a, b) = if l.from < l.to { (&l.from, &l.to) } else { (&l.to, &l.from) };
        match pairs.iter_mut().find(|p| p.0 == a && p.1 == b) {
            Some(p) => p.2 += 1,
            None => pairs.push((a, b, 1, l)),
        }
    }
    if pairs.is_empty() {
        return;
    }
    super::heading(ui, tr!("Correspondence"));
    for (a, b, n, last) in pairs {
        let (Some(ca), Some(cb)) = (find(m, a), find(m, b)) else {
            continue;
        };
        let pair = [Some(a), Some(b)];
        let open = pair.contains(&st.letters.from.as_deref())
            && pair.contains(&st.letters.to.as_deref());
        let r = egui::Frame::new()
            .fill(if open { RAISED } else { Color32::TRANSPARENT })
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(10, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(tr!("{a} and {b}", a = first(ca), b = first(cb)))
                        .color(if open { GOLD } else { INK }),
                );
                ui.label(
                    RichText::new(if n == 1 {
                        tr!("One letter, on {day}", day = diary::pretty_day(&last.day))
                    } else {
                        tr!(
                            "{n} letters, the last on {day}",
                            n = n,
                            day = diary::pretty_day(&last.day)
                        )
                    })
                    .small()
                    .color(MUTED),
                );
            })
            .response
            .interact(egui::Sense::click());
        if r.clicked() {
            // Open it on the side of whoever would write next.
            st.letters.from = Some(last.to.clone());
            st.letters.to = Some(last.from.clone());
        }
    }
}

/// What the characters share: the stable and the companions.
fn household(ui: &mut Ui, m: &Model, art: &mut Art) {
    if m.memory.account.is_empty() {
        return;
    }
    super::heading(ui, tr!("What they share"));
    ui.label(
        RichText::new(tr!("Mounts and companions, and who brought each home. Letters may speak of them."))
            .small()
            .color(MUTED),
    );
    ui.add_space(4.0);
    for a in &m.memory.account {
        ui.horizontal(|ui| {
            super::icon(ui, art, a.icon, theme::EDGE, 30.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(&a.name).color(INK));
                let by = m
                    .memory
                    .characters
                    .iter()
                    .find(|c| !a.by.is_empty() && c.guid == a.by)
                    .map(first);
                let when = diary::pretty_day(&diary::day_of(a.first));
                let text = match by {
                    Some(n) => tr!("{name}, {day}", name = n, day = when),
                    None => when,
                };
                ui.label(RichText::new(text).small().color(MUTED));
            });
        });
    }
}

fn thread(
    ui: &mut Ui,
    m: &Model,
    st: &mut State,
    art: &mut Art,
    a: &Character,
    b: &Character,
    all: &[Letter],
) {
    let letters = letters::thread(all, &a.slug, &b.slug);
    let busy = st.letters.job.as_ref().map(|j| (j.from.clone(), j.to.clone()));
    egui::ScrollArea::vertical().id_salt("letters").auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
        ui.set_width(ui.available_width().min(760.0));
        ui.label(
            RichText::new(tr!("{a} and {b}", a = first(a), b = first(b)))
                .font(theme::display_font(30.0))
                .color(INK),
        );
        if let Some(s) = st.letters.status.clone() {
            ui.label(RichText::new(s).small().color(MUTED));
        }
        if let Some(e) = st.letters.error.clone() {
            ui.label(RichText::new(e).color(theme::DANGER));
        }
        ui.add_space(8.0);
        if letters.is_empty() {
            super::quiet(
                ui,
                art,
                133468,
                &tr!(
                    "No letters between them yet. {name} can write the first, from what both have lived and what they share.",
                    name = first(a)
                ),
            );
        }
        for (i, l) in letters.iter().enumerate() {
            let (Some(from), Some(to)) = (find(m, &l.from), find(m, &l.to)) else {
                continue;
            };
            let last = i + 1 == letters.len();
            ui.add_space(10.0);
            ui.label(
                RichText::new(tr!(
                    "From {from} to {to}, {day}",
                    from = from.name,
                    to = to.name,
                    day = diary::pretty_day(&l.day)
                ))
                .color(MUTED),
            );
            if last {
                let stem = l.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                super::diary::narration(ui, st, from, &format!("letter-{stem}"), &l.text);
            }
            ui.add_space(4.0);
            super::widgets::parchment(ui, art, 720.0, |ui| render(ui, &l.text));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if last && crate::claude::available()
                    && ui.add_enabled(busy.is_none(), egui::Button::new(RichText::new(tr!("Reply as {name}", name = first(to))).color(GOLD)))
                        .on_hover_text(tr!("{name} answers this letter in their own voice", name = first(to)))
                        .clicked()
                {
                    start(m, st, to, from, Some((*l).clone()));
                }
                egui::CollapsingHeader::new(RichText::new(tr!("The facts behind this letter")).small().color(MUTED))
                    .id_salt(("letter-facts", &l.path))
                    .show(ui, |ui| {
                        ui.label(RichText::new(&l.facts).small().color(MUTED));
                    });
            });
        }
        ui.add_space(14.0);
        if let Some((f, t)) = &busy {
            let who = find(m, f).map(first).unwrap_or_default();
            let whom = find(m, t).map(first).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(tr!("{name} is writing to {to}…", name = who, to = whom)).color(MUTED));
            });
            return;
        }
        ui.horizontal(|ui| {
            if !crate::claude::available() {
                ui.label(RichText::new(tr!("Nobody is set up to write the letters. Pick a writer in Settings.")).color(theme::DANGER));
                return;
            }
            if ui.button(RichText::new(tr!("Write a letter")).color(GOLD)).clicked() {
                start(m, st, a, b, None);
            }
            ui.label(
                RichText::new(tr!(
                    "{from} writes to {to}, by {writer}, from both their days and what they share.",
                    from = first(a),
                    to = first(b),
                    writer = crate::claude::name()
                ))
                .small()
                .color(MUTED),
            );
        });
        ui.add_space(24.0);
    });
}

/// A letter: paragraphs, with the line breaks of the greeting and signature kept.
fn render(ui: &mut Ui, md: &str) {
    for block in md.split("\n\n") {
        let text = block.trim().replace("**", "").replace(['*', '_'], "");
        if text.is_empty() {
            continue;
        }
        ui.label(
            RichText::new(text)
                .family(theme::italic())
                .size(18.5)
                .color(super::widgets::INK_BROWN),
        );
        ui.add_space(8.0);
    }
}

fn start(m: &Model, st: &mut State, from: &Character, to: &Character, answering: Option<Letter>) {
    let all = st.letters.cache.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
    let thread = letters::thread(&all, &from.slug, &to.slug);
    let facts = letters::facts(m, from, to);
    let prompt = letters::prompt(m, from, to, &facts, &thread, answering.as_ref());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        tx.send(crate::claude::write(letters::SYSTEM, &prompt)).ok();
    });
    let l = &mut st.letters;
    l.error = None;
    l.status = None;
    l.from = Some(from.slug.clone());
    l.to = Some(to.slug.clone());
    l.job = Some(Job {
        from: from.slug.clone(),
        to: to.slug.clone(),
        facts,
        rx,
    });
}

fn finish_job(m: &Model, st: &mut State) {
    let l = &mut st.letters;
    let Some(job) = &l.job else { return };
    let Ok(result) = job.rx.try_recv() else {
        return;
    };
    let job = l.job.take().unwrap();
    let (Some(from), Some(to)) = (find(m, &job.from), find(m, &job.to)) else {
        return;
    };
    match result.and_then(|text| letters::store(&st.repo, from, to, &text, &job.facts)) {
        Ok(_) => {
            l.status = Some(tr!("Written and committed to the archive.").into());
            l.cache = None;
        }
        Err(e) => l.error = Some(e),
    }
}
