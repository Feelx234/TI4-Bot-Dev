//! The Nomad (`nomad`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-nomad.md`.
//!
//! Split for parallel work: this file holds the faction abilities, flagship, technologies, the
//! hero, the commander claim and the breakthrough; `nomad_agents.rs` holds the three agents, the
//! mech, The Cavalry and the combat/ground hooks those need.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * The Company: "During setup, take the 2 additional Nomad faction agents and place them next to
//!   your faction sheet; you have 3 agents." Delivered by `leaders::for_faction`/`deploy` (the
//!   sheet lists three agents, all dealt readied); tested here.
//! * Future Sight: "During the Agenda phase, after an outcome that you voted for or predicted is
//!   resolved: Gain 1 trade good."
//! * Memoria I / II (flagship) and Memoria II (technology): "You may treat this unit as if it were
//!   adjacent to systems that contain 1 or more of your mechs." (Memoria II: cost 8, combat 5
//!   (x2), move 2, capacity 6, SUSTAIN DAMAGE, ANTI-FIGHTER BARRAGE 5 (x3) -- statistics.)
//! * Temporal Command Suite (`tcs`): "After any player's agent becomes exhausted, you may exhaust
//!   this card to ready that agent; if you ready another player's agent, you may perform a
//!   transaction with that player."
//! * Ahk-Syl Siven (`nomadhero`), Probability Matrix: Time Warp: "ACTION: Place this card near the
//!   game board. Your flagship and units it transports can move out of systems that contain your
//!   command tokens during this game round. At the end of that game round, purge this card."
//!   Unlock: "Have 3 scored objectives."
//! * Navarch Feng (`nomadcommander`): "When you produce: You can produce your flagship without
//!   spending resources." Unlock: "Have 1 scored secret objective."
//! * Thunder's Paradox (`nomadbt`): "At the start of any player's turn, you may exhaust 1 of your
//!   agents to ready any other agent."

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::catalogue;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlayerId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus, TransientFlags};

use super::hooks_cards::CardHooks;
use super::hooks_movement::MovementHooks;
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::timing::{Ability, Relation, TimingContext, TimingError};

/// The faction alias; also the faction name in promissory note ids.
pub const FACTION: &str = "nomad";

const HERO: &str = "nomadhero";
const COMMANDER: &str = "nomadcommander";
const TCS: &str = "tcs";
const BREAKTHROUGH: &str = "nomadbt";
/// Memoria I and II.
const FLAGSHIPS: [&str; 2] = ["nomad_flagship", "nomad_flagship2"];
/// The typed event a ready-able agent's exhaustion is announced with: payload `player` (the
/// agent's owner) and `leader`. `leaders::exhaust` stages it for [`watches_agent_exhaustion`].
pub const AGENT_EXHAUSTED: &str = "AGENT_EXHAUSTED";

/// What this faction implements; grows package by package.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &["the_company", "future_sight"],
    technologies: &["m2", TCS],
    units: super::nomad_agents::UNITS,
    promissory: super::nomad_agents::PROMISSORY,
    leaders: super::nomad_agents::LEADERS,
    breakthroughs: &["nomadbt"],
    hooks: Hooks {
        timing_abilities: Some(timing_abilities),
        commander_unlocked: Some(commander_unlocked),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        movement: MovementHooks {
            unit_adjacent_systems: Some(memoria_adjacent),
            ignores_command_tokens: Some(hero_ignores_command_tokens),
            ..MovementHooks::NONE
        },
        cards: CardHooks {
            transaction_reach: Some(tcs_reach),
            ..CardHooks::NONE
        },
        combat: super::nomad_agents::COMBAT_HOOKS,
        ground: super::nomad_agents::GROUND_HOOKS,
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn is_nomad(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, player, &LeaderId::new(leader))
}

fn holds_technology(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new(alias)))
}

/// A technology that is owned and not exhausted.
fn technology_ready(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    crate::technology::technology_text_ready(state, player, alias)
}

/// Every agent `player` holds, in leader-id order.
fn agents_of(state: &GameState, content: &ContentStore, player: &PlayerId) -> Vec<LeaderId> {
    state.player(player).map_or_else(Vec::new, |seat| {
        seat.leaders
            .keys()
            .filter(|leader| {
                crate::leaders::kind_of(content, leader).as_deref() == Some(crate::leaders::AGENT)
            })
            .cloned()
            .collect()
    })
}

/// Whether the engine should announce `AGENT_EXHAUSTED` events at all: only a seated Nomad that
/// could use one (Temporal Command Suite held) has any reason to hear them, so a game without
/// that card is unchanged.
#[must_use]
pub fn watches_agent_exhaustion(state: &GameState) -> bool {
    state
        .players
        .iter()
        .any(|seat| is_nomad(state, &seat.id) && holds_technology(state, &seat.id, TCS))
}

fn ask(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    prompt: String,
    ability: &str,
    subtype: &str,
    mut options: Vec<ChoiceOption>,
) -> Result<ChoiceOption, TimingError> {
    options.push(ChoiceOption::decline());
    let choice = Choice::new(player.clone(), prompt, options).contextualized(DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(ability.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    ));
    context
        .ask_seeing(&choice)
        .map_err(TimingError::IllegalChoice)
}

// -- Memoria: adjacent to systems with your mechs ------------------------------------------------

/// Memoria I/II: "You may treat this unit as if it were adjacent to systems that contain 1 or more
/// of your mechs." Applies to the flagship's own movement only (see
/// `MovementHooks::unit_adjacent_systems`): its route search treats each such system as a
/// neighbour of every step the flagship takes. "May" is the player's: a route that does not use
/// the adjacency is legal as before, so offering the extra neighbours takes nothing away.
fn memoria_adjacent(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    ship_type: &str,
) -> Vec<String> {
    if !FLAGSHIPS.contains(&ship_type) {
        return Vec::new();
    }
    let types = catalogue(content, sources);
    let is_mech = |unit: &ti4_model::units::Unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == "mech")
    };
    state
        .board
        .iter()
        .filter(|(_, board)| {
            board.units.iter().any(is_mech)
                || board
                    .planet_units
                    .values()
                    .any(|units| units.iter().any(is_mech))
        })
        .map(|(system, _)| system.to_string())
        .collect()
}

// -- Future Sight --------------------------------------------------------------------------------

/// Future Sight: "During the Agenda phase, after an outcome that you voted for or predicted is
/// resolved: Gain 1 trade good." Mandatory. Fires in the `AGENDA_RESOLVED` After window, where the
/// vote is mirrored on `agenda_votes` and a prediction (Imperial Rider and kin) is still held; an
/// agenda a Deadly Plot discarded has no resolved outcome and pays nothing.
fn future_sight(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("ability:{owner_name}:future_sight:AGENDA_RESOLVED:after"),
        seat.clone(),
        "AGENDA_RESOLVED",
        Relation::After,
        Arc::new(move |_event, resolver, context| {
            crate::supply::gain_trade_goods_via(context, resolver, &owner, 1, "future_sight")?;
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        let state = &*context.state;
        let Some(outcome) = event.text("player") else {
            return false;
        };
        is_nomad(state, &condition_owner)
            && !state.transient_flags.has(TransientFlags::AGENDA_DISCARDED)
            && (state.agenda_votes.get(&condition_owner).map(String::as_str) == Some(outcome)
                || state
                    .agenda_predictions
                    .get(&condition_owner)
                    .map(String::as_str)
                    == Some(outcome))
    }))
}

// -- Thunder's Paradox ---------------------------------------------------------------------------

/// Every `(exhausting, readied)` pair Thunder's Paradox could resolve: one of `owner`'s readied
/// agents, and a different agent (anyone's) that is exhausted.
fn paradox_pairs(
    state: &GameState,
    content: &ContentStore,
    owner: &PlayerId,
) -> Vec<(LeaderId, PlayerId, LeaderId)> {
    let mine: Vec<LeaderId> = agents_of(state, content, owner)
        .into_iter()
        .filter(|agent| leader_status(state, owner, agent.as_str()) == Some(LeaderStatus::Readied))
        .collect();
    let mut pairs = Vec::new();
    for spent in &mine {
        for seat in &state.players {
            for target in agents_of(state, content, &seat.id) {
                if (seat.id == *owner && target == *spent)
                    || leader_status(state, &seat.id, target.as_str())
                        != Some(LeaderStatus::Exhausted)
                {
                    continue;
                }
                pairs.push((spent.clone(), seat.id.clone(), target));
            }
        }
    }
    pairs
}

/// Thunder's Paradox: "At the start of any player's turn, you may exhaust 1 of your agents to ready
/// any other agent." The window's use/decline is the "may"; with several pairs possible the owner
/// then names one. Offered only when a readied agent of theirs and a different exhausted agent
/// both exist, so the use cannot spend an agent for nothing.
fn thunders_paradox(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("breakthrough:{owner_name}:nomadbt:TURN_BEGAN:after"),
        seat.clone(),
        "TURN_BEGAN",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            let pairs = paradox_pairs(context.state, context.content, &owner);
            let chosen = match pairs.as_slice() {
                [] => return Ok(()),
                [only] => only.clone(),
                _ => {
                    let options = pairs
                        .iter()
                        .enumerate()
                        .map(|(n, (spent, other, target))| {
                            ChoiceOption::labelled(
                                format!("pair|{n}"),
                                "nomadbt",
                                format!("exhaust {spent} to ready {other}'s {target}"),
                            )
                        })
                        .collect();
                    let answer = ask(
                        context,
                        &owner,
                        "Thunder's Paradox: exhaust which agent to ready which".to_owned(),
                        "nomadbt",
                        "thunders_paradox",
                        options,
                    )?;
                    let Some(pair) = pairs
                        .iter()
                        .enumerate()
                        .find(|(n, _)| answer.id == format!("pair|{n}"))
                        .map(|(_, pair)| pair.clone())
                    else {
                        return Ok(());
                    };
                    pair
                }
            };
            let (spent, other, target) = chosen;
            // Revalidated: the answer may have been given against a stale list.
            if crate::leaders::exhaust(context.state, &owner, &spent) {
                crate::leaders::ready(context.state, &other, &target);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        crate::breakthroughs::holds(context.state, &condition_owner, BREAKTHROUGH)
            && !paradox_pairs(context.state, context.content, &condition_owner).is_empty()
    }))
}

// -- Temporal Command Suite ----------------------------------------------------------------------

fn tcs_mark(owner: &PlayerId, other: &PlayerId) -> String {
    format!("nomad:tcs:negotiating:{owner}:{other}")
}

/// The transaction Temporal Command Suite allows reaches its partner without adjacency while it
/// is being negotiated.
fn tcs_reach(state: &GameState, _: &ContentStore, a: &PlayerId, b: &PlayerId) -> bool {
    state.faction_marks.contains_key(&tcs_mark(a, b))
        || state.faction_marks.contains_key(&tcs_mark(b, a))
}

/// Temporal Command Suite: "After any player's agent becomes exhausted, you may exhaust this card to
/// ready that agent; if you ready another player's agent, you may perform a transaction with that
/// player." Heard on [`AGENT_EXHAUSTED`] (payload `player` = the agent's owner, `leader`). The
/// window's use/decline is the "may" to ready; the transaction is a second optional question and
/// follows the readying.
fn temporal_command_suite(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("technology:{owner_name}:tcs:{AGENT_EXHAUSTED}:after"),
        seat.clone(),
        AGENT_EXHAUSTED,
        Relation::After,
        Arc::new(move |event, resolver, context| {
            let (Some(agent_owner), Some(agent)) = (
                event.text("player").map(PlayerId::new),
                event.text("leader").map(LeaderId::new),
            ) else {
                return Ok(());
            };
            if !technology_ready(context.state, &owner, TCS)
                || leader_status(context.state, &agent_owner, agent.as_str())
                    != Some(LeaderStatus::Exhausted)
            {
                return Ok(());
            }
            // The card's costs are paid before the optional transaction is asked; if the
            // transaction (or its window) fails, nothing of this use may remain.
            let snapshot = context.state.clone();
            let result = (|| -> Result<(), TimingError> {
                crate::technology::exhaust_technology_text(context.state, &owner, TCS);
                crate::leaders::ready(context.state, &agent_owner, &agent);
                if agent_owner == owner {
                    return Ok(());
                }
                let (Some(galaxy), true) = (
                    context.galaxy,
                    crate::transactions::may_open_again(
                        context.state,
                        context.content,
                        &owner,
                        &agent_owner,
                    ),
                ) else {
                    return Ok(());
                };
                let answer = ask(
                    context,
                    &owner,
                    format!("Temporal Command Suite: perform a transaction with {agent_owner}"),
                    TCS,
                    "tcs_transaction",
                    vec![ChoiceOption::labelled(
                        "transact",
                        "technology",
                        format!("transact with {agent_owner}"),
                    )],
                )?;
                if answer.id != "transact" {
                    return Ok(());
                }
                let key = tcs_mark(&owner, &agent_owner);
                context
                    .state
                    .faction_marks
                    .insert(key.clone(), "true".to_owned());
                let outcome = (|| -> Result<(), TimingError> {
                    let mut window = crate::transactions::TradeWindow::open_with_content(
                        context.state,
                        context.content,
                        &owner,
                        &agent_owner,
                    );
                    while !window.is_complete() {
                        let Some(choice) = window.pending_choice(context.state, context.content)
                        else {
                            break;
                        };
                        let answer = context
                            .ask_seeing(&choice)
                            .map_err(TimingError::IllegalChoice)?;
                        let result =
                            window.resolve(context.state, context.content, galaxy, &answer);
                        if result == crate::transactions::Traded::Resolved {
                            let payload = crate::transactions::resolved_payload(
                                window.parties().0,
                                window.parties().1,
                            );
                            let event = context
                                .event_sequence
                                .next("TRANSACTION_RESOLVED", payload)?;
                            resolver.emit_with_context(context, event, |_, _| {})?;
                        }
                    }
                    Ok(())
                })();
                context.state.faction_marks.remove(&key);
                outcome
            })();
            if result.is_err() {
                *context.state = snapshot;
            }
            result
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        let (Some(agent_owner), Some(agent)) = (
            event.text("player").map(PlayerId::new),
            event.text("leader").map(LeaderId::new),
        ) else {
            return false;
        };
        is_nomad(context.state, &condition_owner)
            && technology_ready(context.state, &condition_owner, TCS)
            && leader_status(context.state, &agent_owner, agent.as_str())
                == Some(LeaderStatus::Exhausted)
    }))
}

// -- hero ----------------------------------------------------------------------------------------

fn hero_mark(player: &PlayerId) -> String {
    format!("nomad:hero:{player}")
}

/// Whether Ahk-Syl Siven's effect is running for `player`: it was used in this game round.
fn hero_active(state: &GameState, player: &PlayerId) -> bool {
    state
        .faction_marks
        .get(&hero_mark(player))
        .map(String::as_str)
        == Some(state.round.to_string().as_str())
}

fn flagship_on_board(state: &GameState, player: &PlayerId) -> bool {
    state.board.values().any(|board| {
        board
            .units
            .iter()
            .any(|unit| &unit.owner == player && FLAGSHIPS.contains(&unit.type_id.as_str()))
    })
}

/// "Your flagship ... can move out of systems that contain your command tokens during this game
/// round" -- and, through `transit`, so can the units it transports (95.5 lifted for it).
fn hero_ignores_command_tokens(
    state: &GameState,
    _: &ContentStore,
    _: SourceSet,
    player: &PlayerId,
    ship_type: &str,
) -> bool {
    FLAGSHIPS.contains(&ship_type) && hero_active(state, player)
}

fn leader_action(
    state: &GameState,
    _: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == HERO)
        .then(|| !hero_active(state, player) && flagship_on_board(state, player))
}

/// Records the round the effect lasts through. The shared code purges the card as it is used; the
/// effect itself lives in the mark, which is what the movement rules read, so it still runs to the
/// end of the round and ends with it.
fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != HERO {
        return None;
    }
    if hero_active(context.state, player) || !flagship_on_board(context.state, player) {
        return Some(false);
    }
    let round = context.state.round.to_string();
    context.state.faction_marks.insert(hero_mark(player), round);
    Some(true)
}

// -- commander -----------------------------------------------------------------------------------

/// Navarch Feng unlocks with "Have 1 scored secret objective." The free flagship itself is
/// `production.rs`, gated on the unlocked card.
fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    _: SourceSet,
    _: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == COMMANDER)
        .then(|| crate::secrets::scored_count(state, content, player) >= 1)
}

fn timing_abilities(state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    let mut abilities = vec![
        future_sight(owner_name, seat),
        thunders_paradox(owner_name, seat),
        temporal_command_suite(owner_name, seat),
    ];
    abilities.extend(super::nomad_agents::timing_abilities(
        state, owner_name, seat,
    ));
    abilities
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::{BreakthroughId, ObjectiveId, PlanetId, SystemId, UnitTypeId};
    use ti4_model::units::Unit;

    use crate::choice::Window;
    use crate::movement::{Board, MovementRules};

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    fn content() -> &'static ContentStore {
        ContentStore::embedded()
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
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        event_type: &str,
        pairs: &[(&str, &str)],
    ) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, galaxy, table, |ctx| {
            let payload = pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
                .collect();
            let event = ctx
                .event_sequence
                .next(event_type, payload)
                .expect("an event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("the window resolves");
        });
    }

    fn set_status(state: &mut GameState, who: &PlayerId, leader: &str, status: LeaderStatus) {
        state
            .player_mut(who)
            .unwrap()
            .leaders
            .insert(LeaderId::new(leader), status);
    }

    fn status(state: &GameState, who: &PlayerId, leader: &str) -> Option<LeaderStatus> {
        leader_status(state, who, leader)
    }

    fn goods(state: &GameState, who: &PlayerId) -> i32 {
        state.player(who).unwrap().trade_goods
    }

    // -- The Company ------------------------------------------------------------------------------

    #[test]
    fn the_company_deals_three_readied_agents_beside_a_locked_commander_and_hero() {
        let state = game();
        let nomad = state.player(&a()).unwrap();
        for agent in [
            "nomadagentartuno",
            "nomadagentmercer",
            "nomadagentthundarian",
        ] {
            assert_eq!(
                nomad.leaders.get(&LeaderId::new(agent)),
                Some(&LeaderStatus::Readied),
                "{agent}"
            );
        }
        assert_eq!(
            nomad.leaders.get(&LeaderId::new(COMMANDER)),
            Some(&LeaderStatus::Locked)
        );
        assert_eq!(
            nomad.leaders.get(&LeaderId::new(HERO)),
            Some(&LeaderStatus::Locked)
        );
        assert_eq!(nomad.leaders.len(), 5);
        assert_eq!(agents_of(&state, content(), &a()).len(), 3);
        // Another faction keeps its single agent.
        assert_eq!(agents_of(&state, content(), &b()).len(), 1);
    }

    // -- Future Sight -----------------------------------------------------------------------------

    #[test]
    fn future_sight_pays_one_trade_good_for_a_voted_or_predicted_outcome_only() {
        let mut state = game();
        let before = (goods(&state, &a()), goods(&state, &b()));
        let resolved = [("agenda", "secret"), ("player", "for")];
        // Nobody voted for "for": nothing.
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "AGENDA_RESOLVED",
            &resolved,
        );
        assert_eq!(goods(&state, &a()), before.0);
        // Voted for another outcome: nothing.
        state.agenda_votes.insert(a(), "against".to_owned());
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "AGENDA_RESOLVED",
            &resolved,
        );
        assert_eq!(goods(&state, &a()), before.0);
        // Voted for the outcome: one trade good, to the Nomad only.
        state.agenda_votes.insert(a(), "for".to_owned());
        state.agenda_votes.insert(b(), "for".to_owned());
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "AGENDA_RESOLVED",
            &resolved,
        );
        assert_eq!(goods(&state, &a()), before.0 + 1);
        assert_eq!(goods(&state, &b()), before.1, "Sol has no Future Sight");
        // Predicted the outcome instead of voting.
        state.agenda_votes.clear();
        state.agenda_predictions.insert(a(), "for".to_owned());
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "AGENDA_RESOLVED",
            &resolved,
        );
        assert_eq!(goods(&state, &a()), before.0 + 2);
        // A discarded agenda resolves no outcome.
        state.transient_flags.set(TransientFlags::AGENDA_DISCARDED);
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "AGENDA_RESOLVED",
            &resolved,
        );
        assert_eq!(goods(&state, &a()), before.0 + 2);
    }

    #[test]
    fn future_sight_pays_through_a_real_vote_only_when_the_backed_outcome_wins() {
        // Both seats vote on a two-outcome agenda in a driven game. `b` votes first, then the
        // Nomad for "a". `b` backs "a" too and the Nomad is paid one trade good; or `b` backs "b"
        // with stronger planets, "b" wins, and the Nomad is paid nothing.
        for (strong_b, winner, paid) in [(0, "a", 1), (1, "b", 0)] {
            let mut state = game();
            state.phase = ti4_model::state::Phase::Agenda;
            state.custodians_removed = true;
            state.agenda_deck = vec!["secret".to_owned()];
            if strong_b > 0 {
                // Strong planets for `b`: it can out-vote Arcturus with them.
                let strong: Vec<(String, String)> =
                    ti4_content::galaxy::all_planets(content(), DEFAULT)
                        .iter()
                        .filter(|(_, planet)| {
                            planet.influence() >= 2
                                && !planet.is_placed_during_play()
                                && planet.homeworld_of().is_none()
                        })
                        .take(2)
                        .map(|(id, planet)| {
                            (
                                (*id).to_owned(),
                                planet.system_id().unwrap_or("18").to_owned(),
                            )
                        })
                        .collect();
                for (planet, system) in strong {
                    state
                        .system_mut(&SystemId::new(system))
                        .set_control(PlanetId::new(planet), b());
                }
            }
            let before = goods(&state, &a());
            // Only the first answer is scripted (`b` backs "b"); the rest take the first option.
            let table = if strong_b > 0 {
                scripted(&["b"])
            } else {
                scripted(&[])
            };
            let mut game = crate::game::Game::with_table(state, content(), table);
            let mut guard = 0;
            while !game
                .events
                .iter()
                .any(|e| e.starts_with("AGENDA_RESOLVED:secret:"))
                && guard < 200
            {
                assert_eq!(game.step().error, None);
                guard += 1;
            }
            assert!(
                game.events
                    .iter()
                    .any(|e| e == &format!("AGENDA_RESOLVED:secret:{winner}")),
                "log {:?}",
                game.events
            );
            assert_eq!(
                goods(&game.state, &a()),
                before + paid,
                "winner {winner}; log {:?}",
                game.events
            );
        }
    }

    // -- Memoria ----------------------------------------------------------------------------------

    /// A one-ring hub with the Nomad flagship on `outer[0]`, an enemy ship blocking the centre and
    /// the active system on the far side (`across(outer[0])`). Returns the hub, state, origin,
    /// active, and a ring neighbour of the active system (not the centre).
    fn blocked_hub(flagship: &str) -> (crate::fixtures::Hub, GameState, String, String, String) {
        let hub = crate::fixtures::plain_hub();
        let mut state = game();
        let origin = hub.outer[0].clone();
        let active = hub.across(&origin);
        let beside = hub
            .galaxy
            .adjacent(&active)
            .into_iter()
            .map(ToOwned::to_owned)
            .find(|id| id != &hub.centre)
            .expect("a ring neighbour");
        crate::fixtures::put(
            &mut state,
            &SystemId::new(origin.clone()),
            flagship,
            &a(),
            1,
        );
        crate::fixtures::put(
            &mut state,
            &SystemId::new(hub.centre.clone()),
            "destroyer",
            &b(),
            1,
        );
        (hub, state, origin, active, beside)
    }

    fn rules_for<'h>(
        hub: &'h crate::fixtures::Hub,
        state: &GameState,
        active: &str,
        player: &PlayerId,
    ) -> MovementRules<'h> {
        MovementRules::with_laws(
            &hub.galaxy,
            content(),
            DEFAULT,
            active,
            Board::for_player(state, content(), DEFAULT, player),
            Some(state),
        )
    }

    fn mech_on_planet(state: &mut GameState, system: &str, owner: &PlayerId) {
        state
            .system_mut(&SystemId::new(system))
            .planet_units
            .entry(PlanetId::new("test_planet"))
            .or_default()
            .push(Unit::new(UnitTypeId::new("nomad_mech"), owner.clone()));
    }

    #[test]
    fn memoria_is_adjacent_to_systems_with_your_mechs_for_the_flagship_only() {
        for flagship in FLAGSHIPS {
            let (hub, mut state, origin, active, beside) = blocked_hub(flagship);
            let reach = |state: &GameState, ship: &str, value: i32| {
                rules_for(&hub, state, &active, &a()).can_reach_ship(&origin, value, Some(ship))
            };
            // No mech anywhere: the blockaded centre leaves the long way round.
            assert!(
                !reach(&state, flagship, 2),
                "{flagship}: no mech, no shortcut"
            );
            // A mech next to the active system: origin -> there (adjacent by Memoria) -> active.
            mech_on_planet(&mut state, &beside, &a());
            assert!(
                reach(&state, flagship, 2),
                "{flagship}: adjacent to the mech"
            );
            assert!(
                !reach(&state, flagship, 1),
                "{flagship}: still costs a step"
            );
            assert!(
                !reach(&state, "carrier", 2),
                "only the flagship is adjacent"
            );
            // A mech in the active system itself: one step from anywhere.
            mech_on_planet(&mut state, &active, &a());
            assert!(
                reach(&state, flagship, 1),
                "{flagship}: the active system holds a mech"
            );
            // Another player's mech does nothing for the Nomad.
            let (hub2, mut other, origin2, active2, beside2) = blocked_hub(flagship);
            mech_on_planet(&mut other, &beside2, &b());
            assert!(
                !rules_for(&hub2, &other, &active2, &a()).can_reach_ship(
                    &origin2,
                    2,
                    Some(flagship)
                ),
                "{flagship}: a rival's mech is not yours"
            );
        }
    }

    #[test]
    fn memoria_is_offered_through_the_tactical_movement_list() {
        let (hub, mut state, origin, active, beside) = blocked_hub("nomad_flagship2");
        let movable = |state: &GameState| {
            crate::tactical::movable_into(
                state,
                content(),
                DEFAULT,
                &hub.galaxy,
                &a(),
                &SystemId::new(active.clone()),
            )
            .into_iter()
            .filter(|m| m.unit.type_id.as_str() == "nomad_flagship2" && m.origin.as_str() == origin)
            .count()
        };
        assert_eq!(movable(&state), 0, "blockaded, no mech");
        mech_on_planet(&mut state, &beside, &a());
        assert_eq!(
            movable(&state),
            1,
            "the flagship may use Memoria's adjacency"
        );
    }

    #[test]
    fn memoria_changes_nothing_in_a_game_without_the_nomad() {
        let hub = crate::fixtures::plain_hub();
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let origin = hub.outer[0].clone();
        let active = hub.across(&origin);
        crate::fixtures::put(
            &mut state,
            &SystemId::new(origin.clone()),
            "carrier",
            &a(),
            1,
        );
        crate::fixtures::put(
            &mut state,
            &SystemId::new(hub.centre.clone()),
            "destroyer",
            &b(),
            1,
        );
        let rules = rules_for(&hub, &state, &active, &a());
        let plain = MovementRules::with_laws(
            &hub.galaxy,
            content(),
            DEFAULT,
            &active,
            Board::for_player(&state, content(), DEFAULT, &a()),
            None,
        );
        for value in 0..4 {
            assert_eq!(
                rules.path_from_ship(&origin, value, Some("carrier")),
                plain.path_from_ship(&origin, value, Some("carrier"))
            );
        }
    }

    #[test]
    fn memoria_ii_is_the_data_driven_upgrade_with_the_printed_statistics() {
        let c = content();
        let one = ti4_content::units::unit_type(c, "nomad_flagship", DEFAULT).unwrap();
        let two = ti4_content::units::unit_type(c, "nomad_flagship2", DEFAULT).unwrap();
        assert_eq!((one.move_value(), one.capacity()), (1, 3));
        assert_eq!((two.move_value(), two.capacity()), (2, 6));
        let record = c
            .get(
                ti4_model::content_types::ContentType::Units,
                "nomad_flagship2",
            )
            .unwrap();
        assert_eq!(record.int("combatHitsOn"), Some(5));
        assert_eq!(record.int("combatDieCount"), Some(2));
        assert_eq!(record.int("afbHitsOn"), Some(5));
        assert_eq!(record.int("afbDieCount"), Some(3));
        assert!(record.flag("sustainDamage"));
        // Researching Memoria II turns the flagship on the board into Memoria II.
        let mut state = game();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new("m2"));
        crate::technology::apply_unit_upgrades(&mut state, c, DEFAULT, &a());
        let flagships: Vec<&str> = state
            .board
            .values()
            .flat_map(|board| board.units.iter())
            .filter(|unit| unit.owner == a() && unit.type_id.as_str().contains("flagship"))
            .map(|unit| unit.type_id.as_str())
            .collect();
        assert_eq!(flagships, ["nomad_flagship2"]);
    }

    // -- Ahk-Syl Siven ----------------------------------------------------------------------------

    fn unlock_hero(state: &mut GameState) {
        for n in 0..3 {
            state.record_score(&a(), ObjectiveId::new(format!("scored_{n}")));
        }
        let unlocked = crate::leaders::check_unlocks(state, content(), DEFAULT, None, &a());
        assert!(unlocked.contains(&LeaderId::new(HERO)));
    }

    fn hero_offered(state: &GameState) -> bool {
        crate::leaders::component_actions(state, content(), &a())
            .iter()
            .any(|o| o.id == format!("component|leader|{HERO}"))
    }

    #[test]
    fn the_hero_frees_the_flagship_and_its_cargo_from_command_tokens_for_the_round() {
        let hub = crate::fixtures::plain_hub();
        let mut state = game();
        let (origin, active) = (hub.outer[0].clone(), hub.outer[1].clone());
        let origin_id = SystemId::new(origin.clone());
        crate::fixtures::put(&mut state, &origin_id, "nomad_flagship", &a(), 1);
        crate::fixtures::put(&mut state, &origin_id, "fighter", &a(), 1);
        crate::fixtures::put(&mut state, &origin_id, "carrier", &a(), 1);
        state.system_mut(&origin_id).command_tokens.insert(a());
        state.round = 3;
        let movable = |state: &GameState, kind: &str| {
            crate::tactical::movable_into(
                state,
                content(),
                DEFAULT,
                &hub.galaxy,
                &a(),
                &SystemId::new(active.clone()),
            )
            .into_iter()
            .any(|m| m.unit.type_id.as_str() == kind && m.origin.as_str() == origin)
        };
        assert!(!movable(&state, "nomad_flagship"), "pinned by the token");
        assert!(!hero_offered(&state), "locked");
        unlock_hero(&mut state);
        assert!(
            hero_offered(&state),
            "unlocked with a flagship on the board"
        );
        let used = crate::fixtures::with_context(
            &mut state,
            DEFAULT,
            Some(&hub.galaxy),
            &mut scripted(&[]),
            |ctx| crate::leaders::use_leader(ctx, &a(), &LeaderId::new(HERO)),
        );
        assert!(used);
        assert_eq!(status(&state, &a(), HERO), Some(LeaderStatus::Purged));
        assert!(movable(&state, "nomad_flagship"), "freed this round");
        assert!(!movable(&state, "carrier"), "only the flagship");
        // The units it transports are loadable from the pinned system; without the hero they are not.
        let flagship = Unit::new(UnitTypeId::new("nomad_flagship"), a());
        let path = vec![origin.clone(), active.clone()];
        let cargo = |state: &GameState| {
            crate::transit::CargoWindow::for_ship(
                state,
                content(),
                DEFAULT,
                &a(),
                &origin_id,
                &flagship,
                &path,
            )
            .pending_choice()
            .map_or(0, |choice| choice.options.len())
        };
        assert!(cargo(&state) > 0, "the fighter may ride along");
        state.round = 4;
        assert!(
            !movable(&state, "nomad_flagship"),
            "the effect ends with the round"
        );
        assert_eq!(cargo(&state), 0, "and so does the cargo exception");
    }

    #[test]
    fn the_hero_is_not_offered_without_a_flagship_or_to_anyone_else() {
        let mut state = game();
        unlock_hero(&mut state);
        for board in state.board.values_mut() {
            board
                .units
                .retain(|unit| unit.type_id.as_str() != "nomad_flagship");
        }
        assert!(!hero_offered(&state), "nothing for the effect to free");
        let used =
            crate::fixtures::with_context(&mut state, DEFAULT, None, &mut scripted(&[]), |ctx| {
                crate::leaders::use_leader(ctx, &a(), &LeaderId::new(HERO))
            });
        assert!(!used);
        assert_eq!(status(&state, &a(), HERO), Some(LeaderStatus::Unlocked));
        assert!(state.faction_marks.is_empty());
        assert!(!hero_active(&state, &b()));
    }

    // -- Navarch Feng -----------------------------------------------------------------------------

    #[test]
    fn the_commander_unlocks_on_one_scored_secret_objective_only() {
        let mut state = game();
        let first = |kind| {
            content()
                .records(kind)
                .iter()
                .find_map(|r| r.text("alias").or_else(|| r.text("id")))
                .expect("an objective")
                .to_owned()
        };
        let secret = first(ti4_model::content_types::ContentType::SecretObjectives);
        let public = first(ti4_model::content_types::ContentType::PublicObjectives);
        state.record_score(&a(), ObjectiveId::new(public));
        crate::leaders::check_unlocks(&mut state, content(), DEFAULT, None, &a());
        assert_eq!(
            status(&state, &a(), COMMANDER),
            Some(LeaderStatus::Locked),
            "a public objective is not a secret"
        );
        state.record_score(&a(), ObjectiveId::new(secret));
        crate::leaders::check_unlocks(&mut state, content(), DEFAULT, None, &a());
        assert_eq!(
            status(&state, &a(), COMMANDER),
            Some(LeaderStatus::Unlocked)
        );
        assert_eq!(
            status(&state, &a(), HERO),
            Some(LeaderStatus::Locked),
            "two objectives"
        );
        assert!(crate::promissory::has_commander_ability(
            &state,
            &a(),
            COMMANDER
        ));
    }

    #[test]
    fn the_unlocked_commander_makes_the_flagship_free_in_production() {
        let mut state = game();
        let home = state.player(&a()).unwrap().home_system.clone().unwrap();
        for board in state.board.values_mut() {
            board
                .units
                .retain(|unit| !(unit.owner == a() && unit.type_id.as_str() == "nomad_flagship"));
        }
        state.player_mut(&a()).unwrap().trade_goods = 0;
        let planets: Vec<PlanetId> = state
            .controlled_planets(&a())
            .into_iter()
            .map(|(_, p)| p.clone())
            .collect();
        state.exhausted_planets.extend(planets);
        let offer = |state: &GameState| {
            let window = crate::production::ProductionWindow::for_ability(
                state,
                content(),
                DEFAULT,
                &a(),
                &home,
                Some(2),
            );
            window
                .pending_choice(state, content(), DEFAULT)
                .and_then(|choice| {
                    choice
                        .options
                        .into_iter()
                        .find(|option| option.id == "build|nomad_flagship|1")
                })
        };
        assert!(offer(&state).is_none(), "locked: nothing to pay with");
        set_status(&mut state, &a(), COMMANDER, LeaderStatus::Unlocked);
        let option = offer(&state).expect("free flagship");
        assert_eq!(option.payload["cost"], 0);
    }

    // -- Thunder's Paradox ------------------------------------------------------------------------

    const PARADOX: &str = "breakthrough:nomad:nomadbt:TURN_BEGAN:after";

    fn with_paradox(state: &mut GameState) {
        state.player_mut(&a()).unwrap().breakthrough = Some(BreakthroughId::new(BREAKTHROUGH));
    }

    fn agents_ready(state: &GameState, who: &PlayerId) -> usize {
        agents_of(state, content(), who)
            .iter()
            .filter(|agent| status(state, who, agent.as_str()) == Some(LeaderStatus::Readied))
            .count()
    }

    #[test]
    fn thunders_paradox_exhausts_one_agent_to_ready_another_players() {
        let mut state = game();
        with_paradox(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        // Three of the Nomad's agents could be spent; the owner names Artuno.
        emit(
            &mut state,
            &mut scripted(&[PARADOX, "pair|0"]),
            None,
            "TURN_BEGAN",
            &[("player", "b")],
        );
        assert_eq!(
            status(&state, &a(), "nomadagentartuno"),
            Some(LeaderStatus::Exhausted)
        );
        assert_eq!(
            status(&state, &a(), "nomadagentmercer"),
            Some(LeaderStatus::Readied)
        );
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Readied)
        );
    }

    #[test]
    fn thunders_paradox_may_ready_one_of_its_own_other_agents() {
        let mut state = game();
        with_paradox(&mut state);
        set_status(
            &mut state,
            &a(),
            "nomadagentmercer",
            LeaderStatus::Exhausted,
        );
        emit(
            &mut state,
            &mut scripted(&[PARADOX, "pair|1"]),
            None,
            "TURN_BEGAN",
            &[("player", "a")],
        );
        assert_eq!(
            status(&state, &a(), "nomadagentmercer"),
            Some(LeaderStatus::Readied)
        );
        assert_eq!(agents_ready(&state, &a()), 2, "exactly one agent paid");
    }

    #[test]
    fn thunders_paradox_is_not_offered_without_the_breakthrough_or_a_target() {
        // No breakthrough: nothing.
        let mut state = game();
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        let before = state.players.clone();
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "TURN_BEGAN",
            &[("player", "b")],
        );
        assert_eq!(state.players, before);
        // Breakthrough but nothing exhausted: nothing to ready, so nothing is spent.
        let mut state = game();
        with_paradox(&mut state);
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "TURN_BEGAN",
            &[("player", "b")],
        );
        assert_eq!(agents_ready(&state, &a()), 3);
        // Breakthrough and a target, but declined.
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            None,
            "TURN_BEGAN",
            &[("player", "b")],
        );
        assert_eq!(agents_ready(&state, &a()), 3);
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Exhausted)
        );
        // No readied agent of the Nomad's own to exhaust.
        for agent in [
            "nomadagentartuno",
            "nomadagentmercer",
            "nomadagentthundarian",
        ] {
            set_status(&mut state, &a(), agent, LeaderStatus::Exhausted);
        }
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            "TURN_BEGAN",
            &[("player", "b")],
        );
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Exhausted)
        );
    }

    // -- Temporal Command Suite -------------------------------------------------------------------

    const TCS_ABILITY: &str = "technology:nomad:tcs:AGENT_EXHAUSTED:after";

    fn with_tcs(state: &mut GameState) {
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(TCS));
    }

    #[test]
    fn tcs_readies_an_exhausted_agent_and_exhausts_itself() {
        let mut state = game();
        with_tcs(&mut state);
        set_status(
            &mut state,
            &a(),
            "nomadagentmercer",
            LeaderStatus::Exhausted,
        );
        emit(
            &mut state,
            &mut scripted(&[TCS_ABILITY]),
            None,
            AGENT_EXHAUSTED,
            &[("player", "a"), ("leader", "nomadagentmercer")],
        );
        assert_eq!(
            status(&state, &a(), "nomadagentmercer"),
            Some(LeaderStatus::Readied)
        );
        assert!(
            state
                .player(&a())
                .unwrap()
                .exhausted_technologies
                .contains(&TechnologyId::new(TCS))
        );
        // Exhausted: the next exhaustion is not offered.
        set_status(
            &mut state,
            &a(),
            "nomadagentmercer",
            LeaderStatus::Exhausted,
        );
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            AGENT_EXHAUSTED,
            &[("player", "a"), ("leader", "nomadagentmercer")],
        );
        assert_eq!(
            status(&state, &a(), "nomadagentmercer"),
            Some(LeaderStatus::Exhausted)
        );
    }

    #[test]
    fn tcs_readies_another_players_agent_then_offers_a_transaction_beyond_neighbours() {
        let hub = crate::fixtures::plain_hub();
        let mut state = game();
        with_tcs(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        assert!(
            !crate::transactions::may_transact(&state, content(), &hub.galaxy, &a(), &b()),
            "not neighbours on this map"
        );
        let (decider, seen) =
            crate::choice::Capturing::new(Box::new(crate::choice::Scripted::new([
                TCS_ABILITY,
                "transact",
                "decline",
            ])));
        emit(
            &mut state,
            &mut crate::choice::Table::with_default(Box::new(decider)),
            Some(&hub.galaxy),
            AGENT_EXHAUSTED,
            &[("player", "b"), ("leader", "solagent")],
        );
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Readied)
        );
        let asked = seen.borrow();
        assert_eq!(asked.len(), 3, "window, transact?, then the deal itself");
        assert!(
            asked[2].ids().len() > 1,
            "the non-neighbour was offered real deals, not an empty table"
        );
        assert!(
            state.transacted_with(&a()).contains(&b()),
            "the transaction was opened with a non-neighbour"
        );
        assert!(
            !state
                .faction_marks
                .keys()
                .any(|k| k.starts_with("nomad:tcs")),
            "the reach mark does not outlive the transaction"
        );
        assert!(!tcs_reach(&state, content(), &a(), &b()));
    }

    #[test]
    fn a_staged_agent_exhaustion_reaches_tcs_through_the_staged_event_flush() {
        // The route `leaders::exhaust` takes once it stages the event: stage, flush with a timing
        // handle, and the Nomad's window opens.
        let mut state = game();
        with_tcs(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        let payload = std::collections::BTreeMap::from([
            ("player".to_owned(), serde_json::Value::from("b")),
            ("leader".to_owned(), serde_json::Value::from("solagent")),
        ]);
        assert!(crate::supply::stage_event(
            &mut state,
            AGENT_EXHAUSTED,
            &payload
        ));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(0);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = scripted(&[TCS_ABILITY]);
        let mut ctx = crate::choice::Resolving {
            content: content(),
            sources: DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        assert_eq!(crate::supply::flush_staged_events(&mut state, &mut ctx), 1);
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Readied)
        );
    }

    #[test]
    fn tcs_declining_the_transaction_leaves_only_the_readied_agent() {
        let hub = crate::fixtures::plain_hub();
        let mut state = game();
        with_tcs(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        emit(
            &mut state,
            &mut scripted(&[TCS_ABILITY, "decline"]),
            Some(&hub.galaxy),
            AGENT_EXHAUSTED,
            &[("player", "b"), ("leader", "solagent")],
        );
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Readied)
        );
        assert!(state.transacted_with(&a()).is_empty());
    }

    #[test]
    fn tcs_error_in_the_transaction_leaves_the_agent_and_the_card_untouched() {
        let hub = crate::fixtures::plain_hub();
        let mut state = game();
        with_tcs(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        let before = state.clone();
        let mut table = scripted(&[TCS_ABILITY, "transact", "not-an-offered-deal"]);
        let mut resolver = crate::fixtures::armed_resolver(&mut state);
        let result = crate::fixtures::with_context(
            &mut state,
            DEFAULT,
            Some(&hub.galaxy),
            &mut table,
            |ctx| {
                let payload = [
                    ("player".to_owned(), serde_json::Value::from("b")),
                    ("leader".to_owned(), serde_json::Value::from("solagent")),
                ]
                .into_iter()
                .collect();
                let event = ctx
                    .event_sequence
                    .next(AGENT_EXHAUSTED, payload)
                    .expect("an event id");
                resolver.emit_with_context(ctx, event, |_, _| {})
            },
        );
        assert!(result.is_err(), "an unoffered deal answer is refused");
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Exhausted),
            "the agent is not left readied"
        );
        assert!(
            !state
                .player(&a())
                .unwrap()
                .exhausted_technologies
                .contains(&TechnologyId::new(TCS)),
            "the card is not left exhausted"
        );
        assert_eq!(
            state.faction_marks, before.faction_marks,
            "no negotiating mark remains"
        );
    }

    #[test]
    fn tcs_is_not_heard_without_the_technology_or_for_a_readied_agent() {
        let mut state = game();
        set_status(&mut state, &b(), "solagent", LeaderStatus::Exhausted);
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            AGENT_EXHAUSTED,
            &[("player", "b"), ("leader", "solagent")],
        );
        assert_eq!(
            status(&state, &b(), "solagent"),
            Some(LeaderStatus::Exhausted)
        );
        with_tcs(&mut state);
        set_status(&mut state, &b(), "solagent", LeaderStatus::Readied);
        emit(
            &mut state,
            &mut scripted(&[]),
            None,
            AGENT_EXHAUSTED,
            &[("player", "b"), ("leader", "solagent")],
        );
        assert!(
            state
                .player(&a())
                .unwrap()
                .exhausted_technologies
                .is_empty()
        );
        assert!(!watches_agent_exhaustion(&crate::fixtures::seated_game(
            &[("a", "sol"), ("b", "hacan")],
            DEFAULT
        )));
        assert!(watches_agent_exhaustion(&state));
    }

    // -- games without the Nomad ------------------------------------------------------------------

    #[test]
    fn no_nomad_window_opens_in_a_game_without_the_faction() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        state.agenda_votes.insert(a(), "for".to_owned());
        set_status(&mut state, &b(), "hacanagent", LeaderStatus::Exhausted);
        let before = state.players.clone();
        for (event, pairs) in [
            (
                "AGENDA_RESOLVED",
                vec![("agenda", "secret"), ("player", "for")],
            ),
            ("TURN_BEGAN", vec![("player", "a")]),
            (
                AGENT_EXHAUSTED,
                vec![("player", "b"), ("leader", "hacanagent")],
            ),
        ] {
            emit(&mut state, &mut scripted(&[]), None, event, &pairs);
        }
        assert_eq!(state.players, before);
        assert!(state.faction_marks.is_empty());
    }
}
