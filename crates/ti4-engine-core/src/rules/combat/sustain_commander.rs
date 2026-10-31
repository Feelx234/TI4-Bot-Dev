//! Worked example: the Rear Admiral Farran offer of the combat window (string `ask_sustain_commander`,
//! `combat.rs`), as a [`Window`].
//!
//! LRR 16 of the rules version 16: "After 1 of your units uses SUSTAIN DAMAGE: you may gain 1 trade
//! good", asked once per use, only of the holder of an unlocked Letnev commander. Accept: the trade good
//! is gained and `TRADE_GOODS_GAINED` is staged (the string `note_trade_goods_gained`), to be announced by
//! whoever flushes the staged events.
//!
//! Shape to copy (B1, B3): a `Stage` with one variant per suspension point; `step` only moves between
//! stages and returns `Next::Continue(ask)` at a question; `ask_of` is a pure function of `(window, state)`;
//! `answer` is the only place the answer changes the state.

use crate::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::Leader;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::LeaderStatus;
use crate::state::{Code, CoreState};

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Does the holder have the commander? (no question yet)
    Check,
    /// Waiting on the holder's answer.
    Asking,
    Done,
}

/// The offer to `player`. `source` is the interned text of the source of the trade good
/// (`"letnevcommander"`), which the adapter owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SustainCommander {
    pub player: Seat,
    pub source: Code,
    stage: Stage,
}

/// Whether the seat holds an unlocked Rear Admiral Farran. Only the leader slice is read; the other ways
/// to hold the ability (a granted mark, the Mahact Imperia, an "An" promissory note) belong to package B7's
/// `has_commander_ability`, and the dual skips a game position where one of them is in play.
#[must_use]
pub fn pays_on_sustain(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).and_then(|p| p.leaders.status.get(&Leader::RearAdmiralFarran).copied()) == Some(LeaderStatus::Unlocked)
}

impl SustainCommander {
    #[must_use]
    pub const fn new(player: Seat, source: Code) -> Self {
        Self { player, source, stage: Stage::Check }
    }

    const ASK: Ask = Ask::Leaders(LAsk::SustainCommander);

    /// The offer already answered (below `RULES_FACTION_EFFECTS` the trade good is paid without a question, by the
    /// window that hosts this one): it asks nothing and ends at once.
    #[must_use]
    pub const fn settled(player: Seat, source: Code) -> Self {
        Self { player, source, stage: Stage::Done }
    }
}

impl Window for SustainCommander {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Check {
            self.stage = if pays_on_sustain(sx.state, self.player) { Stage::Asking } else { Stage::Done };
        }
        match self.stage {
            Stage::Asking => Next::Continue(Self::ASK),
            Stage::Check | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.player,
            ask: Self::ASK,
            ctx: Ctx {
                origin: Origin::Content(Content::Leader(Leader::RearAdmiralFarran)),
                subtype: Subtype::Leaders(LSub::SustainCommander),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: [Opt::Leaders(LOpt::GainTradeGood), Opt::Decline].into_iter().collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Leaders(LOpt::GainTradeGood)) => {
                fx::gain_trade_goods_staged(sx.state, self.player, 1, self.source);
            }
            Pick::Opt(Opt::Decline) => {}
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{EventKind, Player};
    use crate::generated::Faction;

    fn game(unlocked: bool) -> CoreState {
        let mut s = CoreState::all_loaded();
        let mut p = Player::new(Seat(0), Faction::from_index(0).unwrap());
        if unlocked {
            p.leaders.status.insert(Leader::RearAdmiralFarran, LeaderStatus::Unlocked);
        }
        s.players = crate::state::Players::from_vec(vec![p]);
        s
    }

    fn driver() -> Driver<SustainCommander> {
        Driver::new(SustainCommander::new(Seat(0), Code(7)), Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    #[test]
    fn a_holder_is_asked_and_accepting_gains_a_trade_good_and_stages_the_event() {
        let (k, mut s) = (Kernel::plain(), game(true));
        let mut d = driver();
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.actor, Seat(0));
        assert_eq!(c.opts.as_slice(), &[Opt::Leaders(LOpt::GainTradeGood), Opt::Decline]);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "nothing changes before the answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Leaders(LOpt::GainTradeGood))).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1);
        assert_eq!(s.pending().staged.len(), 1);
        assert_eq!(s.pending().staged[0].kind, EventKind::TradeGoodsGained);
        assert!(d.is_done());
    }

    #[test]
    fn declining_changes_nothing_and_an_illegal_pick_is_refused_without_effect() {
        let (k, mut s) = (Kernel::plain(), game(true));
        let mut d = driver();
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Number(3)), Err(crate::driver::DriverError::Illegal));
        let before = s.clone();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(s, before);
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(crate::driver::DriverError::NotAsking));
    }

    #[test]
    fn without_the_commander_nothing_is_asked() {
        let (k, mut s) = (Kernel::plain(), game(false));
        assert_eq!(driver().run(&mut s, &k).unwrap(), Status::Done);
    }
}
