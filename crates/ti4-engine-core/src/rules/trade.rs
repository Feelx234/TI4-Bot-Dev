//! Transactions that change the typed state (`transactions.rs`: `take`, `give`, the per-turn and
//! per-round records). Package B4.
//!
//! Only what lives in the typed slices moves here: trade goods, commodities, relic fragments, an
//! action card, a secret objective. A promissory note and a planet card change hands in slices
//! other packages own (cards, board); the adapter keeps those on the string engine.

use crate::generated::{ExploreType, Faction, Planet, PromissoryNote};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::Kernel;
use crate::query::promissory as qn;
use crate::query::trade::{Env, NoteMove, Offer, Terms};
use crate::state::{CoreState, LedgerReason, NoteKey, TransientFlags, VpRec};

/// The hooks of the A5 table answer the two slots that exist there; the rest is neutral.
pub struct KernelEnv<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
}

impl Env for KernelEnv<'_> {
    fn has_iihq(&self, _: Seat) -> bool {
        false
    }
    fn wide(&self, who: Seat) -> bool {
        self.kernel.hooks.general.ignores_neighbours(self.core, self.kernel, who)
    }
    fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn trades_action_cards(&self, who: Seat) -> bool {
        self.kernel.hooks.general.trades_action_cards(self.core, self.kernel, who)
    }
    fn black_market(&self) -> bool {
        self.core.transient().misc.flags.has(TransientFlags::BLACK_MARKET)
    }
}



/// `transactions::take`: remove what a seat gives away (goods, commodities, fragments, the first
/// matching action card and secret).
pub fn take(core: &mut CoreState, seat: Seat, terms: &Terms) {
    if let Some((planet, to)) = terms.planet.and_then(|p| p.mv) {
        move_planet_units(core, seat, planet, to);
    }
    let Some(p) = core.players_mut().by_seat_mut(seat) else { return };
    if let Some(card) = terms.action_card
        && let Some(at) = p.cards.hand.as_slice().iter().position(|c| *c == card)
    {
        p.cards.hand.remove(at);
    }
    if let Some(secret) = terms.secret
        && let Some(at) = p.cards.secrets.as_slice().iter().position(|c| *c == secret)
    {
        p.cards.secrets.remove(at);
    }
    p.econ.trade_goods -= i16::try_from(terms.trade_goods).unwrap_or(0);
    p.econ.commodities -= i16::try_from(terms.commodities).unwrap_or(0);
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        let n = terms.fragments[kind as usize];
        if n > 0
            && let Some(held) = p.cards.fragments.get(kind)
        {
            p.cards.fragments.set(kind, held - i16::try_from(n).unwrap_or(0));
        }
    }
    // `relic_fragments.retain(|_, held| *held > 0)`: a spent kind is gone, not zero.
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        if p.cards.fragments.get(kind).is_some_and(|n| n <= 0) {
            p.cards.fragments.remove(kind);
        }
    }
}

/// `transactions::take` of a planet card (Pride of Kenara): the giver's units on the planet move to its other planet
/// `to`, and the planet is no longer controlled. Nothing happens unless the giver controls both.
fn move_planet_units(core: &mut CoreState, seat: Seat, planet: Planet, to: Planet) {
    let b = core.board();
    if b.planet(planet).control != Some(seat) || b.planet(to).control != Some(seat) {
        return;
    }
    let home = core.board().planet(planet).placed_in.or(planet.data().system);
    let dest = core.board().planet(to).placed_in.or(to.data().system);
    let all: Vec<_> = core.board().planet(planet).units.as_slice().to_vec();
    let (mine, rest): (Vec<_>, Vec<_>) = all.into_iter().partition(|u| u.owner() == seat);
    let cell = core.board_mut().planet_mut(planet);
    cell.units.clear();
    for u in rest {
        cell.units.push(u);
    }
    cell.control = None;
    let cell = core.board_mut().planet_mut(to);
    for u in mine {
        cell.units.push(u);
    }
    cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
    for system in [home, dest].into_iter().flatten() {
        core.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
    }
}

/// `transactions::give`: hand over what a seat receives. A commodity becomes a trade good on
/// arrival (LRR 21.5).
pub fn give(core: &mut CoreState, seat: Seat, terms: &Terms) {
    if let Some((planet, _)) = terms.planet.and_then(|p| p.mv) {
        // `set_control` refuses a planet that was destroyed: there is no card to take.
        let cell = core.board_mut().planet_mut(planet);
        if !cell.flags.has(crate::state::PlanetFlags::PURGED) {
            cell.control = Some(seat);
        }
    }
    {
        let Some(p) = core.players_mut().by_seat_mut(seat) else { return };
        if let Some(card) = terms.action_card {
            p.cards.hand.push(card);
        }
        if let Some(secret) = terms.secret {
            p.cards.secrets.push(secret);
        }
        for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
            let n = terms.fragments[kind as usize];
            if n > 0 {
                let held = p.cards.fragments.get(kind).unwrap_or(0);
                p.cards.fragments.set(kind, held + i16::try_from(n).unwrap_or(0));
            }
        }
    }
    fx::gain_trade_goods(core, seat, terms.trade_goods + terms.commodities);
    // The note comes last, as in the string `give`: Support scores its point, any other note changes hands.
    match terms.note.and_then(|n| n.mv) {
        Some(NoteMove::Held(key)) => take_note(core, seat, key),
        Some(NoteMove::Support(owner)) => {
            let _ = receive_support(core, seat, owner);
        }
        None => {}
    }
}

/// The goods half of `transactions::resolve`: both sides are taken before either is given.
///
/// `None` (nothing written) when a Support for the Throne leg names a faction two seats play.
pub fn exchange(core: &mut CoreState, offer: &Offer) -> Option<()> {
    for leg in [offer.given, offer.received] {
        if let Some(NoteMove::Support(f)) = leg.note.and_then(|n| n.mv) {
            owner_of(core, f)?;
        }
    }
    take(core, offer.proposer, &offer.given);
    take(core, offer.partner, &offer.received);
    give(core, offer.partner, &offer.given);
    give(core, offer.proposer, &offer.received);
    Some(())
}

/// `GameState::record_transaction`: LRR 94.1 limits each pair, so both sides remember it.
pub fn record_transaction(core: &mut CoreState, a: Seat, b: Seat) {
    let t = core.trade_mut();
    t.transactions_this_turn.get_or_insert_with(a, Default::default).insert(b);
    t.transactions_this_turn.get_or_insert_with(b, Default::default).insert(a);
}

/// `GameState::note_negotiation`: one negotiation `seat` initiated, this action and this round.
pub fn note_negotiation(core: &mut CoreState, seat: Seat) {
    let t = core.trade_mut();
    for tally in [&mut t.negotiations_this_action, &mut t.negotiations_this_round] {
        let n = tally.get_or_insert_with(seat, || 0);
        *n = n.saturating_add(1);
    }
}

/// `GameState.transactions_this_round.push((proposer, partner))` (duplicates kept).
pub fn record_resolved(core: &mut CoreState, proposer: Seat, partner: Seat) {
    core.trade_mut().this_round.edit().push((proposer, partner));
}

// -- promissory notes changing hands (`promissory.rs`: `take`, `give_back`, the two ACTION placements, the
// Support for the Throne loan, `return_all_foreign`) ---------------------------------------------------
//
// A note is a `(note, owner faction)` key held by a seat; its owner is the seat playing the faction. A faction
// played by two seats has no unique owner: those calls answer `None` and the string engine decides.

/// `objectives::VICTORY_TARGET`.
const VICTORY_TARGET: i32 = 10;

/// The one seat playing `faction`: `Some(None)` when nobody does, `None` when two seats do.
fn owner_of(core: &CoreState, faction: Faction) -> Option<Option<Seat>> {
    let mut seats = core.players().iter().filter(|p| p.faction == faction).map(|p| p.seat);
    let first = seats.next();
    if seats.next().is_some() { None } else { Some(first) }
}

/// `promissory::take`: the holder gets the note, faceup if that is where the card lives (69.3).
pub fn take_note(core: &mut CoreState, holder: Seat, key: NoteKey) {
    core.cards_mut().promissory.edit().insert(key, holder);
    if qn::is_play_area(key.note, key.faction) && !qn::is_action_placed(core, key.note) && key.note != PromissoryNote::Antivirus {
        core.cards_mut().promissory_faceup.edit().insert(key, ());
    }
}

/// `promissory::give_back`: the note goes home to its owner and leaves the play area. `None`: the
/// owner is not unique. A faction nobody plays has no seat to return to: nothing happens.
pub fn give_back(core: &mut CoreState, key: NoteKey) -> Option<()> {
    let Some(owner) = owner_of(core, key.faction)? else { return Some(()) };
    if core.cards().promissory.get(&key) == Some(&owner) {
        return Some(());
    }
    core.cards_mut().promissory.edit().insert(key, owner);
    core.cards_mut().promissory_faceup.edit().remove(&key);
    Some(())
}

/// `promissory::play_action_note`: place a note from the hand faceup (only one `seat` holds in hand and
/// whose text is such an ACTION).
pub fn play_action_note(core: &mut CoreState, seat: Seat, key: NoteKey) -> bool {
    if !qn::action_notes_in_hand(core, seat).contains(&key) {
        return false;
    }
    core.cards_mut().promissory_faceup.edit().insert(key, ());
    true
}

/// `promissory::play_convoys`: the first Trade Convoys in the hand (by the text of its faction id, the
/// order of the string map, given by `text_order`) goes faceup.
pub fn play_convoys<K: Ord>(core: &mut CoreState, seat: Seat, text_order: impl Fn(Faction) -> K) -> bool {
    let Some(key) = qn::action_notes_in_hand(core, seat)
        .into_iter()
        .filter(|k| k.note == PromissoryNote::TradeConvoys)
        .min_by_key(|k| text_order(k.faction))
    else {
        return false;
    };
    core.cards_mut().promissory_faceup.edit().insert(key, ());
    true
}

/// `promissory::receive`: another player's Support for the Throne goes into `holder`'s play area and scores
/// a point. `Some(false)` when it cannot move (one's own, already lent); `None`: the owner is not unique.
pub fn receive_support(core: &mut CoreState, holder: Seat, owner_faction: Faction) -> Option<bool> {
    let Some(owner) = owner_of(core, owner_faction)? else { return Some(false) };
    if owner == holder || core.trade().support_holders.contains_key(&owner) {
        return Some(false);
    }
    core.trade_mut().support_holders.insert(owner, holder);
    if let Some(p) = core.players_mut().by_seat_mut(holder) {
        let before = i32::from(p.econ.victory_points);
        let after = (before + 1).min(VICTORY_TARGET);
        p.econ.victory_points = i16::try_from(after).unwrap_or(0);
        core.ledger_mut().vp.push(VpRec { player: holder, delta: after - before, reason: LedgerReason::SupportForTheThrone });
    }
    Some(true)
}

/// `promissory::return_support`: the loan ends and the point goes back with the card (the ledger follows
/// only when the total moved).
pub fn return_support(core: &mut CoreState, owner: Seat) -> bool {
    let Some(holder) = core.trade_mut().support_holders.remove(&owner) else { return false };
    if let Some(p) = core.players_mut().by_seat_mut(holder) {
        let before = i32::from(p.econ.victory_points);
        let after = (before - 1).max(0);
        p.econ.victory_points = i16::try_from(after).unwrap_or(0);
        if after != before {
            core.ledger_mut().vp.push(VpRec { player: holder, delta: after - before, reason: LedgerReason::SupportForTheThrone });
        }
    }
    true
}

/// `promissory::return_all_foreign`: every copy of `note` held by a seat of another faction goes home.
/// `None`: an owner is not unique.
pub fn return_all_foreign(core: &mut CoreState, note: PromissoryNote) -> Option<()> {
    let lent: Vec<NoteKey> = core
        .cards()
        .promissory
        .iter()
        .filter(|(k, holder)| k.note == note && qn::faction_of(core, **holder) != Some(k.faction))
        .map(|(k, _)| *k)
        .collect();
    // Nothing is written unless every owner is unique, so a bail leaves the state as it was.
    for key in &lent {
        owner_of(core, key.faction)?;
    }
    for key in lent {
        give_back(core, key)?;
    }
    Some(())
}

/// `promissory::use_ceasefire`: the Ceasefire of `mover`'s faction that is lent out (held by a seat of another faction) goes
/// home. `Some(held)`; `None` (nothing written) when the owner's faction is played by two seats.
pub fn use_ceasefire(core: &mut CoreState, mover: Seat) -> Option<bool> {
    let faction = qn::faction_of(core, mover)?;
    owner_of(core, faction)?;
    if qn::holder_of(core, PromissoryNote::Ceasefire, mover).is_none() {
        return Some(false);
    }
    give_back(core, NoteKey { note: PromissoryNote::Ceasefire, faction })?;
    Some(true)
}

/// Whether `seat` has a unit in `system`: in its space area or on one of its planets.
fn has_unit_in(core: &CoreState, seat: Seat, system: crate::generated::System) -> bool {
    core.board().sys(system).units.as_slice().iter().any(|u| u.owner() == seat)
        || core
            .board()
            .planets()
            .any(|(planet, cell)| cell.placed_in.or(planet.data().system) == Some(system) && cell.units.as_slice().iter().any(|u| u.owner() == seat))
}

/// `promissory::spend_support_on_activation`: activating `system` sends home every Support for the Throne `activator` holds
/// whose owner has a unit there (the point goes back with it), and the faceup Alliances, Trade Convoys, Blood Pacts, Dark Pacts
/// and Antiviruses of such owners. Returns the owners whose Support was spent, in seat order (the string lists them in the order
/// of their ids). `None` (nothing written) when two seats play one faction.
pub fn spend_support_on_activation(core: &mut CoreState, activator: Seat, system: crate::generated::System) -> Option<crate::containers::SmallList<Seat, 8>> {
    for p in core.players().iter() {
        owner_of(core, p.faction)?;
    }
    let owners: crate::containers::SmallList<Seat, 8> = core
        .trade()
        .support_holders
        .iter()
        .filter(|(owner, holder)| **holder == activator && has_unit_in(core, **owner, system))
        .map(|(owner, _)| *owner)
        .collect();
    for owner in owners.as_slice() {
        return_support(core, *owner);
    }
    for note in [PromissoryNote::Alliance, PromissoryNote::TradeConvoys, PromissoryNote::BloodPact, PromissoryNote::DarkPact, PromissoryNote::Antivirus] {
        let home: Vec<NoteKey> = core
            .cards()
            .promissory
            .iter()
            .filter(|(k, holder)| k.note == note && **holder == activator && core.cards().promissory_faceup.contains_key(*k))
            .filter(|(k, _)| owner_of(core, k.faction).flatten().is_some_and(|owner| owner != activator && has_unit_in(core, owner, system)))
            .map(|(k, _)| *k)
            .collect();
        for key in home {
            give_back(core, key)?;
        }
    }
    Some(owners)
}

/// `promissory::trade_agreement_on_replenish` without the Cabal observer and the staged event: when
/// `owner` replenishes and another seat holds their Trade Agreement, the commodities become the holder's
/// trade goods and the note goes home. `Some(Some((holder, goods)))` when it fired, `Some(None)` when it
/// did not; `None` (nothing written) when the owner's faction is played by two seats.
pub fn trade_agreement(core: &mut CoreState, owner: Seat) -> Option<Option<(Seat, i32)>> {
    let faction = qn::faction_of(core, owner)?;
    owner_of(core, faction)?;
    let Some(holder) = qn::holder_of(core, PromissoryNote::TradeAgreement, owner) else { return Some(None) };
    let given = i32::from(core.players().by_seat(owner)?.econ.commodities);
    if given <= 0 {
        return Some(None);
    }
    core.players_mut().by_seat_mut(owner)?.econ.commodities = 0;
    let h = core.players_mut().by_seat_mut(holder)?;
    h.econ.trade_goods = h.econ.trade_goods.saturating_add(i16::try_from(given).unwrap_or(0));
    give_back(core, NoteKey { note: PromissoryNote::TradeAgreement, faction })?;
    Some(Some((holder, given)))
}

/// `GameState::clear_transactions`: a new turn restores everyone's one transaction per neighbour.
pub fn clear_turn_transactions(core: &mut CoreState) {
    core.trade_mut().transactions_this_turn.clear();
}

/// `negotiations_this_action.clear()`: a new action for the negotiation limit.
pub fn clear_action_negotiations(core: &mut CoreState) {
    core.trade_mut().negotiations_this_action.clear();
}

/// The round reset of `phase::advance_phase`: this round's transactions (Lie in Wait counts them) and both
/// negotiation tallies.
pub fn reset_round(core: &mut CoreState) {
    let t = core.trade_mut();
    t.this_round.edit().clear();
    t.negotiations_this_round.clear();
    t.negotiations_this_action.clear();
}
