//! Conversations: the NPCs a character spoke with, and each exchange played
//! back as a scene on parchment, a line at a time, like Immersion does in-game.

use super::widgets::{INK_BROWN, INK_RED};
use super::{character, quests};
use crate::State;
use crate::art::Art;
use crate::data::Model;
use crate::data::conversations::{self, Npc, Said, Scene};
use crate::theme::{self, EDGE, GOLD, INK, MUTED, RAISED};
use crate::tr;
use egui::{Align2, Color32, RichText, Sense, Stroke, Ui, vec2};

/// Letters a second as a line is written out.
const SPEED: f64 = 70.0;
const INK_BLUE: Color32 = Color32::from_rgb(0x1c, 0x2e, 0x52);

/// Which NPC and scene are open, and how far the scene has been played.
#[derive(Default)]
pub struct Talk {
    pub npc: Option<String>,
    pub search: String,
    scene: Option<usize>,
    /// Lines shown in full; the next one is being written out.
    shown: usize,
    started: f64,
    /// Open on everything said (FM_TALK, for screenshots).
    pub reveal: bool,
}

impl Talk {
    fn restart(&mut self, now: f64) {
        self.shown = 0;
        self.started = now;
    }
}

pub fn all(ui: &Ui, m: &Model, st: &State) -> std::sync::Arc<Vec<Npc>> {
    let c = character(m, st);
    super::story::memo(ui, m, c, "conversations", || conversations::build(m, c))
}

pub fn show(ui: &mut Ui, m: &Model, st: &mut State, art: &mut Art) {
    let npcs = all(ui, m, st);
    let list_w = 360.0;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(list_w);
            quests::tabs(ui, m, st);
            list(ui, &npcs, st);
        });
        ui.add_space(16.0);
        let w = ui.available_width();
        ui.vertical(|ui| {
            ui.set_width(w);
            let sel = st.talk.npc.as_ref().and_then(|n| npcs.iter().find(|x| &x.name == n));
            match sel {
                Some(n) => npc(ui, m, st, n, art),
                None if npcs.is_empty() => {
                    let name = character(m, st).name.split(' ').next().unwrap_or("").to_string();
                    super::empty(
                        ui,
                        art,
                        super::widgets::icons::SCROLL,
                        tr!("No words exchanged yet"),
                        &tr!("Nobody has spoken to {name} yet. Every word an NPC says to them will be kept here.", name = name),
                    )
                }
                None => super::hint(ui, tr!("Pick someone on the left.")),
            }
        });
    });
}

fn list(ui: &mut Ui, npcs: &[Npc], st: &mut State) {
    ui.add(
        egui::TextEdit::singleline(&mut st.talk.search)
            .hint_text(tr!("Filter by name or place"))
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    let needle = st.talk.search.to_lowercase();
    let shown: Vec<&Npc> = npcs
        .iter()
        .filter(|n| {
            needle.is_empty()
                || n.name.to_lowercase().contains(&needle)
                || n.place.to_lowercase().contains(&needle)
        })
        .collect();
    if shown.is_empty() && !npcs.is_empty() {
        ui.label(RichText::new(tr!("No one matches.")).color(MUTED));
    }
    if !st
        .talk
        .npc
        .as_ref()
        .is_some_and(|x| shown.iter().any(|n| &n.name == x))
    {
        st.talk.npc = shown.first().map(|n| n.name.clone());
        st.talk.scene = None;
    }
    egui::ScrollArea::vertical()
        .id_salt("npc-list")
        .auto_shrink(false)
        .show(ui, |ui| {
            for n in shown {
                let selected = st.talk.npc.as_ref() == Some(&n.name);
                let frame = egui::Frame::new()
                    .fill(if selected {
                        RAISED
                    } else {
                        Color32::TRANSPARENT
                    })
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 7));
                let r = frame
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            portrait(ui, &n.name, 34.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(&n.name).size(16.0).color(if selected {
                                    GOLD
                                } else {
                                    INK
                                }));
                                let times = if n.scenes.len() == 1 {
                                    tr!("once").to_string()
                                } else {
                                    tr!("{n} times", n = n.scenes.len())
                                };
                                let sub = [n.place.clone(), times, theme::day(n.last as f64)]
                                    .into_iter()
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                ui.label(RichText::new(sub).small().color(MUTED));
                            });
                        });
                    })
                    .response
                    .interact(Sense::click());
                if r.clicked() && !selected {
                    st.talk.npc = Some(n.name.clone());
                    st.talk.scene = None;
                }
            }
        });
}

/// A face for someone the game shows no portrait of: their initial on a
/// dark medallion with a brass rim.
pub fn portrait(ui: &mut Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size / 2.0;
    p.circle_filled(c, r, Color32::from_rgb(0x1a, 0x14, 0x10));
    // A hint of a hooded head and shoulders behind the letter.
    p.circle_filled(
        c + vec2(0.0, -r * 0.18),
        r * 0.36,
        Color32::from_rgb(0x2c, 0x22, 0x1a),
    );
    p.add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-r * 0.62, r * 0.86),
            c + vec2(-r * 0.4, r * 0.3),
            c + vec2(r * 0.4, r * 0.3),
            c + vec2(r * 0.62, r * 0.86),
        ],
        Color32::from_rgb(0x2c, 0x22, 0x1a),
        Stroke::NONE,
    ));
    let initial: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    p.text(
        c + vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        initial,
        theme::display_font(size * 0.5),
        Color32::from_rgb(0xe8, 0xc8, 0x7a),
    );
    p.circle_stroke(
        c,
        r - 1.0,
        Stroke::new(2.0, Color32::from_rgb(0x8a, 0x6d, 0x2c)),
    );
}

fn npc(ui: &mut Ui, m: &Model, st: &mut State, n: &Npc, art: &mut Art) {
    let now = ui.input(|i| i.time);
    let last = n.scenes.len().saturating_sub(1);
    let scene_i = match st.talk.scene {
        Some(i) if i <= last => i,
        _ => {
            st.talk.scene = Some(last);
            st.talk.restart(now);
            last
        }
    };
    if std::mem::take(&mut st.talk.reveal) {
        st.talk.shown = usize::MAX;
    }
    ui.horizontal(|ui| {
        portrait(ui, &n.name, 64.0);
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&n.name)
                    .font(theme::display_font(30.0))
                    .color(INK),
            );
            let met = if n.scenes.len() == 1 {
                tr!("met once, {when}", when = theme::when(n.first as f64))
            } else {
                tr!(
                    "met {n} times, first {first}, last {last}",
                    n = n.scenes.len(),
                    first = theme::day(n.first as f64),
                    last = theme::day(n.last as f64)
                )
            };
            let meta = if n.place.is_empty() {
                met
            } else {
                format!("{}, {met}", n.place)
            };
            ui.label(RichText::new(meta).color(MUTED));
        });
    });
    ui.add_space(8.0);
    if n.scenes.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (i, s) in n.scenes.iter().enumerate() {
                let on = i == scene_i;
                let r = ui
                    .add(
                        egui::Button::new(
                            RichText::new(theme::when(s.start as f64)).color(if on {
                                GOLD
                            } else {
                                INK
                            }),
                        )
                        .fill(if on { RAISED } else { Color32::TRANSPARENT })
                        .stroke(Stroke::new(1.0, EDGE)),
                    )
                    .on_hover_text(&s.place);
                if r.clicked() && !on {
                    st.talk.scene = Some(i);
                    st.talk.restart(now);
                }
            }
        });
        ui.add_space(8.0);
    }
    let scene = &n.scenes[scene_i];
    let me = character(m, st)
        .name
        .split(' ')
        .next()
        .unwrap_or("")
        .to_string();
    egui::ScrollArea::vertical()
        .id_salt("scene")
        .auto_shrink(false)
        .show(ui, |ui| {
            let done = st.talk.shown >= scene.lines.len();
            ui.horizontal(|ui| {
                if !done && ui.button(tr!("Show all")).clicked() {
                    st.talk.shown = usize::MAX;
                }
                if st.talk.shown > 0 && ui.button(tr!("From the start")).clicked() {
                    st.talk.restart(now);
                }
            });
            ui.add_space(6.0);
            let r = ui.scope(|ui| {
                super::widgets::parchment(ui, art, 720.0, |ui| {
                    play(ui, m, st, n, scene, &me, now);
                })
            });
            let click = ui.interact(r.response.rect, ui.id().with("scene-click"), Sense::click());
            let space = ui.input(|i| i.key_pressed(egui::Key::Space))
                && ui.memory(|mem| mem.focused().is_none());
            if (click.clicked() || space) && st.talk.shown < scene.lines.len() {
                let line = &scene.lines[st.talk.shown].1;
                let typed = ((now - st.talk.started) * SPEED) as usize;
                if typed < line.text().chars().count() {
                    st.talk.started = f64::MIN; // finish the line
                } else {
                    st.talk.shown += 1;
                    st.talk.started = now;
                }
            }
            if st.talk.shown < scene.lines.len() {
                click.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            ui.add_space(24.0);
        });
}

/// The lines so far, the current one written out letter by letter.
fn play(ui: &mut Ui, m: &Model, st: &State, n: &Npc, scene: &Scene, me: &str, now: f64) {
    ui.label(
        RichText::new(format!(
            "{}, {}",
            scene.place,
            theme::when(scene.start as f64)
        ))
        .family(theme::italic())
        .size(15.0)
        .color(INK_RED.gamma_multiply(0.85)),
    );
    ui.add_space(10.0);
    let mut speaker: Option<bool> = None;
    let typing = st.talk.shown.min(scene.lines.len());
    for (i, (_, said)) in scene.lines.iter().enumerate().take(typing + 1) {
        let by_npc = said.by_npc();
        if speaker != Some(by_npc) {
            if speaker.is_some() {
                ui.add_space(8.0);
            }
            let who = if by_npc { n.name.as_str() } else { me };
            let who = match said {
                Said::Yell(_) => tr!("{name} yells", name = who),
                _ => who.to_string(),
            };
            ui.label(
                RichText::new(who)
                    .font(theme::display_font(18.0))
                    .color(if by_npc { INK_RED } else { INK_BLUE }),
            );
            speaker = Some(by_npc);
        }
        let full = said.text();
        let text: String = if i < typing {
            full.to_string()
        } else {
            let k = ((now - st.talk.started).max(0.0) * SPEED) as usize;
            if k < full.chars().count() {
                ui.ctx().request_repaint();
            }
            full.chars().take(k).collect()
        };
        let complete = text.chars().count() >= full.chars().count();
        line(ui, m, said, &text.replace("$B", "\n"), complete);
        ui.add_space(6.0);
    }
    ui.add_space(8.0);
    let hint = if typing < scene.lines.len() {
        tr!("Click to go on")
    } else {
        tr!("The conversation ends here.")
    };
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        ui.label(
            RichText::new(hint)
                .small()
                .family(theme::italic())
                .color(INK_BROWN.gamma_multiply(0.7)),
        );
    });
}

fn line(ui: &mut Ui, m: &Model, said: &Said, text: &str, complete: bool) {
    let prose = |t: &str| RichText::new(t.to_string()).size(17.5).color(INK_BROWN);
    let aside = |t: String| {
        RichText::new(t)
            .size(15.5)
            .family(theme::italic())
            .color(INK_BLUE)
    };
    match said {
        Said::Offer {
            title, objective, ..
        } => {
            ui.label(
                RichText::new(title)
                    .font(theme::display_font(20.0))
                    .color(INK_RED),
            );
            ui.label(prose(text));
            if complete && !objective.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(objective.replace("$B", "\n"))
                        .size(16.0)
                        .family(theme::italic())
                        .color(INK_BROWN.gamma_multiply(0.85)),
                );
            }
        }
        Said::Yell(_) => {
            ui.label(RichText::new(text).size(19.0).color(INK_RED));
        }
        Said::Chose(_) => {
            ui.label(aside(format!("› {text}")));
        }
        Said::Accepted(_) => {
            ui.label(aside(tr!("Took on “{quest}”", quest = text)));
        }
        Said::TurnedIn { choice, .. } => {
            ui.label(aside(tr!("Saw “{quest}” through", quest = text)));
            if complete && let Some(l) = choice.as_deref().and_then(crate::data::memory::parse_link)
            {
                let name = if l.name.is_empty() {
                    m.memory
                        .items
                        .get(&l.id)
                        .map(|i| i.name.clone())
                        .unwrap_or_default()
                } else {
                    l.name
                };
                if !name.is_empty() {
                    ui.label(
                        RichText::new(tr!("and chose {item}", item = name))
                            .size(15.0)
                            .family(theme::italic())
                            .color(INK_BLUE.gamma_multiply(0.8)),
                    );
                }
            }
        }
        _ => {
            ui.label(prose(text));
        }
    }
}
