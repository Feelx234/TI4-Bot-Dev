//! arborec: Bioplasmosis (`factions::arborec::bioplasmosis`): "At the end of the status phase, you may remove any number of infantry from planets you control and place them on 1 or more planets you control
//! in the same or adjacent systems." Optional (the window's own use / decline is the "may"). Each infantry is one question ("move one from X to Y, or stop"), asked against the infantry still unmoved;
//! nothing is moved until the player has stopped, and an infantry already moved is not moved again. Registered for the Arborec seats (`technology:*:bio:STATUS_PHASE_ENDED:after`; the technology is the Arborec's own).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, System, Technology, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, PlanetFlags, SysFlags};

/// Bioplasmosis (`technology:*:bio:STATUS_PHASE_ENDED:after`).
pub const BIO: Slot = Slot(42_014);

type Site = (System, Planet);

/// One infantry moved between planets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shift {
    from: Site,
    to: Site,
}

fn infantry_on(s: &CoreState, who: Seat, planet: Planet) -> usize {
    board::planet_units(s, planet).iter().filter(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry).count()
}

/// `infantry_sources`: the infantry the seat has on planets it controls, per planet (the string's key order: the system, then the planet).
fn sources(s: &CoreState, who: Seat) -> Vec<(Site, usize)> {
    board::controlled_planets(s, who).as_slice().iter().filter_map(|(system, planet)| (infantry_on(s, who, *planet) > 0).then(|| ((*system, *planet), infantry_on(s, who, *planet)))).collect()
}

/// `near`: the system itself and, with a map, its neighbours.
fn near(s: &CoreState, from: System, to: System) -> bool {
    from == to || board::neighbours(s, from).is_some_and(|adjacent| adjacent.contains(to.index()))
}

/// `shifts`: every single-infantry move still open, given what has been taken from each planet already.
fn shifts(s: &CoreState, who: Seat, available: &[(Site, usize)]) -> Vec<Shift> {
    let controlled: Vec<Site> = board::controlled_planets(s, who).as_slice().to_vec();
    let mut found = Vec::new();
    for (from, count) in available {
        if *count == 0 {
            continue;
        }
        for to in &controlled {
            if to != from && near(s, from.0, to.0) {
                found.push(Shift { from: *from, to: *to });
            }
        }
    }
    found
}

fn ready(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::Bioplasmosis)) && !shifts(s, who, &sources(s, who)).is_empty()
}

fn condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner)
}

fn move_infantry(s: &mut CoreState, who: Seat, shift: Shift) {
    let from = &mut s.board_mut().planet_mut(shift.from.1).units;
    let Some(at) = from.as_slice().iter().position(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Infantry) else { return };
    let piece = from.as_slice()[at];
    from.remove(at);
    let to = s.board_mut().planet_mut(shift.to.1);
    to.units.push(piece);
    to.flags.set(PlanetFlags::UNITS_ENTRY, true);
    s.board_mut().sys_mut(shift.to.0).flags.set(SysFlags::TOUCHED, true);
}

fn bioplasmosis(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.read().0, me) {
        return Ok(());
    }
    let mut available = sources(eff.read().0, me);
    let mut planned: Vec<Shift> = Vec::new();
    // (Every move takes one infantry from the board: the loop ends.)
    loop {
        let open = shifts(eff.read().0, me, &available);
        if open.is_empty() {
            break;
        }
        let mut opts: Opts = open
            .iter()
            .enumerate()
            .map(|(index, shift)| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Shift { index: u8::try_from(index).unwrap_or(u8::MAX), from: shift.from, to: shift.to }))))
            .collect();
        opts.push(Opt::Decline);
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::Bio { so_far: u8::try_from(planned.len()).unwrap_or(u8::MAX) }))),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Technology(Technology::Bioplasmosis)),
                subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::Bio))),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Shift { index, .. })))) = eff.ask(&choice) else { break };
        let Some(shift) = open.get(usize::from(index)).copied() else { break };
        if let Some((_, count)) = available.iter_mut().find(|(site, _)| *site == shift.from) {
            *count -= 1;
        }
        planned.push(shift);
    }
    for shift in planned {
        move_infantry(eff.state(), me, shift);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(BIO, AbilityKind::Technology, EventKind::StatusPhaseEnded, Relation::After, bioplasmosis).applies(Applies::Faction(Faction::TheArborec)).optional().when(condition));
}
