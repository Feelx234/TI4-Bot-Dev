//! The display facts of a question asked inside an emission (a reaction window's pick, an ability's question) and the tables a core half
//! needs for one position. Copied out of the legacy `dual_window.rs`: these are render code (the nested rendering of the live path), the
//! rest of that module is the string-against-core lockstep and stays in the legacy crate.

use ti4_engine_core::choice::Choice as CoreChoice;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::generated::IdStr;

use crate::core_sync::Ctx;
use crate::render::Render;
use crate::wire::choice::Choice;

/// One push to `Game::events` as the recorder saw it (package P1.1b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// A plain event (`Game::emit`, `Live::emit`), the text as pushed: `NAME` or `NAME:argument`.
    Plain(String),
    /// The name of a typed event mirrored from the timing log (`mirror_timing_log`).
    Mirrored(String),
}

impl Pushed {
    /// The text as `Game::events` holds it.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Plain(t) | Self::Mirrored(t) => t,
        }
    }
}


/// The display facts of a question asked inside an emission (a reaction window's pick, an ability's question):
/// the trigger of its window, the `active` detail of a pick, the battle or invasion the position is in. What
/// the string builds from the event in hand and `TimingContext::ask_seeing`.
pub fn complete_nested(
    r: &Render,
    choice: &CoreChoice,
    facts: &ti4_engine_core::kernel::NestedFacts,
    pos: &NestedPos,
    core: &ti4_engine_core::state::CoreState,
    rendered: &mut Choice,
) {
    if let Some(ctx) = rendered.context.take() {
        // The window's own pick carries its trigger; a question an ability asks carries one only when the ability
        // puts it there (`with_trigger_of`).
        let ctx = if with_trigger_of(choice) {
            let trigger = crate::kernel_sync::convert::trigger_text(
                r,
                &ti4_engine_core::kernel::trigger::Trigger::from_event(&facts.event, facts.relation, &facts.chain),
            );
            // A decision raised by an agenda being revealed or resolved carries the printed agenda as display-only
            // `details.agenda` (`Table::ask_seeing`), unless the producer attached it already.
            if let Some(agenda) = trigger.agenda.as_deref()
                && !rendered.details.contains_key("agenda")
            {
                rendered.details.insert(
                    "agenda".to_owned(),
                    crate::ties::strategy_cards::agenda_details(ti4_content::ContentStore::embedded(), agenda),
                );
            }
            ctx.with_trigger(trigger)
        } else {
            ctx
        };
        rendered.context = Some(nested_position(choice, ctx, pos));
    }
    crate::render::leaders::complete_round_agent(r, choice, rendered);
    if matches!(choice.ask, ti4_engine_core::choice::Ask::Reaction(ti4_engine_core::choice::reaction::Ask::Legionnaire { .. })) {
        // `strategy_cards::commodity_standing`: display only, the asked seat's commodities, their printed limit and trade goods.
        if let Some(p) = core.players().by_seat(choice.actor) {
            rendered.details.insert(
                "commodities".to_owned(),
                serde_json::json!({"have": p.econ.commodities, "max": ti4_engine_core::rules::strategy::commodity_limit(p.faction), "trade_goods": p.econ.trade_goods}),
            );
        }
    }
    if is_window_pick(choice) {
        slot_options(r, choice, facts, rendered);
    }
    if is_window_pick(choice) && pos.action_phase {
        if let Some(active) = &pos.active {
            rendered.details.insert(crate::ties::timing::ACTIVE_DETAIL.to_owned(), active.clone().into());
        }
    }
}

/// The few facts of the position a nested question's display reads (the string `GameState`'s `phase`, `active`, `active_space_combat` and
/// `active_invasion`; the core position's `meta` and `transient`): the replay path takes them from the string, the live path from the core.
#[derive(Debug, Clone, Default)]
pub struct NestedPos {
    /// The action phase is on (a reaction window's pick then names the active seat).
    pub action_phase: bool,
    /// The active seat's player id.
    pub active: Option<String>,
    /// The system of the space combat in progress.
    pub combat_system: Option<ti4_model::id::SystemId>,
    /// The system and sequence number of the invasion in progress.
    pub invasion: Option<(ti4_model::id::SystemId, u64)>,
}

impl NestedPos {
    /// From the string position.
    #[must_use]
    pub fn of_gs(gs: &GameState) -> Self {
        Self {
            action_phase: gs.phase == ti4_model::state::Phase::Action,
            active: gs.active.as_ref().map(ToString::to_string),
            combat_system: gs.active_space_combat.as_ref().map(|(system, _, _)| system.clone()),
            invasion: gs.active_invasion.as_ref().map(|a| (a.system.clone(), a.seq)),
        }
    }

    /// From the core position (the live path): `r` names the seats.
    #[must_use]
    pub fn of_core(r: &Render, core: &ti4_engine_core::state::CoreState) -> Self {
        use crate::generated::IdStr;
        let system = |s: ti4_engine_core::generated::System| ti4_model::id::SystemId::new(s.as_str());
        Self {
            action_phase: core.meta().phase == ti4_engine_core::state::Phase::Action,
            active: core.meta().active.map(|a| r.seat_text(a).to_owned()),
            combat_system: core.transient().combat.active_space_combat.map(|(s, _, _)| system(s)),
            invasion: core.transient().invasion.active.as_ref().map(|a| (system(a.system), a.seq)),
        }
    }
}

/// Whether a nested question carries the trigger of its window in its context: the window's own pick, and the
/// questions whose string ability builds one (`DecisionTrigger::from_event`).
fn with_trigger_of(choice: &CoreChoice) -> bool {
    use ti4_engine_core::choice::reaction::Ask as RAsk;
    match choice.ask {
        ti4_engine_core::choice::Ask::Reaction(RAsk::G(g)) => g.carries_trigger(),
        ti4_engine_core::choice::Ask::Reaction(RAsk::Act(a)) => a.carries_trigger(),
        ti4_engine_core::choice::Ask::Reaction(a) => !matches!(a, RAsk::Legionnaire { .. } | RAsk::Cavalry | RAsk::Assimilate | RAsk::Watcher { .. } | RAsk::Salvage | RAsk::Indoctrination { .. } | RAsk::IcarusDrive | RAsk::AgentShip { .. } | RAsk::Edict),
        ti4_engine_core::choice::Ask::Cards(ti4_engine_core::choice::cards::Ask::PlayCard { .. }) => true,
        _ => false,
    }
}

/// The battle and invasion a nested question is asked in. The string puts a payment an ability asks (`pay_seeing`, the
/// table's own `ask_seeing`) in the position of an invasion only, never of a battle; the other nested questions
/// (`TimingContext::ask_seeing`) name both.
pub fn nested_position(choice: &CoreChoice, ctx: crate::wire::decision_context::DecisionContext, pos: &NestedPos) -> crate::wire::decision_context::DecisionContext {
    if matches!(choice.ask, ti4_engine_core::choice::Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
        ctx.about_invasion_in(pos.invasion.clone())
    } else {
        ctx.about_battle_in(pos.combat_system.clone()).about_invasion_in(pos.invasion.clone())
    }
}

/// Whether a nested question is the window's own pick (the one that carries the `active` detail of the action phase).
fn is_window_pick(choice: &CoreChoice) -> bool {
    matches!(choice.ask, ti4_engine_core::choice::Ask::Reaction(ti4_engine_core::choice::reaction::Ask::Window { .. }))
}

/// The options of a reaction window that are the standing card slot show what the seat could play: the label
/// ("Play X", "Choose an action card…") and the card payload (`reaction_card_options` and `pick_with_context`).
fn slot_options(
    r: &Render,
    choice: &CoreChoice,
    facts: &ti4_engine_core::kernel::NestedFacts,
    rendered: &mut Choice,
) {
    use ti4_engine_core::choice::reaction::{AbilityKind, Opt as ROpt};
    let content = ti4_content::ContentStore::embedded();
    for (i, o) in choice.opts.iter().enumerate() {
        let ti4_engine_core::choice::Opt::Reaction(ROpt::Ability(id)) = o else { continue };
        if id.kind != AbilityKind::Reaction {
            continue;
        }
        let cards = &facts.offered;
        let name_of = |c: &ti4_engine_core::generated::ActionCard| crate::ties::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str()));
        let mut names: Vec<String> = Vec::new();
        for c in cards.iter() {
            let n = name_of(c);
            if !names.contains(&n) {
                names.push(n);
            }
        }
        let Some(option) = rendered.options.get_mut(i) else { continue };
        option.payload.insert("card_names".to_owned(), serde_json::json!(names));
        if let Some(first) = cards.first().filter(|f| cards.iter().all(|c| name_of(c) == name_of(f))) {
            let name = name_of(first);
            option.label = format!("Play {name}");
            option.payload.insert("card".to_owned(), first.as_str().into());
            option.payload.insert("card_name".to_owned(), name.into());
        } else if cards.len() > 1 {
            option.label = "Choose an action card…".to_owned();
        }
    }
    let _ = r;
}

/// The tables a core half needs, for one position. `None`: more than 16 owners.
pub fn tables(gs: &GameState) -> Option<(Ctx, Render)> {
    tables_with(gs, &[])
}

/// [`tables`] with owners the position does not hold yet in the table (the neutral owner of a card that places units for it).
pub fn tables_with(gs: &GameState, extra: &[&str]) -> Option<(Ctx, Render)> {
    crate::core_sync::unsupported::check(gs).ok()?;
    let cx = Ctx::with_extra(gs, extra).ok()?;
    let r = Render::new(cx.owners.ids().to_vec());
    Some((cx, r))
}

/// [`tables`] for a window about these sides. The neutral force has no seat in the game, and its owner is read from the board: once the
/// last of its units has fallen (the end announcement of the fight that wiped it out) it would have none, so a side that is the neutral
/// owner puts it in the table.
pub fn tables_of_sides(gs: &GameState, sides: &[&PlayerId]) -> Option<(Ctx, Render)> {
    if sides.iter().any(|p| crate::ties::neutral_units::is_neutral(p)) {
        tables_with(gs, &[crate::ties::neutral_units::NEUTRAL])
    } else {
        tables(gs)
    }
}
