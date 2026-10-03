# BF inventory gain baseline — 2026-10-03

Game construction now records held relics and breakthroughs only when a new faction module is seated. Existing saved gain baselines are preserved, so a pending gain still dispatches after reconstruction. Six-original-faction state remains unchanged. Naalu’s running-game regression now grants its relic after construction.

Validation on the clean checkout based at ec8e957c: focused gain regressions passed; full ti4-engine suite passed (1,919 library tests, 1 ignored, all integration tests and doctests). Independent Terra review by naalu_hooks_terra found no actionable findings. A symmetric pending-breakthrough regression was suggested as optional coverage.

Integration used the clean source patch with git apply and git apply --cached. The unrelated game.rs income experiment remains unstaged. No Fracture suppression was introduced; Iconoclast retains its sole RELIC_GAINED listener.
