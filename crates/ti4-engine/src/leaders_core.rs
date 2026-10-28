//! `Area::LeadersAbilities` (package B7), the mutation half: the status writes of `leaders.rs`
//! (`ready_all`, `ready`, `exhaust`, `purge`, the write of `check_unlocks`) as `ti4_engine_core::rules::leaders`
//! functions over the Players slice. Every function answers `None` when the position is outside what the core
//! holds (more than 16 owners, a leader id the tables do not know, a value the codecs refuse): the caller
//! keeps the string write. The side effects the core cannot do (Nomad's agent hooks, the staged event) stay
//! in the string callers, after the status write.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::leaders as rules;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::leader_windows::{HacanAgent, Helmsman, RinHero, XxchaAgent};
    use ti4_model::content_types::SourceSet;

    use crate::core_adapter::typed;
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::Recording;
    use crate::generated::IdStr;
    use crate::core_sync::{Footprint, Sources};

    const WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);

    fn lend<T>(gs: &mut GameState, player: &PlayerId, f: impl FnOnce(&mut CoreState, Seat) -> T) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let who = r.seat(player.as_str())?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| f(core, who)).ok()
    }

    fn ids(list: &[Leader]) -> Vec<LeaderId> {
        list.iter().map(|l| LeaderId::new(l.as_str())).collect()
    }

    /// `leaders::ready_all`.
    pub fn ready_all(gs: &mut GameState, player: &PlayerId) -> Option<Vec<LeaderId>> {
        lend(gs, player, |core, who| ids(rules::ready_all(core, who).as_slice()))
    }

    /// `leaders::ready`.
    pub fn ready(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::ready(core, who, l))
    }

    /// `leaders::exhaust`.
    pub fn exhaust(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::exhaust(core, who, l))
    }

    /// `leaders::purge`.
    pub fn purge(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::purge(core, who, l))
    }

    /// Evelyn DeLouis / Viscount Unlenn: the extra die of this combat round. `base` is the unit type the
    /// string side resolved (a form the tables do not know bails).
    pub fn extra_die_agent(gs: &mut GameState, player: &PlayerId, base: &str) -> Option<bool> {
        let unit = typed::<ti4_engine_core::generated::Unit>(base)?;
        lend(gs, player, |core, who| rules::extra_die_agent(core, who, unit))
    }

    /// Darktalon Treilla's activation.
    pub fn activate_letnev_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        lend(gs, player, |core, who| rules::activate_letnev_hero(core, who))
    }

    /// Hacan's hero activation.
    pub fn activate_hacan_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        lend(gs, player, |core, who| rules::activate_hacan_hero(core, who))
    }

    /// `leaders::end_of_round`.
    pub fn end_of_round(gs: &mut GameState) -> Option<Vec<PlayerId>> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seats = crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| {
            rules::end_of_round(core).as_slice().to_vec()
        })
        .ok()?;
        Some(seats.into_iter().map(|s| PlayerId::new(r.seat_text(s))).collect())
    }


    /// With a map the Map slice is loaded (the string galaxy lends it); without one it cannot be, and the window
    /// never reads it.
    const XXCHA: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Pending], &[Slice::Board]);
    const XXCHA_NO_MAP: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board]);
    const HACAN: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    const BOARD: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Board]);

    /// Jace X. 4th Air Legion.
    pub fn sol_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let who = r.seat(player.as_str())?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::sol_hero(core, who)).ok()
    }

    /// I48S: the swap of one infantry of `target` for `mech` (a unit form without a core `Unit`, the Naaz mech
    /// whose placement is announced, and a favour that keeps a diplomacy promise stay with the string arm).
    pub fn l1z1x_agent(
        gs: &mut GameState,
        sources: SourceSet,
        player: &PlayerId,
        target: &PlayerId,
        system: &ti4_model::id::SystemId,
        mech: &str,
    ) -> Option<bool> {
        if mech == "naaz_mech" || (gs.diplomacy.enabled && target != player) {
            return None;
        }
        let (mech, system) = (typed::<ti4_engine_core::generated::Unit>(mech)?, typed::<ti4_engine_core::generated::System>(system.as_str())?);
        let scope = ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(target.as_str())?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::l1z1x_agent(core, scope, seat, system, mech)).ok()
    }

    /// What a window of this file cannot follow: a question asked inside a battle or an invasion (its context
    /// carries the battle), diplomacy promises a favour keeps, a nested emission.
    fn outside(gs: &GameState, script: &Recording) -> bool {
        script.nested || gs.active_space_combat.is_some() || gs.active_invasion.is_some() || gs.diplomacy.enabled
    }

    /// The kernel of a leader window: no hooks, the game's source scope.
    fn kernel(sources: SourceSet) -> Option<Kernel> {
        let mut kernel = Kernel::plain();
        kernel.sources = ti4_engine_core::kernel::Sources(u8::try_from(crate::core_adapter::scope(sources).0).ok()?);
        Some(kernel)
    }

    /// The core half of `leaders::xxcha_agent`. `None` when the position is outside what the core holds (see
    /// [`outside`]; a map on one side only).
    pub fn xxcha_agent(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(XxchaAgent::new(seat, has_map), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { XXCHA } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .ok()??
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::l1z1x_hero` (The Helmsman). `None` as for [`xxcha_agent`].
    pub fn l1z1x_hero(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(Helmsman::new(seat, has_map), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { XXCHA } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .ok()??
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    const PLAYERS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    /// The core half of `leaders::jolnar_hero` (Rin). `None` where `apply_unit_upgrades` would change the
    /// position (the units are not in the form the owned upgrades demand: the string re-forms them after a swap
    /// and the core does not), and as [`xxcha_agent`] otherwise.
    pub fn jolnar_hero(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) {
            return None;
        }
        let mut formed = gs.clone();
        crate::technology::apply_unit_upgrades(&mut formed, content, sources, player);
        if !formed.identical(gs) {
            return None;
        }
        // The corpus file's order of the technologies (the string lists replacements in it).
        let order: Vec<_> = content
            .from_sources(ti4_model::content_types::ContentType::Technologies, sources)
            .filter_map(|r| r.text("alias").and_then(typed::<ti4_engine_core::generated::Technology>))
            .collect();
        let mut by_alias: Vec<_> = ti4_engine_core::generated::Technology::ALL.to_vec();
        by_alias.sort_by_key(|t| t.as_str());
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut driver = Driver::new(RinHero::new(seat, order, by_alias), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PLAYERS, |core, _| {
            crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script)
        })
        .ok()?
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::hacan_agent`. `None` where a commodity change causes events the core does not
    /// hold: a siphoning Firmament plot, a Cabal at the table, a Trade Agreement in play.
    pub fn hacan_agent(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) {
            return None;
        }
        let others_cause_events = gs.players.iter().any(|p| {
            p.faction.as_str() == "cabal"
                || crate::factions::firmament_plots::siphon_watches(gs, &p.id)
                || crate::promissory::holder_of(gs, "ta", &p.id).is_some()
        });
        if others_cause_events {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(HacanAgent::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), HACAN, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, rendered| {
                rendered.details.insert("commodities".to_owned(), crate::strategy_cards::commodity_standing(&before, content, player));
            })
        })
        .ok()?
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The status write of `leaders::check_unlocks`.
    pub fn unlock(gs: &mut GameState, player: &PlayerId, leaders: &[LeaderId]) -> Option<()> {
        let list: Option<Vec<Leader>> = leaders.iter().map(|l| typed::<Leader>(l.as_str())).collect();
        let list = list?;
        lend(gs, player, |core, who| rules::unlock(core, who, &list))
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    pub fn ready_all(_: &mut GameState, _: &PlayerId) -> Option<Vec<LeaderId>> {
        None
    }
    pub fn ready(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn exhaust(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn purge(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn unlock(_: &mut GameState, _: &PlayerId, _: &[LeaderId]) -> Option<()> {
        None
    }
    pub fn extra_die_agent(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }
    pub fn activate_letnev_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn activate_hacan_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn end_of_round(_: &mut GameState) -> Option<Vec<PlayerId>> {
        None
    }
    pub fn sol_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn l1z1x_agent(
        _: &mut GameState,
        _: ti4_model::content_types::SourceSet,
        _: &PlayerId,
        _: &PlayerId,
        _: &ti4_model::id::SystemId,
        _: &str,
    ) -> Option<bool> {
        None
    }
    pub fn l1z1x_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn jolnar_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn xxcha_agent(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn hacan_agent(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
}
