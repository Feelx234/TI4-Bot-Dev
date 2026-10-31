//! mentak: Suffi An, the agent (`factions::mentak::{suffi_an, native_suffi_an_window, borrowed_suffi_an}`): "After the Pillage faction ability is used against another player: you may exhaust this card; if you do,
//! you and that player each draw 1 action card." Pillage (`silent::pillage`) announces the use as `PILLAGE_USED` (`player`, `target`) only when a Ssruu copy of the agent could answer
//! (the string keeps the event stream of games without one); otherwise the agent is offered right there. Three pieces:
//!
//! * [`offer`]: the owner's question (`Suffi An: exhaust to draw 1 action card each with {target}`: exhaust or decline), the exhaustion and the two draws;
//! * the owner's window on `PILLAGE_USED` (`leader:*:mentakagent:PILLAGE_USED:after`, mandatory: it runs the question above);
//! * Ssruu's copy (`leader:*:{lender}:yssarilagent:mentakagent:PILLAGE_USED:after`, a pair rule, optional: the window's own use / decline is the "may"): Ssruu is exhausted, no further question.
//!
//! The draws are the core's draw window stepped inside the effect (`g3::draw::draw`): Scheming's bonus card and discard and the hand limit are asked.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx_strategy;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus, Payload, Val};

use crate::cd_registry::yssaril::ssruu::borrowable;

/// The owner's window (`leader:*:mentakagent:PILLAGE_USED:after`).
pub const SUFFI_AN: Slot = Slot(42_011);
/// Ssruu's copy (`leader:{lender faction}:{lender}:yssarilagent:mentakagent:PILLAGE_USED:after`).
pub const SUFFI_AN_COPY: Slot = Slot(42_012);

const AGENT: Leader = Leader::SuffiAnMentakagent;

fn status(s: &CoreState, who: Seat, card: Leader) -> Option<LeaderStatus> {
    s.players().by_seat(who).and_then(|p| p.leaders.status.get(&card).copied())
}

/// `has_copy`: some seat could copy Suffi An through its Ssruu.
// mutants:skip (only the event journal differs from announcing the use always: the owner's window asks the same question; the lockstep and the dual see the stream)
fn has_copy(s: &CoreState) -> bool {
    s.players().iter().any(|borrower| s.players().iter().any(|lender| borrowable(s, borrower.seat, Some(lender.seat), AGENT)))
}

/// The use of Pillage by `me` against `target` is over: announce it when a copy could answer, else offer the agent at once (`pillage_effect`'s tail).
pub fn after_pillage(eff: &mut Eff<'_, '_>, me: Seat, target: Seat) -> Result<(), TimingError> {
    if has_copy(eff.read().0) {
        let payload = Payload::new().with(Key::Player, Val::Seat(me)).with(Key::Target, Val::Seat(target));
        eff.emit(ti4_engine_core::kernel::fx::event(EventKind::PillageUsed, payload))?;
        Ok(())
    } else {
        offer(eff, me, target)
    }
}

/// `suffi_an`: the agent is readied and the deck holds a card; asked of the owner, exhausted, one card each.
fn offer(eff: &mut Eff<'_, '_>, me: Seat, target: Seat) -> Result<(), TimingError> {
    if status(eff.read().0, me, AGENT) != Some(LeaderStatus::Readied) || eff.read().0.decks().action.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::SuffiAn { target }))),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Leader(AGENT)),
            subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::SuffiAn))),
            optional: false, // mutants:skip (the answers' context: `optional` is the window's, shown by the lockstep of the string tests (dual))
            target: None,
            outstanding: SmallList::new(),
            space_battle: false, // mutants:skip (the adapter puts the combat into every nested context)
            invasion_seq: None,
        },
        opts: [Opt::Reaction(ROpt::G(GOpt::T(TOpt::Exhaust))), Opt::Decline].into_iter().collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Exhaust)))) = eff.ask(&choice) else { return Ok(()) };
    let (s, k) = eff.state_and_kernel();
    if !fx_strategy::exhaust_agent(s, k, me, AGENT) {
        return Ok(());
    }
    for who in [me, target] {
        super::draw::draw(eff, who, 1, SUFFI_AN);
    }
    Ok(())
}

/// The event names the seat that used Pillage and a target that is a player.
fn used(p: &Payload) -> Option<(Seat, Seat)> {
    Some((p.seat(Key::Player)?, p.seat(Key::Target)?))
}

// mutants:skip (the effect, `offer`, re-checks the same facts: a window offered too widely asks nothing)
fn window_when(c: &CondCx<'_>) -> bool {
    status(c.state, c.def.owner, AGENT) == Some(LeaderStatus::Readied) && used(&c.event.payload).is_some_and(|(by, _)| by == c.def.owner) && !c.state.decks().action.is_empty()
}

fn window(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((_, target)) = used(&eff.event.payload) else { return Ok(()) };
    offer(eff, me, target)
}

/// The pair: the lender holds the agent card and the borrower Ssruu (the string's static roster).
// mutants:skip (the roster is the string's static one; the live check is `copy_when`'s `borrowable`)
fn mentak_lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    let holds = |who: Seat, card: Leader| status(s, who, card).is_some();
    holds(lender, AGENT) && holds(borrower, Leader::Ssruu)
}

fn copy_when(c: &CondCx<'_>) -> bool {
    used(&c.event.payload).is_some_and(|(_, target)| c.state.players().by_seat(target).is_some()) && !c.state.decks().action.is_empty() && borrowable(c.state, c.def.owner, c.def.pair_source(), AGENT)
}

fn copy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    let Some((_, target)) = used(&eff.event.payload) else { return Ok(()) };
    if !borrowable(eff.read().0, me, lender, AGENT) {
        return Ok(());
    }
    let (s, k) = eff.state_and_kernel();
    if !fx_strategy::exhaust_agent(s, k, me, Leader::Ssruu) {
        return Ok(());
    }
    for who in [me, target] {
        super::draw::draw(eff, who, 1, SUFFI_AN_COPY);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SUFFI_AN, AbilityKind::Leader, EventKind::PillageUsed, Relation::After, window).when(window_when));
    t.add(Spec::new(SUFFI_AN_COPY, AbilityKind::Leader, EventKind::PillageUsed, Relation::After, copy).applies(Applies::Faction(Faction::TheMentakCoalition)).pair(mentak_lends).optional().when(copy_when));
}
