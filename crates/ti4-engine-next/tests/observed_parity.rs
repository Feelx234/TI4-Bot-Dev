//! `Observed` (docs/todo/D2-8-wrapper.md 8b3): the thin copy answers every accessor as the legacy `choice::Observed` does, on the same
//! positions: the start of a game and positions played forward by seeded random deciders (so the board, hands, laws and objectives have
//! content), for every seat and, where an accessor takes a system or planet, for every one on the board.

use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::content_types::POK;
use ti4_model::state::GameState;

mod surface_common;

/// Positions of seated games on real maps (lane B: the native queries answer for what the core can hold, and a table with no faction
/// seated is not a game; the unseated positions of lane A answered through the string fallback of the legacy view), then the real-faction fixture.
fn positions() -> Vec<GameState> {
    let mut out: Vec<GameState> = surface_common::positions().into_iter().map(|p| p.state).collect();
    out.push(old::fixtures::real_game(&["a", "b", "c"]));
    out
}

/// `assert_eq!` of the same call on both views, with what it was.
macro_rules! same {
    ($old:expr, $new:expr, $what:expr) => {
        assert_eq!($old, $new, "{}", $what)
    };
}

#[test]
fn every_accessor_answers_as_the_legacy_one_does() {
    let content = ti4_content::ContentStore::embedded();
    let mut compared = 0_usize;
    for (n, state) in positions().iter().enumerate() {
        let (o, t) = (old::choice::Observed::new(state, content, POK, None), thin::observed::Observed::new(state, content, POK, None));
        let at = |what: &str| format!("position {n}: {what}");
        same!(o.space_battle(), t.space_battle(), at("space_battle"));
        same!(format!("{:?}", o.invasion()), format!("{:?}", t.invasion()), at("invasion"));
        same!(o.round(), t.round(), at("round"));
        same!(o.phase(), t.phase(), at("phase"));
        same!(o.active_player(), t.active_player(), at("active_player"));
        same!(o.speaker(), t.speaker(), at("speaker"));
        same!(o.initiative_order(), t.initiative_order(), at("initiative_order"));
        same!(o.pending_step(), t.pending_step(), at("pending_step"));
        same!(o.custodians_removed(), t.custodians_removed(), at("custodians_removed"));
        same!(o.laws(), t.laws(), at("laws"));
        same!(o.board().len(), t.board().len(), at("board"));
        same!(o.active_system(), t.active_system(), at("active_system"));
        same!(o.agenda_in_play(), t.agenda_in_play(), at("agenda_in_play"));
        same!(o.agendas_resolved_this_round(), t.agendas_resolved_this_round(), at("agendas_resolved_this_round"));
        same!(o.players(), t.players(), at("players"));
        same!(o.revealed_objectives(), t.revealed_objectives(), at("revealed_objectives"));
        same!(o.promissory_notes(), t.promissory_notes(), at("promissory_notes"));
        same!(o.support_holders(), t.support_holders(), at("support_holders"));
        same!(o.public_diplomacy_deals().count(), t.public_diplomacy_deals().count(), at("public_diplomacy_deals"));
        same!(o.public_diplomacy_signals().count(), t.public_diplomacy_signals().count(), at("public_diplomacy_signals"));
        for system in o.board().keys() {
            same!(o.system(system), t.system(system), at("system"));
            same!(o.is_fracture_system(system), t.is_fracture_system(system), at("is_fracture_system"));
            same!(o.active_breach_in(system), t.active_breach_in(system), at("active_breach_in"));
            compared += 1;
        }
        for player in state.players.iter().map(|p| &p.id) {
            let who = |what: &str| format!("position {n}, {player}: {what}");
            same!(format!("{:?}", o.seat(player)), format!("{:?}", t.seat(player)), who("seat"));
            same!(o.controlled_planets(player), t.controlled_planets(player), who("controlled_planets"));
            same!(o.systems_with_units_of(player), t.systems_with_units_of(player), who("systems_with_units_of"));
            same!(o.systems_with_token(player), t.systems_with_token(player), who("systems_with_token"));
            same!(o.opponent_slots(player), t.opponent_slots(player), who("opponent_slots"));
            same!(o.scored_by(player), t.scored_by(player), who("scored_by"));
            same!(o.units_held(player), t.units_held(player), who("units_held"));
            same!(o.fleet_value_permille(player), t.fleet_value_permille(player), who("fleet_value_permille"));
            same!(o.production_system_count(player), t.production_system_count(player), who("production_system_count"));
            same!(o.agenda_prediction_of(player), t.agenda_prediction_of(player), who("agenda_prediction_of"));
            same!(o.available_spend(player, old::production::Spend::Resources), t.available_spend(player, thin::queries::production::Spend::Resources), who("available_spend"));
            same!(o.fleet_supply_limit(player), t.fleet_supply_limit(player), who("fleet_supply_limit (delegate)"));
            same!(o.scoreable_public(player), t.scoreable_public(player), who("scoreable_public (delegate)"));
            same!(o.scoreable_secret(player), t.scoreable_secret(player), who("scoreable_secret (delegate)"));
            for other in state.players.iter().map(|p| &p.id) {
                same!(format!("{:?}", o.opponent_relationship(player, other)), format!("{:?}", t.opponent_relationship(player, other)), who("opponent_relationship"));
                same!(o.recent_diplomacy_attack(player, other), t.recent_diplomacy_attack(player, other), who("recent_diplomacy_attack"));
            }
            compared += 1;
        }
        for card in state.players.iter().flat_map(|p| p.strategy_cards.iter()) {
            same!(o.card_initiative(card), t.card_initiative(card), at("card_initiative"));
            same!(o.strategy_card_goods(card), t.strategy_card_goods(card), at("strategy_card_goods"));
        }
    }
    assert!(compared >= 20, "only {compared} comparisons");
}
