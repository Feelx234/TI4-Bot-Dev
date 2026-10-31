//! Airo Shir Aur (Mahact hero; lent-bodies lane 9, the string `mahact_units::use_leader_timed`): "ACTION: Move all units in the space area of
//! any system to an adjacent system that contains a different player's ships. Space combat is resolved in that system. Neither player can
//! retreat or resolve abilities that would move their ships. Then, purge this card."
//!
//! The pairs (a system holding the seat's ships, an adjacent system with exactly one other seated player's ships and no neutral ones) are the
//! adapter's read of the map, in board order; a single pair is taken unasked. The units of the seat in the space area of the first system move
//! to the second, the fleet supply and the capacity are enforced there (a child, [`Spawn::Seeing`]), the active system is the destination and the
//! barring mark is set for the length of the fight, and the combat is [`FightToEnd`] as a child. When it is over the feats of the combat are
//! recorded against its occurrence, the mark and the active system go back, and the card resolved. The adapter prepares what the fight needs of
//! the string position ([`Prepared`]: the environment of the machine and the fields of a fresh window, the limits of the enforcement and the
//! snapshot of the sides), one per pair it vouches for.

use super::feats::{self, Outcome};
use super::fight::FightToEnd;
use super::machine::{Env, Fields};
use super::{ships_of, Rd};
use crate::choice::generic::Content;
use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Leader, System, UnitBaseType};
use crate::ids::Seat;
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::kernel::children::{ErasedChild, Spawn, SeeingLimits};
use crate::kernel::fx_move;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Before, CoreState};

/// What the fight of one pair needs of the string position.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub from: System,
    pub to: System,
    /// The limits the enforcement at the destination judges by.
    pub limits: SeeingLimits,
    /// The environment of the machine and the fields of a fresh window, as the string `settle` hands them over.
    pub env: Env,
    pub fields: Fields,
    /// The notes held when the fight opened (the sides and their capital ships are read when the fight starts, after the enforcement).
    pub before: Before,
}

/// The suspension points of [`MahactHero`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MStage {
    Start,
    /// Waiting on the pair (more than one qualifies).
    Pair,
    /// The fleet supply and the capacity are enforced at the destination (a child).
    Enforcing,
    /// The combat (a child).
    Fighting,
    Done,
}

#[derive(Debug, Clone)]
pub struct MahactHero {
    pub who: Seat,
    pairs: Vec<(System, System)>,
    prepared: Vec<Prepared>,
    stage: MStage,
    chosen: Option<usize>,
    enforce: ErasedChild,
    fight: Child<FightToEnd>,
    previous: Option<System>,
    /// The sides, their capital ships and the notes of the fight that started (the feats are decided from them).
    before: Option<Before>,
    /// The card resolved (the fight was fought).
    pub used: bool,
    /// The run holds something the window does not carry: a pair without a preparation, no link to the enforcement, a fight the machine
    /// does not carry. The adapter declines it.
    pub unsupported: bool,
    /// Why the machine stopped, when it did.
    pub bailed: Option<super::machine::Bail>,
}

impl MahactHero {
    /// `pairs` in board order; `prepared` for the pairs the adapter vouches for.
    #[must_use]
    pub fn new(who: Seat, pairs: Vec<(System, System)>, prepared: Vec<Prepared>) -> Self {
        Self { who, pairs, prepared, stage: MStage::Start, chosen: None, enforce: ErasedChild::idle(), fight: Child::idle(), previous: None, before: None, used: false, unsupported: false, bailed: None }
    }

    fn decline(&mut self) {
        self.unsupported = true;
        self.stage = MStage::Done;
    }

    /// The pair is chosen: the fleet moves and its destination is enforced.
    fn chosen(&mut self, sx: &mut Sx<'_>, at: usize) {
        self.chosen = Some(at);
        let (from, to) = self.pairs[at];
        let Some(prepared) = self.prepared.iter().find(|p| p.from == from && p.to == to) else { return self.decline() };
        let fleet: Vec<crate::state::Piece> = sx.state.board().sys(from).units.as_slice().iter().copied().filter(|p| p.owner() == self.who).collect();
        for piece in fleet {
            fx_move::move_ship(sx.state, from, to, piece);
        }
        let request = Spawn::Seeing { who: self.who, system: to, limits: prepared.limits };
        match sx.table.links.start(&request) {
            Some(window) => {
                self.enforce.start(window);
                self.stage = MStage::Enforcing;
            }
            None => self.decline(),
        }
    }

    /// The destination is enforced: the active system is the destination and the barring mark is set for the length of the fight.
    fn fight(&mut self, sx: &mut Sx<'_>) {
        let Some(at) = self.chosen else { return self.decline() };
        let (_, to) = self.pairs[at];
        let Some(prepared) = self.prepared.iter().find(|p| p.to == to && p.from == self.pairs[at].0).cloned() else { return self.decline() };
        self.previous = sx.state.meta().active_system;
        sx.state.meta_mut().active_system = Some(to);
        sx.state.marks_mut().tail.edit().insert(MarkKey::MahactBenediction(), MarkVal::Atom(Atom::System(to)));
        // `CombatWindow::new` and `before_combat` read the sides when the fight opens: after the fleet has moved and been enforced.
        let (env, mut fields, mut before) = (prepared.env, prepared.fields, prepared.before);
        let rd = Rd::new(sx.state, sx.kernel);
        fields.at_start = [env.attacker, env.defender]
            .into_iter()
            .map(|side| {
                let mut fleet: Vec<crate::generated::Unit> = ships_of(rd, side, to).iter().map(|p| p.kind()).collect();
                fleet.sort_unstable();
                (side, fleet)
            })
            .collect();
        before.sides = sx.state.seats().seating_order().iter().copied().filter(|seat| !ships_of(rd, *seat, to).is_empty()).collect();
        before.capitals = before
            .sides
            .as_slice()
            .iter()
            .map(|seat| {
                let n = sx.state.board().sys(to).units.as_slice().iter().filter(|p| p.owner() == *seat && matches!(p.kind().data().base_type, UnitBaseType::Warsun | UnitBaseType::Flagship)).count();
                (*seat, u16::try_from(n).unwrap_or(u16::MAX))
            })
            .collect();
        self.before = Some(before);
        self.fight.start(FightToEnd::new(env, fields));
        self.stage = MStage::Fighting;
    }

    /// The fight is over: the mark and the active system go back, the feats are recorded.
    fn fought(&mut self, sx: &mut Sx<'_>, fight: &FightToEnd) {
        sx.state.marks_mut().tail.edit().remove(&MarkKey::MahactBenediction());
        sx.state.meta_mut().active_system = self.previous;
        self.stage = MStage::Done;
        let Some((winner, rounds)) = fight.outcome() else {
            self.unsupported = true;
            self.bailed = fight.bailed;
            return;
        };
        let Some(at) = self.chosen else { return };
        let (_, to) = self.pairs[at];
        let Some(before) = self.before.take() else { return };
        // `complete_window`: a combat that began an occurrence notes the feats it earned against it.
        if fight.occurrence() > 0 {
            let list = feats::feats(Rd::new(sx.state, sx.kernel), to, &before, Outcome { winner, rounds });
            feats::record(sx.state, &list, fight.occurrence());
        }
        self.used = true;
    }

    fn options(&self) -> Opts {
        self.pairs.iter().map(|(from, to)| Opt::Leaders(LOpt::MahactMove(*from, *to))).collect::<Opts>()
    }
}

fn ctx() -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(Leader::AiroShirAur)),
        subtype: Subtype::Leaders(LSub::MahactMove),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

impl Window for MahactHero {
    type Stage = MStage;

    fn stage(&self) -> MStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                MStage::Start => match self.pairs.len() {
                    0 => self.stage = MStage::Done,
                    1 => self.chosen(sx, 0),
                    _ => self.stage = MStage::Pair,
                },
                MStage::Pair => return Next::Continue(Ask::Leaders(LAsk::MahactMove)),
                MStage::Enforcing => match self.enforce.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => self.fight(sx),
                },
                MStage::Fighting => match self.fight.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(fight)) => self.fought(sx, &fight),
                    None => self.decline(),
                },
                MStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            MStage::Pair => Some(Choice { actor: self.who, ask: Ask::Leaders(LAsk::MahactMove), ctx: ctx(), opts: self.options() }),
            MStage::Enforcing => self.enforce.ask_of(s, k),
            MStage::Fighting => self.fight.ask_of(s, k),
            MStage::Start | MStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (MStage::Enforcing, pick) => self.enforce.answer(sx, pick),
            (MStage::Fighting, pick) => self.fight.answer(sx, pick),
            (MStage::Pair, Pick::Opt(Opt::Leaders(LOpt::MahactMove(from, to)))) => {
                let at = self.pairs.iter().position(|pair| *pair == (from, to)).ok_or(Illegal)?;
                self.chosen(sx, at);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
