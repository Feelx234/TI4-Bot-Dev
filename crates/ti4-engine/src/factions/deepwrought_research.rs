//! Doctor Carrina's research window, shared by every route that researches a technology.
//!
//! > When another player researches a technology: You may exhaust this card to allow that player
//! > to ignore 1 prerequisite; if they do, you may place 1 infantry from your reinforcements into
//! > coexistence on a non-home planet they control.
//!
//! "Research" is a keyword, so the window opens for every route that researches (the Technology
//! strategy card, action cards, Sardakk's N'orr Supremacy, Specialist Compounds, Visionaria
//! Select, Ta Zern). A gain is not a research and never opens it. The routes differ in how they ask
//! and pay, but they share this one pair of calls:
//!
//! * [`open`] runs before the researcher lists what they can take (the waiver widens that list).
//! * [`settle`] runs after the route finished, and places the infantry.
//!
//! Operator ruling 2026-10-07: "if they do" means the researcher actually needed the ignored
//! prerequisite. The agent is therefore offered only when the waiver opens a technology that was
//! closed, and the placement follows only when a technology gained was one of those. (The card's
//! note says a prerequisite may be ignored even when it is already met; the ruling overrides it.)

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlayerId, TechnologyId};
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, IllegalChoice, Observed, Table};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::timing::TimingContext;

use super::deepwrought::{
    AGENT, agent_holders, arm_waiver, disarm_waiver, infantry_targets, place_into_coexistence,
};

/// What a route needs to put a question: the state, the corpus and the game's one table.
pub(crate) struct Host<'a> {
    state: &'a mut GameState,
    content: &'a ContentStore,
    sources: SourceSet,
    galaxy: Option<&'a Galaxy>,
    table: &'a mut Table,
}

impl<'a> Host<'a> {
    pub(crate) fn new(
        state: &'a mut GameState,
        content: &'a ContentStore,
        sources: SourceSet,
        galaxy: Option<&'a Galaxy>,
        table: &'a mut Table,
    ) -> Self {
        Self {
            state,
            content,
            sources,
            galaxy,
            table,
        }
    }

    /// The same four things, borrowed from a timing context.
    pub(crate) fn of(context: &'a mut TimingContext<'_>) -> Self {
        Self {
            state: &mut *context.state,
            content: context.content,
            sources: context.sources,
            galaxy: context.galaxy,
            table: &mut *context.table,
        }
    }

    fn put(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.table.ask_seeing(
            choice,
            &Observed::new(&*self.state, self.content, self.sources, self.galaxy),
        )
    }
}

/// An open Carrina window: who armed the waiver, what the researcher could take without it, and
/// what they owned when the research began.
#[derive(Debug, Clone, Default)]
pub(crate) struct Window {
    holder: Option<PlayerId>,
    plain: BTreeSet<TechnologyId>,
    before: BTreeSet<TechnologyId>,
}

fn owned(state: &GameState, player: &PlayerId) -> BTreeSet<TechnologyId> {
    state
        .player(player)
        .map(|seat| seat.technologies.clone())
        .unwrap_or_default()
}

/// The first half, asked of each holder in seating order as `researcher`'s research begins.
/// `relevant` narrows which technologies this route could take (a unit upgrade only, say); the
/// agent is offered only when the waiver opens one of them that was closed.
///
/// # Errors
/// An answer the holder could not give.
pub(crate) fn open(
    host: &mut Host<'_>,
    researcher: &PlayerId,
    relevant: &dyn Fn(&ContentStore, &TechnologyId) -> bool,
) -> Result<Window, IllegalChoice> {
    let mut window = Window {
        before: owned(host.state, researcher),
        ..Window::default()
    };
    for holder in agent_holders(host.state, researcher) {
        let takeable = |state: &GameState| -> BTreeSet<TechnologyId> {
            crate::technology::researchable(state, host.content, host.sources, researcher)
                .into_iter()
                .filter(|tech| relevant(host.content, tech))
                .collect()
        };
        let plain = takeable(host.state);
        arm_waiver(host.state, researcher, &holder);
        let waived = takeable(host.state);
        disarm_waiver(host.state, researcher);
        if waived.is_subset(&plain) {
            continue; // ignoring a prerequisite would open nothing: the agent would be spent for nothing
        }
        let choice = Choice::new(
            holder.clone(),
            format!("Doctor Carrina: exhaust to let {researcher} ignore 1 prerequisite"),
            vec![
                ChoiceOption::labelled("use".to_owned(), "leader", "exhaust the agent".to_owned()),
                ChoiceOption::decline(),
            ],
        )
        .contextualized(DecisionContext::new(
            holder.clone(),
            DecisionSource::Content(AGENT.to_owned()),
            "deepwrought_agent_exhaust",
            host.state.phase,
            host.state.round,
        ));
        if host.put(&choice)?.is_decline()
            || !crate::leaders::exhaust(host.state, &holder, &LeaderId::new(AGENT))
        {
            continue;
        }
        // Exhausted for another seat's research: a promise to use this agent for them is kept here.
        crate::diplomacy::evaluate_event(
            host.state,
            &crate::diplomacy::DiplomacyEventContext::LeaderUsedFor {
                user: holder.clone(),
                leader: AGENT.to_owned(),
                beneficiary: researcher.clone(),
            },
        )
        .expect("validated diplomacy promises settle deterministically");
        arm_waiver(host.state, researcher, &holder);
        window.holder = Some(holder);
        window.plain = plain;
        return Ok(window);
    }
    Ok(window)
}

/// The research is over. Closes the waiver and settles the second half: when a technology was
/// gained that the researcher could not have taken without the waiver, the holder may place an
/// infantry into coexistence on a non-home planet the researcher controls. A research that gained
/// nothing undoes the use (the agent is readied again); a research that needed no waiver leaves
/// the agent spent and gives no placement.
///
/// # Errors
/// An answer the holder could not give.
pub(crate) fn settle(
    host: &mut Host<'_>,
    researcher: &PlayerId,
    window: Window,
    finished_cleanly: bool,
) -> Result<(), IllegalChoice> {
    if window.holder.is_none() {
        return Ok(());
    }
    let Some(holder) = disarm_waiver(host.state, researcher) else {
        return Ok(());
    };
    let gained: Vec<TechnologyId> = owned(host.state, researcher)
        .difference(&window.before)
        .cloned()
        .collect();
    if gained.is_empty() {
        crate::leaders::ready(host.state, &holder, &LeaderId::new(AGENT));
        return Ok(());
    }
    if !finished_cleanly || gained.iter().all(|tech| window.plain.contains(tech)) {
        return Ok(()); // no prerequisite was actually ignored
    }
    let targets = infantry_targets(host.state, host.content, host.sources, &holder, researcher);
    if targets.is_empty() {
        return Ok(());
    }
    let mut options: Vec<ChoiceOption> = targets
        .iter()
        .map(|(system, planet)| {
            ChoiceOption::labelled(
                format!("{system}|{planet}"),
                "planet",
                format!("place an infantry on {planet} in {system}"),
            )
        })
        .collect();
    options.push(ChoiceOption::decline());
    let choice = Choice::new(
        holder.clone(),
        format!(
            "Doctor Carrina: place an infantry into coexistence on a planet {researcher} controls"
        ),
        options,
    )
    .contextualized(DecisionContext::new(
        holder.clone(),
        DecisionSource::Content(AGENT.to_owned()),
        "deepwrought_agent_place",
        host.state.phase,
        host.state.round,
    ));
    let answer = host.put(&choice)?;
    if let Some((system, planet)) = targets
        .into_iter()
        .find(|(system, planet)| answer.id == format!("{system}|{planet}"))
    {
        place_into_coexistence(
            host.state,
            host.content,
            host.sources,
            &holder,
            &system,
            &planet,
        );
    }
    Ok(())
}

/// Every technology counts: the common case for a route that takes any research.
pub(crate) fn any(_: &ContentStore, _: &TechnologyId) -> bool {
    true
}

/// Only unit upgrades count (Reveal Prototype, N'orr Supremacy).
pub(crate) fn unit_upgrades(content: &ContentStore, tech: &TechnologyId) -> bool {
    crate::technology::is_unit_upgrade(content, tech)
}

/// Run a whole research route that is a plain function of the timing context: open, run, settle.
/// A holder's illegal answer ends the route (the same convention as `action_cards::pick`).
pub(crate) fn around(
    context: &mut TimingContext<'_>,
    researcher: &PlayerId,
    relevant: &dyn Fn(&ContentStore, &TechnologyId) -> bool,
    route: impl FnOnce(&mut TimingContext<'_>),
) {
    let Ok(window) = open(&mut Host::of(context), researcher, relevant) else {
        return;
    };
    route(context);
    let _ = settle(&mut Host::of(context), researcher, window, true);
}
