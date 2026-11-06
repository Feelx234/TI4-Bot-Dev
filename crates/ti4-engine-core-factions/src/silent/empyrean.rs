//! empyrean: Acamar (the agent), Voidwatch (the technology) and Xuange's token return (the commander) (`factions::empyrean_units::acamar`,
//! `factions::empyrean::voidwatch`, `factions::borrowed_commanders_b`). All are registered for every seat; the conditions limit them to the holder.
//!
//! * Acamar (MOVEMENT_FINISHED, optional): after a player moves ships into a system with no planets the Empyrean may exhaust the agent, and that player gains 1 command token.
//! * Voidwatch (MOVEMENT_FINISHED): after another player moves ships into a system with a unit of the holder they give the holder 1 promissory note from their hand,
//!   which the mover chooses.
//! * Xuange (SHIP_MOVED, optional): after another player moves ships into a system with 1 of the holder's command tokens the holder may return that token.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::strategy::{gain_answer, gain_pool_choice, gain_pools};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, PromissoryNote, System, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::{fx, fx_strategy, Kernel};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{board, leaders, promissory as qn};
use ti4_engine_core::state::{CoreState, EventKind, EventRec, Key, LeaderStatus, NoteKey};

use crate::common::scope;

/// `leader:*:empyreanagent:MOVEMENT_FINISHED:after`.
pub const ACAMAR: Slot = Slot(43_040);
/// `technology:*:vw:MOVEMENT_FINISHED:after`.
pub const VOIDWATCH: Slot = Slot(43_041);
/// `leader:*:empyreancommander:SHIP_MOVED:after`.
pub const XUANGE: Slot = Slot(43_042);

// -- Acamar ----------------------------------------------------------------------------------------------------------------------

/// `acamar_window`: the mover, when the agent is readied, ships moved and the system has no planets.
fn acamar_window(s: &CoreState, k: &Kernel, owner: Seat, event: &EventRec) -> Option<Seat> {
    if leaders::status(s, owner, Leader::Acamar) != Some(LeaderStatus::Readied) || event.payload.int(Key::ShipsMoved).is_none_or(|n| n <= 0) {
        return None;
    }
    let mover = event.payload.seat(Key::Player)?;
    s.players().by_seat(mover)?;
    let system = event.payload.system(Key::System)?;
    (scope(k).system(system).is_some() && system.data().planets.is_empty()).then_some(mover)
}

fn acamar_when(c: &CondCx<'_>) -> bool {
    acamar_window(c.state, c.kernel, c.def.owner, c.event).is_some()
}

fn acamar(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(mover) = ({
        let (s, k) = eff.read();
        acamar_window(s, k, me, eff.event)
    }) else {
        return Ok(());
    };
    {
        let (s, k) = eff.state_and_kernel();
        if leaders::status(s, me, Leader::Acamar) != Some(LeaderStatus::Readied) {
            return Ok(());
        }
        fx_strategy::exhaust_agent(s, k, me, Leader::Acamar);
    }
    if let Some(split) = gain_answer(eff.ask(&gain_pool_choice(mover, 1)), 1) {
        for pool in gain_pools(split) {
            fx::gain_token(eff.state(), mover, pool, 1);
        }
    }
    Ok(())
}

// -- Voidwatch ---------------------------------------------------------------------------------------------------------------------

/// A note of a hand the mover can give: a promissory note, or the seat's own Support for the Throne.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Given {
    Note(NoteKey),
    Support(Faction),
}

fn seat_of(s: &CoreState, faction: Faction) -> Option<Seat> {
    s.players().iter().find(|p| p.faction == faction).map(|p| p.seat)
}

/// `promissory::commander_unlocked`: any commander of the seat is unlocked.
fn commander_unlocked(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.iter().any(|(l, st)| *st == LeaderStatus::Unlocked && l.data().kind == ti4_engine_core::generated::LeaderType::Commander))
}

/// `hand`: the notes in hand that convey something, in the text order of their ids, then the seat's Support for the Throne when it is not lent.
/// `None`: the adapter gave no order of the notes.
fn hand(c: &CondCx<'_>, who: Seat) -> Option<Vec<Given>> {
    hand_of(c.state, c.facts, who)
}

fn hand_of(s: &CoreState, facts: &ti4_engine_core::kernel::ability::AdapterFacts, who: Seat) -> Option<Vec<Given>> {
    let order = facts.plays.as_ref()?.note_order;
    let mut notes: Vec<NoteKey> = qn::held_by(s, who)
        .into_iter()
        .filter(|k| !s.cards().promissory_faceup.contains_key(k))
        .filter(|k| k.note != PromissoryNote::Alliance || seat_of(s, k.faction).is_some_and(|owner| commander_unlocked(s, owner)))
        .collect();
    notes.sort_by(|a, b| order(*a, *b));
    let mut out: Vec<Given> = notes.into_iter().map(Given::Note).collect();
    if let Some(faction) = s.players().by_seat(who).map(|p| p.faction)
        && !s.trade().support_holders.contains_key(&who)
    {
        out.push(Given::Support(faction));
    }
    Some(out)
}

/// `watch_ready`'s part that needs no hand: another player moved ships into a system with a unit of the holder.
fn watch_ready(s: &CoreState, owner: Seat, event: &EventRec) -> Option<(Seat, System)> {
    let (player, system) = (event.payload.seat(Key::Player)?, event.payload.system(Key::System)?);
    let here = board::space_units(s, system).iter().any(|u| u.owner() == owner) || board::planets_of(s, system).any(|planet| board::planet_units(s, planet).iter().any(|u| u.owner() == owner));
    (ti4_engine_core::query::reach::technology_text(s, owner, Technology::Voidwatch) && player != owner && event.payload.int(Key::ShipsMoved).unwrap_or(0) > 0 && here).then_some((player, system))
}

fn watch_when(c: &CondCx<'_>) -> bool {
    let Some((player, _)) = watch_ready(c.state, c.def.owner, c.event) else { return false };
    match hand(c, player) {
        Some(notes) => !notes.is_empty(),
        None => c.unknown(Unknown::MissingFact),
    }
}

fn voidwatch(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((mover, _)) = watch_ready(eff.read().0, me, eff.event) else { return Ok(()) };
    let notes = match hand_of(eff.read().0, eff.adapter_facts(), mover) {
        Some(notes) if !notes.is_empty() => notes,
        Some(_) => return Ok(()),
        None => {
            eff.unknown(Unknown::MissingFact);
            return Ok(());
        }
    };
    let choice = Choice {
        actor: mover,
        ask: Ask::Reaction(RAsk::G(GAsk::Voidwatch)),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Technology(Technology::Voidwatch)),
            subtype: Subtype::Reaction(RSub::G(GSub::Voidwatch)),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: notes
            .iter()
            .map(|given| match given {
                Given::Note(key) => Opt::Reaction(ROpt::G(GOpt::Note(*key))),
                Given::Support(faction) => Opt::Reaction(ROpt::G(GOpt::Support(*faction))),
            })
            .collect::<Opts>(),
    };
    let picked = match eff.ask(&choice) {
        Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Note(key)))) if notes.contains(&Given::Note(key)) => Given::Note(key),
        Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Support(faction)))) if notes.contains(&Given::Support(faction)) => Given::Support(faction),
        _ => return Ok(()),
    };
    match picked {
        Given::Note(key) => fx_strategy::take_note(eff.state(), me, key),
        Given::Support(faction) => {
            let _ = ti4_engine_core::rules::trade::receive_support(eff.state(), me, faction);
        }
    }
    Ok(())
}

// -- Xuange ---------------------------------------------------------------------------------------------------------------------

/// `empyrean_ready`: the mover is another player, the holder has the commander's ability and a command token in the system.
fn xuange_ready(s: &CoreState, owner: Seat, event: &EventRec) -> Option<System> {
    let (mover, system) = (event.payload.seat(Key::Player)?, event.payload.system(Key::System)?);
    (mover != owner && leaders::has_commander_ability_in(s, owner, Leader::Xuange) && board::has_token(s, system, owner)).then_some(system)
}

fn xuange_when(c: &CondCx<'_>) -> bool {
    xuange_ready(c.state, c.def.owner, c.event).is_some()
}

fn xuange(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(system) = xuange_ready(eff.read().0, me, eff.event) {
        fx::remove_command_token(eff.state(), me, system);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    use EventKind as E;
    t.add(Spec::new(VOIDWATCH, AbilityKind::Technology, E::MovementFinished, Relation::After, voidwatch).when(watch_when));
    t.add(Spec::new(ACAMAR, AbilityKind::Leader, E::MovementFinished, Relation::After, acamar).optional().when(acamar_when));
}

/// Xuange's return of a token: the string registers the acquired commanders (`borrowed_commanders_b`) after every faction module and the technologies.
pub fn register_last(t: &mut AbilityTable) {
    t.add(Spec::new(XUANGE, AbilityKind::Leader, EventKind::ShipMoved, Relation::After, xuange).optional().when(xuange_when));
}
