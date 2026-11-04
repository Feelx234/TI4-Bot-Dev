//! The forecasts of the bot as bit patterns, for the differential tests of the views (docs/todo/D3-typed-bot.md section 3.3:
//! "float forecasts: bit-identical, same expression order").
//!
//! A pick that does not flip hides a one-ulp difference of a score, so the decisions alone do not prove the floating-point
//! paths. [`forecast_bits`] evaluates the public float-valued entry points of the bot on one position, through any
//! [`SeatView`], and returns `(name, f64::to_bits)` rows; two views of the same position must return the same rows.

use ti4_model::content_types::ContentType;
use ti4_engine::production::Spend;
use ti4_model::id::{PlayerId, SystemId};
use ti4_rule_bot::view::SeatView;

/// The forecasts of the seat `me` on this position: the value of every outcome of every agenda in play-able content
/// (`agenda_vote::evaluate` over "for", "against" and each player), the activation value of every system on the board
/// (`activation_value`), and the objective goals (`objectives::goals`: points, progress, threshold).
#[must_use]
pub fn forecast_bits(view: &dyn SeatView, me: &PlayerId) -> Vec<(String, u64)> {
    let mut rows = Vec::new();
    let players: Vec<String> = view.players().iter().map(|p| p.as_str().to_owned()).collect();
    let mut outcomes: Vec<&str> = vec!["for", "against"];
    outcomes.extend(players.iter().map(String::as_str));
    for record in view.content().records(ContentType::Agendas) {
        let Some(agenda) = record.id() else { continue };
        if let Some(verdict) = ti4_rule_bot::agenda_vote::evaluate(agenda, &outcomes, view, me) {
            for (outcome, score) in &verdict.scores {
                rows.push((format!("agenda {agenda} {outcome}"), score.to_bits()));
            }
        }
    }
    // The string board keeps the entry of a system that has emptied (units left, token removed); the typed board has none for it.
    // An empty entry and no entry answer alike everywhere the bot reads (`system()` is empty for an absent system), so the systems
    // asked about are the ones with something on them.
    let systems: Vec<SystemId> = view.board().iter().filter(|(_, s)| !is_empty(s)).map(|(id, _)| id.clone()).collect();
    for system in &systems {
        rows.push((format!("activation_value {system}"), ti4_rule_bot::activation_value(view, me, system).to_bits()));
    }
    for goal in ti4_rule_bot::objectives::goals(view, me, &ti4_engine::objectives::Imagined::NONE) {
        let key = format!("goal {} {}", goal.alias, goal.secret);
        rows.push((format!("{key} points"), goal.points.to_bits()));
        rows.push((format!("{key} have"), goal.have.to_bits()));
        rows.push((format!("{key} threshold"), goal.threshold.to_bits()));
        rows.push((format!("{key} satisfied"), u64::from(goal.satisfied)));
    }
    rows.extend(accessor_rows(view, me, &systems));
    rows
}

/// FNV-1a of a debug text: the accessor answers that are not floats, as one 64-bit row each.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// The accessors the decisions may not reach on a given position (rifts, laws, planet cards, seat sheets, ...), one row per
/// question: the same questions, asked of any view, must give the same answer.
fn accessor_rows(view: &dyn SeatView, me: &PlayerId, systems: &[SystemId]) -> Vec<(String, u64)> {
    let mut rows = Vec::new();
    let mut put = |what: String, value: String| rows.push((what, fnv(&value)));
    put("round/phase".to_owned(), format!("{} {:?} {:?} {} {}", view.round(), view.phase(), view.active_player(), view.speaker(), view.custodians_removed()));
    put("laws".to_owned(), format!("{:?}", view.laws()));
    put("agenda".to_owned(), format!("{:?} {} {:?}", view.agenda_in_play(), view.agendas_resolved_this_round(), view.invasion()));
    put("space_battle".to_owned(), format!("{:?}", view.space_battle()));
    put("promissory_notes".to_owned(), format!("{:?}", view.promissory_notes()));
    put("hand".to_owned(), format!("{:?} {:?}", view.held_secrets(), view.held_action_cards()));
    put("active_system".to_owned(), format!("{:?}", view.active_system()));
    // A planet with no units is a present key with an empty list on the string side and no key on the typed side: the same
    // position (`SystemState`'s equality treats them alike), so the text leaves the empty lists out.
    for (id, state) in view.board().iter().filter(|(_, s)| !is_empty(s)) {
        put(format!("board {id}"), system_text(state));
    }
    let players: Vec<PlayerId> = view.players().into_iter().cloned().collect();
    put("players".to_owned(), format!("{players:?}"));
    for player in &players {
        put(format!("seat {player}"), format!("{:?}", view.seat(player)));
        put(format!("agenda_prediction {player}"), format!("{}", view.has_agenda_prediction(player)));
        put(format!("scored_by {player}"), format!("{:?}", view.scored_by(player)));
        put(format!("scoreable_public {player}"), format!("{}", view.scoreable_public(player)));
        put(format!("controlled {player}"), format!("{:?}", view.controlled_planets(player)));
        put(format!("tokens {player}"), format!("{:?}", view.systems_with_token(player)));
        for kind in [Spend::Resources, Spend::Influence] {
            put(format!("spend {player} {kind:?}"), format!("{}", view.available_spend(player, kind)));
        }
    }
    for (_, controlled) in players.iter().map(|p| (p, view.controlled_planets(p))) {
        for (_, planet) in controlled {
            put(format!("ready {planet}"), format!("{}", view.planet_is_ready(planet)));
        }
    }
    for system in systems {
        put(format!("implicating {system}"), format!("{:?}", view.objectives_implicating_system(me, system)));
        put(format!("system {system}"), system_text(&view.system(system)));
        put(format!("loadable {system}"), format!("{:?}", view.loadable(me, system)));
        let movable = view.movable_into(me, system);
        put(format!("movable {system}"), format!("{movable:?}"));
        for m in &movable {
            for boosts in [(false, false), (m.gravity_drive, m.ionian)] {
                put(format!("rift {} -> {system} {boosts:?}", m.origin), format!("{}", view.rift_exits_into(me, &m.origin, m.unit.type_id.as_str(), boosts, system)));
            }
        }
        for origin in systems.iter().take(6) {
            for value in 1..=3 {
                put(format!("can_reach {origin} -> {system} {value}"), format!("{}", view.can_reach(me, origin, system, value)));
            }
        }
    }
    rows
}

fn system_text(state: &ti4_model::state::SystemState) -> String {
    let planet_units: Vec<_> = state.planet_units.iter().filter(|(_, units)| !units.is_empty()).collect();
    format!(
        "{:?} {:?} {:?} {planet_units:?} {:?} {:?}",
        state.units, state.command_tokens, state.planet_control, state.coexisting, state.purged_planets
    )
}

fn is_empty(state: &ti4_model::state::SystemState) -> bool {
    state.units.is_empty()
        && state.command_tokens.is_empty()
        && state.planet_control.is_empty()
        && state.planet_units.values().all(Vec::is_empty)
        && state.coexisting.is_empty()
        && state.purged_planets.is_empty()
}
