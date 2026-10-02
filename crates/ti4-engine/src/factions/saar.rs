//! The Clan of Saar (`saar`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-saar.md`.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Scavenge: "After you gain control of a planet: Gain 1 trade good."
//! * Scavenger Zeta (mech): "DEPLOY: After you gain control of a planet, you may spend 1 trade
//!   good to place 1 mech on that planet"
//! * Ragh's Call (`ragh`): "After you commit 1 or more units to land on a planet: Remove all of
//!   the Saar player's ground forces from that planet and place them on a planet controlled by the
//!   Saar player. Then, return this card to the Saar player."
//! * Chaos Mapping (`cm`): "Other players cannot activate asteroid fields that contain 1 or more
//!   of your ships. At the start of your turn during the action phase, you may produce 1 unit in a
//!   system that contains at least 1 of your units that has PRODUCTION." (second sentence only;
//!   the activation bar needs a shared hook, see the evidence file)
//! * Gurno Aggero (`saarhero`): "ACTION: Choose 1 system that is adjacent to 1 of your space
//!   docks. Destroy all other players' infantry and fighters in that system. Then, purge this
//!   card." Unlock: "Have 3 scored objectives."
//! * Rowl Sarring (`saarcommander`) unlock: "Have 3 space docks on the game board."
//! * Son of Ragh (flagship): ANTI-FIGHTER BARRAGE 6 (x4) -- statistics only.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption, Resolving, TimingHandle, Window};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::production::ProductionWindow;
use crate::timing::{Ability, Relation, Resolver, TimingContext, TimingError};

/// The faction alias; also the faction name in promissory note ids (`ragh:saar`).
const FACTION: &str = "saar";
/// Ragh's Call's note id.
const RAGH_NOTE: &str = "ragh:saar";

/// What this faction implements.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &[],
    technologies: &[],
    units: &["saar_flagship"],
    promissory: &["ragh"],
    leaders: &["saarhero"],
    breakthroughs: &[],
    hooks: Hooks {
        timing_abilities: Some(timing_abilities),
        commander_unlocked: Some(commander_unlocked),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn is_saar(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn has_technology(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    state.player(player).is_some_and(|seat| {
        seat.technologies
            .contains(&ti4_model::id::TechnologyId::new(alias))
    })
}

/// The Saar seat, if one is playing.
fn saar_seat(state: &GameState) -> Option<PlayerId> {
    state
        .players
        .iter()
        .find(|seat| seat.faction.as_str() == FACTION)
        .map(|seat| seat.id.clone())
}

fn illegal(error: crate::choice::IllegalChoice) -> TimingError {
    TimingError::IllegalChoice(error)
}

fn ask(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    prompt: String,
    ability: &str,
    subtype: &str,
    mut options: Vec<ChoiceOption>,
    optional: bool,
) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
    if optional {
        options.push(ChoiceOption::decline());
    }
    let choice = Choice::new(player.clone(), prompt, options).contextualized(DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(ability.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    ));
    context.ask_seeing(&choice)
}

/// Every unit of `player` (space and planets) whose base type is `base`, with where it stands.
fn units_of_base(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    base: &str,
) -> Vec<(SystemId, Option<PlanetId>, Unit)> {
    let types = catalogue(content, sources);
    let is = |unit: &Unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == base)
    };
    let mut found = Vec::new();
    for (system, board) in &state.board {
        for unit in board.units.iter().filter(|unit| is(unit)) {
            found.push((system.clone(), None, unit.clone()));
        }
        for (planet, units) in &board.planet_units {
            for unit in units.iter().filter(|unit| is(unit)) {
                found.push((system.clone(), Some(planet.clone()), unit.clone()));
            }
        }
    }
    found
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    crate::leaders::status(state, player, &LeaderId::new(leader))
}

// -- Rowl Sarring unlock -------------------------------------------------------------------------

/// "Have 3 space docks on the game board." A Floating Factory is a space dock (same base type).
fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != "saarcommander" {
        return None;
    }
    Some(units_of_base(state, content, sources, player, "spacedock").len() >= 3)
}

// -- timing abilities ----------------------------------------------------------------------------

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    let mut abilities = vec![
        scavenge(owner_name, seat),
        scavenger_zeta(owner_name, seat),
        ragh_call(owner_name, seat),
    ];
    // Chaos Mapping is half implemented (the activation bar needs a shared hook), so its
    // production is registered only under test until `cm` can be claimed whole.
    if cfg!(test) {
        abilities.push(chaos_mapping_production(owner_name, seat));
    }
    abilities
}

fn gained_by(event: &crate::event::Event, player: &PlayerId) -> Option<(SystemId, PlanetId)> {
    if event.text("player") != Some(player.as_str()) {
        return None;
    }
    Some((
        SystemId::new(event.text("system")?),
        PlanetId::new(event.text("planet")?),
    ))
}

/// Scavenge: "After you gain control of a planet: Gain 1 trade good." Mandatory.
fn scavenge(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("ability:{owner_name}:scavenge:PLANET_CONTROL_GAINED:after"),
        seat.clone(),
        "PLANET_CONTROL_GAINED",
        Relation::After,
        Arc::new(move |_event, resolver, context| {
            crate::supply::gain_trade_goods_via(context, resolver, &owner, 1, "scavenge")?;
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        crate::faction_abilities::has(context.state, context.content, &condition_owner, "scavenge")
            && gained_by(event, &condition_owner).is_some()
    }))
}

fn mech_id(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Option<UnitTypeId> {
    let faction = state.player(player)?.faction.to_string();
    ti4_content::units::faction_unit(content, &faction, "mech", sources)
        .map(|unit| UnitTypeId::new(unit.id().to_owned()))
}

/// Scavenger Zeta DEPLOY: "After you gain control of a planet, you may spend 1 trade good to place
/// 1 mech on that planet." The window's own use/decline is the "may".
fn scavenger_zeta(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("unit:{owner_name}:saar_mech:PLANET_CONTROL_GAINED:after"),
        seat.clone(),
        "PLANET_CONTROL_GAINED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some((system, planet)) = gained_by(event, &owner) else {
                return Ok(());
            };
            if !deploy_ready(context.state, context.content, context.sources, &owner) {
                return Ok(());
            }
            if let Some(seat) = context.state.player_mut(&owner) {
                seat.trade_goods -= 1;
            }
            crate::action_cards::place_units_counted(
                context,
                &owner,
                &system,
                Some(&planet),
                "mech",
                1,
            );
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        gained_by(event, &condition_owner).is_some()
            && deploy_ready(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
    }))
}

/// A trade good to spend and a mech left in the box (31.4).
fn deploy_ready(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> bool {
    is_saar(state, owner)
        && state
            .player(owner)
            .is_some_and(|seat| seat.trade_goods >= 1)
        && mech_id(state, content, sources, owner)
            .is_some_and(|id| crate::supply::allowed(state, content, sources, owner, &id, 1) > 0)
}

// -- Ragh's Call ---------------------------------------------------------------------------------

/// The Saar ground forces on a planet, and the Saar player's other planets they can go to.
#[allow(
    clippy::type_complexity,
    reason = "one private tuple: saar seat, its forces, its destinations"
)]
fn ragh_targets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    holder: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> Option<(PlayerId, Vec<Unit>, Vec<(SystemId, PlanetId)>)> {
    if crate::promissory::held_foreign(state, holder, "ragh") == 0 {
        return None;
    }
    let saar = saar_seat(state)?;
    if &saar == holder {
        return None;
    }
    let types = catalogue(content, sources);
    let forces: Vec<Unit> = state
        .system_state(system)
        .on_planet_of(planet, &saar)
        .into_iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force)
        })
        .cloned()
        .collect();
    if forces.is_empty() {
        return None;
    }
    let destinations: Vec<(SystemId, PlanetId)> = state
        .controlled_planets(&saar)
        .into_iter()
        .filter(|(to_system, to_planet)| !(*to_system == system && *to_planet == planet))
        .map(|(to_system, to_planet)| (to_system.clone(), to_planet.clone()))
        .collect();
    if destinations.is_empty() {
        return None;
    }
    Some((saar, forces, destinations))
}

/// Ragh's Call, played by its holder after committing units to a planet. The Saar player picks the
/// destination (it is "a planet controlled by the Saar player"); the choice is asked before any
/// unit moves.
fn ragh_call(owner_name: &str, seat: &PlayerId) -> Ability {
    let holder = seat.clone();
    let condition_holder = seat.clone();
    Ability::stateful(
        format!("promissory:{owner_name}:ragh:UNITS_COMMITTED:after"),
        seat.clone(),
        "UNITS_COMMITTED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some((system, planet)) = gained_by(event, &holder) else {
                return Ok(());
            };
            let Some((saar, forces, destinations)) = ragh_targets(
                context.state,
                context.content,
                context.sources,
                &holder,
                &system,
                &planet,
            ) else {
                return Ok(());
            };
            let options = destinations
                .iter()
                .map(|(to_system, to_planet)| {
                    ChoiceOption::labelled(
                        format!("{to_system}|{to_planet}"),
                        "planet",
                        format!("move the Saar ground forces to {to_planet}"),
                    )
                })
                .collect();
            let answer = ask(
                context,
                &saar,
                format!("Ragh's Call: where do the Saar ground forces on {planet} go"),
                "ragh",
                "ragh_destination",
                options,
                false,
            )
            .map_err(illegal)?;
            let Some((to_system, to_planet)) = destinations
                .iter()
                .find(|(s, p)| answer.id == format!("{s}|{p}"))
            else {
                return Ok(());
            };
            context
                .state
                .system_mut(&system)
                .remove_from_planet(&planet, &forces);
            context
                .state
                .system_mut(to_system)
                .planet_units
                .entry(to_planet.clone())
                .or_default()
                .extend(forces);
            crate::promissory::give_back(context.state, RAGH_NOTE);
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        gained_by(event, &condition_holder).is_some_and(|(system, planet)| {
            ragh_targets(
                context.state,
                context.content,
                context.sources,
                &condition_holder,
                &system,
                &planet,
            )
            .is_some()
        })
    }))
}

// -- Chaos Mapping: start-of-turn production -----------------------------------------------------

/// Systems where `player` could produce at least 1 unit by ability now.
fn production_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<SystemId> {
    state
        .board
        .keys()
        .filter(|system| {
            !crate::production::producers(state, content, sources, player, system).is_empty()
                && ProductionWindow::for_ability(state, content, sources, player, system, Some(1))
                    .pending_choice(state, content, sources)
                    .is_some()
        })
        .cloned()
        .collect()
}

fn chaos_ready(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> bool {
    has_technology(state, owner, "cm")
        && state.phase == ti4_model::state::Phase::Action
        && !production_systems(state, content, sources, owner).is_empty()
}

/// "At the start of your turn during the action phase, you may produce 1 unit in a system that
/// contains at least 1 of your units that has PRODUCTION."
fn chaos_mapping_production(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("technology:{owner_name}:cm:TURN_BEGAN:after"),
        seat.clone(),
        "TURN_BEGAN",
        Relation::After,
        Arc::new(move |event, resolver, context| {
            if event.text("player") != Some(owner.as_str())
                || !chaos_ready(context.state, context.content, context.sources, &owner)
            {
                return Ok(());
            }
            let systems =
                production_systems(context.state, context.content, context.sources, &owner);
            let system = if let [only] = systems.as_slice() {
                only.clone()
            } else {
                let options = systems
                    .iter()
                    .map(|s| ChoiceOption::labelled(s.to_string(), "system", format!("system {s}")))
                    .collect();
                let answer = ask(
                    context,
                    &owner,
                    "Chaos Mapping: produce 1 unit in which system".to_owned(),
                    "cm",
                    "chaos_system",
                    options,
                    true,
                )
                .map_err(illegal)?;
                if answer.is_decline() {
                    return Ok(());
                }
                let Some(picked) = systems.iter().find(|s| s.as_str() == answer.id) else {
                    return Ok(());
                };
                picked.clone()
            };
            produce(context, resolver, &owner, &system)
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && chaos_ready(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
    }))
}

fn produce(
    context: &mut TimingContext<'_>,
    resolver: &mut Resolver,
    player: &PlayerId,
    system: &SystemId,
) -> Result<(), TimingError> {
    let TimingContext {
        state,
        content,
        sources,
        table,
        dice,
        rng,
        event_sequence,
        galaxy,
    } = context;
    let galaxy = *galaxy;
    let mut ctx = Resolving {
        content,
        sources: *sources,
        dice,
        rng,
        table,
        timing: Some(TimingHandle {
            resolver,
            sequence: event_sequence,
            galaxy,
        }),
    };
    crate::production::produce_by_ability(state, &mut ctx, galaxy, player, system, Some(1))
        .map_err(TimingError::IllegalChoice)?;
    Ok(())
}

// -- Gurno Aggero --------------------------------------------------------------------------------

fn dock_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<SystemId> {
    let mut found: Vec<SystemId> = units_of_base(state, content, sources, player, "spacedock")
        .into_iter()
        .map(|(system, _, _)| system)
        .collect();
    found.dedup();
    found
}

fn leader_action(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == "saarhero").then(|| {
        // Any adjacent system is a legal choice, so the action can resolve whenever a dock stands.
        !dock_systems(state, content, ti4_model::content_types::DEFAULT, player).is_empty()
            && leader_status(state, player, "saarhero").is_some()
    })
}

fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == "saarhero").then(|| hero(context, player))
}

/// "Choose 1 system that is adjacent to 1 of your space docks. Destroy all other players' infantry
/// and fighters in that system." The system is chosen before anything is destroyed.
fn hero(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let Some(galaxy) = context.galaxy else {
        return false;
    };
    let docks = dock_systems(context.state, context.content, context.sources, player);
    let mut candidates: Vec<SystemId> = Vec::new();
    let adjacency = crate::movement::PlayerAdjacency::new(
        context.state,
        context.content,
        context.sources,
        galaxy,
        player,
    );
    for dock in &docks {
        for neighbour in adjacency.neighbours(dock.as_str()) {
            let neighbour = SystemId::new(neighbour);
            if !candidates.contains(&neighbour) {
                candidates.push(neighbour);
            }
        }
    }
    candidates.sort();
    if candidates.is_empty() {
        return false;
    }
    let options = candidates
        .iter()
        .map(|s| ChoiceOption::labelled(s.to_string(), "system", format!("system {s}")))
        .collect();
    let Ok(answer) = ask(
        context,
        player,
        "Gurno Aggero: destroy all other players' infantry and fighters in which system".to_owned(),
        "saarhero",
        "armageddon_relay",
        options,
        false,
    ) else {
        return false;
    };
    let Some(system) = candidates.iter().find(|s| s.as_str() == answer.id).cloned() else {
        return false;
    };
    // Decide everything first, then remove. Fighters and infantry alike are removed directly (no
    // `pending_destructions` entry is staged: a leader effect has no way to announce them).
    let mut doomed: Vec<(Option<PlanetId>, Unit)> = Vec::new();
    for seat in &context.state.players {
        if &seat.id == player {
            continue;
        }
        for base in ["fighter", "infantry"] {
            for (at, planet, unit) in units_of_base(
                context.state,
                context.content,
                context.sources,
                &seat.id,
                base,
            ) {
                if at == system {
                    doomed.push((planet, unit));
                }
            }
        }
    }
    for (planet, unit) in doomed {
        let board = context.state.system_mut(&system);
        match planet {
            Some(planet) => board.remove_from_planet(&planet, std::slice::from_ref(&unit)),
            None => board.remove(std::slice::from_ref(&unit)),
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn emit(
        state: &mut GameState,
        table: &mut crate::choice::Table,
        event_type: &str,
        pairs: &[(&str, &str)],
    ) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        crate::fixtures::with_context(state, DEFAULT, None, table, |ctx| {
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

    fn scripted(answers: &[&str]) -> crate::choice::Table {
        crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(
            answers.iter().map(|s| (*s).to_owned()),
        )))
    }

    fn trade_goods(state: &GameState, who: &PlayerId) -> i32 {
        state.player(who).unwrap().trade_goods
    }

    /// A planet in the Saar home system, as `(system, planet)`.
    fn home_planet(state: &GameState, who: &PlayerId) -> (SystemId, PlanetId) {
        let (s, p) = state.controlled_planets(who)[0];
        (s.clone(), p.clone())
    }

    fn mechs(state: &GameState, system: &SystemId, planet: &PlanetId, who: &PlayerId) -> usize {
        state
            .system_state(system)
            .on_planet_of(planet, who)
            .iter()
            .filter(|unit| unit.type_id.as_str() == "saar_mech")
            .count()
    }

    #[test]
    fn scavenge_pays_a_trade_good_for_each_planet_gained_and_only_to_the_gainer() {
        let mut state = game();
        let (system, planet) = home_planet(&state, &a());
        let before = (trade_goods(&state, &a()), trade_goods(&state, &b()));
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(trade_goods(&state, &a()), before.0 + 1);
        assert_eq!(trade_goods(&state, &b()), before.1);
        // The other player gaining a planet pays Saar nothing.
        emit(
            &mut state,
            &mut scripted(&[]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "b"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(trade_goods(&state, &a()), before.0 + 1);
    }

    #[test]
    fn scavenger_zeta_costs_a_trade_good_and_places_a_mech_or_is_declined() {
        let mut state = game();
        let (system, planet) = home_planet(&state, &a());
        let id = "unit:saar:saar_mech:PLANET_CONTROL_GAINED:after";
        let base = mechs(&state, &system, &planet, &a());
        let tg = trade_goods(&state, &a());
        emit(
            &mut state,
            &mut scripted(&[id]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(mechs(&state, &system, &planet, &a()), base + 1);
        assert_eq!(trade_goods(&state, &a()), tg, "+1 Scavenge, -1 DEPLOY");
        // Declined: the mech is not placed and the trade good is kept.
        let mut state = game();
        let tg = trade_goods(&state, &a());
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "PLANET_CONTROL_GAINED",
            &[
                ("player", "a"),
                ("system", system.as_str()),
                ("planet", planet.as_str()),
            ],
        );
        assert_eq!(mechs(&state, &system, &planet, &a()), base);
        assert_eq!(trade_goods(&state, &a()), tg + 1);
    }

    #[test]
    fn the_stats_of_the_flagship_are_data_driven() {
        let content = ContentStore::embedded();
        let types = catalogue(content, DEFAULT);
        let flagship = types.get("saar_flagship").expect("flagship");
        assert!(flagship.is_ship());
        assert_eq!(flagship.base_type(), "flagship");
    }

    #[test]
    fn the_commander_unlocks_at_three_space_docks() {
        let content = ContentStore::embedded();
        let mut state = game();
        let leader = LeaderId::new("saarcommander");
        let (system, planet) = home_planet(&state, &a());
        let unlocked =
            |state: &GameState| commander_unlocked(state, content, DEFAULT, None, &a(), &leader);
        let docks = units_of_base(&state, content, DEFAULT, &a(), "spacedock").len();
        assert!(docks < 3);
        for _ in docks..2 {
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "saar_spacedock", &a(), 1);
        }
        assert_eq!(unlocked(&state), Some(false), "two docks");
        // A dock in the space area counts (a Floating Factory).
        state
            .system_mut(&system)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        assert_eq!(unlocked(&state), Some(true), "three docks");
        assert_eq!(
            commander_unlocked(&state, content, DEFAULT, None, &a(), &LeaderId::new("x")),
            None
        );
    }

    #[test]
    fn a_game_without_saar_is_offered_no_saar_ability() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let (system, planet) = {
            let (s, p) = state.controlled_planets(&a())[0];
            (s.clone(), p.clone())
        };
        let before = state.clone();
        for event in ["PLANET_CONTROL_GAINED", "UNITS_COMMITTED", "TURN_BEGAN"] {
            // An empty script fails the test if any choice is asked.
            emit(
                &mut state,
                &mut scripted(&[]),
                event,
                &[
                    ("player", "a"),
                    ("system", system.as_str()),
                    ("planet", planet.as_str()),
                ],
            );
        }
        assert_eq!(
            state.player(&a()).unwrap().trade_goods,
            before.player(&a()).unwrap().trade_goods
        );
        assert_eq!(state.board, before.board);
        let content = ContentStore::embedded();
        assert_eq!(
            leader_action(&state, content, &a(), &LeaderId::new("saarhero")),
            Some(false)
        );
    }

    #[test]
    fn ragh_s_call_moves_the_saar_forces_to_a_planet_they_control_and_returns_the_note() {
        let mut state = game();
        let (home_system, home_planet_id) = home_planet(&state, &a());
        let (their_system, their_planet) = home_planet(&state, &b());
        crate::fixtures::put_on_planet(
            &mut state,
            &their_system,
            &their_planet,
            "infantry",
            &a(),
            2,
        );
        state.promissory_notes.insert(RAGH_NOTE.to_owned(), b());
        let destination = format!("{home_system}|{home_planet_id}");
        let at_home = |state: &GameState| {
            state
                .system_state(&home_system)
                .on_planet_of(&home_planet_id, &a())
                .len()
        };
        let home_before = at_home(&state);
        emit(
            &mut state,
            &mut scripted(&["promissory:sol:ragh:UNITS_COMMITTED:after", &destination]),
            "UNITS_COMMITTED",
            &[
                ("player", "b"),
                ("system", their_system.as_str()),
                ("planet", their_planet.as_str()),
            ],
        );
        assert!(
            state
                .system_state(&their_system)
                .on_planet_of(&their_planet, &a())
                .is_empty()
        );
        assert_eq!(at_home(&state), home_before + 2);
        assert_ne!(
            state.promissory_notes.get(RAGH_NOTE),
            Some(&b()),
            "returned"
        );
    }

    #[test]
    fn ragh_s_call_is_not_offered_without_the_note_or_without_saar_forces_there() {
        let mut state = game();
        let (their_system, their_planet) = home_planet(&state, &b());
        crate::fixtures::put_on_planet(
            &mut state,
            &their_system,
            &their_planet,
            "infantry",
            &a(),
            1,
        );
        let pairs = [
            ("player", "b"),
            ("system", their_system.as_str()),
            ("planet", their_planet.as_str()),
        ];
        // No note.
        let before = state.board.clone();
        emit(&mut state, &mut scripted(&[]), "UNITS_COMMITTED", &pairs);
        assert_eq!(state.board, before);
        // Note held, but no Saar ground force on the planet.
        let mut state = game();
        state.promissory_notes.insert(RAGH_NOTE.to_owned(), b());
        let before = state.board.clone();
        emit(&mut state, &mut scripted(&[]), "UNITS_COMMITTED", &pairs);
        assert_eq!(state.board, before);
    }

    #[test]
    fn the_hero_destroys_other_players_infantry_and_fighters_next_to_a_dock_only() {
        let mut state = game();
        let hub = crate::fixtures::plain_hub();
        let dock = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.centre.clone());
        let far = SystemId::new(hub.across(&hub.outer[0]));
        state
            .system_mut(&dock)
            .units
            .push(Unit::new(UnitTypeId::new("saar_spacedock"), a()));
        let planet = PlanetId::new("hero_test_planet");
        for system in [&target, &far] {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new("fighter"), b()));
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new("infantry"), b()));
            crate::fixtures::put_on_planet(&mut state, system, &planet, "infantry", &b(), 1);
            crate::fixtures::put_on_planet(&mut state, system, &planet, "infantry", &a(), 1);
        }
        let mut table = scripted(&[target.as_str()]);
        let done = crate::fixtures::with_context(
            &mut state,
            DEFAULT,
            Some(&hub.galaxy),
            &mut table,
            |context| use_leader(context, &a(), &LeaderId::new("saarhero")),
        );
        assert_eq!(done, Some(true));
        let board = state.system_state(&target);
        assert!(
            board.units.iter().all(|u| u.type_id.as_str() != "fighter"),
            "the fighter is destroyed"
        );
        assert_eq!(
            board.units.len(),
            3,
            "the carriers and destroyer are not fighters"
        );
        assert!(board.on_planet_of(&planet, &b()).is_empty());
        assert_eq!(
            board.on_planet_of(&planet, &a()).len(),
            1,
            "Saar's own stay"
        );
        let board = state.system_state(&far);
        assert_eq!(
            board.units.len(),
            2,
            "a system not adjacent to a dock is untouched"
        );
        assert_eq!(board.on_planet_of(&planet, &b()).len(), 1);
    }

    #[test]
    fn chaos_mapping_production_needs_the_technology_the_action_phase_and_a_producer() {
        let content = ContentStore::embedded();
        let mut state = game();
        state.phase = ti4_model::state::Phase::Action;
        assert!(!has_technology(&state, &a(), "cm") || chaos_ready(&state, content, DEFAULT, &a()));
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .retain(|t| t.as_str() != "cm");
        assert!(
            !chaos_ready(&state, content, DEFAULT, &a()),
            "no technology"
        );
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("cm"));
        assert!(
            !production_systems(&state, content, DEFAULT, &a()).is_empty(),
            "the home dock can produce"
        );
        assert!(chaos_ready(&state, content, DEFAULT, &a()));
        assert!(!chaos_ready(&state, content, DEFAULT, &b()), "b has no cm");
        state.phase = ti4_model::state::Phase::Status;
        assert!(
            !chaos_ready(&state, content, DEFAULT, &a()),
            "not the action phase"
        );
        // Declined through the window: nothing is produced.
        state.phase = ti4_model::state::Phase::Action;
        let before = state.board.clone();
        emit(
            &mut state,
            &mut scripted(&["decline"]),
            "TURN_BEGAN",
            &[("player", "a")],
        );
        assert_eq!(state.board, before);
    }
}
