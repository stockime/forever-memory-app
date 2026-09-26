//! Standing: where a character stands with the factions of the world, and how it changed.

use crate::State;
use crate::art::Art;
use crate::data::Model;
use egui::Ui;

pub fn show(ui: &mut Ui, _m: &Model, _st: &mut State, _art: &mut Art) {
    super::heading(ui, crate::tr!("Standing"));
}
