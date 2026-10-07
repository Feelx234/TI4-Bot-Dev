# BF-NAAZ-COMBAT-COMPLETION (in progress)

Objective: complete Maximum participation, round repair and actual unit-ability immunity before claiming its printed card.
Dependencies: cadbbe2d and 920c852c. Tier C; independent review remains required.
Permission: P1, edits combat.rs, invasion.rs, factions/naaz.rs, this evidence and execution state. No external mutation/deletion. Focused builds use one worker and debug info disabled.
Normative sources: committed Thunder's Edge unit/breakthrough corpus; LRR 2.0 sections 1.26 and 87.4a. Ambush, Devotion and Impulse Core are faction/technology effects rather than named unit abilities. Harrow invokes bombardment. Corpus clarification requires a supporting fleet for a planet-standing Maximum; this package preserves that restriction and removes cargo-only drift.

Review of inherited commits: sustain filtering is directionally correct under 87.4a. Ground bombardment/space-cannon defence lacked immunity; planetary round repair and participation predicates diverged. The effect-placement gate and captured-form normalization already exist in 920c852c, contradicting the inherited evidence. They still need call-site coverage verification. The recorded six-original 30-seed metrics are diagnostic evidence, not proof of every decision boundary.

Changes: one shared planetary participation predicate; repair participating planetary Maximum at space round start; filter ground unit-ability hits before sustain/casualty for bombardment, space-cannon defence and synchronous Harrow.
Validation so far: planetary_maximum filter: 2 passed, 0 failed (2026-10-04), low-memory test profile, shared working checkout. Further ground-path and crate tests pending. Ledger remains unclaimed.
