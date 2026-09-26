# Forever Memory

A local desktop app (Rust, egui) for struci's World of Warcraft: Forever
characters: the armory, plus everything the memory recorder keeps.

- **Overview**: leveling curve and gold against hours played, time per zone,
  XP per hour per session, open quests nearest to done.
- **Armory**: gear on the class scene with the game's tooltips and when each
  item was first acquired, the character sheet, both talent specs on their
  Classic backgrounds, Legacy trees.
- **Journal**: sessions sortable by date, length, experience, leveling speed,
  loot or deaths; each one as a feed of loot, money, XP, quests, places,
  deaths, NPCs and the chat going on at the time, with filters.
- **Diary**: a personality note per character, and for each day played an
  entry the character writes themselves, in first person and from inside the
  world, written by Claude (`claude-opus-5`) from that day's recorded facts.
  The facts sit under each entry so every sentence can be checked; entries and
  notes are committed to the memory archive (`characters/<name>/diary/`,
  `personality.md`). Other players' chat is never sent. Needs an Anthropic API
  key: `ANTHROPIC_API_KEY`, or paste one in the app (kept in
  `~/.config/forever-memory/anthropic-api-key`, mode 600).
- **Quests**: active (with objective progress), completed and abandoned, with
  the full quest text, rewards offered and chosen, time taken, and how the
  objectives progressed.
- **Map**: the recorded path on the zone's Classic world map with quests,
  deaths and level-ups where they happened, and a replay scrubber.
- **Combat**: fights and damage per second, abilities with crit rates, kills,
  and the ten seconds before every death, from the native combat logs.
- **Gold & loot**: where money comes from and goes, every item that went
  through the bags.
- **Players**: everyone met, searchable, with a class guess from their spells,
  what passed between you, what they cast and what they said.

Ctrl+K searches quests, players and items from anywhere; F5 reloads. The app
also reloads on its own when armory-sync commits a save.

## Data

| What | Where (override with) |
|---|---|
| Memory archive | `~/Work/personal/forever-memory` (`FM_REPO`) |
| Archived native logs | `~/.local/share/forever-memory/logs` (`FM_RAW_LOGS`) |
| Live native logs | the beta client's `Logs/` folder (`FM_LIVE_LOGS`) |
| Game art cache | `~/.cache/forever-memory/art` (`FM_ART`) |
| Art renderer | `~/.local/bin/wowdata` from stru.ci/wow (`FM_WOWDATA`) |

Missing art (icons, banners, talent backgrounds, world maps) is rendered from
the local game install by `wowdata art` in the background and cached.

## Build

    cargo build --release
    install -Dm755 target/release/forever-memory ~/.local/bin/forever-memory

`FM_SHOT=out.png FM_PAGE=quests forever-memory` saves a screenshot of a page
and quits (used to check layouts).
