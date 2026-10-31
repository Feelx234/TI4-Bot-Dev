//! The exploration writes that cards of other areas make (package B6d, layering): the draw of an exploration card, a fragment
//! gained, the frontier token spent, the wormhole token placed, a card kept in the play area, the commodities replenished or
//! converted, the log row opened when a card is drawn.
//!
//! These bodies are state-only primitives (no window, no kernel service), and an action card plays them in the middle of its own
//! window (Exploration Probe, Merchant Station): areas do not import each other, so the bodies are here and
//! `rules::explore` re-exports them (the same split as [`super::fx_relic`]).

use crate::generated::{Explore, ExploreType, Planet, System, Wormhole};
use crate::ids::Seat;
use crate::state::{CoreState, PlanetFlags};

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

/// `state.frontier_tokens.remove(system)`: whether a token was there.
pub fn take_frontier(core: &mut CoreState, system: System) -> bool {
    core.explore_mut().frontier.remove(system)
}

/// `state.wormhole_tokens.insert(kind, system)` (Gamma Wormhole, Gamma Relay).
pub fn place_wormhole_token(core: &mut CoreState, kind: Wormhole, system: System) {
    core.explore_mut().wormhole_tokens.insert(kind, system);
}

/// `seat.exploration_cards.push(card)` (Enigmatic Device, the exploration card): kept faceup in the play area.
pub fn play_area_card(core: &mut CoreState, who: Seat, card: Explore) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        p.cards.explore.push(card);
    }
}

/// `exploration::gain_commodities`: up to `count` commodities, never past the faction's value. Returns the amount
/// gained (the string side announces it to a Siphon, which the adapter does not follow).
pub fn gain_commodities(core: &mut CoreState, who: Seat, count: i32) -> i32 {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return 0 };
    let limit = i32::from(p.faction.data().commodities);
    let held = i32::from(p.econ.commodities);
    let after = (held + count).min(limit);
    p.econ.commodities = i16::try_from(after).unwrap_or(i16::MAX);
    (after - held).max(0)
}

/// `exploration::convert_commodities`, the commodities half: up to `most` commodities (all when `None`) leave the
/// seat. Returns how many; the trade goods they become are gained by the caller (`supply`).
pub fn spend_commodities(core: &mut CoreState, who: Seat, most: Option<i32>) -> i32 {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return 0 };
    let held = i32::from(p.econ.commodities);
    let moved = most.map_or(held, |cap| held.min(cap));
    p.econ.commodities = i16::try_from(held - moved).unwrap_or(0);
    moved
}

/// `exploration::explore_drawn`'s record: the log row opened when the card is drawn, before it resolves. The
/// outcome, result, gains and placed texts come with `rules::explore::close_record`; `empty` is the interned empty text.
pub fn open_record(
    core: &mut CoreState,
    who: Seat,
    deck: ExploreType,
    card: Explore,
    planet: Option<Planet>,
    system: Option<System>,
    empty: crate::state::Code,
) {
    let round = core.meta().round;
    core.explore_mut().log.push(crate::state::ExploreRec {
        round,
        player: who,
        deck,
        card,
        planet,
        outcome: empty,
        system,
        result: None,
        gains: Default::default(),
        placed: Default::default(),
    });
}

/// `planets::place` as Mirage uses it ("place the Mirage planet token in this system, gain the planet card and
/// ready it"): the planet is put into `system` under the seat's control, readied. `false` when the planet is
/// already on the board (a card cannot place it twice); a planet that was destroyed keeps no controller.
pub fn place_planet(core: &mut CoreState, who: Seat, system: System, planet: Planet) -> bool {
    let cell = core.board_mut().planet_mut(planet);
    if cell.placed_in.is_some() {
        return false;
    }
    cell.placed_in = Some(system);
    if !cell.flags.has(PlanetFlags::PURGED) {
        cell.control = Some(who);
    }
    cell.flags.set(PlanetFlags::EXHAUSTED, false);
    core.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
    true
}

// -- the words of a log row (the adapter's) --------------------------------------------------------------

/// What the adapter needs to write the words of the log row of a card the seat resolved (`exploration::row_words`): the planet (if
/// any), the card, how it resolved, what the seat held before the draw and after the last effect, the gamma token newly placed, the ion
/// storm token placed, the last card kept in the seat's play area and the attachment last put on the planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowIn {
    pub planet: Option<Planet>,
    pub card: Explore,
    pub how: crate::state::EHow,
    pub before: crate::state::Holdings,
    pub after: crate::state::Holdings,
    pub gamma: Option<System>,
    pub ion: Option<(System, Wormhole)>,
    pub kept: Option<Explore>,
    pub attached: Option<crate::generated::Attachment>,
}

/// The texts a log row is closed with (`ExplorationRecord::{outcome, result, gains, placed}`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowOut {
    pub outcome: String,
    pub result: String,
    pub gains: Vec<(String, i32)>,
    pub placed: Vec<String>,
}

/// The adapter's `row_words`.
pub type WordsFn = fn(&RowIn) -> RowOut;

/// Close the log row at `index` with the words (the texts are minted in the position's text table).
pub fn close_row(core: &mut CoreState, index: usize, words: &RowOut) -> Option<()> {
    use crate::state::{Code, CODE_CAPACITY};
    let mut mint = |text: &str| -> Option<Code> { core.texts.codes.intern(text.as_bytes(), CODE_CAPACITY).and_then(|i| u16::try_from(i).ok()).map(Code) };
    let outcome = mint(&words.outcome)?;
    // The string writes an empty `result` as the empty text and the core keeps `None` for it.
    let result = if words.result.is_empty() { None } else { Some(mint(&words.result)?) };
    let mut gains = Vec::with_capacity(words.gains.len());
    for (key, n) in &words.gains {
        gains.push((mint(key)?, *n));
    }
    let mut placed = Vec::with_capacity(words.placed.len());
    for text in &words.placed {
        placed.push(mint(text)?);
    }
    let rec = core.explore_mut().log.get_mut(index)?;
    rec.outcome = outcome;
    rec.result = result;
    rec.gains = gains.into_iter().collect();
    rec.placed = placed.into_iter().collect();
    Some(())
}

/// The empty text of an open log row.
pub fn empty_text(core: &mut CoreState) -> Option<crate::state::Code> {
    use crate::state::{Code, CODE_CAPACITY};
    core.texts.codes.intern(b"", CODE_CAPACITY).and_then(|i| u16::try_from(i).ok()).map(Code)
}
