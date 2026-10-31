//! `Area::ExplorationRelics` (promissory notes, package W14 of lent-bodies lane 5): the core half of Military Support
//! (`promissory::turn_started`) beside its string body, and the placement window it is built on
//! (`ti4_engine_core::rules::promissory_windows`, the planets a seat controls).
//!
//! The core half answers `None` (the string result stands, counted as a bail) where the position holds more than the window: a
//! unit type the typed tables do not know, a question inside a battle or an invasion, diplomacy on, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::promissory_windows::{PlaceUnits, Then};
    use ti4_engine_core::state::Slice;
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::core_sync::{Footprint, Sources, with_core};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::{Recording, apply_staged, drive_with};

    /// What the window reads (the planets, the box, the laws) and writes (the units, the token, the note).
    const WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Cards, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Cards, Slice::Pending],
    );

    /// The core half of Military Support (`promissory::turn_started`; the value: the card resolved). `holder` places the infantry
    /// (`form`: the unit type the string would place, `None` when it places nothing or a faction rule forbids it), `owner` is the
    /// Sol seat whose token is spent and to whom the note returns.
    pub fn military_support(
        gs: &mut GameState,
        owner: &PlayerId,
        holder: &PlayerId,
        form: Option<&UnitTypeId>,
        sources: ti4_model::content_types::SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let unit = match form {
            Some(id) => Some(crate::core_adapter::typed::<Unit>(id.as_str()).why("a unit form the typed tables do not know")?),
            None => None,
        };
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let (owner_seat, seat) = (r.seat(owner.as_str()).why(crate::dual::reason::NO_SEAT)?, r.seat(holder.as_str()).why(crate::dual::reason::NO_SEAT)?);
        let scope = ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let before = gs.clone();
        let kernel = Kernel::plain();
        let window = PlaceUnits::new(seat, unit, 2, None, true, scope, r.code("ms"), Then::MilitarySupport { owner: owner_seat, reason: r.code("military_support") });
        let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[seat, owner_seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = with_core(gs, &mut cx, Sources::default(), WINDOW, |core, _| {
            let driven = drive_with(&mut driver, core, &kernel, &r, &before, script, &|c, _, rendered| crate::render::promissory::complete_place_units(c, rendered));
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn military_support(
        _: &mut GameState,
        _: &PlayerId,
        _: &PlayerId,
        _: Option<&UnitTypeId>,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }
}
