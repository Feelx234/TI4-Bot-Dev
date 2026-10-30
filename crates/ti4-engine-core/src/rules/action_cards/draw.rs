//! Drawing action cards and the hand limit (package W6; string `action_cards::draw`, `enforce_hand_limit`) as a [`Window`].
//!
//! The windows that draw cards (the Politics abilities of the strategy cards, the Politics and Keleres Riders, Unconventional
//! Measures) ran the draw only where the hand stayed within its limit: over it the string asks which card to discard (rule
//! 2.4) and the draw was a documented bail. [`DrawCards`] is that draw with its question; a window of another area runs it as
//! a child (`Spawn::DrawCards`).
//!
//! What stays the string's, by the caller: a draw a faction hook changes (Yssaril's Scheming and Crafty, the Deepgloom
//! Executable) and a deck reserve (a dropped feature). The Data Skimmer (Ral Nel, never seated) would take a discard.

use crate::choice::cards::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::ActionCard;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::CoreState;

/// The suspension points of [`DrawCards`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DStage {
    Start,
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
    /// (`first_of_each` is a `BTreeMap` keyed by the printed name).
    fn options(&self, core: &CoreState) -> Option<SmallList<(u8, ActionCard), 12>> {
        let hand = core.players().by_seat(self.who)?.cards.hand.as_slice();
        let mut firsts: SmallList<(u8, ActionCard), 12> = SmallList::new();
        let mut seen: SmallList<u8, 12> = SmallList::new();
        for (index, card) in hand.iter().enumerate() {
            let group = *self.groups.get(*card as usize)?;
            if !seen.as_slice().contains(&group) {
                seen.push(group);
                firsts.push((u8::try_from(index).ok()?, *card));
            }
        }
        firsts.as_mut_slice().sort_by_key(|(_, card)| card.name_rank());
        Some(firsts)
    }

    /// The hand size when it is over the limit (Sanctions caps it at three; the faction override is the caller's bail).
    fn over(&self, core: &CoreState) -> Option<u8> {
        let held = core.players().by_seat(self.who)?.cards.hand.len();
        let limit = super::hand_limit(core, false, |l| l);
        (held > limit).then(|| u8::try_from(held).unwrap_or(u8::MAX))
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
            fx::draw_action_cards(sx.state, self.who, self.count, self.drop_unofficial);
            self.stage = DStage::Done;
        }
        match self.over(sx.state) {
            Some(held) if self.options(sx.state).is_some() => {
                self.stage = DStage::Asking;
                Next::Continue(Ask::Cards(CAsk::HandLimit { held }))
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

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != DStage::Asking {
            return None;
        }
        let held = self.over(core)?;
        let opts = self.options(core)?.as_slice().iter().map(|(index, card)| Opt::Cards(COpt::Discard { index: *index, card: *card })).collect();
        Some(Choice {
            actor: self.who,
            ask: Ask::Cards(CAsk::HandLimit { held }),
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
        let Pick::Opt(Opt::Cards(COpt::Discard { index, card })) = pick else { return Err(Illegal) };
        if self.stage != DStage::Asking {
            return Err(Illegal);
        }
        if !self.options(sx.state).ok_or(Illegal)?.as_slice().contains(&(index, card)) {
            return Err(Illegal);
        }
        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
            p.cards.hand.remove(usize::from(index));
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
        assert_eq!(choice.ask, Ask::Cards(CAsk::HandLimit { held: 8 }));
        let mut expected: Vec<(u8, ActionCard)> = vec![(0, SEVEN[0]), (1, SEVEN[1]), (2, SEVEN[2]), (4, SEVEN[4]), (5, SEVEN[5]), (6, SEVEN[6])];
        // The drawn Flank Speed 3 is a third copy of the name of copy 1 (the groups say so only for copy 2): its own group here.
        expected.push((7, ActionCard::FlankSpeed3));
        expected.sort_by_key(|(_, card)| card.name_rank());
        let offered: Vec<(u8, ActionCard)> = choice.opts.iter().map(|o| match o {
            Opt::Cards(COpt::Discard { index, card }) => (*index, *card),
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

    /// Sanctions caps the hand at three: a hand of five is asked twice, one card at a time, until it is three.
    #[test]
    fn a_draw_asks_again_until_the_hand_is_within_the_limit() {
        let mut s = position(&SEVEN[..5], &[]);
        s.agenda_mut().laws.insert(crate::query::laws::SANCTIONS, crate::keys::Outcome::For);
        let (status, mut d) = drive(&mut s, 0, true);
        let Status::Asking(first) = status else { panic!("five cards under Sanctions: a question") };
        let status = d.resume(&mut s, &Kernel::plain(), first.pick(0).unwrap()).unwrap();
        let Status::Asking(second) = status else { panic!("four cards: asked again") };
        assert_eq!(second.ask, Ask::Cards(CAsk::HandLimit { held: 4 }));
        let status = d.resume(&mut s, &Kernel::plain(), second.pick(0).unwrap()).unwrap();
        assert!(matches!(status, Status::Done), "three cards: within the cap");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.len(), 3);
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
