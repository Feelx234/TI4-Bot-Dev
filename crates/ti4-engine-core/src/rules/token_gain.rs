//! The command token gain of the status phase (81.5; `tokens::TokenGain` and `Game::step_token_gain`) as a HELD window: the first
//! arm run natively end to end (package M0 vertical slice, `docs/D2_LF_RESUME.md`; lane C extends it as KS4).
//!
//! The payload on the stack is `state::Tokens` (who still owes a token, what was placed, the status report it travels with);
//! the machine is that payload and nothing else, so `from_pending(to_pending(m)) == m`.
//!
//! NO QUESTION (multi-ask conversion, `docs/MULTIASK1_RESUME.md`): where a token goes is dead information here, because every seat
//! that holds a token is asked the redistribute question right after (81.5, second sentence: [`Redistribute`] builds its options from
//! the sheet total and the floor only). The window places every owed token into the canonical pool [`STATUS_GAIN_POOL`] (at most what
//! is in reinforcements, 20.4a) and queues the plain `COMMAND_TOKEN_GAINED:<pool>` per token, then ends.
//!
//! What stays the string's (KS5): the close, steps 81.6-81.8 (`finish_status_phase`), which runs after the last token.
//!
//! [`Redistribute`]: crate::driver::windows::status_finish::Redistribute

use crate::choice::strategy::Pool;
use crate::choice::{Choice, Pick};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::{CoreState, PlainArg, PlainKind, PlainRec, Tokens, Window as OpenWindow, WindowKind};

/// The pool the status phase gain puts its tokens in (the redistribute question that follows decides where they end up).
pub const STATUS_GAIN_POOL: Pool = Pool::Tactic;

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

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        for who in std::mem::take(&mut self.tokens.pending).as_slice() {
            if sx.state.players().by_seat(*who).is_none() {
                continue;
            }
            fx::gain_token(sx.state, *who, STATUS_GAIN_POOL, 1);
            self.tokens.placed.push((*who, STATUS_GAIN_POOL));
            sx.plain(PlainRec::one(PlainKind::CommandTokenGained, PlainArg::Pool(STATUS_GAIN_POOL)));
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
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

    /// The multi-ask conversion: the status phase gain asks nothing; every token lands in the canonical pool and the window leaves the stack.
    #[test]
    fn the_status_gain_asks_nothing_and_places_every_token_in_the_canonical_pool() {
        let (k, mut s) = (Kernel::plain(), position());
        let env = env(&k);
        open(&mut s, &owed(&[0, 0, 1]));
        assert_eq!(ask::<TokenGain>(&s, &env).unwrap(), None, "no question");
        let (live, _) = advance::<TokenGain>(&mut s, &env, None).unwrap();
        assert_eq!(live, Live::Done);
        let econ = |n: u8| s.players().by_seat(Seat(n)).unwrap().econ.clone();
        assert_eq!((econ(0).tactic_tokens, econ(0).fleet_tokens, econ(1).tactic_tokens), (2, 0, 1));
        assert!(!crate::driver::live::is_open::<TokenGain>(&s));
        let events: Vec<_> = s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect();
        assert_eq!(events, [PlainRec::one(PlainKind::CommandTokenGained, PlainArg::Pool(STATUS_GAIN_POOL)); 3]);
    }

    #[test]
    fn the_stage_follows_the_tokens_still_owed() {
        assert_eq!(owed(&[0]).stage(), Stage::Owed);
        assert_eq!(owed(&[]).stage(), Stage::Complete);
        assert!(owed(&[]).is_complete() && !owed(&[1]).is_complete());
    }

    #[test]
    fn a_pick_is_refused_and_an_empty_reinforcement_still_consumes_the_gain() {
        let (k, mut s) = (Kernel::plain(), position());
        let env = env(&k);
        open(&mut s, &owed(&[0]));
        let held = s.clone();
        assert_eq!(advance::<TokenGain>(&mut s, &env, Some(Pick::Number(1))), Err(crate::driver::live::LiveError::NotAsking));
        assert_eq!(s, held);
        // 20.4a: nothing in reinforcements, the token is not gained but the gain is consumed.
        s.players.edit(0).econ.fleet_tokens = 16;
        let (live, _) = advance::<TokenGain>(&mut s, &env, None).unwrap();
        assert_eq!(live, Live::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens, 0);
    }
}
