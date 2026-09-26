//! "Previously on…" for streams: a page to add as an OBS browser source that
//! reads the last diary entry aloud in the character's voice before the
//! stream starts, the text following along on the game's parchment, with a
//! countdown to when the reading ends.

use crate::data::memory::Character;
use crate::tr;
use std::path::{Path, PathBuf};

pub fn dir() -> PathBuf {
    crate::platform::data_dir().join("stream")
}

/// Writes previously.html (and its narration, fonts and parchment) and
/// returns the page's path.
pub fn previously(c: &Character, day: &str, prose: &str, narration: &Path, parchment: Option<&Path>) -> Result<PathBuf, String> {
    let out = dir();
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::copy(narration, out.join("previously.mp3")).map_err(|e| format!("{}: {e}", narration.display()))?;
    for (name, bytes) in [
        ("marcellus.ttf", crate::theme::FONT_DISPLAY),
        ("alegreya-italic.ttf", crate::theme::FONT_ITALIC),
    ] {
        std::fs::write(out.join(name), bytes).map_err(|e| e.to_string())?;
    }
    // Only the paper part of the game's quest parchment texture, as the app uses it.
    let has_parchment = parchment
        .and_then(|p| image::open(p).ok())
        .map(|img| {
            let (w, h) = (img.width() as f32, img.height() as f32);
            img.crop_imm(0, (h * 0.012) as u32, (w * 0.586) as u32, (h * 0.784) as u32)
        })
        .is_some_and(|img| img.save(out.join("parchment.png")).is_ok());

    let mut blocks = prose.split("\n\n").map(str::trim).filter(|b| !b.is_empty());
    let title = blocks
        .next()
        .map(|t| t.trim_start_matches('#').trim().to_string())
        .unwrap_or_default();
    let body: String = blocks
        .map(|b| format!("<p>{}</p>", html(&b.replace("**", "").replace('*', "").replace('\n', " "))))
        .collect();
    let first = c.name.split(' ').next().unwrap_or(&c.name);
    let page = TEMPLATE
        .replace("{lang}", crate::i18n::current().code())
        .replace("{kicker}", &html(&tr!("Previously, in the journal of {name}", name = c.name)))
        .replace("{day}", &html(&crate::data::diary::pretty_day(day)))
        .replace("{title}", &html(&title))
        .replace("{body}", &body)
        .replace("{starting}", &html(&tr!("{name} reads; the stream begins in", name = first)))
        .replace("{now}", &html(tr!("now")))
        .replace("{bg}", if has_parchment { "url(parchment.png)" } else { "#e3c993" });
    let path = out.join("previously.html");
    std::fs::write(&path, page).map_err(|e| e.to_string())?;
    Ok(path)
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The page: the entry on parchment, scrolling with the narration, and a
/// countdown to when the reading ends. The audio starts on load; in OBS,
/// "Refresh browser when scene becomes active" restarts it.
const TEMPLATE: &str = r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<title>Previously</title>
<style>
@font-face { font-family: Marcellus; src: url(marcellus.ttf); }
@font-face { font-family: Alegreya; font-style: italic; src: url(alegreya-italic.ttf); }
html, body { margin: 0; height: 100%; background: #0a0e1f; color: #2e1d0e; overflow: hidden; }
body { display: grid; grid-template-columns: 1fr minmax(0, 46vw); gap: 4vw; align-items: center; padding: 0 6vw; box-sizing: border-box; }
.side { color: #ece6d6; font-family: Marcellus, serif; }
.side .kicker { color: #ffd100; font-size: 2.2vw; margin-bottom: 1vw; }
.side .day { color: #9b947f; font-size: 1.4vw; margin-bottom: 5vw; }
.label { color: #9b947f; font-size: 1.3vw; margin: 0; }
.clock { font-size: 7vw; margin: 0; color: #ffd100; letter-spacing: 0.02em; }
.bar { height: 0.5vw; background: #10162e; border-radius: 1vw; margin-top: 2vw; overflow: hidden; }
.bar div { height: 100%; width: 0; background: linear-gradient(90deg, #9a6a08, #f2c23a); }
.page { height: 84vh; background: {bg}; background-size: 100% 100%; border-radius: 0.4vw; box-shadow: 0 1vw 4vw rgba(0,0,0,.6); overflow: hidden; position: relative; }
.scroll { position: absolute; left: 3.2vw; right: 3.6vw; top: 0; will-change: transform; }
h1 { font-family: Marcellus, serif; color: #6b1e0a; font-weight: normal; font-size: 2.3vw; margin: 3vw 0 1.6vw; }
.side p { margin: 0; }
.page p { font-family: Alegreya, serif; font-style: italic; font-size: 1.45vw; line-height: 1.55; margin: 0 0 1.2vw; }
.page::after { content: ""; position: absolute; inset: auto 0 0 0; height: 18%; background: linear-gradient(transparent, rgba(227,201,147,.9)); }
</style>
</head>
<body>
<div class="side">
  <p class="kicker">{kicker}</p>
  <p class="day">{day}</p>
  <p class="label">{starting}</p>
  <p class="clock" id="clock">0:00</p>
  <div class="bar"><div id="bar"></div></div>
</div>
<div class="page"><div class="scroll" id="scroll"><h1>{title}</h1>{body}<div style="height:40vh"></div></div></div>
<audio id="voice" src="previously.mp3" autoplay></audio>
<script>
const voice = document.getElementById('voice'), scroll = document.getElementById('scroll');
const clock = document.getElementById('clock'), bar = document.getElementById('bar');
const fmt = s => Math.floor(s / 60) + ':' + String(Math.floor(s % 60)).padStart(2, '0');
function frame() {
  const d = voice.duration || 0, t = voice.currentTime || 0;
  if (d) {
    const left = Math.max(0, d - t);
    clock.textContent = left > 0.5 ? fmt(left) : '{now}';
    bar.style.width = (100 * t / d) + '%';
    // The text follows the reading: scroll so the spoken part stays in view.
    const page = scroll.parentElement.clientHeight, total = scroll.scrollHeight - page * 0.9;
    scroll.style.transform = 'translateY(' + (-Math.max(0, total) * Math.min(1, t / d)) + 'px)';
  }
  requestAnimationFrame(frame);
}
voice.addEventListener('loadedmetadata', () => { clock.textContent = fmt(voice.duration); });
voice.play().catch(() => {});
requestAnimationFrame(frame);
</script>
</body>
</html>
"#;
