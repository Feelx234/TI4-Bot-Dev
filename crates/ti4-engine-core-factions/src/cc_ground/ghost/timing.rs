//! ghost: the timing abilities of the activation window (lane M6c).
//!
//! Emissary Taivra (`leader:*:ghostagent:SYSTEM_ACTIVATED:after`): "After a player activates a system that contains a non-delta wormhole:
//! you may exhaust this card; if you do, that system is adjacent to all other systems that contain a wormhole during this tactical
//! action." The adjacency is `ghost::movement::linked_systems`, keyed on the mark this writes.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, System, Unit, Wormhole};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};
use ti4_engine_core::tables::{printed_wormholes, wormhole_bit, WH_ALPHA_BETA};

/// Emissary Taivra.
pub const TAIVRA: Slot = Slot(22001);

const AGENT: Leader = Leader::EmissaryTaivra;
/// The delta bit (the kind the agent does not count).
fn delta() -> u8 {
    wormhole_bit(ti4_engine_core::generated::Wormhole::Delta)
}

pub(crate) fn ready(s: &CoreState, owner: Seat) -> bool {
    s.players().by_seat(owner).is_some_and(|p| p.leaders.status.get(&AGENT) == Some(&LeaderStatus::Readied))
}

/// `hooks_movement::wormholes_at`: the printed wormholes, the Creuss tokens, the ion storm's face, the unlocked Nexus and the pieces' own, as a bit set.
pub(crate) fn wormholes_at(s: &CoreState, k: &Kernel, system: System) -> u8 {
    let mut kinds = printed_wormholes(system);
    for (kind, at) in s.explore().wormhole_tokens.iter() {
        if *at == system {
            kinds |= wormhole_bit(*kind);
        }
    }
    if let Some((at, face)) = s.meta().ion_storm
        && at == system
    {
        kinds |= wormhole_bit(face);
    }
    if s.meta().flags.has(ti4_engine_core::state::MetaFlags::NEXUS_UNLOCKED) && system == System::MalliceLockedNexus {
        kinds |= WH_ALPHA_BETA;
    }
    for (at, kind) in k.hooks.movement.extra_wormhole_set(s).as_slice() {
        if *at == system {
            kinds |= wormhole_bit(*kind);
        }
    }
    kinds
}

/// The galaxy of the position, when the map slice is loaded (a position lent without its map has none: the string's `context.galaxy` is `None`).
fn galaxy_of(s: &CoreState) -> Option<&ti4_engine_core::containers::Cow<ti4_engine_core::galaxy::CoreGalaxy>> {
    if s.loaded.has(ti4_engine_core::state::Slice::Map) { s.galaxy() } else { None }
}

/// `agent_target_usable`: the system holds a non-delta wormhole and the map has another system with one; false without a map.
pub(crate) fn target_usable(s: &CoreState, k: &Kernel, system: System) -> bool {
    galaxy_of(s).is_some_and(|galaxy| wormholes_at(s, k, system) & !delta() != 0 && galaxy.wormhole_systems().iter().any(|other| ti4_engine_core::ids::system_at(other) != system))
}

fn when(c: &CondCx<'_>) -> bool {
    if !ready(c.state, c.def.owner) {
        return false;
    }
    let Some(system) = c.event.payload.system(Key::System) else { return false };
    target_usable(c.state, c.kernel, system)
}

fn taivra(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let usable = {
        let (state, kernel) = eff.read();
        ready(state, owner) && target_usable(state, kernel, system)
    };
    if !usable {
        return Ok(());
    }
    // Through `leaders::exhaust`, so a watching Nomad's Temporal Command Suite hears it.
    let (state, kernel) = eff.state_and_kernel();
    ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, owner, AGENT);
    let seq = eff.state().meta().activation_seq;
    eff.state().marks_mut().tail.edit().insert(MarkKey::GhostAgentLink(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(system))]));
    Ok(())
}

// -- Icarus Drive (the mech) --------------------------------------------------------------------------

/// Icarus Drive (`unit:*:ghost_mech:SYSTEM_ACTIVATED:after`).
pub const ICARUS: Slot = Slot(22002);

/// The Creuss tokens (`tokens::CREUSS_TOKENS`: alpha, then beta).
const TOKENS: [Wormhole; 2] = [Wormhole::Alpha, Wormhole::Beta];

/// `mech_destinations`: every system on the hex grid where `owner` has an Icarus Drive mech, paired with each Creuss token not already there
/// (systems in board order, tokens alpha then beta). `None`: no galaxy.
fn mech_destinations(s: &CoreState, owner: Seat) -> Option<Vec<(Wormhole, System)>> {
    use ti4_engine_core::query::board;
    let galaxy = galaxy_of(s)?;
    let mut found = Vec::new();
    for (system, cell) in s.board().systems() {
        let present = cell.units.as_slice().iter().any(|u| u.owner() == owner && u.kind() == Unit::IcarusDrive)
            || board::planets_of(s, system).any(|planet| board::planet_units(s, planet).iter().any(|u| u.owner() == owner && u.kind() == Unit::IcarusDrive));
        if !present || galaxy.coord_of(system).is_none() {
            continue;
        }
        for kind in TOKENS {
            if !s.explore().wormhole_tokens.iter().any(|(k, at)| *k == kind && *at == system) {
                found.push((kind, system));
            }
        }
    }
    Some(found)
}

fn mech_when(c: &CondCx<'_>) -> bool {
    mech_destinations(c.state, c.def.owner).is_some_and(|found| !found.is_empty())
}

/// "Remove this unit from the game board to place or move a Creuss wormhole token into this system": ask the token and the system (not
/// declinable once the ability is chosen), place it, remove one mech of that system (a planet first, then the space area).
fn icarus_drive(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
    let me = eff.owner();
    let Some(destinations) = mech_destinations(eff.state(), me) else { return Ok(()) };
    if destinations.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::IcarusDrive),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(eff.codes().activation.ghost_mech)),
            subtype: Subtype::Reaction(RSub::IcarusDrive),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: destinations.iter().map(|(kind, system)| Opt::Reaction(ROpt::Token(*kind, *system))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::Token(kind, system))) = eff.ask(&choice) else { return Ok(()) };
    if !destinations.contains(&(kind, system)) {
        return Ok(());
    }
    // Place first, then remove the mech.
    ti4_engine_core::kernel::fx_explore::place_wormhole_token(eff.state(), kind, system);
    let planets: Vec<_> = ti4_engine_core::query::board::planets_of(eff.state(), system).collect();
    for planet in planets {
        let cell = eff.state().board_mut().planet_mut(planet);
        if let Some(at) = cell.units.as_slice().iter().position(|u| u.owner() == me && u.kind() == Unit::IcarusDrive) {
            cell.units.remove(at);
            return Ok(());
        }
    }
    let cell = eff.state().board_mut().sys_mut(system);
    if let Some(at) = cell.units.as_slice().iter().position(|u| u.owner() == me && u.kind() == Unit::IcarusDrive) {
        cell.units.remove(at);
    }
    Ok(())
}

/// Ssruu copying Emissary Taivra (`leader:{lender faction}:{lender}:yssarilagent:ghostagent:SYSTEM_ACTIVATED:after`).
pub const TAIVRA_COPY: Slot = Slot(22003);

/// The string's static roster: a lender that holds the agent card and a borrower that holds Ssruu.
fn lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    let holds = |who: Seat, card: Leader| s.players().by_seat(who).is_some_and(|p| p.leaders.status.contains_key(&card));
    holds(lender, AGENT) && holds(borrower, Leader::Ssruu)
}

fn copy_when(c: &CondCx<'_>) -> bool {
    crate::cd_registry::yssaril::ssruu::borrowable(c.state, c.def.owner, c.def.pair_source(), AGENT)
        && c.event.payload.system(Key::System).is_some_and(|system| target_usable(c.state, c.kernel, system))
}

/// The same link, with Ssruu exhausted for it.
fn taivra_copy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let usable = {
        let (state, kernel) = eff.read();
        crate::cd_registry::yssaril::ssruu::borrowable(state, me, lender, AGENT) && target_usable(state, kernel, system)
    };
    if !usable {
        return Ok(());
    }
    let (state, kernel) = eff.state_and_kernel();
    if ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, me, Leader::Ssruu) {
        let seq = eff.state().meta().activation_seq;
        eff.state().marks_mut().tail.edit().insert(MarkKey::GhostAgentLink(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(system))]));
    }
    Ok(())
}

/// The abilities of the activation window (`activation::register`): the mech, then the agent and its Ssruu copy (the string's order).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(ICARUS, AbilityKind::Unit, EventKind::SystemActivated, Relation::After, icarus_drive).optional().when(mech_when));
    t.add(Spec::new(TAIVRA, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, taivra).optional().when(when));
    t.add(Spec::new(TAIVRA_COPY, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, taivra_copy).applies(Applies::Faction(Faction::TheGhostsOfCreuss)).pair(lends).optional().when(copy_when));
}

// -- Sai Seravus (the commander, lane M6d) ----------------------------------------------------------------------------------------------

/// Sai Seravus, the record of a ship that moved through a wormhole (`leader:*:ghostcommander:SHIP_MOVED:after`).
pub const SAI_SHIP_MOVED: Slot = Slot(50_060);
/// Sai Seravus, the fighters once movement is finished (`leader:*:ghostcommander:MOVEMENT_FINISHED:after`).
pub const SAI_FINISHED: Slot = Slot(50_061);

/// "After your ships move: for each ship that has a capacity value and moved through 1 or more wormholes": the ship is in the active system, its
/// route crossed a wormhole and it carries something.
fn sai_moved_when(c: &CondCx<'_>) -> bool {
    let (s, me) = (c.state, c.def.owner);
    let p = &c.event.payload;
    p.seat(Key::Player) == Some(me)
        && p.system(Key::System) == s.meta().active_system
        && ti4_engine_core::query::leaders::has_commander_ability_in(s, me, Leader::SaiSeravus)
        && matches!(p.get(Key::Wormholes), Some(ti4_engine_core::state::Val::Int(hops)) if hops > 0)
        && p.unit(Key::Unit).is_some_and(|u| crate::common::scope(c.kernel).unit(u).is_some_and(|d| d.ship && ti4_engine_core::query::fleet::capacity(u) > 0))
}

fn sai_key(s: &CoreState, me: Seat) -> MarkKey {
    MarkKey::PrivateGhostCommander(me, ti4_engine_core::keys::DecimalTextOrd(s.meta().activation_seq))
}

fn sai_count(s: &CoreState, me: Seat) -> u32 {
    match s.marks().tail.get(&sai_key(s, me)) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

fn sai_moved(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let key = sai_key(eff.state(), me);
    let now = sai_count(eff.state(), me).saturating_add(1);
    eff.state().marks_mut().tail.edit().insert(key, MarkVal::Atom(Atom::Num(now)));
    Ok(())
}

fn sai_finished_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && ti4_engine_core::query::leaders::has_commander_ability_in(c.state, me, Leader::SaiSeravus) && sai_count(c.state, me) > 0
}

/// `commander_fighters`: the fighters that fit the unused capacity and the reinforcements, never more than the ships recorded.
fn sai_fighters(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, me: Seat, system: System) -> usize {
    use ti4_engine_core::generated::UnitBaseType;
    if !ti4_engine_core::query::leaders::has_commander_ability_in(s, me, Leader::SaiSeravus) {
        return 0;
    }
    let sc = crate::common::scope(k);
    let free = ti4_engine_core::query::fleet::standing(s, sc, me, system, 0, None, &|_| false).capacity_free();
    let plastic = crate::cd_registry::yin::timing::placed_form(s, sc, me, UnitBaseType::Fighter).map_or(0, |fighter| ti4_engine_core::query::fleet::remaining(s, sc, me, fighter, 0));
    usize::try_from(sai_count(s, me)).unwrap_or(0).min(usize::try_from(free).unwrap_or(0)).min(usize::try_from(plastic).unwrap_or(0))
}

fn sai_finished(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::activation::{Ask as MAsk, Opt as MOpt, Subtype as MSub};
    use ti4_engine_core::choice::generic::Content;
    use ti4_engine_core::choice::reaction::{Ask as RAsk, Opt as ROpt, Subtype as RSub};
    use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let key = sai_key(eff.state(), me);
    let available = {
        let (s, k) = eff.read();
        sai_fighters(s, k, me, system)
    };
    if available == 0 {
        eff.state().marks_mut().tail.edit().remove(&key);
        return Ok(());
    }
    let mut opts: Opts = (1..=available).filter_map(|n| u8::try_from(n).ok()).map(|n| Opt::Reaction(ROpt::Act(MOpt::Fighters(n)))).collect();
    opts.push(Opt::Decline);
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::Act(MAsk::SaiSeravus { system })),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(eff.codes().activation.ghost_commander)),
            subtype: Subtype::Reaction(RSub::Act(MSub::SaiSeravus)),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    if let Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Fighters(n)))) = eff.ask(&choice)
        && usize::from(n) <= available
    {
        let sc = crate::common::scope(eff.kernel());
        let (s, k) = eff.state_and_kernel();
        let form = crate::cd_registry::yin::timing::placed_form_overridden(s, k, sc, me, ti4_engine_core::generated::UnitBaseType::Fighter);
        let _ = ti4_engine_core::kernel::fx::place_counted(s, k, sc, me, system, None, form, usize::from(n));
    }
    eff.state().marks_mut().tail.edit().remove(&key);
    Ok(())
}

/// The commander's two abilities (the string's order in `ghost::timing_abilities`; the abilities of other windows of the faction are not in these
/// buckets).
pub fn register_events(t: &mut AbilityTable) {
    t.add(Spec::new(SAI_SHIP_MOVED, AbilityKind::Leader, EventKind::ShipMoved, Relation::After, sai_moved).when(sai_moved_when));
    t.add(Spec::new(SAI_FINISHED, AbilityKind::Leader, EventKind::MovementFinished, Relation::After, sai_finished).when(sai_finished_when));
}
