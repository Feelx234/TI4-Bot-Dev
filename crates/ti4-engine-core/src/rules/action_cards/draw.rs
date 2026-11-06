//! Drawing action cards and the hand limit (package W6; string `action_cards::draw`, `enforce_hand_limit`) as a [`Window`].
//!
//! The windows that draw cards (the Politics abilities of the strategy cards, the Politics and Keleres Riders, Unconventional
//! Measures) ran the draw only where the hand stayed within its limit: over it the string asks which card to discard (rule
//! 2.4) and the draw was a documented bail. [`DrawCards`] is that draw with its question; a window of another area runs it as
//! a child (`Spawn::DrawCards`).
//!
//! The draw follows the hooks of the kernel (package W14): a bonus card with the draw (`economy.action_card_draw_bonus`: Yssaril's Scheming, a
//! test hook), the hand limit the faction hooks override (`economy.action_card_limit`: Yssaril's Crafty), and Scheming's own "Then, choose and
//! discard 1 action card from your hand", asked as `OwnHand { Scheming }` after the draw and before the limit (one card of a name left in the
//! hand discards it without a question), the discard staged as the string stages it. What stays the string's, by the caller: the Deepgloom
//! Executable (a Thunder's Edge breakthrough that lets another seat borrow Scheming) and a deck reserve (a dropped feature). The Data Skimmer
//! (Ral Nel, never seated) would take a discard.

use crate::choice::cards::{Ask as CAsk, Opt as COpt, Reason, Subtype as CSub};
use crate::choice::generic::{Content, Rule};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Ability, ActionCard};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::CoreState;

/// The suspension points of [`DrawCards`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DStage {
    Start,
    /// Waiting on Scheming's "choose and discard 1 action card from your hand".
    Scheming,
    /// Waiting on "over the hand limit: discard one".
    Asking,
    Done,
}

/// `action_cards::draw`: up to `count` cards from the top of the deck, then discards (one question each) until the hand is
/// within its limit.
#[derive(Debug, Clone)]
pub struct DrawCards {
    pub who: Seat,
    count: usize,
    /// The unofficial cards leave the deck before the draw (`deck::drop_unofficial_action_cards`).
    drop_unofficial: bool,
    stage: DStage,
    /// The printed-name group of each card, taken from the table's facts when the window starts.
    groups: Vec<u8>,
    /// The adapter gave no printed-name groups: the window did nothing and the caller declines the run.
    pub unsupported: bool,
}

impl DrawCards {
    #[must_use]
    pub const fn new(who: Seat, count: usize, drop_unofficial: bool) -> Self {
        Self { who, count, drop_unofficial, stage: DStage::Start, groups: Vec::new(), unsupported: false }
    }

    /// The options of the question: one per printed name, the first copy in hand order, in the order of the names
    /// (`first_of_each` is a `BTreeMap` keyed by the printed name), with the copies of that name held.
    fn options(&self, core: &CoreState) -> Option<SmallList<(u8, ActionCard, u8), 12>> {
        let hand = core.players().by_seat(self.who)?.cards.hand.as_slice();
        let mut firsts: SmallList<(u8, ActionCard, u8), 12> = SmallList::new();
        let mut seen: SmallList<u8, 12> = SmallList::new();
        for (index, card) in hand.iter().enumerate() {
            let group = *self.groups.get(*card as usize)?;
            if !seen.as_slice().contains(&group) {
                seen.push(group);
                let copies = hand.iter().filter(|c| self.groups.get(**c as usize) == Some(&group)).count();
                firsts.push((u8::try_from(index).ok()?, *card, u8::try_from(copies).unwrap_or(u8::MAX)));
            }
        }
        firsts.as_mut_slice().sort_by_key(|(_, card, _)| card.name_rank());
        Some(firsts)
    }

    /// How many cards over the limit the hand holds.
    fn excess(&self, core: &CoreState, k: &Kernel) -> Option<u8> {
        let held = core.players().by_seat(self.who)?.cards.hand.len();
        let limit = super::hand_limit(core, false, |l| k.hooks.economy.action_card_limit(core, k, self.who, l));
        u8::try_from(held.checked_sub(limit)?).ok().filter(|n| *n > 0)
    }

    /// Whether the drawer's Scheming follows a draw with a discard: its faction ability. (The permission a Deepgloom Executable gives another
    /// seat is a mark the caller's bail keeps out: nobody holds the breakthrough in a game without Thunder's Edge.)
    fn scheming(core: &CoreState, who: Seat) -> bool {
        core.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Scheming))
    }

    /// The hand size when it is over the limit (Sanctions caps it at three, the faction hooks override it last: Yssaril's Crafty has none; the
    /// blank turn of Keleres' Law's Order, which lifts the cap, is the caller's bail).
    fn over(&self, core: &CoreState, k: &Kernel) -> Option<u8> {
        let held = core.players().by_seat(self.who)?.cards.hand.len();
        let limit = super::hand_limit(core, false, |l| k.hooks.economy.action_card_limit(core, k, self.who, l));
        (held > limit).then(|| u8::try_from(held).unwrap_or(u8::MAX))
    }

    /// Scheming's options: the first copy in hand of each printed name, in hand order (not in the order of the names, as the limit's are).
    fn scheming_options(&self, core: &CoreState) -> Option<SmallList<ActionCard, 12>> {
        let hand = core.players().by_seat(self.who)?.cards.hand.as_slice();
        let mut out: SmallList<ActionCard, 12> = SmallList::new();
        let mut seen: SmallList<u8, 12> = SmallList::new();
        for card in hand {
            let group = *self.groups.get(*card as usize)?;
            if !seen.as_slice().contains(&group) {
                seen.push(group);
                out.push(*card);
            }
        }
        Some(out)
    }
}

impl Window for DrawCards {
    type Stage = DStage;

    fn stage(&self) -> DStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == DStage::Start {
            self.groups = sx.table.facts.card_group_table().map(<[u8]>::to_vec).unwrap_or_default();
            // A draw of nothing is only the limit's enforcement; a real draw takes the cards the hooks add.
            let requested = if self.count > 0 { self.count + sx.kernel.hooks.economy.action_card_draw_bonus(sx.state, sx.kernel, self.who, self.count) } else { 0 };
            let drawn = fx::draw_action_cards(sx.state, self.who, requested, self.drop_unofficial);
            self.stage = DStage::Done;
            if requested > 0 && Self::scheming(sx.state, self.who) {
                if drawn > 0 {
                    match self.scheming_options(sx.state) {
                        Some(options) if options.as_slice().len() > 1 => {
                            self.stage = DStage::Scheming;
                            return Next::Continue(Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }));
                        }
                        Some(options) => {
                            if let Some(only) = options.as_slice().first() {
                                fx::discard_action_card_staged(sx.state, self.who, *only);
                            }
                        }
                        None => {
                            self.unsupported = true;
                            return Next::Done;
                        }
                    }
                }
            }
        }
        if self.stage == DStage::Scheming {
            return Next::Continue(Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }));
        }
        match self.over(sx.state, sx.kernel) {
            Some(held) if self.options(sx.state).is_some() => {
                self.stage = DStage::Asking;
                Next::Continue(Ask::Cards(CAsk::HandLimit { held, discard: self.excess(sx.state, sx.kernel).unwrap_or(1) }))
            }
            Some(_) => {
                self.unsupported = true;
                self.stage = DStage::Done;
                Next::Done
            }
            None => {
                self.stage = DStage::Done;
                Next::Done
            }
        }
    }

    fn ask_of(&self, core: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == DStage::Scheming {
            return Some(Choice {
                actor: self.who,
                ask: Ask::Cards(CAsk::OwnHand { reason: Reason::Scheming }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Ability(Ability::Scheming)),
                    subtype: Subtype::Cards(Reason::Scheming.subtype()),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: self.scheming_options(core)?.as_slice().iter().map(|card| Opt::Cards(COpt::Card(*card))).collect(),
            });
        }
        if self.stage != DStage::Asking {
            return None;
        }
        let held = self.over(core, k)?;
        let discard = self.excess(core, k)?;
        let opts = self.options(core)?.as_slice().iter().map(|(index, card, copies)| Opt::Cards(COpt::Discard { index: *index, card: *card, copies: *copies })).collect();
        Some(Choice {
            actor: self.who,
            ask: Ask::Cards(CAsk::HandLimit { held, discard }),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R2_4),
                subtype: Subtype::Cards(CSub::DiscardOverHandLimit),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if let (DStage::Scheming, Pick::Opt(Opt::Cards(COpt::Card(card)))) = (self.stage, pick) {
            if !self.scheming_options(sx.state).ok_or(Illegal)?.as_slice().contains(&card) {
                return Err(Illegal);
            }
            fx::discard_action_card_staged(sx.state, self.who, card);
            self.stage = DStage::Done;
            return Ok(());
        }
        // ONE answer for every card over the limit (the multi-ask conversion): how many of each printed name go.
        let Pick::Opt(Opt::Cards(COpt::DiscardSet { counts })) = pick else { return Err(Illegal) };
        let counts = counts.bits();
        if self.stage != DStage::Asking {
            return Err(Illegal);
        }
        let options = self.options(sx.state).ok_or(Illegal)?;
        let opts: SmallList<COpt, 12> = options.as_slice().iter().map(|(index, card, copies)| COpt::Discard { index: *index, card: *card, copies: *copies }).collect();
        let discard = self.excess(sx.state, sx.kernel).ok_or(Illegal)?;
        if !crate::choice::cards::discard_set_ok(opts.as_slice(), discard, counts) {
            return Err(Illegal);
        }
        // The first copies of each printed name go (the string's answer named the first copy each time).
        let groups = &self.groups;
        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
            for (i, (_, card, _)) in options.as_slice().iter().enumerate() {
                let group = groups.get(*card as usize).copied();
                for _ in 0..crate::choice::cards::discard_count(counts, i) {
                    if let Some(at) = p.cards.hand.as_slice().iter().position(|c| groups.get(*c as usize).copied() == group) {
                        p.cards.hand.remove(at);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Deck;
    use crate::driver::{Driver, Status};
    use crate::generated::Faction;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    /// Each card its own name, except the copies of Flank Speed (one name).
    fn groups() -> Vec<u8> {
        let mut g: Vec<u8> = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
        g[ActionCard::FlankSpeed2 as usize] = u8::try_from(ActionCard::FlankSpeed1 as usize).unwrap();
        g
    }

    fn position(hand: &[ActionCard], deck: &[ActionCard]) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec)]);
        s.seats = crate::state::Seats::new(&[Seat(0)], &[Seat(0)], 1);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = hand.iter().copied().collect();
        s.decks_mut().action = Deck::new(deck.to_vec());
        s
    }

    fn table(with_groups: bool) -> AbilityTable {
        let mut t = AbilityTable::default();
        if with_groups {
            t.facts.card_groups = groups();
        }
        t
    }

    fn drive(s: &mut CoreState, count: usize, with_groups: bool) -> (Status, Driver<DrawCards>) {
        let mut d = Driver::new(DrawCards::new(Seat(0), count, false), Resolver::new(&[Seat(0)], Some(Seat(0))), table(with_groups), 1);
        let status = d.run(s, &Kernel::plain()).unwrap();
        (status, d)
    }

    const SEVEN: [ActionCard; 7] = [
        ActionCard::Rally,
        ActionCard::FlankSpeed1,
        ActionCard::NavSuite,
        ActionCard::FlankSpeed2,
        ActionCard::CoupDEtat,
        ActionCard::Sabotage1,
        ActionCard::Spy,
    ];

    /// Rule 2.4: a hand of seven plus the drawn card asks which card goes; one option per printed name (the first copy in hand order),
    /// in the order of the names; the card chosen leaves the hand and the hand is within its limit again.
    #[test]
    fn a_draw_over_the_limit_asks_one_option_per_printed_name_in_name_order() {
        let mut s = position(&SEVEN, &[ActionCard::FlankSpeed3]);
        let (status, mut d) = drive(&mut s, 1, true);
        let Status::Asking(choice) = status else { panic!("eight cards, the limit is seven: a question") };
        assert_eq!(choice.ask, Ask::Cards(CAsk::HandLimit { held: 8, discard: 1 }));
        assert_eq!((choice.actor, choice.ctx.origin, choice.ctx.subtype), (Seat(0), Origin::Rule(Rule::R2_4), Subtype::Cards(CSub::DiscardOverHandLimit)));
        assert!(!choice.ctx.optional && !choice.ctx.space_battle && choice.ctx.target.is_none() && choice.ctx.invasion_seq.is_none());
        let mut expected: Vec<(u8, ActionCard)> = vec![(0, SEVEN[0]), (1, SEVEN[1]), (2, SEVEN[2]), (4, SEVEN[4]), (5, SEVEN[5]), (6, SEVEN[6])];
        // The drawn Flank Speed 3 is a third copy of the name of copy 1 (the groups say so only for copy 2): its own group here.
        expected.push((7, ActionCard::FlankSpeed3));
        expected.sort_by_key(|(_, card)| card.name_rank());
        let offered: Vec<(u8, ActionCard)> = choice.opts.iter().map(|o| match o {
            Opt::Cards(COpt::Discard { index, card, .. }) => (*index, *card),
            other => panic!("{other:?}"),
        }).collect();
        assert_eq!(offered, expected, "the second Flank Speed (index 3) is the same name as the first: not offered");
        let pick = choice.opts.iter().position(|o| matches!(o, Opt::Cards(COpt::Discard { index: 2, .. }))).unwrap();
        let status = d.resume(&mut s, &Kernel::plain(), choice.pick(pick).unwrap()).unwrap();
        assert!(matches!(status, Status::Done));
        let hand = s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
        assert_eq!(hand.len(), 7);
        assert!(!hand.contains(&ActionCard::NavSuite), "the chosen card is gone");
    }

    /// Sanctions caps the hand at three: a hand of five discards two in ONE question (the multi-ask conversion).
    #[test]
    fn a_draw_over_the_limit_by_two_discards_both_in_one_answer() {
        let mut s = position(&SEVEN[..5], &[]);
        s.agenda_mut().laws.insert(crate::query::laws::SANCTIONS, crate::keys::Outcome::For);
        let (status, mut d) = drive(&mut s, 0, true);
        let Status::Asking(first) = status else { panic!("five cards under Sanctions: a question") };
        assert_eq!(first.ask, Ask::Cards(CAsk::HandLimit { held: 5, discard: 2 }));
        assert!(first.is_multi_pick());
        // One card is not enough, three are too many; a single option is no answer.
        let one = Pick::Opt(Opt::Cards(COpt::DiscardSet { counts: crate::choice::cards::Counts::of(0x1) }));
        let three = Pick::Opt(Opt::Cards(COpt::DiscardSet { counts: crate::choice::cards::Counts::of(0x111) }));
        for wrong in [one, three, Pick::Opt(first.opts.as_slice()[0])] {
            assert!(!first.accepts(&wrong));
            assert!(d.resume(&mut s, &Kernel::plain(), wrong).is_err());
        }
        let status = d.resume(&mut s, &Kernel::plain(), first.pick(0).unwrap()).unwrap();
        assert!(matches!(status, Status::Done), "three cards: within the cap");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 3);
    }

    /// The multi-ask conversion's proof: the one discard answer (`discard_set_ok`) reaches exactly the hands the string's one-card-per-question
    /// loop reached (each question offers the first copy of each printed name; the loop stops at the limit), brute-forced over small hands.
    #[test]
    fn the_one_discard_answer_reaches_exactly_what_the_card_by_card_loop_reached() {
        use std::collections::BTreeSet;
        // Groups: each card its own name except the Flank Speed copies (one name).
        let flank = [ActionCard::FlankSpeed1, ActionCard::FlankSpeed2];
        let group = |c: ActionCard| if flank.contains(&c) { ActionCard::FlankSpeed1 } else { c };
        fn old(hand: &[ActionCard], limit: usize, group: &dyn Fn(ActionCard) -> ActionCard, out: &mut BTreeSet<Vec<ActionCard>>) {
            if hand.len() <= limit {
                let mut h = hand.to_vec();
                h.sort();
                out.insert(h);
                return;
            }
            let mut seen = Vec::new();
            for (i, c) in hand.iter().enumerate() {
                if seen.contains(&group(*c)) {
                    continue;
                }
                seen.push(group(*c));
                let mut next = hand.to_vec();
                next.remove(i);
                old(&next, limit, group, out);
            }
        }
        for hand in [&SEVEN[..5], &[ActionCard::FlankSpeed1, ActionCard::FlankSpeed2, SEVEN[0], SEVEN[1], SEVEN[2]][..], &SEVEN[..]] {
            {
                // Sanctions caps the hand at three.
                let limit = 3usize;
                let mut want = BTreeSet::new();
                old(hand, limit, &group, &mut want);
                let mut s = position(hand, &[]);
                s.agenda_mut().laws.insert(crate::query::laws::SANCTIONS, crate::keys::Outcome::For);
                let (status, _) = drive(&mut s, 0, true);
                let Status::Asking(c) = status else { assert_eq!(want.len(), 1); continue };
                let mut got = BTreeSet::new();
                // Every count vector with each option at most twice (Flank Speed has two copies here), the accepted ones answered.
                let n = c.opts.len();
                let all: Vec<u64> = (0..3u64.pow(u32::try_from(n).unwrap())).map(|mut x| {
                    let mut bits = 0u64;
                    for i in 0..n {
                        bits = crate::choice::cards::with_discard_count(bits, i, u8::try_from(x % 3).unwrap());
                        x /= 3;
                    }
                    bits
                }).collect();
                for counts in all {
                    let pick = Pick::Opt(Opt::Cards(COpt::DiscardSet { counts: crate::choice::cards::Counts::of(counts) }));
                    if !c.accepts(&pick) {
                        continue;
                    }
                    let mut t = s.clone();
                    let (_, mut d2) = drive(&mut t, 0, true);
                    assert!(matches!(d2.resume(&mut t, &Kernel::plain(), pick).unwrap(), Status::Done));
                    let mut h = t.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice().to_vec();
                    h.sort();
                    got.insert(h);
                }
                assert_eq!(got, want, "{hand:?} limit {limit}");
            }
        }
    }

    /// Within the limit, with an empty deck, or drawing nothing: no question.
    #[test]
    fn a_draw_within_the_limit_or_from_an_empty_deck_asks_nothing() {
        let mut s = position(&SEVEN[..5], &[ActionCard::Rally, ActionCard::Spy]);
        let (status, d) = drive(&mut s, 2, true);
        assert!(matches!(status, Status::Done) && !d.window.unsupported);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 7);
        let mut s = position(&SEVEN, &[]);
        let (status, _) = drive(&mut s, 2, true);
        assert!(matches!(status, Status::Done), "an empty deck draws nothing and the hand is at the limit, not over it");
    }

    /// Bail: a table without printed-name groups cannot build the question: the window declines (`unsupported`), never guesses.
    #[test]
    fn bail_a_draw_without_name_groups_over_the_limit_is_unsupported() {
        let mut s = position(&SEVEN, &[ActionCard::Spy]);
        let (status, d) = drive(&mut s, 1, false);
        assert!(matches!(status, Status::Done));
        assert!(d.window.unsupported);
        // Within the limit the groups are not needed.
        let mut s = position(&SEVEN[..3], &[ActionCard::Spy]);
        let (_, d) = drive(&mut s, 1, false);
        assert!(!d.window.unsupported);
    }
}
