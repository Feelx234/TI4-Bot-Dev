# Inherited base-faction commit review, 2026-10-04

Reviewer: root Codex, separate from Claude's implementation of cadbbe2d and 920c852c. This is findings on inherited commits, not independent acceptance of root's subsequent shared-engine changes. Subsequent WIP needs its own required review.

## cadbbe2d: sustain and Eidolon Maximum

The filter direction is correct. SUSTAIN DAMAGE cancels a hit before assignment, but LRR 87.4a requires that the hit could be assigned to that unit. Immunity therefore removes both casualty eligibility and the offer to sustain that hit. Describing sustain as part of assignment is imprecise; assignability is the prerequisite for this separate cancellation. LRR 15.2a is specifically bombardment and should not justify every immune hit.

The original evidence's producer taxonomy is incorrect: Mentak Ambush and Yin Devotion are faction abilities, Impulse Core is technology. They are not unit abilities merely because they produce hits outside combat rolls. Bombardment, space cannon offense/defense, anti-fighter barrage (including Waylay), and Sardakk's mech ability are unit-ability origins. Saar's breakthrough is a breakthrough effect. The new WIP labels these actual producers accordingly.

The inherited report also misidentifies missing placement/release scope: effect placement gating and captured-form normalization already exist in 920c852c. Subsequent real-combat fixtures cover planetary Maximum participation, repair, retreat and unit-ability immunity. A new priority regression found that non-fighter preference resurrected immune Maximum candidates after filtering; root reproduced and fixed it by intersecting forced non-fighters with the already eligible pool.

The ledger contract in factions/mod.rs and the BF plan is complete printed-card behavior with tests, not merely representable assets. Keeping Voltron and Absolute Synergy unclaimed pending coverage/review was conservative and appropriate; coverage should be reconciled against actual code rather than the inaccurate inherited gap list.

## 920c852c: inherited wave

The SHIP_MOVED path payload belongs to Muaat's breakthrough, whose listener reads the traversed path. A real Game movement fixture now traverses Avernus and validates this route. Excluding unrelated round-income changes from game.rs is appropriate.

The wave is larger than the normal one-to-five-file atomic package bounds. Repairing an inherited interdependent compile failure explains its integration boundary, but does not waive focused review. Preserve shared history; audit the constituent packages separately and use the recorded smaller splits for subsequent commits.

The research-waiver fixture's dynamic lookup selects its own stable test offer ID rather than assuming registry index zero. The substantive assertions remain independent: missing selected payment rejects without mutation, resolver-less research cannot pay a timed waiver, legal payment grants the technology and pays once. This change is not tautological.

The reviewed decision classifications match their actual observed delivery: the two resolve_research constructors use strategy_cards::ask; Yssaril action_card_draw_requested asks with observation locally; its transaction choice is also observed locally. The inventory is a structural coverage check; it does not prove hidden-information correctness by itself.

Matching all ten metrics on thirty six-faction seeds is diagnostic evidence for that sample. It is insufficient for an unrestricted behavior-neutrality claim or a decision-boundary compatibility gate. Record that limit explicitly. Rebaseline intervals remain a separately reviewable artifact.

## Open acceptance

Root-owned combat, ground timing, leader continuation, Alliance foundations, new legality corrections and the later integration phase remain uncommitted and not independently frontier-reviewed. Supplemental Luna findings have been preserved and addressed, including staged ground-event error swallowing; this is not a substitute for the mandated review tier. No inherited or current behavior is declared fully complete from soak metrics alone.
