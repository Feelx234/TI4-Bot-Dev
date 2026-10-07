//! Titans of Ul leaders (Tellurian, Tungstantus, Ul the Progenitor), the Terraform promissory note
//! and the Slumberstate Computing breakthrough. Faction abilities and units are in `titans.rs`.
//!
//! Card text (content corpus, latest printing):
//!
//! * Tellurian, agent: "Before a hit would be assigned: You may exhaust this card to cancel that
//!   hit."
//! * Tungstantus, commander: unlock "Have 5 structures on the game board."; the effect ("When 1 or
//!   more of your units use PRODUCTION: You may gain 1 trade good.") is the shared
//!   `titanscommander` window in `borrowed_commanders.rs`, gated on
//!   `promissory::has_commander_ability`, so it also works through an Alliance.
//! * Ul the Progenitor, hero: "ACTION: Ready Elysium and attach this card to it. Its resource and
//!   influence values are each increased by 3, and it gains the SPACE CANNON 5(x3) ability as if
//!   it were a unit." Unlock: "Have 3 scored objectives." (generic, `leaders::check_unlocks`).
//! * Terraform, promissory note: "ACTION: Attach this card to a non-home planet you control other
//!   than Mecatol Rex. Its resource and influence values are each increased by 1, and it is
//!   treated as having all 3 planet traits (cultural, hazardous, and industrial)."
//!
//! Both attachments live in `GameState::planet_attachments` under their `attachments.json` ids
//! (`titanshero` = Geoform, `titanspn` = Terraform), so the existing value readers
//! (`production::attachment_bonus`) apply the +3/+3 and +1/+1, `planets::traits_now` reads the
//! traits and `planets::attachment_cannons` the Geoform SPACE CANNON.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};

use super::Hooks;
use crate::choice::ChoiceOption;
use crate::timing::{Ability, Relation, TimingContext};

/// Leaders claimed by the Titans module: the commander (Tungstantus), the agent (Tellurian, on the
/// space `HITS_TO_ASSIGN` / `SPACE_CANNON_HITS` windows and the ground, bombardment, Harrow and
/// anti-fighter-barrage hit windows) and the hero (Ul the Progenitor, Geoform: its SPACE CANNON
/// fires in space cannon offense and defense). Terraform is claimed under `PROMISSORY` and
/// Slumberstate Computing under `BREAKTHROUGHS`.
pub const LEADERS: &[&str] = &["titanscommander", "titansagent", "titanshero"];
/// Promissory notes claimed by the Titans module.
pub const PROMISSORY: &[&str] = &[TERRAFORM];
/// Breakthroughs claimed by the Titans module.
pub const BREAKTHROUGHS: &[&str] = &[BREAKTHROUGH];

const AGENT: &str = "titansagent";
const BREAKTHROUGH: &str = "titansbt";
const COMMANDER: &str = "titanscommander";
const HERO: &str = "titanshero";
const TERRAFORM: &str = "terraform";
/// Attachment ids in `attachments.json`.
const GEOFORM: &str = "titanshero";
const TERRAFORM_ATTACHMENT: &str = "titanspn";
const ELYSIUM: &str = "elysium";
/// Component option id prefix of Terraform; the planet follows.
const TERRAFORM_ACTION: &str = "faction|titans|terraform|";
/// Structures the commander wants on the board.
const COMMANDER_STRUCTURES: usize = 5;

/// Hooks contributed by the leaders file; `titans.rs` sets `timing_abilities` itself and chains
/// [`timing_abilities`] below. A module that overrides `component_actions` or
/// `perform_component` must chain [`component_actions`] / [`perform_component`] (Terraform).
pub const HOOKS: Hooks = Hooks {
    commander_unlocked: Some(commander_unlocked),
    leader_action: Some(leader_action),
    use_leader: Some(use_leader),
    component_actions: Some(component_actions),
    perform_component: Some(perform_component),
    economy: super::hooks_economy::EconomyHooks {
        action_card_draw_bonus: Some(slumberstate_draw_bonus),
        ..super::hooks_economy::EconomyHooks::NONE
    },
    ..Hooks::NONE
};

/// Timing abilities of the leaders, note and breakthrough, for one seat.
pub(crate) fn timing_abilities(
    _state: &GameState,
    owner_name: &str,
    seat: &PlayerId,
) -> Vec<Ability> {
    vec![
        tellurian(owner_name, seat, "HITS_TO_ASSIGN"),
        tellurian(owner_name, seat, "SPACE_CANNON_HITS"),
        tellurian(owner_name, seat, crate::invasion::GROUND_HITS_TO_ASSIGN),
        tellurian(owner_name, seat, "ANTI_FIGHTER_BARRAGE_HITS"),
        sleeper_allowance(owner_name, seat),
    ]
}

// -- Tellurian ---------------------------------------------------------------------------------

/// Whether `player` holds a readied Tellurian.
fn agent_ready(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.leaders.get(&LeaderId::new(AGENT)) == Some(&LeaderStatus::Readied))
}

/// Tellurian: "Before a hit would be assigned: You may exhaust this card to cancel that hit."
///
/// Offered on the two windows that precede the assignment of hits to the owner's ships:
/// `HITS_TO_ASSIGN` (a space combat round's hits, emitted once as the first hit of a batch is about
/// to land) and `SPACE_CANNON_HITS` (a space cannon roll's hits, absorbed straight after). The
/// effect exhausts the card and grants one cancellation to the owner's round pool, which
/// `combat` spends on the very next hit it assigns. Hits on ground forces (ground combat,
/// bombardment, space cannon defense) have no such window yet; see the evidence file.
fn tellurian(owner_name: &str, seat: &PlayerId, event: &'static str) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("leader:{owner_name}:{AGENT}:{event}:when"),
        seat.clone(),
        event,
        Relation::When,
        Arc::new(move |_, _, context| {
            if crate::leaders::exhaust(context.state, &owner, &LeaderId::new(AGENT)) {
                crate::combat::grant_hit_cancellation(context.state, &owner, 1);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && event.integer("hits").is_none_or(|hits| hits > 0)
            && agent_ready(context.state, &condition_owner)
    }))
}

// -- Slumberstate Computing --------------------------------------------------------------------

/// Breakthrough text: "When COALESCENCE results in a ground combat, if you commit no other units,
/// you may choose for your units to coexist instead. During the status phase, for each player you
/// are coexisting with, you and that player each draw 1 additional action card. Other players may
/// allow you to place a sleeper token on a planet they control."
///
/// The coexist choice is asked in `invasion.rs` (`add_forced_combats`); this module holds the other
/// two halves.
fn holds_slumberstate(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == super::titans::FACTION)
        && crate::breakthroughs::holds(state, player, BREAKTHROUGH)
}

/// Players sharing a coexisting planet with `player`: the controller and every coexister of each
/// planet in coexistence on which `player` is one of them.
fn coexisting_with(state: &GameState, player: &PlayerId) -> std::collections::BTreeSet<PlayerId> {
    let mut found = std::collections::BTreeSet::new();
    for (system, board) in &state.board {
        for planet in board.coexisting.keys() {
            let mut here = crate::coexistence::coexisters(state, system, planet);
            if let Some(holder) = board.planet_control.get(planet) {
                here.insert(holder.clone());
            }
            if here.contains(player) {
                found.extend(here.into_iter().filter(|other| other != player));
            }
        }
    }
    found
}

/// "During the status phase, for each player you are coexisting with, you and that player each
/// draw 1 additional action card." For the holder, one per partner; for a partner, one per
/// Slumberstate holder it coexists with.
fn slumberstate_draw_bonus(
    state: &GameState,
    _content: &ContentStore,
    player: &PlayerId,
    _requested: usize,
) -> usize {
    if state.phase != ti4_model::state::Phase::Status {
        return 0;
    }
    if holds_slumberstate(state, player) {
        return coexisting_with(state, player).len();
    }
    state
        .players
        .iter()
        .filter(|seat| holds_slumberstate(state, &seat.id))
        .filter(|seat| coexisting_with(state, &seat.id).contains(player))
        .count()
}

/// Mark prefix recording that the allowance was offered to this player this round (public).
const ALLOWANCE_ASKED: &str = "titans:allowance_asked:";

fn allowance_mark(owner: &PlayerId, round: u32) -> String {
    format!("{ALLOWANCE_ASKED}{owner}:{round}")
}

/// Planets other players control where the Titans could still put a sleeper token. Only planets an
/// Awaken could use: not demilitarized, not a space station (the predicates `awakenings` applies).
/// Empty when all five tokens are out or the box holds neither a PDS nor a mech.
fn allowance_planets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> Vec<(PlanetId, SystemId, PlayerId)> {
    if super::titans::sleeper_planets(state, owner).len() >= super::titans::SLEEPER_TOKENS
        || !super::titans::awakening_plastic_available(state, content, sources, owner)
    {
        return Vec::new();
    }
    state
        .board
        .iter()
        .flat_map(|(system, board)| {
            board
                .planet_control
                .iter()
                .map(move |(planet, holder)| (planet.clone(), system.clone(), holder.clone()))
        })
        .filter(|(planet, _, holder)| holder != owner && !super::titans::has_sleeper(state, planet))
        .filter(|(planet, _, _)| {
            !crate::laws::planet_is_demilitarized(state, planet)
                && !ti4_content::galaxy::is_space_station(content, planet.as_str(), sources)
        })
        .collect()
}

/// "Other players may allow you to place a sleeper token on a planet they control." The notes say
/// "at any time, without a transaction or explore necessary"; the engine offers it at the start of
/// the holder's turn: the Titans pick a planet another player controls, that player is asked to
/// allow it, and on a yes the token is placed.
fn sleeper_allowance(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("breakthrough:{owner_name}:{BREAKTHROUGH}_sleeper:TURN_BEGAN:after"),
        seat.clone(),
        "TURN_BEGAN",
        Relation::After,
        Arc::new(move |_, _, context| {
            let candidates =
                allowance_planets(context.state, context.content, context.sources, &owner);
            if candidates.is_empty() {
                return Ok(());
            }
            // Offered at most once per game round, whatever the answer.
            let mark = allowance_mark(&owner, context.state.round);
            if context.state.faction_marks.contains_key(&mark) {
                return Ok(());
            }
            context.state.faction_marks.insert(mark, String::new());
            let ask = |context: &mut TimingContext<'_>,
                       who: &PlayerId,
                       subtype: &str,
                       prompt: String,
                       options: Vec<ChoiceOption>| {
                let choice = crate::choice::Choice::new(who.clone(), prompt, options)
                    .contextualized(crate::decision_context::DecisionContext::new(
                        who.clone(),
                        crate::decision_context::DecisionSource::FactionAbility(
                            BREAKTHROUGH.to_owned(),
                        ),
                        subtype,
                        context.state.phase,
                        context.state.round,
                    ));
                context
                    .ask_seeing(&choice)
                    .map(|answer| answer.id)
                    .map_err(crate::timing::TimingError::IllegalChoice)
            };
            // Every question before the mutation.
            let mut options: Vec<ChoiceOption> = candidates
                .iter()
                .map(|(planet, _, _)| {
                    ChoiceOption::labelled(
                        planet.to_string(),
                        "sleeper",
                        format!("ask to place a sleeper token on {planet}"),
                    )
                })
                .collect();
            options.push(ChoiceOption::decline());
            let picked = ask(
                context,
                &owner,
                "slumberstate_planet",
                "Slumberstate Computing: sleeper token on a planet another player controls"
                    .to_owned(),
                options,
            )?;
            let Some((planet, system, holder)) = candidates
                .into_iter()
                .find(|(planet, _, _)| planet.as_str() == picked)
            else {
                return Ok(());
            };
            let allowed = ask(
                context,
                &holder,
                "slumberstate_allow",
                format!("allow the Titans to place a sleeper token on {planet}?"),
                vec![
                    ChoiceOption::labelled("allow".to_owned(), "sleeper", "allow".to_owned()),
                    ChoiceOption::decline(),
                ],
            )?;
            if allowed == "allow" {
                super::titans::place_sleeper(context.state, &owner, &system, &planet);
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && holds_slumberstate(context.state, &condition_owner)
            && !context
                .state
                .faction_marks
                .contains_key(&allowance_mark(&condition_owner, context.state.round))
            && !allowance_planets(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
            .is_empty()
    }))
}

// -- Tungstantus -------------------------------------------------------------------------------

/// Structures `player` has on the game board: in space (Floating Factory) or on planets.
fn structures_on_board(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> usize {
    let types = ti4_content::units::catalogue(content, sources);
    state
        .board
        .values()
        .flat_map(|board| {
            board
                .units
                .iter()
                .chain(board.planet_units.values().flatten())
        })
        .filter(|unit| &unit.owner == player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_structure)
        })
        .count()
}

fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == COMMANDER)
        .then(|| structures_on_board(state, content, sources, player) >= COMMANDER_STRUCTURES)
}

// -- Ul the Progenitor ---------------------------------------------------------------------------

/// The system holding the planet, when anyone controls it.
fn controlled_planet_system(state: &GameState, planet: &PlanetId) -> Option<SystemId> {
    state
        .board
        .iter()
        .find(|(_, board)| board.planet_control.contains_key(planet))
        .map(|(system, _)| system.clone())
}

/// Whether Elysium is in play (controlled by somebody) and not already wearing Geoform.
fn elysium_available(state: &GameState) -> bool {
    let planet = PlanetId::new(ELYSIUM);
    controlled_planet_system(state, &planet).is_some()
        && !state
            .planet_attachments
            .get(&planet)
            .is_some_and(|attached| attached.iter().any(|id| id == GEOFORM))
}

fn leader_action(
    state: &GameState,
    _content: &ContentStore,
    _player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == HERO).then(|| elysium_available(state))
}

/// Geoform: ready Elysium and attach the card. The shared leader code purges the hero afterwards;
/// the attachment itself stays on the planet.
fn use_leader(
    context: &mut TimingContext<'_>,
    _player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != HERO {
        return None;
    }
    if !elysium_available(context.state) {
        return Some(false);
    }
    let planet = PlanetId::new(ELYSIUM);
    context.state.exhausted_planets.remove(&planet);
    context
        .state
        .planet_attachments
        .entry(planet)
        .or_default()
        .push(GEOFORM.to_owned());
    Some(true)
}

// -- Terraform ---------------------------------------------------------------------------------

/// The Terraform note `player` holds but does not own (a player cannot play their own note).
fn held_terraform(state: &GameState, player: &PlayerId) -> Option<String> {
    let own = crate::promissory::faction_name(state, player);
    state
        .promissory_notes
        .iter()
        .find(|(note, holder)| {
            *holder == player
                && crate::promissory::alias_of(note) == TERRAFORM
                && crate::promissory::owner_of(note).is_some_and(|owner| owner != own)
        })
        .map(|(note, _)| note.clone())
}

/// Whether Terraform is already attached to a planet.
fn terraform_attached(state: &GameState) -> bool {
    state
        .planet_attachments
        .values()
        .any(|attached| attached.iter().any(|id| id == TERRAFORM_ATTACHMENT))
}

/// Whether the planet is anyone's home planet: seated homes, and any printed faction homeworld.
fn is_home_planet(state: &GameState, content: &ContentStore, planet: &PlanetId) -> bool {
    state
        .players
        .iter()
        .any(|seat| seat.home_planets.contains(planet))
        || ti4_content::galaxy::all_planets(content, ti4_model::content_types::FULL)
            .get(planet.as_str())
            .is_some_and(|record| record.homeworld_of().is_some())
}

/// The planets `player` may attach Terraform to: controlled, not a home planet, not Mecatol Rex.
fn terraform_targets(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> Vec<PlanetId> {
    if held_terraform(state, player).is_none() || terraform_attached(state) {
        return Vec::new();
    }
    state
        .controlled_planets(player)
        .into_iter()
        .map(|(_, planet)| planet.clone())
        .filter(|planet| {
            !crate::seating::is_mecatol_planet(planet.as_str())
                && !is_home_planet(state, content, planet)
        })
        .collect()
}

fn component_actions(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    terraform_targets(state, content, player)
        .into_iter()
        .map(|planet| {
            ChoiceOption::labelled(
                format!("{TERRAFORM_ACTION}{planet}"),
                crate::faction_abilities::ACTION_KIND,
                format!("Terraform: attach it to {planet} (+1 resource, +1 influence, all traits)"),
            )
        })
        .collect()
}

fn perform_component(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    option: &ChoiceOption,
) -> bool {
    let Some(planet) = option.id.strip_prefix(TERRAFORM_ACTION) else {
        return false;
    };
    let planet = PlanetId::new(planet);
    let Some(note) = held_terraform(context.state, player) else {
        return false;
    };
    if !terraform_targets(context.state, context.content, player).contains(&planet) {
        return false;
    }
    context
        .state
        .planet_attachments
        .entry(planet)
        .or_default()
        .push(TERRAFORM_ATTACHMENT.to_owned());
    // Attached: the card is in play, no longer a note in hand to trade.
    context.state.promissory_faceup.insert(note);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::Spend;
    use ti4_model::content_types::DEFAULT;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }
    fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", "titans"), ("b", "sol")], DEFAULT)
    }
    fn scripted(answers: &[&str]) -> crate::choice::Table {
        crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(
            answers.iter().map(|s| (*s).to_owned()),
        )))
    }
    /// A non-home planet put under `who`'s control; returns it and its system.
    fn take_planet(state: &mut GameState, who: &PlayerId, index: usize) -> (PlanetId, SystemId) {
        let content = ContentStore::embedded();
        let catalogue = ti4_content::galaxy::all_planets(content, DEFAULT);
        let planet = crate::fixtures::non_home_planets(400)
            .into_iter()
            .filter(|p| !crate::seating::is_mecatol_planet(p))
            .filter(|p| {
                catalogue
                    .get(p.as_str())
                    .and_then(ti4_content::Planet::system_id)
                    .is_some()
            })
            .nth(index)
            .map(PlanetId::new)
            .expect("a planet");
        let system = SystemId::new(catalogue[planet.as_str()].system_id().unwrap());
        state
            .system_mut(&system)
            .planet_control
            .insert(planet.clone(), who.clone());
        (planet, system)
    }

    // -- Tungstantus -----------------------------------------------------------------------

    #[test]
    fn the_commander_unlocks_on_five_structures() {
        let mut state = game();
        let content = ContentStore::embedded();
        let leader = LeaderId::new(COMMANDER);
        let (planet, system) = take_planet(&mut state, &a(), 0);
        let count = |state: &GameState| structures_on_board(state, content, DEFAULT, &a());
        let base = count(&state);
        for _ in 0..4 {
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &a(), 1);
        }
        assert_eq!(count(&state), base + 4);
        let want = COMMANDER_STRUCTURES.saturating_sub(base + 4);
        if want > 0 {
            assert_eq!(
                crate::leaders::commander_unlocked(&state, content, DEFAULT, None, &a(), &leader),
                Some(false),
                "{want} short"
            );
            for _ in 0..want {
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &a(), 1);
            }
        }
        assert_eq!(
            crate::leaders::commander_unlocked(&state, content, DEFAULT, None, &a(), &leader),
            Some(true)
        );
        crate::leaders::check_unlocks(&mut state, content, DEFAULT, None, &a());
        assert_eq!(
            state.player(&a()).unwrap().leaders.get(&leader),
            Some(&LeaderStatus::Unlocked),
            "the shared unlock sweep reaches it"
        );
    }

    /// The unlocked commander's printed effect, through the real PRODUCTION_USED window: the
    /// Titans owner gains a trade good; locked, nothing is offered.
    #[test]
    fn the_unlocked_commander_gains_a_trade_good_when_production_is_used() {
        let content = ContentStore::embedded();
        let gain = |unlocked: bool| {
            let mut state = game();
            if unlocked {
                state
                    .player_mut(&a())
                    .unwrap()
                    .leaders
                    .insert(LeaderId::new(COMMANDER), LeaderStatus::Unlocked);
            }
            let before = state.player(&a()).unwrap().trade_goods;
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut table = scripted(&["leader:titans:titanscommander:PRODUCTION_USED:when"]);
            crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
                let event = crate::event::EventSequence::new()
                    .next(
                        "PRODUCTION_USED",
                        [("player".to_owned(), "a".into())].into_iter().collect(),
                    )
                    .unwrap();
                resolver.emit_with_context(ctx, event, |_, _| {}).unwrap();
            });
            state.player(&a()).unwrap().trade_goods - before
        };
        let _ = content;
        assert_eq!(gain(true), 1);
        assert_eq!(gain(false), 0);
    }

    #[test]
    fn only_the_owners_structures_count() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (planet, system) = take_planet(&mut state, &b(), 1);
        for _ in 0..6 {
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
        }
        let base_a = structures_on_board(&state, content, DEFAULT, &a());
        assert!(base_a < COMMANDER_STRUCTURES, "the home starts below five");
        assert_eq!(
            crate::leaders::commander_unlocked(
                &state,
                content,
                DEFAULT,
                None,
                &a(),
                &LeaderId::new(COMMANDER)
            ),
            Some(false)
        );
    }

    // -- Ul the Progenitor ---------------------------------------------------------------------

    fn use_hero(state: &mut GameState) -> bool {
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(HERO), LeaderStatus::Unlocked);
        crate::fixtures::with_context(state, DEFAULT, None, &mut scripted(&[]), |ctx| {
            crate::leaders::use_leader(ctx, &a(), &LeaderId::new(HERO))
        })
    }

    #[test]
    fn the_hero_is_offered_as_an_action_only_unlocked() {
        let mut state = game();
        let content = ContentStore::embedded();
        let offered = |state: &GameState| {
            crate::leaders::component_actions(state, content, &a())
                .iter()
                .any(|o| o.id == "component|leader|titanshero")
        };
        assert!(!offered(&state), "locked");
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(HERO), LeaderStatus::Unlocked);
        assert!(offered(&state));
        state
            .system_mut(&SystemId::new("55"))
            .planet_control
            .remove(&PlanetId::new(ELYSIUM));
        assert!(!offered(&state), "no Elysium to attach to");
    }

    #[test]
    fn geoform_readies_elysium_and_adds_three_and_three() {
        let mut state = game();
        let content = ContentStore::embedded();
        let elysium = PlanetId::new(ELYSIUM);
        state.exhausted_planets.insert(elysium.clone());
        let before = (
            crate::production::planet_value_now(
                &state,
                content,
                DEFAULT,
                &elysium,
                Spend::Resources,
            ),
            crate::production::planet_value_now(
                &state,
                content,
                DEFAULT,
                &elysium,
                Spend::Influence,
            ),
        );
        assert!(use_hero(&mut state));
        assert!(!state.exhausted_planets.contains(&elysium), "readied");
        assert_eq!(
            crate::production::planet_value_now(
                &state,
                content,
                DEFAULT,
                &elysium,
                Spend::Resources
            ),
            before.0 + 3
        );
        assert_eq!(
            crate::production::planet_value_now(
                &state,
                content,
                DEFAULT,
                &elysium,
                Spend::Influence
            ),
            before.1 + 3
        );
        assert_eq!(
            state
                .player(&a())
                .unwrap()
                .leaders
                .get(&LeaderId::new(HERO)),
            Some(&LeaderStatus::Purged)
        );
        assert!(
            crate::leaders::component_actions(&state, content, &a())
                .iter()
                .all(|o| o.id != "component|leader|titanshero"),
            "purged"
        );
    }

    #[test]
    fn geoform_space_cannon_fires_at_ships_activating_the_system() {
        let mut state = game();
        let content = ContentStore::embedded();
        let system = SystemId::new("55");
        let planet = PlanetId::new(ELYSIUM);
        // b attacks Elysium's system with ships.
        crate::fixtures::put(&mut state, &system, "carrier", &b(), 2);
        let fire = |state: &mut GameState| {
            let mut dice = crate::dice::Dice::new();
            let mut rng = crate::rng::GameRng::new(1);
            crate::combat::space_cannon_offense(
                state,
                content,
                DEFAULT,
                &mut dice,
                &mut rng,
                &system,
                &b(),
                None,
            )
            .into_iter()
            .map(|(who, hits, rolls)| {
                (
                    who,
                    hits,
                    rolls.iter().map(|r| r.faces.len()).sum::<usize>(),
                )
            })
            .collect::<Vec<_>>()
        };
        let dice_before: usize = fire(&mut state).iter().map(|(_, _, n)| n).sum();
        assert!(use_hero(&mut state));
        let after = fire(&mut state);
        let dice_after: usize = after.iter().map(|(_, _, n)| n).sum();
        assert_eq!(
            dice_after,
            dice_before + 3,
            "SPACE CANNON 5 (x3), a new gun of three dice"
        );
        assert!(
            crate::planets::attachment_cannons(&state, content, &system, &planet)
                == vec![(a(), 5, 3)]
        );
        // A planet nobody controls fires nothing.
        state.system_mut(&system).planet_control.remove(&planet);
        assert!(crate::planets::attachment_cannons(&state, content, &system, &planet).is_empty());
    }

    // -- Terraform ---------------------------------------------------------------------------

    fn give_terraform(state: &mut GameState, to: &PlayerId) -> String {
        let note = crate::promissory::note_id(TERRAFORM, "titans");
        crate::promissory::take(state, ContentStore::embedded(), to, &note);
        state.promissory_faceup.remove(&note); // still in hand, as handed over
        note
    }

    fn perform(state: &mut GameState, who: &PlayerId, planet: &PlanetId) -> bool {
        let id = format!("{TERRAFORM_ACTION}{planet}");
        let option = component_actions(state, ContentStore::embedded(), who)
            .into_iter()
            .find(|o| o.id == id);
        let Some(option) = option else {
            return false;
        };
        crate::fixtures::with_context(state, DEFAULT, None, &mut scripted(&[]), |ctx| {
            perform_component(ctx, who, &option)
        })
    }

    #[test]
    fn terraform_attaches_to_a_controlled_non_home_planet() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (planet, _) = take_planet(&mut state, &b(), 0);
        assert!(
            component_actions(&state, content, &b()).is_empty(),
            "not held"
        );
        let note = give_terraform(&mut state, &b());
        let offered = component_actions(&state, content, &b());
        assert!(offered.iter().any(|o| o.id.ends_with(planet.as_str())));
        let value = |state: &GameState, kind| {
            crate::production::planet_value_now(state, content, DEFAULT, &planet, kind)
        };
        let before = (
            value(&state, Spend::Resources),
            value(&state, Spend::Influence),
        );
        let traits_before = crate::planets::traits_now(&state, content, DEFAULT, &planet);
        assert!(perform(&mut state, &b(), &planet));
        assert_eq!(value(&state, Spend::Resources), before.0 + 1);
        assert_eq!(value(&state, Spend::Influence), before.1 + 1);
        let traits = crate::planets::traits_now(&state, content, DEFAULT, &planet);
        for kind in ["CULTURAL", "HAZARDOUS", "INDUSTRIAL"] {
            assert!(traits.iter().any(|t| t == kind), "{kind} in {traits:?}");
        }
        assert!(traits.len() >= traits_before.len());
        assert!(state.promissory_faceup.contains(&note), "in play");
        assert_eq!(
            state.promissory_notes.get(&note),
            Some(&b()),
            "stays with the holder"
        );
        assert!(
            component_actions(&state, content, &b()).is_empty(),
            "one attachment per card"
        );
    }

    /// The real exploration route: `choose_deck` offers the Terraformed planet every deck.
    #[test]
    fn a_terraformed_planet_explores_into_any_deck() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (planet, _) = (0..40)
            .map(|i| take_planet(&mut state, &b(), i))
            .find(|(p, _)| crate::exploration::traits_of(content, DEFAULT, p).len() == 1)
            .expect("a one-trait planet");
        let printed = crate::exploration::traits_of(content, DEFAULT, &planet)[0].clone();
        let other = ["CULTURAL", "HAZARDOUS", "INDUSTRIAL"]
            .into_iter()
            .find(|kind| *kind != printed)
            .unwrap();
        let ask = |state: &GameState| {
            crate::fixtures::with_context(
                &mut state.clone(),
                DEFAULT,
                None,
                &mut scripted(&[other]),
                |ctx| {
                    let (content, sources) = (ctx.content, ctx.sources);
                    let mut resolving = crate::choice::Resolving {
                        content,
                        sources,
                        dice: ctx.dice,
                        rng: ctx.rng,
                        table: ctx.table,
                        timing: None,
                    };
                    crate::exploration::choose_deck(&mut resolving, state, &b(), &planet)
                },
            )
        };
        assert_eq!(
            ask(&state).as_deref(),
            Some(printed.as_str()),
            "printed only"
        );
        give_terraform(&mut state, &b());
        assert!(perform(&mut state, &b(), &planet));
        assert_eq!(
            ask(&state).as_deref(),
            Some(other),
            "all three decks offered"
        );
    }

    #[test]
    fn terraform_refuses_home_planets_mecatol_and_the_owner() {
        let mut state = game();
        let content = ContentStore::embedded();
        give_terraform(&mut state, &b());
        // Put Mecatol Rex and b's own home planet under b's control: neither is offered.
        let mecatol = crate::seating::mecatol_for(content, DEFAULT);
        let mr = if mecatol == crate::seating::MECATOL_TE {
            "mrte"
        } else {
            "mr"
        };
        state
            .system_mut(&SystemId::new(mecatol))
            .planet_control
            .insert(PlanetId::new(mr), b());
        let home = state.player(&b()).unwrap().home_planets.clone();
        let home_system = state.player(&b()).unwrap().home_system.clone().unwrap();
        assert!(!home.is_empty());
        assert!(
            component_actions(&state, content, &b()).is_empty(),
            "only Mecatol and a home planet: nothing legal"
        );
        let _ = home_system;
        // The note's owner cannot play it.
        let (_, _) = take_planet(&mut state, &a(), 2);
        state
            .promissory_notes
            .insert(crate::promissory::note_id(TERRAFORM, "titans"), a());
        assert!(component_actions(&state, content, &a()).is_empty());
    }

    #[test]
    fn terraform_cannot_be_offered_twice_and_a_wrong_option_is_refused() {
        let mut state = game();
        let content = ContentStore::embedded();
        let (planet, _) = take_planet(&mut state, &b(), 0);
        give_terraform(&mut state, &b());
        let bogus = ChoiceOption::labelled(
            format!("{TERRAFORM_ACTION}elysium"),
            crate::faction_abilities::ACTION_KIND,
            "x",
        );
        let before = state.clone();
        let done =
            crate::fixtures::with_context(&mut state, DEFAULT, None, &mut scripted(&[]), |ctx| {
                perform_component(ctx, &b(), &bogus)
            });
        assert!(!done, "Elysium is a home planet and not b's");
        assert_eq!(state.planet_attachments, before.planet_attachments);
        assert!(perform(&mut state, &b(), &planet));
        assert!(!perform(&mut state, &b(), &planet));
        assert_eq!(
            state.planet_attachments.get(&planet).map(Vec::len),
            Some(1),
            "attached once"
        );
        let _ = content;
    }

    // -- Tellurian -----------------------------------------------------------------------------

    fn tellurian_offered(state: &GameState, event_type: &str, victim: &PlayerId) -> bool {
        let mut state = state.clone();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let window = format!("leader:titans:{AGENT}:{event_type}:when");
        let mut table = scripted(&[window.as_str()]);
        let mut seen = false;
        crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            let event = crate::event::EventSequence::new()
                .next(
                    event_type,
                    [
                        ("player".to_owned(), victim.to_string().into()),
                        ("hits".to_owned(), 2.into()),
                    ]
                    .into_iter()
                    .collect(),
                )
                .unwrap();
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("emits");
            seen = crate::combat::cancellable_hits(ctx.state, &a()) == 1;
        });
        seen
    }

    #[test]
    fn tellurian_cancels_a_hit_before_it_is_assigned() {
        let state = game();
        for event in ["HITS_TO_ASSIGN", "SPACE_CANNON_HITS"] {
            assert!(
                tellurian_offered(&state, event, &a()),
                "{event}: used by its owner"
            );
            assert!(
                !tellurian_offered(&state, event, &b()),
                "{event}: not for another player's hits"
            );
        }
        let mut exhausted = state.clone();
        exhausted
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(AGENT), LeaderStatus::Exhausted);
        assert!(
            !tellurian_offered(&exhausted, "HITS_TO_ASSIGN", &a()),
            "an exhausted agent cancels nothing"
        );
    }

    #[test]
    fn tellurian_grants_one_cancellation_when_used() {
        let mut state = game();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = scripted(&["leader:titans:titansagent:SPACE_CANNON_HITS:when"]);
        crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
            let event = crate::event::EventSequence::new()
                .next(
                    "SPACE_CANNON_HITS",
                    [
                        ("player".to_owned(), "a".into()),
                        ("hits".to_owned(), 2.into()),
                    ]
                    .into_iter()
                    .collect(),
                )
                .unwrap();
            resolver.emit_with_context(ctx, event, |_, _| {}).unwrap();
        });
        assert_eq!(crate::combat::cancellable_hits(&state, &a()), 1);
        assert_eq!(
            state
                .player(&a())
                .unwrap()
                .leaders
                .get(&LeaderId::new(AGENT)),
            Some(&LeaderStatus::Exhausted)
        );
    }

    // -- Neutrality ----------------------------------------------------------------------------

    #[test]
    fn games_without_the_titans_see_nothing_of_it() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT);
        let content = ContentStore::embedded();
        for who in [a(), b()] {
            assert!(component_actions(&state, content, &who).is_empty());
            assert!(!agent_ready(&state, &who));
            assert!(
                crate::planets::attachment_cannons(
                    &state,
                    content,
                    &SystemId::new("18"),
                    &PlanetId::new("mr")
                )
                .is_empty()
            );
        }
        assert_eq!(
            leader_action(&state, content, &a(), &LeaderId::new("solhero")),
            None
        );
        let cannon_before = {
            let mut dice = crate::dice::Dice::new();
            let mut rng = crate::rng::GameRng::new(1);
            crate::combat::space_cannon_offense(
                &mut state,
                content,
                DEFAULT,
                &mut dice,
                &mut rng,
                &SystemId::new("18"),
                &a(),
                None,
            )
        };
        assert!(cannon_before.is_empty());
    }
}
