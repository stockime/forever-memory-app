//! STUB — replaced by the i18n branch on merge.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Lang { #[default] En, De, Fr, Es, Pt, Zh }
impl Lang {
    pub const ALL: [Lang; 6] = [Lang::En, Lang::De, Lang::Fr, Lang::Es, Lang::Pt, Lang::Zh];
    pub fn code(self) -> &'static str { ["en","de","fr","es","pt","zh"][self as usize] }
    pub fn native_name(self) -> &'static str { ["English","Deutsch","Français","Español","Português (Brasil)","简体中文"][self as usize] }
    pub fn english_name(self) -> &'static str { ["English","German","French","Spanish","Brazilian Portuguese","Simplified Chinese"][self as usize] }
    pub fn from_code(s: &str) -> Option<Lang> { let s = s.to_lowercase(); Lang::ALL.into_iter().find(|l| !s.is_empty() && s.starts_with(l.code())) }
    pub fn from_game_locale(s: &str) -> Option<Lang> { Lang::from_code(&s[..s.len().min(2)]) }
    pub fn detect_os() -> Lang { sys_locale::get_locale().and_then(|l| Lang::from_code(&l)).unwrap_or_default() }
}
static CUR: AtomicU8 = AtomicU8::new(0);
pub fn set(lang: Lang) { CUR.store(lang as u8, Ordering::Relaxed) }
pub fn current() -> Lang { Lang::ALL[CUR.load(Ordering::Relaxed) as usize] }
pub fn t(en: &'static str) -> &'static str { en }
pub fn tf(en: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = en.to_string();
    for (k, v) in args { s = s.replace(&format!("{{{k}}}"), &v.to_string()); }
    s
}
#[macro_export]
macro_rules! tr {
    ($s:literal) => { $crate::i18n::t($s) };
    ($s:literal, $($k:ident = $v:expr),+ $(,)?) => { $crate::i18n::tf($s, &[$((stringify!($k), &$v as &dyn std::fmt::Display)),+]) };
}
