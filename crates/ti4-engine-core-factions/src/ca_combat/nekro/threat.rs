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
