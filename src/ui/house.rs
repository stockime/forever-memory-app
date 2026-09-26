//! The House: everything a player's characters share, and the story of the account.

use crate::State;
use crate::art::Art;
use crate::data::Model;
use egui::Ui;

pub fn show(ui: &mut Ui, _m: &Model, _st: &mut State, _art: &mut Art) {
    super::heading(ui, crate::tr!("House"));
}
