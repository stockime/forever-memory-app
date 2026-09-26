//! The Chronicle: a character's diary bound as a tome, read page by page.

use crate::State;
use crate::art::Art;
use crate::data::Model;
use egui::Ui;

pub fn show(ui: &mut Ui, _m: &Model, _st: &mut State, _art: &mut Art) {
    super::heading(ui, crate::tr!("Chronicle"));
}
