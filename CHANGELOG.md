# Changelog

## 0.2.1

- Files a partial game install hasn't downloaded yet (icons, maps, talent
  art) come from Blizzard's CDN, the way the game streams them, and are
  cached.
- Quest objectives and the open quests on the Overview no longer show a
  dot at 0%.
- A day without a diary entry shows what happened on the parchment too.
- Chart axes for gold leave out the parts that are zero ("2g").
- MIT license.

## 0.2.0

- The map reveals only what your character has explored: the rest of the
  zone stays dark, as on the game's map. Needs the updated addon (0.2.1; the
  app offers the update in Settings) and a login; it also records when each
  place was discovered.
- A speed selector for reading the diary aloud (0.75× to 2×), remembered.
  Faster or slower, the voice keeps its pitch; the speed can change while
  it plays, and playback starts right away.
- `forever-memory sync` runs the recorder without the window, e.g. as a
  service, so saves, logs and backups are kept even while the app is closed.
  Only one recorder works on an archive at a time.
- The backup check reads the bucket instead of writing a test file (which a
  bucket with object lock would keep for years), and runs every minute, so
  diary entries and notes are backed up too.
- Bar chart tooltips float above the chart instead of being cut off at its
  top.

## 0.1.1

- Tables and lists no longer flicker: rows with equal values (the same
  damage, count or gold) keep their places instead of swapping every frame.

## 0.1.0

The first release for everyone.

- Settings: the game install and client (found on their own on Linux, macOS
  and Windows), the language, who writes the diary, the ElevenLabs key, the
  archive and an optional S3 backup.
- The armory addon ships with the app and installs with one click.
- The app records the game's saves into the archive itself, moves finished
  chat and combat logs out of the game folder between sessions, and renders
  the game's art from your install; nothing else to install.
- The diary is written by any agent CLI (Claude Code, Codex, Gemini CLI or a
  command of your own) or any OpenAI-compatible API, hosted or local.
- English, German, French, Spanish, Brazilian Portuguese and Simplified
  Chinese; diary entries are written and read aloud in the chosen language,
  with a voice per language.
