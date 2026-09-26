//! The diary writer as the pages call it: whatever the settings pick (see
//! writer.rs).

use crate::{config, writer};

pub fn resolved() -> Option<writer::Resolved> {
    writer::resolve(&config::get().writer)
}

pub fn available() -> bool {
    resolved().is_some()
}

pub fn name() -> String {
    resolved().map(|r| r.name()).unwrap_or_default()
}

pub fn write(system: &str, prompt: &str) -> Result<String, String> {
    let r = resolved().ok_or("No writer is set up. Pick one in Settings.")?;
    writer::write(&r, system, prompt)
}
pub const WRITER: &str = "the writer"; // TODO(merge): ui/diary.rs uses name()
