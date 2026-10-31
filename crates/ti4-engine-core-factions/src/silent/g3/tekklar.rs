//! sardakk: Tekklar Legion, the promissory note of the Sardakk N'orr (`factions::sardakk::{tekklar_legion, tekklar_cleanup_ended, tekklar_cleanup_turn}`):
//! "At the start of an invasion combat: apply +1 to the result of each of your units' rolls during this combat. If your opponent is the N'orr player, apply -1
//! to the result of each of his units' rolls during this combat. Then, return this card to the N'orr player."
//!
//! Three rules. The holder's play (`promissory:*:tekklar:GROUND_COMBAT_STARTED:after`, optional) writes the mark `sardakk:tekklar:combat` = `system|planet|holder|norr`
//! the roll shifts of `cd_registry::sardakk::query` read, and the note goes home. The mark ends with the combat it was played into
//! (`...tekklar_end:GROUND_COMBAT_ENDED:after`) or, if that combat never ended, at the next turn (`...tekklar_turn:TURN_BEGAN:after`); both clean-ups are the N'orr seat's
//! (a game with no N'orr player never has them, a game with one has them once).

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, Payload};

use crate::common::mark;

/// Tekklar Legion, played (`promissory:*:tekklar:GROUND_COMBAT_STARTED:after`).
pub const TEKKLAR: Slot = Slot(42_002);
/// The bonus ends with its combat (`promissory:*:tekklar_end:GROUND_COMBAT_ENDED:after`).
pub const TEKKLAR_END: Slot = Slot(42_003);
/// The bonus of a combat that never ended ends with the turn (`promissory:*:tekklar_turn:TURN_BEGAN:after`).
pub const TEKKLAR_TURN: Slot = Slot(42_004);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::TekklarLegion, faction: Faction::SardakkNorr };

/// `held_foreign(.., "tekklar") > 0`: the seat holds the N'orr's note and is not the N'orr.
fn holds(s: &CoreState, who: Seat) -> bool {
    s.cards().promissory.get(&NOTE) == Some(&who) && s.players().by_seat(who).is_some_and(|p| p.faction != Faction::SardakkNorr)
}

/// `fought_in`: the seat is a side of the combat the event names.
fn fought_in(p: &Payload, who: Seat) -> bool {
    p.seat(Key::Attacker) == Some(who) || p.seat(Key::Defender) == Some(who)
}

fn play_condition(c: &CondCx<'_>) -> bool {
    fought_in(&c.event.payload, c.def.owner) && holds(c.state, c.def.owner)
}

fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let p = eff.event.payload.clone();
    let (Some(system), Some(planet), Some(attacker), Some(defender)) = (p.system(Key::System), p.planet(Key::Planet), p.seat(Key::Attacker), p.seat(Key::Defender)) else { return Ok(()) };
    if !holds(eff.state(), me) {
        return Ok(());
    }
    let opponent = if attacker == me { defender } else { attacker };
    let norr = if eff.state().players().by_seat(opponent).is_some_and(|p| p.faction == Faction::SardakkNorr) { MarkVal::Atom(Atom::Seat(opponent)) } else { MarkVal::Atom(Atom::Absent) };
    let record = MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Planet(planet)), MarkVal::Atom(Atom::Seat(me)), norr]);
    eff.state().marks_mut().tail.edit().insert(MarkKey::SardakkTekklarCombat(), record);
    fx::give_back_note(eff.state(), NOTE);
    Ok(())
}

/// `is_norr_seat` and a mark to clean up.
fn cleanup_condition(c: &CondCx<'_>) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.faction == Faction::SardakkNorr) && mark(c.state, MarkKey::SardakkTekklarCombat()).is_some()
}

fn end(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (Some(system), Some(planet)) = (eff.event.payload.system(Key::System), eff.event.payload.planet(Key::Planet)) else { return Ok(()) };
    let here = matches!(mark(eff.state(), MarkKey::SardakkTekklarCombat()), Some(MarkVal::Rec(f)) if matches!(f.as_slice(), [MarkVal::Atom(Atom::System(s)), MarkVal::Atom(Atom::Planet(p)), ..] if *s == system && *p == planet));
    if here {
        eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkTekklarCombat());
    }
    Ok(())
}

fn turn(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkTekklarCombat());
    Ok(())
}

/// The play, registered where the string's Sardakk module comes among the combat windows (`g3::register_after_ca_combat`).
pub fn register_play(t: &mut AbilityTable) {
    t.add(Spec::new(TEKKLAR, AbilityKind::Promissory, EventKind::GroundCombatStarted, Relation::After, play).optional().when(play_condition));
}

/// The clean-ups, registered after the rules that wait on a card (`TURN_BEGAN` of the Arborec's Letani Warriors and Infantry II come first).
pub fn register_cleanups(t: &mut AbilityTable) {
    t.add(Spec::new(TEKKLAR_END, AbilityKind::Promissory, EventKind::GroundCombatEnded, Relation::After, end).when(cleanup_condition));
    t.add(Spec::new(TEKKLAR_TURN, AbilityKind::Promissory, EventKind::TurnBegan, Relation::After, turn).when(cleanup_condition));
}
