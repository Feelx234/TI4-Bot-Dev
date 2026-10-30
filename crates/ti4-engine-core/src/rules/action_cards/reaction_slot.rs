//! The standing reaction-card slot (`reactions::slot`, `reactions::playable_now` of the string engine): one
//! registered ability per seat and printed window, optional, unlimited and repeatable, eligible while the seat
//! holds an action card that could be played into the window.
//!
//! Ported: the eligibility (hand, reaction modes, the printed window and its guard, 22.3 `reaction_can_matter`,
//! the faction and law bars) and the list of cards the slot would offer ([`playable`]), which the adapter reads
//! to render the option ("Play X" / "Choose an action card…"). The play ([`play`]): the second question when
//! cards of several printed names could be played, then, from rules version 3 and outside the Sabotage window,
//! the card is only declared (spent, announced when the window closes: `announce::announce_declared`); otherwise
//! it is spent and announced at once, and a played Sabotage cancels the play that opened the window.
//!
//! The windows: every non-`Action` printed timing maps to an event and a relation (`kernel::reaction::window_of`);
//! the distinct pairs, sorted, are the slots `SLOT_BASE + index`. The string sorts the same pairs by event text;
//! the order between windows is not observable (a bucket holds one slot per owner).

use crate::choice::cards::{Ask, Opt, Subtype};
use crate::choice::generic::Rule;
use crate::choice::reaction::{AbilityKind, Relation};
use crate::choice::{Choice, Ctx, Opts, Origin, Pick};
use crate::containers::SmallList;
use crate::kernel::ability::{Eff, Runner};
use crate::kernel::resolver::TimingError;
use crate::generated::{ActionCard, Timing};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Slot, Spec};
use crate::kernel::reaction::window_of;
use crate::kernel::Kernel;
use crate::state::{CoreState, EventKind, EventRec, Key, ReactionMode};
use crate::tables::Scope;

use super::{cost_is_payable, reaction_can_matter, Window, RULES_ACTION_CARD_COSTS};

/// The first slot number of the reaction-card slots.
pub const SLOT_BASE: u16 = 100;

pub use crate::kernel::ability::Unknown;

/// The distinct `(event, relation)` windows of the printed timings, sorted.
#[must_use]
pub fn windows() -> Vec<(EventKind, Relation)> {
    let mut all: Vec<(EventKind, Relation)> = Timing::ALL.iter().filter_map(|t| window_of(*t)).map(|w| (w.event, w.relation)).collect();
    all.sort();
    all.dedup();
    all
}

/// The window of a slot number.
#[must_use]
pub fn window_of_slot(slot: Slot) -> Option<(EventKind, Relation)> {
    windows().get(usize::from(slot.0.checked_sub(SLOT_BASE)?)).copied()
}

/// The cards in the hand (and on the salvage yard, while Garbozia is controlled) that could be played into the
/// window of `event` now, in hand order (`playable_now`).
///
/// # Errors
/// [`Unknown`] when a rule the answer reads is a documented bail of the core.
pub fn playable(s: &CoreState, k: &Kernel, facts: &AdapterFacts, barred: bool, who: Seat, event: &EventRec, relation: Relation) -> Result<SmallList<ActionCard, 12>, Unknown> {
    let mut out = SmallList::new();
    let Some(p) = s.players().by_seat(who) else { return Ok(out) };
    if barred || k.hooks.economy.action_cards_forbidden(s, who) {
        return Ok(out);
    }
    let scope = Scope(u32::MAX);
    let salvaged = crate::query::explore::salvaged(s, who);
    for card in p.cards.hand.as_slice().iter().chain(salvaged) {
        if p.cards.reaction_modes.get(card) == Some(&ReactionMode::Never) {
            continue;
        }
        let Some(spec) = window_of(card.data().timing) else { continue };
        if spec.event != event.kind || spec.relation != relation || !spec.guard.holds(event, who, s, k) {
            continue;
        }
        if !can_matter(s, scope, who, *card, event, facts)? {
            continue;
        }
        out.push(*card);
    }
    Ok(out)
}

/// `action_cards::reaction_can_matter`: the printed cost first (rules version 23), then the per-card requirement
/// for a window that names a system.
fn can_matter(s: &CoreState, scope: Scope, who: Seat, card: ActionCard, event: &EventRec, facts: &AdapterFacts) -> Result<bool, Unknown> {
    // Reveal Prototype is played by the core only for a seat the adapter vouches for (its research and its unit upgrades).
    if card == ActionCard::RevealPrototype && !facts.plays.as_ref().and_then(|f| f.seats.iter().find(|p| p.who == who)).is_some_and(|p| p.reveal.is_some()) {
        return Err(Unknown::CardCost);
    }
    if s.meta().rules_at_least(RULES_ACTION_CARD_COSTS) {
        if card == ActionCard::Bribery {
            return Err(Unknown::BriberyGoods);
        }
        let payable = match cost_is_payable(s, who, card, 0) {
            Some(payable) => payable,
            // Reveal Prototype's four resources and its unit upgrade: the adapter vouches for the seat (`SeatPlay::reveal`).
            None if card == ActionCard::RevealPrototype => {
                let seat = facts.plays.as_ref().and_then(|f| f.seats.iter().find(|p| p.who == who)).filter(|p| p.reveal.is_some());
                seat.ok_or(Unknown::CardCost)?.reveal_payable
            }
            None => return Err(Unknown::CardCost),
        };
        if !payable {
            return Ok(false);
        }
    }
    let Some(system) = event.payload.system(Key::System) else { return Ok(true) };
    let actor = event.payload.seat(Key::Player);
    reaction_can_matter(s, scope, who, card, Window { system, actor }).ok_or(Unknown::HarnessOrInfiltrate)
}

fn condition(c: &CondCx<'_>) -> bool {
    match playable(c.state, c.kernel, c.facts, c.barred, c.def.owner, c.event, c.def.relation) {
        Ok(cards) => !cards.is_empty(),
        Err(why) => c.unknown(why),
    }
}

/// `RULES_SABOTAGE_TIMING`: from this version a play in an ordinary window is declared, not announced.
pub const RULES_SABOTAGE_TIMING: u32 = 3;

/// `reactions::spend`: the card leaves the hand (or the salvage yard) for the announcement; `false` when the seat
/// holds no such card.
fn spend(s: &mut CoreState, who: Seat, card: ActionCard) -> bool {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return false };
    if let Some(at) = p.cards.hand.as_slice().iter().position(|c| *c == card) {
        p.cards.hand.remove(at);
        return true;
    }
    if !crate::query::explore::salvaged(s, who).contains(&card) {
        return false;
    }
    match s.cards().salvage_yard.iter().position(|c| *c == card) {
        Some(at) => {
            s.cards_mut().salvage_yard.edit().remove(at);
            true
        }
        None => false,
    }
}

const fn is_sabotage(card: ActionCard) -> bool {
    matches!(card, ActionCard::Sabotage1 | ActionCard::Sabotage2 | ActionCard::Sabotage3 | ActionCard::Sabotage4)
}

/// The question that picks the card to play when the cards could not be told apart by name.
fn card_question(who: Seat, relation: Relation, event: EventKind, cards: &[ActionCard]) -> Choice {
    let mut opts: Opts = cards.iter().map(|c| crate::choice::Opt::Cards(Opt::Card(*c))).collect();
    opts.push(crate::choice::Opt::Decline);
    Choice {
        actor: who,
        ask: crate::choice::Ask::Cards(Ask::PlayCard { relation, event }),
        ctx: Ctx {
            origin: Origin::Rule(Rule::R22_1),
            subtype: crate::choice::Subtype::Cards(Subtype::PlayReaction { relation, event }),
            optional: true,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    }
}

/// The effect of the slot (`reactions::slot`'s closure): play a card into the window.
///
/// # Errors
/// What the resolver refuses with.
pub fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (who, relation) = (eff.owner(), eff.def.relation);
    let event = eff.event.clone();
    let barred = eff.barred();
    let cards = {
        let (s, k) = eff.read();
        playable(s, k, eff.adapter_facts(), barred, who, &event, relation)
    };
    let cards = match cards {
        Ok(cards) => cards,
        Err(why) => {
            eff.inline().unknown(why);
            return Ok(());
        }
    };
    let Some(&first) = cards.as_slice().first() else { return Ok(()) };
    let Some(groups) = eff.adapter_facts().plays.as_ref().map(|p| p.name_group.clone()) else {
        eff.inline().unknown(Unknown::PlayFacts);
        return Ok(());
    };
    let group = |c: ActionCard| groups.get(c as usize).copied().unwrap_or(u8::MAX);
    let chosen = if cards.as_slice().iter().all(|c| group(*c) == group(first)) {
        first
    } else {
        // One option per printed name, in the string's order: the walk from the back fixes each name's slot, and the
        // card kept is the first of that name in hand order.
        let mut slots: SmallList<ActionCard, 12> = SmallList::new();
        for card in cards.as_slice().iter().rev() {
            match slots.as_mut_slice().iter_mut().find(|slot| group(**slot) == group(*card)) {
                Some(slot) => *slot = *card,
                None => slots.push(*card),
            }
        }
        match eff.ask(&card_question(who, relation, event.kind, slots.as_slice())) {
            Pick::Opt(crate::choice::Opt::Cards(Opt::Card(card))) => card,
            _ => return Ok(()),
        }
    };
    if eff.read().0.meta().rules_at_least(RULES_SABOTAGE_TIMING) && event.kind != EventKind::ActionCardPlayed {
        if spend(eff.state(), who, chosen) {
            eff.state().transient_mut().misc.declared_card_plays.push((who, chosen));
        }
        return Ok(());
    }
    if !spend(eff.state(), who, chosen) {
        return Ok(());
    }
    let played = super::announce::play_inline(&mut eff.inline(), who, chosen)?;
    if played && event.kind == EventKind::ActionCardPlayed && is_sabotage(chosen) {
        eff.cancel();
    }
    Ok(())
}

/// Register the slot of every window.
pub fn register(t: &mut AbilityTable) {
    super::announce::register(t);
    t.slot_cards = Some(|s, k, facts, barred, who, event, relation| {
        playable(s, k, facts, barred, who, event, relation).map(|c| c.as_slice().to_vec()).unwrap_or_default()
    });
    for (i, (event, relation)) in windows().into_iter().enumerate() {
        let slot = Slot(SLOT_BASE + u16::try_from(i).unwrap_or(0));
        t.add(
            Spec::new(slot, AbilityKind::Reaction, event, relation, play)
                .when(condition)
                .optional()
                .frequency(crate::choice::reaction::Frequency::Unlimited)
                .repeatable(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;

    #[test]
    fn every_printed_window_has_one_slot_and_the_slot_numbers_are_dense() {
        let w = windows();
        assert!(w.len() > 20 && w.len() < 100, "{}", w.len());
        let mut t = AbilityTable::default();
        register(&mut t);
        assert_eq!(t.len(), w.len());
        for (i, _) in w.iter().enumerate() {
            assert_eq!(window_of_slot(Slot(SLOT_BASE + u16::try_from(i).unwrap())), Some(w[i]));
        }
        assert!(w.contains(&(EventKind::HitsToAssign, Relation::When)));
    }

    #[test]
    fn a_condition_the_core_cannot_answer_is_unknown_not_false() {
        use crate::state::{Payload, Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheYssarilTribes), Player::new(Seat(1), Faction::TheVuilraithCabal)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(ActionCard::ShieldsHolding1);
        let k = Kernel::plain();
        let e = EventRec { id: 1, kind: EventKind::HitsToAssign, payload: Payload::new().with(Key::Player, crate::state::Val::Seat(Seat(0))), cancelled: false };
        // Transparasteel Plating is the kernel's hook (`economy.action_cards_forbidden`), not a bail of the seat.
        assert_eq!(playable(&s, &k, &AdapterFacts::default(), false, Seat(0), &e, Relation::When).unwrap().as_slice(), &[ActionCard::ShieldsHolding1]);
        // Bribery's goods are the Keleres agent's commodities: a documented bail of the cost check.
        s.meta_mut().rules_version = RULES_ACTION_CARD_COSTS;
        assert_eq!(can_matter(&s, Scope(u32::MAX), Seat(1), ActionCard::Bribery, &e, &AdapterFacts::default()), Err(Unknown::BriberyGoods));
        s.meta_mut().rules_version = 0;
        // Another faction with the card in hand: Shields Holding listens to the hits before they are assigned.
        s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.push(ActionCard::ShieldsHolding1);
        let e1 = EventRec { id: 1, kind: EventKind::HitsToAssign, payload: Payload::new().with(Key::Player, crate::state::Val::Seat(Seat(1))), cancelled: false };
        assert_eq!(playable(&s, &k, &AdapterFacts::default(), false, Seat(1), &e1, Relation::When).unwrap().as_slice(), &[ActionCard::ShieldsHolding1]);
        assert!(playable(&s, &k, &AdapterFacts::default(), true, Seat(1), &e1, Relation::When).unwrap().is_empty(), "Political Secret bars the play");
        assert!(playable(&s, &k, &AdapterFacts::default(), false, Seat(1), &e1, Relation::After).unwrap().is_empty(), "the other relation is another window");
    }

    /// A window of the table holds cards of several printed names: the slot asks which to play, one option per name
    /// (the card kept for a name is its first copy in hand order), then a decline; a single name asks nothing.
    #[test]
    fn a_hand_of_two_printed_names_asks_which_card_and_keeps_the_first_copy_of_a_name() {
        use crate::driver::{Driver, Status};
        use crate::kernel::stage::{Illegal, Sx};
        use crate::kernel::{fx, Next, Resolver};
        use crate::state::{Payload, Player, Players, Val};

        #[derive(Clone)]
        struct Activate;
        impl crate::kernel::stage::Window for Activate {
            type Stage = ();
            fn stage(&self) {}
            fn step(&mut self, sx: &mut Sx<'_>) -> Next {
                let rec = fx::event(
                    EventKind::SystemActivated,
                    Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::System, Val::System(crate::generated::System::ALL[0])),
                );
                let _ = sx.emit(rec);
                Next::Done
            }
            fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<crate::choice::Choice> {
                None
            }
            fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
                Err(Illegal)
            }
        }
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheYssarilTribes), Player::new(Seat(1), Faction::TheVuilraithCabal)]);
        // Flank Speed x2, Nav Suite: two printed names in the window after a system is activated by the holder.
        let hand = [ActionCard::FlankSpeed2, ActionCard::NavSuite, ActionCard::FlankSpeed1];
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = hand.into_iter().collect();
        let mut table = AbilityTable::default();
        register(&mut table);
        let mut groups = vec![0u8; ActionCard::COUNT];
        for (i, g) in groups.iter_mut().enumerate() {
            *g = u8::try_from(i).unwrap();
        }
        let (a, b) = (ActionCard::FlankSpeed1 as usize, ActionCard::FlankSpeed2 as usize);
        groups[b] = u8::try_from(a).unwrap();
        table.facts.plays = Some(crate::kernel::ability::PlayFacts {
            scope: u32::MAX,
            blanked: false,
            action_card: crate::state::Code(0),
            supply_base: crate::state::Code(0),
            note_order: |x, y| x.cmp(&y),
            seats: Vec::new(),
            name_group: groups,
            has_effect: vec![true; ActionCard::COUNT],
            skimmer: false,
            roster: None,
            rules_version: u32::MAX,
            tech_order: |x, y| x.cmp(&y),
        });
        let k = Kernel::plain();
        let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
        let order = [Seat(0), Seat(1)];
        resolver.register(table.registry(&s, &order, &mut |spec, who, _| crate::choice::reaction::AbilityId { kind: spec.kind, key: crate::state::Code(spec.slot.0 * 4 + u16::from(who.0)) }));
        let mut d = Driver::new(Activate, resolver, table, 1);
        let mut asked = Vec::new();
        let mut status = d.run(&mut s, &k).unwrap();
        while let Status::Asking(choice) = status {
            asked.push(choice.clone());
            // Take the slot in the window's pick, then the first card of the card question.
            let at = choice.opts.iter().position(|o| matches!(o, crate::choice::Opt::Reaction(crate::choice::reaction::Opt::Ability(_)))).unwrap_or(0);
            let pick = choice.pick(at).unwrap();
            status = d.resume(&mut s, &k, pick).unwrap();
        }
        let card_q = asked.iter().find(|c| matches!(c.ask, crate::choice::Ask::Cards(Ask::PlayCard { .. }))).expect("the card question is asked");
        let offered: Vec<_> = card_q.opts.iter().copied().collect();
        // One option per name, the first copy of Flank Speed in hand order (the walk from the back replaces), then decline.
        assert_eq!(
            offered,
            vec![
                crate::choice::Opt::Cards(Opt::Card(ActionCard::FlankSpeed2)),
                crate::choice::Opt::Cards(Opt::Card(ActionCard::NavSuite)),
                crate::choice::Opt::Decline
            ]
        );
        assert_eq!(card_q.actor, Seat(0));
        assert!(card_q.ctx.optional);
    }

    /// `reactions::spend`: the card leaves the hand, or the salvage yard while Garbozia is controlled, and a card the
    /// seat does not hold is not spent.
    #[test]
    fn spend_takes_the_card_from_the_hand_or_the_salvage_yard() {
        use crate::state::{Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheYssarilTribes)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = [ActionCard::Rally, ActionCard::CoupDEtat].into_iter().collect();
        s.cards_mut().salvage_yard.edit().push(ActionCard::MoraleBoost1);
        assert!(spend(&mut s, Seat(0), ActionCard::CoupDEtat));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Rally]);
        assert!(!spend(&mut s, Seat(0), ActionCard::MoraleBoost1), "the yard is the controller of Garbozia's");
        s.board_mut().planet_mut(crate::generated::Planet::Garbozia).control = Some(Seat(0));
        assert!(spend(&mut s, Seat(0), ActionCard::MoraleBoost1));
        assert!(s.cards().salvage_yard.is_empty());
        assert!(!spend(&mut s, Seat(0), ActionCard::MoraleBoost1), "nothing left to spend");
    }
}
