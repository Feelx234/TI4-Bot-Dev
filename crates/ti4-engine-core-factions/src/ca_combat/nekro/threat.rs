//! nekro: Galactic Threat (package W9; string `nekro::threat_predict`, `threat_resolved`).
//!
//! "Once per agenda phase, after an agenda is revealed: you may predict aloud the outcome of that agenda" (optional, asks which
//! outcome unless the agenda has one), and "if your prediction is correct, gain 1 technology that is owned by a player who voted
//! how you predicted" (mandatory, after `AGENDA_RESOLVED`; the same question as Technological Singularity's, over every such voter).
//!
//! The marks: `nekro:threat_phase:{owner}` is the round of the last prediction, `nekro:threat:{owner}` is `{agenda_seq}|{outcome}`.
//! Registered for every seat, as the string does (each condition names the Nekro). Not covered: the Z token of the Valefar
//! Assimilator breakthrough (the window bails, as Singularity's does).

use ti4_engine_core::choice::agenda::Frame;
use ti4_engine_core::choice::cards::Ask as CAsk;
use ti4_engine_core::choice::generic::Rule;
use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Outcome};
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, TransientFlags, Val};

use super::singularity::{holds_z, is_nekro, options, take};

/// The prediction (`ability:*:galactic_threat:AGENDA_REVEALED:after`).
pub const PREDICT: Slot = Slot(2009);
/// The payout (`ability:*:galactic_threat:AGENDA_RESOLVED:after`).
pub const PAYOUT: Slot = Slot(2010);

/// `threat_ready`: a Nekro, an agenda with outcomes, and no prediction yet this round.
fn ready(s: &CoreState, who: Seat) -> bool {
    is_nekro(s, who)
        && !s.agenda().choices.is_empty()
        && !matches!(s.marks().tail.get(&MarkKey::NekroThreatPhase(who)), Some(MarkVal::Atom(Atom::Num(round))) if *round == s.meta().round)
}

fn predict_condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner)
}

fn predict(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.read().0, me) {
        return Ok(());
    }
    let (choices, agenda, covert, round, seq) = {
        let s = eff.read().0;
        (s.agenda().choices.as_slice().to_vec(), s.agenda().in_play, s.agenda().covert, s.meta().round, s.meta().agenda_seq)
    };
    let predicted = match (single(&choices), choices.as_slice()) {
        (Some(only), _) => only,
        (None, []) => return Ok(()),
        (None, many) => {
            let Some(agenda) = agenda else { return Ok(()) };
            let choice = Choice {
                actor: me,
                ask: Ask::Cards(CAsk::PredictThreat { frame: Frame { agenda, covert, reveal: false } }),
                ctx: Ctx {
                    origin: Origin::Rule(Rule::R8),
                    subtype: Subtype::Cards(ti4_engine_core::choice::cards::Subtype::PredictOutcome),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: many.iter().map(|o| Opt::Cards(ti4_engine_core::choice::cards::Opt::Outcome(*o))).collect::<Opts>(),
            };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Cards(ti4_engine_core::choice::cards::Opt::Outcome(o))) if many.contains(&o) => o,
                _ => return Ok(()),
            }
        }
    };
    let marks = eff.state().marks_mut().tail.edit();
    marks.insert(MarkKey::NekroThreatPhase(me), MarkVal::Atom(Atom::Num(round)));
    marks.insert(MarkKey::NekroThreat(me), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Outcome(predicted))]));
    Ok(())
}

/// An agenda with one outcome needs no question: the prediction names it (`predicted_outcome`'s `[only]`).
fn single(choices: &[Outcome]) -> Option<Outcome> {
    match choices {
        [only] => Some(*only),
        _ => None,
    }
}

/// The outcome an `AGENDA_RESOLVED` event names.
fn resolved_outcome(p: &Payload) -> Option<Outcome> {
    match p.get(Key::Player)? {
        Val::Seat(seat) => Some(Outcome::Seat(seat)),
        Val::Outcome(outcome) => Some(outcome),
        _ => None,
    }
}

/// `threat_payout` up to the sources: a standing prediction of this agenda that the resolution fulfils (`None`: no).
fn fulfilled(s: &CoreState, nekro: Seat, resolved: &Payload) -> Option<Outcome> {
    if !is_nekro(s, nekro) || s.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED) {
        return None;
    }
    let Some(MarkVal::Rec(held)) = s.marks().tail.get(&MarkKey::NekroThreat(nekro)) else { return None };
    let [MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Outcome(predicted))] = held.as_slice() else { return None };
    (*seq == s.meta().agenda_seq && resolved_outcome(resolved) == Some(*predicted)).then_some(*predicted)
}

/// The players who voted for `outcome` on the agenda being resolved and have something to take, in seat order (the string walks
/// the ballot in the text order of the ids, which is the seat order).
fn sources(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, nekro: Seat, outcome: Outcome) -> Vec<Seat> {
    s.agenda().votes.iter().filter(|(voter, voted)| **voter != nekro && **voted == outcome).map(|(voter, _)| *voter).filter(|voter| !options(s, k, nekro, *voter).is_empty()).collect()
}

fn payout_condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let Some(outcome) = fulfilled(c.state, me, &c.event.payload) else { return false };
    // The Z token adds an option of its own (a flagship's text lent): the string's.
    if holds_z(c.state, me) {
        return c.unknown(Unknown::MissingFact);
    }
    !sources(c.state, c.kernel, me, outcome).is_empty()
}

fn payout(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let payload = eff.event.payload.clone();
    let offered = {
        let (s, k) = eff.read();
        let Some(outcome) = fulfilled(s, me, &payload) else { return Ok(()) };
        let from = sources(s, k, me, outcome);
        if from.is_empty() {
            return Ok(());
        }
        from.into_iter().flat_map(|voter| options(s, k, me, voter).into_iter().map(move |(how, tech)| (how, tech, voter))).collect::<Vec<_>>()
    };
    eff.state().marks_mut().tail.edit().remove(&MarkKey::NekroThreat(me));
    let source = eff.codes().galactic_threat;
    take(eff, &offered, source)
}

pub fn register_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    t.add(Spec::new(PREDICT, AbilityKind::Ability, EventKind::AgendaRevealed, Relation::After, predict).when(predict_condition).optional());
    t.add(Spec::new(PAYOUT, AbilityKind::Ability, EventKind::AgendaResolved, Relation::After, payout).when(payout_condition));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::{Faction, Technology};

    fn game() -> CoreState {
        let (mut s, _) = table(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan, Faction::TheArborec]);
        s.agenda_mut().choices = [Outcome::For, Outcome::Against].into_iter().collect();
        s.meta_mut().round = 2;
        s.meta_mut().agenda_seq = 4;
        s
    }

    fn resolved(o: Outcome) -> Payload {
        Payload::new().with(Key::Player, Val::Outcome(o))
    }

    fn predicted(s: &mut CoreState, seq: u32, o: Outcome) {
        s.marks_mut().tail.edit().insert(MarkKey::NekroThreat(Seat(0)), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Outcome(o))]));
    }

    #[test]
    fn a_prediction_is_open_to_a_nekro_with_outcomes_and_none_yet_this_round() {
        let mut s = game();
        assert!(ready(&s, Seat(0)));
        assert!(!ready(&s, Seat(1)), "only the Nekro");
        s.marks_mut().tail.edit().insert(MarkKey::NekroThreatPhase(Seat(0)), MarkVal::Atom(Atom::Num(2)));
        assert!(!ready(&s, Seat(0)), "once per round");
        s.marks_mut().tail.edit().insert(MarkKey::NekroThreatPhase(Seat(0)), MarkVal::Atom(Atom::Num(1)));
        assert!(ready(&s, Seat(0)), "a prediction of an earlier round does not count");
        s.agenda_mut().choices.clear();
        assert!(!ready(&s, Seat(0)), "an agenda with no outcome");
    }

    #[test]
    fn an_agenda_with_one_outcome_is_predicted_without_a_question() {
        assert_eq!(single(&[Outcome::For]), Some(Outcome::For));
        assert_eq!(single(&[Outcome::For, Outcome::Against]), None);
        assert_eq!(single(&[]), None);
    }

    #[test]
    fn the_payout_needs_the_same_agenda_the_same_outcome_and_an_agenda_not_discarded() {
        let mut s = game();
        assert_eq!(fulfilled(&s, Seat(0), &resolved(Outcome::For)), None, "no prediction held");
        predicted(&mut s, 4, Outcome::For);
        assert_eq!(fulfilled(&s, Seat(0), &resolved(Outcome::For)), Some(Outcome::For));
        assert_eq!(fulfilled(&s, Seat(0), &resolved(Outcome::Against)), None, "another outcome won");
        assert_eq!(fulfilled(&s, Seat(1), &resolved(Outcome::For)), None, "not a Nekro");
        predicted(&mut s, 3, Outcome::For);
        assert_eq!(fulfilled(&s, Seat(0), &resolved(Outcome::For)), None, "a prediction of an earlier agenda");
        predicted(&mut s, 4, Outcome::For);
        s.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
        assert_eq!(fulfilled(&s, Seat(0), &resolved(Outcome::For)), None, "a discarded agenda resolves nothing");
    }

    #[test]
    fn the_sources_are_the_other_voters_for_the_outcome_with_something_to_take() {
        let mut s = game();
        let kernel = ti4_engine_core::kernel::Kernel::plain();
        s.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.insert(Technology::GravityDrive);
        s.agenda_mut().votes.insert(Seat(1), Outcome::For);
        s.agenda_mut().votes.insert(Seat(2), Outcome::For);
        s.agenda_mut().votes.insert(Seat(0), Outcome::For);
        assert_eq!(sources(&s, &kernel, Seat(0), Outcome::For), vec![Seat(1)], "the Arborec owns nothing, the Nekro is not its own source");
        assert!(sources(&s, &kernel, Seat(0), Outcome::Against).is_empty(), "nobody voted that way");
    }
}
