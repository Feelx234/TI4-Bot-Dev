# BF-YIN-HERO-TIMING (in progress)

Objective: resolve Dannel's committed infantry through actual ground combat timing, all rival owners, without bombardment or space-cannon defence.
Dependencies: combat-origin wave; existing Yin landing/selection path. Tier C. Permission P1: factions/mod.rs, leaders.rs, invasion.rs, game.rs isolated leader-call hunk, factions/yin.rs, evidence/state. No external mutation or deletions.
Normative source: yinhero printed corpus and LRR ground-combat/leader timing. Existing six keep the original leader dispatcher unless a registered timed module claims the leader. Status gating and purge/exhaust are centralized.
Root provides timed leader hook and eventful already-committed ground combat helper; Luna implements bounded Yin consumer/tests. Validation and independent review pending. No asset claim yet.
