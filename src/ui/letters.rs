//! Letters between a player's characters, in their own voices.

use crate::State;
use crate::art::Art;
use crate::data::Model;
use egui::Ui;

pub fn show(ui: &mut Ui, _m: &Model, _st: &mut State, _art: &mut Art) {
    super::heading(ui, crate::tr!("Letters"));
}
