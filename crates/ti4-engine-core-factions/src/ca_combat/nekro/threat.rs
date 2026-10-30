//! nekro: Galactic Threat (package W7), the two halves of the ability.
//!
//! * The prediction, "once per agenda phase, after an agenda is revealed: you may predict aloud the outcome of that agenda"
//!   (`AGENDA_REVEALED` after, optional): one outcome on offer is predicted without a question, several ask
//!   `Galactic Threat: predict the agenda outcome`. The prediction is the mark `nekro:threat:<seat>` (`<agenda sequence>|<outcome>`)
//!   and the round of the phase the mark `nekro:threat_phase:<seat>`.
//! * The payout, "if your prediction is correct, gain 1 technology that is owned by a player who voted how you predicted"
//!   (`AGENDA_RESOLVED` after, mandatory): the voters of the predicted outcome other than the Nekro, in seat order, with something
//!   to take; `singularity::take` asks and applies as for Technological Singularity.
//!
//! Registered for every seat, as the string does (each condition names the Nekro). The string orders the voters by player id text
//! (a `BTreeMap`); the core orders them by seat, the same where the ids follow the seating, which the lobby's do.
//!
//! Not covered (the window bails, tests `bail_*`): the Z token of the Valefar Assimilator (a flagship's text lent), as for the
//! Singularity.

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use ti4_engine_core::choice::generic::Rule;
use ti4_engine_core::choice::reaction::{AbilityKind, Relation, Take};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::Technology;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Outcome};
use ti4_engine_core::state::{CoreState, EventKind, Key, TransientFlags, Val};

use super::singularity::{holds_z, is_nekro, options, take};

/// The prediction (`ability:*:galactic_threat:AGENDA_REVEALED:after`).
pub const PREDICT: Slot = Slot(2009);
/// The payout (`ability:*:galactic_threat:AGENDA_RESOLVED:after`).
pub const PAYOUT: Slot = Slot(2010);

/// `threat_ready`: the agenda has outcomes and the Nekro has not predicted in this agenda phase (the mark holds a round).
fn ready(s: &CoreState, me: Seat) -> bool {
    is_nekro(s, me)
        && !s.agenda().choices.is_empty()
        && !matches!(s.marks().tail.get(&MarkKey::NekroThreatPhase(me)), Some(MarkVal::Atom(Atom::Num(round))) if *round == s.meta().round)
}

fn predict_when(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner)
}

fn predict(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.read().0, me) {
        return Ok(());
    }
    let (choices, agenda) = {
        let s = eff.read().0;
        (s.agenda().choices.as_slice().to_vec(), s.agenda().in_play)
    };
    let predicted = match choices.as_slice() {
        [only] => *only,
        _ => {
            let Some(agenda) = agenda else { return Ok(()) };
            let frame = ti4_engine_core::rules::agenda::frame(eff.read().0, agenda, me);
            let choice = Choice {
                actor: me,
                ask: Ask::Cards(CAsk::PredictThreat { frame }),
                ctx: Ctx {
                    origin: Origin::Rule(Rule::R8),
                    subtype: Subtype::Cards(CSub::PredictOutcome),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: choices.iter().map(|o| Opt::Cards(COpt::Outcome(*o))).collect::<Opts>(),
            };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Cards(COpt::Outcome(outcome))) => outcome,
                _ => return Ok(()),
            }
        }
    };
    let (round, seq) = {
        let m = eff.read().0.meta();
        (m.round, m.agenda_seq)
    };
    let marks = eff.state().marks_mut().tail.edit();
    marks.insert(MarkKey::NekroThreatPhase(me), MarkVal::Atom(Atom::Num(round)));
    marks.insert(MarkKey::NekroThreat(me), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Outcome(predicted))]));
    Ok(())
}

/// The outcome the resolved agenda decided (the payload names a seat that was elected, else the outcome itself).
fn resolved(p: &ti4_engine_core::state::Payload) -> Option<Outcome> {
    match p.get(Key::Player)? {
        Val::Seat(seat) => Some(Outcome::Seat(seat)),
        Val::Outcome(outcome) => Some(outcome),
        _ => None,
    }
}

/// `threat_payout`: the voters to take from when this resolution fulfils the standing prediction; `Err` when the Z token makes the
/// string's options differ.
fn payout(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, event: &ti4_engine_core::state::EventRec, me: Seat) -> Result<Vec<Seat>, ()> {
    if !is_nekro(s, me) || s.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED) {
        return Ok(Vec::new());
    }
    let Some(MarkVal::Rec(held)) = s.marks().tail.get(&MarkKey::NekroThreat(me)) else { return Ok(Vec::new()) };
    let [MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::Outcome(predicted))] = held.as_slice() else { return Ok(Vec::new()) };
    if *seq != s.meta().agenda_seq || resolved(&event.payload) != Some(*predicted) {
        return Ok(Vec::new());
    }
    let voters: Vec<Seat> = s.agenda().votes.iter().filter(|(voter, voted)| **voter != me && **voted == *predicted).map(|(voter, _)| *voter).collect();
    if holds_z(s, me) && !voters.is_empty() {
        return Err(());
    }
    Ok(voters.into_iter().filter(|voter| !options(s, k, me, *voter).is_empty()).collect())
}

fn payout_when(c: &CondCx<'_>) -> bool {
    match payout(c.state, c.kernel, c.event, c.def.owner) {
        Ok(sources) => !sources.is_empty(),
        Err(()) => c.unknown(Unknown::MissingFact),
    }
}

fn pay(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let sources = {
        let (s, k) = eff.read();
        match payout(s, k, &eff.event, me) {
            Ok(sources) if !sources.is_empty() => sources,
            _ => return Ok(()),
        }
    };
    eff.state().marks_mut().tail.edit().remove(&MarkKey::NekroThreat(me));
    let mut offered: Vec<(Take, Technology, Seat)> = Vec::new();
    {
        let (s, k) = eff.read();
        for source in &sources {
            offered.extend(options(s, k, me, *source).into_iter().map(|(how, tech)| (how, tech, *source)));
        }
    }
    let code = eff.codes().galactic_threat;
    take(eff, me, code, &offered)
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(PREDICT, AbilityKind::Ability, EventKind::AgendaRevealed, Relation::After, predict).when(predict_when).optional());
    t.add(Spec::new(PAYOUT, AbilityKind::Ability, EventKind::AgendaResolved, Relation::After, pay).when(payout_when));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::state::{EventRec, Payload, Player, Players, Seats};

    const NEKRO: Seat = Seat(0);

    /// A Nekro (seat 0) with a prediction of `For` made at agenda 3, and two rivals owning a technology the Nekro lacks.
    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let seats = [Seat(0), Seat(1), Seat(2)];
        s.players = Players::from_vec(vec![
            Player::new(Seat(0), Faction::TheNekroVirus),
            Player::new(Seat(1), Faction::TheEmiratesOfHacan),
            Player::new(Seat(2), Faction::TheFederationOfSol),
        ]);
        s.seats = Seats::new(&seats, &seats, 3);
        s.meta_mut().agenda_seq = 3;
        for rival in [Seat(1), Seat(2)] {
            s.players_mut().by_seat_mut(rival).unwrap().tech.owned.insert(Technology::GravityDrive);
        }
        s.marks_mut().tail.edit().insert(MarkKey::NekroThreat(NEKRO), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(3)), MarkVal::Atom(Atom::Outcome(Outcome::For))]));
        s.agenda_mut().votes.insert(Seat(1), Outcome::For);
        s.agenda_mut().votes.insert(Seat(2), Outcome::Against);
        s
    }

    fn resolved_for(outcome: Outcome) -> EventRec {
        EventRec { id: 1, kind: EventKind::AgendaResolved, payload: Payload::new().with(Key::Player, Val::Outcome(outcome)), cancelled: false }
    }

    /// The voters of the predicted outcome, other than the Nekro, with something to take.
    #[test]
    fn the_payout_names_the_voters_of_the_predicted_outcome_that_have_something_to_take() {
        let s = table();
        let k = Kernel::plain();
        assert_eq!(payout(&s, &k, &resolved_for(Outcome::For), NEKRO), Ok(vec![Seat(1)]));
        // The Nekro's own vote is not a source.
        let mut own = table();
        own.agenda_mut().votes.insert(NEKRO, Outcome::For);
        assert_eq!(payout(&own, &k, &resolved_for(Outcome::For), NEKRO), Ok(vec![Seat(1)]));
        // A voter with nothing the Nekro lacks is skipped.
        let mut poor = table();
        poor.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.remove(Technology::GravityDrive);
        assert_eq!(payout(&poor, &k, &resolved_for(Outcome::For), NEKRO), Ok(Vec::new()));
    }

    /// Nothing is paid for a prediction of another agenda, another outcome, an agenda a Deadly Plot discarded, or by a seat that is no
    /// Nekro.
    #[test]
    fn the_payout_needs_the_same_agenda_the_same_outcome_and_no_discard() {
        let k = Kernel::plain();
        let s = table();
        assert_eq!(payout(&s, &k, &resolved_for(Outcome::Against), NEKRO), Ok(Vec::new()), "another outcome");
        let mut later = table();
        later.meta_mut().agenda_seq = 4;
        assert_eq!(payout(&later, &k, &resolved_for(Outcome::For), NEKRO), Ok(Vec::new()), "another agenda");
        let mut discarded = table();
        discarded.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
        assert_eq!(payout(&discarded, &k, &resolved_for(Outcome::For), NEKRO), Ok(Vec::new()), "a discarded agenda resolves nothing");
        assert_eq!(payout(&s, &k, &resolved_for(Outcome::For), Seat(1)), Ok(Vec::new()), "a seat that is no Nekro");
        // Bail: the Z token lends a flagship's text, so the string's options differ.
        let mut z = table();
        z.players_mut().by_seat_mut(NEKRO).unwrap().cards.breakthrough = Some(ti4_engine_core::generated::Breakthrough::ValefarAssimilatorZ);
        assert_eq!(payout(&z, &k, &resolved_for(Outcome::For), NEKRO), Err(()));
    }

    /// The prediction is open once per round: the phase mark holds the round of the last prediction.
    #[test]
    fn the_prediction_is_open_once_per_round() {
        let mut s = table();
        s.meta_mut().round = 2;
        s.agenda_mut().choices = [Outcome::For, Outcome::Against].into_iter().collect();
        assert!(ready(&s, NEKRO));
        s.marks_mut().tail.edit().insert(MarkKey::NekroThreatPhase(NEKRO), MarkVal::Atom(Atom::Num(2)));
        assert!(!ready(&s, NEKRO), "already predicted in this round");
        s.meta_mut().round = 3;
        assert!(ready(&s, NEKRO), "a new round");
        s.agenda_mut().choices.clear();
        assert!(!ready(&s, NEKRO), "an agenda with no outcome");
        s.agenda_mut().choices = [Outcome::For].into_iter().collect();
        assert!(!ready(&s, Seat(1)), "a seat that is no Nekro");
    }
}
