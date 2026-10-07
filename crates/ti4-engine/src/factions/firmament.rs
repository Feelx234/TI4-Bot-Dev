//! The Firmament (`firmament`, alias in `factions.json`), part A of the two-sided Firmament /
//! Obsidian faction. See `plans/evidence/BF-firmament.md`.
//!
//! This file holds the plot-card model, Plots Within Plots (scoring), the technologies, units,
//! leaders, Black Ops and The Sowing; `firmament_flip.rs` holds Puppets of the Blade and the typed
//! transition to The Obsidian. The Obsidian's own cards are part B.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Plots Within Plots: "You can score secret objectives already scored by other players if you
//!   fulfill their requirements; this does not count against your secret objective limit or the
//!   number you can score in a round. When you score another player's secret objective: do not gain
//!   a victory point; instead, place a facedown plot card into your play area with that player's
//!   control token on it."
//! * Puppets of the Blade: "If you have at least 1 plot card in your play area, gain the following
//!   ability: ACTION: Purge The Firmament's faction sheet, leaders, planet cards, and promissory
//!   note. Then, gain all of the faction components for The Obsidian."
//! * Planesplitter (Firmament): "When you gain this card, put The Fracture into play. Flip this card
//!   if the Obsidian faction is in play."
//! * Neural Parasite (Firmament): "At the start of the status phase, you may place 1 infantry from
//!   your reinforcements on a planet you control in your home system. Flip this card if the
//!   Obsidian faction is in play."
//! * Heaven's Eye (flagship): "If the active system contains units that belong to a player who has a
//!   control marker on 1 of your plots, apply +1 to this ship's move value and repair it at the end
//!   of every combat round."
//! * Viper EX-23 (mech): "When ground forces are committed to this planet, you may choose for your
//!   units to coexist, if they were not already. Flip this card if your faction becomes the
//!   Obsidian."
//! * Myru Vos (agent): "When a player moves ships: You may exhaust this card; if you do, SPACE
//!   CANNON cannot be used against those ships. If they are not transporting units, they can also
//!   move through other players' ships."
//! * Captain Aroz (commander): "You can treat planets in systems that contain your ships as if they
//!   were controlled by you for the purpose of scoring secret objectives." Unlock: "Have a plot card
//!   in play." (The effect is `borrowed_commanders_b::firmament_planets`, reached through
//!   `promissory::has_commander_ability`, so Alliance, Yin, Mahact Imperia and Nekro reach it.)
//! * Sharsiss (hero): "ACTION: Place 1 of your plot cards in play with any other player's control
//!   token on it. Then, you may place any player's control token on 1 of your in-play plot cards;
//!   one plot cannot have two of the same player's tokens. Then, purge this card."
//! * Black Ops (promissory note): "When you receive this card, if you are not the Firmament: The
//!   Firmament player may place 1 facedown plot card in their play area with your control token on
//!   it. Then, gain 2 command tokens, gain 2 trade goods, and purge this card."
//! * The Sowing (`firmamentbt`): "When you gain this card and at the start of the status phase, you
//!   may place up to 3 of your trade goods on this card. Flip this card if you become The Obsidian
//!   faction."
//!
//! # Plots
//!
//! A plot card is a [`Plot`] (`ti4_model::plots`): the control tokens on it and whether it is
//! faceup. They live in `Player::plots` (one stored string per card). While facedown only the owner
//! reads the tokens: `ti4_model::view` replaces other players' facedown cards with the hidden marker
//! (the count survives), so a bot or a viewer built on a view never sees them. The engine itself
//! reads the real state.
//!
//! # Scoring a plot
//!
//! The scoring window ([`crate::objectives::ScoringWindow`]) asks the Firmament, besides its own
//! options, one `plot|<secret>|<player>` option per secret objective another player has scored that
//! the Firmament meets the requirement of at that timing ([`plot_options`]). Choosing it pays the
//! secret's price, records the score and the plot ([`score_as_plot`]) and gives no victory point.
//! Neither per-window cap applies, and the secret does not count against the hand limit
//! (`secrets::scored_count`).

use std::collections::BTreeSet;
use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_content::units::UnitType;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SecretObjectiveId, SystemId, UnitTypeId};
use ti4_model::plots::Plot;
use ti4_model::state::{FeatOccurrence, GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::hooks_movement::MovementHooks;
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::secrets::Timing;
use crate::timing::{Ability, Relation, TimingContext, TimingError};

/// The faction alias; also the faction name in promissory note ids.
pub const FACTION: &str = "firmament";
/// The faction the Firmament becomes.
pub const OBSIDIAN: &str = "obsidian";

/// Plots Within Plots.
pub const PLOTS: &str = "plotsplots";
/// Puppets of the Blade.
pub const PUPPETS: &str = "puppetsoftheblade";
/// Planesplitter (Firmament side).
pub const PLANESPLITTER: &str = "planesplitter-firm";
/// Neural Parasite (Firmament side).
pub const PARASITE: &str = "parasite-firm";
/// Heaven's Eye.
pub const FLAGSHIP: &str = "firmament_flagship";
/// Viper EX-23.
pub const MECH: &str = "firmament_mech";
/// Myru Vos.
pub const AGENT: &str = "firmamentagent";
/// Captain Aroz.
pub const COMMANDER: &str = "firmamentcommander";
/// Sharsiss.
pub const HERO: &str = "firmamenthero";
/// Black Ops.
pub const NOTE: &str = "blackops";
/// The Sowing.
pub const BREAKTHROUGH: &str = "firmamentbt";

/// The prefix of a scoring option that scores another player's secret as a plot.
const PLOT_OPTION: &str = "plot|";
/// `firmament:sowing:<player>`: trade goods on The Sowing (read by The Reaping, part B).
const SOWING_PREFIX: &str = "firmament:sowing:";
/// `firmament:agent` = `<activation_seq>|<mover>`: Myru Vos was used for this activation.
const AGENT_MARK: &str = "firmament:agent";

/// What this faction implements.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    abilities: &[PLOTS, PUPPETS],
    technologies: &[PLANESPLITTER, PARASITE],
    units: &[FLAGSHIP, MECH],
    promissory: &[NOTE],
    leaders: &[AGENT, COMMANDER, HERO],
    breakthroughs: &[BREAKTHROUGH],
    hooks: Hooks {
        timing_abilities: Some(timing_abilities),
        commander_unlocked: Some(commander_unlocked),
        mapped_component_actions: Some(super::firmament_flip::component_actions),
        perform_component: Some(super::firmament_flip::perform_component),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        movement: MovementHooks {
            move_bonus: Some(move_bonus),
            unladen_pass: Some(unladen_pass),
            ..MovementHooks::NONE
        },
        ..Hooks::NONE
    },
};

/// Whether `player` plays the Firmament (not yet the Obsidian).
#[must_use]
pub fn is_firmament(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn illegal(error: crate::choice::IllegalChoice) -> TimingError {
    TimingError::IllegalChoice(error)
}

/// The one place the faction asks. Every Firmament choice made inside a timing window, a component
/// action or a leader action goes through here.
pub(crate) fn ask(
    context: &mut TimingContext<'_>,
    who: &PlayerId,
    prompt: String,
    card: &str,
    subtype: &str,
    options: Vec<ChoiceOption>,
) -> Result<ChoiceOption, TimingError> {
    let choice = Choice::new(who.clone(), prompt, options).contextualized(DecisionContext::new(
        who.clone(),
        DecisionSource::FactionAbility(card.to_owned()),
        subtype,
        context.state.phase,
        context.state.round,
    ));
    context.ask_seeing(&choice).map_err(illegal)
}

// -- plots -----------------------------------------------------------------------------------------

/// `player`'s plot cards, in the order they were placed.
#[must_use]
pub fn plots(state: &GameState, player: &PlayerId) -> Vec<Plot> {
    state
        .player(player)
        .map(|seat| ti4_model::plots::read(&seat.plots))
        .unwrap_or_default()
}

/// Whether `player` has at least one plot card in play (either face).
#[must_use]
pub fn has_plot(state: &GameState, player: &PlayerId) -> bool {
    !plots(state, player).is_empty()
}

fn write_plots(state: &mut GameState, player: &PlayerId, plots: &[Plot]) {
    if let Some(seat) = state.player_mut(player) {
        seat.plots = plots.iter().map(Plot::encode).collect();
    }
}

/// Put a new plot card with `token`'s control token on it into `owner`'s play area: facedown for
/// the Firmament, faceup for the Obsidian.
pub fn place_plot(state: &mut GameState, owner: &PlayerId, token: &PlayerId) {
    let mut cards = plots(state, owner);
    let mut card = Plot::facedown(token);
    card.faceup = !is_firmament(state, owner);
    cards.push(card);
    write_plots(state, owner, &cards);
}

/// Put `token`'s control token on plot card `index` of `owner`. `false`, changing nothing, when the
/// card does not exist or already carries that player's token ("one plot cannot have two of the
/// same player's tokens").
pub fn add_token(state: &mut GameState, owner: &PlayerId, index: usize, token: &PlayerId) -> bool {
    let mut cards = plots(state, owner);
    let Some(card) = cards.get_mut(index) else {
        return false;
    };
    if !card.tokens.insert(token.clone()) {
        return false;
    }
    write_plots(state, owner, &cards);
    true
}

/// Flip every plot card of `owner` faceup (becoming the Obsidian).
pub fn flip_plots(state: &mut GameState, owner: &PlayerId) {
    let mut cards = plots(state, owner);
    for card in &mut cards {
        card.faceup = true;
    }
    write_plots(state, owner, &cards);
}

/// The players whose control tokens are on any of `owner`'s plot cards (the puppeted players).
#[must_use]
pub fn puppeted(state: &GameState, owner: &PlayerId) -> BTreeSet<PlayerId> {
    plots(state, owner)
        .into_iter()
        .flat_map(|card| card.tokens)
        .collect()
}

/// Whether `system` holds a unit (in space or on a planet) of any player in `players`.
fn holds_units_of(state: &GameState, system: &SystemId, players: &BTreeSet<PlayerId>) -> bool {
    let board = state.system_state(system);
    board
        .units
        .iter()
        .chain(board.planet_units.values().flatten())
        .any(|unit| players.contains(&unit.owner))
}

// -- Plots Within Plots ----------------------------------------------------------------------------

/// The scoring option that scores `secret`, scored by `token`, as a plot.
#[must_use]
pub fn option_id(secret: &SecretObjectiveId, token: &PlayerId) -> String {
    format!("{PLOT_OPTION}{secret}|{token}")
}

/// Read a scoring option made by [`option_id`].
#[must_use]
pub fn parse_option(id: &str) -> Option<(SecretObjectiveId, PlayerId)> {
    let (secret, token) = id.strip_prefix(PLOT_OPTION)?.split_once('|')?;
    Some((SecretObjectiveId::new(secret), PlayerId::new(token)))
}

/// Secret objectives another player has scored that `player` (the Firmament) may score as a plot at
/// this timing, with the player whose token goes on the plot, in seating then alphabetical order.
///
/// A secret that has become a public objective (Classified Document Leaks, the Neuraloop) is not
/// "scored by another player" as a secret and is left to the public rules.
#[must_use]
pub fn plot_options(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    when: Timing,
    occurrence: Option<FeatOccurrence>,
    galaxy: Option<&Galaxy>,
) -> Vec<(SecretObjectiveId, PlayerId)> {
    if !is_firmament(state, player) {
        return Vec::new();
    }
    let mine = state.scored_by(player);
    let mut found = Vec::new();
    for other in state.seating_order.iter().filter(|seat| *seat != player) {
        for scored in state.scored_by(other) {
            if mine.contains(&scored) || state.revealed_objectives.contains(&scored) {
                continue;
            }
            let secret = SecretObjectiveId::new(scored.as_str());
            if content
                .get(ContentType::SecretObjectives, secret.as_str())
                .is_none()
                || !crate::secrets::fulfils(
                    state, content, sources, player, &secret, when, occurrence, galaxy,
                )
            {
                continue;
            }
            found.push((secret, other.clone()));
        }
    }
    found
}

/// Score `secret`, which `token` scored, as a plot: no victory point, a facedown plot card with
/// `token`'s control token on it. `false`, changing nothing, when it is not a legal plot score.
pub fn score_as_plot(
    state: &mut GameState,
    content: &ContentStore,
    player: &PlayerId,
    secret: &SecretObjectiveId,
    token: &PlayerId,
) -> bool {
    let id = ti4_model::id::ObjectiveId::new(secret.as_str());
    if !is_firmament(state, player)
        || player == token
        || !state.scored_by(token).contains(&id)
        || state.scored_by(player).contains(&id)
        || content
            .get(ContentType::SecretObjectives, secret.as_str())
            .is_none()
    {
        return false;
    }
    if !crate::secrets::award_plot(state, player, secret) {
        return false;
    }
    place_plot(state, player, token);
    true
}

// -- the units -------------------------------------------------------------------------------------

/// Whether `player` has a Viper EX-23 on `planet`.
fn has_viper_on(
    state: &GameState,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> bool {
    state
        .system_state(system)
        .on_planet(planet)
        .iter()
        .any(|unit| &unit.owner == player && unit.type_id.as_str() == MECH)
}

/// Whether the Viper's choice is open for `invader`, whose ground forces (a Viper among them) have
/// been committed to `planet`: another player controls it and the invader is not already coexisting
/// there. The caller adds the rival-ground-force test it already owns.
#[must_use]
pub fn viper_open(
    state: &GameState,
    invader: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> bool {
    has_viper_on(state, invader, system, planet)
        && !crate::coexistence::is_coexisting(state, system, planet, invader)
        && state
            .system_state(system)
            .planet_control
            .get(planet)
            .is_some_and(|holder| holder != invader)
}

/// Whether the Viper's choice is open for `defender`, whose Viper stands on `planet` when `invader`
/// commits ground forces there and whose units are not already coexisting. The defender decides.
#[must_use]
pub fn viper_defender_open(
    state: &GameState,
    defender: &PlayerId,
    invader: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> bool {
    defender != invader
        && has_viper_on(state, defender, system, planet)
        && !crate::coexistence::is_coexisting(state, system, planet, defender)
}

/// Heaven's Eye: +1 move value while the active system holds units of a player with a control
/// marker on one of the owner's plots.
fn move_bonus(state: &GameState, site: &super::hooks_movement::MoveSite<'_>) -> i32 {
    if !super::flagship_has_text(state, site.player, site.ship.id(), FLAGSHIP) {
        return 0;
    }
    let Some(active) = state.active_system.as_ref() else {
        return 0;
    };
    i32::from(holds_units_of(state, active, &puppeted(state, site.player)))
}

/// Heaven's Eye: repair it at the end of every combat round in a system that holds units of a
/// puppeted player. Mandatory, and only registered effects that have something to repair fire.
fn repairs(owner_name: &str, seat: &PlayerId, event: &'static str) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("unit:{owner_name}:{FLAGSHIP}:{event}:after"),
        seat.clone(),
        event,
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(system) = event.text("system").map(SystemId::new) else {
                return Ok(());
            };
            for unit in damaged_flagships(context.state, &owner, &system) {
                let mut repaired = unit.clone();
                repaired.sustained_damage = false;
                context
                    .state
                    .system_mut(&system)
                    .replace_unit(&unit, repaired);
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event
            .text("system")
            .map(SystemId::new)
            .is_some_and(|system| {
                !damaged_flagships(context.state, &condition_owner, &system).is_empty()
            })
    }))
}

/// `owner`'s damaged ships with the Heaven's Eye text in `system`'s space area, when the system
/// holds units of a player with a control marker on one of the owner's plots.
fn damaged_flagships(state: &GameState, owner: &PlayerId, system: &SystemId) -> Vec<Unit> {
    if !holds_units_of(state, system, &puppeted(state, owner)) {
        return Vec::new();
    }
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| {
            &unit.owner == owner
                && unit.sustained_damage
                && super::flagship_has_text(state, owner, unit.type_id.as_str(), FLAGSHIP)
        })
        .cloned()
        .collect()
}

// -- Myru Vos --------------------------------------------------------------------------------------

/// Whether `mover`'s activation is the one Myru Vos was used for.
fn agent_covers(state: &GameState, mover: &PlayerId) -> bool {
    state.faction_marks.get(AGENT_MARK).is_some_and(|mark| {
        mark.split_once('|').is_some_and(|(seq, who)| {
            seq.parse::<u32>().ok() == Some(state.activation_seq) && who == mover.as_str()
        })
    })
}

/// SPACE CANNON cannot be used against `active`'s ships during an activation Myru Vos covers.
#[must_use]
pub fn space_cannon_silenced(state: &GameState, active: &PlayerId) -> bool {
    agent_covers(state, active)
}

fn unladen_pass(state: &GameState, mover: &PlayerId) -> bool {
    agent_covers(state, mover)
}

/// Whether Myru Vos bars `player`'s ship from carrying units along `path`: the agent covers this
/// activation, no other rule lets the ship pass, and an intermediate system holds another player's
/// ships that do not allow the passage.
#[must_use]
pub fn unladen_route_forbids_cargo(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    ship: &Unit,
    path: &[String],
) -> bool {
    if !agent_covers(state, player) || path.len() < 3 {
        return false;
    }
    let Some(active) = path.last() else {
        return false;
    };
    let site = super::hooks_movement::PassSite {
        player,
        active: &SystemId::new(active.as_str()),
        ship_type: ship.type_id.as_str(),
    };
    if super::hooks_movement::may_move_through_ships(state, content, sources, &site) {
        return false;
    }
    let allowed = super::hooks_movement::passable_owners(state, player);
    let types = ti4_content::units::catalogue(content, sources);
    path[1..path.len() - 1].iter().any(|step| {
        state
            .board
            .get(&SystemId::new(step.as_str()))
            .is_some_and(|system| {
                system.units.iter().any(|unit| {
                    &unit.owner != player
                        && !allowed.contains(&unit.owner)
                        && types
                            .get(unit.type_id.as_str())
                            .is_some_and(UnitType::is_ship)
                })
            })
    })
}

/// Whether `player` has a ship outside `except`.
fn has_ship_outside(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    except: &str,
) -> bool {
    let types = ti4_content::units::catalogue(content, sources);
    state.board.iter().any(|(system, board)| {
        system.as_str() != except
            && board.units.iter().any(|unit| {
                &unit.owner == player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_ship)
            })
    })
}

fn agent_ready(state: &GameState, owner: &PlayerId) -> bool {
    state
        .player(owner)
        .is_some_and(|seat| seat.leaders.get(&LeaderId::new(AGENT)) == Some(&LeaderStatus::Readied))
}

fn agent_window(
    context: &TimingContext<'_>,
    owner: &PlayerId,
    mover: Option<&str>,
    system: Option<&str>,
) -> bool {
    let (Some(mover), Some(system)) = (mover, system) else {
        return false;
    };
    agent_ready(context.state, owner)
        && has_ship_outside(
            context.state,
            context.content,
            context.sources,
            &PlayerId::new(mover),
            system,
        )
}

/// Myru Vos: after a player activates a system (the moment before they move ships), the owner may
/// exhaust the agent for that activation.
fn myru_vos(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{AGENT}:SYSTEM_ACTIVATED:after"),
        seat.clone(),
        "SYSTEM_ACTIVATED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(mover) = event.text("player") else {
                return Ok(());
            };
            if !crate::leaders::exhaust(context.state, &owner, &LeaderId::new(AGENT)) {
                return Ok(());
            }
            let seq = context.state.activation_seq;
            context
                .state
                .faction_marks
                .insert(AGENT_MARK.to_owned(), format!("{seq}|{mover}"));
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        agent_window(
            context,
            &condition_owner,
            event.text("player"),
            event.text("system"),
        )
    }))
}

// -- technologies ----------------------------------------------------------------------------------

/// The planets of `owner`'s home system that `owner` controls.
fn home_planets_held(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> Vec<(SystemId, PlanetId)> {
    let Some(home) = state
        .player(owner)
        .and_then(|seat| seat.home_system.clone())
    else {
        return Vec::new();
    };
    let Some(tile) = ti4_content::galaxy::system(content, home.as_str(), sources) else {
        return Vec::new();
    };
    tile.planets()
        .into_iter()
        .map(PlanetId::new)
        .filter(|planet| state.system_state(&home).planet_control.get(planet) == Some(owner))
        .map(|planet| (home.clone(), planet))
        .collect()
}

fn infantry_left(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
) -> bool {
    crate::supply::remaining(state, content, sources, owner, &UnitTypeId::new("infantry")) > 0
}

/// Neural Parasite: at the start of the status phase the owner may place 1 infantry on a planet
/// they control in their home system.
fn parasite(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("technology:{owner_name}:{PARASITE}:STATUS_PHASE_BEGAN:after"),
        seat.clone(),
        "STATUS_PHASE_BEGAN",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            let held = home_planets_held(context.state, context.content, context.sources, &owner);
            let mut options: Vec<ChoiceOption> = held
                .iter()
                .map(|(_, planet)| {
                    ChoiceOption::labelled(
                        planet.to_string(),
                        "planet",
                        format!("place 1 infantry on {planet}"),
                    )
                })
                .collect();
            options.push(ChoiceOption::decline());
            let answer = ask(
                context,
                &owner,
                "Neural Parasite: place 1 infantry on a planet you control in your home system"
                    .to_owned(),
                PARASITE,
                "neural_parasite",
                options,
            )?;
            if let Some((system, planet)) =
                held.iter().find(|(_, planet)| planet.as_str() == answer.id)
            {
                crate::action_cards::place_units(
                    context,
                    &owner,
                    system,
                    Some(planet),
                    "infantry",
                    1,
                );
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        crate::technology::has_technology_text(context.state, &condition_owner, PARASITE)
            && infantry_left(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
            && !home_planets_held(
                context.state,
                context.content,
                context.sources,
                &condition_owner,
            )
            .is_empty()
    }))
}

/// Planesplitter: "When you gain this card, put The Fracture into play." Rule 11: its owner chooses
/// one specialty planet of each colour for the ingress tokens.
fn planesplitter(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("technology:{owner_name}:{PLANESPLITTER}:TECHNOLOGY_GAINED:after"),
        seat.clone(),
        "TECHNOLOGY_GAINED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            crate::fracture::enter_play_by_effect(
                context.state,
                context.content,
                context.sources,
                context.galaxy,
                context.table,
                &owner,
                PLANESPLITTER,
            )
            .map_err(illegal)?;
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && event.text("technology") == Some(PLANESPLITTER)
            && context.state.player(&condition_owner).is_some_and(|seat| {
                seat.technologies
                    .contains(&ti4_model::id::TechnologyId::new(PLANESPLITTER))
            })
            && !context.state.fracture_in_play
    }))
}

// -- The Sowing ------------------------------------------------------------------------------------

fn sowing_key(player: &PlayerId) -> String {
    format!("{SOWING_PREFIX}{player}")
}

/// Trade goods on The Sowing (or, once flipped, The Reaping).
#[must_use]
pub fn goods_on_card(state: &GameState, player: &PlayerId) -> i32 {
    state
        .faction_marks
        .get(&sowing_key(player))
        .and_then(|goods| goods.parse().ok())
        .unwrap_or(0)
}

/// "You may place up to 3 of your trade goods on this card."
fn sow(context: &mut TimingContext<'_>, owner: &PlayerId) -> Result<(), TimingError> {
    let held = context
        .state
        .player(owner)
        .map_or(0, |seat| seat.trade_goods);
    let most = held.min(3);
    if most <= 0 {
        return Ok(());
    }
    let mut options: Vec<ChoiceOption> = (1..=most)
        .map(|count| {
            ChoiceOption::labelled(
                count.to_string(),
                "trade_goods",
                format!("place {count} trade good(s) on The Sowing"),
            )
        })
        .collect();
    options.push(ChoiceOption::decline());
    let answer = ask(
        context,
        owner,
        "The Sowing: place up to 3 of your trade goods on this card".to_owned(),
        BREAKTHROUGH,
        "the_sowing",
        options,
    )?;
    let Some(count) = answer
        .id
        .parse::<i32>()
        .ok()
        .filter(|count| (1..=most).contains(count))
    else {
        return Ok(());
    };
    let total = goods_on_card(context.state, owner) + count;
    if let Some(seat) = context.state.player_mut(owner) {
        seat.trade_goods -= count;
    }
    context
        .state
        .faction_marks
        .insert(sowing_key(owner), total.to_string());
    Ok(())
}

fn sowing_holds(state: &GameState, owner: &PlayerId) -> bool {
    crate::breakthroughs::holds(state, owner, BREAKTHROUGH)
        && state.player(owner).is_some_and(|seat| seat.trade_goods > 0)
}

fn sowing_gained(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("breakthrough:{owner_name}:{BREAKTHROUGH}:BREAKTHROUGH_GAINED:after"),
        seat.clone(),
        "BREAKTHROUGH_GAINED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| sow(context, &owner)),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        event.text("player") == Some(condition_owner.as_str())
            && event.text("breakthrough") == Some(BREAKTHROUGH)
            && sowing_holds(context.state, &condition_owner)
    }))
}

fn sowing_status(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("breakthrough:{owner_name}:{BREAKTHROUGH}:STATUS_PHASE_BEGAN:after"),
        seat.clone(),
        "STATUS_PHASE_BEGAN",
        Relation::After,
        Arc::new(move |_event, _resolver, context| sow(context, &owner)),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        sowing_holds(context.state, &condition_owner)
    }))
}

// -- Black Ops -------------------------------------------------------------------------------------

/// Who holds the Firmament's Black Ops as a received card (not its owner).
fn black_ops_holder(state: &GameState, owner: &PlayerId) -> Option<PlayerId> {
    if !is_firmament(state, owner) {
        return None;
    }
    let note = crate::promissory::note_id(NOTE, FACTION);
    state
        .promissory_notes
        .get(&note)
        .filter(|holder| *holder != owner)
        .cloned()
}

/// Black Ops, when it has been received: the Firmament player may place a facedown plot card with
/// the holder's control token on it; the holder gains 2 command tokens and 2 trade goods; the card
/// is purged. Resolved at the transaction that moved it ("when you receive this card").
fn black_ops(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_owner) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("promissory:{owner_name}:{NOTE}:TRANSACTION_RESOLVED:after"),
        seat.clone(),
        "TRANSACTION_RESOLVED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| {
            let Some(holder) = black_ops_holder(context.state, &owner) else {
                return Ok(());
            };
            let before = context.state.clone();
            if let Err(error) = receive_black_ops(context, &owner, &holder) {
                *context.state = before;
                return Err(error);
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |_event, _, context| {
        black_ops_holder(context.state, &condition_owner).is_some()
    }))
}

fn receive_black_ops(
    context: &mut TimingContext<'_>,
    owner: &PlayerId,
    holder: &PlayerId,
) -> Result<(), TimingError> {
    let answer = ask(
        context,
        owner,
        format!("Black Ops: place a facedown plot card with {holder}'s control token on it"),
        NOTE,
        "black_ops_plot",
        vec![
            ChoiceOption::labelled(
                "place",
                "plot",
                format!("place a plot with {holder}'s token"),
            ),
            ChoiceOption::decline(),
        ],
    )?;
    if answer.id == "place" {
        place_plot(context.state, owner, holder);
    }
    crate::strategy_cards::gain_tokens(
        context.state,
        context.content,
        context.sources,
        context.galaxy,
        context.table,
        holder,
        2,
    )
    .map_err(illegal)?;
    crate::supply::gain_trade_goods_staged(context.state, holder, 2, NOTE);
    let note = crate::promissory::note_id(NOTE, FACTION);
    context.state.promissory_notes.remove(&note);
    context.state.promissory_faceup.remove(&note);
    Ok(())
}

// -- Captain Aroz ----------------------------------------------------------------------------------

fn commander_unlocked(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    _galaxy: Option<&Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == COMMANDER).then(|| has_plot(state, player))
}

// -- Sharsiss --------------------------------------------------------------------------------------

fn leader_action(
    state: &GameState,
    _content: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == HERO).then(|| {
        is_firmament(state, player) && state.seating_order.iter().any(|seat| seat != player)
    })
}

fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == HERO).then(|| {
        let before = context.state.clone();
        match hero(context, player) {
            Ok(done) => done,
            Err(_) => {
                *context.state = before;
                false
            }
        }
    })
}

/// The Blade Beckons - Knife in the Back. The first sentence reads "1 of your plot cards" as a plot
/// card from the owner's supply, placed in play with another player's control token on it (the
/// reading that gives the sentence somewhere to put it; see the evidence file).
fn hero(context: &mut TimingContext<'_>, owner: &PlayerId) -> Result<bool, TimingError> {
    let others: Vec<PlayerId> = context
        .state
        .seating_order
        .iter()
        .filter(|seat| *seat != owner)
        .cloned()
        .collect();
    let target = match others.as_slice() {
        [] => return Ok(false),
        [only] => only.clone(),
        _ => {
            let answer = ask(
                context,
                owner,
                "Sharsiss: place a plot card with another player's control token on it".to_owned(),
                HERO,
                "sharsiss_plot",
                others
                    .iter()
                    .map(|seat| {
                        ChoiceOption::labelled(
                            seat.to_string(),
                            "player",
                            format!("{seat}'s control token"),
                        )
                    })
                    .collect(),
            )?;
            let Some(chosen) = others.into_iter().find(|seat| seat.as_str() == answer.id) else {
                return Ok(false);
            };
            chosen
        }
    };
    place_plot(context.state, owner, &target);
    // "Then, you may place any player's control token on 1 of your in-play plot cards."
    let cards = plots(context.state, owner);
    let mut options = Vec::new();
    for seat in context.state.seating_order.clone() {
        for (index, card) in cards.iter().enumerate() {
            if !card.tokens.contains(&seat) {
                options.push(ChoiceOption::labelled(
                    format!("token|{seat}|{index}"),
                    "plot_token",
                    format!("put {seat}'s control token on plot card {}", index + 1),
                ));
            }
        }
    }
    if options.is_empty() {
        return Ok(true);
    }
    options.push(ChoiceOption::decline());
    let answer = ask(
        context,
        owner,
        "Sharsiss: you may place any player's control token on one of your plot cards".to_owned(),
        HERO,
        "sharsiss_token",
        options,
    )?;
    if let Some(rest) = answer.id.strip_prefix("token|")
        && let Some((seat, index)) = rest.rsplit_once('|')
        && let Ok(index) = index.parse::<usize>()
    {
        add_token(context.state, owner, index, &PlayerId::new(seat));
    }
    Ok(true)
}

// -- the module's timing abilities -----------------------------------------------------------------

pub(crate) fn timing_abilities(
    _state: &GameState,
    owner_name: &str,
    seat: &PlayerId,
) -> Vec<Ability> {
    vec![
        parasite(owner_name, seat),
        planesplitter(owner_name, seat),
        sowing_gained(owner_name, seat),
        sowing_status(owner_name, seat),
        repairs(owner_name, seat, "SPACE_COMBAT_ROUND_ENDED"),
        repairs(owner_name, seat, "GROUND_COMBAT_ROUND_ENDED"),
        myru_vos(owner_name, seat),
        black_ops(owner_name, seat),
    ]
}

#[cfg(test)]
pub(crate) mod testkit {
    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;

    pub(crate) fn a() -> PlayerId {
        PlayerId::new("a")
    }
    pub(crate) fn b() -> PlayerId {
        PlayerId::new("b")
    }
    pub(crate) fn c() -> PlayerId {
        PlayerId::new("c")
    }
    pub(crate) fn content() -> &'static ContentStore {
        ContentStore::embedded()
    }
    /// `a` the Firmament, `b` Sol, `c` Hacan.
    pub(crate) fn game() -> GameState {
        crate::fixtures::seated_game(&[("a", "firmament"), ("b", "sol"), ("c", "hacan")], DEFAULT)
    }
    /// A one-ring map: `centre` with `ring` (six systems) around it, tiles of the whole corpus.
    pub(crate) fn ring(centre: &str, ring: &[&str]) -> Galaxy {
        let mut ids = vec![centre];
        ids.extend_from_slice(ring);
        Galaxy::build(content(), &ids, DEFAULT, 1).expect("a valid map")
    }
    /// The ring system opposite `from`: two apart, with only the centre between.
    pub(crate) fn across(galaxy: &Galaxy, ring: &[&str], from: &str) -> String {
        let near = |id: &str| -> std::collections::BTreeSet<String> {
            galaxy
                .adjacent(id)
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        };
        let mine = near(from);
        ring.iter()
            .find(|other| {
                **other != from
                    && galaxy.distance(from, other) == Some(2)
                    && mine.intersection(&near(other)).count() == 1
            })
            .map(|id| (*id).to_owned())
            .expect("an opposite system")
    }
    /// Ordinary systems with planets, no wormhole, anomaly or home, for filling a ring.
    pub(crate) fn plain(count: usize) -> Vec<String> {
        let homes = ti4_content::galaxy::home_systems(content(), DEFAULT);
        ti4_content::galaxy::all_systems(content(), DEFAULT)
            .iter()
            .filter(|(id, system)| {
                !system.is_anomaly()
                    && !system.is_hyperlane()
                    && system.wormholes().is_empty()
                    && !system.planets().is_empty()
                    && !homes.contains(*id)
                    && **id != "18"
            })
            .map(|(id, _)| (*id).to_owned())
            .take(count)
            .collect()
    }
    pub(crate) fn home(state: &GameState, who: &PlayerId) -> SystemId {
        state.player(who).unwrap().home_system.clone().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use ti4_model::content_types::DEFAULT;
    use ti4_model::id::{ObjectiveId, TechnologyId};
    use ti4_model::state::Feat;

    use super::super::crimson::testkit::{emit, scripted};
    use super::testkit::*;
    use super::*;
    use crate::objectives::{EventScoreLimit, ScoringWindow};

    fn goods(state: &GameState, who: &PlayerId) -> i32 {
        state.player(who).unwrap().trade_goods
    }

    // -- Plots Within Plots ---------------------------------------------------------------------

    /// `a` (Firmament) has three space docks; `b` has scored `fwm` ("Have 3 space docks").
    fn docks_game() -> GameState {
        let mut state = game();
        crate::fixtures::put(&mut state, &SystemId::new("18"), "spacedock", &a(), 2);
        state.record_score(&b(), ObjectiveId::new("fwm"));
        state
    }

    fn status_window(state: &GameState) -> ScoringWindow {
        ScoringWindow::new(&state.initiative_order())
    }

    fn pick(window: &mut ScoringWindow, state: &mut GameState, id: &str) -> Option<ObjectiveId> {
        let choice = window
            .pending_choice(state, content(), DEFAULT)
            .expect("something to score");
        let option = choice
            .option(id)
            .unwrap_or_else(|| panic!("{id} in {:?}", choice.ids()))
            .clone();
        window
            .resolve(state, content(), DEFAULT, option)
            .expect("resolves")
    }

    #[test]
    fn plots_within_plots_scores_another_players_secret_as_a_plot_for_no_victory_point() {
        let mut state = docks_game();
        let mut window = status_window(&state);
        let choice = window
            .pending_choice(&state, content(), DEFAULT)
            .expect("the Firmament is asked");
        assert_eq!(choice.player, a());
        assert!(choice.option("plot|fwm|b").is_some(), "{:?}", choice.ids());
        let vp = state.player(&a()).unwrap().victory_points;
        let scored = pick(&mut window, &mut state, "plot|fwm|b");
        assert_eq!(scored, Some(ObjectiveId::new("fwm")));
        let seat = state.player(&a()).unwrap();
        assert_eq!(seat.victory_points, vp, "no victory point");
        assert_eq!(seat.plots, ["d:b"], "a facedown plot with b's token");
        assert!(
            seat.plot_objectives
                .contains(&SecretObjectiveId::new("fwm"))
        );
        assert!(state.scored_by(&a()).contains(&ObjectiveId::new("fwm")));
        assert!(
            window.pending_choice(&state, content(), DEFAULT).is_none(),
            "scored once"
        );
    }

    #[test]
    fn a_plot_is_offered_only_to_the_firmament_that_meets_the_requirement() {
        // Sol has three docks and the Firmament scored it: Sol has no Plots Within Plots.
        let mut sol = game();
        crate::fixtures::put(&mut sol, &SystemId::new("18"), "spacedock", &b(), 3);
        sol.record_score(&a(), ObjectiveId::new("fwm"));
        let window = status_window(&sol);
        assert!(window.pending_choice(&sol, content(), DEFAULT).is_none());
        // The Firmament with one dock does not meet it.
        let mut state = game();
        state.record_score(&b(), ObjectiveId::new("fwm"));
        assert!(
            status_window(&state)
                .pending_choice(&state, content(), DEFAULT)
                .is_none()
        );
        // Nor does it offer a secret the Firmament has already scored itself.
        let mut mine = docks_game();
        mine.record_score(&a(), ObjectiveId::new("fwm"));
        assert!(
            status_window(&mine)
                .pending_choice(&mine, content(), DEFAULT)
                .is_none()
        );
    }

    #[test]
    fn a_plot_does_not_use_the_one_secret_per_status_phase_or_the_secret_limit() {
        // The Firmament also holds `eap` (4 PDS): the plot and the ordinary score are both open.
        for plot_first in [true, false] {
            let mut state = docks_game();
            crate::fixtures::put(&mut state, &SystemId::new("18"), "pds", &a(), 4);
            state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("eap")];
            let mut window = status_window(&state);
            let (first, second) = if plot_first {
                ("plot|fwm|b", "eap")
            } else {
                ("eap", "plot|fwm|b")
            };
            pick(&mut window, &mut state, first);
            pick(&mut window, &mut state, second);
            assert!(window.pending_choice(&state, content(), DEFAULT).is_none());
            let seat = state.player(&a()).unwrap();
            assert_eq!(seat.victory_points, 1, "only the ordinary secret scores");
            // Two scored secrets, but the plot does not count against the limit of three.
            assert_eq!(crate::secrets::scored_count(&state, content(), &a()), 1);
            assert_eq!(crate::secrets::held_count(&state, content(), &a()), 1);
        }
    }

    #[test]
    fn a_plot_is_scored_in_an_event_window_without_using_the_per_combat_cap() {
        let mut state = game();
        state.record_score(&b(), ObjectiveId::new("dtgs"));
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new("btv")];
        let occurrence = state.begin_feat_occurrence();
        state.record_event_feat(&a(), Feat::DestroyedACapitalShip, occurrence);
        state.record_event_feat(&a(), Feat::WonInAnAnomaly, occurrence);
        let mut window = ScoringWindow::for_occurrence(
            &[a()],
            crate::secrets::Timing::Action,
            occurrence,
            EventScoreLimit::OnePerPlayer,
        );
        pick(&mut window, &mut state, "btv");
        // The ordinary cap is spent, the plot is still on offer.
        pick(&mut window, &mut state, "plot|dtgs|b");
        assert!(window.pending_choice(&state, content(), DEFAULT).is_none());
        assert_eq!(state.player(&a()).unwrap().plots, ["d:b"]);
        // Without the feat the plot is not on offer.
        let mut bare = game();
        bare.record_score(&b(), ObjectiveId::new("dtgs"));
        let occurrence = bare.begin_feat_occurrence();
        let window = ScoringWindow::for_occurrence(
            &[a()],
            crate::secrets::Timing::Action,
            occurrence,
            EventScoreLimit::OnePerPlayer,
        );
        assert!(window.pending_choice(&bare, content(), DEFAULT).is_none());
    }

    #[test]
    fn the_plots_tokens_are_hidden_from_other_players_until_the_flip() {
        let mut state = docks_game();
        let mut window = status_window(&state);
        pick(&mut window, &mut state, "plot|fwm|b");
        let for_b = ti4_model::view::view_for(&state, &b());
        assert_eq!(
            for_b.player(&a()).unwrap().plots,
            ["?"],
            "the count survives"
        );
        assert!(ti4_model::view::leaks(&for_b, &b()).is_empty());
        assert_eq!(
            ti4_model::view::view_for(&state, &a())
                .player(&a())
                .unwrap()
                .plots,
            ["d:b"]
        );
        flip_plots(&mut state, &a());
        let for_c = ti4_model::view::view_for(&state, &c());
        assert_eq!(for_c.player(&a()).unwrap().plots, ["u:b"]);
    }

    #[test]
    fn a_plot_card_never_carries_two_tokens_of_one_player() {
        let mut state = game();
        place_plot(&mut state, &a(), &b());
        assert!(!add_token(&mut state, &a(), 0, &b()));
        assert!(add_token(&mut state, &a(), 0, &c()));
        assert!(!add_token(&mut state, &a(), 1, &c()), "no such card");
        assert_eq!(state.player(&a()).unwrap().plots, ["d:b,c"]);
        assert_eq!(puppeted(&state, &a()), BTreeSet::from([b(), c()]));
    }

    // -- Captain Aroz ---------------------------------------------------------------------------

    #[test]
    fn captain_aroz_unlocks_with_a_plot_card_in_play() {
        let mut state = game();
        let leader = LeaderId::new(COMMANDER);
        crate::leaders::check_unlocks(&mut state, content(), DEFAULT, None, &a());
        assert_eq!(
            state.player(&a()).unwrap().leaders.get(&leader),
            Some(&LeaderStatus::Locked)
        );
        place_plot(&mut state, &a(), &b());
        crate::leaders::check_unlocks(&mut state, content(), DEFAULT, None, &a());
        assert_eq!(
            state.player(&a()).unwrap().leaders.get(&leader),
            Some(&LeaderStatus::Unlocked)
        );
    }

    // -- Neural Parasite --------------------------------------------------------------------------

    fn infantry_at(state: &GameState, who: &PlayerId, planet: &str) -> usize {
        let home = home(state, who);
        state
            .system_state(&home)
            .on_planet_of(&PlanetId::new(planet), who)
            .iter()
            .filter(|unit| unit.type_id.as_str().contains("infantry"))
            .count()
    }

    #[test]
    fn neural_parasite_places_an_infantry_on_a_home_planet_at_the_start_of_the_status_phase() {
        let mut state = game();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PARASITE));
        let before = infantry_at(&state, &a(), "tallin");
        emit(&mut state, None, &["tallin"], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(infantry_at(&state, &a(), "tallin"), before + 1);
        // It is a may.
        let mut declined = game();
        declined
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PARASITE));
        let before = infantry_at(&declined, &a(), "tallin");
        emit(&mut declined, None, &["decline"], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(infantry_at(&declined, &a(), "tallin"), before);
        // Only planets the owner controls count.
        let mut lost = game();
        lost.player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PARASITE));
        let home_system = home(&lost, &a());
        for planet in ["cronos", "tallin"] {
            lost.system_mut(&home_system)
                .set_control(PlanetId::new(planet), b());
        }
        let before = infantry_at(&lost, &a(), "tallin");
        emit(&mut lost, None, &[], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(
            infantry_at(&lost, &a(), "tallin"),
            before,
            "no planet of its own, no offer"
        );
    }

    #[test]
    fn a_nekro_with_the_parasite_text_places_on_its_own_home_planets() {
        let mut state = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "firmament")], &[]);
        // Valefar Assimilator X on the Firmament's Neural Parasite.
        let seat = state.player_mut(&b()).unwrap();
        seat.technologies.insert(TechnologyId::new(PARASITE));
        let nekro = a();
        let _ = nekro;
        assert!(crate::technology::has_technology_text(
            &state,
            &b(),
            PARASITE
        ));
        let before = state.system_state(&home(&state, &b())).planet_units.clone();
        emit(&mut state, None, &["tallin"], "STATUS_PHASE_BEGAN", &[]);
        assert_ne!(
            state.system_state(&home(&state, &b())).planet_units,
            before,
            "the holder (b) placed"
        );
    }

    // -- Planesplitter ----------------------------------------------------------------------------

    #[test]
    fn planesplitter_puts_the_fracture_into_play_when_it_is_gained() {
        let mut state = game();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(TechnologyId::new(PLANESPLITTER));
        assert!(!state.fracture_in_play);
        let gained = [("player", "a".into()), ("technology", PLANESPLITTER.into())];
        emit(&mut state, None, &[], "TECHNOLOGY_GAINED", &gained);
        assert!(state.fracture_in_play);
        assert!(!state.ingress_tokens.is_empty());
        // Once only, and for the card only.
        let again = state.clone();
        emit(&mut state, None, &[], "TECHNOLOGY_GAINED", &gained);
        assert_eq!(state.ingress_tokens, again.ingress_tokens);
        let mut other = game();
        emit(
            &mut other,
            None,
            &[],
            "TECHNOLOGY_GAINED",
            &[("player", "a".into()), ("technology", "ac2".into())],
        );
        assert!(!other.fracture_in_play);
        let mut foreign = game();
        emit(
            &mut foreign,
            None,
            &[],
            "TECHNOLOGY_GAINED",
            &[("player", "b".into()), ("technology", PLANESPLITTER.into())],
        );
        assert!(
            !foreign.fracture_in_play,
            "only its owner puts it into play"
        );
    }

    // -- The Sowing -------------------------------------------------------------------------------

    fn sower() -> GameState {
        let mut state = game();
        let seat = state.player_mut(&a()).unwrap();
        seat.breakthrough = Some(ti4_model::id::BreakthroughId::new(BREAKTHROUGH));
        seat.trade_goods = 5;
        state
    }

    #[test]
    fn the_sowing_takes_up_to_three_trade_goods_when_gained_and_at_the_start_of_the_status_phase() {
        let mut state = sower();
        let gained = [
            ("player", "a".into()),
            ("breakthrough", BREAKTHROUGH.into()),
        ];
        emit(&mut state, None, &["2"], "BREAKTHROUGH_GAINED", &gained);
        assert_eq!(goods(&state, &a()), 3);
        assert_eq!(goods_on_card(&state, &a()), 2);
        emit(&mut state, None, &["3"], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(goods(&state, &a()), 0);
        assert_eq!(goods_on_card(&state, &a()), 5);
        // Nothing to place, nothing asked.
        emit(&mut state, None, &[], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(goods_on_card(&state, &a()), 5);
        // Never more than three at once, and never more than are held.
        let mut some = sower();
        some.player_mut(&a()).unwrap().trade_goods = 2;
        emit(&mut some, None, &["2"], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(goods_on_card(&some, &a()), 2);
    }

    #[test]
    fn the_sowing_is_a_may_and_belongs_to_its_holder() {
        let mut state = sower();
        emit(&mut state, None, &["decline"], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!((goods(&state, &a()), goods_on_card(&state, &a())), (5, 0));
        let mut none = game();
        none.player_mut(&a()).unwrap().trade_goods = 5;
        emit(&mut none, None, &[], "STATUS_PHASE_BEGAN", &[]);
        assert_eq!(goods(&none, &a()), 5, "no breakthrough, no card");
    }

    // -- Black Ops --------------------------------------------------------------------------------

    fn received() -> GameState {
        let mut state = game();
        state
            .promissory_notes
            .insert(crate::promissory::note_id(NOTE, FACTION), b());
        state
    }

    fn resolved() -> Vec<(&'static str, serde_json::Value)> {
        vec![("proposer", "b".into()), ("partner", "a".into())]
    }

    fn tokens(state: &GameState, who: &PlayerId) -> i32 {
        let seat = state.player(who).unwrap();
        seat.tactic_tokens + seat.fleet_tokens + seat.strategic_tokens
    }

    #[test]
    fn black_ops_lets_the_firmament_place_a_plot_and_pays_its_holder() {
        let mut state = received();
        let (goods_before, tokens_before) = (goods(&state, &b()), tokens(&state, &b()));
        emit(
            &mut state,
            None,
            &["place"],
            "TRANSACTION_RESOLVED",
            &resolved(),
        );
        assert_eq!(state.player(&a()).unwrap().plots, ["d:b"]);
        assert_eq!(goods(&state, &b()), goods_before + 2);
        assert_eq!(tokens(&state, &b()), tokens_before + 2);
        assert!(
            !state.promissory_notes.contains_key("blackops:firmament"),
            "purged"
        );
    }

    #[test]
    fn black_ops_plot_is_a_may_but_the_gains_and_the_purge_are_not() {
        let mut state = received();
        let (goods_before, tokens_before) = (goods(&state, &b()), tokens(&state, &b()));
        emit(
            &mut state,
            None,
            &["decline"],
            "TRANSACTION_RESOLVED",
            &resolved(),
        );
        assert!(state.player(&a()).unwrap().plots.is_empty());
        assert_eq!(goods(&state, &b()), goods_before + 2);
        assert_eq!(tokens(&state, &b()), tokens_before + 2);
        assert!(!state.promissory_notes.contains_key("blackops:firmament"));
    }

    #[test]
    fn black_ops_does_nothing_while_its_owner_holds_it() {
        let mut state = game();
        state
            .promissory_notes
            .insert(crate::promissory::note_id(NOTE, FACTION), a());
        let before = state.clone();
        emit(&mut state, None, &[], "TRANSACTION_RESOLVED", &resolved());
        assert!(state.promissory_notes.contains_key("blackops:firmament"));
        assert_eq!(goods(&state, &b()), goods(&before, &b()));
        assert!(state.player(&a()).unwrap().plots.is_empty());
    }

    // -- Heaven's Eye -----------------------------------------------------------------------------

    /// The Firmament has a plot with `b`'s token and a flagship in its home system, which is the
    /// active system and holds a Sol unit.
    fn eye() -> (GameState, SystemId) {
        let mut state = game();
        place_plot(&mut state, &a(), &b());
        let system = home(&state, &a());
        crate::fixtures::put(&mut state, &system, "firmament_flagship", &a(), 1);
        crate::fixtures::put(&mut state, &system, "destroyer", &b(), 1);
        state.active_system = Some(system.clone());
        (state, system)
    }

    fn move_value(state: &GameState, system: &SystemId) -> i32 {
        let types = ti4_content::units::catalogue(content(), DEFAULT);
        let kind = types.get(FLAGSHIP).expect("the flagship");
        let index = state
            .ships_of(&a(), system)
            .iter()
            .position(|unit| unit.type_id.as_str() == FLAGSHIP);
        crate::tactical::effective_move_value_for_ship(
            state,
            kind,
            &a(),
            system,
            index,
            false,
            false,
        )
    }

    #[test]
    fn heavens_eye_gains_a_step_when_the_active_system_holds_a_puppeted_players_units() {
        let (mut state, system) = eye();
        assert_eq!(move_value(&state, &system), 2, "printed 1, +1");
        let bare = SystemId::new("18");
        state.active_system = Some(bare);
        assert_eq!(move_value(&state, &system), 1, "no puppeted units there");
        // A player without a token on a plot does not count.
        let (mut state, system) = eye();
        crate::fixtures::put(&mut state, &system, "destroyer", &c(), 1);
        state
            .system_mut(&system)
            .units
            .retain(|unit| unit.owner != b());
        assert_eq!(move_value(&state, &system), 1);
    }

    #[test]
    fn heavens_eye_repairs_itself_at_the_end_of_every_combat_round() {
        for event in ["SPACE_COMBAT_ROUND_ENDED", "GROUND_COMBAT_ROUND_ENDED"] {
            let (mut state, system) = eye();
            let flagship = state
                .system_state(&system)
                .units
                .iter()
                .find(|unit| unit.type_id.as_str() == FLAGSHIP)
                .cloned()
                .unwrap();
            state
                .system_mut(&system)
                .replace_unit(&flagship, flagship.sustained());
            let pairs = [("system", system.to_string().into())];
            emit(&mut state, None, &[], event, &pairs);
            assert!(
                state
                    .system_state(&system)
                    .units
                    .iter()
                    .all(|unit| !unit.sustained_damage),
                "{event}: repaired"
            );
            // No puppeted unit there: it stays damaged.
            let mut alone = state.clone();
            let flagship = alone
                .system_state(&system)
                .units
                .iter()
                .find(|unit| unit.type_id.as_str() == FLAGSHIP)
                .cloned()
                .unwrap();
            alone
                .system_mut(&system)
                .replace_unit(&flagship, flagship.sustained());
            alone
                .system_mut(&system)
                .units
                .retain(|unit| unit.owner != b());
            emit(&mut alone, None, &[], event, &pairs);
            assert!(
                alone
                    .system_state(&system)
                    .units
                    .iter()
                    .any(|unit| unit.sustained_damage),
                "{event}: no puppeted unit, no repair"
            );
        }
    }

    #[test]
    fn a_nekro_flagship_carrying_heavens_eye_text_is_only_inert_without_plots() {
        // The Z token lends the text; the +1 and the repair still need plots, which a Nekro has none of.
        let mut state = crate::fixtures::nekro_with_z(
            &[("a", "nekro"), ("b", "sol"), ("c", "firmament")],
            &["firmament"],
        );
        let system = home(&state, &a());
        crate::fixtures::put(&mut state, &system, "nekro_flagship", &a(), 1);
        crate::fixtures::put(&mut state, &system, "destroyer", &b(), 1);
        state.active_system = Some(system.clone());
        let types = ti4_content::units::catalogue(content(), DEFAULT);
        let kind = types.get("nekro_flagship").expect("the flagship");
        let printed = i32::try_from(kind.move_value()).unwrap();
        assert!(super::super::flagship_has_text(
            &state,
            &a(),
            "nekro_flagship",
            FLAGSHIP
        ));
        assert_eq!(
            crate::tactical::effective_move_value_for_ship(
                &state,
                kind,
                &a(),
                &system,
                Some(0),
                false,
                false
            ),
            printed
        );
    }

    // -- Viper EX-23 ------------------------------------------------------------------------------

    use super::super::deepwrought::testkit::{infantry_on, plain_planet, steer};

    fn invade(
        state: &mut GameState,
        who: &PlayerId,
        prefer: &[&str],
        system: &SystemId,
    ) -> (
        crate::invasion::InvasionReport,
        crate::dice::Dice,
        Vec<String>,
    ) {
        let (mut table, seen) = steer(prefer);
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(7);
        let report = crate::invasion::resolve(
            state,
            content(),
            DEFAULT,
            &mut table,
            &mut dice,
            &mut rng,
            system,
            who,
        )
        .expect("the invasion resolves");
        let prompts = seen.borrow().clone();
        (report, dice, prompts)
    }

    #[test]
    fn a_committed_viper_may_coexist_instead_of_fighting() {
        let mut state = game();
        let (system, planet) = plain_planet();
        state.system_mut(&system).set_control(planet.clone(), b());
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
        crate::fixtures::put(&mut state, &system, MECH, &a(), 1);
        let commit = format!("commit|0|{planet}");
        let (report, dice, prompts) = invade(
            &mut state,
            &a(),
            &[commit.as_str(), "coexist", "done_committing"],
            &system,
        );
        assert!(
            prompts.iter().any(|p| p.contains("Viper EX-23")),
            "{prompts:?}"
        );
        assert!(dice.rolled("ground combat").is_empty(), "nobody fought");
        assert!(crate::coexistence::is_coexisting(
            &state,
            &system,
            &planet,
            &a()
        ));
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b()),
            "the controller keeps the planet (coexistence 3.1)"
        );
        assert!(report.captured.is_empty());
        assert_eq!(infantry_on(&state, &system, &planet, &b()), 2);
    }

    #[test]
    fn the_viper_is_a_may_and_needs_the_mech_and_a_rival_force() {
        let setup = |mech: bool, rival: bool| {
            let mut state = game();
            let (system, planet) = plain_planet();
            state.system_mut(&system).set_control(planet.clone(), b());
            if rival {
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 2);
            }
            crate::fixtures::put(
                &mut state,
                &system,
                if mech { MECH } else { "infantry" },
                &a(),
                1,
            );
            (state, system, planet)
        };
        let (mut state, system, planet) = setup(true, true);
        let commit = format!("commit|0|{planet}");
        let (_, dice, _) = invade(
            &mut state,
            &a(),
            &[commit.as_str(), "fight", "done_committing"],
            &system,
        );
        assert!(!dice.rolled("ground combat").is_empty(), "declining fights");
        assert!(!crate::coexistence::is_coexisting(
            &state,
            &system,
            &planet,
            &a()
        ));
        for (mech, rival) in [(false, true), (true, false)] {
            let (mut state, system, planet) = setup(mech, rival);
            let commit = format!("commit|0|{planet}");
            let (_, _, prompts) = invade(
                &mut state,
                &a(),
                &[commit.as_str(), "coexist", "done_committing"],
                &system,
            );
            assert!(prompts.iter().all(|p| !p.contains("Viper")), "{prompts:?}");
        }
    }

    #[test]
    fn a_defending_viper_lets_its_owner_choose_to_coexist() {
        let mut state = game();
        let (system, planet) = plain_planet();
        state.system_mut(&system).set_control(planet.clone(), a());
        crate::fixtures::put_on_planet(&mut state, &system, &planet, MECH, &a(), 1);
        crate::fixtures::put(&mut state, &system, "infantry", &b(), 2);
        let commit = format!("commit|0|{planet}");
        let (report, dice, prompts) = invade(
            &mut state,
            &b(),
            &[commit.as_str(), "coexist", "done_committing"],
            &system,
        );
        assert!(
            prompts.iter().any(|p| p.contains("Viper EX-23")),
            "{prompts:?}"
        );
        assert!(dice.rolled("ground combat").is_empty());
        assert!(crate::coexistence::is_coexisting(
            &state,
            &system,
            &planet,
            &a()
        ));
        assert_eq!(report.coexisted, vec![planet.clone()]);
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b()),
            "a controller who coexists steps aside (coexistence 3.2)"
        );
    }

    // -- Myru Vos ---------------------------------------------------------------------------------

    const MYRU: &str = "leader:firmament:firmamentagent:SYSTEM_ACTIVATED:after";

    fn agent_status(state: &GameState) -> Option<LeaderStatus> {
        state
            .player(&a())
            .unwrap()
            .leaders
            .get(&LeaderId::new(AGENT))
            .copied()
    }

    #[test]
    fn myru_vos_covers_the_activation_it_is_used_in_and_exhausts() {
        let (mut state, _) = eye();
        let target = SystemId::new("18");
        crate::fixtures::put(&mut state, &SystemId::new("19"), "cruiser", &b(), 1);
        let activated = [
            ("system", target.to_string().into()),
            ("player", "b".into()),
        ];
        emit(&mut state, None, &[MYRU], "SYSTEM_ACTIVATED", &activated);
        assert_eq!(agent_status(&state), Some(LeaderStatus::Exhausted));
        assert!(space_cannon_silenced(&state, &b()));
        assert!(
            !space_cannon_silenced(&state, &c()),
            "only the mover's ships"
        );
        state.activation_seq += 1;
        assert!(!space_cannon_silenced(&state, &b()), "only that activation");
    }

    #[test]
    fn myru_vos_is_a_may_and_needs_a_mover_with_ships_elsewhere_and_a_ready_agent() {
        let target = SystemId::new("18");
        let activated = [
            ("system", target.to_string().into()),
            ("player", "b".into()),
        ];
        // Declined (the default decline answer).
        let mut declined = game();
        crate::fixtures::put(&mut declined, &SystemId::new("19"), "cruiser", &b(), 1);
        emit(
            &mut declined,
            None,
            &["decline"],
            "SYSTEM_ACTIVATED",
            &activated,
        );
        assert_eq!(agent_status(&declined), Some(LeaderStatus::Readied));
        assert!(!space_cannon_silenced(&declined, &b()));
        // The mover has no ship outside the active system: nothing to cover.
        let mut none = game();
        none.board
            .values_mut()
            .for_each(|system| system.units.retain(|unit| unit.owner != b()));
        emit(&mut none, None, &[MYRU], "SYSTEM_ACTIVATED", &activated);
        assert_eq!(agent_status(&none), Some(LeaderStatus::Readied));
        // An exhausted agent is not offered.
        let mut spent = game();
        crate::fixtures::put(&mut spent, &SystemId::new("19"), "cruiser", &b(), 1);
        spent
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(AGENT), LeaderStatus::Exhausted);
        emit(&mut spent, None, &[MYRU], "SYSTEM_ACTIVATED", &activated);
        assert!(!space_cannon_silenced(&spent, &b()));
    }

    #[test]
    fn space_cannon_cannot_be_used_against_the_covered_ships() {
        let mut state = game();
        let (system, planet) = plain_planet();
        state.system_mut(&system).set_control(planet.clone(), c());
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &c(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let shots = |state: &mut GameState| -> usize {
            let mut dice = crate::dice::Dice::from_faces(std::iter::repeat_n(10, 12));
            let mut rng = crate::rng::GameRng::new(0);
            crate::combat::space_cannon_offense(
                state,
                content(),
                DEFAULT,
                &mut dice,
                &mut rng,
                &system,
                &b(),
                None,
            )
            .into_iter()
            .filter(|(owner, _, _)| *owner == c())
            .map(|(_, hits, _)| hits)
            .sum()
        };
        assert!(
            shots(&mut state.clone()) > 0,
            "the PDS fires at an uncovered ship"
        );
        crate::fixtures::put(&mut state, &SystemId::new("19"), "cruiser", &b(), 1);
        let activated = [
            ("system", system.to_string().into()),
            ("player", "b".into()),
        ];
        emit(&mut state, None, &[MYRU], "SYSTEM_ACTIVATED", &activated);
        assert_eq!(
            shots(&mut state),
            0,
            "SPACE CANNON cannot be used against them"
        );
    }

    /// A one-ring map with a Sol cruiser at one ring system, a Hacan destroyer in the centre, and
    /// the system opposite as the active system: the only route of two steps crosses the centre.
    fn blockade() -> (GameState, Galaxy, String, String, String) {
        let ids = plain(7);
        let refs: Vec<&str> = ids[1..].iter().map(String::as_str).collect();
        let galaxy = ring(&ids[0], &refs);
        let origin = ids[1].clone();
        let active = across(&galaxy, &refs, &origin);
        let mut state = game();
        crate::fixtures::put(
            &mut state,
            &SystemId::new(origin.as_str()),
            "carrier",
            &b(),
            1,
        );
        crate::fixtures::put(
            &mut state,
            &SystemId::new(ids[0].as_str()),
            "destroyer",
            &c(),
            1,
        );
        (state, galaxy, origin, active, ids[0].clone())
    }

    fn route(
        state: &GameState,
        galaxy: &Galaxy,
        origin: &str,
        active: &str,
    ) -> Option<Vec<String>> {
        crate::movement::MovementRules::with_laws(
            galaxy,
            content(),
            DEFAULT,
            active,
            crate::movement::Board::for_player(state, content(), DEFAULT, &b()),
            Some(state),
        )
        .path_from_ship(origin, 2, Some("carrier"))
    }

    #[test]
    fn an_unladen_ship_covered_by_myru_vos_moves_through_other_players_ships_without_cargo() {
        let (mut state, galaxy, origin, active, centre) = blockade();
        assert_eq!(route(&state, &galaxy, &origin, &active), None, "blockaded");
        let activated = [("system", active.clone().into()), ("player", "b".into())];
        emit(
            &mut state,
            Some(&galaxy),
            &[MYRU],
            "SYSTEM_ACTIVATED",
            &activated,
        );
        let path = route(&state, &galaxy, &origin, &active).expect("through the blockade");
        assert!(path.contains(&centre));
        // Cargo: the ship may not carry units along a route that crosses the blockade.
        let origin_id = SystemId::new(origin.as_str());
        crate::fixtures::put(&mut state, &origin_id, "infantry", &b(), 1);
        let ship = Unit::new(UnitTypeId::new("carrier"), b());
        assert!(unladen_route_forbids_cargo(
            &state,
            content(),
            DEFAULT,
            &b(),
            &ship,
            &path
        ));
        let hold = crate::transit::CargoWindow::for_ship(
            &state,
            content(),
            DEFAULT,
            &b(),
            &origin_id,
            &ship,
            &path,
        );
        assert!(hold.is_complete(), "nothing can be loaded");
        // Without the agent the same ship may load (on a route that is open).
        let open: Vec<String> = vec![origin.clone(), active.clone()];
        assert!(!unladen_route_forbids_cargo(
            &state,
            content(),
            DEFAULT,
            &b(),
            &ship,
            &open
        ));
        let hold = crate::transit::CargoWindow::for_ship(
            &state,
            content(),
            DEFAULT,
            &b(),
            &origin_id,
            &ship,
            &open,
        );
        assert!(!hold.is_complete(), "an open route can carry the infantry");
    }

    #[test]
    fn myru_vos_changes_nothing_for_a_ship_that_can_sail_clear() {
        let (mut state, galaxy, origin, active, _) = blockade();
        // Remove the blockade: the ordinary route is found first and cargo stays allowed.
        state
            .board
            .values_mut()
            .for_each(|system| system.units.retain(|unit| unit.owner != c()));
        let activated = [("system", active.clone().into()), ("player", "b".into())];
        emit(
            &mut state,
            Some(&galaxy),
            &[MYRU],
            "SYSTEM_ACTIVATED",
            &activated,
        );
        let path = route(&state, &galaxy, &origin, &active).expect("clear");
        let ship = Unit::new(UnitTypeId::new("carrier"), b());
        assert!(!unladen_route_forbids_cargo(
            &state,
            content(),
            DEFAULT,
            &b(),
            &ship,
            &path
        ));
    }

    // -- Sharsiss ---------------------------------------------------------------------------------

    fn hero_game() -> GameState {
        let mut state = game();
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new(HERO), LeaderStatus::Unlocked);
        state
    }

    fn use_hero(state: &mut GameState, answers: &[&str]) -> bool {
        let mut table = scripted(answers);
        crate::fixtures::with_context(state, DEFAULT, None, &mut table, |context| {
            crate::leaders::use_leader(context, &a(), &LeaderId::new(HERO))
        })
    }

    #[test]
    fn sharsiss_places_a_plot_with_another_players_token_then_may_add_a_token_and_is_purged() {
        let mut state = hero_game();
        assert!(use_hero(&mut state, &["b", "token|c|0"]));
        assert_eq!(state.player(&a()).unwrap().plots, ["d:b,c"]);
        assert_eq!(
            state
                .player(&a())
                .unwrap()
                .leaders
                .get(&LeaderId::new(HERO)),
            Some(&LeaderStatus::Purged)
        );
    }

    #[test]
    fn sharsiss_second_clause_is_a_may_and_never_doubles_a_token() {
        let mut state = hero_game();
        assert!(use_hero(&mut state, &["b", "decline"]));
        assert_eq!(state.player(&a()).unwrap().plots, ["d:b"]);
        // The second clause never offers b's token for the card that already carries it.
        let mut state = hero_game();
        let mut table = scripted(&["b", "token|b|0"]);
        let result =
            crate::fixtures::with_context(&mut state, DEFAULT, None, &mut table, |context| {
                hero(context, &a())
            });
        match result {
            Err(TimingError::IllegalChoice(crate::choice::IllegalChoice::ScriptDiverged {
                offered,
                ..
            })) => {
                assert!(!offered.iter().any(|id| id == "token|b|0"), "{offered:?}");
                assert!(offered.iter().any(|id| id == "token|a|0"), "{offered:?}");
            }
            other => panic!("expected the script to diverge, got {other:?}"),
        }
    }

    #[test]
    fn sharsiss_can_be_used_as_the_leaders_action_only_by_the_unlocked_firmament() {
        let state = hero_game();
        assert!(crate::leaders::usable(&state, content(), &a()).contains(&LeaderId::new(HERO)));
        assert!(!crate::leaders::usable(&game(), content(), &a()).contains(&LeaderId::new(HERO)));
    }

    // -- neutrality -------------------------------------------------------------------------------

    #[test]
    fn a_game_without_the_firmament_is_untouched_by_every_window() {
        let mut state =
            crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")], DEFAULT);
        state.record_score(&b(), ObjectiveId::new("fwm"));
        crate::fixtures::put(&mut state, &SystemId::new("18"), "spacedock", &a(), 3);
        state
            .promissory_notes
            .insert(crate::promissory::note_id(NOTE, FACTION), b());
        let before = state.clone();
        let activated = [("system", "18".into()), ("player", "b".into())];
        emit(&mut state, None, &[], "STATUS_PHASE_BEGAN", &[]);
        emit(&mut state, None, &[], "SYSTEM_ACTIVATED", &activated);
        emit(&mut state, None, &[], "TRANSACTION_RESOLVED", &resolved());
        emit(
            &mut state,
            None,
            &[],
            "SPACE_COMBAT_ROUND_ENDED",
            &[("system", "18".into())],
        );
        emit(
            &mut state,
            None,
            &[],
            "TECHNOLOGY_GAINED",
            &[("player", "a".into()), ("technology", PLANESPLITTER.into())],
        );
        assert_eq!(state.faction_marks, before.faction_marks);
        assert_eq!(state.board, before.board);
        assert!(!state.fracture_in_play);
        for seat in &state.players {
            assert_eq!(
                seat.trade_goods,
                before.player(&seat.id).unwrap().trade_goods
            );
            assert!(seat.plots.is_empty());
        }
        assert!(
            status_window(&state)
                .pending_choice(&state, content(), DEFAULT)
                .is_none(),
            "nobody is offered a plot"
        );
        assert!(!space_cannon_silenced(&state, &a()));
    }

    use ti4_content::galaxy::Galaxy;
}
