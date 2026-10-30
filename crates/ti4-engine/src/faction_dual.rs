//! `Area::Factions` (package W11 of lent-bodies lane 5): the core halves of the faction component actions the core owns
//! (`ti4_engine_core::rules::faction_component`), beside their string bodies in `factions/*.rs`.
//!
//! A core half answers `None` (the string result stands, counted as a bail) where the position holds more than the body: a
//! unit type the typed tables do not know, a question inside a battle or an invasion, diplomacy on, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::rules::faction_component as rules;
    use ti4_engine_core::state::Slice;
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::core_sync::{Footprint, Sources, with_core};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;

    /// What the leaf bodies read (the cards, the planets, the box) and write (the exhausted card, the units).
    const LEAF: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks], &[Slice::Players, Slice::Board]);

    /// `scope` of the sources, as the core reads it.
    fn scope(sources: ti4_model::content_types::SourceSet) -> Option<ti4_engine_core::tables::Scope> {
        Some(ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?))
    }

    /// The core half of Winnu's Lazax Gate Folding (`factions::winnu::perform_component`; the value: the action resolved).
    /// `form` is the infantry the string would place (`None`: nothing, or a faction rule forbids it).
    pub fn lazax_gate_folding(
        gs: &mut GameState,
        player: &PlayerId,
        mecatol: &str,
        form: Option<&UnitTypeId>,
        sources: ti4_model::content_types::SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let mecatol = crate::core_adapter::typed::<System>(mecatol).why(crate::dual::reason::UNKNOWN_ID)?;
        let form = match form {
            Some(id) => Some(crate::core_adapter::typed::<Unit>(id.as_str()).why("a unit form the typed tables do not know")?),
            None => None,
        };
        let scope = scope(sources)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let done = with_core(gs, &mut cx, Sources::default(), LEAF, |core, _| rules::lazax_gate_folding(core, scope, seat, mecatol, form))
            .ok()
            .why(crate::dual::reason::NOT_LENDABLE)?;
        Some(CoreWindow { run: Run::new(done).with_events_vec(Vec::new()), questions: Vec::new() })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn lazax_gate_folding(
        _: &mut GameState,
        _: &PlayerId,
        _: &str,
        _: Option<&UnitTypeId>,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }
}
