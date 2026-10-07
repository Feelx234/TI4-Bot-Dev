# BF-NAAZ-HERO-WARFARE

Scope: correct Hesh and Prit's readied/unchosen secondary candidate list; no primary or tactical continuation is involved. Permission P1; writable naaz.rs and this evidence, no external state or deletion. Tier C review pending.

The inherited candidate filter excludes TE Warfare because its primary returns FreeTactical, but Hesh and Prit resolves the secondary. strategy_cards::secondary dispatches Warfare to home_production and returns Resolved. Preserve readied/unchosen and no-repeat constraints; include Warfare and verify actual secondary production. Focused red/green validation pending.

Validation: the new offered-Warfare regression failed at hero_cards.contains(te6warfare), then passed after removing the primary-based exclusion. The strengthened fixture selects TE Warfare and executes actual home production; unit count increases and exactly one relic is gained. Focused 1/1 passed. Required tier-C acceptance remains pending.

Latest coordinated affected-crate validation: cargo test -p ti4-engine -q -j1, TEST_DEBUG=0 / TEST_INCREMENTAL=false, LIBTORCH unset: 1,993 lib passed, zero failed, one ignored; integrations 1/1, 4/4 delivery inventory, 5/5; doctests passed. Required review and package commit remain pending.
