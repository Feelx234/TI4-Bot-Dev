# B9 agenda: resume note (branch core-b9-agenda, worktree .claude/worktrees/core-b9)

State: core-a4-objectives (ff), core-reactions and core-integration are merged in (conflicts resolved:
Custodians `lift` rides `fx::adjust_victory_points`; Deadly Plot keeps its agenda dual window; `Asked.nested: false`
in vote_dual; dual.rs keeps both `note_window`/`local_windows` and `FOCUS`; render uninhabited list drops objectives,
explore, relic, cards). `cargo check -p ti4-engine --all-targets` is clean.

Done (uncommitted-until-WIP-commit, NOT yet run): `rules/agenda/table_effects.rs` (core, `Table`/`EffectFacts`),
`Resolve::of_facts`, `dual_window::agenda_effect` takes `EffectArgs` (voters, sources), `windowed` takes `sources`,
18 more `resolve_with` arms are wrapped in `windowed`: conscription, standardization, cladenstine, regulations,
economic_equality, articles_war, execution, conventions, unconventional (Against), schematics, shared_research,
wormhole_recon, arms_reduction, rearmament (Against), disarmament, plowshares (For), core_mining, demilitarized_zone.
Lockstep tests: `crates/ti4-engine/src/agenda_effects_lockstep.rs` (5 tests, never run yet).

Next step:
  export CARGO_TARGET_DIR=/root/deploytmp/core-b9m2 CARGO_BUILD_JOBS=3
  cargo test -p ti4-engine --lib agenda_effects::lockstep
Fix differences/bails (likely: tech/unit ids in the test, Marks/Meta footprint refusals, staged event order), then
mutation-check each `Table::apply` arm (flip a constant, expect the lockstep test to fail), one bail test each
(Sanctions/Artifact dice, Rearm For, Plowshares Against, Unconventional For, Antiquities, Grant Reallocation, Crisis,
Travel Ban/Wormhole Research galaxy, galvanized ground force, Action phase, deck reserve).

Still to do: incentive (reveal_stage, bail on deck_reserve), questions (seed_empire tie, redistribution, defense_act:
new Ask/Opt/Subtype in choice/agenda.rs + render/agenda.rs byte-exact), galaxy effects (Travel Ban, Wormhole Research
via Sources.galaxy + Slice::Map), reveal/resolution Stage windows with dual_window!, laws::offer_discard,
issue lines in core-integration docs/ENGINE_KNOWN_ISSUES.md (own hunk only).

Gate (only at the end, one cargo run at a time, delete the target dir afterwards):
  CARGO_TARGET_DIR=/root/deploytmp/core-b9m2 CARGO_BUILD_JOBS=3 SKIP_AB=1 SKIP_WASM=1 \
    tools/engine_core/verify_package.sh agenda <out_dir> <base_selfplay> 
  (profile gate; needs bench + bench-v1..v5 200/200 on the final commit, golden dual driver=dual 0 differences,
  board_equivalence 0 mismatches, clippy clean). Not run yet.
