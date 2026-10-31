//! yssaril: Spy Net, the promissory note of the Yssaril (`action_cards::spy_net`, `look_at_hand_and_take`): "At the start of your turn: look at the Yssaril player's hand of action cards.
//! Choose 1 of those cards and add it to your hand. Then, return this card to the Yssaril player." Optional (the holder's own use / decline is the "may"); registered for every seat
//! (`promissory:*:spynet:TURN_BEGAN:after`), the condition names the holder of the note on the turn of the holder and a Yssaril hand that is not empty.
//!
//! The hand is shown to the taker only for this one question (the string marks it revealed for the question and clears the mark on every path out): here the question lists the cards
//! (one per printed name, the string's order) and nothing is marked. One card is taken without a question. The taken card is staged as taken; the hand limit of the holder is the
//! action-card area's window stepped inside the effect (`g3::draw::draw` with no card to draw).

use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::rules::promissory_windows::distinct_cards;
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey};

use super::super::g3::draw::draw_with;

/// Spy Net (`promissory:*:spynet:TURN_BEGAN:after`).
pub const SPY_NET: Slot = Slot(44_002);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::SpyNet, faction: Faction::TheYssarilTribes };

/// `spy_net_owner`: the Yssaril seat whose hand `holder` may look at.
fn yssaril_of(s: &CoreState, holder: Seat) -> Option<Seat> {
    if s.cards().promissory.get(&NOTE) != Some(&holder) {
        return None;
    }
    let owner = s.players().iter().find(|p| p.faction == Faction::TheYssarilTribes)?;
    (owner.seat != holder && !owner.cards.hand.as_slice().is_empty()).then_some(owner.seat)
}

fn condition(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && yssaril_of(c.state, c.def.owner).is_some()
}

fn groups(facts: &AdapterFacts) -> Vec<u8> {
    facts.card_group_table().map(<[u8]>::to_vec).unwrap_or_default()
}

fn spy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let Some(owner) = yssaril_of(eff.read().0, me) else { return Ok(()) };
    let groups = groups(eff.adapter_facts());
    let shown = distinct_cards(eff.read().0, owner, &groups);
    if shown.is_empty() {
        // The adapter gave no printed-name groups: it could not have asked.
        eff.unknown(Unknown::AbilityGap(SPY_NET));
        return Ok(());
    }
    let card = match shown.as_slice() {
        [only] => *only,
        _ => {
            let choice = Choice {
                actor: me,
                ask: Ask::Cards(CAsk::TakeRevealed { owner }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Promissory(PromissoryNote::SpyNet)),
                    subtype: Subtype::Cards(CSub::TakeRevealedActionCard),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: shown.iter().map(|card| Opt::Cards(COpt::Card(*card))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Cards(COpt::Card(card))) = eff.ask(&choice) else { return Ok(()) };
            if !shown.contains(&card) {
                return Ok(());
            }
            card
        }
    };
    if fx::take_action_card_staged(eff.state(), me, owner, card) {
        draw_with(eff, me, 0, false, SPY_NET);
    }
    fx::give_back_note(eff.state(), NOTE);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SPY_NET, AbilityKind::Promissory, EventKind::TurnBegan, Relation::After, spy).optional().when(condition));
}

