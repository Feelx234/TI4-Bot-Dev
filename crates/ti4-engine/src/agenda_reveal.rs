//! The agenda reveal chain over a context (package B9c-1): `Game::reveal_agenda` and what it calls, split out of
//! `Game` so that a window dual can lend the string closure everything the chain touches besides the state.
//!
//! [`Live`] holds the fields of `Game` that the chain reads and writes (the table, the timing resolver, the dice,
//! the rng, the event allocator, the plain event list, the map), borrowed field by field from `Game::split`, so the
//! closure of `dual_window!` can own them while `&mut GameState` is passed beside them. The bodies are the former
//! methods of `Game` unchanged; the core half is `ti4_engine_core::rules::agenda::reveal::Reveal`
//! (`agenda_reveal_dual`).

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::{Resolving as _, Table};
use crate::dice::Dice;
use crate::event::EventSequence;
use crate::game::GameError;
use crate::rng::GameRng;
use crate::timing::{Resolver, TimingContext};
use crate::vote::{is_law, outcomes};

/// The fields of `Game` besides the state (see the module docs).
pub struct Live<'a> {
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub galaxy: Option<&'a Galaxy>,
    pub table: &'a mut Table,
    pub timing: &'a mut Resolver,
    pub dice: &'a mut Dice,
    pub rng: &'a mut GameRng,
    pub event_sequence: &'a mut EventSequence,
    pub events: &'a mut Vec<String>,
}

/// Copy the event labels the timing resolver logged since `from` into the plain event list.
pub fn mirror_timing_log(timing: &Resolver, events: &mut Vec<String>, from: usize) {
    let emitted: Vec<String> = timing
        .log()
        .iter()
        .skip(from)
        .filter_map(|line| line.strip_prefix("emit "))
        .map(|line| line.split('#').next().unwrap_or(line).to_owned())
        .collect();
    crate::dual_window::note_mirrored_events(&emitted);
    events.extend(emitted);
}

/// `Game::push_agenda_record` over the state and the content.
pub fn push_agenda_record(
    state: &mut GameState,
    content: &ContentStore,
    alias: &str,
    outcome: Option<&str>,
    votes: BTreeMap<String, i64>,
    speaker_decided: bool,
    discarded: bool,
) {
    let law = is_law(content, alias);
    let record = ti4_model::state::AgendaRecord {
        round: state.round,
        agenda: alias.to_owned(),
        name: content
            .get(ti4_model::content_types::ContentType::Agendas, alias)
            .and_then(|record| record.text("name"))
            .unwrap_or_default()
            .to_owned(),
        outcome: outcome.map(str::to_owned),
        votes,
        speaker_decided,
        law,
        enacted: law && !discarded && outcome.is_some_and(|o| o != crate::vote::AGAINST),
        discarded,
        redirected_to: if discarded {
            None
        } else {
            state
                .agenda_elected_override
                .as_ref()
                .map(ToString::to_string)
                .filter(|to| Some(to.as_str()) != outcome)
        },
        discard_reason: None,
    };
    state.agenda_log.push(record);
}

/// Notes of `alias` held by someone other than their owner: `(note, holder, owner)`.
#[must_use]
pub fn lent_notes(state: &GameState, alias: &str) -> Vec<(String, PlayerId, PlayerId)> {
    state
        .promissory_notes
        .iter()
        .filter(|(note, _)| crate::promissory::alias_of(note) == alias)
        .filter_map(|(note, holder)| {
            let name = crate::promissory::owner_of(note)?;
            // By faction, as in `promissory::holder_of`: seats sharing a faction share the
            // note id, and a seat comparison would read a player's own card as lent out.
            if crate::promissory::faction_name(state, holder) == name {
                return None;
            }
            let owner = crate::promissory::seat_of(state, &name)?;
            Some((note.clone(), holder.clone(), owner))
        })
        .collect()
}

impl Live<'_> {
    pub fn emit(&mut self, event: &str) {
        crate::dual_window::note_plain_event(event);
        self.events.push(event.to_owned());
    }

    /// `Game::emit_typed`: emit a typed event through the resolver, opening its WHEN and AFTER windows.
    ///
    /// # Errors
    /// [`GameError`] when the event id space is exhausted or a decider answers illegally.
    pub fn emit_typed(
        &mut self,
        state: &mut GameState,
        event_type: &str,
        payload: BTreeMap<String, serde_json::Value>,
    ) -> Result<bool, GameError> {
        let event = self.event_sequence.next(event_type, payload)?;
        let logged = self.timing.log().len();
        let emitted = {
            let mut context = TimingContext {
                state: &mut *state,
                content: self.content,
                sources: self.sources,
                table: &mut *self.table,
                dice: &mut *self.dice,
                rng: &mut *self.rng,
                event_sequence: &mut *self.event_sequence,
                galaxy: self.galaxy,
            };
            self.timing.emit_with_context(&mut context, event, |_, _| {})?
        };
        mirror_timing_log(self.timing, self.events, logged);
        // A faction effect resolved in that window may have staged card moves (a Scheming discard,
        // a card taken); announce them now rather than at the next component action.
        self.announce_staged_cards(state)?;
        Ok(!emitted.cancelled)
    }

    /// `Game::announce_staged_cards`.
    ///
    /// # Errors
    /// [`GameError`] when a window cannot be resolved.
    pub fn announce_staged_cards(&mut self, state: &mut GameState) -> Result<(), GameError> {
        if !crate::factions::hooks_cards::has_staged(state) {
            return Ok(());
        }
        let logged = self.timing.log().len();
        {
            let mut context = TimingContext {
                state: &mut *state,
                content: self.content,
                sources: self.sources,
                table: &mut *self.table,
                dice: &mut *self.dice,
                rng: &mut *self.rng,
                event_sequence: &mut *self.event_sequence,
                galaxy: self.galaxy,
            };
            crate::reactions::announce_staged_card_events(&mut context, self.timing)?;
        }
        mirror_timing_log(self.timing, self.events, logged);
        Ok(())
    }

    /// Reveal an agenda and follow the Veto replacement chain it triggers. Returns the agenda
    /// to vote on and its choices, or `None` when the reveal (and every replacement) elected
    /// nothing.
    ///
    /// The initial agenda is the one drawn from this phase's queue; each Veto played into a
    /// reveal window draws its replacement from the top of the agenda deck and discards the
    /// agenda it interrupted, and the chain continues from there. A Veto on a Veto is legal —
    /// an agenda revealed by a Veto is still "an agenda revealed" — so the chain is a loop,
    /// bounded by the finite deck.
    ///
    /// # Errors
    /// [`GameError`] when a decider answers illegally or has no answer yet.
    pub fn reveal_agenda(
        &mut self,
        state: &mut GameState,
        initial_alias: &str,
    ) -> Result<Option<(String, Vec<String>)>, GameError> {
        let mut alias = initial_alias.to_owned();
        loop {
            // Cards scoped to "this agenda" hang off this counter, so it moves before the reveal
            // window opens and any of them can be played.
            state.agenda_seq = state.agenda_seq.saturating_add(1);
            self.emit(&format!("AGENDA_REVEALED:{alias}"));
            let choices = self.reveal_choices(state, &alias);
            if choices.is_empty() {
                self.emit(&format!("AGENDA_DISCARDED:{alias}"));
                if state.rules_version < ti4_model::state::RULES_AGENDA_GRANT_REVEAL {
                    return Ok(None);
                }
                // "If there are no scored secret objectives / laws in play, discard this card and
                // reveal another agenda from the top of the deck": the replacement takes the slot.
                self.note_discarded_agenda(state, &alias, false);
                let Some(next) = ti4_model::deck_reserve::take_top(
                    &mut state.deck_reserve,
                    "agenda",
                    &mut state.agenda_deck,
                    None,
                ) else {
                    return Ok(None);
                };
                alias = next;
                continue;
            }

            // The outcomes have to be on the state before the window opens, because a card
            // played into it predicts one of them. Nineteen action cards read "when" or "after
            // an agenda is revealed", and this is the event they hang off.
            state.agenda_choices.clone_from(&choices);
            state.agenda_in_play = Some(alias.clone());
            let mut payload = BTreeMap::new();
            payload.insert(
                "agenda".to_owned(),
                serde_json::Value::String(alias.clone()),
            );
            self.emit_typed(state, "AGENDA_REVEALED", payload)?;
            self.offer_replacements(state, &alias)?;

            // Veto, played into the window, discards this agenda and reveals the one it drew
            // from the top of the deck: continue the chain from the replacement.
            let Some(replacement) = state.agenda_veto_replacement.take() else {
                // This is the agenda players will vote on: Political Secret bars its owner here.
                self.offer_political_secrets(state, &alias)?;
                return Ok(Some((alias, choices)));
            };
            self.emit(&format!("AGENDA_DISCARDED:{alias}"));
            if state.rules_version >= ti4_model::state::RULES_AGENDA_GRANT_REVEAL {
                self.note_discarded_agenda(state, &alias, true);
            }
            // Covert Legislation replaced: the agenda the speaker drew for it goes with it.
            state.covert_agenda = None;
            alias = replacement;
        }
    }

    /// The outcomes an agenda is voted on: Covert Legislation draws its face-down agenda's, any other
    /// agenda reads its own.
    pub(crate) fn reveal_choices(&mut self, state: &mut GameState, alias: &str) -> Vec<String> {
        if alias == crate::vote::COVERT
            && state.rules_version >= ti4_model::state::RULES_COVERT_LEGISLATION
        {
            crate::game::draw_covert_agenda(state, self.content, self.sources)
        } else {
            outcomes(state, self.content, self.sources, alias)
        }
    }

    /// Quash and Political Favor replace the agenda the same way Veto does, so each is offered only
    /// when nothing has replaced it already.
    pub(crate) fn offer_replacements(&mut self, state: &mut GameState, alias: &str) -> Result<(), GameError> {
        if state.agenda_veto_replacement.is_none() && self.offer_quash(state, alias)? {
            self.emit("QUASH_USED");
        }
        if state.agenda_veto_replacement.is_none() && self.offer_political_favor(state, alias)? {
            self.emit("POLITICAL_FAVOR_USED");
        }
        Ok(())
    }

    /// List an agenda that was revealed and discarded without a vote in `agenda_log`, so the
    /// table is told (rules version 14). It is not a resolved agenda: it has no outcome.
    fn note_discarded_agenda(&mut self, state: &mut GameState, alias: &str, replaced: bool) {
        let reason = if replaced { "replaced" } else { "no_choice" };
        push_agenda_record(state, self.content, alias, None, BTreeMap::new(), false, true);
        if let Some(record) = state.agenda_log.last_mut() {
            record.discard_reason = Some(reason.to_owned());
        }
    }

    /// Ask a note's holder (or an ability's owner) whether to use it now, on the agenda just
    /// revealed. The agenda rides along as display-only `details.agenda` (name, type, target,
    /// printed text), as on every vote decision: the question is about that card, so a client
    /// has to be able to show it before the answer.
    fn ask_to_use_note(
        &mut self,
        state: &GameState,
        holder: &PlayerId,
        alias: &str,
        prompt: String,
        agenda: &str,
    ) -> Result<bool, GameError> {
        let choice = crate::choice::Choice::new(
            holder.clone(),
            prompt,
            vec![
                crate::choice::ChoiceOption::labelled(
                    "use",
                    "promissory_note",
                    format!("use {alias}"),
                ),
                crate::choice::ChoiceOption::decline(),
            ],
        )
        .contextualized(crate::decision_context::DecisionContext::new(
            holder.clone(),
            crate::decision_context::DecisionSource::Content(alias.to_owned()),
            alias,
            state.phase,
            state.round,
        ))
        .detailed(
            "agenda",
            crate::strategy_cards::agenda_details(self.content, agenda),
        );
        let answer = self
            .table
            .ask_seeing(
                &choice,
                &crate::choice::Observed::new(state, self.content, self.sources, self.galaxy),
            )
            .map_err(GameError::IllegalChoice)?;
        Ok(!answer.is_decline())
    }

    /// Political Favor (Xxcha): "When an agenda is revealed: Remove 1 token from the Xxcha player's
    /// strategy pool and return it to their reinforcements. Then, discard the revealed agenda and
    /// reveal 1 agenda from the top of the deck. Players vote on this agenda instead. Then, return
    /// this card to the Xxcha player."
    ///
    /// Offered to each holder in turn until one uses it. The replacement is handed over exactly as
    /// Veto hands its own, so the reveal loop discards this agenda and continues. Priced for
    /// trading since the port, the card was never offered and could not be used.
    pub(crate) fn offer_political_favor(&mut self, state: &mut GameState, agenda: &str) -> Result<bool, GameError> {
        for (note, holder, owner) in lent_notes(state, "favor") {
            let payable = state
                .player(&owner)
                .is_some_and(|seat| seat.tokens(ti4_model::state::TokenPool::Strategic) > 0);
            if !payable || state.agenda_deck.is_empty() {
                continue;
            }
            let prompt = "Political Favor: discard this agenda and reveal the next".to_owned();
            if !self.ask_to_use_note(state, &holder, "favor", prompt, agenda)? {
                continue;
            }
            if let Some(seat) = state.player_mut(&owner) {
                seat.gain_token_uncapped(ti4_model::state::TokenPool::Strategic, -1);
            }
            crate::supply::note_strategy_token_spent(state, &owner, "political_favor");
            let Some(replacement) = ti4_model::deck_reserve::take_top(
                &mut state.deck_reserve,
                "agenda",
                &mut state.agenda_deck,
                None,
            ) else {
                return Ok(false);
            };
            state.agenda_veto_replacement = Some(replacement);
            crate::promissory::give_back(state, &note);
            return Ok(true);
        }
        Ok(false)
    }

    /// Quash (Xxcha): "When an agenda is revealed: You may spend 1 token from your strategy pool to
    /// discard that agenda and reveal 1 agenda from the top of the deck. Players vote on this
    /// agenda instead."
    ///
    /// Offered to each seat with the ability and a strategy token, in seat order, until one uses
    /// it. The replacement is handed over exactly as Veto and Political Favor hand theirs, so the
    /// reveal loop discards this agenda and continues (and offers Quash again on the new one).
    pub(crate) fn offer_quash(&mut self, state: &mut GameState, agenda: &str) -> Result<bool, GameError> {
        if state.agenda_deck.is_empty() {
            return Ok(false);
        }
        let owners: Vec<PlayerId> = state
            .players
            .iter()
            .filter(|seat| seat.tokens(ti4_model::state::TokenPool::Strategic) > 0)
            .map(|seat| seat.id.clone())
            .filter(|id| crate::faction_abilities::has(state, self.content, id, "quash"))
            .collect();
        for owner in owners {
            let prompt = "Quash: spend 1 strategy token to discard this agenda and reveal the next"
                .to_owned();
            if !self.ask_to_use_note(state, &owner, "quash", prompt, agenda)? {
                continue;
            }
            if let Some(seat) = state.player_mut(&owner) {
                seat.gain_token_uncapped(ti4_model::state::TokenPool::Strategic, -1);
            }
            crate::supply::note_strategy_token_spent(state, &owner, "quash");
            let Some(replacement) = ti4_model::deck_reserve::take_top(
                &mut state.deck_reserve,
                "agenda",
                &mut state.agenda_deck,
                None,
            ) else {
                return Ok(false);
            };
            state.agenda_veto_replacement = Some(replacement);
            return Ok(true);
        }
        Ok(false)
    }

    /// Political Secret: "When an agenda is revealed: The <color> player cannot vote, play action
    /// cards, or use faction abilities until after that agenda has been resolved. Then, return this
    /// card to the <color> player."
    ///
    /// Offered for the agenda players will vote on. The vote bar is the marker Assassinate
    /// Representative sets; barring action cards and faction abilities is not modelled. Priced for
    /// trading since the port, the card was never offered and could not be used.
    pub(crate) fn offer_political_secrets(&mut self, state: &mut GameState, agenda: &str) -> Result<(), GameError> {
        for (note, holder, owner) in lent_notes(state, "ps") {
            if state.agenda_predictions.contains_key(&owner) {
                continue; // already cannot vote on this agenda
            }
            let prompt = format!("Political Secret: {owner} cannot vote on this agenda");
            if !self.ask_to_use_note(state, &holder, "ps", prompt, agenda)? {
                continue;
            }
            state
                .agenda_predictions
                .insert(owner.clone(), "none|political_secret".to_owned());
            crate::promissory::give_back(state, &note);
            self.emit(&format!("POLITICAL_SECRET_USED:{owner}"));
        }
        Ok(())
    }
}
