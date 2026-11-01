//! `fx`: the shared state-changing primitives that also fire their trigger (docs/ENGINE_CORE_STATE.md
//! 4.1: "pure helpers shared by several areas are L3 primitives").
//!
//! Each primitive makes the change on the state and either returns the event it would announce
//! (`Option<EventRec>` for the caller to emit through its resolver) or, for a site with no resolver in
//! reach, stages it in `pending.staged` ([`stage`]) for [`announce_staged`]. Nothing here asks a
//! question; nothing calls a rule area.

use crate::choice::reaction::Relation;
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::resolver::{Host, Line, Resolver, TimingError};
use crate::kernel::Cx;
use crate::generated::SecretObjective;
use crate::state::{Code, CoreState, Emit, EventKind, EventRec, Key, LedgerReason, ObjectiveRef, Payload, Piece, PlainRec, PlanetFlags, Val, VpRec};

/// A new event record (id 0: the emitter assigns the trace id).
#[must_use]
pub fn event(kind: EventKind, payload: Payload) -> EventRec {
    EventRec { id: 0, kind, payload, cancelled: false }
}

// -- staging ------------------------------------------------------------------------------------

/// Stage an event for a later announcement.
pub fn stage(s: &mut CoreState, rec: EventRec) {
    s.pending_mut().staged.push(rec);
}

/// Whether anything is staged.
#[must_use]
pub fn has_staged(s: &CoreState) -> bool {
    !s.pending().staged.is_empty()
}

/// Take the staged events in the order they were staged.
pub fn drain_staged(s: &mut CoreState) -> Vec<EventRec> {
    std::mem::take(&mut s.pending_mut().staged)
}

/// Announce every staged event through `resolver`, in order, until none is left (a window may stage
/// more). `next_id` allocates the trace ids.
///
/// # Errors
/// Whatever the resolver refuses with; events not yet announced stay staged.
pub fn announce_staged<H: Host + ?Sized>(
    resolver: &mut Resolver,
    host: &mut H,
    state: impl Fn(&mut H) -> &mut CoreState,
    next_id: &mut u64,
) -> Result<usize, TimingError> {
    let mut announced = 0;
    loop {
        let batch = drain_staged(state(host));
        if batch.is_empty() {
            return Ok(announced);
        }
        let mut it = batch.into_iter();
        while let Some(mut rec) = it.next() {
            rec.id = *next_id;
            *next_id += 1;
            if let Err(e) = resolver.emit(host, rec, |_, _, _| {}) {
                // The ones after it were drained already: put them back in front.
                let mut again: Vec<EventRec> = it.collect();
                let s = state(host);
                again.append(&mut s.pending_mut().staged);
                s.pending_mut().staged = again;
                return Err(e);
            }
            announced += 1;
        }
    }
}

// -- the emission queue (package P1) -------------------------------------------------------------

/// Append a plain event (`Game::emit`) to the ordered emission queue `ledger.events`, at once, as the string pushes it.
///
/// The queue is output only: the codecs neither load nor store it, so this needs no slice in a footprint (it writes the
/// field, not through the `ledger_mut` accessor). It lives in the state, so a step the driver rolls back takes the plain
/// events it wrote with it.
pub fn plain(s: &mut CoreState, rec: PlainRec) {
    debug_assert!(rec.is_valid(), "a plain event whose arguments do not fit its kind: {rec:?}");
    s.ledger.events.push(Emit::Plain(rec));
}

/// Write events a window emitted (`Out::emitted`, the records as they ended) to the emission queue as mirrored ones, in order: the
/// string's `mirror_timing_log` for a step whose windows return their emissions instead of mirroring them one by one.
pub fn mirror_recs(s: &mut CoreState, recs: Vec<EventRec>) {
    for rec in recs {
        s.ledger.events.push(Emit::Typed(rec));
    }
}

/// The cursor of [`mirror_journal`]: how many lines the resolver's trace has now. Take it BEFORE the timing call
/// whose events are to be mirrored (the string's `let logged = self.timing.log().len()`).
#[must_use]
pub fn journal_mark(resolver: &Resolver) -> usize {
    resolver.log().len()
}

/// Mirror the events the resolver emitted since `from` into the emission queue, in the resolver's order, and return the
/// new cursor. The string engine's `mirror_timing_log` does the same with the bare names of every `emit NAME#id` line of
/// the trace since `logged`, the events nested abilities caused included, when the surrounding call returns: a window
/// calls this at the same program point, so a plain event pushed between the emission and this call is stored BEFORE
/// the emission's events, exactly as in `Game::events`.
pub fn mirror_journal(s: &mut CoreState, resolver: &Resolver, from: usize) -> usize {
    for rec in journal_recs(resolver, from) {
        s.ledger.events.push(Emit::Typed(rec));
    }
    resolver.log().len()
}

/// The events the resolver emitted since `from`, in the order of its trace and as they ended (cancelled or not, payload as the windows left
/// it): [`mirror_journal`] without the write. Nested emissions (the card a reaction plays inside another event) are in it, after their parent.
#[must_use]
pub fn journal_recs(resolver: &Resolver, from: usize) -> Vec<EventRec> {
    let mut out = Vec::new();
    for line in resolver.log().iter().skip(from) {
        if let Line::Emit { kind, id } = line {
            // The journal keeps the event as it ended; the trace line only names it. A trace line without a journal entry (a failed emission)
            // keeps its bare name.
            let rec = resolver
                .applied_events()
                .iter()
                .rev()
                .find(|e| e.id == *id && e.kind == *kind)
                .cloned()
                .unwrap_or(EventRec { id: *id, kind: *kind, payload: Payload::new(), cancelled: false });
            out.push(rec);
        }
    }
    out
}

// -- economy ------------------------------------------------------------------------------------

/// Give `who` `amount` trade goods and return how many were gained (`0` for a missing seat or a
/// non-positive amount). The counter is an `i16`: a gain that would overflow is clamped, never wrapped.
pub fn gain_trade_goods(s: &mut CoreState, who: Seat, amount: i32) -> i32 {
    if amount <= 0 {
        return 0;
    }
    let Some(p) = s.players_mut().by_seat_mut(who) else { return 0 };
    let before = i32::from(p.econ.trade_goods);
    let after = (before + amount).min(i32::from(i16::MAX));
    p.econ.trade_goods = i16::try_from(after).unwrap_or(i16::MAX);
    after - before
}

/// Spend `amount` trade goods (`supply::spend_goods` without a Keleres commodity window): all of it or none,
/// `false` and the state untouched when `who` has fewer.
pub fn spend_trade_goods(s: &mut CoreState, who: Seat, amount: i32) -> bool {
    if amount <= 0 {
        return true;
    }
    let Some(p) = s.players_mut().by_seat_mut(who) else { return false };
    if i32::from(p.econ.trade_goods) < amount {
        return false;
    }
    p.econ.trade_goods -= i16::try_from(amount).unwrap_or(i16::MAX);
    true
}

/// `promissory::give_back`: the note goes home to the first seat (in seating order) that plays its faction;
/// nothing when it is already there or nobody plays the faction.
pub fn give_back_note(s: &mut CoreState, note: crate::state::NoteKey) {
    let owner = s
        .seats()
        .seating_order()
        .iter()
        .copied()
        .find(|seat| s.players().by_seat(*seat).is_some_and(|p| p.faction == note.faction));
    let Some(owner) = owner else { return };
    if s.cards().promissory.get(&note) == Some(&owner) {
        return;
    }
    let cards = s.cards_mut();
    cards.promissory.edit().insert(note, owner);
    cards.promissory_faceup.edit().remove(&note);
}

/// `strategy_cards::replenish` without the announcements: the seat's commodities go to its faction's printed
/// value (a seat above it is set down to it, as the string does). Returns how many were gained; `0` for a seat
/// that is not there. The Siphon event, Cabal's note and the Trade Agreement hook are the string callers'.
pub fn replenish_commodities(s: &mut CoreState, who: Seat) -> i32 {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return 0 };
    let limit = i16::from(p.faction.data().commodities);
    let gained = (limit - p.econ.commodities).max(0);
    p.econ.commodities = limit;
    i32::from(gained)
}

/// `strategy_cards::replenish` with what the string announces after the commodity write: the `COMMODITIES_REPLENISHED`
/// Cabal's agent listens to (`cabal::note_replenished`: staged only while a Cabal is seated, for a seat that holds commodities),
/// then the Trade Agreement (`promissory::trade_agreement_on_replenish`): the owner's commodities become the holder's trade
/// goods, the note goes home, and the holder's `TRADE_GOODS_GAINED` (source `source`) is staged. Returns how many commodities
/// the replenish gained. `None` (nothing written) where the owner's faction is played by two seats, as
/// `rules::trade::trade_agreement`. The Siphon event of an Obsidian plot is not here (a plot nobody can hold).
pub fn replenish_announced(s: &mut CoreState, who: Seat, source: Code) -> Option<i32> {
    let faction = s.players().by_seat(who)?.faction;
    if s.players().iter().filter(|p| p.faction == faction).count() > 1 {
        return None;
    }
    let gained = replenish_commodities(s, who);
    let held = i32::from(s.players().by_seat(who).map_or(0, |p| p.econ.commodities));
    if held > 0 && s.players().iter().any(|p| p.faction == crate::generated::Faction::TheVuilraithCabal) {
        stage(s, event(EventKind::CommoditiesReplenished, Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Commodities, Val::Int(i64::from(held)))));
    }
    let note = crate::state::NoteKey { note: crate::generated::PromissoryNote::TradeAgreement, faction };
    if let Some(holder) = crate::query::promissory::holder_of(s, note.note, who) {
        if held > 0 {
            if let Some(p) = s.players_mut().by_seat_mut(who) {
                p.econ.commodities = 0;
            }
            if let Some(h) = s.players_mut().by_seat_mut(holder) {
                h.econ.trade_goods = h.econ.trade_goods.saturating_add(i16::try_from(held).unwrap_or(0));
            }
            give_back_note(s, note);
            stage(s, trade_goods_gained(holder, held, source));
        }
    }
    Some(gained)
}

/// The `TRADE_GOODS_GAINED` event of a gain (`player`, `amount`, `source`).
#[must_use]
pub fn trade_goods_gained(who: Seat, gained: i32, source: Code) -> EventRec {
    event(
        EventKind::TradeGoodsGained,
        Payload::new()
            .with(Key::Player, Val::Seat(who))
            .with(Key::Amount, Val::Int(i64::from(gained)))
            .with(Key::Source, Val::Code(source)),
    )
}

/// [`gain_trade_goods`], then the event to announce (`None` for a gain of zero).
pub fn gain_trade_goods_announced(s: &mut CoreState, who: Seat, amount: i32, source: Code) -> (i32, Option<EventRec>) {
    let gained = gain_trade_goods(s, who, amount);
    (gained, (gained > 0).then(|| trade_goods_gained(who, gained, source)))
}

/// `supply::note_trade_goods_gained`: stage the `TRADE_GOODS_GAINED` of a gain the caller has already made, with the amount the caller
/// names (no cap is applied). Nothing for `amount <= 0`.
pub fn stage_gain(s: &mut CoreState, who: Seat, amount: i32, source: Code) {
    if amount > 0 {
        stage(s, trade_goods_gained(who, amount, source));
    }
}

/// [`gain_trade_goods`] for a site with no resolver: the event is staged.
pub fn gain_trade_goods_staged(s: &mut CoreState, who: Seat, amount: i32, source: Code) -> i32 {
    let (gained, rec) = gain_trade_goods_announced(s, who, amount, source);
    if let Some(rec) = rec {
        stage(s, rec);
    }
    gained
}

/// Add victory points (negative removes) and append the ledger row. Returns the new total.
pub fn gain_victory_points(s: &mut CoreState, who: Seat, delta: i32, reason: LedgerReason) -> i32 {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return 0 };
    let total = (i32::from(p.econ.victory_points) + delta).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
    p.econ.victory_points = i16::try_from(total).unwrap_or(0);
    s.ledger_mut().vp.push(VpRec { player: who, delta, reason });
    total
}

/// `laws::steal_throne_card`: the elected owner of a throne card (the Crown of Emphidia) loses its point (no floor at zero: the string takes it
/// away as it holds it) and the taker gains one; the card is elected onto the taker. `false` when nobody holds it or the taker already does.
pub fn steal_throne_card(s: &mut CoreState, law: crate::generated::Agenda, taker: Seat) -> bool {
    use crate::keys::Outcome;
    let Some(Outcome::Seat(owner)) = s.agenda().laws.get(&law).copied() else { return false };
    if owner == taker {
        return false;
    }
    if let Some(p) = s.players_mut().by_seat_mut(owner) {
        p.econ.victory_points = p.econ.victory_points.saturating_sub(1);
    }
    s.ledger_mut().vp.push(VpRec { player: owner, delta: -1, reason: LedgerReason::Agenda(law) });
    adjust_victory_points(s, taker, 1, LedgerReason::Agenda(law));
    s.agenda_mut().laws.insert(law, Outcome::Seat(taker));
    true
}

/// `objectives::reveal_stage` (61.13, 61.14a): the first facedown public objective of this stage turns faceup,
/// wherever it is in the deck. Shared with the agenda effects (Incentive Program); areas do not import each other.
pub fn reveal_stage(s: &mut CoreState, stage: u8) -> Option<crate::generated::PublicObjective> {
    let at = s.decks().objective.cards().iter().position(|o| crate::query::objectives::stage_of(*o) == Some(stage))?;
    let card = s.decks_mut().objective.remove(at);
    s.objectives_mut().revealed.push(ObjectiveRef::Public(card));
    Some(card)
}

/// `objectives::VICTORY_TARGET`: no seat holds more points than this (98.4a).
pub const VICTORY_TARGET: i32 = 10;

/// `objectives::adjust_victory_points`: a gain stops at [`VICTORY_TARGET`], a loss at zero, and the ledger row
/// carries the change that happened (a row of 0 included). Returns that change; 0, with no row, for a seat that
/// is not there.
pub fn adjust_victory_points(s: &mut CoreState, who: Seat, delta: i32, reason: LedgerReason) -> i32 {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return 0 };
    let before = i32::from(p.econ.victory_points);
    let after = if delta >= 0 { (before + delta).min(VICTORY_TARGET) } else { (before + delta).max(0) };
    p.econ.victory_points = i16::try_from(after).unwrap_or(0);
    let applied = after - before;
    s.ledger_mut().vp.push(VpRec { player: who, delta: applied, reason });
    applied
}

/// `GameState::record_score`: the objective is one the seat scored.
pub fn record_score(s: &mut CoreState, who: Seat, objective: ObjectiveRef) {
    let o = s.objectives_mut();
    o.scored_present.insert(who);
    let cell = &mut o.scored[usize::from(who.0)];
    match objective {
        ObjectiveRef::Public(p) => {
            cell.public.insert(p);
        }
        ObjectiveRef::Secret(x) => {
            cell.secret.insert(x);
        }
    }
}

/// `GameState::record_occurrence_score`: the seat used its score of one feat occurrence.
pub fn record_occurrence_score(s: &mut CoreState, who: Seat, occurrence: u64) {
    let feats = s.objectives_mut().scored_feats.edit();
    if !feats.contains(&(who, occurrence)) {
        feats.push((who, occurrence));
    }
}

/// The state half of `objectives::award` (once the price is paid): the score is recorded and the points move.
/// Returns the points.
pub fn score_public(s: &mut CoreState, who: Seat, objective: ObjectiveRef) -> i32 {
    let points = match objective {
        ObjectiveRef::Public(p) => crate::query::objectives::points_of(p),
        ObjectiveRef::Secret(x) => crate::query::objectives::secret_points(x),
    };
    record_score(s, who, objective);
    adjust_victory_points(s, who, i32::from(points), LedgerReason::Objective);
    i32::from(points)
}

/// The state half of `secrets::award` (once the price is paid): the score is recorded, the secret leaves the
/// hand, the points move. `false` (nothing changes) when the secret is not in the hand.
pub fn score_secret(s: &mut CoreState, who: Seat, secret: SecretObjective) -> bool {
    if !s.players().by_seat(who).is_some_and(|p| p.cards.secrets.as_slice().contains(&secret)) {
        return false;
    }
    record_score(s, who, ObjectiveRef::Secret(secret));
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.cards.secrets.retain(|held| *held != secret);
    }
    adjust_victory_points(s, who, i32::from(crate::query::objectives::secret_points(secret)), LedgerReason::SecretObjective);
    true
}

/// `state.finished = true` once somebody reached the target (`objectives::winner`).
pub fn finish_if_won(s: &mut CoreState) {
    if s.players().iter().any(|p| i32::from(p.econ.victory_points) >= VICTORY_TARGET) {
        s.meta_mut().finished = true;
    }
}

/// The draw of `secrets::draw` (before the hand limit): the top secret goes to the seat's hand. `None` for an
/// empty deck or a seat that is not there.
pub fn draw_secret(s: &mut CoreState, who: Seat) -> Option<SecretObjective> {
    s.players().by_seat(who)?;
    let top = s.decks_mut().secret.draw()?;
    s.players_mut().by_seat_mut(who)?.cards.secrets.push(top);
    Some(top)
}

/// `secrets::enforce_hand_limit`'s hand-back: the secret leaves the hand for the bottom of the deck.
pub fn return_secret(s: &mut CoreState, who: Seat, secret: SecretObjective) {
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.cards.secrets.retain(|held| *held != secret);
    }
    s.decks_mut().secret.put_bottom(secret);
}

/// Classified Document Leaks (`laws::classified_leak`, `agenda_effects::effect_classified`): the secret becomes a
/// public objective, scoreable by everybody. `false`, nothing changed, when it already is one.
pub fn make_public(s: &mut CoreState, secret: SecretObjective) -> bool {
    let o = ObjectiveRef::Secret(secret);
    if s.objectives().revealed.as_slice().contains(&o) {
        return false;
    }
    s.objectives_mut().revealed.push(o);
    true
}

// -- board --------------------------------------------------------------------------------------

/// Place a unit in a system's space area, or on a planet when `planet` is given.
pub fn place_unit(s: &mut CoreState, who: Seat, system: System, planet: Option<Planet>, unit: Unit) {
    let piece = Piece::new(unit, who);
    match planet {
        Some(p) => {
            let cell = s.board_mut().planet_mut(p);
            cell.units.push(piece);
            cell.flags.set(PlanetFlags::UNITS_ENTRY, true);
            s.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
        }
        None => {
            let cell = s.board_mut().sys_mut(system);
            cell.units.push(piece);
            cell.flags.set(crate::state::SysFlags::TOUCHED, true);
        }
    }
}

/// `supply::stage_naaz_mech_placed`: a Naaz mech put on the board announces itself (`NAAZ_MECH_PLACED`), so Absolute Synergy
/// can resolve before the next action. Other units and other factions stage nothing.
pub fn stage_naaz_mech_placed(s: &mut CoreState, who: Seat, system: System, unit: Unit) {
    if unit == Unit::Eidolon && s.players().by_seat(who).is_some_and(|p| p.faction == crate::generated::Faction::TheNaazRokhaAlliance) {
        let payload = Payload::new().with(Key::Player, Val::Seat(who)).with(Key::System, Val::System(system));
        stage(s, event(EventKind::NaazMechPlaced, payload));
    }
}

/// Remove the first unit of this type and owner from a system's space area or a planet; whether one
/// was removed. Order of the others is kept.
pub fn remove_unit(s: &mut CoreState, who: Seat, system: System, planet: Option<Planet>, unit: Unit) -> bool {
    let list = match planet {
        Some(p) => &mut s.board_mut().planet_mut(p).units,
        None => &mut s.board_mut().sys_mut(system).units,
    };
    let Some(at) = list.as_slice().iter().position(|u| u.kind() == unit && u.owner() == who) else {
        return false;
    };
    list.remove(at);
    true
}

/// Place a command token of `who` in `system`; whether it was new.
pub fn place_command_token(s: &mut CoreState, who: Seat, system: System) -> bool {
    let cell = s.board_mut().sys_mut(system);
    cell.flags.set(crate::state::SysFlags::TOUCHED, true);
    cell.cmd.insert(who)
}

/// Take `who`'s command token out of a system; whether one was there.
pub fn remove_command_token(s: &mut CoreState, who: Seat, system: System) -> bool {
    s.board_mut().sys_mut(system).cmd.remove(who)
}

/// How many of `who`'s command tokens sit in a Mahact fleet pool (`GameState::tokens_held_by_mahact`: the fleet pool marks of every seat).
/// Zero where the marks are not part of the lend (the window's footprint does not read them).
#[must_use]
pub fn tokens_held_by_mahact(s: &CoreState, who: Seat) -> i32 {
    use crate::keys::{Atom, MarkKey, MarkVal};
    if !s.loaded.has(crate::state::Slice::Marks) {
        return 0;
    }
    let held: usize = s
        .marks()
        .tail
        .iter()
        .filter(|(k, _)| matches!(k, MarkKey::MahactFleet(_)))
        .map(|(_, v)| match v {
            MarkVal::List(items) => items.iter().filter(|i| matches!(i, MarkVal::Atom(Atom::Seat(owner)) if *owner == who)).count(),
            _ => 0,
        })
        .sum();
    i32::try_from(held).unwrap_or(i32::MAX)
}

/// Command tokens in a seat's reinforcements: sixteen less the sheet, the board and the Mahact fleet pools (`GameState::
/// tokens_in_reinforcements`).
#[must_use]
pub fn tokens_in_reinforcements(s: &CoreState, who: Seat) -> i32 {
    let on_sheet = s.players().by_seat(who).map_or(0, |p| {
        i32::from(p.econ.tactic_tokens) + i32::from(p.econ.fleet_tokens) + i32::from(p.econ.strategic_tokens)
    });
    let on_board = s.board().systems().filter(|(_, cell)| cell.cmd.contains(who)).count();
    (TOKENS_PER_FACTION - on_sheet - i32::try_from(on_board).unwrap_or(i32::MAX) - tokens_held_by_mahact(s, who)).max(0)
}

/// `GameState::gain_token` for a positive count: at most what is in reinforcements. Returns how many were gained.
pub fn gain_token(s: &mut CoreState, who: Seat, pool: crate::choice::strategy::Pool, count: i32) -> i32 {
    use crate::choice::strategy::Pool;
    let granted = count.min(tokens_in_reinforcements(s, who)).max(0);
    if granted > 0
        && let Some(p) = s.players_mut().by_seat_mut(who)
    {
        let slot = match pool {
            Pool::Tactic => &mut p.econ.tactic_tokens,
            Pool::Fleet => &mut p.econ.fleet_tokens,
            Pool::Strategic => &mut p.econ.strategic_tokens,
        };
        *slot = slot.saturating_add(i16::try_from(granted).unwrap_or(i16::MAX));
    }
    granted
}

/// The next `CardsStaged` mark: a card event waiting for the game step (`hooks_cards::stage`), `word` says which, `from` is who the card came
/// from (a taken card; absent for a discard).
fn stage_card_event(s: &mut CoreState, word: crate::keys::Word, who: Seat, from: Option<Seat>, card: crate::generated::ActionCard) {
    use crate::keys::{Atom, MarkKey, MarkVal};
    let next = s.marks().tail.iter().filter_map(|(k, _)| if let MarkKey::CardsStaged(n) = k { Some(*n) } else { None }).max().map_or(0, |n| n + 1);
    let from = from.map_or(MarkVal::Atom(Atom::Absent), |seat| MarkVal::Atom(Atom::Seat(seat)));
    let row = MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(word)), MarkVal::Atom(Atom::Seat(who)), from, MarkVal::Atom(Atom::Card(card))]);
    s.marks_mut().tail.edit().insert(MarkKey::CardsStaged(next), row);
}

/// `hooks_cards::discard_chosen`: the first copy of `card` leaves the seat's hand and the discard waits for the game step, staged as the
/// next `CardsStaged` mark (`discarded|<seat>||<card>`). `false` (nothing written) when the seat does not hold the card.
pub fn discard_action_card_staged(s: &mut CoreState, who: Seat, card: crate::generated::ActionCard) -> bool {
    let Some(at) = s.players().by_seat(who).and_then(|p| p.cards.hand.as_slice().iter().position(|held| *held == card)) else {
        return false;
    };
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.cards.hand.remove(at);
    }
    stage_card_event(s, crate::keys::Word::Discarded, who, None, card);
    true
}

/// `hooks_cards::take_revealed_action_card`: the first copy of `card` in `from`'s hand goes to the end of `taker`'s hand and the event
/// waits for the game step, staged as the next `CardsStaged` mark (`taken|<taker>|<from>|<card>`). The hand limit is the caller's to
/// enforce. `false` (nothing written) when the seats are the same or `from` does not hold the card.
pub fn take_action_card_staged(s: &mut CoreState, taker: Seat, from: Seat, card: crate::generated::ActionCard) -> bool {
    if taker == from || s.players().by_seat(taker).is_none() {
        return false;
    }
    let Some(at) = s.players().by_seat(from).and_then(|p| p.cards.hand.as_slice().iter().position(|held| *held == card)) else {
        return false;
    };
    if let Some(p) = s.players_mut().by_seat_mut(from) {
        p.cards.hand.remove(at);
    }
    if let Some(p) = s.players_mut().by_seat_mut(taker) {
        p.cards.hand.push(card);
    }
    stage_card_event(s, crate::keys::Word::Taken, taker, Some(from), card);
    true
}

/// The plain action card draw (`action_cards::draw` with no faction hook, no reserve and a hand that stays
/// within the limit): up to `count` cards from the top of the deck into the hand. Returns how many came.
/// `drop_unofficial`: the two unofficial cards leave the deck first (`deck::drop_unofficial_action_cards`).
pub fn draw_action_cards(s: &mut CoreState, who: Seat, count: usize, drop_unofficial: bool) -> usize {
    use crate::generated::ActionCard;
    if drop_unofficial && s.decks().action.cards().iter().any(|c| matches!(c, ActionCard::Veto3 | ActionCard::Veto4)) {
        let kept: Vec<ActionCard> = s.decks().action.cards().iter().copied().filter(|c| !matches!(c, ActionCard::Veto3 | ActionCard::Veto4)).collect();
        s.decks_mut().action.set(kept);
    }
    let mut drawn = 0;
    for _ in 0..count {
        let Some(card) = s.decks_mut().action.draw() else { break };
        if let Some(p) = s.players_mut().by_seat_mut(who) {
            p.cards.hand.push(card);
        }
        drawn += 1;
    }
    drawn
}

/// The plain gain of a researched technology (`technology::grant`: the card joins the seat's technologies;
/// the unit upgrades, the laws and the notes that follow a research are the caller's). Whether it was new.
pub fn grant_technology(s: &mut CoreState, who: Seat, tech: crate::generated::Technology) -> bool {
    s.players_mut().by_seat_mut(who).is_some_and(|p| p.tech.owned.insert(tech))
}

/// Spend one command token from the strategy pool (`Player::spend_token(Strategic)`): whether the pool had one.
pub fn spend_strategic_token(s: &mut CoreState, who: Seat) -> bool {
    let Some(p) = s.players_mut().by_seat_mut(who) else { return false };
    if p.econ.strategic_tokens > 0 {
        p.econ.strategic_tokens -= 1;
        true
    } else {
        false
    }
}

/// Command tokens a faction has (`ti4_model::state::TOKENS_PER_FACTION`, LRR 20.4).
pub const TOKENS_PER_FACTION: i32 = 16;

/// Exhaust a planet; whether it was ready.
pub fn exhaust_planet(s: &mut CoreState, planet: Planet) -> bool {
    let cell = s.board_mut().planet_mut(planet);
    let was_ready = !cell.flags.has(PlanetFlags::EXHAUSTED);
    cell.flags.set(PlanetFlags::EXHAUSTED, true);
    was_ready
}

/// Ready a planet; whether it was exhausted.
pub fn ready_planet(s: &mut CoreState, planet: Planet) -> bool {
    let cell = s.board_mut().planet_mut(planet);
    let was_exhausted = cell.flags.has(PlanetFlags::EXHAUSTED);
    cell.flags.set(PlanetFlags::EXHAUSTED, false);
    was_exhausted
}

/// `who` gains control of a planet: set the controller, run the faction `control_gained` hooks (before
/// the shared code, in module order), and return the `PLANET_CONTROL_GAINED` event to announce
/// (`previous_owner` when there was one). A planet already controlled by `who` changes nothing and
/// returns `None`.
pub fn gain_control(cx: &mut Cx<'_>, who: Seat, system: System, planet: Planet) -> Option<EventRec> {
    let previous = cx.state.board().planet(planet).control;
    if previous == Some(who) {
        return None;
    }
    cx.state.board_mut().planet_mut(planet).control = Some(who);
    cx.state.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
    let kernel = cx.kernel;
    kernel.hooks.general.control_gained(cx, who, system, planet);
    let mut payload = Payload::new()
        .with(Key::Player, Val::Seat(who))
        .with(Key::System, Val::System(system))
        .with(Key::Planet, Val::Planet(planet));
    if let Some(prev) = previous {
        payload.set(Key::PreviousOwner, Val::Seat(prev));
    }
    Some(event(EventKind::PlanetControlGained, payload))
}

/// The window relations in the order a caller walks them (`When`, then `After`).
pub const RELATIONS: [Relation; 2] = [Relation::When, Relation::After];

/// `action_cards::max_fit` (package W11, lane 7): how many of `wanted` units of `unit` may be put in `system`'s space area (`in_space`) or on one of
/// its planets without making the fleet pool (37.1) or the space capacity (16.1) worse than it is now: the largest `n <= wanted` for which
/// [`fleet::standing`] shows no new fleet excess and no new capacity excess. The box is a separate limit ([`place_counted`]). `fleet_limit` is the
/// limit the string worked out (`fleet::limit`, or `fleet::UNLIMITED_FLEET_BILL`): the pool, the laws and the abilities are the adapter's.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "the position, the seat, the place, the unit, how many and the limit")]
pub fn max_fit(core: &CoreState, k: &crate::kernel::Kernel, scope: crate::tables::Scope, who: Seat, system: System, in_space: bool, unit: Unit, wanted: usize, fleet_limit: i64) -> usize {
    use crate::query::fleet::{self, Arrival};
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(core, k, who, u);
    let before = fleet::standing(core, scope, who, system, fleet_limit, None, &half);
    (1..=wanted)
        .rev()
        .find(|n| {
            let arrival = Arrival { kind: unit, count: i64::try_from(*n).unwrap_or(i64::MAX), in_space };
            let after = fleet::standing(core, scope, who, system, fleet_limit, Some(arrival), &half);
            after.fleet_excess() <= before.fleet_excess() && after.capacity_excess <= before.capacity_excess
        })
        .unwrap_or(0)
}

/// `supply::potential_goods`: the trade goods the seat can spend (the commodities an open Keleres agent window lets it spend as goods
/// included) plus what a readied Keleres agent could still grant: the up-front gate of an option that costs trade goods (Bribery, a vote
/// spend, Munitions Reserves).
#[must_use]
pub fn potential_goods(core: &CoreState, k: &crate::kernel::Kernel, who: Seat) -> i64 {
    // (The Keleres terms are dropped content: no table seats the faction, so the lockstep and the unit tests see them as 0.)
    let goods = core.players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods.max(0))); // mutants:skip: a seat of the list exists
    let granted = if k.hooks.direct.keleres_agent_could_grant(core, who) { core.players().by_seat(who).map_or(0, |p| i64::from(p.econ.commodities.max(0))) } else { 0 }; // mutants:skip: Keleres is dropped
    goods + k.hooks.direct.keleres_spendable_commodities(core, who) + granted // mutants:skip: Keleres is dropped
}

/// Whether the seat's box holds a piece of `unit` it may put on the board by an effect (`supply::allowed(.., 1) > 0`: a faction rule that
/// forbids the form places none, and an exhausted plastic limit too).
#[must_use]
pub fn box_has(core: &CoreState, k: &crate::kernel::Kernel, scope: crate::tables::Scope, who: Seat, unit: Unit) -> bool {
    !k.hooks.economy.effect_placement_forbidden(core, k, who, unit) && crate::query::fleet::remaining_all(core, scope, who, unit) > 0
}

/// `action_cards::place_units_counted`: put `count` units of `unit` at one spot, capped by the box, and return how many arrived. The spot
/// is not validated. A form the seat does not place (`None`) and a form a faction rule forbids place none.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "the position, the seat, the spot, the unit and how many")]
pub fn place_counted(core: &mut CoreState, k: &crate::kernel::Kernel, scope: crate::tables::Scope, who: Seat, system: System, planet: Option<Planet>, unit: Option<Unit>, count: usize) -> usize {
    let Some(unit) = unit else { return 0 };
    if k.hooks.economy.effect_placement_forbidden(core, k, who, unit) {
        return 0;
    }
    let room = usize::try_from(crate::query::fleet::remaining_all(core, scope, who, unit)).unwrap_or(0);
    let count = count.min(room);
    for _ in 0..count {
        place_unit(core, who, system, planet, unit);
    }
    count
}

/// `supply::spend_strategy_token_staged`: one token leaves the strategy pool and `STRATEGY_TOKEN_SPENT` waits for the game step (`reason` is
/// the interned text). Whether the pool held one.
pub fn spend_strategy_token_staged(s: &mut CoreState, who: Seat, reason: Code) -> bool {
    if !spend_strategic_token(s, who) {
        return false;
    }
    stage(s, event(EventKind::StrategyTokenSpent, Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Reason, Val::Code(reason))));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{FirstOption, Kernel};
    use crate::state::{Player, Players};
    use crate::generated::Faction;

    fn state() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s
    }

    #[test]
    fn trade_goods_gain_is_clamped_and_staged_in_order() {
        let mut s = state();
        assert_eq!(gain_trade_goods(&mut s, Seat(0), 3), 3);
        assert_eq!(gain_trade_goods(&mut s, Seat(0), -2), 0);
        assert_eq!(gain_trade_goods(&mut s, Seat(9), 2), 0);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = i16::MAX - 1;
        assert_eq!(gain_trade_goods(&mut s, Seat(0), 5), 1, "clamped, never wrapped");
        gain_trade_goods_staged(&mut s, Seat(1), 2, Code(1));
        gain_trade_goods_staged(&mut s, Seat(1), 0, Code(1));
        gain_trade_goods_staged(&mut s, Seat(1), 4, Code(2));
        let staged = drain_staged(&mut s);
        assert_eq!(staged.len(), 2);
        assert_eq!(staged[0].payload.int(Key::Amount), Some(2));
        assert_eq!(staged[1].payload.int(Key::Amount), Some(4));
        assert!(!has_staged(&s));
    }

    #[test]
    fn a_replenish_announces_the_cabal_note_and_pays_a_trade_agreement_in_the_string_order() {
        use crate::generated::PromissoryNote;
        use crate::state::NoteKey;
        let sol = Faction::ALL.into_iter().find(|f| f.data().commodities == 4).expect("a four-commodity faction");
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), sol), Player::new(Seat(1), Faction::TheVuilraithCabal), Player::new(Seat(2), Faction::ALL.into_iter().find(|f| *f != sol && *f != Faction::TheVuilraithCabal).unwrap())]);
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1), Seat(2)], &[Seat(0), Seat(1), Seat(2)], 3);
        let note = NoteKey { note: PromissoryNote::TradeAgreement, faction: sol };
        s.cards_mut().promissory.edit().insert(note, Seat(2));
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 1;
        assert_eq!(replenish_announced(&mut s, Seat(0), Code(5)), Some(3), "the commodities gained");
        let p = |seat: u8| s.players().by_seat(Seat(seat)).unwrap().econ.clone();
        assert_eq!(p(0).commodities, 0, "the Trade Agreement took them");
        assert_eq!(p(2).trade_goods, 4, "and the holder got them as trade goods");
        assert_eq!(s.cards().promissory.get(&note), Some(&Seat(0)), "the note went home");
        let staged = drain_staged(&mut s);
        let kinds: Vec<_> = staged.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![EventKind::CommoditiesReplenished, EventKind::TradeGoodsGained], "Cabal first, then the Trade Agreement");
        assert_eq!(staged[0].payload.int(Key::Commodities), Some(4), "the note carries the commodities held after the replenish");
        assert_eq!(staged[1].payload.seat(Key::Player), Some(Seat(2)));
        assert_eq!(staged[1].payload.int(Key::Amount), Some(4));
        // Without a Cabal and a note: the commodity write only, and nothing is staged.
        let mut t = CoreState::all_loaded();
        t.players = Players::from_vec(vec![Player::new(Seat(0), sol), Player::new(Seat(1), Faction::ALL.into_iter().find(|f| *f != sol && *f != Faction::TheVuilraithCabal).unwrap())]);
        assert_eq!(replenish_announced(&mut t, Seat(0), Code(5)), Some(4));
        assert!(!has_staged(&t));
        assert_eq!(t.players().by_seat(Seat(0)).unwrap().econ.commodities, 4);
        // Two seats of one faction: nothing is written.
        let mut u = CoreState::all_loaded();
        u.players = Players::from_vec(vec![Player::new(Seat(0), sol), Player::new(Seat(1), sol)]);
        assert_eq!(replenish_announced(&mut u, Seat(0), Code(5)), None);
        assert_eq!(u.players().by_seat(Seat(0)).unwrap().econ.commodities, 0);
    }

    #[test]
    fn units_move_in_and_out_keeping_order() {
        let mut s = state();
        let sys = System::ALL[10];
        place_unit(&mut s, Seat(0), sys, None, Unit::ALL[0]);
        place_unit(&mut s, Seat(1), sys, None, Unit::ALL[1]);
        place_unit(&mut s, Seat(0), sys, None, Unit::ALL[0]);
        assert!(remove_unit(&mut s, Seat(0), sys, None, Unit::ALL[0]));
        let left: Vec<_> = s.board().sys(sys).units.as_slice().iter().map(|p| (p.kind(), p.owner())).collect();
        assert_eq!(left, vec![(Unit::ALL[1], Seat(1)), (Unit::ALL[0], Seat(0))]);
        assert!(!remove_unit(&mut s, Seat(1), sys, None, Unit::ALL[0]));
    }

    #[test]
    fn control_runs_the_module_hooks_and_names_the_previous_owner() {
        let mut s = state();
        let mut k = Kernel::plain();
        k.hooks.general.control_gained.push(|cx, who, _, _| {
            cx.state.players_mut().by_seat_mut(who).unwrap().econ.commodities += 1;
        });
        let mut asker = FirstOption;
        let (sys, planet) = (System::ALL[10], Planet::ALL[3]);
        let mut cx = Cx::new(&mut s, &k, &mut asker);
        assert!(gain_control(&mut cx, Seat(0), sys, planet).unwrap().payload.seat(Key::PreviousOwner).is_none());
        assert!(gain_control(&mut cx, Seat(0), sys, planet).is_none());
        let ev = gain_control(&mut cx, Seat(1), sys, planet).unwrap();
        assert_eq!(ev.payload.seat(Key::PreviousOwner), Some(Seat(0)));
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.commodities, 1);
        assert_eq!(s.board().planet(planet).control, Some(Seat(1)));
    }

    #[test]
    fn tokens_planets_and_points() {
        let mut s = state();
        let sys = System::ALL[4];
        assert!(place_command_token(&mut s, Seat(1), sys));
        assert!(!place_command_token(&mut s, Seat(1), sys));
        let planet = Planet::ALL[2];
        assert!(exhaust_planet(&mut s, planet) && !exhaust_planet(&mut s, planet));
        assert!(ready_planet(&mut s, planet) && !ready_planet(&mut s, planet));
        assert_eq!(gain_victory_points(&mut s, Seat(0), 2, LedgerReason::Custodians), 2);
        assert_eq!(gain_victory_points(&mut s, Seat(0), -1, LedgerReason::ImperialPrimary), 1);
        assert_eq!(s.ledger().vp.len(), 2);
    }
}

/// `laws::repeal`: the law leaves play; Political Censure's holder loses its point. Whether the law was in play. The body
/// of `Resolve::repeal` (agenda area) and of the Repeal Law card (action-card area), which may not import each other.
pub fn repeal_law(s: &mut CoreState, law: crate::generated::Agenda) -> bool {
    use crate::keys::Outcome;
    let owner = s.agenda_mut().laws.remove(&law);
    if let (crate::generated::Agenda::PoliticalCensure, Some(Outcome::Seat(holder))) = (law, owner) {
        adjust_victory_points(s, holder, -1, LedgerReason::Agenda(law));
    }
    owner.is_some()
}

/// `GameState::deal_strategy_card`: the card joins the holding in initiative order. `false`: no such seat. The body of
/// `strategy_flow::deal` (strategy area), repeated for the cards that redeal (Public Disgrace).
pub fn deal_strategy_card(s: &mut CoreState, who: Seat, card: crate::generated::StrategyCard) -> bool {
    let Some(p) = s.players().by_seat(who) else { return false };
    let mut held: Vec<crate::generated::StrategyCard> = p.strat.as_slice().to_vec();
    held.push(card);
    let init = &s.strategy().initiative;
    held.sort_by_key(|c| init.get(c).copied().unwrap_or(99));
    if let Some(p) = s.players_mut().by_seat_mut(who) {
        p.strat = held.into_iter().collect();
    }
    true
}

/// `supply::capture_from_board`: take `piece` off `system` (its space area, or `planet`) and onto `captor`'s
/// sheet. `false`, nothing changed, when the unit is not there, when a seat is missing or when they are the same
/// player.
pub fn capture_from_board(s: &mut CoreState, captor: Seat, system: System, planet: Option<Planet>, piece: Piece) -> bool {
    let owner = piece.owner();
    if captor == owner || s.players().by_seat(captor).is_none() || s.players().by_seat(owner).is_none() {
        return false;
    }
    let list = match planet {
        Some(p) => &mut s.board_mut().planet_mut(p).units,
        None => &mut s.board_mut().sys_mut(system).units,
    };
    let Some(at) = list.as_slice().iter().position(|found| *found == piece) else { return false };
    list.remove(at);
    if let Some(p) = s.players_mut().by_seat_mut(captor) {
        p.misc.captured_units.push((owner, piece.kind()));
    }
    true
}
