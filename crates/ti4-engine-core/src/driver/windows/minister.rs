//! Minister of War at the end of a turn (package D1b2; `Game::minister_of_war`): "the owner of this card may discard this card
//! after performing an action to remove 1 of their command tokens from the game board and return it to their reinforcements
//! -- then they may perform 1 additional action". The offer to discard is `rules::agenda::offer_discard::OfferDiscard`; the
//! token to retrieve is asked here ("which command token comes back"), the token goes back to the tactic pool and the turn is
//! retained (the additional-action flag).
//!
//! The systems where the owner has a token are the caller's facts, in the string's order (system ids by text); a table with no
//! token on the board is never asked (22.3).

use crate::choice::turn::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::choice::generic::Rule;
use crate::generated::{Agenda, System};
use crate::ids::Seat;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::rules::agenda::offer_discard::OfferDiscard;
use crate::state::{Code, CoreState, TransientFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Offering,
    Choosing,
    Done,
}

#[derive(Debug, Clone)]
pub struct MinisterOfWar {
    owner: Seat,
    systems: Vec<System>,
    law: Agenda,
    prompt: Code,
    source: Code,
    stage: Stage,
    offer: Child<OfferDiscard>,
    /// The law was discarded.
    pub discarded: bool,
    /// The system the token came back from.
    pub retrieved: Option<System>,
}

impl MinisterOfWar {
    /// `systems`: where `owner` has a command token (the string's order); `prompt` and `source` are the interned prompt of the
    /// offer and the law's alias.
    #[must_use]
    pub fn new(owner: Seat, systems: Vec<System>, law: Agenda, prompt: Code, source: Code) -> Self {
        Self { owner, systems, law, prompt, source, stage: Stage::Offering, offer: Child::idle(), discarded: false, retrieved: None }
    }
}

impl Window for MinisterOfWar {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Offering => {
                    if self.systems.is_empty() {
                        self.stage = Stage::Done;
                        continue;
                    }
                    if !self.offer.running() {
                        self.offer.start(OfferDiscard::new(self.law, self.owner, self.prompt, self.source));
                    }
                    match self.offer.step(sx) {
                        Some(Ran::Asks(ask)) => return Next::Continue(ask),
                        Some(Ran::Pass(next)) => return next,
                        Some(Ran::Done(offer)) => {
                            self.discarded = offer.taken();
                            self.stage = if self.discarded { Stage::Choosing } else { Stage::Done };
                        }
                        None => self.stage = Stage::Done,
                    }
                }
                Stage::Choosing => return Next::Continue(Ask::Turn(TAsk::MinisterToken)),
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Offering => self.offer.ask_of(s, k),
            Stage::Choosing => Some(Choice {
                actor: self.owner,
                ask: Ask::Turn(TAsk::MinisterToken),
                ctx: Ctx { origin: Origin::Rule(Rule::R22), subtype: Subtype::Turn(TSub::MinisterToken), optional: false, target: None, outstanding: Default::default(), space_battle: false, invasion_seq: None }, // mutants:skip: the string asks this question without a context, so the lockstep never reads these flags
                opts: self.systems.iter().map(|system| Opt::Turn(TOpt::Token(*system))).collect::<Opts>(),
            }),
            Stage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            Stage::Offering => self.offer.answer(sx, pick),
            Stage::Choosing => {
                let Pick::Opt(Opt::Turn(TOpt::Token(system))) = pick else { return Err(Illegal) };
                if !self.systems.contains(&system) {
                    return Err(Illegal);
                }
                sx.state.board_mut().sys_mut(system).cmd.remove(self.owner);
                crate::kernel::fx::gain_token(sx.state, self.owner, crate::choice::strategy::Pool::Tactic, 1);
                sx.state.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
                self.retrieved = Some(system);
                self.stage = Stage::Done;
                Ok(())
            }
            Stage::Done => Err(Illegal),
        }
    }
}
