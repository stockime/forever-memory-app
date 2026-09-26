# Changelog

## 0.4.2

- The Journal reads as a timeline: a chapter for each zone entered, the
  game's icons on a rule, and runs folded into one line (a hunt's kills,
  experience and loot; gear put on; a trainer's lessons and fee; skill-ups).
  Quests open in Quests, NPCs in Conversations, deaths in the Book of the
  Dead. The session's figures sit in a card with a way to the session
  before and after.

## 0.4.1

- Help: what Forever Memory is, how it records, and where to find what.
  It opens on the first start, and stays next to Settings.
- Every page and card that has nothing to show yet says what will appear
  there, with the game's own icons, instead of empty charts and tables.
- The House's emblem sits on the banner's cloth instead of running off it.

## 0.4.0

- The House: the account as a household, with a banner, a name you can
  change, its members, the mounts and companions it owns, the Legacy and a
  history of firsts, deaths, deeds, letters and notable kills.
- Standing: every faction's standing, when each was reached, and how it
  moved. Needs addon 0.4.0 (offered in Settings) and a login.
- Conversations under Quests: what each NPC said, replayed as a scene.
- Roleplay profiles from Total RP 3, MyRolePlay and XRP on the Players page.
- Empty pages say what will appear there, in the game's own look.
- Pages no longer carry a title of their own.

## 0.3.0

- Deeds: 14 for everyone and 6 per class, earned from what was recorded.
- Item stories in the Armory and Gold & loot tooltips.
- The Chronicle: the diary as a book with chapters and turning pages.
- Letters between a player's characters, in their own voices, about what
  they share: mounts and companions (recorded by addon 0.3.0), Legacy.
- The Book of the Dead: every death, its last ten seconds, and an epitaph.
- Fellowship and nemeses on the Players page.
- "Previously on…": a page for OBS that reads the last entry aloud before a
  stream, with a countdown; also `forever-memory previously <character>`.
- Every race and class combination has a personality to start from.
- Loading is 10 to 40 times faster: finished logs are parsed once and
  cached, and reloads only read what changed. Position samples from
  archived logs are no longer dropped when the game folder sorts first.

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
