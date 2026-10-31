//! A draw of action cards the way the string's `action_cards::draw` makes it, faction hooks included (lane D, KV3): the Politics Rider,
//! Unconventional Measures and every other window that spawns `Spawn::DrawCards`.
//!
//! Without a hook it is [`DrawCards`] itself. With one: the bonus card of Yssaril's Scheming joins the draw, Scheming's discard
//! ([`SchemingDiscard`]) comes before the limit, and the hand limit is the faction's own (Crafty has none). The status phase's draw
//! (`status_bookkeeping::draw`) is the same sequence with its own request.

use super::status_bookkeeping::SchemingDiscard;
use crate::choice::{Choice, Pick};
use crate::generated::Faction;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::query::reach;
use crate::rules::action_cards::draw::DrawCards;
use crate::rules::action_cards::hand_limit;
use crate::state::CoreState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HStage {
    Start,
    /// No hook changes the draw: the plain window runs.
    Plain,
    Scheming,
    Limit,
    Done,
}

/// The draw of `count` cards for `who`.
#[derive(Debug, Clone)]
pub struct HookedDraw {
    who: Seat,
    count: usize,
    drop_unofficial: bool,
    stage: HStage,
    plain: DrawCards,
    scheming: Option<SchemingDiscard>,
    limit: Option<DrawCards>,
    /// The table gave no printed-name groups: a discard could not be asked (the caller declines the run).
    pub unsupported: bool,
}

impl HookedDraw {
    #[must_use]
    pub fn new(who: Seat, count: usize, drop_unofficial: bool) -> Self {
        Self { who, count, drop_unofficial, stage: HStage::Start, plain: DrawCards::new(who, count, drop_unofficial), scheming: None, limit: None, unsupported: false }
    }

    fn finish(&mut self) -> Next {
        self.stage = HStage::Done;
        Next::Done
    }

    /// Run the child at `stage`; its question is this window's.
    fn child(next: Next, step_done: &mut bool) -> Option<Next> {
        match next {
            Next::Continue(ask) => Some(Next::Continue(ask)),
            _ => {
                *step_done = true;
                None
            }
        }
    }
}

impl Window for HookedDraw {
    type Stage = HStage;

    fn stage(&self) -> HStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == HStage::Start {
            let k = sx.kernel;
            let bonus = if self.count > 0 { k.hooks.economy.action_card_draw_bonus(sx.state, k, self.who, self.count) } else { 0 };
            let blanked = reach::blanked(sx.state);
            let yssaril = sx.state.players().by_seat(self.who).is_some_and(|p| p.faction == Faction::TheYssarilTribes);
            let limit_hooked = hand_limit(sx.state, blanked, |l| k.hooks.economy.action_card_limit(sx.state, k, self.who, l)) != hand_limit(sx.state, blanked, |l| l);
            if bonus == 0 && !yssaril && !limit_hooked && !blanked {
                self.stage = HStage::Plain;
            } else {
                // Scheming: "draw 1 additional action card", then "choose and discard 1 action card from your hand", before the limit.
                let drawn = fx::draw_action_cards(sx.state, self.who, self.count + bonus, self.drop_unofficial);
                self.stage = if drawn > 0 && yssaril {
                    self.scheming = Some(SchemingDiscard::new(self.who));
                    HStage::Scheming
                } else {
                    HStage::Limit
                };
            }
        }
        loop {
            match self.stage {
                HStage::Plain => {
                    let next = self.plain.step(sx);
                    if self.plain.unsupported {
                        self.unsupported = true;
                    }
                    return next;
                }
                HStage::Scheming => {
                    let Some(window) = self.scheming.as_mut() else { return self.finish() };
                    let mut done = false;
                    if let Some(next) = Self::child(window.step(sx), &mut done) {
                        return next;
                    }
                    if window.unsupported {
                        self.unsupported = true;
                        return self.finish();
                    }
                    self.stage = HStage::Limit;
                }
                HStage::Limit => {
                    let k = sx.kernel;
                    let blanked = reach::blanked(sx.state);
                    let limit = hand_limit(sx.state, blanked, |l| k.hooks.economy.action_card_limit(sx.state, k, self.who, l));
                    let held = sx.state.players().by_seat(self.who).map_or(0, |p| p.cards.hand.len());
                    if self.limit.is_none() {
                        if held <= limit {
                            return self.finish();
                        }
                        self.limit = Some(DrawCards::new(self.who, 0, false));
                    }
                    let Some(window) = self.limit.as_mut() else { return self.finish() };
                    let next = window.step(sx);
                    if window.unsupported {
                        self.unsupported = true;
                    }
                    return match next {
                        Next::Continue(ask) => Next::Continue(ask),
                        _ => self.finish(),
                    };
                }
                HStage::Start | HStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            HStage::Plain => self.plain.ask_of(s, k),
            HStage::Scheming => self.scheming.as_ref().and_then(|w| w.ask_of(s, k)),
            HStage::Limit => self.limit.as_ref().and_then(|w| w.ask_of(s, k)),
            HStage::Start | HStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            HStage::Plain => self.plain.answer(sx, pick),
            HStage::Scheming => self.scheming.as_mut().ok_or(Illegal)?.answer(sx, pick),
            HStage::Limit => self.limit.as_mut().ok_or(Illegal)?.answer(sx, pick),
            HStage::Start | HStage::Done => Err(Illegal),
        }
    }
}
