# Replay and save formats

Every file that a person can take out of the game, what it holds, and how to replay it. Since
format 2 of the replay export, a file replays by itself: no setting has to be guessed or supplied.

## The exports

| Export | Where it is made | Format | Replays with |
| --- | --- | --- | --- |
| Replay export of an online game | `GET /api/games/{id}/replay` (`ti4-server`, `http/games.rs::get_replay`, `replay_export.rs`); the web "Copy history" button (`web/src/protocol/client.ts::fetchReplay`, `presentation/replayCopy.ts`) | `ti4-replay`, `format_version` 2 | `replay_save` example, `ti4_host::export::ReplayExport::replay` |
| Replay export from the data directory | `server feedback-save ID` for a report of an online game (`replay_export::from_store`) | the same | the same |
| Offline save | `HostSession::save_state` (`ti4-wasm`); the offline game's "copy history" (`LocalTransport.fetchReplay`) and "export game" (`store.exportGame`) write it; the feedback button attaches it as `context.local.save` | `format: 2` (a number) | `replay_save` example, the `resume` of `ti4_host_start` |
| Server data directory | `history.json`, `init.json`, `lobby.json`, `player_sessions.json` of a game | not an export | `FileGameStore::recover_session` |

`ti4-replayer` (the egui recorder) and `ti4-cli` have their own session files and read none of these;
`ti4-review` reads a `history.json`. Nothing but `replay_save` and the server reads an export.

### Before format 2

The online export (`format: "ti4-replay"`, `version: 1`) held `game_id`, `seed`, `player_ids`,
`map_template` and `history` (decisions, redo, events, batches, `rng_marks`, counters). It lacked the
factions, the names, the card set, the speaker order, the starting-technology and Thunder's Edge
switches, who the bots were, the rules version and the build, so a plain replay failed and the setup
had to be guessed. It also went through `response.json()` and `JSON.stringify` in the page, which
rounds any integer above 2^53: the seed (`u64`) and the end of an open deck segment
(`rng_marks.decks[].until`, `usize::MAX`) came out as floats, so even a correct guess of the setup
could not reproduce the game.

The offline save (`format: 2`) already held the whole table (`game`: the setup or table with its
choices filled in, `table`, `scenario`), `seed`, `bot_version`, `rules_version`, `humans`, every
decision and the engine build. Its seeds stay below 2^53 (the launcher draws up to 2^52, a dev seed
must be a safe integer, and `web-shared/src/storage/validate.ts` refuses a larger `save.seed` on
import), so it survives `JSON.parse`. It now also lists `seats`.

## Replay export, format 2

```jsonc
{
  "format": "ti4-replay",
  "version": 2,              // the field of format 1, kept
  "format_version": 2,       // missing = 1
  "game_id": "game_...",
  "seed": "18432220952314264515",       // decimal string
  "player_ids": ["player_...", ...],     // seat order
  "map_template": "6pScpt2021Prelim",    // alias, null = seeded random board
  "setup": {                             // null/missing: the game was not made from a lobby (tests)
    "table": {                           // ti4_host::setup::TableSetup
      "player_count": 6, "map": {"kind": "template", "alias": "..."},
      "strategy_cards": "te", "speaker_order": "seat" | "random",
      "starting_tech": true, "thunders_edge": true, "seed": "..." // string
      // "start_from" (scenario) and "preset" when the table had them
    },
    "factions": ["mentak", ...],         // the faction each seat drew, seat order
    "initial_state_hash": "f14fd5337a3c6a18"  // hash of the opening state the setup must rebuild
  },
  "seats": [{"seat": 0, "player_id": "...", "nickname": "Ada", "kind": "human|smart|random|...",
             "faction": "mentak", "rule_bot_version": 5}],
  "map_layout": {"version": 1, "active_sources": [...], "placements": [{"system_id","q","r"}]},
  "build": {"build_commit": "...", "engine_version": "0.1.0", "content_digest": "...",
            "rules_version": 16, "rule_bot_version": 5},   // + "wasm_sha256" for a page-made file
  "check": {"decisions": 620, "decisions_hash": "f83899cab19ab910"},
  "history": {                           // the same as history.json
    "decisions": [DecisionRecord...], "redo": [...], "events": [...], "redo_events": [...],
    "event_counter": 0, "revision": 0, "generation": 0, "batches": [...],
    "rng_marks": {"331": {"dice": "11"}, "decks": [{"from": "863", "until": "18446744073709551615", "reserved": []}]}
  }
}
```

* `setup` is the table the lobby showed; it is not secret. `initial_state_hash` is a hash only.
* Colours are not in the game (the page derives them from the seat), so they are not exported.
* `rule_bot_version` is the version of the exporting build. The journal holds every bot decision,
  so a replay never runs a bot; the field only says what to use to continue the game.
* The page cannot know the wasm hash of a server game; `wasm_sha256` is only written by a file made
  in the browser (the feedback `context.local.wasm_sha256` has it for offline games).

### Big integers

Integers a JavaScript `Number` cannot hold (`seed`, every `rng_marks` position and segment bound,
`setup.table.seed`) are decimal strings. Readers (`ti4_host::wire`) accept a number or a string, and
also a float that a browser rounded: 2^64 reads as `u64::MAX`, which is what `usize::MAX` became, so
an old file's open segment is open again. The seed of an old file reads as the number that is
written, which is already rounded if the file went through a browser; such a file cannot be
replayed exactly, and nothing can fix that afterwards. TypeScript never parses the export: the
page saves the server's text as it is (`fetchReplay` returns `response.text()`). Stored records
(`history.json`, the checksums) are unchanged; only the export is written with strings.

### Compatibility

* A file without `format_version` is format 1: it loads (`ReplayExport::parse`), but has no `setup`,
  so `replay()` says "no setup" instead of guessing. `replay_from` replays it from an opening state
  the caller supplies.
* A file with a newer `format_version` than the build knows is refused with a message.
* Events and batches are kept as JSON: a file made by another build may carry fields this build does
  not know.
* The stored records, decision hashes, checksums and engine are unchanged.

### Replaying

`cargo run -p ti4-wasm --example replay_save -- FILE` reads the export or an offline save (or a stored
report). For an export it rebuilds the opening state from `setup` and the player ids, checks it
against `initial_state_hash`, plays every journaled decision through the engine (with the marks of a
turn redo), then goes on for as long as the engine needs no answer (the end of a phase, the end of
the game) and prints the round, phase, state hash and whether the decision count and hash equal
`check`. Exit code 1 when they do not.

### What the host answers by itself

Nothing that matters is left out of the journal. The server journals every decision of every seat
(bots, declined reaction windows, the starting-technology picks) in `history.decisions`. What the
engine settles by itself is deterministic and is not a decision: single-option steps
(`auto_resolved` notes, which leave the journal alone), the steps between questions, the end of a
phase and of the game.
A replay that stops at the last journaled decision shows the state right after that answer, not the
state the live game shows (a finished game is still in the action phase); `replay` therefore keeps
stepping until the engine asks for an answer or the game ends
(`replay_session_to_next_question`). Test: a full bot game and a game in progress, replayed from the
exported file alone, end in the same state hash as the live session
(`crates/ti4-server/tests/it/replay_export.rs`).

### Secrecy

The export has what it always had (seat holders only; every seat's decisions and the seed) plus the
setup, which is public at the table. The decisions and the seed reveal every hand and deck (the seed
shuffles the decks); that is an existing property of the endpoint and is not widened. The branch
`redact-hidden-decks` conceals deck order and the seed in the state sent to clients; it does not
touch this endpoint. If the seed is ever withheld from the export, the setup stays valid but a
replay then needs the seed from elsewhere.

## Offline save

`format: 2` as before (`SAVE_FORMAT`), plus `seats` (id, name, who plays it, faction), for a reader
of the file. `game` is what a replay is made from. Integers are numbers (seeds below 2^53); the
readers (`Setup.seed`, `Local.seed`, `TableSetup.seed`, `replay_save`) also accept strings.
