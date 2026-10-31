//! The command token gain of the status phase (81.5; `tokens::TokenGain` and `Game::step_token_gain`) as a HELD window: the first
//! arm run natively end to end (package M0 vertical slice, `docs/D2_LF_RESUME.md`; lane C extends it as KS4).
//!
//! The payload on the stack is `state::Tokens` (who still owes a token, what was placed, the status report it travels with);
//! the machine is that payload and nothing else, so `from_pending(to_pending(m)) == m`. One question per token
//! ([`gain_pool_choice`], `left` = the tokens the same seat still places, this one included); an answer places the token (at most
//! what is in reinforcements, 20.4a: the offer happens either way) and queues the plain `COMMAND_TOKEN_GAINED:<pool>`.
//!
//! What stays the string's (KS5): the close, steps 81.6-81.8 (`finish_status_phase`), which runs after the last token.

use crate::choice::strategy::{gain_pool_choice, Ask as SAsk, Opt as SOpt};
use crate::choice::{Ask, Choice, Opt, Pick};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::{CoreState, PlainArg, PlainKind, PlainRec, Tokens, Window as OpenWindow, WindowKind};

/// The token gain window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenGain {
    tokens: Tokens,
}

impl TokenGain {
    /// A window over `tokens` (the opener builds the payload: one entry of `pending` per token owed, in initiative order).
    #[must_use]
    pub const fn new(tokens: Tokens) -> Self {
        Self { tokens }
    }

    /// The payload (what the stack holds).
    #[must_use]
    pub const fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// Every token is placed (`TokenGain::is_complete`).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.tokens.pending.is_empty()
    }

    /// How many tokens the next seat still has to place in this window, this one included.
    fn left(&self) -> u8 {
        let p = self.tokens.pending.as_slice();
        let Some(first) = p.first() else { return 0 };
        u8::try_from(p.iter().take_while(|s| *s == first).count()).unwrap_or(u8::MAX)
    }
}

/// The suspension points: a token is owed, or none is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Owed,
    Complete,
}

impl Window for TokenGain {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        if self.is_complete() { Stage::Complete } else { Stage::Owed }
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        match self.tokens.pending.as_slice().first() {
            Some(_) => Next::Continue(Ask::Strategy(SAsk::GainPool { left: self.left() })),
            None => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        let who = *self.tokens.pending.as_slice().first()?;
        Some(gain_pool_choice(who, self.left()))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(SOpt::Pool(pool))) = pick else { return Err(Illegal) };
        let Some(&who) = self.tokens.pending.as_slice().first() else { return Err(Illegal) };
        if sx.state.players().by_seat(who).is_none() {
            return Err(Illegal);
        }
        fx::gain_token(sx.state, who, pool, 1);
        self.tokens.pending.remove(0);
        self.tokens.placed.push((who, pool));
        sx.plain(PlainRec::one(PlainKind::CommandTokenGained, PlainArg::Pool(pool)));
        Ok(())
    }
}

impl Held for TokenGain {
    const KIND: WindowKind = WindowKind::Tokens;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Tokens)? {
            OpenWindow::Tokens(t) => Some(Self { tokens: (**t).clone() }),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Tokens(Box::new(self.tokens.clone())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::strategy::Pool;
    use crate::containers::{Cow, SmallList};
    use crate::driver::live::{advance, ask, open, Env, Live};
    use crate::driver::NoSeeds;
    use crate::generated::Faction;
    use crate::ids::Seat;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Registry;
    use crate::state::{Player, Players, StatusReport};

    fn position() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s
    }

    fn owed(seats: &[u8]) -> TokenGain {
        TokenGain::new(Tokens {
            pending: SmallList::from_slice(&seats.iter().map(|s| Seat(*s)).collect::<Vec<_>>()),
            placed: SmallList::new(),
            report: StatusReport::default(),
        })
    }

    fn env(k: &Kernel) -> Env<'_> {
        Env { kernel: k, registry: Cow::new(Registry::default()), table: AbilityTable::default(), seeds: std::rc::Rc::new(NoSeeds) }
    }

    fn pool(p: Pool) -> Pick {
        Pick::Opt(Opt::Strategy(SOpt::Pool(p)))
    }

    #[test]
    fn two_seats_place_three_tokens_one_question_each_and_the_window_leaves_the_stack() {
        let (k, mut s) = (Kernel::plain(), position());
        let env = env(&k);
        open(&mut s, &owed(&[0, 0, 1]));
        let Live::Asks(q) = advance::<TokenGain>(&mut s, &env, None).unwrap().0 else { panic!("a token is owed") };
        assert_eq!((q.choice.actor, q.choice.ask), (Seat(0), Ask::Strategy(SAsk::GainPool { left: 2 })), "seat 0 places two, this one included");
        let Live::Asks(q) = advance::<TokenGain>(&mut s, &env, Some(pool(Pool::Fleet))).unwrap().0 else { panic!("the second token of seat 0") };
        assert_eq!((q.choice.actor, q.choice.ask), (Seat(0), Ask::Strategy(SAsk::GainPool { left: 1 })));
        let Live::Asks(q) = advance::<TokenGain>(&mut s, &env, Some(pool(Pool::Tactic))).unwrap().0 else { panic!("seat 1") };
        assert_eq!((q.choice.actor, q.choice.ask), (Seat(1), Ask::Strategy(SAsk::GainPool { left: 1 })));
        let (live, _) = advance::<TokenGain>(&mut s, &env, Some(pool(Pool::Strategic))).unwrap();
        assert_eq!(live, Live::Done);
        let econ = |n: u8| s.players().by_seat(Seat(n)).unwrap().econ.clone();
        assert_eq!((econ(0).fleet_tokens, econ(0).tactic_tokens, econ(1).strategic_tokens), (1, 1, 1));
        assert!(!crate::driver::live::is_open::<TokenGain>(&s));
        // The plain events, in order, as the string pushes them: `COMMAND_TOKEN_GAINED:<pool>`.
        let events: Vec<_> = s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect();
        assert_eq!(
            events,
            [Pool::Fleet, Pool::Tactic, Pool::Strategic].map(|p| PlainRec::one(PlainKind::CommandTokenGained, PlainArg::Pool(p)))
        );
    }

    #[test]
    fn the_stage_follows_the_tokens_still_owed() {
        assert_eq!(owed(&[0]).stage(), Stage::Owed);
        assert_eq!(owed(&[]).stage(), Stage::Complete);
        assert!(owed(&[]).is_complete() && !owed(&[1]).is_complete());
    }

    #[test]
    fn the_question_is_a_pure_read_of_the_stack_and_a_clone_continues_identically() {
        let (k, mut s) = (Kernel::plain(), position());
        let env = env(&k);
        open(&mut s, &owed(&[1, 0]));
        let asked = ask::<TokenGain>(&s, &env).unwrap().expect("a fresh window already knows its question");
        assert_eq!(asked.choice.actor, Seat(1));
        advance::<TokenGain>(&mut s, &env, None).unwrap();
        let mut fork = s.clone();
        for state in [&mut s, &mut fork] {
            advance::<TokenGain>(state, &env, Some(pool(Pool::Tactic))).unwrap();
            advance::<TokenGain>(state, &env, Some(pool(Pool::Fleet))).unwrap();
        }
        assert_eq!(s, fork);
    }

    #[test]
    fn a_pick_that_is_no_pool_changes_nothing_and_an_empty_reinforcement_still_consumes_the_offer() {
        let (k, mut s) = (Kernel::plain(), position());
        let env = env(&k);
        open(&mut s, &owed(&[0]));
        advance::<TokenGain>(&mut s, &env, None).unwrap();
        let held = s.clone();
        assert_eq!(advance::<TokenGain>(&mut s, &env, Some(Pick::Number(1))), Err(crate::driver::live::LiveError::Illegal));
        assert_eq!(s, held);
        // 20.4a: nothing in reinforcements, the token is not gained but the offer is answered.
        s.players.edit(0).econ.tactic_tokens = 16;
        let (live, _) = advance::<TokenGain>(&mut s, &env, Some(pool(Pool::Fleet))).unwrap();
        assert_eq!(live, Live::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, 0);
    }
}
