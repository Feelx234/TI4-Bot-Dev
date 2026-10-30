//! The start of one ground combat round (LRR 42.1; the counting part of the string `begin_ground_round`), as a
//! [`Window`] that never asks.
//!
//! The round counter of the active invasion goes up when the active fight is on this planet against this
//! defender, and from rules version 16 the ground round is a numbered combat round (`combat_round_seq`), so
//! what a card marks "for this round" cannot outlive it.
//!
//! A copied round agent (the Sol agent lent by Yssaril, or the Sol agent readied) makes the round numbered in any
//! rules version and announces `GROUND_COMBAT_ROUND_STARTED`.
//!
//! The faction effects at the start of the round are the hooks of `ground_combat_round_started` (Naaz: the Eidolon Maximum is
//! repaired and Supercharge is offered; the question rides on the tape of the step). Not covered (the adapter returns `None`, the
//! string result stands): Proxima Targeting VI, nested activity.
//!
//! The Dunlain Reaper (Letnev, package W2): after the faction effects, each participant that is Letnev with an infantry on the planet and a mech
//! in the reinforcements, who can pay 2 resources (`Facts::can_pay`), is asked; the payment is a child window of the production area
//! (`Spawn::Bill`) and a paid deploy replaces the first infantry by the mech.

use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Faction, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Report, Spawn};
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::economy::Spend;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Piece, Val};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// The Dunlain Reaper of participant `at` (0: the invader, 1: the defender).
    Dunlain,
    Asking,
    Paying,
    Done,
}

/// The facts of one round's start.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub system: System,
    pub planet: Planet,
    pub invader: Seat,
    pub defender: Seat,
    /// A copied round agent listens: the round is numbered and announced (the resolver carries the timing).
    pub copied: bool,
    /// `rules_version >= RULES_FACTION_EFFECTS`: the ground round is a numbered combat round.
    pub numbered: bool,
    /// Per participant (invader, defender): at least 2 resources can be spent (the string's `production::available`).
    pub can_pay: [bool; 2],
    /// The interned source `dunlain_reaper` of the Reaper's question.
    pub dunlain: Code,
}

/// The cost of the Reaper's deploy, in resources.
const REAPER_COST: i64 = 2;

#[derive(Debug, Clone)]
pub struct BeginGroundRound {
    pub f: Facts,
    stage: Stage,
    at: usize,
    pay: ErasedChild,
}

impl BeginGroundRound {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, stage: Stage::Start, at: 0, pay: ErasedChild::idle() }
    }

    fn seat(&self) -> Seat {
        if self.at == 0 { self.f.invader } else { self.f.defender }
    }

    /// Whether the Reaper is offered to participant `at`: a Letnev seat with an infantry in this combat, a mech in the reinforcements
    /// that no effect forbids placing, and 2 resources to spend.
    fn offered(&self, s: &CoreState, k: &Kernel) -> bool {
        let who = self.seat();
        let scope = crate::tables::Scope(u32::from(k.sources.0));
        s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev)
            && s.board().planet(self.f.planet).units.iter().any(|u| u.owner() == who && u.kind().data().base_type == UnitBaseType::Infantry)
            && !k.hooks.economy.effect_placement_forbidden(s, k, who, Unit::DunlainReaper)
            && crate::query::fleet::remaining_all(s, scope, who, Unit::DunlainReaper) > 0
            && self.f.can_pay[self.at]
    }

    /// "Replace 1 of your infantry in that combat with 1 mech": the first infantry of the seat on the planet takes the mech's place.
    fn replace(&self, s: &mut CoreState) {
        let who = self.seat();
        let cell = s.board_mut().planet_mut(self.f.planet);
        if let Some(at) = cell.units.as_slice().iter().position(|u| u.owner() == who && u.kind().data().base_type == UnitBaseType::Infantry) {
            cell.units.as_mut_slice()[at] = Piece::new(Unit::DunlainReaper, who);
        }
    }
}

impl Window for BeginGroundRound {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage != Stage::Start {
            return self.reaper(sx);
        }
        let f = self.f;
        if let Some(active) = sx.state.transient_mut().invasion.active.as_mut()
            && active.planet == Some(f.planet)
            && active.defender == Some(f.defender)
        {
            active.edit().ground_round += 1;
        }
        if f.numbered || f.copied {
            sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        }
        if f.copied {
            let payload = Payload::new()
                .with(Key::System, Val::System(f.system))
                .with(Key::Planet, Val::Planet(f.planet))
                .with(Key::Attacker, Val::Seat(f.invader))
                .with(Key::Defender, Val::Seat(f.defender))
                .with(Key::RoundSeq, Val::Int(i64::from(sx.state.meta().combat_round_seq)));
            let _ = sx.emit(fx::event(EventKind::GroundCombatRoundStarted, payload));
        }
        // The faction effects of the start of the round, for each participant (Naaz: the Eidolon Maximum is repaired and Supercharge is
        // offered). A question goes to the tape: the step is run again with the answer.
        for who in [f.invader, f.defender] {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.ground.ground_combat_round_started(&mut cx, who, f.system, f.planet);
        }
        self.stage = Stage::Dunlain;
        self.reaper(sx)
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Asking => {
                let system = self.f.system;
                let invasion_seq = s.transient().invasion.active.as_ref().map(|a| a.seq);
                let _ = k;
                Some(Choice {
                    actor: self.seat(),
                    ask: Ask::Combat(CAsk::DeployMech { planet: self.f.planet }),
                    ctx: Ctx {
                        origin: Origin::Content(Content::Other(self.f.dunlain)),
                        subtype: Subtype::Combat(CSub::DeployMech),
                        optional: false,
                        target: Some(Target::Planet { system, planet: self.f.planet }),
                        outstanding: SmallList::new(),
                        space_battle: false,
                        invasion_seq,
                    },
                    opts: Opts([Opt::Combat(COpt::DeployMech), Opt::Decline].into_iter().collect()),
                })
            }
            Stage::Paying => self.pay.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            Stage::Asking => match pick {
                Pick::Opt(Opt::Decline) => {
                    self.at += 1;
                    self.stage = Stage::Dunlain;
                    Ok(())
                }
                Pick::Opt(Opt::Combat(COpt::DeployMech)) => {
                    let who = self.seat();
                    let Some(window) = sx.table.links.start(&Spawn::Bill { who, kind: Spend::Resources, cost: REAPER_COST }) else {
                        return Err(Illegal);
                    };
                    self.pay.start(window);
                    self.stage = Stage::Paying;
                    Ok(())
                }
                _ => Err(Illegal),
            },
            Stage::Paying => self.pay.answer(sx, pick),
            _ => Err(Illegal),
        }
    }
}

impl BeginGroundRound {
    /// The Dunlain Reaper's offers, participant by participant.
    fn reaper(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Done => return Next::Done,
                Stage::Start => return Next::Done,
                Stage::Dunlain => {
                    if self.at >= 2 {
                        self.stage = Stage::Done;
                        continue;
                    }
                    if self.offered(sx.state, sx.kernel) {
                        self.stage = Stage::Asking;
                        return Next::Continue(Ask::Combat(CAsk::DeployMech { planet: self.f.planet }));
                    }
                    self.at += 1;
                }
                Stage::Asking => return Next::Continue(Ask::Combat(CAsk::DeployMech { planet: self.f.planet })),
                Stage::Paying => match self.pay.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(report)) => {
                        if report == Report::Paid(true) {
                            self.replace(sx.state);
                        }
                        self.at += 1;
                        self.stage = Stage::Dunlain;
                    }
                    None => {
                        self.at += 1;
                        self.stage = Stage::Dunlain;
                    }
                },
            }
        }
    }
}
