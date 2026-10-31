//! Steps 81.2 to 81.4 and the opening of the 81.5 token window (package KS3, lane C): `Game::begin_status_bookkeeping` and
//! `status::resolve_before_token_gain_with`, as ONE job of the turn flow ([`TurnJob::StatusBookkeeping`]) that composes the windows the
//! string composes in the string's order and asks every question through the tape (`turn::inline::run`).
//!
//! The order is the string's: the leaders that unlock (51.7), the reveal (81.2; an empty deck ends the game), the action cards each seat
//! draws in initiative order (81.3: the Neural Motivator's extra card, the faction hooks' bonus, Scheming's discard, the hand limit), the
//! command tokens that leave the board (81.4), the Neuraloop's offer, `STATUS_BOOKKEEPING_RESOLVED`, and the token window with the
//! report it carries (Cybernetic Enhancements' notes go home once their extra token is counted).
//!
//! By decision: Deepgloom Executable (`action_card_draw_requested`, a Thunder's Edge breakthrough) is out of scope, so the hook is not
//! run. The Minister of Policy's draw is recorded on the report and never performed, as in the string (`status::resolve_after_token_gain`).

use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::{SeatSet, SmallList};
use crate::driver::kernel_env::KernelEnv;
use crate::driver::turn::inline::{gap, halted, run, Flow};
use crate::driver::turn::start;
use crate::generated::{Ability, ActionCard, PromissoryNote, Relic, Technology};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::{fx, Kernel, Next};
use crate::query::seats::initiative_order;
use crate::rules::action_cards::draw::DrawCards;
use crate::rules::action_cards::hand_limit;
use crate::rules::leaders;
use crate::rules::relic_windows::Neuraloop;
use crate::rules::{abilities, objectives as objective_rules, trade};
use crate::state::{CoreState, ObjectiveRef, PlainArg, PlainKind, PlainRec, StatusReport, Tokens};

use crate::rules::token_gain::TokenGain;
use crate::kernel::stage::Held;

/// `tokens::STATUS_TOKENS`: the printed gain of the status phase.
const STATUS_TOKENS: i32 = 2;

/// `RULES_FACTION_BUGFIXES`: from here the status draw goes through the hand limit.
const RULES_FACTION_BUGFIXES: u32 = 17;

// -- Scheming ------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SStage {
    Start,
    Asking,
    Done,
}

/// Yssaril's Scheming, the discard half (`factions::yssaril::action_cards_drawn`): "Then, choose and discard 1 action card from your
/// hand." One option per printed name, in hand order, the first copy of each; no question with one. The discard is staged (the game
/// announces it with the next component or leader action).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemingDiscard {
    pub who: Seat,
    groups: Vec<u8>,
    stage: SStage,
    /// The table gave no printed-name groups: the window did nothing.
    pub unsupported: bool,
}

impl SchemingDiscard {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, groups: Vec::new(), stage: SStage::Start, unsupported: false }
    }

    /// The first card of each printed name, in hand order (`distinct_hand_options`).
    fn cards(&self, core: &CoreState) -> Option<Vec<ActionCard>> {
        let hand = core.players().by_seat(self.who)?.cards.hand.as_slice();
        let mut seen: Vec<u8> = Vec::new();
        let mut firsts = Vec::new();
        for card in hand {
            let group = *self.groups.get(*card as usize)?;
            if !seen.contains(&group) {
                seen.push(group);
                firsts.push(*card);
            }
        }
        Some(firsts)
    }
}

impl Window for SchemingDiscard {
    type Stage = SStage;

    fn stage(&self) -> SStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == SStage::Start {
            let Some(groups) = sx.table.facts.card_group_table().map(<[u8]>::to_vec) else {
                self.unsupported = true;
                self.stage = SStage::Done;
                return Next::Done;
            };
            self.groups = groups;
            match self.cards(sx.state).as_deref() {
                Some([]) | None => self.stage = SStage::Done,
                Some([only]) => {
                    fx::discard_action_card_staged(sx.state, self.who, *only);
                    self.stage = SStage::Done;
                }
                Some(_) => self.stage = SStage::Asking,
            }
        }
        if self.stage == SStage::Asking { Next::Continue(Ask::Cards(CAsk::OwnHand { reason: Reason::SchemingDiscard })) } else { Next::Done }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != SStage::Asking {
            return None;
        }
        let cards = self.cards(core)?;
        Some(Choice {
            actor: self.who,
            ask: Ask::Cards(CAsk::OwnHand { reason: Reason::SchemingDiscard }),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Ability(Ability::Scheming)),
                subtype: Subtype::Cards(CSub::SchemingDiscard),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: cards.into_iter().map(|c| Opt::Cards(COpt::Card(c))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (SStage::Asking, Pick::Opt(Opt::Cards(COpt::Card(card)))) if self.cards(sx.state).is_some_and(|c| c.contains(&card)) => {
                fx::discard_action_card_staged(sx.state, self.who, card);
                self.stage = SStage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- 81.3: one seat's draw ---------------------------------------------------------------------------------

/// One seat's draw of the status phase: the Neural Motivator's extra card, the hooks' bonus, the draw (the unofficial Veto cards leave the
/// deck first), Scheming's discard, the hand limit. The cards drawn.
fn draw(sx: &mut Sx<'_>, seat: Seat) -> Option<u32> {
    let motivated = sx.state.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(Technology::NeuralMotivator));
    let requested = 1 + usize::from(motivated);
    let requested = requested + sx.kernel.hooks.economy.action_card_draw_bonus(sx.state, sx.kernel, seat, requested);
    let drawn = fx::draw_action_cards(sx.state, seat, requested, true);
    if drawn == 0 {
        return Some(0);
    }
    // Scheming: "draw 1 additional action card. Then, choose and discard 1 action card from your hand" (the bonus above is the card).
    let schemes = sx.state.players().by_seat(seat).is_some_and(|p| p.faction == crate::generated::Faction::TheYssarilTribes);
    if schemes {
        let mut window = SchemingDiscard::new(seat);
        if run(sx, &mut window) == Flow::Halt {
            return None;
        }
        if window.unsupported {
            gap(sx, TurnGap::NoRoster);
            return None;
        }
    }
    // Rules version 17 (2.4): the draw goes through the same hand limit as every other draw, after Scheming's discard.
    if sx.state.meta().rules_at_least(RULES_FACTION_BUGFIXES) {
        let k = sx.kernel;
        let limit = hand_limit(sx.state, crate::query::reach::blanked(sx.state), |l| k.hooks.economy.action_card_limit(sx.state, k, seat, l));
        let held = sx.state.players().by_seat(seat).map_or(0, |p| p.cards.hand.len()); // mutants:skip: a seat that draws exists
        // mutants:skip: at the limit the window asks nothing either way (its own check is `held > limit`)
        if held > limit {
            let mut window = DrawCards::new(seat, 0, false); // mutants:skip: nothing is drawn, so nothing unofficial can be dropped again
            // mutants:skip: the driver rolls a dry step back whatever the job returns
            if run(sx, &mut window) == Flow::Halt {
                return None;
            }
            if window.unsupported {
                gap(sx, TurnGap::NoRoster);
                return None;
            }
        }
    }
    u32::try_from(drawn).ok()
}

/// The token gain the seats owe, in initiative order (`TokenGain::for_status`): the printed two, Versatile, Hyper Metabolism, the
/// Cybernetic Enhancements notes held, the modules' fold.
fn tokens_owed(core: &CoreState, kernel: &Kernel, order: &[Seat]) -> SmallList<Seat, 16> {
    let env = KernelEnv::new(core, kernel);
    let cx = abilities::Cx { core, env: &env };
    let mut pending = SmallList::new();
    for seat in order {
        for _ in 0..abilities::status_tokens(&cx, *seat, STATUS_TOKENS).max(0) { // mutants:skip: a negative fold is clamped to nothing owed
            pending.push(*seat);
        }
    }
    pending
}

/// The job.
pub fn run_bookkeeping(sx: &mut Sx<'_>) -> Flow {
    // 51.7 again: a commander unlocks on a condition that changes during play, not only when an objective is scored.
    let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    for seat in &seats {
        let Some(unlocking) = start::unlockable(sx, *seat, false) else { return Flow::Halt };
        leaders::unlock(sx.state, *seat, unlocking.as_slice());
        for leader in unlocking.as_slice() {
            sx.plain(PlainRec::one(PlainKind::LeaderUnlocked, PlainArg::Leader(*leader)));
        }
    }
    // 81.2: revealing is mandatory; with no objective left the game ends.
    let Some(revealed) = objective_rules::reveal_top(sx.state) else {
        sx.state.meta_mut().finished = true;
        sx.plain(PlainRec::bare(PlainKind::GameFinished));
        return Flow::Done;
    };
    let mut report = StatusReport { revealed_objective: Some(ObjectiveRef::Public(revealed)), ..StatusReport::default() };
    // 81.3: every draw uses the still-intact initiative order.
    let order: Vec<Seat> = initiative_order(sx.state).iter().copied().collect();
    report.initiative_order = order.iter().copied().collect();
    for seat in &order {
        let Some(drawn) = draw(sx, *seat) else { return Flow::Halt };
        report.action_cards_drawn.push((*seat, drawn));
    }
    // 81.4: the command tokens leave the board.
    let holding: Vec<_> = sx.state.board().systems().filter(|(_, c)| !c.cmd.is_empty()).map(|(s, c)| (s, c.cmd)).collect();
    for (system, cmd) in holding {
        for holder in cmd.iter() {
            report.returned_command_tokens.push((system, holder));
        }
        sx.state.board_mut().sys_mut(system).cmd = SeatSet::EMPTY;
    }
    if halted(sx) {
        return Flow::Halt;
    }
    // The Neuraloop reacts to the objective this reveal just turned up.
    let holder = sx.state.players().iter().find(|p| p.cards.relics.as_slice().contains(&Relic::Neuraloop)).map(|p| p.seat);
    if let Some(holder) = holder {
        let mut window = Neuraloop::new(holder, revealed);
        // mutants:skip: the driver rolls a dry step back whatever the job returns
        if run(sx, &mut window) == Flow::Halt {
            return Flow::Halt;
        }
        if let Some(replacement) = window.replacement {
            sx.plain(PlainRec::two(PlainKind::ObjectiveReplaced, PlainArg::Obj(ObjectiveRef::Public(revealed)), PlainArg::Obj(replacement)));
            report.revealed_objective = Some(replacement);
        }
    }
    sx.plain(PlainRec::bare(PlainKind::StatusBookkeepingResolved));
    let pending = tokens_owed(sx.state, sx.kernel, &order);
    // Cybernetic Enhancements: its extra token is already counted; the notes that paid for it go home now.
    if trade::return_all_foreign(sx.state, PromissoryNote::CyberneticEnhancements).is_none() {
        gap(sx, TurnGap::NoteOwner);
        return Flow::Halt;
    }
    TokenGain::new(Tokens { pending, placed: SmallList::new(), report }).to_pending(sx.state);
    Flow::Done
}
