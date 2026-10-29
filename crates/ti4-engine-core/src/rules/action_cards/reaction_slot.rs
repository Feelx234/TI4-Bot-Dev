//! The standing reaction-card slot (`reactions::slot`, `reactions::playable_now` of the string engine): one
//! registered ability per seat and printed window, optional, unlimited and repeatable, eligible while the seat
//! holds an action card that could be played into the window.
//!
//! Ported: the eligibility (hand, reaction modes, the printed window and its guard, 22.3 `reaction_can_matter`,
//! the faction and law bars) and the list of cards the slot would offer ([`playable`]), which the adapter reads
//! to render the option ("Play X" / "Choose an action card…"), and the play of the stamps of [`Simple`]: the choice of
//! the card (a question when the cards differ), the spend into the declared plays, and at the close of the window
//! the announcement (`ACTION_CARD_PLAYED` with its own windows, the effect, the discard pile, `ACTION_CARD_DISCARDED`).
//! Not ported (the adapter names the card and the window bails): a card whose effect asks or places, a card played at
//! once (the window of `ACTION_CARD_PLAYED`, or a rules version before 3), a card from the salvage yard, Data Skimmer.
//!
//! The windows: every non-`Action` printed timing maps to an event and a relation (`kernel::reaction::window_of`);
//! the distinct pairs, sorted, are the slots `SLOT_BASE + index`. The string sorts the same pairs by event text;
//! the order between windows is not observable (a bucket holds one slot per owner).

use crate::choice::generic::Rule;
use crate::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Timing};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Eff, RulesHost, Slot, Spec};
use crate::kernel::reaction::window_of;
use crate::kernel::resolver::{Resolver, TimingError};
use crate::kernel::Kernel;
use crate::state::{CoreState, EventKind, EventRec, Key, Payload, ReactionMode, Val};
use crate::tables::Scope;

use super::{cost_is_payable, play_simple, reaction_can_matter, Env, Placed, Simple, Window, RULES_ACTION_CARD_COSTS};

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
pub fn playable(s: &CoreState, k: &Kernel, barred: bool, who: Seat, event: &EventRec, relation: Relation) -> Result<SmallList<ActionCard, 12>, Unknown> {
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
        if !can_matter(s, scope, who, *card, event)? {
            continue;
        }
        out.push(*card);
    }
    Ok(out)
}

/// `action_cards::reaction_can_matter`: the printed cost first (rules version 23), then the per-card requirement
/// for a window that names a system.
fn can_matter(s: &CoreState, scope: Scope, who: Seat, card: ActionCard, event: &EventRec) -> Result<bool, Unknown> {
    if s.meta().rules_at_least(RULES_ACTION_CARD_COSTS) {
        if card == ActionCard::Bribery {
            return Err(Unknown::BriberyGoods);
        }
        if !cost_is_payable(s, who, card, 0).ok_or(Unknown::CardCost)? {
            return Ok(false);
        }
    }
    let Some(system) = event.payload.system(Key::System) else { return Ok(true) };
    let actor = event.payload.seat(Key::Player);
    reaction_can_matter(s, scope, who, card, Window { system, actor }).ok_or(Unknown::HarnessOrInfiltrate)
}

fn condition(c: &CondCx<'_>) -> bool {
    match playable(c.state, c.kernel, c.barred, c.def.owner, c.event, c.def.relation) {
        Ok(cards) => !cards.is_empty(),
        Err(why) => c.unknown(why),
    }
}

/// The stamps and flags the core plays from a reaction window (a [`Simple`] effect that reads nothing the adapter
/// would have to lend: no unit placed, no note order, no tokens held by Mahact). Not Reflective Shielding: it stages
/// hits that the sustain window of the string takes up afterwards (a state the core windows do not carry).
#[must_use]
pub const fn effect_ported(card: ActionCard) -> bool {
    matches!(
        Simple::of(card),
        Some(
            Simple::MoraleBoost
                | Simple::FlankSpeed
                | Simple::SolarFlare
                | Simple::LostStar
                | Simple::Rout
                | Simple::Waylay
                | Simple::FighterPrototype
                | Simple::Bunker
                | Simple::WarMachine
                | Simple::Blitz
                | Simple::NavSuite
                | Simple::ManeuveringJets
                | Simple::ShieldsHolding
                | Simple::Disable
                | Simple::Salvage
                | Simple::EmergencyRepairs
                | Simple::ReverseEngineer
        )
    )
}

/// `RULES_SABOTAGE_TIMING`: from this version a play in an ordinary window is declared and announced at its close.
const RULES_SABOTAGE_TIMING: u32 = 3;

/// The effect of the slot: choose the card, spend it, declare it (`reactions::slot`'s effect for an ordinary window).
fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let event = eff.event.clone();
    let relation = eff.def.relation;
    if event.kind == EventKind::ActionCardPlayed || !eff.state().meta().rules_at_least(RULES_SABOTAGE_TIMING) {
        eff.unknown(Unknown::ImmediateCardPlay);
        return Ok(());
    }
    let barred = eff.barred(me);
    let cards = {
        let (s, k) = eff.read();
        playable(s, k, barred, me, &event, relation)
    };
    let cards = match cards {
        Ok(cards) => cards,
        Err(why) => {
            eff.unknown(why);
            return Ok(());
        }
    };
    let Some(&first) = cards.as_slice().first() else { return Ok(()) };
    let chosen = if cards.as_slice().iter().all(|c| eff.card_key(*c) == eff.card_key(first)) {
        first
    } else {
        // One option per printed card: the walk is reversed, so a name keeps the first copy in hand and the options come
        // in the order of the last copy (`reaction_card_options`).
        let mut slots: Vec<(u32, ActionCard)> = Vec::new();
        for card in cards.as_slice().iter().rev() {
            let key = eff.card_key(*card);
            match slots.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = *card,
                None => slots.push((key, *card)),
            }
        }
        let mut opts: Opts = slots.iter().map(|(_, card)| Opt::Reaction(ROpt::Card(*card))).collect();
        opts.push(Opt::Decline);
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::PlayCard { event: event.kind, relation }),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R22_1),
                subtype: Subtype::Reaction(RSub::PlayCard { event: event.kind, relation }),
                optional: true,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        match eff.ask(&choice) {
            Pick::Opt(Opt::Reaction(ROpt::Card(card))) => card,
            _ => return Ok(()),
        }
    };
    let Some(at) = eff.state().players().by_seat(me).and_then(|p| p.cards.hand.iter().position(|held| *held == chosen)) else {
        // A card on the salvage yard (Garbozia) is played from there.
        eff.unknown(Unknown::UnportedCard);
        return Ok(());
    };
    if super::discard_from_hand(eff.state(), me, at).is_some() {
        eff.state().transient_mut().misc.declared_card_plays.push((me, chosen));
    }
    Ok(())
}

fn event(host: &mut RulesHost<'_>, kind: EventKind, who: Seat, card: ActionCard) -> EventRec {
    let _ = host;
    EventRec { id: 0, kind, payload: Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Card, Val::Action(card)), cancelled: false }
}

fn emit(host: &mut RulesHost<'_>, resolver: &mut Resolver, mut rec: EventRec) -> Result<EventRec, TimingError> {
    rec.id = *host.next_id;
    *host.next_id += 1;
    resolver.emit(host, rec, |_, _, _| {})
}

/// `reactions::announce_discard`: the card is on the pile (Data Skimmer is the string's), then the window that reads it.
fn announce_discard(host: &mut RulesHost<'_>, resolver: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    host.state.cards_mut().discarded_action.edit().push(card);
    host.state.transient_mut().combat.last_action_discarded = Some((who, card));
    let rec = event(host, EventKind::ActionCardDiscarded, who, card);
    emit(host, resolver, rec)?;
    Ok(())
}

/// `reactions::announce` of a declared card: the play is logged and announced; a cancelled card is only discarded;
/// otherwise its effect runs, then the discard.
fn announce(host: &mut RulesHost<'_>, resolver: &mut Resolver, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    host.state.transient_mut().misc.action_card_plays.push((who, card));
    let rec = event(host, EventKind::ActionCardPlayed, who, card);
    let announced = emit(host, resolver, rec)?;
    if announced.cancelled {
        return announce_discard(host, resolver, who, card);
    }
    match Simple::of(card).filter(|_| effect_ported(card)) {
        Some(effect) => {
            let env = Env {
                mahact_held: 0,
                scope: Scope(u32::from(host.kernel.sources.0)),
                blanked: crate::query::reach::blanked(host.state),
                action_card: host.table.codes.action_card,
                supply_base: host.table.codes.supply_base,
                note_order: |_, _| core::cmp::Ordering::Equal,
                infantry: Placed::NONE,
                cruiser: Placed::NONE,
            };
            play_simple(host.state, who, effect, env);
        }
        None => host.tape.unknown.set(Some(Unknown::UnportedCard)),
    }
    announce_discard(host, resolver, who, card)
}

/// The close of a window (rules version 3): the cards declared in it are announced, in declaration order.
fn announce_declared(host: &mut RulesHost<'_>, resolver: &mut Resolver) -> Result<(), TimingError> {
    let declared = std::mem::take(&mut host.state.transient_mut().misc.declared_card_plays);
    for (who, card) in declared.as_slice() {
        announce(host, resolver, *who, *card)?;
    }
    Ok(())
}

/// Register the slot of every window.
pub fn register(t: &mut AbilityTable) {
    t.closed = Some(announce_declared);
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
        assert_eq!(playable(&s, &k, false, Seat(0), &e, Relation::When).unwrap().as_slice(), &[ActionCard::ShieldsHolding1]);
        // Bribery's goods are the Keleres agent's commodities: a documented bail of the cost check.
        s.meta_mut().rules_version = RULES_ACTION_CARD_COSTS;
        assert_eq!(can_matter(&s, Scope(u32::MAX), Seat(1), ActionCard::Bribery, &e), Err(Unknown::BriberyGoods));
        s.meta_mut().rules_version = 0;
        // Another faction with the card in hand: Shields Holding listens to the hits before they are assigned.
        s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.push(ActionCard::ShieldsHolding1);
        let e1 = EventRec { id: 1, kind: EventKind::HitsToAssign, payload: Payload::new().with(Key::Player, crate::state::Val::Seat(Seat(1))), cancelled: false };
        assert_eq!(playable(&s, &k, false, Seat(1), &e1, Relation::When).unwrap().as_slice(), &[ActionCard::ShieldsHolding1]);
        assert!(playable(&s, &k, true, Seat(1), &e1, Relation::When).unwrap().is_empty(), "Political Secret bars the play");
        assert!(playable(&s, &k, false, Seat(1), &e1, Relation::After).unwrap().is_empty(), "the other relation is another window");
    }
}
