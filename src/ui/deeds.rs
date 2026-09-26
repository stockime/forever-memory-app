//! Deeds: class-specific feats a character has earned, from what was recorded.

use crate::State;
use crate::art::Art;
use crate::data::Model;
use egui::Ui;

pub fn show(ui: &mut Ui, _m: &Model, _st: &mut State, _art: &mut Art) {
    super::heading(ui, crate::tr!("Deeds"));
}
