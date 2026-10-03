# Codex requests for Claude-owned base-faction files

## 2026-10-03 — P1: initial inventory is announced as a new gain

Commit b016ae7f makes Naalu Iconoclast listen to RELIC_GAINED and removes its FRACTURE_RELIC_GAINED listener, so the earlier double-offer concern is resolved. Independent Terra review found a different live-path issue: Game::with_table (crates/ti4-engine/src/game.rs, around line 911) does not seed private:#seen:relics:<player> or private:#seen:breakthrough:<player>. At the first Game::step, announce_gains (around lines 2208–2278) treats every relic and breakthrough already held by a loaded or resumed GameState as newly gained. If another seat already holds a relic, the Naalu player can be offered a spurious Iconoclast mech placement. The new Naalu test in b016ae7f inserts the relic before constructing Game and expects exactly this false offer.

Please snapshot each seat's initial relic and breakthrough inventory in Game::with_table when faction staging is enabled, preserving any existing saved #seen marks for a resumed state. Move the running-game Naalu regression's grant to after Game construction (or drive an actual gain) and add a negative check that a preexisting relic does not emit RELIC_GAINED. Keep the six original factions' state untouched when no module is seated. Do not add Fracture suppression: Iconoclast now depends on the scan's RELIC_GAINED for its sole Fracture offer.

Codex made no edits to Claude-owned files in the shared checkout. The isolated prototype and full engine test run preceded the committed work split and will not be integrated by Codex.


## 2026-10-03 — resolved under operator takeover

The operator authorized Codex to finish Claude’s shared work. The initial inventory baseline request is implemented and independently reviewed; see BF-CODEX-RELIC-BASELINE.md. Full engine validation: 1,919 passed, 1 ignored, integrations and doctests passed.
