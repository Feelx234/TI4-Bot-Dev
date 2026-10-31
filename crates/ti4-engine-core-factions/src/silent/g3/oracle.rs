//! naalu: The Oracle, the hero (`factions::naalu::hero`): "At the end of the status phase: each other player gives you 1 promissory note from their hand. Then, purge this card." Optional; offered only when someone
//! has a note, so "if you do" is always met. Every answer is asked first (a player with one note gives it without a question), then the notes move, then the card is purged.
//! Registered for every seat (`leader:*:naaluhero:STATUS_PHASE_ENDED:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::Leader;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx_strategy;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::promissory;
use ti4_engine_core::state::{CoreState, EventKind, LeaderStatus, NoteKey};

/// The Oracle (`leader:*:naaluhero:STATUS_PHASE_ENDED:after`).
pub const ORACLE: Slot = Slot(42_015);

const HERO: Leader = Leader::TheOracle;

/// `hand_notes`: the notes `who` holds that are not faceup, in the string's order (the notes' text).
fn hand(s: &CoreState, facts: &AdapterFacts, who: Seat) -> Vec<NoteKey> {
    let mut held: Vec<NoteKey> = promissory::held_by(s, who).into_iter().filter(|key| !s.cards().promissory_faceup.contains_key(key)).collect();
    match facts.plays.as_ref() {
        Some(plays) => held.sort_by(|a, b| (plays.note_order)(*a, *b)),
        None => held.sort(),
    }
    held
}

/// `hero_victims`: the other players with a note in hand, in seat order.
fn victims(s: &CoreState, facts: &AdapterFacts, me: Seat) -> Vec<Seat> {
    s.players().iter().map(|p| p.seat).filter(|other| *other != me && !hand(s, facts, *other).is_empty()).collect()
}

fn ready(s: &CoreState, facts: &AdapterFacts, me: Seat) -> bool {
    s.players().by_seat(me).is_some_and(|p| p.leaders.status.get(&HERO) == Some(&LeaderStatus::Unlocked)) && !victims(s, facts, me).is_empty()
}

fn condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.facts, c.def.owner)
}

fn oracle(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.read().0, eff.adapter_facts(), me) {
        return Ok(());
    }
    // Every answer first, then the transfer.
    let mut taken: Vec<NoteKey> = Vec::new();
    for victim in victims(eff.read().0, eff.adapter_facts(), me) {
        let notes = hand(eff.read().0, eff.adapter_facts(), victim);
        let note = if let [only] = notes.as_slice() {
            *only
        } else {
            let choice = Choice {
                actor: victim,
                ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::OracleGive))),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Leader(HERO)),
                    subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::OracleGive))),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: notes.iter().map(|key| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Note(*key))))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Note(key))))) = eff.ask(&choice) else { continue };
            if !notes.contains(&key) {
                continue;
            }
            key
        };
        taken.push(note);
    }
    for key in taken {
        fx_strategy::take_note(eff.state(), me, key);
    }
    ti4_engine_core::rules::leaders::purge(eff.state(), me, HERO);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ORACLE, AbilityKind::Leader, EventKind::StatusPhaseEnded, Relation::After, oracle).optional().when(condition));
}
