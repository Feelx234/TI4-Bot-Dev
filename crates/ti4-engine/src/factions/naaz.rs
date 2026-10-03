//! The Naaz-Rokha Alliance (`naaz`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-naaz.md`.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Fabrication: "ACTION: Either purge 2 of your relic fragments of the same type to gain 1
//!   relic; or purge 1 of your relic fragments to gain 1 command token."
//! * Black Market Forgery (`bmf`): "ACTION: Purge 2 of your relic fragments of the same type to
//!   gain 1 relic. Then, return this card to the Naaz-Rokha player."
//! * Supercharge (`sc`): "At the start of a combat round, you may exhaust this card to apply +1 to
//!   the result of each of your unit's combat rolls during this combat round."
//! * Visz El Vir (flagship): "Your mechs in this system roll 1 additional die during combat."
//! * Garv and Gunn (`naazagent`): "At the end of a player's turn: You may exhaust this card to
//!   allow that player to explore 1 of their planets."
//! * Dart and Tai (`naazcommander`): "After you gain control of a planet that was controlled by
//!   another player: You may explore that planet." Unlock: "Have mechs in 3 systems."
//! * Hesh and Prit (`naazhero`): "ACTION: Gain 1 relic and perform the secondary ability of up to
//!   2 readied or unchosen strategy cards. During this action, spend command tokens from your
//!   reinforcements instead of your strategy pool. Then, purge this card."
//!
//! Not implemented here, see the evidence file: Distant Suns and Pre-Fab Arcologies (no explore
//! hook), the mech flip (shared code never flips), the Eidolon Maximum and Absolute Synergy.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::catalogue;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, StrategyCardId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus, TokenPool};

use super::hooks_economy::EconomyHooks;
use super::hooks_ground::GroundHooks;
use super::{CombatUnit, FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::timing::{Ability, Relation, TimingContext};

const FACTION: &str = "naaz";
const AGENT: &str = "naazagent";
const COMMANDER: &str = "naazcommander";
const HERO: &str = "naazhero";
const BMF: &str = "bmf";
const FAB_RELIC: &str = "faction|naaz|fabrication_relic";
const FAB_TOKEN: &str = "faction|naaz|fabrication_token";
const BMF_ACTION: &str = "faction|naaz|bmf";
/// The three decks a fragment type can name; frontier fragments stand in for any of them.
const TYPES: [&str; 3] = ["CULTURAL", "HAZARDOUS", "INDUSTRIAL"];

/// What this faction implements. `sc`, the mech forms, `distant_suns`, `pfa` and `naazbt` are not
/// claimed: see the evidence file.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &["fabrication", "distant_suns"],
    technologies: &["pfa", "sc"],
    units: &["naaz_flagship", "naaz_mech", "naaz_mech_space"],
    promissory: &[BMF],
    leaders: &[AGENT, COMMANDER, HERO],
    breakthroughs: &[],
    hooks: Hooks {
        component_actions: Some(component_actions),
        perform_component: Some(perform_component),
        commander_unlocked: Some(commander_unlocked),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        timing_abilities: Some(timing_abilities),
        unit_roll_modifier: Some(unit_roll_modifier),
        unit_dice: Some(unit_dice),
        space_combat_round_started: Some(space_combat_round_started),
        ground: GroundHooks {
            ground_combat_round_started: Some(ground_combat_round_started),
            ..GroundHooks::NONE
        },
        economy: EconomyHooks {
            explore_extra_draw: Some(explore_extra_draw),
            explored: Some(explored),
            ..EconomyHooks::NONE
        },
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn is_naaz(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, player, &LeaderId::new(leader))
}

fn decision(state: &GameState, player: &PlayerId, card: &str, subtype: &str) -> DecisionContext {
    DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(card.to_owned()),
        subtype,
        state.phase,
        state.round,
    )
}

fn fragments(state: &GameState, player: &PlayerId, kind: &str) -> i32 {
    state
        .player(player)
        .and_then(|seat| seat.relic_fragments.get(kind).copied())
        .unwrap_or(0)
}

/// Types of which the player can purge `count` fragments (frontier fragments count as any type).
fn purgeable_types(state: &GameState, player: &PlayerId, count: i32) -> Vec<&'static str> {
    let frontier = fragments(state, player, crate::exploration::FRONTIER);
    TYPES
        .iter()
        .copied()
        .filter(|kind| fragments(state, player, kind) + frontier >= count)
        .collect()
}

/// Purge `count` fragments for a type, matching ones first. `false` and untouched if short.
fn purge_fragments(state: &mut GameState, player: &PlayerId, kind: &str, count: i32) -> bool {
    let matching = fragments(state, player, kind);
    let frontier = if kind == crate::exploration::FRONTIER {
        0
    } else {
        fragments(state, player, crate::exploration::FRONTIER)
    };
    if matching + frontier < count {
        return false;
    }
    let from_matching = matching.min(count);
    let Some(seat) = state.player_mut(player) else {
        return false;
    };
    if from_matching > 0 {
        *seat.relic_fragments.entry(kind.to_owned()).or_insert(0) -= from_matching;
    }
    if count > from_matching {
        *seat
            .relic_fragments
            .entry(crate::exploration::FRONTIER.to_owned())
            .or_insert(0) -= count - from_matching;
    }
    true
}

fn ask(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    card: &str,
    subtype: &str,
    prompt: &str,
    options: Vec<ChoiceOption>,
) -> Option<String> {
    let choice = Choice::new(player.clone(), prompt.to_owned(), options).contextualized(decision(
        context.state,
        player,
        card,
        subtype,
    ));
    context.ask_seeing(&choice).ok().map(|answer| answer.id)
}

fn type_options(kinds: &[&str], verb: &str) -> Vec<ChoiceOption> {
    kinds
        .iter()
        .map(|kind| {
            ChoiceOption::labelled((*kind).to_owned(), "fragment", format!("{verb} {kind}"))
        })
        .collect()
}

/// Explore `planet` for `actor` through the shared path.
fn explore_planet(context: &mut TimingContext<'_>, actor: &PlayerId, planet: &PlanetId) {
    let mut resolving = crate::choice::Resolving {
        content: context.content,
        sources: context.sources,
        dice: context.dice,
        rng: context.rng,
        table: context.table,
        timing: None,
    };
    if let Some(deck) =
        crate::exploration::choose_deck(&mut resolving, context.state, actor, planet)
    {
        let _ = crate::exploration::explore_with(
            context.state,
            &mut resolving,
            actor,
            &deck,
            Some(planet),
        );
    }
}

fn explorable(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    planet: &PlanetId,
) -> bool {
    !crate::exploration::traits_of(content, sources, planet).is_empty()
        && crate::exploration::traits_of(content, sources, planet)
            .iter()
            .any(|deck| {
                state
                    .exploration_decks
                    .get(deck)
                    .is_some_and(|cards| !cards.is_empty())
            })
}

// -- Fabrication and Black Market Forgery --------------------------------------------------------

/// The held Black Market Forgery of a player who is not the Naaz-Rokha player.
fn held_bmf(state: &GameState, player: &PlayerId) -> Option<String> {
    let own = crate::promissory::faction_name(state, player);
    state
        .promissory_notes
        .iter()
        .find(|(note, holder)| {
            *holder == player
                && crate::promissory::alias_of(note) == BMF
                && crate::promissory::owner_of(note).is_some_and(|owner| owner != own)
        })
        .map(|(note, _)| note.clone())
}

fn component_actions(
    state: &GameState,
    _content: &ContentStore,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    let mut options = Vec::new();
    let relic_left = !state.relic_deck.is_empty();
    if is_naaz(state, player) {
        if relic_left && !purgeable_types(state, player, 2).is_empty() {
            options.push(ChoiceOption::labelled(
                FAB_RELIC,
                crate::faction_abilities::ACTION_KIND,
                "Fabrication: purge 2 relic fragments of the same type to gain 1 relic",
            ));
        }
        let any_fragment = state
            .player(player)
            .is_some_and(|seat| seat.relic_fragments.values().any(|held| *held > 0));
        if any_fragment && state.tokens_in_reinforcements(player) > 0 {
            options.push(ChoiceOption::labelled(
                FAB_TOKEN,
                crate::faction_abilities::ACTION_KIND,
                "Fabrication: purge 1 relic fragment to gain 1 command token",
            ));
        }
    }
    if relic_left
        && held_bmf(state, player).is_some()
        && !purgeable_types(state, player, 2).is_empty()
    {
        options.push(ChoiceOption::labelled(
            BMF_ACTION,
            crate::faction_abilities::ACTION_KIND,
            "Black Market Forgery: purge 2 relic fragments of the same type to gain 1 relic",
        ));
    }
    options
}

/// Choose a fragment type (asking only when there is a choice), purge `count`, gain a relic.
fn purge_for_relic(context: &mut TimingContext<'_>, player: &PlayerId, card: &str) -> bool {
    let mut kinds = purgeable_types(context.state, player, 2);
    // Only frontier fragments: every type is the same purge, so offer one.
    if kinds
        .iter()
        .all(|kind| fragments(context.state, player, kind) == 0)
    {
        kinds.truncate(1);
    }
    let kind = match kinds.as_slice() {
        [] => return false,
        [only] => (*only).to_owned(),
        _ => {
            let Some(id) = ask(
                context,
                player,
                card,
                "fragment_type",
                "purge 2 fragments of which type",
                type_options(&kinds, "purge 2"),
            ) else {
                return false;
            };
            if !kinds.contains(&id.as_str()) {
                return false;
            }
            id
        }
    };
    if context.state.relic_deck.is_empty() || !purge_fragments(context.state, player, &kind, 2) {
        return false;
    }
    crate::relics::gain(context.state, player).is_some()
}

fn perform_component(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    option: &ChoiceOption,
) -> bool {
    match option.id.as_str() {
        FAB_RELIC if is_naaz(context.state, player) => {
            purge_for_relic(context, player, "fabrication")
        }
        FAB_TOKEN if is_naaz(context.state, player) => fabricate_token(context, player),
        BMF_ACTION => {
            let Some(note) = held_bmf(context.state, player) else {
                return false;
            };
            if !purge_for_relic(context, player, BMF) {
                return false;
            }
            crate::promissory::give_back(context.state, &note);
            true
        }
        _ => false,
    }
}

fn fabricate_token(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    if context.state.tokens_in_reinforcements(player) <= 0 {
        return false;
    }
    let held: Vec<&str> = TYPES
        .iter()
        .copied()
        .chain(std::iter::once(crate::exploration::FRONTIER))
        .filter(|kind| fragments(context.state, player, kind) > 0)
        .collect();
    let kind = match held.as_slice() {
        [] => return false,
        [only] => (*only).to_owned(),
        _ => {
            let Some(id) = ask(
                context,
                player,
                "fabrication",
                "fragment_type",
                "purge 1 fragment of which type",
                type_options(&held, "purge 1"),
            ) else {
                return false;
            };
            if !held.contains(&id.as_str()) {
                return false;
            }
            id
        }
    };
    let pools = [
        ("tactic", TokenPool::Tactic),
        ("fleet", TokenPool::Fleet),
        ("strategic", TokenPool::Strategic),
    ];
    let options = pools
        .iter()
        .map(|(id, _)| {
            ChoiceOption::labelled((*id).to_owned(), "pool", format!("gain 1 token in {id}"))
        })
        .collect();
    let Some(id) = ask(
        context,
        player,
        "fabrication",
        "token_pool",
        "Fabrication: which pool gains the token",
        options,
    ) else {
        return false;
    };
    let Some((_, pool)) = pools.iter().find(|(name, _)| *name == id) else {
        return false;
    };
    if !purge_fragments(context.state, player, &kind, 1) {
        return false;
    }
    context.state.gain_token(player, *pool, 1) > 0
}

// -- Distant Suns and Pre-Fab Arcologies ----------------------------------------------------------

/// Distant Suns: "When you explore a planet that contains 1 of your mechs: You may draw 1
/// additional card; choose 1 to resolve and discard the rest." The shared route draws and asks.
fn explore_extra_draw(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    planet: &PlanetId,
) -> bool {
    if !is_naaz(state, player) {
        return false;
    }
    let types = catalogue(content, sources);
    state.board.values().any(|here| {
        here.planet_units.get(planet).is_some_and(|units| {
            units.iter().any(|unit| {
                &unit.owner == player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(|kind| kind.base_type() == "mech")
            })
        })
    })
}

/// Pre-Fab Arcologies: "After you explore a planet, ready that planet."
fn explored(
    state: &mut GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    player: &PlayerId,
    planet: &PlanetId,
) {
    if state
        .player(player)
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new("pfa")))
    {
        state.exhausted_planets.remove(planet);
    }
}

// -- Supercharge ---------------------------------------------------------------------------------

fn supercharge_key(player: &PlayerId) -> String {
    format!("naaz:supercharge:{player}")
}

fn technology_ready(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    let id = TechnologyId::new(alias);
    state.player(player).is_some_and(|seat| {
        seat.technologies.contains(&id) && !seat.exhausted_technologies.contains(&id)
    })
}

fn space_combat_round_started(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut crate::choice::Table,
    player: &PlayerId,
) {
    offer_supercharge(state, content, sources, table, player, "space");
}

fn ground_combat_round_started(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut crate::choice::Table,
    player: &PlayerId,
    _system: &SystemId,
    _planet: &PlanetId,
) {
    offer_supercharge(state, content, sources, table, player, "ground");
}

/// "At the start of a combat round": the mark names the kind of combat and the round sequence, so
/// it lapses by itself when the round moves on.
fn offer_supercharge(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut crate::choice::Table,
    player: &PlayerId,
    context: &str,
) {
    state.faction_marks.remove(&supercharge_key(player));
    if !technology_ready(state, player, "sc") {
        return;
    }
    let choice = Choice::new(
        player.clone(),
        "Supercharge: exhaust to apply +1 to each combat roll this round".to_owned(),
        vec![
            ChoiceOption::labelled("sc", "technology", "exhaust Supercharge"),
            ChoiceOption::decline(),
        ],
    )
    .contextualized(decision(state, player, "sc", "supercharge"));
    let Ok(answer) = table.ask_seeing(
        &choice,
        &crate::choice::Observed::new(state, content, sources, None),
    ) else {
        return;
    };
    if answer.is_decline() {
        return;
    }
    let seq = state.combat_round_seq;
    state
        .faction_marks
        .insert(supercharge_key(player), format!("{context}:{seq}"));
    if let Some(seat) = state.player_mut(player) {
        seat.exhausted_technologies.insert(TechnologyId::new("sc"));
    }
}

fn unit_roll_modifier(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    unit: &CombatUnit<'_>,
) -> i64 {
    i64::from(
        state.faction_marks.get(&supercharge_key(unit.player))
            == Some(&format!("{}:{}", unit.context, state.combat_round_seq)),
    )
}

// -- Visz El Vir ---------------------------------------------------------------------------------

fn unit_dice(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &CombatUnit<'_>,
    dice: i64,
) -> i64 {
    let Some(system) = unit.system else {
        return dice;
    };
    let is_mech = catalogue(content, sources)
        .get(unit.unit_type)
        .is_some_and(|kind| kind.base_type() == "mech")
        && unit.unit_type.starts_with("naaz_");
    let flagship = state
        .system_state(system)
        .units
        .iter()
        .any(|ship| &ship.owner == unit.player && ship.type_id.as_str() == "naaz_flagship");
    if is_mech && flagship { dice + 1 } else { dice }
}

// -- Garv and Gunn -------------------------------------------------------------------------------

fn explorable_planets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    actor: &PlayerId,
) -> Vec<PlanetId> {
    state
        .controlled_planets(actor)
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .filter(|planet| explorable(state, content, sources, planet))
        .collect()
}

fn agent(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:naazagent:TURN_PASSED:after"),
        seat.clone(),
        "TURN_PASSED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(actor) = event.text("player").map(PlayerId::new) else {
                return Ok(());
            };
            if leader_status(context.state, &owner, AGENT) != Some(LeaderStatus::Readied) {
                return Ok(());
            }
            let planets =
                explorable_planets(context.state, context.content, context.sources, &actor);
            if planets.is_empty() {
                return Ok(());
            }
            // Every question first: the player whose turn ended picks the planet.
            let options = planets
                .iter()
                .map(|planet| {
                    ChoiceOption::labelled(
                        planet.to_string(),
                        "planet",
                        format!("explore {planet}"),
                    )
                })
                .collect();
            let Some(id) = ask(
                context,
                &actor,
                "naazagent",
                "agent_planet",
                "Garv and Gunn: explore which of your planets",
                options,
            ) else {
                return Ok(());
            };
            let Some(planet) = planets.into_iter().find(|p| p.as_str() == id) else {
                return Ok(());
            };
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new(AGENT)) {
                return Ok(());
            }
            explore_planet(context, &actor, &planet);
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        leader_status(context.state, &condition_owner, AGENT) == Some(LeaderStatus::Readied)
            && event
                .text("player")
                .map(PlayerId::new)
                .is_some_and(|actor| {
                    !explorable_planets(context.state, context.content, context.sources, &actor)
                        .is_empty()
                })
    }))
}

// -- Dart and Tai --------------------------------------------------------------------------------

fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != COMMANDER {
        return None;
    }
    let types = catalogue(content, sources);
    let is_mech = |unit: &ti4_model::units::Unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == "mech")
    };
    let systems = state
        .board
        .values()
        .filter(|here| {
            here.units.iter().any(is_mech)
                || here
                    .planet_units
                    .values()
                    .any(|units| units.iter().any(is_mech))
        })
        .count();
    Some(systems >= 3)
}

fn commander(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:naazcommander:PLANET_CONTROL_GAINED:after"),
        seat.clone(),
        "PLANET_CONTROL_GAINED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if let Some(planet) = event.text("planet").map(PlanetId::new) {
                explore_planet(context, &owner, &planet);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && event
                .text("previous_owner")
                .is_some_and(|prev| !prev.is_empty())
            && leader_status(context.state, &condition_owner, COMMANDER)
                == Some(LeaderStatus::Unlocked)
            && event
                .text("planet")
                .map(PlanetId::new)
                .is_some_and(|planet| {
                    explorable(context.state, context.content, context.sources, &planet)
                })
    }))
}

// -- Hesh and Prit -------------------------------------------------------------------------------

/// Strategy cards that are readied (held and not exhausted) or unchosen. Thunder's Edge Warfare is
/// left out: its primary is a free tactical action, which cannot run inside a leader effect.
fn hero_cards(state: &GameState) -> Vec<StrategyCardId> {
    let mut cards: Vec<StrategyCardId> = state
        .players
        .iter()
        .flat_map(|seat| {
            seat.strategy_cards
                .iter()
                .filter(|card| !seat.exhausted_strategy_cards.contains(*card))
        })
        .cloned()
        .chain(state.unclaimed_strategy_cards.iter().cloned())
        .filter(|card| card.as_str() != "te6warfare")
        .collect();
    cards.sort();
    cards.dedup();
    cards
}

/// Every secondary but Leadership's (influence) costs a command token.
fn costs_token(content: &ContentStore, card: &StrategyCardId) -> bool {
    crate::strategy_cards::card_name(content, card.as_str()).as_deref() != Some("Leadership")
}

fn leader_action(
    state: &GameState,
    _content: &ContentStore,
    _player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == HERO).then_some(!state.relic_deck.is_empty())
}

fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != HERO {
        return None;
    }
    if context.state.relic_deck.is_empty() {
        return Some(false);
    }
    // Every choice before the first mutation: up to 2 cards, each at most once.
    let mut chosen: Vec<StrategyCardId> = Vec::new();
    while chosen.len() < 2 {
        let cards: Vec<StrategyCardId> = hero_cards(context.state)
            .into_iter()
            .filter(|card| !chosen.contains(card))
            .collect();
        let spare = context.state.tokens_in_reinforcements(player) > 0;
        let cards: Vec<StrategyCardId> = cards
            .into_iter()
            .filter(|card| spare || !costs_token(context.content, card))
            .collect();
        if cards.is_empty() {
            break;
        }
        let mut options: Vec<ChoiceOption> = cards
            .iter()
            .map(|card| {
                ChoiceOption::labelled(
                    card.to_string(),
                    "strategy_card",
                    crate::draft::strategy_card_label(context.content, card.as_str()),
                )
            })
            .collect();
        options.push(ChoiceOption::decline());
        let Some(id) = ask(
            context,
            player,
            HERO,
            "hero_card",
            "Hesh and Prit: perform the secondary of which card",
            options,
        ) else {
            break;
        };
        match cards.into_iter().find(|card| card.as_str() == id) {
            Some(card) => chosen.push(card),
            None => break,
        }
    }
    let snapshot = context.state.clone();
    if crate::relics::gain(context.state, player).is_none() {
        return Some(false);
    }
    for card in &chosen {
        // A token spent from reinforcements returns to reinforcements: nothing leaves the pools,
        // but one has to be there to spend.
        if costs_token(context.content, card) && context.state.tokens_in_reinforcements(player) <= 0
        {
            break;
        }
        let outcome = crate::strategy_cards::secondary(
            context.state,
            context.content,
            context.sources,
            context.galaxy,
            context.table,
            player,
            card.as_str(),
        );
        if outcome.is_err() {
            *context.state = snapshot;
            return Some(false);
        }
    }
    Some(true)
}

// -- timing abilities ----------------------------------------------------------------------------

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    vec![agent(owner_name, seat), commander(owner_name, seat)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use ti4_model::content_types::DEFAULT;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }
    fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", FACTION), ("b", "sol")], DEFAULT)
    }
    fn scripted(answers: &[&str]) -> crate::choice::Table {
        crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(
            answers.iter().map(|s| (*s).to_owned()),
        )))
    }
    fn emit(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        event_type: &str,
        pairs: &[(&str, &str)],
    ) {
        let payload: BTreeMap<String, serde_json::Value> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
            .collect();
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, None, table, |ctx| {
            let event = ctx
                .event_sequence
                .next(event_type, payload)
                .expect("an event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("the window resolves");
        });
    }
    fn give_fragments(state: &mut GameState, who: &PlayerId, kind: &str, n: i32) {
        state
            .player_mut(who)
            .unwrap()
            .relic_fragments
            .insert(kind.to_owned(), n);
    }
    fn perform(state: &mut GameState, who: &PlayerId, id: &str, answers: &[&str]) -> bool {
        let options = component_actions(state, ContentStore::embedded(), who);
        let option = options.into_iter().find(|o| o.id == id).expect("offered");
        crate::fixtures::with_context(state, DEFAULT, None, &mut scripted(answers), |ctx| {
            perform_component(ctx, who, &option)
        })
    }
    /// An explorable planet outside every home system, put under `who`'s control.
    fn take_explorable(state: &mut GameState, who: &PlayerId) -> PlanetId {
        let content = ContentStore::embedded();
        let catalogue = ti4_content::galaxy::all_planets(content, DEFAULT);
        let planet = crate::fixtures::non_home_planets(200)
            .into_iter()
            .map(PlanetId::new)
            .find(|p| {
                explorable(state, content, DEFAULT, p)
                    && catalogue
                        .get(p.as_str())
                        .and_then(ti4_content::Planet::system_id)
                        .is_some()
            })
            .expect("an explorable planet");
        let system = SystemId::new(catalogue[planet.as_str()].system_id().unwrap());
        state
            .system_mut(&system)
            .planet_control
            .insert(planet.clone(), who.clone());
        planet
    }
    fn controlled_by(state: &GameState, who: &PlayerId) -> Vec<(SystemId, PlanetId)> {
        state
            .controlled_planets(who)
            .into_iter()
            .map(|(s, p)| (s.clone(), p.clone()))
            .collect()
    }

    // -- neutrality ------------------------------------------------------------------------------

    #[test]
    fn a_game_without_naaz_is_offered_nothing() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        give_fragments(&mut state, &a(), "CULTURAL", 3);
        assert!(component_actions(&state, content, &a()).is_empty());
        assert_eq!(
            leader_action(&state, content, &a(), &LeaderId::new(HERO)),
            Some(true),
            "the id check is by leader, ownership is the leaders module's"
        );
        let before = state.clone();
        let planet = controlled_by(&state, &a())[0].1.clone();
        emit(
            &mut state,
            &mut scripted(&[]),
            "TURN_PASSED",
            &[("player", "a")],
        );
        emit(
            &mut state,
            &mut scripted(&[]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("planet", planet.as_str()),
                ("previous_owner", "b"),
            ],
        );
        assert_eq!(state.exploration_log, before.exploration_log);
        assert_eq!(state.board, before.board);
        assert!(state.faction_marks.is_empty());
    }

    // -- Fabrication -----------------------------------------------------------------------------

    #[test]
    fn fabrication_turns_two_matching_fragments_into_a_relic() {
        let mut state = game();
        assert!(component_actions(&state, ContentStore::embedded(), &a()).is_empty());
        give_fragments(&mut state, &a(), "CULTURAL", 1);
        assert!(
            !component_actions(&state, ContentStore::embedded(), &a())
                .iter()
                .any(|o| o.id == FAB_RELIC),
            "one fragment is not enough"
        );
        give_fragments(&mut state, &a(), "CULTURAL", 2);
        let relics = state.player(&a()).unwrap().relics.len();
        assert!(perform(&mut state, &a(), FAB_RELIC, &[]));
        assert_eq!(state.player(&a()).unwrap().relics.len(), relics + 1);
        assert_eq!(fragments(&state, &a(), "CULTURAL"), 0);
    }

    #[test]
    fn fabrication_frontier_fragments_stand_in_and_are_spent_last() {
        let mut state = game();
        give_fragments(&mut state, &a(), "HAZARDOUS", 1);
        give_fragments(&mut state, &a(), "FRONTIER", 1);
        assert!(perform(&mut state, &a(), FAB_RELIC, &[]));
        assert_eq!(fragments(&state, &a(), "HAZARDOUS"), 0);
        assert_eq!(fragments(&state, &a(), "FRONTIER"), 0);
    }

    #[test]
    fn fabrication_turns_one_fragment_into_a_token_of_the_chosen_pool() {
        let mut state = game();
        give_fragments(&mut state, &a(), "INDUSTRIAL", 1);
        let before = state.player(&a()).unwrap().tokens(TokenPool::Fleet);
        assert!(perform(&mut state, &a(), FAB_TOKEN, &["fleet"]));
        assert_eq!(
            state.player(&a()).unwrap().tokens(TokenPool::Fleet),
            before + 1
        );
        assert_eq!(fragments(&state, &a(), "INDUSTRIAL"), 0);
        assert!(
            !component_actions(&state, ContentStore::embedded(), &a())
                .iter()
                .any(|o| o.id == FAB_TOKEN),
            "no fragment left"
        );
    }

    // -- Black Market Forgery --------------------------------------------------------------------

    #[test]
    fn black_market_forgery_is_the_holders_action_and_returns_home() {
        let mut state = game();
        give_fragments(&mut state, &b(), "CULTURAL", 2);
        assert!(
            component_actions(&state, ContentStore::embedded(), &b()).is_empty(),
            "not held"
        );
        let note = crate::promissory::note_id(BMF, "naaz");
        state.promissory_notes.insert(note.clone(), b());
        let relics = state.player(&b()).unwrap().relics.len();
        assert!(perform(&mut state, &b(), BMF_ACTION, &[]));
        assert_eq!(state.player(&b()).unwrap().relics.len(), relics + 1);
        assert_eq!(state.promissory_notes.get(&note), Some(&a()));
        assert!(
            !component_actions(&state, ContentStore::embedded(), &a())
                .iter()
                .any(|o| o.id == BMF_ACTION),
            "its owner cannot play it"
        );
    }

    #[test]
    fn black_market_forgery_needs_two_matching_fragments() {
        let mut state = game();
        state
            .promissory_notes
            .insert(crate::promissory::note_id(BMF, "naaz"), b());
        give_fragments(&mut state, &b(), "CULTURAL", 1);
        assert!(component_actions(&state, ContentStore::embedded(), &b()).is_empty());
    }

    // -- Visz El Vir -----------------------------------------------------------------------------

    #[test]
    fn the_flagship_gives_mechs_in_its_system_a_die() {
        let mut state = game();
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mech = |state: &GameState, kind: &str, who: &PlayerId| {
            unit_dice(
                state,
                content,
                DEFAULT,
                &CombatUnit {
                    player: who,
                    system: Some(&system),
                    planet: None,
                    unit_type: kind,
                    context: "space",
                },
                2,
            )
        };
        assert_eq!(mech(&state, "naaz_mech", &a()), 2, "no flagship");
        crate::fixtures::put(&mut state, &system, "naaz_flagship", &a(), 1);
        assert_eq!(mech(&state, "naaz_mech", &a()), 3);
        assert_eq!(mech(&state, "naaz_mech_space", &a()), 3);
        assert_eq!(mech(&state, "naaz_flagship", &a()), 2, "not a mech");
        assert_eq!(mech(&state, "sol_mech", &a()), 2, "not a Naaz mech");
        assert_eq!(mech(&state, "naaz_mech", &b()), 2, "another player's");
    }

    // -- Supercharge (space combat rounds) ---------------------------------------------------------

    #[test]
    fn supercharge_adds_one_for_the_round_it_was_exhausted_in() {
        let mut state = game();
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let probe = |state: &GameState| {
            unit_roll_modifier(
                state,
                content,
                DEFAULT,
                &CombatUnit {
                    player: &a(),
                    system: Some(&system),
                    planet: None,
                    unit_type: "naaz_flagship",
                    context: "space",
                },
            )
        };
        // Not owned: not asked.
        space_combat_round_started(&mut state, content, DEFAULT, &mut scripted(&["sc"]), &a());
        assert_eq!(probe(&state), 0);
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("sc"));
        space_combat_round_started(
            &mut state,
            content,
            DEFAULT,
            &mut scripted(&["decline"]),
            &a(),
        );
        assert_eq!(probe(&state), 0);
        assert!(technology_ready(&state, &a(), "sc"));
        space_combat_round_started(&mut state, content, DEFAULT, &mut scripted(&["sc"]), &a());
        assert_eq!(probe(&state), 1);
        assert!(!technology_ready(&state, &a(), "sc"));
        state.combat_round_seq += 1;
        assert_eq!(probe(&state), 0, "only the round it was used in");
    }

    #[test]
    fn supercharge_also_starts_ground_combat_rounds() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (system, planet) = (SystemId::new("18"), PlanetId::new("mr"));
        let probe = |state: &GameState, context: &str| {
            unit_roll_modifier(
                state,
                content,
                DEFAULT,
                &CombatUnit {
                    player: &a(),
                    system: Some(&system),
                    planet: Some(&planet),
                    unit_type: "naaz_mech",
                    context,
                },
            )
        };
        let ask = |state: &mut GameState, answers: &[&str]| {
            ground_combat_round_started(
                state,
                content,
                DEFAULT,
                &mut scripted(answers),
                &a(),
                &system,
                &planet,
            );
        };
        ask(&mut state, &["sc"]);
        assert_eq!(probe(&state, "ground"), 0, "not owned: not asked");
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("sc"));
        ask(&mut state, &["decline"]);
        assert_eq!(probe(&state, "ground"), 0);
        ask(&mut state, &["sc"]);
        assert_eq!(probe(&state, "ground"), 1);
        assert_eq!(
            probe(&state, "space"),
            0,
            "a ground round's use is not a space round's"
        );
        assert!(!technology_ready(&state, &a(), "sc"));
    }

    // -- Distant Suns and Pre-Fab Arcologies -------------------------------------------------------

    fn system_of(planet: &PlanetId) -> SystemId {
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), DEFAULT);
        SystemId::new(catalogue[planet.as_str()].system_id().unwrap())
    }

    fn deck_len(state: &GameState, planet: &PlanetId) -> usize {
        let deck = crate::exploration::trait_of(ContentStore::embedded(), DEFAULT, planet).unwrap();
        state.exploration_decks[&deck].len()
    }

    fn explore_once(state: &mut GameState, who: &PlayerId, planet: &PlanetId) {
        let deck = crate::exploration::trait_of(ContentStore::embedded(), DEFAULT, planet).unwrap();
        crate::exploration::explore(state, ContentStore::embedded(), who, &deck, Some(planet))
            .expect("a card");
    }

    #[test]
    fn distant_suns_draws_an_extra_card_for_a_planet_with_a_mech() {
        let mut state = game();
        let planet = take_explorable(&mut state, &a());
        let system = system_of(&planet);
        let before = deck_len(&state, &planet);
        explore_once(&mut state, &a(), &planet);
        assert_eq!(deck_len(&state, &planet), before - 1, "no mech: one card");
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "naaz_mech", &a(), 1);
        explore_once(&mut state, &a(), &planet);
        assert_eq!(deck_len(&state, &planet), before - 3, "a mech: one extra");
        // Another player's mech does not count, and neither does a Sol player's own.
        let other = take_explorable(&mut state, &b());
        crate::fixtures::put_on_planet(&mut state, &system_of(&other), &other, "sol_mech", &b(), 1);
        let held = deck_len(&state, &other);
        explore_once(&mut state, &b(), &other);
        assert_eq!(deck_len(&state, &other), held - 1);
    }

    #[test]
    fn pre_fab_arcologies_readies_the_planet_after_exploring() {
        let mut state = game();
        let planet = take_explorable(&mut state, &a());
        state.exhausted_planets.insert(planet.clone());
        explore_once(&mut state, &a(), &planet);
        assert!(state.exhausted_planets.contains(&planet), "no technology");
        crate::technology::grant(&mut state, &a(), &TechnologyId::new("pfa"));
        explore_once(&mut state, &a(), &planet);
        assert!(!state.exhausted_planets.contains(&planet));
    }

    // -- Eidolon forms ---------------------------------------------------------------------------

    #[test]
    fn the_eidolon_is_a_ship_in_the_active_space_area_and_flips_back() {
        let mut state = game();
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        crate::fixtures::put(&mut state, &system, "naaz_mech", &a(), 1);
        let kinds = |state: &GameState| -> Vec<String> {
            state
                .system_state(&system)
                .units
                .iter()
                .filter(|unit| unit.owner == a())
                .map(|unit| unit.type_id.to_string())
                .collect()
        };
        let flipped =
            crate::fleet::flip_to_ship_forms(&mut state, content, DEFAULT, &[a()], &system);
        assert_eq!(flipped.len(), 1);
        assert_eq!(kinds(&state), ["naaz_mech_space"]);
        let types = catalogue(content, DEFAULT);
        assert!(types["naaz_mech_space"].is_ship() && !types["naaz_mech"].is_ship());
        crate::fleet::flip_to_ground_forms(&mut state, content, DEFAULT, &[a()], &system);
        assert_eq!(kinds(&state), ["naaz_mech"]);
        // Another faction's mech never flips.
        crate::fixtures::put(&mut state, &system, "sol_mech", &b(), 1);
        assert!(
            crate::fleet::flip_to_ship_forms(&mut state, content, DEFAULT, &[b()], &system)
                .is_empty()
        );
    }

    // -- Garv and Gunn ---------------------------------------------------------------------------

    const AGENT_ABILITY: &str = "leader:naaz:naazagent:TURN_PASSED:after";

    #[test]
    fn the_agent_lets_a_player_explore_one_of_their_planets() {
        let mut state = game();
        let planet = take_explorable(&mut state, &b());
        let log = state.exploration_log.len();
        emit(
            &mut state,
            &mut scripted(&[AGENT_ABILITY, planet.as_str()]),
            "TURN_PASSED",
            &[("player", "b")],
        );
        assert_eq!(state.exploration_log.len(), log + 1);
        assert_eq!(state.exploration_log[log].player, b());
        assert_eq!(state.exploration_log[log].planet.as_ref(), Some(&planet));
        assert_eq!(
            leader_status(&state, &a(), AGENT),
            Some(LeaderStatus::Exhausted)
        );
    }

    #[test]
    fn the_agent_may_be_declined_and_must_be_ready() {
        let mut state = game();
        take_explorable(&mut state, &b());
        let log = state.exploration_log.len();
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "TURN_PASSED",
            &[("player", "b")],
        );
        assert_eq!(state.exploration_log.len(), log);
        assert_eq!(
            leader_status(&state, &a(), AGENT),
            Some(LeaderStatus::Readied)
        );
        crate::leaders::exhaust(&mut state, &a(), &LeaderId::new(AGENT));
        emit(
            &mut state,
            &mut scripted(&[AGENT_ABILITY]),
            "TURN_PASSED",
            &[("player", "b")],
        );
        assert_eq!(state.exploration_log.len(), log);
    }

    // -- Dart and Tai ----------------------------------------------------------------------------

    #[test]
    fn the_commander_unlocks_with_mechs_in_three_systems() {
        let mut state = game();
        let content = ContentStore::embedded();
        let ask = |state: &GameState, leader: &str| {
            commander_unlocked(state, content, DEFAULT, None, &a(), &LeaderId::new(leader))
        };
        let systems = ["18", "19", "20", "21"].map(SystemId::new);
        // The home system already holds the starting mech.
        crate::fixtures::put(&mut state, &systems[0], "naaz_mech", &a(), 2);
        assert_eq!(ask(&state, COMMANDER), Some(false), "two systems");
        crate::fixtures::put(&mut state, &systems[2], "naaz_mech", &b(), 1);
        assert_eq!(ask(&state, COMMANDER), Some(false), "another player's mech");
        crate::fixtures::put(&mut state, &systems[1], "naaz_mech_space", &a(), 1);
        assert_eq!(ask(&state, COMMANDER), Some(true));
        assert_eq!(ask(&state, "naalucommander"), None);
    }

    #[test]
    fn the_commander_explores_a_planet_taken_from_another_player() {
        let mut state = game();
        let planet = take_explorable(&mut state, &a());
        let ability = "leader:naaz:naazcommander:PLANET_CONTROL_GAINED:after";
        let pairs = [
            ("player", "a"),
            ("planet", planet.as_str()),
            ("previous_owner", "b"),
        ];
        let log = state.exploration_log.len();
        // Locked: nothing.
        emit(
            &mut state,
            &mut scripted(&[ability]),
            "PLANET_CONTROL_GAINED",
            &pairs,
        );
        assert_eq!(state.exploration_log.len(), log);
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(COMMANDER), LeaderStatus::Unlocked);
        // A planet nobody held is explored by the rules, not by the commander.
        emit(
            &mut state,
            &mut scripted(&[ability]),
            "PLANET_CONTROL_GAINED",
            &[("player", "a"), ("planet", planet.as_str())],
        );
        assert_eq!(state.exploration_log.len(), log);
        emit(
            &mut state,
            &mut scripted(&[ability]),
            "PLANET_CONTROL_GAINED",
            &pairs,
        );
        assert_eq!(state.exploration_log.len(), log + 1);
        assert_eq!(state.exploration_log[log].player, a());
    }

    // -- Hesh and Prit ---------------------------------------------------------------------------

    #[test]
    fn the_hero_gains_a_relic_and_performs_up_to_two_secondaries() {
        let mut state = game();
        let hero = LeaderId::new(HERO);
        let cards = hero_cards(&state);
        assert!(cards.len() >= 2);
        let relics = state.player(&a()).unwrap().relics.len();
        let deck = state.relic_deck.len();
        let pick = |card: &StrategyCardId| card.to_string();
        let done = crate::fixtures::with_context(
            &mut state,
            DEFAULT,
            None,
            &mut scripted(&[&pick(&cards[0]), "decline"]),
            |ctx| use_leader(ctx, &a(), &hero),
        );
        assert_eq!(done, Some(true));
        assert_eq!(state.player(&a()).unwrap().relics.len(), relics + 1);
        assert_eq!(state.relic_deck.len(), deck - 1);
        assert_eq!(
            use_leader_other(&mut state),
            None,
            "another leader is not this module's"
        );
    }

    fn use_leader_other(state: &mut GameState) -> Option<bool> {
        crate::fixtures::with_context(state, DEFAULT, None, &mut scripted(&[]), |ctx| {
            use_leader(ctx, &a(), &LeaderId::new("naaluhero"))
        })
    }

    #[test]
    fn the_hero_is_not_offered_without_a_relic_to_gain() {
        let mut state = game();
        state.relic_deck.clear();
        assert_eq!(
            leader_action(&state, ContentStore::embedded(), &a(), &LeaderId::new(HERO)),
            Some(false)
        );
        let done =
            crate::fixtures::with_context(&mut state, DEFAULT, None, &mut scripted(&[]), |ctx| {
                use_leader(ctx, &a(), &LeaderId::new(HERO))
            });
        assert_eq!(done, Some(false));
    }

    #[test]
    fn the_hero_does_not_offer_exhausted_cards() {
        let mut state = game();
        let card = state.unclaimed_strategy_cards[0].clone();
        state.player_mut(&b()).unwrap().strategy_cards = vec![card.clone()];
        state
            .player_mut(&b())
            .unwrap()
            .exhausted_strategy_cards
            .insert(card.clone());
        state.unclaimed_strategy_cards.retain(|c| *c != card);
        assert!(!hero_cards(&state).contains(&card));
    }

    #[test]
    fn the_claims_are_the_sheet() {
        assert!(MODULE.units.contains(&"naaz_flagship"));
        assert!(MODULE.leaders.len() == 3);
    }
}
