//! `Area::LeadersAbilities` (package W10 of lent-bodies lane 8): the core halves of the heroes composed from windows
//! (`ti4_engine_core::rules::hero_windows`), beside their string bodies in `factions/*.rs`.
//!
//! A core half answers `None` (the string result stands, counted as a bail with its reason) where the position holds more than the body:
//! a unit type the typed tables do not know, a question inside a battle or an invasion, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

/// One planet UNIT.DSGN.FLAYESH can devour as the string read it: the system, the planet, its resources and influence now and the
/// technologies of its colours the seat may gain (content order).
#[derive(Debug, Clone)]
pub struct FlayeshTarget {
    pub system: ti4_model::id::SystemId,
    pub planet: ti4_model::id::PlanetId,
    pub goods: i64,
    pub techs: Vec<ti4_model::id::TechnologyId>,
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{Planet, System, Technology};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::hero_windows::{Devourable, NekroHero};
    use ti4_engine_core::state::Slice;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use super::FlayeshTarget;
    use crate::core_adapter::typed;
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;

    fn kernel(sources: SourceSet) -> Option<Kernel> {
        let mut kernel = Kernel::plain();
        kernel.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::core_adapter::scope(sources).0).ok()?);
        Some(kernel)
    }

    /// What UNIT.DSGN.FLAYESH reads (the units of the planets, the trade goods) and writes (the units, the goods, the technology, the staged
    /// ground row and the staged event).
    const FLAYESH: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Marks, Slice::Pending],
    );

    /// The core half of Nekro's UNIT.DSGN.FLAYESH (`factions::nekro_units::flayesh`; the value: a planet was devoured). `targets` are the
    /// planets the string offers, in its order (empty: the hero is not playable); the facts are the string's reads.
    pub fn nekro_hero(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        player: &PlayerId,
        targets: &[FlayeshTarget],
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let planets: Vec<Devourable> = targets
            .iter()
            .map(|t| {
                Some(Devourable {
                    system: typed::<System>(t.system.as_str())?,
                    planet: typed::<Planet>(t.planet.as_str())?,
                    goods: i32::try_from(t.goods).ok()?,
                    techs: t.techs.iter().map(|alias| typed::<Technology>(alias.as_str())).collect::<Option<_>>()?,
                })
            })
            .collect::<Option<_>>()
            .why("a planet or technology the typed tables do not know")?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut staged = Vec::new();
        let mut driver = Driver::new(NekroHero::new(seat, r.code("nekrohero"), planets), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), FLAYESH, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("a galvanized ground force fell (the staged row cannot hold it)");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        // The string re-forms the units after the gain (`technology::gain`): a position it would change is outside the window.
        let mut formed = gs.clone();
        crate::technology::apply_unit_upgrades(&mut formed, content, sources, player);
        if !formed.identical(gs) {
            crate::dual::note_bail_here("the unit upgrades would re-form units after the gain");
            return None;
        }
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    pub fn nekro_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &[super::FlayeshTarget],
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
}
