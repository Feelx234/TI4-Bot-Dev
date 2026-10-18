//! One test per technology effect, so "the technology could be owned and did nothing" cannot
//! happen unnoticed. The coverage table is `docs/TECH_EFFECT_COVERAGE.md`.
//!
//! Test names: `<tech id>_<effect number>_<what it does>` (the id with `-` written `_`), the
//! number being the effect's row in the table. An effect the engine does not implement (or
//! implements wrongly) keeps its test and is marked `#[ignore = "MISSING: <id> ..."]`, so the
//! suite stays green while the gap stays visible (`cargo test -- --ignored` lists them).
//!
//! Group files: `g1` generic techs, `g2`..`g4` faction techs. Shared helpers live here.

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, TechnologyId};
use ti4_model::state::GameState;

mod g1;
mod g2;
mod g3;
mod g4;

/// The embedded content, for the tests that need it.
pub(super) fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

/// `PlayerId::new` for a seat name.
pub(super) fn pid(name: &str) -> PlayerId {
    PlayerId::new(name)
}

/// Give `player` the technology `alias` (owned, ready), without paying for it.
pub(super) fn give(state: &mut GameState, player: &str, alias: &str) {
    crate::technology::grant(state, &pid(player), &TechnologyId::new(alias));
}

/// Does `player` own `alias`?
#[allow(dead_code)]
pub(super) fn owns(state: &GameState, player: &str, alias: &str) -> bool {
    state
        .player(&pid(player))
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new(alias)))
}

/// A seated, deployed game (home systems, starting units) of `(seat, faction)` pairs at POK.
pub(super) fn seated(seats: &[(&str, &str)]) -> GameState {
    crate::fixtures::seated_game(seats, POK)
}
