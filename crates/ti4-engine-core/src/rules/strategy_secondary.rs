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
//! Not ported (the adapter bails, one test each): a waiver taken (the module pays its own cost in the string
//! state: Acquiescence, Jae Mir Kan, Mindsieve), a window that is a preview or resolves a card nobody holds (Winnu
//! hero), a position with a nested emission.
//!
//! The follow EFFECT (`strategy_cards::follow`, `resolve_faction_strategy`) is not this window: the string runs it
//! after the step, and each card's effect is already its own window.

use crate::choice::strategy::{ctx, rule_ctx, Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Opt, Opts, Pick};
use crate::containers::SmallList;
use crate::generated::{StrategyCard, StrategyCardKind};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, Payload, Val};

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
    /// The token-free ways offered beside the follow (when it costs a token), as interned option shapes.
    pub waivers: Vec<Code>,
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

    /// `Enervate` is remembered per follower; every other answer forgets it (`record_follow_answer`).
    fn note_answer(core: &mut CoreState, who: Seat, primary_instead: bool) {
        let marks = core.marks_mut().tail.edit();
        if primary_instead {
            marks.insert(MarkKey::FirmamentEnervate(), MarkVal::Atom(Atom::Seat(who)));
        } else {
            marks.remove(&MarkKey::FirmamentEnervate());
        }
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
        let mut opts = Opts::default();
        opts.push(Opt::Strategy(SOpt::Skip));
        // A follower who must pay a token and has none is offered a waiver instead of a "yes" that could not be paid.
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
                opts.push(Opt::Strategy(SOpt::Waive(*w)));
            }
        }
        let leadership = self.card.data().kind == StrategyCardKind::Leadership;
        Some(Choice {
            actor: f.who,
            ask: Ask::Strategy(SAsk::Follow { card: self.card, primary: self.primary, costs_token: f.costs_token }),
            // Leadership's window question is the purchase question itself (52.3); the others carry no context.
            ctx: if leadership { rule_ctx(Rule::R52_3, SSub::BuyTokenWithInfluence) } else { ctx(self.card, true, SSub::FollowSecondary) },
            opts,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Some(f) = self.follower().filter(|_| self.stage == Stage::Asking).cloned() else { return Err(Illegal) };
        let Pick::Opt(Opt::Strategy(opt)) = pick else { return Err(Illegal) };
        let resolution = match opt {
            SOpt::Skip => Resolution::Declined,
            SOpt::Follow => {
                if f.costs_token {
                    if !fx::spend_strategic_token(sx.state, f.who) {
                        return Err(Illegal);
                    }
                    // `supply::note_strategy_token_spent(.., "strategy_secondary")`.
                    fx::stage(
                        sx.state,
                        EventRec {
                            id: 0,
                            kind: EventKind::StrategyTokenSpent,
                            payload: Payload::new().with(Key::Player, Val::Seat(f.who)).with(Key::Reason, Val::Code(self.reason)),
                            cancelled: false,
                        },
                    );
                }
                Resolution::Followed
            }
            SOpt::PrimaryInstead if !f.costs_token && f.primary_instead => Resolution::Followed,
            // A waiver's cost is paid by the module that offered it, in the string state: the adapter does not run it.
            _ => return Err(Illegal),
        };
        Self::note_answer(sx.state, f.who, opt == SOpt::PrimaryInstead);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

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

    #[test]
    fn a_tokenless_follower_with_a_waiver_is_not_offered_the_follow() {
        let mut s = game([0, 0, 0]);
        let mut f = fact(1, true, true);
        f.waivers.push(Code(7));
        let k = Kernel::plain();
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.opts.as_slice(), &[Opt::Strategy(SOpt::Skip), Opt::Strategy(SOpt::Waive(Code(7)))]);
    }

    /// Bail: a waiver taken is a module's own payment in the string state; the window refuses it and the adapter does not run it.
    #[test]
    fn bail_a_waiver_taken_is_refused() {
        let mut s = game([0, 0, 0]);
        let mut f = fact(1, true, true);
        f.waivers.push(Code(7));
        let k = Kernel::plain();
        let step = SecondaryStep::new(Seat(0), StrategyCard::Trade, true, 1, &[f], REASON);
        let mut d = Driver::new(step, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert!(d.resume(&mut s, &k, c.pick(1).unwrap()).is_err());
    }
}
