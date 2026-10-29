//! The offers that open a use of PRODUCTION or a payment, as [`Window`]s (package B3, the tier-W offers; docs/ENGINE_REACTIONS.md
//! section 9): the Hacan hero ("Harrugh Gefhara: make this use of PRODUCTION free", string `leaders::offer_production_hero`) and
//! the Keleres agent ("let any player spend commodities as if they were trade goods", string `keleres::offer_agent`).
//!
//! Neither is an ability of the resolver: the string asks them between the windows of the production, directly of the seat.
//! Each is a window of its own with one question per seat that could answer.

use crate::choice::generic::Content;
use crate::choice::production::{Ask as PAsk, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::{Opt32, SmallList};
use crate::generated::{Faction, Leader};
use crate::ids::Seat;
use crate::kernel::ability::Unknown;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::state::{CoreState, LeaderStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

fn ctx(subtype: PSub, who: Leader) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(who)),
        subtype: Subtype::Production(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

fn opts() -> Opts {
    [Opt::Production(POpt::Use), Opt::Decline].into_iter().collect()
}

/// Harrugh Gefhara, offered to the producer when a use of PRODUCTION starts.
#[derive(Debug, Clone, Copy)]
pub struct HeroOffer {
    pub who: Seat,
    /// The producer purged the hero: this use is free.
    pub used: bool,
    stage: Stage,
}

impl HeroOffer {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, used: false, stage: Stage::Start }
    }

    fn unlocked(s: &CoreState, who: Seat) -> bool {
        s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::HarrughGefhara) == Some(&LeaderStatus::Unlocked))
    }
}

impl Window for HeroOffer {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if Self::unlocked(sx.state, self.who) { Stage::Asking } else { Stage::Done };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Production(PAsk::Hero)),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice { actor: self.who, ask: Ask::Production(PAsk::Hero), ctx: ctx(PSub::Hero, Leader::HarrughGefhara), opts: opts() })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Production(POpt::Use)) => {
                let seq = sx.state.meta().production_seq;
                if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                    p.production_timers.free_production_use = Opt32::some(seq);
                    p.leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Purged);
                }
                self.used = true;
            }
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}

/// Xander Alexin Victori III, offered to each Keleres holder of a readied agent when `payer` pays (or starts a use of
/// PRODUCTION) holding commodities: if one uses it the payer's commodities count as trade goods until the window is closed;
/// if every holder declines, the window stays open without them.
#[derive(Debug, Clone)]
pub struct AgentOffer {
    pub payer: Seat,
    holders: SmallList<Seat, 8>,
    at: usize,
    /// The offer opened a window the caller must close (a holder answered, or every one declined).
    pub opened: bool,
    stage: Stage,
}

impl AgentOffer {
    #[must_use]
    pub fn new(payer: Seat) -> Self {
        Self { payer, holders: SmallList::new(), at: 0, opened: false, stage: Stage::Start }
    }

    fn keleres(f: Faction) -> bool {
        matches!(f, Faction::TheCouncilKeleresArgent | Faction::TheCouncilKeleresMentak | Faction::TheCouncilKeleresXxcha)
    }

    /// `keleres::agent_holders`: the Keleres seats that could use the agent for `payer` now.
    fn holders_of(s: &CoreState, payer: Seat) -> SmallList<Seat, 8> {
        let mut out = SmallList::new();
        if s.marks().tail.contains_key(&MarkKey::KeleresAgentGoods(payer)) || s.players().by_seat(payer).is_none_or(|p| p.econ.commodities <= 0) {
            return out;
        }
        for p in s.players().iter() {
            if Self::keleres(p.faction) && p.leaders.status.get(&Leader::XanderAlexinVictoriIII) == Some(&LeaderStatus::Readied) {
                out.push(p.seat);
            }
        }
        out
    }

    fn mark(s: &mut CoreState, payer: Seat, value: Atom) {
        s.marks_mut().tail.edit().insert(MarkKey::KeleresAgentGoods(payer), MarkVal::Atom(value));
    }
}

impl Window for AgentOffer {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.holders = Self::holders_of(sx.state, self.payer);
            self.stage = if self.holders.is_empty() { Stage::Done } else { Stage::Asking };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Production(PAsk::AgentOffer { payer: self.payer })),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        let holder = *self.holders.as_slice().get(self.at)?;
        (self.stage == Stage::Asking).then(|| Choice {
            actor: holder,
            ask: Ask::Production(PAsk::AgentOffer { payer: self.payer }),
            ctx: ctx(PSub::AgentOffer, Leader::XanderAlexinVictoriIII),
            opts: opts(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Some(&holder) = self.holders.as_slice().get(self.at) else { return Err(Illegal) };
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let mut next = true;
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Production(POpt::Use)) => {
                // `leaders::exhaust`: the agent is exhausted; a Nomad that hears it is told (the core does not tell).
                let ready = sx.state.players().by_seat(holder).is_some_and(|p| p.leaders.status.get(&Leader::XanderAlexinVictoriIII) == Some(&LeaderStatus::Readied));
                if ready {
                    if sx.kernel.hooks.direct.nomad_watches_agent_exhaustion(sx.state) {
                        sx.tape.unknown.set(Some(Unknown::NomadAgentExhaustion));
                    }
                    if let Some(p) = sx.state.players_mut().by_seat_mut(holder) {
                        p.leaders.status.insert(Leader::XanderAlexinVictoriIII, LeaderStatus::Exhausted);
                    }
                    Self::mark(sx.state, self.payer, Atom::Seat(holder));
                    self.opened = true;
                    self.stage = Stage::Done;
                    next = false;
                }
            }
            _ => return Err(Illegal),
        }
        if next {
            self.at += 1;
            if self.at >= self.holders.len() {
                Self::mark(sx.state, self.payer, Atom::Word(Word::Declined));
                self.opened = true;
                self.stage = Stage::Done;
            }
        }
        Ok(())
    }
}
