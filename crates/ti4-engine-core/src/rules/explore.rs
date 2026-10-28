//! Exploration, fragments and relics: the mutation half (package B5; string `exploration.rs`, `relics.rs`).
//!
//! The reads are `query::{explore, relic}`. Here the writes of the cards area, each the body of one string
//! function over the slices it names: the draw of an exploration card, a fragment gained, three fragments
//! purged for a relic, a relic gained (the Shard scores at once), exhausted or purged, an attachment put on
//! a planet, the frontier tokens, the wormhole token an exploration card places.
//!
//! Copied string behaviour, on purpose (dual parity): `purge_for_relic` logs the relic with 0 points even
//! when it is the Shard (the Shard scores only through `relic_gain`); asked for the FRONTIER trait it takes
//! the matching fragments and then the shortfall from the same pile, which can leave a negative count.
//! Both are listed in `docs/ENGINE_KNOWN_ISSUES.md`.

use crate::generated::{Attachment, ExploreType, Planet, Relic, System, Wormhole};
use crate::generated::Explore;
use crate::ids::Seat;
use crate::keys::LedgerReason;
use crate::query::explore::FRAGMENTS_PER_RELIC;
use crate::state::{CoreState, PlanetFlags, RelicRec, VpRec};

/// `objectives::VICTORY_TARGET`.
pub const VICTORY_TARGET: i32 = 10;

/// `relics::SHARD`.
pub const SHARD: Relic = Relic::ShardOfTheThrone;

/// `exploration::draw` on a position without a deck reserve: the top card of one deck. `None` when the
/// string map has no such key or the deck is empty.
pub fn draw(core: &mut CoreState, deck: ExploreType) -> Option<Explore> {
    let d = core.decks_mut();
    if d.explore_present & (1 << deck as u8) == 0 {
        return None;
    }
    d.explore[deck as usize].draw()
}

/// `exploration::gain_fragment`: one fragment of `kind`. Nothing happens for a seat that is not there.
pub fn gain_fragment(core: &mut CoreState, who: Seat, kind: ExploreType) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        let n = p.cards.fragments.get(kind).unwrap_or(0);
        p.cards.fragments.set(kind, n + 1);
    }
}

fn log_relic(core: &mut CoreState, who: Seat, relic: Relic, points: i32) {
    let round = core.meta().round;
    core.ledger_mut().relics.push(RelicRec { round, player: who, relic, victory_points: points });
}

/// `exploration::purge_for_relic`: three fragments (frontier ones make up a shortfall, spent after the
/// matching ones) for the top relic. `None`: not enough fragments, or the relic deck is empty (the
/// fragments stay).
pub fn purge_for_relic(core: &mut CoreState, who: Seat, kind: ExploreType) -> Option<Relic> {
    let p = core.players().by_seat(who)?;
    let matching = i32::from(p.cards.fragments.get(kind).unwrap_or(0));
    let frontier = i32::from(p.cards.fragments.get(ExploreType::Frontier).unwrap_or(0));
    if matching + frontier < FRAGMENTS_PER_RELIC {
        return None;
    }
    let from_matching = matching.min(FRAGMENTS_PER_RELIC);
    let from_frontier = FRAGMENTS_PER_RELIC - from_matching;
    let relic = core.decks_mut().relic.draw()?;
    let p = core.players_mut().by_seat_mut(who)?;
    let have = p.cards.fragments.get(kind).unwrap_or(0);
    p.cards.fragments.set(kind, have - i16::try_from(from_matching).ok()?);
    if from_frontier > 0 {
        let have = p.cards.fragments.get(ExploreType::Frontier).unwrap_or(0);
        p.cards.fragments.set(ExploreType::Frontier, have - i16::try_from(from_frontier).ok()?);
    }
    p.cards.relics.push(relic);
    log_relic(core, who, relic, 0);
    Some(relic)
}

/// `relics::gain` (73.2): the top relic to the seat; the Shard scores at once (capped at the target).
/// `None` for an empty deck (73.2a) or a seat that is not there.
pub fn relic_gain(core: &mut CoreState, who: Seat) -> Option<Relic> {
    core.players().by_seat(who)?;
    let top = core.decks_mut().relic.draw()?;
    core.players_mut().by_seat_mut(who)?.cards.relics.push(top);
    let mut points = 0;
    if top == SHARD {
        let p = core.players_mut().by_seat_mut(who)?;
        let before = i32::from(p.econ.victory_points);
        let after = (before + 1).min(VICTORY_TARGET);
        p.econ.victory_points = i16::try_from(after).unwrap_or(0);
        core.ledger_mut().vp.push(VpRec { player: who, delta: after - before, reason: LedgerReason::Relic(SHARD) });
        points = 1;
    }
    log_relic(core, who, top, points);
    Some(top)
}

/// `relics::exhaust`: a held, ready relic is exhausted.
pub fn exhaust_relic(core: &mut CoreState, who: Seat, relic: Relic) -> bool {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return false };
    if !p.cards.relics.as_slice().contains(&relic) {
        return false;
    }
    p.cards.relics_exhausted.insert(relic)
}

/// `relics::purge`: the relic leaves the seat's hand (an exhausted mark stays, as in the string state).
pub fn purge_relic(core: &mut CoreState, who: Seat, relic: Relic) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.relics.retain(|held| *held != relic);
    }
}

/// `planet_attachments.entry(planet).or_default().push(attachment)`.
pub fn attach(core: &mut CoreState, planet: Planet, attachment: Attachment) {
    let cell = core.board_mut().planet_mut(planet);
    cell.flags.set(PlanetFlags::ATTACH_ENTRY, true);
    cell.attach.push(attachment);
}

/// `state.frontier_tokens.remove(system)`: whether a token was there.
pub fn take_frontier(core: &mut CoreState, system: System) -> bool {
    core.explore_mut().frontier.remove(system)
}

/// `exploration::place_frontier_tokens`: the token set becomes exactly `systems`.
pub fn place_frontier_tokens(core: &mut CoreState, systems: &[System]) {
    let set = &mut core.explore_mut().frontier;
    *set = systems.iter().copied().collect();
}

/// `state.wormhole_tokens.insert(kind, system)` (Gamma Wormhole, Gamma Relay).
pub fn place_wormhole_token(core: &mut CoreState, kind: Wormhole, system: System) {
    core.explore_mut().wormhole_tokens.insert(kind, system);
}

/// `state.ion_storm = Some((system, face))`.
pub fn place_ion_storm(core: &mut CoreState, system: System, face: Wormhole) {
    core.meta_mut().ion_storm = Some((system, face));
}

/// `relics::crown_of_emphidia_point`: a held Crown with the Tomb of Emphidia on a planet the seat controls is
/// purged for 1 victory point (capped at the target; the ledger row carries the applied amount, 0 included).
pub fn crown_point(core: &mut CoreState, who: Seat) -> bool {
    let crown = Relic::TheCrownOfEmphidia;
    if !core.players().by_seat(who).is_some_and(|p| p.cards.relics.as_slice().contains(&crown)) {
        return false;
    }
    let has_tomb = crate::query::board::controlled_planets(core, who)
        .as_slice()
        .iter()
        .any(|(_, planet)| core.board().planet(*planet).attach.as_slice().contains(&Attachment::TombOfEmphidia));
    if !has_tomb {
        return false;
    }
    purge_relic(core, who, crown);
    let Some(p) = core.players_mut().by_seat_mut(who) else { return true };
    let before = i32::from(p.econ.victory_points);
    let after = (before + 1).min(VICTORY_TARGET);
    p.econ.victory_points = i16::try_from(after).unwrap_or(0);
    core.ledger_mut().vp.push(VpRec { player: who, delta: after - before, reason: LedgerReason::Relic(crown) });
    true
}

/// `exploration::purge_for_relic`'s sibling in `relics::codex`: the card leaves the discard pile (its first
/// occurrence) for the seat's hand. `false` when the pile does not hold it or the seat is not there.
pub fn take_discarded_action_card(core: &mut CoreState, who: Seat, card: crate::generated::ActionCard) -> bool {
    if core.players().by_seat(who).is_none() {
        return false;
    }
    let pile = core.cards_mut().discarded_action.edit();
    let Some(at) = pile.iter().position(|held| *held == card) else { return false };
    pile.remove(at);
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.hand.push(card);
    }
    true
}

/// `SystemState::purge_planet` and `planet_attachments.remove` (Stellar Converter): the planet's units, control,
/// coexistence and attachments go and it is marked purged.
pub fn purge_planet(core: &mut CoreState, planet: Planet) {
    let cell = core.board_mut().planet_mut(planet);
    cell.units = Default::default();
    cell.control = None;
    cell.coexist = Default::default();
    cell.attach = Default::default();
    for flag in [PlanetFlags::UNITS_ENTRY, PlanetFlags::COEXIST_ENTRY, PlanetFlags::ATTACH_ENTRY] {
        cell.flags.set(flag, false);
    }
    cell.flags.set(PlanetFlags::PURGED, true);
}

/// Status phase 81.6 (`status::resolve_after_token_gain`), the relic and legendary half: every seat's relic cards
/// and legendary ability cards are readied. (The technologies it readies there are the strategy area's.)
pub fn ready_relics_and_legendary(core: &mut CoreState) {
    for i in 0..core.players().len() {
        let p = core.players_mut().edit(i);
        p.cards.relics_exhausted.clear();
        p.cards.legendary_exhausted.clear();
    }
}

/// `legendary::exhaust`: the ability card of a legendary planet is spent.
pub fn exhaust_legendary(core: &mut CoreState, who: Seat, planet: Planet) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.legendary_exhausted.insert(planet, ());
    }
}

/// `seat.exploration_cards.push(card)` (Enigmatic Device, the exploration card): kept faceup in the play area.
pub fn play_area_card(core: &mut CoreState, who: Seat, card: Explore) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.explore.push(card);
    }
}

/// Maw of Worlds: every planet the seat controls is exhausted (the cost, paid whatever is chosen).
pub fn exhaust_controlled_planets(core: &mut CoreState, who: Seat) {
    let planets: Vec<Planet> = crate::query::board::controlled_planets(core, who).as_slice().iter().map(|(_, p)| *p).collect();
    for planet in planets {
        crate::kernel::fx::exhaust_planet(core, planet);
    }
}
