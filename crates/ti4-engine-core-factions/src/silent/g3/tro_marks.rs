//! sardakk: the clean-ups of the marks the Sardakk's cards leave (T'ro's activation, Sh'val's skip). T'ro: the clean-up of T'ro's activation mark (`factions::sardakk::tactical_unmark`, `leader:*:sardakkagent_turn:TURN_BEGAN:after` and
//! `leader:*:sardakkagent_strategic:STRATEGIC_ACTION_BEGAN:after`). The mark (`MarkKey::SardakkAgentTactical(player)` = the system the player activated) is written by
//! the tactical chain (`driver::windows::tactical::hooks`); a turn that begins, or a strategic action, is no end of a tactical action, so the activation of that player
//! is forgotten. Mandatory, registered for every seat; the condition names a seat that holds the agent card (in any state) and a mark of the event's player.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Leader;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{EventKind, Key, LeaderStatus};

use crate::common::mark;

/// `leader:*:sardakkagent_turn:TURN_BEGAN:after`.
pub const TURN: Slot = Slot(42_005);
/// `leader:*:sardakkagent_strategic:STRATEGIC_ACTION_BEGAN:after`.
pub const STRATEGIC: Slot = Slot(42_006);
/// Sh'val's skip that never reached its commitment (`leader:*:sardakkhero_turn:TURN_BEGAN:after`).
pub const HERO_TURN: Slot = Slot(42_016);
/// The mark itself (`leader:*:sardakkagent_mark:SYSTEM_ACTIVATED:after`).
pub const MARK: Slot = Slot(42_019);

/// `factions::sardakk::tactical_mark`: the seat holds T'ro readied or exhausted (an exhausted card may still be copied by Ssruu), whoever activates. A registered
/// ability like any other: mandatory, so it runs unasked when it is the owner's only eligible ability of the window, and is one more offer (and a pass that skips
/// it) when the owner has another (the string's `pick`).
fn mark_condition(c: &CondCx<'_>) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| matches!(p.leaders.status.get(&Leader::Tro), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
}

/// The system the activating player activated, kept until the end of that tactical action (`MarkKey::SardakkAgentTactical(player)`).
fn remember(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    if let (Some(actor), Some(system)) = (eff.event.payload.seat(Key::Player), eff.event.payload.system(Key::System)) {
        eff.state().marks_mut().tail.edit().insert(MarkKey::SardakkAgentTactical(actor), MarkVal::Atom(Atom::System(system)));
    }
    Ok(())
}

fn condition(c: &CondCx<'_>) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.leaders.status.contains_key(&Leader::Tro))
        && c.event.payload.seat(Key::Player).is_some_and(|actor| mark(c.state, MarkKey::SardakkAgentTactical(actor)).is_some())
}

fn forget(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    if let Some(actor) = eff.event.payload.seat(Key::Player) {
        eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkAgentTactical(actor));
    }
    Ok(())
}

/// `hero_unmark_turn`: "a skip that never reached its commitment must not leak into a later turn": the mark of the skip (`sardakk:hero:skip`) stands. Mandatory, every seat.
fn hero_condition(c: &CondCx<'_>) -> bool {
    mark(c.state, MarkKey::SardakkHeroSkip()).is_some()
}

fn hero_forget(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    eff.state().marks_mut().tail.edit().remove(&MarkKey::SardakkHeroSkip());
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MARK, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, remember).when(mark_condition));
    t.add(Spec::new(TURN, AbilityKind::Leader, EventKind::TurnBegan, Relation::After, forget).when(condition));
    t.add(Spec::new(STRATEGIC, AbilityKind::Leader, EventKind::StrategicActionBegan, Relation::After, forget).when(condition));
    t.add(Spec::new(HERO_TURN, AbilityKind::Leader, EventKind::TurnBegan, Relation::After, hero_forget).when(hero_condition));
}
