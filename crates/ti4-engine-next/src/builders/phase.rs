//! Moving a built position to a later phase (legacy `phase::{advance_phase, begin_action_turn,
//! stock_unclaimed_cards}`, the string bodies): a scenario deals the strategy cards and starts the
//! action phase.

use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, Phase};

/// What [`advance_phase`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseOutcome {
    /// The action phase began; this player has the first turn.
    ActionBegan(PlayerId),
    /// The status phase began.
    StatusBegan,
    /// The agenda phase began.
    AgendaBegan,
    /// The round ended; the caller must supply the next round's strategy cards.
    RoundEnded,
}

/// Each unclaimed strategy card collects a trade good, so a card nobody picked pays whoever takes it.
pub fn stock_unclaimed_cards(state: &mut GameState) {
    for card in state.unclaimed_strategy_cards.clone() {
        *state.strategy_card_goods.entry(card).or_insert(0) += 1;
    }
}

/// Start one action-phase turn: the next turn of the phase, for `player`.
pub fn begin_action_turn(state: &mut GameState, player: &PlayerId) {
    state.action_turn += 1;
    state.active = Some(player.clone());
    state.turn_seq += 1;
}

/// Naalu, Telepathic: "Place the Naalu '0' token on your strategy card; you are first in
/// initiative order", unless Gift of Prescience was played this round.
fn naalu_strategy_phase_ended(state: &mut GameState) {
    if state.faction_marks.get("naalu:gift_round").map(String::as_str)
        == Some(state.round.to_string().as_str())
    {
        return;
    }
    let naalu: Vec<PlayerId> = state
        .players
        .iter()
        .filter(|seat| seat.faction.as_str() == "naalu")
        .map(|seat| seat.id.clone())
        .collect();
    for player in naalu {
        if state.player(&player).is_some() {
            state.initiative_overrides.insert(player, 0);
        }
    }
}

/// Move the game to its next phase (`phase::advance_phase`).
pub fn advance_phase(state: &mut GameState) -> PhaseOutcome {
    match state.phase {
        Phase::Strategy => {
            stock_unclaimed_cards(state);
            // Political Stability is spent here.
            for player in &mut state.players {
                player.stability = false;
            }
            naalu_strategy_phase_ended(state);
            let first = state.initiative_order().first().cloned();
            state.phase = Phase::Action;
            state.action_turn = 0;
            state.active = None;
            let first = first.unwrap_or_else(|| PlayerId::new(""));
            begin_action_turn(state, &first);
            PhaseOutcome::ActionBegan(first)
        }
        Phase::Action => {
            state.phase = Phase::Status;
            state.active = None;
            PhaseOutcome::StatusBegan
        }
        Phase::Status => {
            // LRR 8.1, 27.4: the agenda phase exists only once the custodians token has been
            // lifted, and then it runs every round.
            if state.custodians_removed && !state.finished {
                state.phase = Phase::Agenda;
                PhaseOutcome::AgendaBegan
            } else {
                PhaseOutcome::RoundEnded
            }
        }
        Phase::Agenda => PhaseOutcome::RoundEnded,
    }
}
