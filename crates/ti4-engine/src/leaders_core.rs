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
    use ti4_engine_core::rules::leader_windows::{ArmageddonRelay, HacanAgent, Helmsman, LetaniOspha, NekroMalleon, RinHero, XxchaAgent};
    use ti4_model::content_types::SourceSet;

    use crate::core_adapter::typed;
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;
    use crate::generated::IdStr;
    use crate::core_sync::{Footprint, Sources};

    const WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);

    fn lend<T>(gs: &mut GameState, player: &PlayerId, f: impl FnOnce(&mut CoreState, Seat) -> T) -> Option<T> {
        crate::dual_window::lend(gs, WRITES, Some(player), |core, who| f(core, who.expect("a seated call")))
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
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        lend(gs, player, |core, who| rules::ready(core, who, l))
    }

    /// `leaders::exhaust`.
    pub fn exhaust(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        lend(gs, player, |core, who| rules::exhaust(core, who, l))
    }

    /// `leaders::purge`.
    pub fn purge(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
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
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
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
    /// Xxekir Grom places units (a Naaz mech is announced: the staged event waits in the pending slice).
    const XXCHA_HERO: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board, Slice::Pending]);
    const HACAN: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Pending], &[Slice::Players, Slice::Cards]);

    const BOARD: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Board]);
    /// [`BOARD`] for the writes that announce a Naaz mech (the staged event waits in the pending slice).
    const BOARD_STAGED: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Pending], &[Slice::Board, Slice::Pending]);

    /// Jace X. 4th Air Legion.
    pub fn sol_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let who = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::sol_hero(core, who)).ok()
    }

    /// Ahk-Syl Siven: the mark of the round the effect runs through.
    pub fn nomad_hero(gs: &mut GameState, player: &PlayerId) -> Option<bool> {
        const MARKS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks], &[Slice::Marks]);
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let who = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), MARKS, |core, _| rules::nomad_hero(core, who)).ok()
    }

    /// Ul the Progenitor: Elysium readied and wearing Geoform.
    pub fn titans_hero(gs: &mut GameState) -> Option<bool> {
        let (mut cx, _) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::titans_hero(core)).ok()
    }

    /// Claire Gibson: the placement of `count` units of `unit` (resolved by the string side) on the planet.
    pub fn sol_commander(
        gs: &mut GameState,
        player: &PlayerId,
        system: &ti4_model::id::SystemId,
        planet: &ti4_model::id::PlanetId,
        unit: &str,
        count: usize,
    ) -> Option<bool> {
        let unit = typed::<ti4_engine_core::generated::Unit>(unit)?;
        let (system, planet) = (
            typed::<ti4_engine_core::generated::System>(system.as_str())?,
            typed::<ti4_engine_core::generated::Planet>(planet.as_str())?,
        );
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let who = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD, |core, _| rules::sol_commander(core, who, system, planet, unit, count)).ok()
    }

    /// I48S: the swap of one infantry of `target` for `mech` (a unit form without a core `Unit` and a favour that keeps a
    /// diplomacy promise stay with the string arm; a Naaz mech is announced by the core rule since package W3).
    pub fn l1z1x_agent(
        gs: &mut GameState,
        sources: SourceSet,
        player: &PlayerId,
        target: &PlayerId,
        system: &ti4_model::id::SystemId,
        mech: &str,
    ) -> Option<bool> {
        if gs.diplomacy.enabled && target != player {
            crate::dual::note_bail_here("a favour that keeps a diplomacy promise");
            return None;
        }
        let (mech, system) = (typed::<ti4_engine_core::generated::Unit>(mech)?, typed::<ti4_engine_core::generated::System>(system.as_str())?);
        let scope = ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?);
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(target.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let mut staged = Vec::new();
        let swapped = crate::core_sync::with_core(gs, &mut cx, Sources::default(), BOARD_STAGED, |core, _| {
            let swapped = rules::l1z1x_agent(core, scope, seat, system, mech);
            staged = std::mem::take(&mut core.pending_mut().staged);
            swapped
        })
        .ok()?;
        crate::dual_window::apply_staged(gs, &r, &staged)?;
        Some(swapped)
    }


    /// Whether a commodity change at this table causes events the core does not hold: a Cabal (its replenish
    /// note), a siphoning Firmament plot, a Trade Agreement in play.
    pub(crate) fn replenish_causes_events(gs: &GameState) -> bool {
        gs.players.iter().any(|p| {
            p.faction.as_str() == "cabal"
                || crate::factions::firmament_plots::siphon_watches(gs, &p.id)
                || crate::promissory::holder_of(gs, "ta", &p.id).is_some()
        })
    }

    /// Why the core windows that replenish (the Trade primary and secondary, Carth of Golden Sands) cannot follow a table: a plot that
    /// listens to commodity gains (Siphon: a Firmament/Obsidian seat, never seated in the lobby), or two seats of one faction (the Trade
    /// Agreement and the option ids are keyed by faction). Cabal's `COMMODITIES_REPLENISHED` and the Trade Agreement are the core's
    /// since package W10 (`fx::replenish_announced`).
    pub(crate) fn replenish_window_unported(gs: &GameState) -> Option<&'static str> {
        if gs.players.iter().any(|p| crate::factions::firmament_plots::siphon_watches(gs, &p.id)) {
            return Some("a siphoning plot listens to the commodities a replenish gains");
        }
        let factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        if factions.iter().enumerate().any(|(i, f)| factions[..i].contains(f)) {
            return Some("two seats of one faction (the Trade Agreement is keyed by faction)");
        }
        None
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
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
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
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of Xxekir Grom (`xxchahero-te`): up to four PDS or mechs onto controlled planets, each planet
    /// readied. `None` as for [`xxcha_agent`], and for a unit form the tables do not know (a Naaz mech is announced by the
    /// window since package W3).
    pub fn xxcha_hero_te(gs: &mut GameState, player: &PlayerId, facts: &crate::leaders::HeroTeFacts, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::generated::{Planet, System, Unit};
        use ti4_engine_core::rules::leader_windows::XxchaHeroTe;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let form = |f: &Option<(ti4_model::id::UnitTypeId, usize)>| -> Option<Option<(Unit, usize)>> {
            match f {
                None => Some(None),
                Some((unit, left)) => Some(Some((typed::<Unit>(unit.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, (*left).min(usize::from(XxchaHeroTe::MOST))))),
            }
        };
        let (pds, mech) = (form(&facts.pds)?, form(&facts.mech)?);
        let planets: Vec<(System, Planet)> = facts
            .planets
            .iter()
            .map(|(s, p)| Some((typed::<System>(s.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, typed::<Planet>(p.as_str()).why(crate::dual::reason::UNKNOWN_ID)?)))
            .collect::<Option<_>>()?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = Kernel::plain();
        let before = gs.clone();
        let mut driver = Driver::new(XxchaHeroTe::new(seat, &planets, pds, mech), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), XXCHA_HERO, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::l1z1x_hero` (The Helmsman). `None` as for [`xxcha_agent`].
    pub fn l1z1x_hero(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
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
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of Saar's Gurno Aggero (`factions::saar::hero`). `plain` says the player's adjacency is the map's own (no faction rule
    /// links or blocks a border); the window reads the map's adjacency, so any other position is the string's. `None` as for [`xxcha_agent`].
    pub fn saar_hero(gs: &mut GameState, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, plain: bool, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if !plain {
            crate::dual::note_bail_here("a seat whose adjacency a faction rule changes (extra wormholes, links, blocked borders)");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(ArmageddonRelay::new(seat, has_map), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { XXCHA } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of Ghost's Riftwalker Meian (`factions::ghost::hero_swap`). `allowed` and `wormholes` are the string's reads of which
    /// systems the card may name and which hold a wormhole now (the window cannot read the Creuss tokens, the ion storm, the Nexus and the
    /// module wormholes). `None` as for [`xxcha_agent`].
    pub fn ghost_hero(
        gs: &mut GameState,
        player: &PlayerId,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        allowed: &[&str],
        wormholes: &[&str],
        sources: SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::leader_windows::{RiftwalkerMeian, SwapFacts};
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let set = |ids: &[&str]| -> Option<ti4_engine_core::bits::SysSet> {
            let mut out = ti4_engine_core::bits::SysSet::EMPTY;
            for id in ids {
                out.insert(typed::<ti4_engine_core::generated::System>(id)?.index());
            }
            Some(out)
        };
        let facts = SwapFacts { allowed: set(allowed).why(crate::dual::reason::UNKNOWN_ID)?, wormholes: set(wormholes).why(crate::dual::reason::UNKNOWN_ID)? };
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let has_map = galaxy.is_some();
        let mut driver = Driver::new(RiftwalkerMeian::new(seat, has_map, facts), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources { galaxy, rng: None }, if has_map { GHOST_HERO } else { XXCHA_NO_MAP }, |core, _| {
            if has_map && core.galaxy().is_none() {
                return None;
            }
            Some(crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The hero reads the map and the board and writes one mark.
    const GHOST_HERO: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Marks, Slice::Pending], &[Slice::Marks]);

    /// What Nekro Malleon reads (hands, tokens) and writes (hands, tokens, trade goods, the staged discard mark and event).
    const MALLEON: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Marks, Slice::Pending],
    );

    /// The core half of Nekro's Malleon (`factions::nekro_units::malleon`). `None` as for [`xxcha_agent`].
    pub fn nekro_agent(gs: &mut GameState, player: &PlayerId, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if gs.player(player).and_then(|p| p.leaders.get(&LeaderId::new("nekroagent"))) != Some(&ti4_model::state::LeaderStatus::Readied) {
            // The string `malleon` declines at once (`leader_action` needs the card readied in the user's own hand; a copy through
            // Ssruu does not qualify): no question, no change. That is the window too (package W10; before it a counted bail).
            return Some(CoreWindow { run: Run::new(false).with_events_vec(Vec::new()), questions: Vec::new() });
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut table = AbilityTable::default();
        table.facts.card_groups = crate::reactions_core::card_groups();
        let mut staged = Vec::new();
        let mut driver = Driver::new(NekroMalleon::new(seat, r.code("nekroagent")), Resolver::new(&seats, Some(seat)), table, 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), MALLEON, |core, _| {
            let driven = crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// What Letani Ospha reads (the ships, the box, the unit costs) and writes (the replaced ship).
    const OSPHA: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Pending], &[Slice::Board]);

    /// The core half of Arborec's Letani Ospha (`factions::arborec::agent_use`). `forms` is the unit type each seat places per ship base type
    /// (the string `player_unit`); a form the typed tables do not know bails. `None` as for [`xxcha_agent`].
    pub fn arborec_agent(gs: &mut GameState, player: &PlayerId, forms: &[(PlayerId, ti4_model::id::UnitTypeId)], sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let forms: Vec<(Seat, ti4_engine_core::generated::Unit)> = forms
            .iter()
            .map(|(owner, unit)| Some((r.seat(owner.as_str())?, typed::<ti4_engine_core::generated::Unit>(unit.as_str())?)))
            .collect::<Option<_>>()
            .why("a unit form the typed tables do not know")?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut driver = Driver::new(LetaniOspha::new(seat, forms), Resolver::new(&seats, Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), OSPHA, |core, _| crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {}))
            .why(crate::dual::reason::NOT_LENDABLE)?
            .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What a production by an ability reads (the position the production reads, the pending staged events) and writes (the units, the
    /// payment, the staged events).
    const PRODUCE: Footprint = Footprint {
        reads: crate::production_view::READS.union(ti4_engine_core::state::SliceSet::of(&[Slice::Pending])),
        writes: ti4_engine_core::state::SliceSet::of(&[Slice::Board, Slice::Players, Slice::Transient, Slice::Marks, Slice::Pending]),
    };

    /// Why the core cannot follow a production by an ability for these seats: a seat the production windows do not carry, the end-of-use
    /// the dropped Last Bastion and Crimson Rebellion (a unit placed away from the producing system has no place text here).
    fn production_declined(gs: &GameState, producers: &[&PlayerId]) -> Option<&'static str> {
        if gs.players.iter().any(|p| [crate::factions::bastion::MODULE.alias, crate::factions::crimson::MODULE.alias].contains(&p.faction.as_str())) {
            return Some("the Last Bastion or the Crimson Rebellion at the table (units placed away from the producing system)");
        }
        if producers.iter().any(|p| crate::production_view::unported_triggered(gs, p)) {
            return Some("a table whose seats the production windows do not carry (Keleres, Deepwrought)");
        }
        // (Hacan's breakthrough, the Prophecy of Ixth and a swapped planet value are the end-of-use effects the adapter runs after the child,
        // `finish_uses`, since package W10.)
        None
    }

    /// Why the core cannot follow a use of PRODUCTION at a home system for these seats: [`production_declined`], except for what the home
    /// production runs itself (Hacan's breakthrough on three ships).
    pub(crate) fn home_declined(gs: &GameState, producer: &PlayerId) -> Option<&'static str> {
        if gs.players.iter().any(|p| [crate::factions::bastion::MODULE.alias, crate::factions::crimson::MODULE.alias].contains(&p.faction.as_str())) {
            return Some("the Last Bastion or the Crimson Rebellion at the table (units placed away from the producing system)");
        }
        if crate::production_view::unported_triggered(gs, producer) {
            return Some("a table whose seats the production windows do not carry (Keleres, Deepwrought)");
        }
        // (The Prophecy of Ixth is discarded by the core window at the end of the use, package W13. A planet value swapped by Hegemonic Trade
        // Policy is declared in the tactical action's production and ends with it: none lingers into a use outside it.)
        None
    }

    /// The facts of the use of PRODUCTION `player` would open in `system` outside a tactical action (`ProductionWindow::new`: the Warfare
    /// secondary's home production), `Err` with the census reason where the core cannot follow it.
    pub(crate) fn home_facts(gs: &GameState, content: &ti4_content::ContentStore, sources: SourceSet, seat: Seat, player: &PlayerId, system: &ti4_model::id::SystemId) -> Result<ti4_engine_core::kernel::children::HomeFacts, &'static str> {
        if let Some(why) = home_declined(gs, player) {
            return Err(why);
        }
        let window = crate::production::ProductionWindow::new(gs, content, sources.clone(), player, system);
        let core = window.to_core(gs, content, sources, seat, false)?;
        Ok(ti4_engine_core::kernel::children::HomeFacts {
            produce: ti4_engine_core::kernel::children::ProduceFacts {
                who: seat,
                system: core.system,
                limit: core.limit,
                max_unit_cost: core.max_unit_cost,
                small_exemptions: core.small_exemptions,
                discount: core.discount_remaining,
                fleet_limit: core.fleet_limit,
                armada: core.armada,
                flagship_free: core.flagship_free,
                buildable: core.buildable,
            },
            agent_rules: core.agent_rules,
        })
    }

    /// The facts of the use `player` would open in `system` with `limit` units, as the string window opens it (`for_ability`).
    #[allow(clippy::too_many_arguments, reason = "the position, the seat, the system, the limit and the cost ceiling")]
    fn produce_facts(gs: &GameState, content: &ti4_content::ContentStore, sources: SourceSet, seat: Seat, player: &PlayerId, system: &ti4_model::id::SystemId, limit: i64, cap: Option<i64>) -> Option<ti4_engine_core::kernel::children::ProduceFacts> {
        let window = crate::production::ProductionWindow::for_ability(gs, content, sources, player, system, Some(limit)).with_max_unit_cost(cap);
        let core = match window.to_core(gs, content, sources, seat, false) {
            Ok(core) => core,
            Err(why) => {
                crate::dual::note_bail_here(why);
                return None;
            }
        };
        Some(ti4_engine_core::kernel::children::ProduceFacts {
            who: seat,
            system: core.system,
            limit: core.limit,
            max_unit_cost: core.max_unit_cost,
            small_exemptions: core.small_exemptions,
            discount: core.discount_remaining,
            fleet_limit: core.fleet_limit,
            armada: core.armada,
            flagship_free: core.flagship_free,
            buildable: core.buildable,
        })
    }

    /// The effects at the end of each use (`ProductionWindow::finish_use` for a production by an ability, in its order): a planet-value swap
    /// lapses, Hacan's breakthrough pays a fleet token for three non-fighter ships, the Prophecy of Ixth is discarded unless two fighters
    /// were made; then the announcement the string stages for a use that made something (`UNITS_PRODUCED`, no timing handle).
    fn announce_uses(gs: &mut GameState, content: &ti4_content::ContentStore, sources: SourceSet, player: &PlayerId, uses: &[(ti4_engine_core::generated::System, ti4_engine_core::kernel::children::Made)]) {
        for (system, made) in uses {
            crate::production::end_value_swap(gs);
            let produced: Vec<(ti4_model::id::UnitTypeId, String)> = made
                .as_slice()
                .iter()
                .map(|(unit, planet)| (ti4_model::id::UnitTypeId::new(unit.as_str()), planet.map_or(crate::production::SPACE, |p| p.as_str()).to_owned()))
                .collect();
            crate::breakthroughs::on_production_finished(gs, content, sources, player, &produced);
            let types = ti4_content::units::catalogue(content, sources);
            let fighters = produced.iter().filter(|(kind, _)| types.get(kind.as_str()).is_some_and(ti4_content::units::UnitType::is_fighter)).count();
            crate::laws::prophecy_after_production(gs, player, fighters);
            if made.as_slice().is_empty() {
                continue;
            }
            let units: Vec<serde_json::Value> = made
                .as_slice()
                .iter()
                .map(|(unit, planet)| serde_json::json!({ "unit_type": unit.as_str(), "place": planet.map_or(crate::production::SPACE, |p| p.as_str()) }))
                .collect();
            let mut payload = std::collections::BTreeMap::new();
            payload.insert("player".to_owned(), player.to_string().into());
            payload.insert("system".to_owned(), system.as_str().into());
            payload.insert("source".to_owned(), "ability".into());
            payload.insert("count".to_owned(), made.as_slice().len().into());
            payload.insert("units".to_owned(), serde_json::Value::Array(units));
            crate::supply::stage_event(gs, "UNITS_PRODUCED", &payload);
        }
    }

    /// The core half of Arborec's Letani Miasmiala (`factions::arborec::use_leader`; the value: some unit was made). `systems` are the
    /// systems holding a ground force of the seat, `limit` the units per use. The effects at the end of a use the child does not run are
    /// declined (see [`production_declined`]).
    pub fn arborec_hero(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, systems: &[ti4_model::id::SystemId], limit: i64, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::leader_production::ArborecHero;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if let Some(why) = production_declined(gs, &[player]) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let facts: Vec<_> = systems.iter().map(|system| produce_facts(gs, content, sources, seat, player, system, limit, None)).collect::<Option<_>>()?;
        let before = gs.clone();
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        let kernel = crate::production_view::kernel();
        let mut driver = Driver::new(ArborecHero::new(seat, facts), Resolver::new(&seats, Some(seat)), table, 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PRODUCE, |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the production window, or a system has no facts");
            return None;
        }
        if !staged.is_empty() && driver.window.used() {
            crate::dual::note_bail_here("staged events of the placements would interleave with the announcements of the uses");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        announce_uses(gs, content, sources, player, &driver.window.made);
        Some(CoreWindow { run: Run::new(driver.window.used()).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of Muaat's Umbat (`factions::muaat::umbat`; the value: the agent was spent). `candidates` are the systems holding a war
    /// sun or the flagship of each seat (the string read; the facts of each use are computed here). A copy through Ssruu is the caller's.
    pub fn muaat_agent(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::rules::leader_production::Umbat;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let everyone: Vec<&PlayerId> = gs.players.iter().map(|p| &p.id).collect();
        if let Some(why) = production_declined(gs, &everyone) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let types = ti4_content::units::catalogue(content, sources);
        let mut facts = Vec::new();
        for owner in &everyone {
            let Some(owner_seat) = r.seat(owner.as_str()) else { continue };
            for (system, board) in &gs.board {
                let holds = board.units.iter().any(|u| &u.owner == *owner && types.get(u.type_id.as_str()).is_some_and(|k| matches!(k.base_type(), "warsun" | "flagship")));
                if holds {
                    facts.push(produce_facts(gs, content, sources, owner_seat, owner, system, 2, Some(4))?);
                }
            }
        }
        let before = gs.clone();
        let mut table = AbilityTable::default();
        ti4_engine_core::driver::children::register(&mut table);
        let kernel = crate::production_view::kernel();
        let mut driver = Driver::new(Umbat::new(seat, facts), Resolver::new(&seats, Some(seat)), table, 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PRODUCE, |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the production window, or a use has no facts");
            return None;
        }
        // A copy through Ssruu is atomic (`umbat_string`): a decision that fails part-way (an answer that is no option) leaves the position as
        // it was and the agent unspent. The recorded decisions stop before the window is over; the position is the one before.
        if driver.window.copied && !driven.finished {
            *gs = before;
            return Some(CoreWindow { run: Run::new(false).with_events_vec(Vec::new()), questions: driven.rendered });
        }
        let made_any = driver.window.made.as_ref().is_some_and(|(_, made)| !made.as_slice().is_empty());
        if !staged.is_empty() && made_any {
            crate::dual::note_bail_here("staged events of the placements would interleave with the announcement of the use");
            return None;
        }
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        if let (Some((system, made)), Some(target)) = (driver.window.made.clone(), driver.window.target) {
            // The use is the chosen player's: the announcement names them.
            announce_uses(gs, content, sources, &PlayerId::new(r.seat_text(target)), &[(system, made)]);
        }
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    const PLAYERS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    /// The core half of `leaders::jolnar_hero` (Rin). `None` where `apply_unit_upgrades` would change the
    /// position (the units are not in the form the owned upgrades demand: the string re-forms them after a swap
    /// and the core does not), and as [`xxcha_agent`] otherwise.
    pub fn jolnar_hero(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, sources: SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
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
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let kernel = kernel(sources)?;
        let before = gs.clone();
        let mut driver = Driver::new(RinHero::new(seat, order, by_alias), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PLAYERS, |core, _| {
            crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script)
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
        Some(CoreWindow { run: Run::new(driver.window.used).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `leaders::hacan_agent`. `None` where a commodity change causes events the core does not
    /// hold: a siphoning Firmament plot, a Cabal at the table, a Trade Agreement in play.
    pub fn hacan_agent(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if let Some(why) = replenish_window_unported(gs) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(HacanAgent::with_source(seat, r.code("trade_agreement")), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let mut staged = Vec::new();
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), HACAN, |core, _| {
            let driven = crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, rendered| {
                rendered.details.insert("commodities".to_owned(), crate::strategy_cards::commodity_standing(&before, content, player));
            });
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
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
    pub fn sol_commander(
        _: &mut GameState,
        _: &PlayerId,
        _: &ti4_model::id::SystemId,
        _: &ti4_model::id::PlanetId,
        _: &str,
        _: usize,
    ) -> Option<bool> {
        None
    }
    pub fn sol_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn nomad_hero(_: &mut GameState, _: &PlayerId) -> Option<bool> {
        None
    }
    pub fn titans_hero(_: &mut GameState) -> Option<bool> {
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
    pub fn saar_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: bool,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn ghost_hero(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &[&str],
        _: &[&str],
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn arborec_agent(
        _: &mut GameState,
        _: &PlayerId,
        _: &[(PlayerId, ti4_model::id::UnitTypeId)],
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn arborec_hero(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: &[ti4_model::id::SystemId],
        _: i64,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn muaat_agent(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: &PlayerId,
        _: ti4_model::content_types::SourceSet,
        _: &crate::dual_window::Recording,
    ) -> Option<crate::dual::CoreWindow<bool>> {
        None
    }
    pub fn nekro_agent(
        _: &mut GameState,
        _: &PlayerId,
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
    pub fn xxcha_hero_te(
        _: &mut GameState,
        _: &PlayerId,
        _: &crate::leaders::HeroTeFacts,
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
