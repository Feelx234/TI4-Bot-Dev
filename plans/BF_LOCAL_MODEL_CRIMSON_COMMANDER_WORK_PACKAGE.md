# Local model work package: BF-BORROWED-CRIMSON-COMBAT

## Task

Implement the Crimson Rebellion commander effect for a player who has acquired its ability, without adding Crimson to supported faction seating. Work only in D:/Projects/ti4-engine-rs, branch wp/base-factions. This is one bounded implementation package; stop after reporting the diff and evidence. Do not spawn agents.

## Dependencies and reading

Read AGENTS.md, plans/SCOPED_PERMISSIONS.md and plans/PI_WORK_PACKAGE_STANDARD.md. Then read only the relevant sections of:

- crates/ti4-content/content/leaders.json: crimsoncommander.
- crates/ti4-engine/src/factions/borrowed_commanders.rs: existing Titans listener and its tests.
- crates/ti4-engine/src/factions/mod.rs: generic acquired-commander registration.
- crates/ti4-engine/src/promissory.rs: has_commander_ability and grant_commander_ability.
- crates/ti4-engine/src/strategy_cards.rs: commodity_limit.
- crates/ti4-engine/src/supply.rs: gain_trade_goods_via.
- crates/ti4-engine/src/combat.rs and invasion.rs: emitted SPACE_COMBAT_ENDED / GROUND_COMBAT_ENDED names, relations and payloads. Read exact producers, not both entire files.
- crates/ti4-engine/tests/decision_delivery_inventory.rs: producer and observed-ask registration format.

The root coordinator has already registered borrowed_commanders::timing_abilities for every seat. Reuse that registration. No fake FactionModule or asset catalog entry is necessary.

## Exact behavior

Use the source card's exact text: at the end of a combat between any players, the recipient gains one commodity or converts one of their commodities to a trade good. The recipient does not need to participate in that combat. Eligibility is promissory::has_commander_ability(state, recipient, "crimsoncommander"); never require the recipient to be Crimson or own an unlocked Crimson leader record.

Register recipient-bound listeners on the actual space and ground combat-ended events at their appropriate after timing. Give distinct stable ability IDs including recipient/faction label, commander ID, event and relation. One combat ending gives one effect per eligible recipient, never once per round or once per opponent.

Generate "gain_commodity" only below commodity_limit, and "convert_commodity" only with an existing commodity. If no option is legal, do nothing. If exactly one option is legal, apply it without a redundant question. If both are legal, ask the recipient using TimingContext::ask_seeing and a typed DecisionContext (FactionAbility("crimsoncommander"), subtype "combat_commodity", current phase/round). Do not expose another player's private hand. Confirm mandatory/optional choice behavior from the printed text; do not invent a decline option.

For conversion, remove exactly one existing commodity and gain exactly one trade good through supply::gain_trade_goods_via so gain listeners (including Pillage) execute. Commodity gain obeys the current limit; conversion still works when at that limit. Validate all choices before mutation. If nested gain timing fails, propagate the error and restore this callback's state, dice/RNG, event sequence and resolver checkpoint; do not swallow the failure or duplicate a payment on retry. Preserve outer timing ordering.

## Acceptance tests

Add focused resolver tests inside borrowed_commanders.rs using fixtures::armed_resolver and fixtures::with_context:

1. Ownerless grant to a non-Crimson recipient: a combat between other seats still offers the recipient its own gain/convert choice.
2. Real space and ground combat-ended producers route to the handler (at minimum a fixture tied to each real event shape, with one actual combat window fixture where practical).
3. Gain below limit increments commodity by one and adds no TG.
4. Conversion decreases commodities by one, gains one TG, and emits TRADE_GOODS_GAINED exactly once; at the commodity limit only conversion is offered.
5. Zero commodities at a zero commodity limit produces no effect/question; no right and a locked ordinary Alliance remain inert; unlocking the Alliance owner enables it.
6. Invalid selected option is an error with no state/dice/RNG/journal mutation. Include a nested gain-reaction failure/retry fixture if needed to verify the conversion rollback.
7. Recipient binding: rights granted to one seat do not give another seat the effect.

Do not mirror helper implementation in assertions. Assert legal options, actual balances, recipient, timing event and rollback behavior.

## Writable paths and exclusions

Permission class P1. Writable paths ONLY:

- crates/ti4-engine/src/factions/borrowed_commanders.rs
- crates/ti4-engine/tests/decision_delivery_inventory.rs (only exact new producer/observed delivery rows)
- plans/evidence/BF-BORROWED-CRIMSON-COMBAT.md

All other paths are read-only for this package. The checkout is shared and intentionally dirty. Preserve the existing Titans implementation and all other changes. Do not edit game.rs, production.rs, mod.rs, EXECUTION_STATE.md, income experiments, policy/training/UI files, or package ledgers. No folders, deletions, reset/checkout/clean, network, dependency changes, pushes, commits, or server launches.

## Validation and delivery

The root coordinator owns Cargo to prevent concurrent builds. Do not run Cargo. Use targeted rustfmt --edition 2024 crates/ti4-engine/src/factions/borrowed_commanders.rs and git diff --check -- <your exact paths>. Do not use cargo fmt or format the workspace.

Write evidence with exact card source, changed functions, focused test names, permission scope, any unresolved semantics, and that Cargo was not run locally. Report findings and stop. Root will run focused tests, engine tests/inventory and lints, review critical logic, and arrange the required independent tier-C acceptance before a scoped commit. The package is not complete merely because a patch exists. If a tool or approval rejects an action, report the exact failure; do not bypass it.

Suggested local runner bounds: 600 seconds, first edit within 120 seconds, one simpler retry, no streaming transcript polling. Submit only this task through an existing authorized Pi controller; do not attach another process to the same Pi session.
