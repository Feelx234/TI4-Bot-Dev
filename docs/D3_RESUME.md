# D3 (typed rule bot): state after D3-0 and D3-1

Branch `core-d3-0`, cut from `core-d2-6a` @ `5557c60a`. Not pushed, not merged. Plan: `docs/todo/D3-bots.md` (branch `core-todo-d3`). Bail rows `D3-bail-*`: `docs/ENGINE_KNOWN_ISSUES.md`.

## Decisions of the user applied

1. Cherry-pick of the live branch's `ti4-rule-bot` changes: **nothing to pick.** `git merge-base core-d2-6a nightly-fixes-2026-10-26` is `93d1b266`, and `git diff 93d1b266 nightly-fixes-2026-10-26 -- crates/ti4-rule-bot` is empty (no live commit since the merge base touched the bot). The 18-file difference the plan measures (`core-d2-6a` vs live: +318 -727) is the integration line's own change (typed card fields, `Arc<str>` ids); picking those 18 files "from live" would revert it. So the bot files stay as they are.
2. The typed bot has its own crate `crates/ti4-rule-bot-typed` (workspace member); `ti4-rule-bot`, `ti4-engine` and `ti4-engine-core` hold no typed bot code. Today the crate only has the self-check entry `choose_typed_explained` (= the string answer).

## Done

* D3-0: `tools/rulebot/d3/{census.py,census.txt,subtypes.tsv,test_census.py,make_base.py}`; `ti4_model::count` + feature `observed-count` (67 `Observed`/`SeatObservation` methods and `PlayerId/PlanetId/SystemId::new` count per decision subtype); `ti4-engine-core-bridge` module `shadow` + bin `bot_shadow` (self-check, mutants `pick|smart|rng`, `--bot`, `--shards`, `--spec`, `--min-coverage`, `--seen-parity`); tests `tests/bot_shadow.rs`.
* D3-1: `ti4_engine_core::seen` (`Seen`, layer 5), loader `ti4_engine::seen_load::SeenOwned`, parity `ti4-engine-core-bridge::seen_parity` (run by `bot_shadow --seen-parity N`), perturbation test, order tests `tests/seen_order.rs`, compile_fail doctests, `to_model_system`.

## Not done

* **Baseline record (D3-0 item 4).** The single heavy run waits for `/root/.ti4-heavy.lock` (other gates were queued first). Script `/root/deploytmp/core-d3-0/baseline.sh` (nohup, blocking `flock`) builds the base commit `5557c60a` (scratch worktree `/root/deploytmp/core-d3-0-base`, profile `gate`), saves `/root/deploytmp/core-d3/base-selfplay` and plays the golden set with `--verify tools/rulebot/baseline.json`; outputs in `/root/deploytmp/core-d3-0/baseline-out/` (`status`, `bench.txt`, `bench.json`, `time`). Then:
  `python3 -I tools/rulebot/d3/make_base.py /root/deploytmp/core-d3-0/baseline-out/bench.json /root/deploytmp/core-d3-0/baseline-out/time /root/deploytmp/core-d3-0/counts.json 5557c60a > tools/rulebot/d3/base.json` and commit it. (`counts.json` = `bot_shadow --features observed-count --json` on games 0,5,11.)
* **D3-2 and later: blocked on a prerequisite.** D3-2 needs the typed question of the core at every decider call (`Game::core_choice()`). After D1b (merged in this line) there is none: the 14 `Typed::new*` sites are dual-window replays after the string window finished, `Session::step` runs only inside `driver=dual` as a comparison, and `Game` keeps no typed `Choice` (`legal_options` returns the string one). The live typed stream is D2's flipped `Game`. D3-3..D3-8 cannot be verified without it (the shadow would skip every call as `NoTyped`).

## Resume

* Parity: `cargo build -p ti4-engine-core-bridge --bin bot_shadow && target/debug/bot_shadow --config tools/engine_profile/profile-20.json --games 0,5,11 --seen-parity 3 --strict` (3 games, stride 3: 1 633 positions, 806 529 accessor answers, 0 differences; about 1 minute in a debug build).
* Tests: `cargo test -p ti4-engine-core-bridge --test bot_shadow --test seen_order`, `cargo test -p ti4-rule-bot-typed`, `cargo test -p ti4-engine-core --lib seen::`, `cargo test --doc -p ti4-engine-core seen`, `python3 -I tools/rulebot/d3/test_census.py`, `tools/engine_core/lint_core.sh`, `python3 -I tools/engine_core/check_layers.py core`.
* Next package when D2 delivers a typed question per decider call: D3-2 (`Typed::choice: Option`, `Game::core_choice`, `Q`, `Verdict`, router stubs in `ti4-rule-bot-typed`, `TypedSource` in `shadow.rs` already exists).
