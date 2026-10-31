//! One step of the follower window of a strategic action (package B8c-1; string `StrategySecondaryWindow::{next_choice,
//! take_choice}` as `Game::step_secondary` drives them): the followers still to be asked, in resolution order, are
//! walked until the first one who may follow; the followers before it are recorded as ineligible (nothing is asked of
//! them), that one is asked whether to follow, decline, use a faction's token-free way (a waiver) or, with Enervate,
//! perform the primary instead, and the answer is applied. When the last follower is done (or none is left who may
//! follow) the primary player's card is exhausted.
//!
//! What the window is given instead of reading: whether each follower may be asked at all, what the follow costs, and
//! the faction ways to follow (`FollowerFacts`). They are the string answers of the faction modules, the research and
//! payment rules and the secret hand (`strategy::secondary_eligible` and its `Env`), which this package does not
//! move; the sums behind them are compared as reads (`strategy::secondary_eligible`).
//!
//! A waiver taken is paid here ([`WaiverEffect`]: Acquiescence's note goes home, Mindsieve's note is handed over, Jae
//! Mir Kan is exhausted and the token comes off the board, Enervate costs nothing); what each waiver does is told by the
//! string side as data, like the rest of the facts.
//!
//! Not ported (the adapter bails, one test each): a waiver whose payment the tables do not carry
//! ([`WaiverEffect::Unported`]: an unknown module, Jae Mir Kan while the Nomad watches an agent exhaust), a window that
//! is a preview or resolves a card nobody holds (Winnu hero), a position with a nested emission.
//!
//! The follow EFFECT (`strategy_cards::follow`, `resolve_faction_strategy`) is not this window: the string runs it
//! after the step, and each card's effect is already its own window.

use crate::choice::strategy::{ctx, rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::containers::SmallList;
use crate::generated::{Leader, StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, fx_strategy, Kernel, Next};
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, NoteKey, Payload, Val};

/// What taking a waiver does to the position (the payment the module that offered it makes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaiverEffect {
    /// Costs nothing (Enervate).
    Free,
    /// Acquiescence: the follower's note goes back to its owner.
    GiveBack(NoteKey),
    /// Mindsieve: the follower hands the primary player the note.
    HandOver(NoteKey),
    /// Jae Mir Kan: the follower's agent is exhausted and the primary player's token comes off `system`.
    Agent { leader: Leader, system: System },
    /// A payment the tables do not carry: the window refuses the waiver (the adapter bails before it is taken).
    Unported,
}

/// One token-free way to follow: the option it offers (the string module's own, interned as an option shape: `Opt::Waive`; or the
/// core module's, `Opt::WaiveBy`) and what it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Waiver {
    pub opt: SOpt,
    pub effect: WaiverEffect,
}

/// What the window is told of one follower, from the string side.
#[derive(Debug, Clone)]
pub struct FollowerFacts {
    pub who: Seat,
    /// The follower is asked (`StrategySecondaryWindow::eligible`).
    pub eligible: bool,
    /// The follow takes a strategy token (`secondary_costs_token && !secondary_is_free`).
    pub costs_token: bool,
    /// Enervate: the primary may be performed instead (only offered when the follow costs no token).
    pub primary_instead: bool,
    /// The token-free ways offered beside the follow (when it costs a token).
    pub waivers: Vec<Waiver>,
}

/// How a follower answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Declined,
    Followed,
}

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

/// What the step did, as the string window records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The followers recorded as ineligible, in order.
    pub skipped: SmallList<Seat, 16>,
    /// The follower who was asked and how they answered.
    pub answered: Option<(Seat, Resolution)>,
    /// The window has no follower left.
    pub complete: bool,
}

/// The held follower window after one step (package D1a; string `StrategySecondaryWindow::{next_choice, take_choice}` on the
/// window itself): the followers skipped are recorded as ineligible and the one asked as it answered, each moving the
/// window on by one follower.
pub fn advance(window: &mut crate::state::Secondary, out: &Outcome) {
    use crate::state::SecondaryHow;
    for seat in out.skipped.as_slice() {
        window.resolved.push((*seat, SecondaryHow::Ineligible));
        window.next += 1;
    }
    if let Some((seat, how)) = out.answered {
        window.resolved.push((
            seat,
            match how {
                Resolution::Declined => SecondaryHow::Declined,
                Resolution::Followed => SecondaryHow::Followed,
            },
        ));
        window.next += 1;
    }
}

/// The plain events the string pushes for one step of the follower window (`Game::step_secondary`), in order, as far as the
/// outcome of the core half decides them: how the follower answered, then the end of the action when no follower is left
/// (package P1: the plain events of a finished area). The events of what the follower's card did, of the leader strategy that
/// completes and of the turn that closes come from other areas and are not in the list.
#[must_use]
pub fn plain_events(out: &Outcome) -> SmallList<crate::state::PlainRec, 2> {
    use crate::state::{PlainKind, PlainRec};
    let mut events = SmallList::new();
    match out.answered {
        Some((_, Resolution::Declined)) => events.push(PlainRec::bare(PlainKind::StrategySecondaryDeclined)),
        Some((_, Resolution::Followed)) => events.push(PlainRec::bare(PlainKind::StrategySecondaryFollowed)),
        None => {}
    }
    if out.complete {
        events.push(PlainRec::bare(PlainKind::StrategicActionComplete));
    }
    events
}

/// Whether every follower of the held window is recorded (`StrategySecondaryWindow::is_complete`).
#[must_use]
pub fn is_complete(window: &crate::state::Secondary) -> bool {
    usize::from(window.next) == window.followers.len()
}

/// The step of the follower window up to and including the first follower who is asked.
#[derive(Debug, Clone)]
pub struct SecondaryStep {
    primary: Seat,
    card: StrategyCard,
    /// Finishing the window exhausts the primary player's copy of the card (not for a card resolved by an effect).
    exhausts_card: bool,
    /// How many followers the window still holds (the ones in `followers` and those after them).
    total: usize,
    /// The followers from the front of the window to the first one who is asked (all of them when none is).
    followers: Vec<FollowerFacts>,
    /// The interned `strategy_secondary`: the reason of the strategy token spent.
    reason: Code,
    asked: Option<usize>,
    stage: Stage,
    out: Outcome,
}

impl SecondaryStep {
    #[must_use]
    pub fn new(primary: Seat, card: StrategyCard, exhausts_card: bool, total: usize, followers: &[FollowerFacts], reason: Code) -> Self {
        Self {
            primary,
            card,
            exhausts_card,
            total,
            followers: followers.to_vec(),
            reason,
            asked: None,
            stage: Stage::Start,
            out: Outcome { skipped: SmallList::new(), answered: None, complete: false },
        }
    }

    /// What the step did (meaningful once the window is done).
    #[must_use]
    pub const fn outcome(&self) -> &Outcome {
        &self.out
    }

    fn exhaust_primary(&self, core: &mut CoreState) {
        if self.exhausts_card {
            // `strategy_write::exhaust` (`strategy_flow::exhaust`: a rule area cannot import another).
            if let Some(p) = core.players_mut().by_seat_mut(self.primary) {
                p.strat_exhausted.insert(self.card);
            }
        }
    }

    fn follower(&self) -> Option<&FollowerFacts> {
        self.followers.get(self.asked?)
    }
}

impl Window for SecondaryStep {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            let mut at = 0;
            while let Some(f) = self.followers.get(at) {
                if f.eligible {
                    self.asked = Some(at);
                    break;
                }
                self.out.skipped.push(f.who);
                at += 1;
            }
            if self.asked.is_some() {
                self.stage = Stage::Asking;
            } else {
                // No follower may follow: the window is over, the card is spent.
                self.out.complete = true;
                self.exhaust_primary(sx.state);
                self.stage = Stage::Done;
            }
        }
        match (self.stage, self.follower()) {
            (Stage::Asking, Some(f)) => Next::Continue(Ask::Strategy(SAsk::Follow { card: self.card, primary: self.primary, costs_token: f.costs_token })),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        let f = self.follower().filter(|_| self.stage == Stage::Asking)?;
        Some(follow_choice(s, self.card, self.primary, f))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Some(f) = self.follower().filter(|_| self.stage == Stage::Asking).cloned() else { return Err(Illegal) };
        let Pick::Opt(Opt::Strategy(opt)) = pick else { return Err(Illegal) };
        let resolution = take_answer(sx.state, sx.kernel, self.primary, self.reason, &f, opt)?;
        self.out.answered = Some((f.who, resolution));
        // The window is complete when the asked follower was the last one.
        let index = self.asked.unwrap_or(0);
        self.out.complete = index + 1 == self.total;
        if self.out.complete {
            self.exhaust_primary(sx.state);
        }
        // The followers before the asked one were recorded as ineligible in `step`.
        self.stage = Stage::Done;
        Ok(())
    }
}

/// The question put to `f` about the secondary of `card` (the string `StrategySecondaryWindow::pending_choice` for a follower who may be asked): follow or
/// not, a token-free way instead (a waiver) or Enervate's primary instead. A follower who must pay a token and has none is offered a waiver instead of a
/// "yes" that could not be paid.
#[must_use]
pub fn follow_choice(s: &CoreState, card: StrategyCard, primary: Seat, f: &FollowerFacts) -> Choice {
    let mut opts = Opts::default();
    opts.push(Opt::Strategy(SOpt::Skip));
    let tokenless = s.players().by_seat(f.who).is_none_or(|p| p.econ.strategic_tokens <= 0);
    if !(f.costs_token && !f.waivers.is_empty() && tokenless) {
        opts.push(Opt::Strategy(SOpt::Follow));
    }
    if !f.costs_token {
        // Enervate: a puppeted player's Leadership may be followed by performing its primary instead.
        if f.primary_instead {
            opts.push(Opt::Strategy(SOpt::PrimaryInstead));
        }
    } else {
        for w in &f.waivers {
            opts.push(Opt::Strategy(w.opt));
        }
    }
    let leadership = card.data().kind == StrategyCardKind::Leadership;
    Choice {
        actor: f.who,
        ask: Ask::Strategy(SAsk::Follow { card, primary, costs_token: f.costs_token }),
        // Leadership's window question is the purchase question itself (52.3); the others carry no context.
        ctx: if leadership { rule_ctx(Rule::R52_3, SSub::BuyTokenWithInfluence) } else { ctx(card, true, SSub::FollowSecondary) },
        opts,
    }
}

/// What taking `opt` does to the position (`StrategySecondaryWindow::take_choice` after the answer was validated): a refusal, a decline, the
/// strategy token spent and announced, or a waiver paid by the module that offered it. `reason` is the interned `strategy_secondary`.
///
/// # Errors
/// The option is not one the follower was offered, or the token cannot be paid.
pub fn take_answer(core: &mut CoreState, k: &Kernel, primary: Seat, reason: Code, f: &FollowerFacts, opt: SOpt) -> Result<Resolution, Illegal> {
    let resolution = match opt {
        SOpt::Skip => Resolution::Declined,
        SOpt::Follow => {
            if f.costs_token {
                if !fx::spend_strategic_token(core, f.who) {
                    return Err(Illegal);
                }
                // `supply::note_strategy_token_spent(.., "strategy_secondary")`.
                fx::stage(
                    core,
                    EventRec {
                        id: 0,
                        kind: EventKind::StrategyTokenSpent,
                        payload: Payload::new().with(Key::Player, Val::Seat(f.who)).with(Key::Reason, Val::Code(reason)),
                        cancelled: false,
                    },
                );
            }
            Resolution::Followed
        }
        SOpt::PrimaryInstead if !f.costs_token && f.primary_instead => Resolution::Followed,
        // A token-free way: the module that offered it pays its cost.
        SOpt::Waive(_) | SOpt::WaiveBy(..) if f.costs_token => {
            let waiver = f.waivers.iter().find(|w| w.opt == opt).ok_or(Illegal)?;
            pay_waiver(core, k, primary, f, waiver.effect)?;
            Resolution::Followed
        }
        _ => return Err(Illegal),
    };
    note_answer(core, f.who, opt == SOpt::PrimaryInstead);
    Ok(resolution)
}

/// `hooks_strategy::secondary_waived` of the module that offered the waiver.
fn pay_waiver(core: &mut CoreState, k: &Kernel, primary: Seat, f: &FollowerFacts, effect: WaiverEffect) -> Result<(), Illegal> {
    match effect {
        WaiverEffect::Free => {}
        WaiverEffect::GiveBack(note) => {
            if core.cards().promissory.get(&note) == Some(&f.who) {
                fx::give_back_note(core, note);
            }
        }
        WaiverEffect::HandOver(note) => {
            if f.who != primary && core.cards().promissory.get(&note) == Some(&f.who) {
                fx_strategy::take_note(core, primary, note);
            }
        }
        WaiverEffect::Agent { leader, system } => {
            if core.board().sys(system).cmd.contains(primary) && fx_strategy::exhaust_ready_agent(core, k, f.who, leader) {
                fx::remove_command_token(core, primary, system);
            }
        }
        WaiverEffect::Unported => return Err(Illegal),
    }
    Ok(())
}

/// `Enervate` is remembered per follower; every other answer forgets it (`record_follow_answer`).
fn note_answer(core: &mut CoreState, who: Seat, primary_instead: bool) {
    let marks = core.marks_mut().tail.edit();
    if primary_instead {
        marks.insert(MarkKey::FirmamentEnervate(), MarkVal::Atom(Atom::Seat(who)));
    } else {
        marks.remove(&MarkKey::FirmamentEnervate());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    fn held(followers: &[u8]) -> crate::state::Secondary {
        crate::state::Secondary {
            primary: Seat(0),
            card: StrategyCard::Politics,
            followers: followers.iter().map(|s| Seat(*s)).collect(),
            next: 0,
            resolved: SmallList::new(),
            exhausts_card: true,
            preview: false,
        }
    }

    #[test]
    fn the_held_window_takes_the_skipped_followers_and_the_answer_and_is_complete_when_none_is_left() {
        use crate::state::SecondaryHow;
        let mut w = held(&[1, 2, 3]);
        let mut skipped = SmallList::new();
        skipped.push(Seat(1));
        advance(&mut w, &Outcome { skipped, answered: Some((Seat(2), Resolution::Followed)), complete: false });
        assert_eq!(w.next, 2);
        assert_eq!(w.resolved.as_slice(), [(Seat(1), SecondaryHow::Ineligible), (Seat(2), SecondaryHow::Followed)]);
        assert!(!is_complete(&w));
        advance(&mut w, &Outcome { skipped: SmallList::new(), answered: Some((Seat(3), Resolution::Declined)), complete: true });
        assert_eq!((w.next, w.resolved.len()), (3, 3));
        assert!(is_complete(&w));
        // A step that only skips the rest (nobody is asked) completes it too.
        let mut w = held(&[1, 2]);
        let mut skipped = SmallList::new();
        skipped.push(Seat(1));
        skipped.push(Seat(2));
        advance(&mut w, &Outcome { skipped, answered: None, complete: true });
        assert!(is_complete(&w));
    }


    const REASON: Code = Code(0);

    fn game(tokens: [i16; 3]) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        for (i, t) in tokens.into_iter().enumerate() {
            s.players_mut().by_seat_mut(Seat(i as u8)).unwrap().econ.strategic_tokens = t;
        }
        s.players_mut().by_seat_mut(Seat(0)).unwrap().strat = [StrategyCard::Warfare].into_iter().collect();
        s
    }

    fn fact(who: u8, eligible: bool, costs_token: bool) -> FollowerFacts {
        FollowerFacts { who: Seat(who), eligible, costs_token, primary_instead: false, waivers: Vec::new() }
    }

    fn run(s: &mut CoreState, step: SecondaryStep, answer: Option<SOpt>) -> (Driver<SecondaryStep>, Vec<Choice>) {
        let k = Kernel::plain();
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1), Seat(2)], Some(Seat(0))), AbilityTable::default(), 1);
        let mut asked = Vec::new();
        let mut status = d.run(s, &k).unwrap();
        while let Status::Asking(c) = status {
            asked.push(c.clone());
            let want = answer.expect("a question was not expected");
            let at = c.opts.iter().position(|o| *o == Opt::Strategy(want)).expect("the answer is offered");
            status = d.resume(s, &k, c.pick(at).unwrap()).unwrap();
        }
        (d, asked)
    }

    #[test]
    fn the_ineligible_are_skipped_and_the_first_eligible_follower_pays_a_token() {
        let mut s = game([0, 0, 2]);
        let step = SecondaryStep::new(Seat(0), StrategyCard::Warfare, true, 2, &[fact(1, false, true), fact(2, true, true)], REASON);
        let (d, asked) = run(&mut s, step, Some(SOpt::Follow));
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].actor, Seat(2));
        let out = d.window.outcome();
        assert_eq!(out.skipped.as_slice(), &[Seat(1)]);
        assert_eq!(out.answered, Some((Seat(2), Resolution::Followed)));
        assert!(out.complete);
        assert_eq!(s.players().by_seat(Seat(2)).unwrap().econ.strategic_tokens, 1, "the token was spent");
        assert_eq!(s.pending().staged.len(), 1);
        assert_eq!(s.pending().staged[0].kind, EventKind::StrategyTokenSpent);
        assert!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Warfare), "the last follower exhausts the card");
    }

    #[test]
    fn a_decline_spends_nothing_and_the_card_waits_for_the_next_follower() {
        let mut s = game([0, 3, 3]);
        let step = SecondaryStep::new(Seat(0), StrategyCard::Warfare, true, 2, &[fact(1, true, true)], REASON);
        let (d, _) = run(&mut s, step, Some(SOpt::Skip));
        assert_eq!(d.window.outcome().answered, Some((Seat(1), Resolution::Declined)));
        assert!(!d.window.outcome().complete);
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 3);
        assert!(s.pending().staged.is_empty());
        assert!(!s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Warfare), "not yet");
    }

    #[test]
    fn when_nobody_may_follow_nothing_is_asked_and_the_card_is_exhausted() {
        let mut s = game([0, 0, 0]);
        let step = SecondaryStep::new(Seat(0), StrategyCard::Warfare, true, 2, &[fact(1, false, true), fact(2, false, true)], REASON);
        let (d, asked) = run(&mut s, step, None);
        assert!(asked.is_empty());
        let out = d.window.outcome();
        assert_eq!(out.skipped.as_slice(), &[Seat(1), Seat(2)]);
        assert!(out.answered.is_none() && out.complete);
        assert!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Warfare));
    }

    #[test]
    fn a_card_resolved_by_an_effect_is_not_exhausted() {
        let mut s = game([0, 1, 0]);
        let step = SecondaryStep::new(Seat(0), StrategyCard::Warfare, false, 1, &[fact(1, true, true)], REASON);
        let (d, _) = run(&mut s, step, Some(SOpt::Follow));
        assert!(d.window.outcome().complete);
        assert!(!s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Warfare));
    }

    #[test]
    fn leadership_costs_no_token_and_enervate_is_remembered_until_another_answer() {
        let mut s = game([0, 0, 0]);
        let mut f = fact(1, true, false);
        f.primary_instead = true;
        let step = SecondaryStep::new(Seat(0), StrategyCard::Leadership, true, 1, &[f.clone()], REASON);
        let (d, asked) = run(&mut s, step, Some(SOpt::PrimaryInstead));
        assert_eq!(asked[0].opts.len(), 3, "no, yes, primary instead");
        assert_eq!(d.window.outcome().answered, Some((Seat(1), Resolution::Followed)));
        assert!(s.pending().staged.is_empty(), "no token");
        assert_eq!(s.marks().tail.get(&MarkKey::FirmamentEnervate()), Some(&MarkVal::Atom(Atom::Seat(Seat(1)))));
        let step = SecondaryStep::new(Seat(0), StrategyCard::Leadership, true, 1, &[f], REASON);
        run(&mut s, step, Some(SOpt::Skip));
        assert!(s.marks().tail.get(&MarkKey::FirmamentEnervate()).is_none(), "a decline forgets it");
    }

    fn waiver(effect: WaiverEffect) -> Waiver {
        Waiver { opt: SOpt::Waive(Code(7)), effect }
    }

    /// Drives one asked follower holding `effect` as its only waiver and answers it by taking the waiver.
    fn take_waiver(s: &mut CoreState, effect: WaiverEffect) -> (Driver<SecondaryStep>, bool) {
        take_waiver_as(s, waiver(effect))
    }

    /// The same, for a waiver in either form (`Waive(shape)` of the dual, `WaiveBy(module, code)` of the live loop).
    fn take_waiver_as(s: &mut CoreState, w: Waiver) -> (Driver<SecondaryStep>, bool) {
        let mut f = fact(1, true, true);
        f.waivers.push(w);
        let k = Kernel::plain();
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(s, &k).unwrap() else { panic!("asks") };
        let at = c.opts.iter().position(|o| *o == Opt::Strategy(w.opt)).expect("the waiver is offered");
        let ok = d.resume(s, &k, c.pick(at).unwrap()).is_ok();
        (d, ok)
    }

    /// `Opt::Waive` (the dual's: the string module's own option, interned) and `Opt::WaiveBy` (the live loop's: the kernel hook's module and
    /// code) are two forms of ONE option: the machine offers, takes and pays them alike.
    #[test]
    fn the_two_forms_of_a_waiver_are_one_option_to_the_machine() {
        let by = |effect| Waiver { opt: SOpt::WaiveBy(crate::generated::Faction::TheWinnu, 0), effect };
        for effect in [WaiverEffect::Free, WaiverEffect::Unported] {
            let (mut a, mut b) = (game([0, 2, 0]), game([0, 2, 0]));
            let (_, ok_shape) = take_waiver_as(&mut a, waiver(effect));
            let (_, ok_by) = take_waiver_as(&mut b, by(effect));
            assert_eq!(ok_shape, ok_by, "{effect:?}");
            assert_eq!(a.players().by_seat(Seat(0)).unwrap().strat_exhausted, b.players().by_seat(Seat(0)).unwrap().strat_exhausted);
            assert_eq!(a.pending().staged.len(), b.pending().staged.len(), "{effect:?}: the same events staged");
        }
        let mut s = game([0, 0, 0]);
        let mut f = fact(1, true, true);
        f.waivers.push(by(WaiverEffect::Free));
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &Kernel::plain()).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.len(), 2, "a tokenless follower is offered the decline and the waiver, as with the shape form");
    }

    #[test]
    fn a_tokenless_follower_with_a_waiver_is_not_offered_the_follow() {
        let mut s = game([0, 0, 0]);
        let mut f = fact(1, true, true);
        f.waivers.push(waiver(WaiverEffect::Free));
        let k = Kernel::plain();
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.as_slice(), &[Opt::Strategy(SOpt::Skip), Opt::Strategy(SOpt::Waive(Code(7)))]);
    }

    /// Bail: a waiver whose payment the tables do not carry is refused (the adapter bails before it is taken).
    #[test]
    fn bail_an_unported_waiver_is_refused_and_changes_nothing() {
        let mut s = game([0, 0, 0]);
        let before = s.clone();
        let (_, ok) = take_waiver(&mut s, WaiverEffect::Unported);
        assert!(!ok);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted, before.players().by_seat(Seat(0)).unwrap().strat_exhausted);
        assert!(s.pending().staged.is_empty());
    }

    #[test]
    fn a_free_waiver_follows_without_a_token_and_forgets_enervate() {
        let mut s = game([0, 2, 0]);
        s.marks_mut().tail.edit().insert(MarkKey::FirmamentEnervate(), MarkVal::Atom(Atom::Seat(Seat(1))));
        let (d, ok) = take_waiver(&mut s, WaiverEffect::Free);
        assert!(ok);
        assert_eq!(d.window.outcome().answered, Some((Seat(1), Resolution::Followed)));
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 2, "no token");
        assert!(s.pending().staged.is_empty());
        assert!(s.marks().tail.get(&MarkKey::FirmamentEnervate()).is_none());
        assert!(s.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(StrategyCard::Trade), "the last follower exhausts the card");
    }

    #[test]
    fn acquiescence_goes_home_when_it_is_taken() {
        let mut s = game([0, 0, 0]);
        let key = NoteKey { note: crate::generated::PromissoryNote::Acquiescence, faction: Faction::ALL[0] };
        let seats = [Seat(0), Seat(1), Seat(2)];
        s.seats = crate::state::Seats::new(&seats, &seats, 3);
        s.cards_mut().promissory.edit().insert(key, Seat(1));
        let (_, ok) = take_waiver(&mut s, WaiverEffect::GiveBack(key));
        assert!(ok);
        assert_eq!(s.cards().promissory.get(&key), Some(&Seat(0)), "back with its owner");
    }

    #[test]
    fn mindsieve_hands_the_primary_player_the_note() {
        let mut s = game([0, 0, 0]);
        let key = NoteKey { note: crate::generated::PromissoryNote::Acquiescence, faction: Faction::ALL[2] };
        s.cards_mut().promissory.edit().insert(key, Seat(1));
        let (_, ok) = take_waiver(&mut s, WaiverEffect::HandOver(key));
        assert!(ok);
        assert_eq!(s.cards().promissory.get(&key), Some(&Seat(0)), "the primary player holds it now");
        // A note the follower does not hold is not handed over.
        let mut s = game([0, 0, 0]);
        s.cards_mut().promissory.edit().insert(key, Seat(2));
        take_waiver(&mut s, WaiverEffect::HandOver(key));
        assert_eq!(s.cards().promissory.get(&key), Some(&Seat(2)));
    }

    #[test]
    fn jae_mir_kan_is_exhausted_and_the_token_leaves_the_board() {
        use crate::generated::{Leader, System};
        let mut s = game([0, 0, 0]);
        s.board_mut().sys_mut(System::ALL[3]).cmd.insert(Seat(0));
        s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::JaeMirKan, crate::state::LeaderStatus::Readied);
        let (_, ok) = take_waiver(&mut s, WaiverEffect::Agent { leader: Leader::JaeMirKan, system: System::ALL[3] });
        assert!(ok);
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&crate::state::LeaderStatus::Exhausted));
        assert!(!s.board().sys(System::ALL[3]).cmd.contains(Seat(0)), "the token is off the board");
        // An agent that is not ready pays nothing and the token stays.
        let mut s = game([0, 0, 0]);
        s.board_mut().sys_mut(System::ALL[3]).cmd.insert(Seat(0));
        take_waiver(&mut s, WaiverEffect::Agent { leader: Leader::JaeMirKan, system: System::ALL[3] });
        assert!(s.board().sys(System::ALL[3]).cmd.contains(Seat(0)));
    }

    /// A Nomad that listens (Temporal Command Suite) is told by the waiver's exhaustion of Jae Mir Kan; the pre-fix core exhausted the card silently (the
    /// adapter then bailed to the string).
    #[test]
    fn jae_mir_kan_tells_a_listening_nomad() {
        use crate::generated::{Leader, System};
        let mut s = game([0, 0, 0]);
        s.board_mut().sys_mut(System::ALL[3]).cmd.insert(Seat(0));
        s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::JaeMirKan, crate::state::LeaderStatus::Readied);
        let mut f = fact(1, true, true);
        f.waivers.push(waiver(WaiverEffect::Agent { leader: Leader::JaeMirKan, system: System::ALL[3] }));
        let mut k = Kernel::plain();
        k.hooks.direct.nomad_watches_agent_exhaustion.push(|_| true);
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        let at = c.opts.iter().position(|o| *o == Opt::Strategy(SOpt::Waive(Code(7)))).expect("the waiver is offered");
        d.resume(&mut s, &k, c.pick(at).unwrap()).unwrap();
        assert_eq!(s.pending().staged.iter().filter(|e| e.kind == EventKind::AgentExhausted).count(), 1);
    }

    #[test]
    fn the_plain_events_of_a_step_follow_the_answer_and_the_end_of_the_action() {
        use crate::state::PlainKind::{StrategicActionComplete, StrategySecondaryDeclined, StrategySecondaryFollowed};
        let kinds = |o: &Outcome| plain_events(o).as_slice().iter().map(|p| p.kind).collect::<Vec<_>>();
        let out = |answered, complete| Outcome { skipped: SmallList::new(), answered, complete };
        assert_eq!(kinds(&out(Some((Seat(1), Resolution::Followed)), false)), [StrategySecondaryFollowed]);
        assert_eq!(kinds(&out(Some((Seat(1), Resolution::Declined)), true)), [StrategySecondaryDeclined, StrategicActionComplete]);
        assert_eq!(kinds(&out(None, true)), [StrategicActionComplete], "the window ran out of followers");
        assert!(kinds(&out(None, false)).is_empty());
    }
}
