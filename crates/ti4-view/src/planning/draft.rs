//! A draft of a tactical action, run in one call.
//!
//! A draft is a script: what the planner chose so far. A run plays the whole script on a
//! disposable [`Game::fork`] with a fresh hypothetical turn, and returns where it stopped and
//! what the planner is shown there. Nothing is kept between two runs, so a changed script is run
//! again from its start, and the caller needs no thread and no game of its own. The server's
//! planning runner keeps a worker thread around the same guard ([`super::fork`]) and the same
//! checks ([`super::audit`]).
//!
//! The run covers the whole action: the activation, the movement, the landings of an invasion
//! and the production. It ends where the script does, or where something random lies ahead:
//! dice, a card that is drawn, a combat, a question of another seat.

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
use super::fork::{TACTICAL_COMPLETE, activity_stop, guard_fork};
use super::steps::{INTERRUPTED, PlanAnswer, PlanStep, plan_answer};
use crate::projection::{FactsContext, SessionUpdate, project_game_view, project_session_update};
use crate::status::ViewerRole;
use crate::view::{BoardTileView, GameView};

/// The nonce of the question a draft is open at. A draft is never answered by its nonce.
pub const DRAFT_NONCE: &str = "draft";

/// How many engine steps a run takes at most. A whole tactical action needs a few: most of its
/// questions are asked inside one step.
const STEP_BOUND: usize = 64;

/// An answer by the id of its option: the action of the turn menu, the system that is activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChosenOption {
    Choose { option_id: String },
}

/// One entry of a draft script: an answer by its id, or a step of the action ([`PlanStep`]) as
/// the page sends it to the live game. The steps that follow each other are one plan
/// ([`plan_answer`]); an answer by id between them is for what the game asks there (an ability
/// of the planner before the production).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DraftStep {
    Chosen(ChosenOption),
    Step(PlanStep),
}

impl DraftStep {
    /// Answer the open question with the option of this id.
    #[must_use]
    pub fn choose(option_id: impl Into<String>) -> Self {
        Self::Chosen(ChosenOption::Choose {
            option_id: option_id.into(),
        })
    }
}

impl From<PlanStep> for DraftStep {
    fn from(step: PlanStep) -> Self {
        Self::Step(step)
    }
}

/// What a script answers to a question of the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptAnswer {
    /// Answer with this option. `next` is the first entry that is still to play. With `script`
    /// the script goes on in this order: a payment changed its place ([`plan_answer`]).
    Answer {
        option: ChoiceOption,
        next: usize,
        script: Option<Vec<DraftStep>>,
    },
    /// Every entry is played: the question is not of the script.
    Done,
    /// The game asks something that the next entry does not answer (a reaction window, another
    /// seat). Entries are left.
    Interrupted { reason: String },
    /// The game does not offer what entry `step` asks for.
    Mismatch { step: usize, reason: String },
}

/// What `script` answers to `choice`, from entry `next` on, for the seat `actor`.
///
/// The one answer of a script to a question, for a run of a draft and for a script that is sent
/// to the live game. The steps that follow each other are one plan ([`plan_answer`]); an answer
/// by id is for whatever the game asks the seat there. The pause between two steps of the action
/// is passed while entries are left.
#[must_use]
pub fn script_answer(
    script: &[DraftStep],
    mut next: usize,
    choice: &Choice,
    actor: &PlayerId,
) -> ScriptAnswer {
    let own = &choice.player == actor;
    let pause = choice
        .context
        .as_ref()
        .is_some_and(|context| context.subtype == "mid_action_pause");
    if own && pause && next < script.len() {
        let passes = choice
            .option(ti4_engine::game::CONTINUE_ACTION_ID)
            .filter(|option| option.kind == ti4_engine::game::CONTINUE_ACTION_KIND);
        if let Some(option) = passes {
            return ScriptAnswer::Answer {
                option: option.clone(),
                next,
                script: None,
            };
        }
    }
    if matches!(script.get(next), Some(DraftStep::Step(_))) {
        // The steps around the next one are one plan.
        let is_step = |entry: &DraftStep| matches!(entry, DraftStep::Step(_));
        let start = script[..next]
            .iter()
            .rposition(|entry| !is_step(entry))
            .map_or(0, |index| index + 1);
        let end = script[next..]
            .iter()
            .position(|entry| !is_step(entry))
            .map_or(script.len(), |offset| next + offset);
        let plan: Vec<PlanStep> = script[start..end]
            .iter()
            .filter_map(|entry| match entry {
                DraftStep::Step(step) => Some(step.clone()),
                DraftStep::Chosen(_) => None,
            })
            .collect();
        match plan_answer(&plan, next - start, choice, actor) {
            PlanAnswer::Answer {
                option,
                next,
                steps,
            } => {
                let script = steps.map(|steps| {
                    let mut order = script.to_vec();
                    order.splice(start..end, steps.into_iter().map(DraftStep::Step));
                    order
                });
                return ScriptAnswer::Answer {
                    option,
                    next: start + next,
                    script,
                };
            }
            // The game needs no more of these steps: what follows them is next.
            PlanAnswer::Done => next = end,
            PlanAnswer::Interrupted { reason } => return ScriptAnswer::Interrupted { reason },
            PlanAnswer::Mismatch { step, reason } => {
                return ScriptAnswer::Mismatch {
                    step: start + step,
                    reason,
                };
            }
        }
    }
    match script.get(next) {
        Some(DraftStep::Chosen(ChosenOption::Choose { option_id })) => {
            if !own {
                return ScriptAnswer::Interrupted {
                    reason: INTERRUPTED.to_owned(),
                };
            }
            match choice.option(option_id) {
                Some(option) => ScriptAnswer::Answer {
                    option: option.clone(),
                    next: next + 1,
                    script: None,
                },
                None => ScriptAnswer::Mismatch {
                    step: next,
                    reason: format!("The game does not offer \"{option_id}\" here."),
                },
            }
        }
        _ => ScriptAnswer::Done,
    }
}

/// Why a run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DraftStop {
    /// The script is used up: the question of the update is open.
    Open,
    /// The game played the whole script, and the tactical action is over.
    Complete,
    /// The game does not offer what step `step` of the script asks for.
    Refused { step: usize, reason: String },
    /// The run reached something a draft cannot show: dice, a card that is drawn, a combat, a
    /// question of another seat or one outside the audited offers.
    Stopped { reason: StopReason },
    /// No hypothetical turn can be made from this position, or the engine failed.
    Failed { reason: String },
}

/// Where a run of a script ended, and what the planner is shown there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftOutcome {
    /// The position where the run ended, with the open question when the stop is
    /// [`DraftStop::Open`]. The same shape as a live update.
    pub update: SessionUpdate,
    /// The question of the first ship to move, before any ship moved: what a whole movement is
    /// staged against. `None` when the run did not reach the movement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<SessionUpdate>,
    /// The question of the first ground force to land: what the landings are staged against.
    /// `None` when the run did not reach them, or the action has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invasion: Option<SessionUpdate>,
    /// The question of the first unit to produce: what a production is staged against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub production: Option<SessionUpdate>,
    /// How many steps of the script the game took, or did not need.
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
#[derive(Clone)]
struct Offer {
    state: GameState,
    choice: Choice,
}

struct Run {
    player: PlayerId,
    /// A payment may change its place among the steps around it ([`plan_answer`]).
    script: Vec<DraftStep>,
    /// How many entries of the script are done.
    next: usize,
    stop: Option<DraftStop>,
    baseline: GameView,
    /// The position of the question that is asked now. The offer callback fills it, and the
    /// decider takes it: a question without a position is not shown.
    seen: Option<GameState>,
    last: Option<Offer>,
    movement: Option<Offer>,
    invasion: Option<Offer>,
    production: Option<Offer>,
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
        let reason = activity_stop(observation)
            .or_else(|| (choice.player != self.player).then_some(StopReason::OtherPlayerRequired));
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
        let subtype = offer
            .choice
            .context
            .as_ref()
            .map_or("", |context| context.subtype.as_str());
        if subtype == "mid_action_pause" {
            // The pause between two steps of the action is no step of a draft: it goes on.
            return offer
                .choice
                .option(ti4_engine::game::CONTINUE_ACTION_ID)
                .cloned();
        }
        // The first question of a step is what the whole step is staged against.
        let first = match subtype {
            crate::tactical::MOVEMENT_SUBTYPE => Some(&mut self.movement),
            crate::tactical::INVASION_SUBTYPE => Some(&mut self.invasion),
            crate::tactical::PRODUCTION_SUBTYPE => Some(&mut self.production),
            _ => None,
        };
        if let Some(first) = first {
            first.get_or_insert_with(|| offer.clone());
        }
        let offered = self.last.insert(offer).choice.clone();
        match script_answer(&self.script, self.next, &offered, &self.player) {
            ScriptAnswer::Answer {
                option,
                next,
                script,
            } => {
                self.next = next;
                if let Some(script) = script {
                    self.script = script;
                }
                Some(option)
            }
            ScriptAnswer::Done => {
                self.next = self.script.len();
                self.end(DraftStop::Open);
                None
            }
            // A question of the planner that the script has no answer for is open, with the
            // rest of the script still to play.
            ScriptAnswer::Interrupted { .. } => {
                self.end(DraftStop::Open);
                None
            }
            ScriptAnswer::Mismatch { step, reason } => {
                self.end(DraftStop::Refused { step, reason });
                None
            }
        }
    }

    /// The action is over. The entries that are left must be such that the game needs no answer
    /// for them: an end that it did not ask for, a payment that it did not need.
    fn complete(&mut self) {
        let needless = |entry: &DraftStep| {
            matches!(
                entry,
                DraftStep::Step(
                    PlanStep::DoneLoading
                        | PlanStep::DoneCommitting
                        | PlanStep::Pay { .. }
                        | PlanStep::Place { .. }
                        | PlanStep::DoneProducing
                )
            )
        };
        if self.script[self.next..].iter().all(needless) {
            self.next = self.script.len();
            self.end(DraftStop::Complete);
        } else {
            self.end(DraftStop::Refused {
                step: self.next,
                reason: "The action is over before this.".to_owned(),
            });
        }
    }

    /// Whether a position tells the planner nothing new.
    fn known(
        &self,
        state: &GameState,
        choice: Option<Choice>,
    ) -> Result<Option<Choice>, StopReason> {
        let viewer = ViewerRole::Player(self.player.clone());
        let publication = SafePublication {
            position: project_game_view(state, &viewer),
            choice,
            events: Vec::new(),
        };
        validate_knowledge(
            &self.baseline,
            &publication,
            &self.player,
            &PlanScope::Tactical,
        )?;
        Ok(publication.choice)
    }

    /// The question as the planner may see it: only the audited options, on a position that
    /// tells nothing new.
    fn audited(&self, state: GameState, choice: &Choice) -> Result<Offer, StopReason> {
        let enabled = audit_offer(choice)?;
        let mut choice = choice.clone();
        choice.options.retain(|option| enabled.contains(&option.id));
        let choice = self
            .known(&state, Some(choice))?
            .expect("the audited choice");
        Ok(Offer { state, choice })
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
    let observation = guard_fork(&mut game, player);
    let run = Arc::new(Mutex::new(Run {
        player: player.clone(),
        script: script.to_vec(),
        next: 0,
        stop: None,
        baseline: project_game_view(&game.state, &viewer),
        seen: None,
        last: None,
        movement: None,
        invasion: None,
        production: None,
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
    // The position after the last step that ended with no question open.
    let mut settled = None;
    let mut events = game.events.len();
    for _ in 0..STEP_BOUND {
        if ended(&run) {
            break;
        }
        let error = game.step().error;
        let mut run = run.lock().expect("draft lock");
        if run.stop.is_some() {
            break;
        }
        if let Some(error) = error {
            run.end(DraftStop::Failed {
                reason: error.to_string(),
            });
        } else if let Some(reason) =
            activity_stop(&observation).or_else(|| run.known(&game.state, None).err())
        {
            // The step did something that no question came after: a roll, a draw.
            run.end(DraftStop::Stopped { reason });
        } else {
            settled = Some(game.state.clone());
            if game.events[events..]
                .iter()
                .any(|event| event == TACTICAL_COMPLETE)
            {
                run.complete();
            }
        }
        events = game.events.len();
    }
    drop(game);
    let mut run = run.lock().expect("draft lock");
    run.end(DraftStop::Stopped {
        reason: StopReason::StepLimit,
    });
    let stop = run.stop.clone().expect("the run ended");

    let context = FactsContext {
        content: base.content,
        sources: base.sources,
        galaxy: base.galaxy,
    };
    let closed = |state: &GameState| {
        project_session_update(state, &viewer, None, base.tiles, Some(&context))
    };
    let shown = |offer: &Offer, open: bool| {
        if !open {
            return closed(&offer.state);
        }
        project_session_update(
            &offer.state,
            &viewer,
            Some((&offer.choice, DRAFT_NONCE)),
            base.tiles,
            Some(&context),
        )
    };
    let first = |offer: &Option<Offer>| offer.as_ref().map(|offer| shown(offer, true));
    let update = match (&stop, &settled, &run.last) {
        // The board after the action.
        (DraftStop::Complete, Some(state), _) => closed(state),
        (_, _, Some(offer)) => shown(offer, stop == DraftStop::Open),
        _ => closed(&base.game.state),
    };
    DraftOutcome {
        update,
        movement: first(&run.movement),
        invasion: first(&run.invasion),
        production: first(&run.production),
        consumed: run.next,
        stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::TacticalFacts;
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

    /// A question of seat `player` with this subtype.
    fn asked(player: &str, subtype: &str, options: Vec<ChoiceOption>) -> Choice {
        use ti4_engine::decision_context::{DecisionContext, DecisionSource};
        Choice::new(PlayerId::new(player), "a question", options).contextualized(
            DecisionContext::new(
                PlayerId::new(player),
                DecisionSource::Rule("test".to_owned()),
                subtype,
                Phase::Action,
                1,
            ),
        )
    }

    #[test]
    fn a_script_answers_by_id_and_by_step_and_stops_at_another_question() {
        let seat = PlayerId::new("a");
        let script = [
            DraftStep::choose("tactical"),
            DraftStep::Step(PlanStep::DoneMoving),
            DraftStep::choose("use"),
            DraftStep::Step(PlanStep::DoneProducing),
        ];
        let id = |answer: &ScriptAnswer| match answer {
            ScriptAnswer::Answer { option, next, .. } => (option.id.clone(), *next),
            other => panic!("no answer: {other:?}"),
        };
        let menu = asked("a", "action_menu", vec![ChoiceOption::new("tactical", "x")]);
        assert_eq!(
            id(&script_answer(&script, 0, &menu, &seat)),
            ("tactical".to_owned(), 1)
        );
        // An answer by id that the game does not offer refuses the script; a step is not
        // answered by id.
        let other = asked("a", "action_menu", vec![ChoiceOption::new("pass", "x")]);
        assert!(matches!(
            script_answer(&script, 0, &other, &seat),
            ScriptAnswer::Mismatch { step: 0, .. }
        ));
        assert!(matches!(
            script_answer(&script, 1, &menu, &seat),
            ScriptAnswer::Interrupted { .. }
        ));
        let moving = asked(
            "a",
            "movement_step",
            vec![ChoiceOption::new("done_moving", "x")],
        );
        assert_eq!(
            id(&script_answer(&script, 1, &moving, &seat)),
            ("done_moving".to_owned(), 2)
        );
        // The pause is passed while entries are left, also before an answer by id.
        let pause = asked(
            "a",
            "mid_action_pause",
            vec![ChoiceOption::new(
                ti4_engine::game::CONTINUE_ACTION_ID,
                ti4_engine::game::CONTINUE_ACTION_KIND,
            )],
        );
        assert_eq!(
            id(&script_answer(&script, 2, &pause, &seat)),
            (ti4_engine::game::CONTINUE_ACTION_ID.to_owned(), 2)
        );
        assert_eq!(script_answer(&script, 4, &pause, &seat), ScriptAnswer::Done);
        // A question of another seat is never answered.
        let theirs = asked("c", "ability", vec![ChoiceOption::new("use", "x")]);
        assert!(matches!(
            script_answer(&script, 2, &theirs, &seat),
            ScriptAnswer::Interrupted { .. }
        ));
        let ability = asked("a", "ability", vec![ChoiceOption::new("use", "x")]);
        assert_eq!(
            id(&script_answer(&script, 2, &ability, &seat)),
            ("use".to_owned(), 3)
        );
        // The game ended the production itself: the end of the script is left out.
        assert_eq!(script_answer(&script, 3, &menu, &seat), ScriptAnswer::Done);
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
        script.push(step(PlanStep::Move {
            origin: ship.origin.clone(),
            unit: ship.unit.clone(),
            damaged: ship.damaged,
            gravity_drive: plan.gravity_drive,
            ionian: plan.ionian,
        }));
        script.push(step(PlanStep::DoneLoading));
        script.push(step(PlanStep::DoneMoving));
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

    fn step(step: PlanStep) -> DraftStep {
        step.into()
    }

    fn choose(id: &str) -> DraftStep {
        DraftStep::choose(id)
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
        // A draft has the facts of a live update, from the same place.
        let content = ContentStore::embedded();
        assert!(
            menu.update.tactical.is_none(),
            "a menu has no step to stage"
        );
        assert!(menu.update.exploration.is_some());
        assert!(menu.update.action_cards.is_some());
        assert!(menu.update.secret_objectives.is_some());
        assert_eq!(
            menu.update.score,
            Some(crate::score::project_score_facts(&game.state, content))
        );
        assert_eq!(
            menu.update.agendas,
            Some(crate::agendas::project_agenda_facts(&game.state))
        );

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

        script.push(step(PlanStep::Move {
            origin: ship.origin.clone(),
            unit: ship.unit.clone(),
            damaged: ship.damaged,
            gravity_drive: plan.gravity_drive,
            ionian: plan.ionian,
        }));
        script.push(step(PlanStep::DoneLoading));
        script.push(step(PlanStep::DoneMoving));
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
            step(PlanStep::Move {
                origin: "nowhere".to_owned(),
                unit: "carrier".to_owned(),
                damaged: false,
                gravity_drive: false,
                ionian: false,
            }),
            step(PlanStep::DoneMoving),
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

    /// The late game of the scenarios: seat A has the turn, two space docks and many fleets.
    fn late_game() -> (Game<'static>, Vec<BoardTileView>) {
        let content = ContentStore::embedded();
        let ids: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .map(|name| PlayerId::new(*name))
            .collect();
        let loader = crate::maps::TemplateLoader::load().expect("the templates load");
        let template = crate::maps::default_template_for(content, &loader, 8, POK)
            .expect("a template for eight");
        let (mut state, mut galaxy) =
            crate::map::create_game_with_template(content, &ids, 42, Some(&template))
                .expect("a game");
        crate::scenario::apply(
            crate::scenario::LATE_GAME,
            &mut state,
            &mut galaxy,
            content,
            POK,
        )
        .expect("the scenario applies");
        let tiles = crate::map::build_board_tiles(content, &galaxy);
        let mut game = Game::with_seeded_random(state, content, 42)
            .with_sources(POK)
            .with_galaxy(galaxy);
        game.table = Table::with_default(Box::new(SeededRandom::new(42)));
        (game, tiles)
    }

    /// The system of the second space dock, the free system with two planets, a system with a
    /// fleet of seat C, and the system north of the free one (`scenario::at`).
    const DOCK: &str = "72";
    const FREE: &str = "38";
    const ENEMY: &str = "32";
    const NORTH: &str = "63";

    #[test]
    fn a_draft_goes_on_to_the_production_and_ends_with_the_action() {
        let (game, tiles) = late_game();
        let before = game.state.clone();
        let mut script = vec![choose("tactical"), choose(DOCK), step(PlanStep::DoneMoving)];

        // AI Development Algorithm asks before the production: the question is open.
        let asked = run(&game, &tiles, &script);
        assert_eq!(asked.stop, DraftStop::Open);
        assert_eq!(subtype(&asked.update), "exhaust_for_production_discount");
        assert_eq!(asked.consumed, 3);
        script.push(choose("decline"));

        let open = run(&game, &tiles, &script);
        assert_eq!(open.stop, DraftStop::Open);
        assert_eq!(subtype(&open.update), "produce_unit");
        assert_eq!(open.production.as_ref(), Some(&open.update));
        assert_eq!(open.invasion, None, "nothing can land");
        let Some(TacticalFacts::Production(facts)) = &open.update.tactical else {
            panic!("no production facts");
        };
        let cruiser = facts
            .units
            .iter()
            .find(|unit| unit.unit == "cruiser2" && unit.unavailable.is_none())
            .expect("a cruiser");
        let produce = step(PlanStep::Produce {
            unit: cruiser.unit.clone(),
        });

        // A build that is not paid is refused at its payment.
        let mut unpaid = script.clone();
        unpaid.extend([produce.clone(), step(PlanStep::DoneProducing)]);
        let refused = run(&game, &tiles, &unpaid);
        assert!(
            matches!(refused.stop, DraftStop::Refused { step: 5, .. }),
            "{:?}",
            refused.stop
        );
        assert_eq!(refused.production, open.production);

        // Every planet pays; the game takes what it needs and the rest is left out.
        script.push(produce);
        script.extend(facts.planets.iter().map(|planet| {
            step(PlanStep::Pay {
                source: format!("exhaust|{}", planet.planet),
            })
        }));
        script.push(step(PlanStep::DoneProducing));
        let produced = run(&game, &tiles, &script);
        assert_eq!(produced.stop, DraftStop::Complete);
        assert_eq!(produced.consumed, script.len());
        assert_eq!(produced.production, open.production);
        assert_eq!(produced.update.pending_choice, None);
        let there = &produced.update.view.board.systems[&ti4_model::id::SystemId::new(DOCK)].units;
        let cruisers = |units: &[crate::view::PlacedUnitView]| {
            units
                .iter()
                .filter(|unit| unit.unit_type.as_str() == cruiser.unit)
                .count()
        };
        let was = &open.update.view.board.systems[&ti4_model::id::SystemId::new(DOCK)].units;
        assert_eq!(cruisers(there), cruisers(was) + 1);

        assert_eq!(game.state, before, "a draft changes nothing");
    }

    #[test]
    fn a_draft_ends_where_a_landing_draws_a_card() {
        let (game, tiles) = late_game();
        let mut script = vec![choose("tactical"), choose(FREE)];
        let movement = run(&game, &tiles, &script);
        let Some(TacticalFacts::Movement(facts)) = &movement.update.tactical else {
            panic!("no movement facts");
        };
        let ship = facts
            .ships
            .iter()
            .find(|ship| ship.origin == NORTH && ship.capacity > 0 && !ship.loads.is_empty())
            .expect("a carrier in the north");
        let plan = ship.r#move.as_ref().expect("it arrives");
        let cargo = ship
            .loads
            .iter()
            .map(|index| &facts.cargo[*index])
            .find(|cargo| cargo.unit.contains("infantry"))
            .expect("infantry to load");
        script.extend([
            step(PlanStep::Move {
                origin: ship.origin.clone(),
                unit: ship.unit.clone(),
                damaged: ship.damaged,
                gravity_drive: plan.gravity_drive,
                ionian: plan.ionian,
            }),
            step(PlanStep::Load {
                origin: ship.origin.clone(),
                pickup_system: cargo.system.clone(),
                unit: cargo.unit.clone(),
                source: cargo.source.clone(),
                damaged: cargo.damaged,
                galvanized: cargo.galvanized,
            }),
            step(PlanStep::DoneLoading),
            step(PlanStep::DoneMoving),
        ]);

        // The landings are open, staged against their first question.
        let landing = run(&game, &tiles, &script);
        assert_eq!(landing.stop, DraftStop::Open);
        assert_eq!(landing.consumed, script.len());
        assert_eq!(subtype(&landing.update), "commit_ground_forces");
        assert_eq!(landing.invasion.as_ref(), Some(&landing.update));
        let Some(TacticalFacts::Invasion(facts)) = &landing.update.tactical else {
            panic!("no invasion facts");
        };
        assert_eq!(facts.active, FREE);
        let planet = facts
            .planets
            .iter()
            .find(|planet| !planet.lands.is_empty())
            .expect("a planet to land on");
        let force = &facts.forces[planet.lands[0]];

        // Land nothing: the action is over, and the draft with it.
        let mut nothing = script.clone();
        nothing.push(step(PlanStep::DoneCommitting));
        assert_eq!(run(&game, &tiles, &nothing).stop, DraftStop::Complete);

        // A planet that nobody explored: the card that the landing draws ends the draft.
        script.extend([
            step(PlanStep::Land {
                planet: planet.planet.clone(),
                unit: force.unit.clone(),
                damaged: force.damaged,
                from_system: force.from_system.clone(),
                from_planet: force.from_planet.clone(),
            }),
            step(PlanStep::DoneCommitting),
        ]);
        let landed = run(&game, &tiles, &script);
        assert_eq!(
            landed.stop,
            DraftStop::Stopped {
                reason: StopReason::Uncertainty
            }
        );
        assert_eq!(landed.invasion, landing.invasion);
        assert_eq!(landed.update.pending_choice, None);
    }

    #[test]
    fn a_draft_ends_before_a_space_combat() {
        let (game, tiles) = late_game();
        let script = [
            choose("tactical"),
            choose(ENEMY),
            step(PlanStep::DoneMoving),
        ];
        let outcome = run(&game, &tiles, &script);
        // The guns of the system roll before the combat is reached.
        assert!(
            matches!(
                outcome.stop,
                DraftStop::Stopped {
                    reason: StopReason::Uncertainty | StopReason::UnsupportedSegment
                }
            ),
            "{:?}",
            outcome.stop
        );
        assert!(outcome.movement.is_some());
        assert_eq!(outcome.invasion, None);
        assert_eq!(outcome.update.pending_choice, None);
    }
}
