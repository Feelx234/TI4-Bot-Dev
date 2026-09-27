# Dev scenario crash recovery

## Goal

A scenario launched from `/dev/scenarios` should remain playable at its original `/games/{game_id}` URL after the server restarts, with the same player credentials, current decision, history, and bot behavior. Preserve existing recovery behavior for ordinary games. This plan is for *future launches*; the existing `data/games/dev_combat_b7078f12de2eb695/` cannot be reconstructed from its lone `player_sessions.json`.

## Diagnosis

- `crates/ti4-server/src/dev/scenarios.rs` builds a `SessionConfig` without a store and calls `GameRegistry::launch_dev_scenario`.
- `launch_dev_scenario` in `crates/ti4-server/src/session/registry.rs` starts and registers an in-memory session, but writes neither `lobby.json` nor `init.json` and does not attach the registry's `FileGameStore` to the session. Some later credential takeovers can write `player_sessions.json` on their own.
- Startup only considers directories with `init.json` or `lobby.json` (`storage.rs::list_saved_games` / `list_saved_lobbies`). A credentials-only directory is skipped. The lobby endpoint returns 404; `web/src/App.tsx` reduces the failed request to “Unable to load lobby.”
- Existing player-session recovery (`storage.rs::recover_player_session`) reconstructs every seat as `Human`. That would silently change the bots in normal dev scenarios even if their init and lobby were saved. The four-view scenarios deliberately use all-human seats.

## Implementation

1. **Persist an entire dev launch before publishing it.** In `GameRegistry::launch_dev_scenario`, when `self.store` exists, attach that store to `SessionConfig`, then save the versioned `PlayerLobbyRecord`, `PlayerSessionsRecord`, and `PlayerGameInitRecord` needed by the existing player-lobby recovery branch. Derive all records from the *final* scenario configuration (including the four-view mutations), not from the base setup. Use the exact launch roster, map tiles, seed, initial state, and credentials. Only start/register the session and return its URL/token once all required writes succeed. Keep the no-store registry path working for existing in-memory tests.
2. **Make bot controllers recoverable.** Add an optional, validated seat-controller map to the immutable player init record, with a backwards-compatible default of all-human for existing records; use it in `recover_player_session` when reconstructing `GameInitRecord`. Check that the map covers exactly the player IDs and does not contain credentials. Update every constructor/fixture for the init record. Confirm format/version and checksum handling remain compatible with previously valid saves.
3. **Define failure and retry semantics.** The three launch files are separate atomic writes, not one transaction. If any write fails, do not advertise a playable in-memory game; clean up only artifacts *created by that failed launch* or ensure a subsequent startup reports the incomplete launch clearly and does not publish a phantom lobby. Never delete an existing game's records on a generated-ID collision. Ensure a retry generates a fresh game ID and cannot mistake a stale directory for an empty destination. Document the ordering and failure points alongside the launch code.
4. **Check scripted scenario startup.** `ongoing_combat` and invasion presets advance the engine through scripted decisions after launch. With the store attached, these decisions must be durably logged using the same path as player/bot decisions before the launch response succeeds. If scripting fails, return an error and leave the save recoverable at the last committed decision (or explicitly clean up a new, unadvertised save); do not return a URL to a partially initialized scenario without documenting it.

## Verification / acceptance

- Add a temp-directory integration test in `crates/ti4-server/tests/dev_scenarios.rs` (or a dedicated recovery test): launch `space_combat` using `GameRegistry::with_store`, accept at least one human decision and allow a bot response, drop the registry/session, create a new registry over the same directory, and call `recover_all_games_report`. Assert the ID is recovered, public lobby and original player session work, state/decision hashes and pending choice match, bots retain their controllers, and play continues with new decisions persisted after the restart.
- Cover a scripted `ongoing_combat` preset and an all-human four-view preset, checking replay of their initial scripted choices and correct seat controllers. Test an ordinary player lobby as a compatibility case.
- Inject a failed launch write and check no partial game becomes playable or advertised after restart; retain the records of unrelated games. Check both old player init records (missing the new controller field) and newly written records.
- Run `cargo fmt --all --check` and the relevant `ti4-server` recovery/dev-scenario tests. If the UI message changes, run the relevant web tests and `npm run build` from `web/`.

## User-facing result

Once this is implemented, restart the server with the same `TI4_DATA_DIR` (or `--data-dir`), reopen the original `/games/{game_id}` URL in the browser tab that holds its `sessionStorage` player token, and continue. A new tab without that token can view the lobby and use the supported takeover flow where available.
