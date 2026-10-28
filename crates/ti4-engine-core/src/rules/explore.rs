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
