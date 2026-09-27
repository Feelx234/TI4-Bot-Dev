# ONLINE-001 — online multiplayer variant of the replayer

Operator request 2026-09-27: "make a variant of the replayer that can be used for online multiplayer".
Decisions (operator, same day): host-authoritative; direct TCP + join code; native `join` client; new
branch `wp/online-multiplayer` in the shared checkout.

## What was built

- `crates/ti4-replayer/src/net/` — `protocol` (u32-prefixed zstd JSON, 16 MiB wire / 64 MiB decoded
  bounds), `host` (`NetHost`: accept, per-connection reader/writer, a pump that relays frames, the
  owner-only pending choice and table status), `client` (`NetClient`), `remote_gui` (join window),
  `redact` (per-seat view).
- Replayer window: port field + **Host online** in the top bar, join code (click to copy), remote
  seat list, **Stop hosting**; the choice panel shows "X is playing this seat online" instead of
  buttons for a remote seat.
- `ti4-replayer join [<host:port>] [--code C] [--seat S] [--name N]`.

## Rules

- Answers go through `Gate::submit`: the fingerprint check refuses stale/invented/duplicate answers
  from the network exactly as from the window. A submit is accepted only from the seat being asked.
- A remote player may claim an Auto seat (it becomes Manual) or resume their own seat with the
  resume token from `Welcome`. Seats the host has on Manual are the host's.
- Redaction (host side, before sending): `view_for`, plus every deck's cards → `?` (counts kept),
  `rng_seed` = 0, manifest seeds and host paths blanked, face-down promissory notes held by others
  renamed `?n`; other seats' decisions dropped; events/rolls/summaries scrubbed of every redacted id.

## Checks

| Command | Result |
|---|---|
| `cargo test -p ti4-replayer --lib` | 42 passed (8 new: protocol, redact, host) |
| `cargo test -p ti4-replayer --test online` | 1 passed — real game over loopback: wrong code refused, seat claim → Manual, taken seat refused, per-frame leak check against `private_ids`, pending only to owner, invented option refused without consuming, real option accepted |
| `cargo clippy -p ti4-replayer --all-targets` | only `too_many_lines` on `gui.rs::choice_panel`, which was already over the limit before this change (+1 line) |
| `cargo test -p ti4-replayer --test live` | `nested_asks_inside_one_engine_step_each_get_their_own_panel` fails (2 distinct vs 7). Nothing it runs was changed here; not re-run on a clean HEAD because no second checkout is allowed |

## Known limits

- Plain TCP: the code travels in the clear. Over the internet, use a VPN (Tailscale/WireGuard) or an
  SSH tunnel, or port-forward and accept that.
- The host's own window is still omniscient (every hand visible). Fair only if the host is not
  playing, or plays honestly.
- Scrubbing is by id. A label that spells out a private card's *name* in another seat's action
  summary (e.g. a transaction detail) is not caught.
- Option policy scores are shown to remote players, as they are to the host.
- The GUI windows were built but not driven by hand in this session.
