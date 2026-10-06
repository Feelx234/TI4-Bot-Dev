# Current execution state

Last verified for documentation maintenance: 2026-10-06. This is a resume point, not a fresh engine/training qualification report.

## Repository and ownership

- Checkout: D:/Projects/ti4-engine-rs; branch wp/base-factions. Inspected HEAD: 573baba32cde9b28e6dd78240b4aec2615405ce6. Main/origin/main were aligned before this archive task. Resolve current HEAD/status on resume; do not use this recorded hash as live state.
- Shared checkout is intentionally dirty. Unrelated engine/income, ML/policy/training, replayer/review/UI, scripts, and tools/pi_rpc_bridge.py changes remain owned by other sessions. Do not stage, format, reset, or fix them as part of documentation work. The pre-existing R02 index entry is preserved in the working tree and excluded from this task's commit.
- Ask peers/operator what is running; this task did not establish live process ownership and did not run Cargo or stop any process.

## Active workstreams and recorded evidence

- **Base and later factions:** [BF plan](BASE_FACTIONS_PLAN_2026-10-02.md), [October 6 Cabal queue](HANDOVER_2026-10-06_CABAL_PI.md), [Cabal evidence](evidence/BF-cabal.md). The latest top-level historical state records all twelve BF factions complete at cb2374fd; current commit history has since added Nomad/Cabal, original-six closure, and Empyrean. These recorded asset results do not close BF20–25 integration: runtime/default roster, versioned policy roster/schema migration, simulator rebaseline, manual UI, paired soak, and exit review remain separately gated.
- **Diplomacy/trade:** [unification](ENGINE_DIPLOMACY_UNIFICATION.md), [trade rework](TRADE_REWORK_2026-09-22.md), [operator bug plan](BUG_PLAN_2026-09-20.md). Historical adapter/journal, leader hooks, browser QA, and review obligations require individual disposition.
- **Policy/training:** [operator guide](PPO_TRAINING_HOWTO.md), [complete decision contract](STAGE2_COMPLETE_DECISION_CONTRACT.md), [OBS-012](OBS-012_DECISION_COMPLETENESS_QUALIFICATION.md), [activation rework](ACTIVATION_REWORK_PLAN_2026-09-17.md). Older pilot outcomes are measurements, not promotion/qualification; live trainers/checkpoints were not inspected in this task.
- **Tools:** [R01](R01_REVIEW_VIEWER.md) and [R02](R02_REPLAYER.md). R02 working-tree/index ownership is separate from this documentation commit.
- **Release:** [M11 bridge](M11_TTS_BRIDGE.md), [M12 qualification](M12_QUALIFICATION.md), [M13 cutover](M13_CUTOVER.md). Obligations and rollback criteria remain open until independently verified.

## Checks, limits, and review debt

- No Rust builds, tests, soaks, training, simulation rebaseline, or browser QA were rerun for archival. Consult package evidence for command/build provenance rather than treating historical counts as checks of this shared dirty tree.
- Historical full-workspace limitations include ti4-review semantic_golden (recorded operator exception) and ti4-mlp smoke_refusals failing before the pool check because local vocabulary generation is invalid. Do not silently convert these to fresh passes or general waivers.
- Preserve outstanding Tier-C/D review debt from BF shared timing/legality/hidden-information seams and M09/M10/OBS records unless later evidence explicitly resolves it. Historical diagnostic notes retain five event-share deviations and a pending attribution/rebaseline decision; archival makes no acceptance claim about them.
- The [archive manifest's active follow-up register](ARCHIVE_MANIFEST_2026-10-06.md#surviving-obligations-active-follow-up-register) retains residual fleet/capacity, exploration, leader, diplomacy, and historical simplification findings. Fleet-supply enforcement and Analytical/Rin review files remain active.
- October handovers remain available for source/evidence details but may contain older authority/status. Current AGENTS.md and later explicit operator decisions govern; verify any contradictory historical completion, branch, model, or process note before acting.

## Documentation task and next action

Adopted AGENTS.md and archived only the obsolete/superseded records enumerated in the [manifest](ARCHIVE_MANIFEST_2026-10-06.md). Moves use plain git mv, with no cleanup, artifact overwrite, or new archive subdirectories. Link repair and independent Tier-A diff review passed before the documentation commit; exact checks are recorded in the manifest.

Next: resume the relevant workstream only after checking current Git state, ownership, evidence, and the open gates above. Verify main/origin publication from Git before assuming this archive migration is published; this file does not claim an unverified push.

The [verbatim historical execution state](archive/EXECUTION_STATE_HISTORY_2026-10-06.txt) preserves all previous entries, including superseded/conflicting notes. It is plain text so original Markdown references remain literal historical paths. Current navigation is here, in [INDEX.md](INDEX.md), and in the manifest; dated history is not a live resume point.
