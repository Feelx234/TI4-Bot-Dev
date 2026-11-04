//! The asker of the wrapper: the questions of the live loop go to the game's [`Table`].

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::driver::live::LiveError;
use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::state::CoreState;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, StrategyCardId};

use super::{AftermathObserver, GameError, StrategicObserver};
use crate::core_sync::{Ctx, materialize};
use crate::live::{Asker, Question, Stop, StepOut};
use crate::observed::Observed;
use crate::wire::choice::{IllegalChoice, Table};

/// Puts each question to the table with the public view of the position it is asked in, and keeps what went wrong.
pub(super) struct TableAsker<'t> {
    table: &'t mut Table,
    content: &'t ContentStore,
    sources: SourceSet,
    galaxy: Option<&'t Galaxy>,
    cx: &'t Ctx,
    base: &'t ti4_model::state::GameState,
    strategic: Option<&'t StrategicObserver>,
    aftermath: Option<&'t AftermathObserver>,
    /// A position that could not be shown to the decider (a defect of the codec, not of the decider).
    defect: Option<String>,
    /// The question that had no option.
    empty: Option<(PlayerId, String)>,
    /// A question showed a seat another seat's cards.
    pub(super) hidden: bool,
    participation: Option<&'t super::Participation>,
}

impl<'t> TableAsker<'t> {
    /// Offer only the reactions this filter lets a seat take (a planning fork).
    pub(super) const fn with_participation(mut self, p: Option<&'t super::Participation>) -> Self {
        self.participation = p;
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        table: &'t mut Table,
        content: &'t ContentStore,
        sources: SourceSet,
        galaxy: Option<&'t Galaxy>,
        cx: &'t Ctx,
        base: &'t ti4_model::state::GameState,
        strategic: Option<&'t StrategicObserver>,
        aftermath: Option<&'t AftermathObserver>,
    ) -> Self {
        Self { table, content, sources, galaxy, cx, base, strategic, aftermath, defect: None, empty: None, hidden: false, participation: None }
    }

    /// The error a stopped live call is reported as.
    pub(super) fn error_of(&self, stop: Stop, label: &str) -> GameError {
        if let Some(why) = &self.defect {
            return GameError::Core(why.clone());
        }
        match stop {
            Stop::Asker(error) => GameError::IllegalChoice(error),
            Stop::Blocked => match &self.empty {
                Some((player, prompt)) => GameError::IllegalChoice(IllegalChoice::NoOptions { player: player.clone(), prompt: prompt.clone() }),
                None => GameError::Core(format!("{label}: the asker refused to go on")),
            },
            Stop::Illegal => GameError::Core(format!("{label}: the answer is no option of the question")),
            Stop::Exhausted => GameError::Core(format!("{label}: no answer to give")),
            Stop::Live(LiveError::Unknown(why)) => {
                let mut cause = format!("{why:?}");
                cause.truncate(160);
                crate::product::note_gap(label, &cause);
                GameError::Core(format!("{label}: {cause}"))
            }
            Stop::Live(other) => GameError::Core(format!("{label}: {other:?}")),
            Stop::NoLiveFn(arm) => GameError::Core(format!("the arm {} has no live function", crate::live::arms::name(arm))),
        }
    }

    /// After an arm ran: the handoff of the movement to its aftermath is told to the observer (the position it hands over).
    pub(super) fn arm_done(&mut self, arm: Arm, _out: &StepOut, core: &CoreState) {
        if arm == Arm::Tactical
            && core.pending().window(ti4_engine_core::state::WindowKind::Aftermath).is_some()
            && let Some(observer) = self.aftermath
            && let Ok(state) = materialize::materialize(core, self.cx, self.base)
        {
            observer(&state, self.content, self.sources);
        }
    }

    fn strategic_action_of(&self, core: &CoreState, id: &str, player: &PlayerId) -> Option<StrategyCardId> {
        let rest = id.strip_prefix(crate::strategy::STRATEGIC_ACTION_ID)?;
        if let Some(card) = rest.strip_prefix('|') {
            return Some(StrategyCardId::new(card));
        }
        if !rest.is_empty() {
            return None;
        }
        // A bare `strategic` names the one unused card the seat holds.
        let state = materialize::materialize(core, self.cx, self.base).ok()?;
        let seat = state.player(player)?;
        match seat.unused_strategy_cards().as_slice() {
            [only] => Some((*only).clone()),
            _ => None,
        }
    }
}

impl Asker for TableAsker<'_> {
    fn ask(&mut self, q: &Question<'_>) -> Result<String, Stop> {
        let rendered = &q.rendered;
        if rendered.options.is_empty() {
            self.empty = Some((rendered.player.clone(), rendered.prompt.clone()));
            return Err(Stop::Blocked);
        }
        if rendered.context.as_ref().is_some_and(|c| c.subtype.contains("show_") || c.subtype.contains("reveal") || c.subtype.contains("take_from_revealed")) {
            self.hidden = true;
        }
        // A planning fork: an optional reaction of a seat the filter keeps out is declined without a question (and without a journal entry),
        // and the table's observer is told the window passed that seat by.
        if let (Some(filter), Some(context)) = (self.participation, rendered.context.as_ref())
            && context.optional
            && matches!(context.source, crate::wire::decision_context::DecisionSource::Reaction(_))
            && !filter(&super::Participant { owner: rendered.player.clone(), optional: true })
            && let Some(decline) = rendered.options.iter().find(|o| o.is_decline())
        {
            if let Some(trigger) = &context.trigger {
                let relation = if trigger.relation == "after" { crate::ties::timing::Relation::After } else { crate::ties::timing::Relation::When };
                if let Ok(state) = materialize::materialize(q.asking.nested.as_ref().map(|(_, at)| &**at).unwrap_or(q.core), self.cx, self.base) {
                    self.table.window_skipped(&rendered.player, &trigger.event_type, relation, &state);
                }
            }
            return Ok(decline.id.clone());
        }
        // The one way left to pay is not a choice either (`production::pay_step` of the string): the payer is told, nothing is journaled.
        if rendered.options.len() == 1
            && rendered.context.as_ref().is_some_and(|c| c.subtype == "pay_resources")
            && let Some(only) = self.table.auto_resolve(rendered, "it was the only way left to pay")
        {
            return Ok(only.id);
        }
        // The last card of the draft is not a choice: the picker takes it and is told so (never journaled).
        if q.asking.choice.ask == ti4_engine_core::choice::Ask::Strategy(ti4_engine_core::choice::strategy::Ask::Draft)
            && rendered.options.len() == 1
            && let Some(only) = self.table.auto_resolve(rendered, "only one strategy card left")
        {
            return Ok(only.id);
        }
        // The position the question is asked in: a nested question's own, else the live core.
        let at = q.asking.nested.as_ref().map(|(_, at)| &**at).unwrap_or(q.core);
        let state = match materialize::materialize(at, self.cx, self.base) {
            Ok(state) => state,
            Err(e) => {
                self.defect = Some(format!("the position cannot be shown to the decider: {e}"));
                return Err(Stop::Blocked);
            }
        };
        let observed = Observed::new(&state, self.content, self.sources, self.galaxy);
        match self.table.ask_seeing(rendered, &observed) {
            Ok(option) => {
                if let Some(observer) = self.strategic
                    && let Some(card) = self.strategic_action_of(q.core, &option.id, &rendered.player)
                {
                    observer(&rendered.player, &card);
                }
                Ok(option.id)
            }
            Err(error) => Err(Stop::Asker(error)),
        }
    }
}
