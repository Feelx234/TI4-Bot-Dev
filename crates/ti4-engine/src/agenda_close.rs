//! The close of a vote over a context (package B9c-2): `Game::close_vote` / `settle_winner` /
//! `resolve_agenda_outcome` up to the agenda's own effect, split out of `Game` like the reveal chain
//! ([`crate::agenda_reveal::Live`]) so that a window dual can lend the string closure everything they touch besides
//! the state. The bodies are the former methods of `Game` unchanged. The core half is
//! `ti4_engine_core::rules::agenda::close::Close` (`agenda_close_dual`); the agenda's effect is its own window
//! (`agenda_effects::resolve_with`) and runs after this one, then the feat and the agenda-timed secret window.

use std::collections::BTreeMap;

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::agenda_reveal::{push_agenda_record, Live};
use crate::game::GameError;
use crate::vote::{is_law, VoteWindow, AGAINST};

/// What the close of a vote leaves for what follows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settled {
    /// The vote had a winner and the emission of `AGENDA_RESOLVED` went through.
    pub resolved: bool,
    /// Deadly Plot discarded the agenda: no effect, no feat for an election.
    pub discarded: bool,
    /// The outcome the effect resolves with (the redirect applied); `None` when nothing resolves.
    pub effective: Option<String>,
    /// The seat the outcome elected (a player, or the controller of the planet it names).
    pub elected: Option<String>,
}

/// The seat the outcome names if it is a player, else the controller of the planet it names (the "you or a planet
/// you control are elected" reading). An outcome that is neither -- a law, "for", "against" -- matches nobody.
#[must_use]
pub fn elected_seat_or_planet(state: &GameState, outcome: &str) -> Option<PlayerId> {
    let elected = PlayerId::new(outcome.to_owned());
    if state.player(&elected).is_some() {
        return Some(elected);
    }
    let planet = ti4_model::id::PlanetId::new(outcome.to_owned());
    state.board.values().find_map(|board| board.planet_control.get(&planet).cloned())
}

impl Live<'_> {
    /// The public record of a finished vote, for viewers only (no rule reads it). Written once the outcome and a
    /// Deadly Plot discard are settled, before the agenda's effect is asked.
    fn log_agenda(&mut self, state: &mut GameState, window: &VoteWindow, outcome: Option<&str>, discarded: bool) {
        let ballot = window.ballot();
        let speaker_decided = outcome.is_some()
            && crate::vote::undisputed_winner(state, window.alias(), ballot, window.choices()).is_none();
        push_agenda_record(state, self.content, window.alias(), outcome, ballot.counts.clone(), speaker_decided, discarded);
    }

    /// The close of `window`: with no winner the vote is logged and the predictions dropped; with one the outcome is
    /// announced (`AGENDA_RESOLVED`, typed), recorded, and resolved unless Deadly Plot discarded the agenda
    /// (redirect, predictions, law). `covert` is the face-down agenda Covert Legislation decides.
    ///
    /// # Errors
    /// [`GameError`] when the typed emission fails (an illegal decider answer).
    pub fn close_resolution(
        &mut self,
        state: &mut GameState,
        window: &VoteWindow,
        covert: Option<&str>,
    ) -> Result<Settled, GameError> {
        let alias = window.alias().to_owned();
        let Some(outcome) = window.winner() else {
            self.log_agenda(state, window, None, false);
            // No outcome, so nobody predicted correctly -- but the cards were still spent, and a
            // prediction left behind would pay out on the next agenda.
            state.agenda_predictions.clear();
            return Ok(Settled::default());
        };
        let outcome = outcome.to_owned();
        self.emit(&format!("AGENDA_RESOLVED:{alias}:{outcome}"));
        // The outcome names a player when the agenda elects one, which is what the "when
        // you are elected" windows read. `elected_player` is set only for a real seat, so
        // a window can tell "a player was elected" from an outcome that is a law, a planet,
        // or For/Against.
        let outcome_is_player = state.player(&PlayerId::new(outcome.clone())).is_some();
        let mut payload = BTreeMap::new();
        payload.insert("agenda".to_owned(), serde_json::Value::String(alias.clone()));
        payload.insert("player".to_owned(), serde_json::Value::String(outcome.clone()));
        if outcome_is_player {
            payload.insert("elected_player".to_owned(), serde_json::Value::String(outcome.clone()));
        }
        // Mirror the ballot for a Deadly Plot guard played into the window below: the
        // ballot itself lives in the vote window this function holds, and a guard can
        // only read the game state.
        state.agenda_votes = window.ballot().votes.clone();
        crate::diplomacy::evaluate_event(
            state,
            &crate::diplomacy::DiplomacyEventContext::VotesRecorded { agenda: alias.clone() },
        )
        .expect("validated vote promises settle deterministically");
        self.emit_typed(state, "AGENDA_RESOLVED", payload)?;
        state.agenda_votes.clear();
        let discarded = state.transient_flags.has(ti4_model::state::TransientFlags::AGENDA_DISCARDED);
        state.transient_flags.clear(ti4_model::state::TransientFlags::AGENDA_DISCARDED);

        self.log_agenda(state, window, Some(&outcome), discarded);
        if let Some(hidden) = covert {
            // The face-down agenda is turned up and listed with the outcome it was given.
            self.emit(&format!("COVERT_AGENDA_RESOLVED:{hidden}:{outcome}"));
            let ballot = window.ballot();
            let speaker_decided =
                crate::vote::undisputed_winner(state, window.alias(), ballot, window.choices()).is_none();
            push_agenda_record(state, self.content, hidden, Some(&outcome), ballot.counts.clone(), speaker_decided, discarded);
        }
        let resolved = covert.unwrap_or(&alias).to_owned();
        if discarded {
            // Deadly Plot: "discard the agenda instead. The agenda is resolved with no
            // effect and it is not replaced." The vote still happened, so the
            // occurrence window below still opens -- but the agenda's own effect, the
            // prediction payouts, any law, and the elected feat are all suppressed. A
            // redirect played into the same window (Confusing / Confounding) is spent
            // on an election that has no effect, so it is dropped with it. (This
            // engine does not draw a replacement after a resolution, so "not
            // replaced" has nothing to suppress.)
            state.agenda_elected_override = None;
            state.agenda_predictions.clear();
            self.emit(&format!("AGENDA_DISCARDED:{alias}"));
            return Ok(Settled { resolved: true, discarded: true, effective: None, elected: None });
        }
        let (effective, elected) = self.resolve_agenda_outcome(state, &resolved, &outcome);
        Ok(Settled { resolved: true, discarded: false, effective: Some(effective), elected: elected.map(|p| p.to_string()) })
    }

    /// Resolve an agenda that was not discarded, up to its effect: the redirect, the prediction payouts and
    /// enacting a law. Returns the effective outcome and the elected player if there was one.
    pub(crate) fn resolve_agenda_outcome(
        &mut self,
        state: &mut GameState,
        alias: &str,
        outcome: &str,
    ) -> (String, Option<PlayerId>) {
        let effective = self.redirect_elected(state, outcome);
        // Drive the Debate: "you or a planet you control are elected". The outcome
        // names a player, a planet, or an outcome like "for" -- so both readings
        // have to be tried, and an outcome that is neither matches nobody. Read
        // from the (possibly redirected) elected player.
        let elected = elected_seat_or_planet(state, &effective);
        self.pay_predictions(state, outcome);
        self.enact_if_law(state, alias, outcome);
        (effective, elected)
    }

    /// Confusing / Confounding Legal Text, played into the window above, redirect
    /// who is the elected player. The vote's own result (`outcome`) still settles
    /// predictions and any law; the agenda's effect on the elected player and the
    /// "elected by an agenda" feat follow the redirect. Returns the effective outcome.
    fn redirect_elected(&mut self, state: &mut GameState, outcome: &str) -> String {
        if let Some(override_player) = state.agenda_elected_override.take() {
            self.emit(&format!("AGENDA_OUTCOME_REDIRECTED:{outcome}:{override_player}"));
            override_player.to_string()
        } else {
            outcome.to_owned()
        }
    }

    /// Imperial Rider pays out before the agenda's own effect, and clears the
    /// predictions. A prediction left behind would pay again on the next agenda,
    /// for a card that was spent on this one.
    fn pay_predictions(&mut self, state: &mut GameState, outcome: &str) {
        for player in crate::action_cards::resolve_predictions_asking(
            state,
            self.content,
            self.sources,
            self.galaxy,
            self.table,
            outcome,
        ) {
            self.emit(&format!("AGENDA_PREDICTION_CORRECT:{player}"));
        }
    }

    /// 8.20 first: an elected or "For" law stays in play, and an effect that reads
    /// the laws must see this one already there. 8.21 discards everything else.
    fn enact_if_law(&mut self, state: &mut GameState, alias: &str, outcome: &str) {
        if is_law(self.content, alias) && outcome != AGAINST {
            state.enact_law(alias, outcome);
            // Classified Document Leaks: "The elected secret objective becomes a public
            // objective." Only for its own enactment -- `outcome` here is whatever *this* agenda
            // elected, so calling it for every law would publish a player name or a planet as an
            // objective the moment Classified was anywhere in play.
            if alias == "classified" {
                crate::laws::classified_leak(state, outcome);
            }
            self.emit(&format!("LAW_ENACTED:{alias}:{outcome}"));
        }
    }

    /// Committee Formation's election, after the owner discarded the card: the owner names the player the agenda
    /// elects, the agenda is listed as resolved with that outcome (no votes) and a law is enacted. `None` when nobody
    /// could be asked or the question was not answered.
    pub fn committee_elect(&mut self, state: &mut GameState, alias: &str, owner: &PlayerId) -> Option<String> {
        let options: Vec<crate::choice::ChoiceOption> = state
            .players
            .iter()
            .map(|seat| crate::choice::ChoiceOption::labelled(seat.id.to_string(), "player", format!("elect {}", seat.id)))
            .collect();
        if options.is_empty() {
            return None;
        }
        let choice = crate::choice::Choice::new(owner.clone(), format!("{alias}: elect which player"), options);
        let chosen = self
            .table
            .ask_seeing(&choice, &crate::choice::Observed::new(state, self.content, self.sources, self.galaxy))
            .ok()
            .map(|answer| answer.id)?;
        self.emit(&format!("AGENDA_RESOLVED:{alias}:{chosen}"));
        push_agenda_record(state, self.content, alias, Some(&chosen), BTreeMap::new(), false, false);
        if is_law(self.content, alias) {
            state.enact_law(alias, &chosen);
            self.emit(&format!("LAW_ENACTED:{alias}:{chosen}"));
        }
        Some(chosen)
    }
}
