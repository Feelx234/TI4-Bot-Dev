# Prompt for Codex — base factions, work split (2026-10-03)

You and Claude Code are both working on branch `wp/base-factions` in the same checkout
(`D:\Projects\ti4-engine-rs`). The operator has split the work so you never edit the same files.

**Read first:** `plans/EXECUTION_STATE.md` (section "Base-faction completion (BF)", bullet "Work split"),
`plans/HANDOVER_2026-10-03_BASE_FACTIONS_CODEX.md`, `plans/BF_AGENT_GUIDE.md`.

## You own (only these)

1. **Seated soaks.** Plan row 11 of the definition of done: a deterministic 200-game soak per new
   faction (arborec, argent, ghost, mentak, muaat, naalu, naaz, saar, sardakk, winnu, yin, yssaril)
   with that faction seated against in-scope factions: zero engine errors, replay-identical, no
   stuck games. Build the harness in **new files only** (e.g. a new example or integration test under
   `crates/ti4-sim/` or `crates/ti4-engine/tests/`), seating via `ti4_engine::fixtures::seated_game`
   or the seating API — do NOT widen `seating::IN_SCOPE_FACTIONS` (operator approval required).
   Report failures with seed, faction and the first bad decision; do not fix engine code yourself —
   append a precise request to `plans/evidence/BF-CODEX-REQUESTS.md`.
2. **Re-verification of finished factions:** ghost, sardakk, arborec (tests, ledger, live paths).
   Findings → `plans/evidence/BF-CODEX-REQUESTS.md`.
3. **Read-only reviews** of Claude's new commits on the branch, findings appended to
   `plans/evidence/BF-CODEX-REQUESTS.md`.
4. **Evidence/ledger reconciliation** in `plans/evidence/BF-CODEX-*.md` (your own files).

## Claude owns — do not edit

- Every shared engine file: `crates/ti4-engine/src/*.rs` (anything not under
  `src/factions/<alias>.rs`), `src/factions/mod.rs`, `src/factions/hooks_*.rs`,
  `crates/ti4-engine/tests/decision_delivery_inventory.rs`, `crates/ti4-model/**`, `crates/ti4-content/**`.
- Faction files: `src/factions/{mentak,muaat,naaz,saar,argent,naalu,winnu,yin,yssaril}.rs` and
  their `plans/evidence/BF-<alias>.md`.
- `plans/EXECUTION_STATE.md` BF section (append a dated line only if you must record a result).

## Rules for both

- Commit only your own paths (`git add <explicit paths>`); never `git add -A`, never stage another
  session's work, never touch the uncommitted `game.rs` income experiment (`round_income`,
  `STATUS_INCOME_TRADE_GOODS`).
- No new folders outside the repo, no new worktrees; reuse an existing idle worktree only for
  detached validation, as you have been.
- Never permanently delete anything; no `cargo clean`.
- The ti4-sim authored-bot behaviour baseline is retired and is not a gate.
- If you find a bug in a Claude-owned file, write the request; Claude will fix it.
