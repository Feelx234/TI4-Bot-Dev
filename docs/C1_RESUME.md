# C1: the bottom crate `ti4-chance`

Lane C1 of docs/FORK_RNG_THEORY.md (branch `core-theory-fork-rng`, section 4.1). Branch `core-chance-1`, started from
`core-heavyfix-1` (b7ac383a9). No other crate depends on the new crate yet (that is lane C2), so neither the engine nor
the wasm build changes.

## API (`crates/ti4-chance`)

| Item | What |
|---|---|
| `Domain` | the 22 literal stream domains plus `Dyn(u16, u32)`, the same variants **in the same order** as the core's `state::RngDomain` (the order is the stream table's sort order and the order of `positions`). `text()` gives today's literal names, `of_text`, `LITERALS` |
| `derive_seed(seed, text)` | `SHA-256(seed_le \|\| text)`, today's stream key |
| `Request { domain, kind, owner: Option<u8>, purpose: u32 }` | one roll group; `Request::new`, `.owned(owner, purpose)`, `len`, `sides`, `accepts(&Faces)` (count and range: the legality check C3 needs), `dice_groups(domain, n, sides, hits_on)` (split into groups of at most 15, in order) |
| `Kind` | `Dice { n, sides, hits_on }`, `Index { n }` (one face in `1..=n`: Kyver, Neuraloop), `DieMod { sides, n, form }` (Spy, Sanctions) |
| `ModForm` | `FaceMinusOne` (Spy: `(face - 1) % n`) and `Face` (Sanctions: `face % n`); `index(face, n)` |
| `Faces` | 16 bytes (`len` + 15 slots), `from_slice`, `try_push`, `as_slice`, `hits(on)`; unused slots stay 0 |
| `Streams` | seed + `Arc<Vec<(Domain, ChaCha8Rng)>>` sorted by domain, copy-on-write like the core's `Cow`. `new`, `seed`, `die` / `try_die` (today's `die(domain, sides: u32)`), `shuffle` (Fisher-Yates from the back), `index(domain, n)` (`random_range(0..n)`, the setup's speaker form), `positions`, `position(domain)`, `set_positions`, `shares_streams_with`, `reseed(seed)`, `reseed_domain(domain, seed)` |
| `answer(&Request, &Streams) -> (Faces, Streams)` | pure; `answer_mut(&Request, &mut Streams) -> Faces` (the hot path, for the C3 function pointer); `try_answer_mut -> Result<Faces, Refused>` |
| `Refused` | `Dyn(domain)`, `TooMany(n)` (> 15), `NoSides`; nothing is drawn on a refusal |
| feature `search` (off by default) | `search::sample(req, rng)`, `element_weights` (Index uniform, DieMod with its bias), `hit_weights` (binomial per hit count), `faces_for_hits(req, k)` (a representative answer with k hits), `outcomes(req, limit)` (every ordered answer and its weight, if at most `limit`) |

Dependencies: `rand` 0.9, `rand_chacha` 0.9, `rand_core` 0.9 (all `default-features = false`, as in the core) and `sha2`
0.10 (`default-features = false`). No `ti4-*` crate. No `unsafe` (the workspace forbids it), no clock, no threads, no
OS entropy. `cargo build -p ti4-chance --target wasm32-unknown-unknown` builds.

## Deviations from the doc sketch

* **`Kind::DieMod` has a third field `form: ModForm`.** The two sites use different formulas today (Spy `(face-1) % held`,
  Sanctions `face % held`, doc #16 vs #22), so one `DieMod { sides, n }` could not describe both. `Faces` holds the **raw**
  face (it is what the ledger records); `ModForm::index` maps it.
* **`Streams` stores `Arc<Vec<Stream>>`**, the same as the core's `Cow<Vec<Stream>>` (which is an `Arc` with
  `make_mut`); the crate cannot name the core's `Cow`.
* **`Dyn` domains**: a `Dyn` stream is carried at its position (with a placeholder key: word positions do not depend on
  the key) and every draw from it is refused (doc Q4, recommended "yes"). Today a `Dyn` stream *could* draw through the
  thin crate's `TextSeeds`; no production rule does (all 22 production domains are literals). If C2 finds a caller that
  needs it, a keyed install is a small addition.
* **Extra API** beyond the sketch: `die`/`try_die` (C2 needs today's `die` until C3 removes the draw sites), `index`
  (the setup's `seeded_speaker_index` draws `random_range(0..seats)`, which is not a die), `position(domain)`,
  `Request::accepts`, `Faces::hits`, `reseed_domain` next to `reseed`.
* `Index { n }` and the dice `sides` are `u8` (faces are bytes). Today's `die` takes a `u32`; `Streams::die` keeps the
  `u32` so wide dice still work for C2; requests are limited to 255 sides/elements (Neuraloop's pool is about 60).
* `rand` 0.9 turned out to widen a `u8` range to `u32` internally (tried: a `u8` sampler drew the same faces), but the code
  keeps today's integer types (`u32` die, `usize` shuffle/index) so the identity does not rest on that.

## How bit-identity is proven

All against the **live old implementation**, not recorded vectors: `ti4-chance` has dev-dependencies on
`ti4-engine-core` (the old `state::Rng`, `RngDomain`) and `ti4-engine-core-setup` (`rng::Literals`, the production seed
source of the setup; `domain_text`; `derive_seed`; `seeded_speaker_index`). Neither depends on `ti4-chance`, so there is
no cycle, and `check_production_graph.sh` is clean in strict mode with `ti4-chance` listed as a production crate (dev
dependencies may not reach legacy or the dev tools; they do not). Nothing was copied as golden vectors.

`crates/ti4-chance/tests/bit_identity.rs`:

* `domains_texts_seeds_and_order_are_todays`: each of the 22 literals maps (exhaustive `match`, so a new variant on
  either side fails to compile) to the old one with the same text and the same `derive_seed`; the `Ord` of every pair
  (incl. `Dyn`) agrees, so stream tables and positions are in the same order.
* `dice_of_every_domain_match_the_core_rng`: 200 seeds x 400 dice, random domain from all 22, sides 1..=255 plus wide
  `u32` dice and `u32::MAX`; every face and the final positions equal.
* `answers_of_every_kind_match_one_core_die_per_face`: 150 seeds x 300 requests of all three kinds, n 0..=15, every
  domain: `answer` and `answer_mut` agree with each other and with one old `die` per face; positions equal.
* `spy_and_sanctions_die_mod_is_todays_biased_pick`: both formulas, held 1..=12, 300 seeds; raw face and picked index
  equal today's.
* `setup_deck_shuffles_match`: the setup's shuffle sequence (objectives twice on one stream, exploration x4, relics,
  agendas, action cards, secrets) with deck-sized lengths incl. 0 and 1, 200 seeds; permutations and positions equal.
* `seeded_speaker_index_matches`: 500 seeds x 1..=8 seats.
* `set_positions_matches_the_core`: positions taken from the old generator, partly dropped and moved, imposed on a
  `Streams` with a different history; the next draws equal.
* `clones_share_until_the_first_draw_like_the_core`.

Unit tests in the crate: every `sides` in 1..=255 x n in 1..=15 x 48 seeds against a bare `ChaCha8Rng::random_range`
(doc 4.1), refusals draw nothing, `Dyn` positions are carried, `Faces` is 16 bytes, group splitting, `accepts`, the
`ModForm` formulas, and (feature `search`) `sample` equals `answer_mut` on the same generator, weights sum to one, the
Spy bias `[0.4, 0.3, 0.3]` for 3 cards.

`crates/ti4-chance/tests/properties.rs`:

* positions: an answer of n dice moves its stream exactly as far as n single dice (a die is not a fixed number of words:
  the sampler can reject and redraw, so the property is stated against single draws), at least one word per die,
  n = 0 moves nothing, other streams do not move;
* `answer` is deterministic and pure (input untouched and still shared);
* a group of 0..=64 dice split with `dice_groups` equals drawing one by one;
* `reseed` equals a fresh game seed, a reseeded copy draws differently from the real future; `reseed_domain` changes only
  that domain (and equals that domain under the other seed); the 22 domains of one seed are pairwise different.

The old thin crate's `GameRng` was not used as a reference (it would pull the whole thin crate into the dev build); the
core's `Rng` is documented as, and dual-tested to be, bit-identical to it.

## Repo checks wired in

* workspace `members` and `[workspace.dependencies] ti4-chance`; `Cargo.lock` only gains the new package.
* `tools/engine_core/check_layers.py chance [CRATE]`: `Cargo.toml` names no `ti4-` crate outside `[dev-dependencies]`
  (also `package =` renames and target sections), and the sources (test code stripped) name no `ti4_` crate and no
  `std::time`, `std::thread`, `getrandom`, `thread_rng`, `rand::rng(`, `from_os_rng`, `from_entropy`, `unsafe`.
  Negative control run on a scratch copy (a `ti4-model` dependency and a `std::time` use): both caught, exit 1; a
  `std::thread` inside `#[cfg(test)] mod` ignored. Mode `core` unchanged (576 files, 0 violations).
* `tools/engine_core/check_production_graph.sh`: `ti4-chance` added to the default `PROD_CRATES`.
* `tools/engine_core/clippy_core.sh`: `-p ti4-chance` and its files count as core files (any warning fails).
* Not done (lane C2, which wires the crate in): the production aliases of `.cargo/config.toml` (`c-web`, `l-web`,
  `t-web`), the resolver-crate rules of mode `chance`, and the no-draw rule of mode `core`.

## wasm impact

None yet: no crate depends on `ti4-chance`, so `ti4-wasm` links nothing new. When C2 wires it in, it *replaces* the
core's `state/rng.rs` and the setup's seeding (same `rand`/`rand_chacha`/`sha2` code); the `search` module stays out
unless the feature is enabled, and `ti4-wasm` must not enable it.

## Checks run

`CARGO_BUILD_JOBS=2`, `CARGO_TARGET_DIR=/root/deploytmp/c1`.

| Check | Result |
|---|---|
| `cargo test -p ti4-chance` and `--all-features` | lib 7 / 9 (with `search`), bit_identity 8, properties 5: all pass (under 1 s) |
| `cargo clippy -p ti4-chance --all-targets --all-features` | no warnings |
| `cargo build -p ti4-chance --target wasm32-unknown-unknown` | builds |
| `tools/engine_core/clippy_core.sh` (now including ti4-chance) | ok (cargo rc 0, engine warnings 41, baseline 41) |
| `check_layers.py chance` / `core` / `setup` | 0 / 0 (576 files) / 0 violations; negative control of `chance` fails as it must |
| `check_production_graph.sh` (strict) and `--self-test` | 0 violations, 0 failed controls; self-test ok |
| `native_play --quick --twice` (gate) | fingerprint `538dc68f23dddcc9`, deterministic, ZERO GAP (unchanged) |
| `perf_kernel suite 1` (gate) | `suite_hash 775f4136e60a99d5` (unchanged) |

Not run (by brief): `end_of_d1.sh`, dual, soak, bench, the mutation ratchet, the wasm size gate (nothing changed for wasm).
