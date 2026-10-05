# BF-NAAZ-ABILITY-PRODUCTION supplemental review

Status: read-only supplemental review; not the independent frontier acceptance. No Cargo/build run.

## Finding

No functional defect found in the reviewed correction. `ProductionWindow::build_options` calls the existing `hooks_economy::cannot_produce` hook for ability production using the explicit `"ability"` producer sentinel before adding a build option. `naaz::cannot_produce` intentionally ignores the producer and therefore blocks a mech while Eidolon Maximum is present. `arborec::cannot_produce` requires `producer_base == "spacedock"`, so ability production remains outside Arborec Mitosis's space-dock-only restriction. The hook contract now documents that distinction. Search of the current hook registrations found these two implementations, and the sentinel does not conflict with either.

The actual Naaz `production_choices_hide_mechs_only_while_the_maximum_stands` fixture covers both sides of the behavior through `ProductionWindow::for_ability` / `pending_choice`: controlled ground destination remains available, a `naaz_mech` offer is absent with Maximum on the board, and returns after Maximum is removed. The resolver regenerates the pending choice before validating the answer, so a stale offered mech cannot bypass the new filter if state changes between offer and resolution.

## Coverage note

Low-priority regression opportunity: add a direct ability-production offer assertion for a player with Mitosis and an otherwise producible infantry, confirming the `"ability"` sentinel leaves that offer available. The current predicate itself already demonstrates the behavior (`producer_base == "spacedock"`), and the existing Mitosis restrictions are unchanged; this is test hardening, not a correctness blocker.

No source change requested. Root still owns focused/affected tests and the required independent tier-C review.
