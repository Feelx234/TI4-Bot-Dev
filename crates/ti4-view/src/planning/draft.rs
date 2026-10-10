//! A draft of a tactical action, run in one call.
//!
//! A draft is a script: what the planner chose so far. A run plays the whole script on a
//! disposable [`Game::fork`] with a fresh hypothetical turn, and returns where it stopped and
//! what the planner is shown there. Nothing is kept between two runs, so a changed script is run
//! again from its start, and the caller needs no thread and no game of its own. The server's
//! planning runner keeps a worker thread around the same checks ([`super::audit`]).
//!
//! The run covers the activation and the movement. It does not answer "done moving": the
//! steps after the movement roll dice or ask other seats, and are not drafted yet.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation, Table};
use ti4_engine::game::Game;
use ti4_engine::observation::ExecutionObservation;

use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::audit::{PlanScope, SafePublication, StopReason, audit_offer, validate_knowledge};
use crate::projection::{SessionUpdate, project_game_view, project_session_update};
use crate::status::ViewerRole;
use crate::tactical::{TacticalFacts, project_tactical_facts};
use crate::view::{BoardTileView, GameView};

/// The nonce of the question a draft is open at. A draft is never answered by its nonce.
pub const DRAFT_NONCE: &str = "draft";

/// How many engine steps a run takes at most. A tactical action up to the end of its movement
/// needs a few.
const STEP_BOUND: usize = 32;

/// One entry of a draft script. The movement steps name what is moved and loaded, not the ids
/// of the options: the engine numbers its options again after each ship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftStep {
    /// Answer the open question with the option of this id.
    Choose {
        option_id: String,
    },
    Move {
        origin: String,
        unit: String,
        damaged: bool,
        gravity_drive: bool,
        ionian: bool,
    },
    Load {
        /// The system that the ship starts in.
        origin: String,
        /// The system where the unit is picked up.
        pickup_system: String,
        unit: String,
        /// The planet, or `None` for the space area.
        source: Option<String>,
        damaged: bool,
        galvanized: bool,
    },
    DoneLoading,
    DoneMoving,
}

impl DraftStep {
    /// The question that this step answers, by the subtype of its context. `None`: any question.
    const fn subtype(&self) -> Option<&'static str> {
        match self {
            Self::Choose { .. } => None,
            Self::Move { .. } | Self::DoneMoving => Some("movement_step"),
            Self::Load { .. } | Self::DoneLoading => Some("load_cargo"),
        }
    }

    /// Whether an offered option is what this step asks for.
    #[must_use]
    pub fn matches_option(&self, option: &ChoiceOption) -> bool {
        let text = |key: &str| option.payload.get(key).and_then(serde_json::Value::as_str);
        let flag = |key: &str| {
            option
                .payload
                .get(key)
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        match self {
            Self::Choose { option_id } => option.id == *option_id,
            Self::Move {
                origin,
                unit,
                damaged,
                gravity_drive,
                ionian,
            } => {
                option.kind == ti4_engine::tactical::MOVE_KIND
                    && text("origin") == Some(origin)
                    && text("unit") == Some(unit)
                    && flag("damaged") == *damaged
                    && flag("gravity_drive") == *gravity_drive
                    && flag("ionian") == *ionian
            }
            Self::Load {
                origin,
                pickup_system,
                unit,
                source,
                damaged,
                galvanized,
            } => {
                option.kind == ti4_engine::transit::LOAD_KIND
                    && text("system") == Some(origin)
                    && text("pickup_system") == Some(pickup_system)
                    && text("unit") == Some(unit)
                    && text("source") == source.as_deref()
                    && flag("damaged") == *damaged
                    && flag("galvanized") == *galvanized
            }
            Self::DoneLoading => option.id == "done_loading",
            Self::DoneMoving => option.id == "done_moving",
        }
    }
}

/// Why a run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DraftStop {
    /// The script is used up: the question of the update is open.
    Open,
    /// The script ends the movement, and the game offers that. The draft goes no further.
    Complete,
    /// The game does not offer what step `step` of the script asks for.
    Refused { step: usize, reason: String },
    /// The run reached something a draft cannot show: dice, a card that is drawn, a question
    /// of another seat or one outside the audited offers.
    Stopped { reason: StopReason },
    /// No hypothetical turn can be made from this position, or the engine failed.
    Failed { reason: String },
}

/// Where a run of a script ended, and what the planner is shown there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftOutcome {
    /// The position where the run ended, with the open question when the stop is
    /// [`DraftStop::Open`] or [`DraftStop::Complete`]. The same shape as a live update.
    pub update: SessionUpdate,
    /// The question of the first ship to move, before any ship moved: what a whole movement is
    /// staged against. `None` when the run did not reach the movement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<SessionUpdate>,
    /// How many steps of the script the game took.
    pub consumed: usize,
    pub stop: DraftStop,
}

/// What a draft is run on: the live game at a completed step, and the map for the facts.
pub struct DraftBase<'a, 'c> {
    pub game: &'a Game<'c>,
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub tiles: &'a [BoardTileView],
    pub galaxy: Option<&'a Galaxy>,
}

/// A question that passed the audit, on the position it was asked in.
struct Offer {
    state: GameState,
    choice: Choice,
}

struct Run {
    player: PlayerId,
    script: Vec<DraftStep>,
    next: usize,
    stop: Option<DraftStop>,
    baseline: GameView,
    /// The position of the question that is asked now. The offer callback fills it, and the
    /// decider takes it: a question without a position is not shown.
    seen: Option<GameState>,
    last: Option<Offer>,
    movement: Option<Offer>,
    /// A ship of the script moved.
    moved: bool,
}

impl Run {
    fn end(&mut self, stop: DraftStop) {
        self.stop.get_or_insert(stop);
    }

    /// The answer of the script to `choice`, or the stop that it runs into.
    fn answer(
        &mut self,
        choice: &Choice,
        observation: &ExecutionObservation,
    ) -> Option<ChoiceOption> {
        let state = self.seen.take();
        if self.stop.is_some() {
            return None;
        }
        let activity = observation.activity();
        // Uncertainty wins over any later unsupported path, as in the server's gate.
        let reason = if activity.randomness || activity.hidden_information {
            Some(StopReason::Uncertainty)
        } else if activity.unsupported_participation {
            Some(StopReason::UnsupportedParticipation)
        } else if activity.unsupported_segment {
            Some(StopReason::UnsupportedSegment)
        } else if choice.player != self.player {
            Some(StopReason::OtherPlayerRequired)
        } else {
            None
        };
        let offer = match (reason, state) {
            (Some(reason), _) => Err(reason),
            (None, None) => Err(StopReason::UnsupportedOffer),
            (None, Some(state)) => self.audited(state, choice),
        };
        let offer = match offer {
            Ok(offer) => offer,
            Err(reason) => {
                self.end(DraftStop::Stopped { reason });
                return None;
            }
        };
        let subtype = choice
            .context
            .as_ref()
            .map_or("", |context| context.subtype.as_str());
        if subtype == "movement_step" && !self.moved && self.movement.is_none() {
            self.movement = Some(Offer {
                state: offer.state.clone(),
                choice: offer.choice.clone(),
            });
        }
        let offered = self.last.insert(offer);
        // A hold that closed itself needs no "done loading".
        if subtype == "movement_step" {
            while self.script.get(self.next) == Some(&DraftStep::DoneLoading) {
                self.next += 1;
            }
        }
        let Some(step) = self.script.get(self.next) else {
            self.end(DraftStop::Open);
            return None;
        };
        // A hold that the script does not load is declined.
        let declined = subtype == "load_cargo" && step.subtype() != Some("load_cargo");
        let step = if declined {
            &DraftStep::DoneLoading
        } else {
            step
        };
        if step.subtype().is_some_and(|wanted| wanted != subtype) {
            self.stop = Some(DraftStop::Refused {
                step: self.next,
                reason: "The game asks for another decision first.".to_owned(),
            });
            return None;
        }
        let mut matching = offered
            .choice
            .options
            .iter()
            .filter(|option| step.matches_option(option));
        let option = match (matching.next(), matching.next()) {
            (Some(option), None) => option.clone(),
            (found, _) => {
                let reason = if found.is_some() {
                    "The game offers this in more than one way."
                } else {
                    "The game does not offer this."
                };
                self.stop = Some(DraftStop::Refused {
                    step: self.next,
                    reason: reason.to_owned(),
                });
                return None;
            }
        };
        if *step == DraftStep::DoneMoving {
            // Offered, and not taken: what follows the movement is not drafted.
            self.next += 1;
            self.end(DraftStop::Complete);
            return None;
        }
        if !declined {
            self.moved |= matches!(step, DraftStep::Move { .. });
            self.next += 1;
        }
        Some(option)
    }

    /// The question as the planner may see it: only the audited options, on a position that
    /// tells nothing new.
    fn audited(&self, state: GameState, choice: &Choice) -> Result<Offer, StopReason> {
        let enabled = audit_offer(choice)?;
        let mut choice = choice.clone();
        choice.options.retain(|option| enabled.contains(&option.id));
        let viewer = ViewerRole::Player(self.player.clone());
        let publication = SafePublication {
            position: project_game_view(&state, &viewer),
            choice: Some(choice),
            events: Vec::new(),
        };
        validate_knowledge(
            &self.baseline,
            &publication,
            &self.player,
            &PlanScope::Tactical,
        )?;
        Ok(Offer {
            state,
            choice: publication.choice.expect("the audited choice"),
        })
    }
}

fn refused(choice: &Choice) -> IllegalChoice {
    IllegalChoice::DeciderFailed {
        player: choice.player.clone(),
        prompt: choice.prompt.clone(),
        reason: "the draft stops here".to_owned(),
    }
}

struct ScriptDecider {
    run: Arc<Mutex<Run>>,
    observation: ExecutionObservation,
    /// False for the table of the timing resolver, which asks without a position.
    observed: bool,
}

impl ScriptDecider {
    fn decide(&self, choice: &Choice, observed: bool) -> Result<ChoiceOption, IllegalChoice> {
        let mut run = self.run.lock().expect("draft lock");
        if !observed {
            run.seen = None;
        }
        run.answer(choice, &self.observation)
            .ok_or_else(|| refused(choice))
    }
}

impl Decider for ScriptDecider {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.decide(choice, false)
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        _seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.decide(choice, self.observed)
    }
}

/// Play `script` as the next turn of `player` on a copy of the base, and tell where it ended.
/// The base is not changed.
#[must_use]
pub fn run_draft(
    base: &DraftBase<'_, '_>,
    player: &PlayerId,
    script: &[DraftStep],
) -> DraftOutcome {
    let viewer = ViewerRole::Player(player.clone());
    let mut game = base.game.fork();
    let observation = ExecutionObservation::default();
    game.bind_observation(observation.clone());
    // Other seats take no optional reactions, and their conditions are never read.
    let planner = player.clone();
    game.timing.set_participation(Arc::new(move |ability| {
        ability.owner == planner || !ability.optional
    }));
    let run = Arc::new(Mutex::new(Run {
        player: player.clone(),
        script: script.to_vec(),
        next: 0,
        stop: None,
        baseline: project_game_view(&game.state, &viewer),
        seen: None,
        last: None,
        movement: None,
        moved: false,
    }));
    let decider = |observed| {
        Box::new(ScriptDecider {
            run: run.clone(),
            observation: observation.clone(),
            observed,
        })
    };
    *game.timing.table_mut() = Table::with_default(decider(false));
    game.table = Table::with_default(decider(true));
    let seen = run.clone();
    game.table.on_observed_offer(move |_, state| {
        seen.lock().expect("draft lock").seen = Some(state.clone());
    });

    let ended = |run: &Arc<Mutex<Run>>| run.lock().expect("draft lock").stop.is_some();
    if let Err(error) = game.prepare_hypothetical_turn(player) {
        run.lock().expect("draft lock").end(DraftStop::Failed {
            reason: error.to_string(),
        });
    }
    for _ in 0..STEP_BOUND {
        if ended(&run) {
            break;
        }
        let error = game.step().error;
        let mut run = run.lock().expect("draft lock");
        if let Some(error) = error {
            run.end(DraftStop::Failed {
                reason: error.to_string(),
            });
        }
    }
    drop(game);
    let mut run = run.lock().expect("draft lock");
    run.end(DraftStop::Stopped {
        reason: StopReason::StepLimit,
    });
    let stop = run.stop.clone().expect("the run ended");

    let explored = |state: &GameState, mut update: SessionUpdate| {
        update.exploration = base.galaxy.map(|galaxy| {
            crate::exploration::project_exploration_facts(state, base.content, base.sources, galaxy)
        });
        update
    };
    let shown = |offer: &Offer, open: bool| {
        let mut update = project_session_update(
            &offer.state,
            &viewer,
            open.then_some((&offer.choice, DRAFT_NONCE)),
            base.tiles,
        );
        if open {
            update.tactical = facts(base, &offer.state, &offer.choice);
        }
        explored(&offer.state, update)
    };
    let open = matches!(stop, DraftStop::Open | DraftStop::Complete);
    DraftOutcome {
        update: run.last.as_ref().map_or_else(
            || {
                explored(
                    &base.game.state,
                    project_session_update(&base.game.state, &viewer, None, base.tiles),
                )
            },
            |offer| shown(offer, open),
        ),
        movement: run.movement.as_ref().map(|offer| shown(offer, true)),
        consumed: run.next,
        stop,
    }
}

fn facts(base: &DraftBase<'_, '_>, state: &GameState, choice: &Choice) -> Option<TacticalFacts> {
    project_tactical_facts(state, base.content, base.sources, base.galaxy?, choice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine::choice::SeededRandom;
    use ti4_model::content_types::POK;
    use ti4_model::state::Phase;

    /// A seeded game of six on the recommended map, played by random seats to its first action.
    fn game() -> (Game<'static>, Vec<BoardTileView>) {
        game_until(|game| game.state.phase == Phase::Action)
    }

    /// The same game, played by random seats until `reached`.
    fn game_until(reached: impl Fn(&Game<'static>) -> bool) -> (Game<'static>, Vec<BoardTileView>) {
        let content = ContentStore::embedded();
        let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|name| PlayerId::new(*name))
            .collect();
        let loader = crate::maps::TemplateLoader::load().expect("the templates load");
        let template = crate::maps::default_template_for(content, &loader, 6, POK)
            .expect("a template for six");
        let (state, galaxy) =
            crate::map::create_game_with_template(content, &ids, 3, Some(&template))
                .expect("a game");
        let tiles = crate::map::build_board_tiles(content, &galaxy);
        let mut game = Game::with_seeded_random(state, content, 3)
            .with_sources(POK)
            .with_galaxy(galaxy);
        game.table = Table::with_default(Box::new(SeededRandom::new(3)));
        while !reached(&game) {
            assert_eq!(game.step().error, None);
        }
        (game, tiles)
    }

    #[test]
    fn a_draft_opens_while_the_strategy_cards_are_chosen() {
        // Two seats have a card, four have none.
        let (game, tiles) = game_until(|game| {
            let seats = game.state.players.iter();
            seats.filter(|seat| !seat.strategy_cards.is_empty()).count() == 2
        });
        assert_eq!(game.state.phase, Phase::Strategy);
        let before = game.state.clone();

        let mut script = vec![choose("tactical")];
        let activation = run(&game, &tiles, &script);
        assert_eq!(activation.stop, DraftStop::Open);
        let Some(TacticalFacts::Activation(facts)) = &activation.update.tactical else {
            panic!("no activation facts");
        };
        let target = facts
            .systems
            .iter()
            .find(|reach| reach.ships > 0)
            .expect("a system in range");
        script.push(choose(&target.system));
        let movement = run(&game, &tiles, &script);
        assert_eq!(movement.stop, DraftStop::Open);
        let Some(TacticalFacts::Movement(facts)) = &movement.update.tactical else {
            panic!("no movement facts");
        };
        let ship = facts
            .ships
            .iter()
            .find(|ship| ship.r#move.is_some())
            .expect("a ship that can move");
        let plan = ship.r#move.as_ref().unwrap();
        script.push(DraftStep::Move {
            origin: ship.origin.clone(),
            unit: ship.unit.clone(),
            damaged: ship.damaged,
            gravity_drive: plan.gravity_drive,
            ionian: plan.ionian,
        });
        script.push(DraftStep::DoneLoading);
        script.push(DraftStep::DoneMoving);
        let moved = run(&game, &tiles, &script);
        assert_eq!(moved.stop, DraftStop::Complete);
        assert_eq!(moved.consumed, script.len());

        assert_eq!(game.state, before, "a draft changes nothing");
    }

    fn run(game: &Game<'static>, tiles: &[BoardTileView], script: &[DraftStep]) -> DraftOutcome {
        let base = DraftBase {
            game,
            content: ContentStore::embedded(),
            sources: POK,
            tiles,
            galaxy: game.galaxy(),
        };
        run_draft(&base, &PlayerId::new("a"), script)
    }

    fn choose(id: &str) -> DraftStep {
        DraftStep::Choose {
            option_id: id.to_owned(),
        }
    }

    fn subtype(update: &SessionUpdate) -> &str {
        let choice = &update.pending_choice.as_ref().expect("a question").choice;
        &choice.context.as_ref().expect("a context").subtype
    }

    #[test]
    fn a_draft_goes_through_activation_and_movement_and_leaves_the_game_as_it_was() {
        let (game, tiles) = game();
        let before = game.state.clone();

        let menu = run(&game, &tiles, &[]);
        assert_eq!(menu.stop, DraftStop::Open);
        assert_eq!(subtype(&menu.update), "action_menu");
        let offered = &menu.update.pending_choice.as_ref().unwrap().choice.options;
        assert_eq!(offered.len(), 1, "only the tactical action is shown");

        let mut script = vec![choose("tactical")];
        let activation = run(&game, &tiles, &script);
        assert_eq!(activation.stop, DraftStop::Open);
        assert_eq!(activation.consumed, 1);
        let Some(TacticalFacts::Activation(facts)) = &activation.update.tactical else {
            panic!("no activation facts");
        };
        let target = facts
            .systems
            .iter()
            .find(|reach| reach.ships > 0)
            .expect("a system in range");

        script.push(choose(&target.system));
        let movement = run(&game, &tiles, &script);
        assert_eq!(movement.stop, DraftStop::Open);
        assert_eq!(subtype(&movement.update), "movement_step");
        assert_eq!(movement.movement.as_ref(), Some(&movement.update));
        let Some(TacticalFacts::Movement(facts)) = &movement.update.tactical else {
            panic!("no movement facts");
        };
        assert_eq!(facts.active, target.system);
        let ship = facts
            .ships
            .iter()
            .find(|ship| ship.r#move.is_some())
            .expect("a ship that can move");
        let plan = ship.r#move.as_ref().unwrap();

        script.push(DraftStep::Move {
            origin: ship.origin.clone(),
            unit: ship.unit.clone(),
            damaged: ship.damaged,
            gravity_drive: plan.gravity_drive,
            ionian: plan.ionian,
        });
        script.push(DraftStep::DoneLoading);
        script.push(DraftStep::DoneMoving);
        let moved = run(&game, &tiles, &script);
        assert_eq!(moved.stop, DraftStop::Complete);
        assert_eq!(moved.consumed, script.len());
        // The facts to stage against are still those from before the first ship moved.
        assert_eq!(moved.movement, movement.movement);
        let arrived = &moved.update.view.board.systems
            [&ti4_model::id::SystemId::new(target.system.as_str())]
            .units;
        assert!(
            arrived
                .iter()
                .any(|unit| unit.owner.as_str() == "a" && unit.unit_type.as_str() == ship.unit)
        );

        assert_eq!(game.state, before, "a draft changes nothing");
    }

    #[test]
    fn a_step_that_the_game_does_not_offer_is_named() {
        let (game, tiles) = game();
        let activation = run(&game, &tiles, &[choose("tactical")]);
        let Some(TacticalFacts::Activation(facts)) = &activation.update.tactical else {
            panic!("no activation facts");
        };
        let script = [
            choose("tactical"),
            choose(&facts.systems[0].system),
            DraftStep::Move {
                origin: "nowhere".to_owned(),
                unit: "carrier".to_owned(),
                damaged: false,
                gravity_drive: false,
                ionian: false,
            },
            DraftStep::DoneMoving,
        ];
        let outcome = run(&game, &tiles, &script);
        assert!(
            matches!(outcome.stop, DraftStop::Refused { step: 2, .. }),
            "{:?}",
            outcome.stop
        );
        assert_eq!(outcome.consumed, 2);
        assert!(outcome.movement.is_some());

        let unknown = run(
            &game,
            &tiles,
            &[choose("tactical"), choose("no such system")],
        );
        assert!(matches!(unknown.stop, DraftStop::Refused { step: 1, .. }));
    }
}
