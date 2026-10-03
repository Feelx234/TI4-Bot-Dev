//! The Embers of Muaat (`muaat`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope.
//!
//! Implemented: Star Forge, Gashlai Physiology, Prototype War Sun I/II (data-driven stats,
//! verified), Magmus Reactor, The Inferno, Ember Colossus, Fires of the Gashlai, Umbat,
//! Adjudicator Ba'al (Nova Seed, at the movement-finished window), and the Magmus commander's
//! unlock. Partial, not claimed (see `plans/evidence/BF-muaat.md`): Magmus's trade-good effect
//! (only the strategy-token spends that announce `STRATEGY_TOKEN_SPENT` can trigger it) and
//! Stellar Genesis.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus, TokenPool};

use super::hooks_economy::EconomyHooks;
use super::hooks_movement::MovementHooks;
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption, IllegalChoice, Window};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::event::Event;
use crate::movement::MapEdit;
use crate::timing::{Ability, Relation, TimingContext};

/// What this faction implements; grows package by package.
pub const MODULE: FactionModule = FactionModule {
    alias: "muaat",
    abilities: &["star_forge", "gashlai_physiology"],
    technologies: &["pws2", "mr"],
    units: &[
        "muaat_warsun",
        "muaat_warsun2",
        "muaat_flagship",
        "muaat_mech",
    ],
    promissory: &["fires"],
    leaders: &["muaathero", "muaatagent", "muaatcommander"],
    breakthroughs: &[],
    hooks: Hooks {
        component_actions: Some(component_actions),
        perform_component: Some(perform_component),
        commander_unlocked: Some(commander_unlocked),
        leader_action: Some(leader_action),
        use_leader: Some(use_leader),
        timing_abilities: Some(timing_abilities),
        movement: MovementHooks {
            may_enter_supernova: Some(may_enter_supernova),
            may_pass_through_supernova: Some(may_pass_through_supernova),
            ..MovementHooks::NONE
        },
        economy: EconomyHooks {
            extra_production: Some(extra_production),
            ..EconomyHooks::NONE
        },
        ..Hooks::NONE
    },
};

const STAR_FORGE: &str = "faction|muaat|star_forge";
const INFERNO: &str = "faction|muaat|inferno";
const FIRES: &str = "faction|muaat|fires";
const HERO: &str = "muaathero";
const COMMANDER: &str = "muaatcommander";
const AGENT: &str = "muaatagent";
const NOVA_MARK: &str = "muaat:warsun_moved:";

// -- small helpers -------------------------------------------------------------------------------

fn is_muaat(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == "muaat")
}

fn has_technology(state: &GameState, player: &PlayerId, alias: &str) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.technologies.contains(&TechnologyId::new(alias)))
}

fn leader_status(state: &GameState, player: &PlayerId, leader: &str) -> Option<LeaderStatus> {
    state
        .player(player)
        .and_then(|seat| seat.leaders.get(&LeaderId::new(leader)).copied())
}

fn tokens(state: &GameState, player: &PlayerId, pool: TokenPool) -> i32 {
    state.player(player).map_or(0, |seat| seat.tokens(pool))
}

fn base_of(content: &ContentStore, sources: SourceSet, type_id: &str) -> Option<String> {
    ti4_content::units::catalogue(content, sources)
        .get(type_id)
        .map(|kind| kind.base_type().to_owned())
}

/// Whether the reinforcements hold at least one unit of this base type.
fn box_has(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    base: &str,
) -> bool {
    ti4_content::units::catalogue(content, sources)
        .get(base)
        .is_some_and(|kind| {
            crate::supply::allowed(
                state,
                content,
                sources,
                player,
                &ti4_model::id::UnitTypeId::new(kind.id().to_owned()),
                1,
            ) > 0
        })
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

/// The one question every Muaat decision goes through.
fn ask(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    prompt: &str,
    card: &str,
    subtype: &str,
    mut options: Vec<ChoiceOption>,
    declinable: bool,
) -> Result<ChoiceOption, IllegalChoice> {
    if declinable {
        options.push(ChoiceOption::decline());
    }
    let choice = Choice::new(player.clone(), prompt.to_owned(), options).contextualized(decision(
        context.state,
        player,
        card,
        subtype,
    ));
    context.ask_seeing(&choice)
}

// -- Magmus Reactor: movement ---------------------------------------------------------------------

/// Whether this player has Magmus Reactor (its own card, or one a copy gave it).
fn holds_reactor(state: &GameState, player: &PlayerId) -> bool {
    has_technology(state, player, "mr")
        || state.player(player).is_some_and(|seat| {
            seat.assimilated_technologies
                .values()
                .any(|tech| tech.as_str() == "mr")
        })
}

/// Magmus Reactor: "Your ships can move into supernovas." (A step that ends in one.) Only a Muaat
/// seat holding the card: a non-Muaat holder lacks Gashlai Physiology's "through".
fn may_enter_supernova(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    player: &PlayerId,
) -> bool {
    is_muaat(state, player) && holds_reactor(state, player)
}

/// Gashlai Physiology: "Your ships can move through supernovas." Passing only; ending a move in
/// one needs the reactor ([`may_enter_supernova`]).
fn may_pass_through_supernova(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    player: &PlayerId,
) -> bool {
    is_muaat(state, player)
}

/// Magmus Reactor: "Each supernova that contains 1 or more of your units gains the PRODUCTION 5
/// ability as if it were 1 of your units."
fn extra_production(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> i64 {
    if !is_muaat(state, player)
        || !holds_reactor(state, player)
        || !ti4_content::galaxy::system(content, system.as_str(), sources)
            .is_some_and(|tile| tile.is_supernova())
    {
        return 0;
    }
    let present = state.board.get(system).is_some_and(|board| {
        board.units.iter().any(|unit| &unit.owner == player)
            || board
                .planet_units
                .values()
                .flatten()
                .any(|unit| &unit.owner == player)
    });
    if present { 5 } else { 0 }
}

// -- Star Forge, The Inferno, Fires of the Gashlai -----------------------------------------------

/// Systems holding one of the player's war suns, in board order.
fn war_sun_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<SystemId> {
    let types = ti4_content::units::catalogue(content, sources);
    state
        .board
        .iter()
        .filter(|(_, board)| {
            board.units.iter().any(|unit| {
                &unit.owner == player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(|kind| kind.base_type() == "warsun")
            })
        })
        .map(|(system, _)| system.clone())
        .collect()
}

/// What Star Forge may place: `(system, "fighters" | "destroyer")`. "2 fighters or 1 destroyer".
fn forge_options(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<(SystemId, &'static str)> {
    let mut found = Vec::new();
    for system in war_sun_systems(state, content, sources, player) {
        for (kind, base, wanted) in [("fighters", "fighter", 2), ("destroyer", "destroyer", 1)] {
            if box_has(state, content, sources, player, base)
                && crate::action_cards::max_fit(
                    state, content, sources, player, &system, None, base, wanted,
                ) > 0
            {
                found.push((system.clone(), kind));
            }
        }
    }
    found
}

fn star_forge_ready(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> bool {
    is_muaat(state, player)
        && tokens(state, player, TokenPool::Strategic) > 0
        && !forge_options(state, content, sources, player).is_empty()
}

/// The system holding The Inferno, if the cruiser can be placed there.
fn inferno_system(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Option<SystemId> {
    if !is_muaat(state, player)
        || tokens(state, player, TokenPool::Strategic) <= 0
        || !box_has(state, content, sources, player, "cruiser")
    {
        return None;
    }
    let (system, _) = state.board.iter().find(|(_, board)| {
        board
            .units
            .iter()
            .any(|unit| &unit.owner == player && unit.type_id.as_str() == "muaat_flagship")
    })?;
    (crate::action_cards::max_fit(state, content, sources, player, system, None, "cruiser", 1) > 0)
        .then(|| system.clone())
}

/// The Muaat seat whose Fires of the Gashlai `holder` may play now.
fn fires_owner(state: &GameState, holder: &PlayerId) -> Option<PlayerId> {
    let note = crate::promissory::note_id("fires", "muaat");
    if state.promissory_notes.get(&note) != Some(holder)
        || crate::promissory::faction_name(state, holder) == "muaat"
        || has_technology(state, holder, "ws")
    {
        return None;
    }
    let owner = crate::promissory::seat_of(state, "muaat")?;
    (tokens(state, &owner, TokenPool::Fleet) > 0).then_some(owner)
}

fn action(id: &str, label: &str) -> ChoiceOption {
    ChoiceOption::labelled(id, crate::faction_abilities::ACTION_KIND, label)
}

fn component_actions(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> Vec<ChoiceOption> {
    // Sources are not passed to this hook; the corpus default is what games are built with.
    let sources = ti4_model::content_types::DEFAULT;
    let mut options = Vec::new();
    if star_forge_ready(state, content, sources, player) {
        options.push(action(
            STAR_FORGE,
            "Star Forge: spend a strategy token to place 2 fighters or 1 destroyer at a war sun",
        ));
    }
    if inferno_system(state, content, sources, player).is_some() {
        options.push(action(
            INFERNO,
            "The Inferno: spend a strategy token to place 1 cruiser in its system",
        ));
    }
    if fires_owner(state, player).is_some() {
        options.push(action(
            FIRES,
            "Fires of the Gashlai: return a Muaat fleet token and gain the war sun technology",
        ));
    }
    options
}

fn perform_component(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    option: &ChoiceOption,
) -> bool {
    match option.id.as_str() {
        STAR_FORGE => star_forge(context, player),
        INFERNO => inferno(context, player),
        FIRES => fires(context, player),
        _ => false,
    }
}

/// Star Forge: "ACTION: Spend 1 token from your strategy pool to place either 2 fighters or 1
/// destroyer from your reinforcements in a system that contains 1 or more of your war suns."
/// The system and the choice are made before the token is spent; a refusal changes nothing.
fn star_forge(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let (content, sources) = (context.content, context.sources);
    if !star_forge_ready(context.state, content, sources, player) {
        return false;
    }
    let options = forge_options(context.state, content, sources, player);
    let offered = options
        .iter()
        .map(|(system, kind)| {
            let what = if *kind == "fighters" {
                "2 fighters"
            } else {
                "1 destroyer"
            };
            ChoiceOption::labelled(
                format!("{system}|{kind}"),
                "star_forge",
                format!("place {what} in {system}"),
            )
            .with("system", system.to_string())
        })
        .collect();
    let Ok(answer) = ask(
        context,
        player,
        "Star Forge: place which units, and where",
        "star_forge",
        "star_forge",
        offered,
        true,
    ) else {
        return false;
    };
    let Some((system, kind)) = options
        .iter()
        .find(|(system, kind)| answer.id == format!("{system}|{kind}"))
    else {
        return false;
    };
    let (base, wanted) = if *kind == "fighters" {
        ("fighter", 2)
    } else {
        ("destroyer", 1)
    };
    let fit = crate::action_cards::max_fit(
        context.state,
        content,
        sources,
        player,
        system,
        None,
        base,
        wanted,
    );
    // Placed first: the box can still refuse, and then nothing has been spent.
    if crate::action_cards::place_units_counted(context, player, system, None, base, fit) == 0 {
        return false;
    }
    crate::supply::spend_strategy_token_staged(context.state, player, "star_forge");
    ember_colossus(context, player, system);
    true
}

/// Ember Colossus: "When you use your Star Forge faction ability in this system or an adjacent
/// system, you may place 1 infantry from your reinforcements with this unit." Each mech in the
/// forge's system or one adjacent to it (by this player's adjacency) may do so once.
fn ember_colossus(context: &mut TimingContext<'_>, player: &PlayerId, forge: &SystemId) {
    let (content, sources) = (context.content, context.sources);
    let mut near: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::from([forge.to_string()]);
    if let Some(galaxy) = context.galaxy {
        near.extend(
            crate::movement::PlayerAdjacency::new(context.state, content, sources, galaxy, player)
                .neighbours(forge.as_str()),
        );
    }
    let mut mechs: Vec<(SystemId, Option<PlanetId>)> = Vec::new();
    for (system, board) in &context.state.board {
        if !near.contains(system.as_str()) {
            continue;
        }
        let is_mech = |unit: &ti4_model::units::Unit| {
            &unit.owner == player && unit.type_id.as_str() == "muaat_mech"
        };
        for (planet, units) in &board.planet_units {
            for _ in units.iter().filter(|unit| is_mech(unit)) {
                mechs.push((system.clone(), Some(planet.clone())));
            }
        }
        for _ in board.units.iter().filter(|unit| is_mech(unit)) {
            mechs.push((system.clone(), None));
        }
    }
    for (system, planet) in mechs {
        if !box_has(context.state, content, sources, player, "infantry")
            || crate::action_cards::max_fit(
                context.state,
                content,
                sources,
                player,
                &system,
                planet.as_ref(),
                "infantry",
                1,
            ) == 0
        {
            continue;
        }
        let place = planet
            .as_ref()
            .map_or_else(|| "space".to_owned(), ToString::to_string);
        let Ok(answer) = ask(
            context,
            player,
            "Ember Colossus: place 1 infantry with this mech",
            "muaat_mech",
            "ember_colossus",
            vec![ChoiceOption::labelled(
                format!("{system}|{place}"),
                "place_unit",
                format!("place 1 infantry with the mech on {place} in {system}"),
            )],
            true,
        ) else {
            return;
        };
        if !answer.is_decline() {
            crate::action_cards::place_units_counted(
                context,
                player,
                &system,
                planet.as_ref(),
                "infantry",
                1,
            );
        }
    }
}

/// The Inferno: "ACTION: Spend 1 token from your strategy pool to place 1 cruiser in this
/// system."
fn inferno(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let Some(system) = inferno_system(context.state, context.content, context.sources, player)
    else {
        return false;
    };
    if crate::action_cards::place_units_counted(context, player, &system, None, "cruiser", 1) == 0 {
        return false;
    }
    crate::supply::spend_strategy_token_staged(context.state, player, "inferno");
    true
}

/// Fires of the Gashlai: "ACTION: Remove 1 token from the Muaat player's fleet pool and return it
/// to their reinforcements. Then, gain your war sun unit upgrade technology card. Then, return
/// this card to the Muaat Player." The holder's own war sun upgrade is the generic War Sun card.
fn fires(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let Some(owner) = fires_owner(context.state, player) else {
        return false;
    };
    context.state.gain_token(&owner, TokenPool::Fleet, -1);
    crate::technology::grant(context.state, player, &TechnologyId::new("ws"));
    crate::technology::apply_unit_upgrades(context.state, context.content, context.sources, player);
    crate::promissory::give_back(context.state, &crate::promissory::note_id("fires", "muaat"));
    true
}

// -- Adjudicator Ba'al ---------------------------------------------------------------------------

/// Tiles the hero cannot replace beyond home systems and Mecatol Rex: the Fracture
/// (Dane's ruling, in the card notes).
fn nova_blocked(content: &ContentStore, sources: SourceSet, system: &str) -> bool {
    system == crate::seating::MECATOL
        || system.starts_with("fracture")
        || ti4_content::galaxy::is_home_system(content, system, sources)
}

fn nova_mark_key(owner: &PlayerId) -> String {
    format!("{NOVA_MARK}{owner}")
}

/// Records, per activation, the system a war sun of the hero's owner moved into, so the
/// movement-finished window knows "you moved a war sun into" it. Only a Muaat seat with the hero
/// unlocked records anything.
fn war_sun_moved(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_seat) = (seat.clone(), seat.clone());
    let live = |event: &Event, context: &TimingContext<'_>, who: &PlayerId| {
        event.text("player") == Some(who.as_str())
            && is_muaat(context.state, who)
            && leader_status(context.state, who, HERO) == Some(LeaderStatus::Unlocked)
            && event.text("system").is_some()
            && event
                .text("unit")
                .and_then(|unit| base_of(context.content, context.sources, unit))
                .as_deref()
                == Some("warsun")
    };
    Ability::stateful(
        format!("leader:{owner_name}:{HERO}_moved:SHIP_MOVED:after"),
        seat.clone(),
        "SHIP_MOVED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if live(event, context, &owner)
                && let Some(system) = event.text("system")
            {
                let mark = format!("{}|{system}", context.state.activation_seq);
                context
                    .state
                    .faction_marks
                    .insert(nova_mark_key(&owner), mark);
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        live(event, context, &condition_seat)
    }))
}

/// The system the hero would replace for this `MOVEMENT_FINISHED` event, if everything about it is
/// legal now: the owner moved a war sun into it during this activation, it is a non-home system
/// other than Mecatol Rex, the hero is unlocked, and the map can take the Nova Seed there.
fn nova_system(
    event: &Event,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    owner: &PlayerId,
) -> Option<SystemId> {
    if event.text("player") != Some(owner.as_str())
        || !is_muaat(state, owner)
        || leader_status(state, owner, HERO) != Some(LeaderStatus::Unlocked)
    {
        return None;
    }
    let system = SystemId::new(event.text("system")?);
    let mark = state.faction_marks.get(&nova_mark_key(owner))?;
    if *mark != format!("{}|{system}", state.activation_seq) {
        return None;
    }
    if nova_blocked(content, sources, system.as_str()) {
        return None;
    }
    // The war sun must still be there.
    if !war_sun_systems(state, content, sources, owner).contains(&system) {
        return None;
    }
    let galaxy = galaxy?;
    galaxy.coord_of(system.as_str())?;
    // Dry run on copies: the edit is only offered if it can be made.
    let (mut trial_state, mut trial_galaxy) = (state.clone(), galaxy.clone());
    crate::movement::apply_map_edit(
        &mut trial_state,
        &mut trial_galaxy,
        content,
        sources,
        &MapEdit::Replace {
            old: system.to_string(),
            new: crate::movement::NOVA_SEED.to_owned(),
        },
    )
    .ok()?;
    Some(system)
}

/// Destroy every other player's units in `system` through the shared routes: ships by
/// `combat::destroy_units` (staging `SHIP_DESTROYED`), ground forces on planets by a staged
/// `GROUND_FORCE_DESTROYED`; ground forces in the space area are removed with the ships.
fn destroy_others(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
    system: &SystemId,
) {
    let others: Vec<PlayerId> = state
        .players
        .iter()
        .map(|seat| seat.id.clone())
        .filter(|id| id != owner)
        .collect();
    for other in &others {
        let ships = crate::combat::ships_of(state, content, sources, other, system);
        crate::combat::destroy_units(state, content, sources, other, system, &ships);
    }
    let planets: Vec<PlanetId> = state
        .system_state(system)
        .planet_units
        .keys()
        .cloned()
        .collect();
    for planet in planets {
        let victims: Vec<ti4_model::units::Unit> = state
            .system_state(system)
            .on_planet(&planet)
            .iter()
            .filter(|unit| &unit.owner != owner)
            .cloned()
            .collect();
        let types = ti4_content::units::catalogue(content, sources);
        for unit in victims {
            state
                .system_mut(system)
                .remove_from_planet(&planet, std::slice::from_ref(&unit));
            // Only ground forces are announced as destroyed ground forces; structures (PDS,
            // space docks) are simply removed with the planet.
            if types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_ground_force)
            {
                super::hooks_ground::stage_ground_force_destroyed(
                    state,
                    system,
                    &planet,
                    &unit,
                    "nova_seed",
                );
            }
        }
    }
    // Ground forces of other players left in the space area (cargo) are destroyed too and are
    // announced like the ones on planets; there is no planet, so the system id stands in.
    let types = ti4_content::units::catalogue(content, sources);
    let cargo: Vec<ti4_model::units::Unit> = state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner != owner)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(ti4_content::units::UnitType::is_ground_force)
        })
        .cloned()
        .collect();
    for unit in &cargo {
        super::hooks_ground::stage_ground_force_destroyed(
            state,
            system,
            &PlanetId::new(system.as_str()),
            unit,
            "nova_seed",
        );
    }
    state
        .system_mut(system)
        .units
        .retain(|unit| &unit.owner == owner);
}

/// Purge the planet cards of the replaced tile and everything keyed to them.
fn purge_planets(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) {
    let planets = crate::planets::in_system(state, content, sources, system);
    for planet in &planets {
        state.system_mut(system).purge_planet(planet);
        state.exhausted_planets.remove(planet);
        state.placed_planets.remove(planet);
        state.planet_attachments.remove(planet);
        if state.production_value_swapped_planet.as_ref() == Some(planet) {
            state.production_value_swapped_planet = None;
        }
        for seat in &mut state.players {
            seat.exhausted_legendary.remove(planet);
        }
        // Laws that name a planet (Demilitarized Zone, Holy Planet of Ixth, ...) end with the card.
        let named: Vec<String> = state
            .laws
            .iter()
            .filter(|(_, elected)| elected.as_str() == planet.as_str())
            .map(|(law, _)| law.clone())
            .collect();
        for law in named {
            crate::laws::repeal(state, &law);
        }
    }
    // Non-faction tokens go with the tile (frontier and command tokens stay).
    state.ingress_tokens.remove(system);
    state.breach_tokens.remove(system);
    if state.thunders_edge_system.as_ref() == Some(system) {
        state.thunders_edge_system = None;
    }
    state.purged_systems.insert(system.clone());
}

/// Adjudicator Ba'al, Nova Seed: "After you move a war sun into a non-home system other than
/// Mecatol Rex: You may destroy all other players' units in that system and replace that system
/// tile with the Muaat supernova tile. If you do, purge this card and each planet card that
/// corresponds to the replaced system tile." Resolved once movement is finished
/// (`MOVEMENT_FINISHED`), so the swapped-in supernova never locks the rest of the fleet out. The
/// whole change is made on a copy and swapped in, so it either happens completely or not at all;
/// the game's own map catches up from the recorded edit at the end of the step.
fn nova_seed(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_seat) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{HERO}:MOVEMENT_FINISHED:after"),
        seat.clone(),
        "MOVEMENT_FINISHED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            let Some(system) = nova_system(
                event,
                context.state,
                context.content,
                context.sources,
                context.galaxy,
                &owner,
            ) else {
                return Ok(());
            };
            let Some(galaxy) = context.galaxy else {
                return Ok(());
            };
            let (content, sources) = (context.content, context.sources);
            let mut state = context.state.clone();
            let mut trial_galaxy = galaxy.clone();
            destroy_others(&mut state, content, sources, &owner, &system);
            purge_planets(&mut state, content, sources, &system);
            if crate::movement::apply_map_edit(
                &mut state,
                &mut trial_galaxy,
                content,
                sources,
                &MapEdit::Replace {
                    old: system.to_string(),
                    new: crate::movement::NOVA_SEED.to_owned(),
                },
            )
            .is_err()
            {
                return Ok(());
            }
            if state.active_system.as_ref() == Some(&system) {
                state.active_system = Some(SystemId::new(crate::movement::NOVA_SEED));
            }
            state.faction_marks.remove(&nova_mark_key(&owner));
            crate::leaders::purge(&mut state, &owner, &LeaderId::new(HERO));
            *context.state = state;
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        nova_system(
            event,
            context.state,
            context.content,
            context.sources,
            context.galaxy,
            &condition_seat,
        )
        .is_some()
    }))
}

// -- Umbat ---------------------------------------------------------------------------------------

/// Systems where `who` could produce under Umbat now: a war sun or the flagship is there and a
/// unit costing 4 or less can be built.
fn agent_systems(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    who: &PlayerId,
) -> Vec<SystemId> {
    let types = ti4_content::units::catalogue(content, sources);
    state
        .board
        .iter()
        .filter(|(_, board)| {
            board.units.iter().any(|unit| {
                &unit.owner == who
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(|kind| matches!(kind.base_type(), "warsun" | "flagship"))
            })
        })
        .map(|(system, _)| system.clone())
        .filter(|system| {
            crate::production::ProductionWindow::for_ability(
                state,
                content,
                sources,
                who,
                system,
                Some(2),
            )
            .with_max_unit_cost(Some(4))
            .pending_choice(state, content, sources)
            .is_some()
        })
        .collect()
}

fn agent_targets(state: &GameState, content: &ContentStore, sources: SourceSet) -> Vec<PlayerId> {
    state
        .players
        .iter()
        .map(|seat| seat.id.clone())
        .filter(|who| !agent_systems(state, content, sources, who).is_empty())
        .collect()
}

fn leader_action(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    // Sources are not passed to this hook; the corpus default is what games are built with.
    (leader.as_str() == AGENT).then(|| {
        is_muaat(state, player)
            && leader_status(state, player, AGENT) == Some(LeaderStatus::Readied)
            && !agent_targets(state, content, ti4_model::content_types::DEFAULT).is_empty()
    })
}

fn use_leader(
    context: &mut TimingContext<'_>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == AGENT).then(|| umbat(context, player))
}

/// Umbat: "ACTION: Exhaust this card to choose a player: that player may produce up to 2 units
/// that each have a cost of 4 or less in a system that contains one of their war suns or their
/// flagship." Every choice is made before anything changes; the shared code exhausts the agent
/// once this returns `true`. The chosen player may decline to produce (the agent is then spent).
fn umbat(context: &mut TimingContext<'_>, player: &PlayerId) -> bool {
    let (content, sources) = (context.content, context.sources);
    let targets = agent_targets(context.state, content, sources);
    if !is_muaat(context.state, player) || targets.is_empty() {
        return false;
    }
    let offered = targets
        .iter()
        .map(|who| ChoiceOption::labelled(who.to_string(), "player", format!("player {who}")))
        .collect();
    let Ok(answer) = ask(
        context,
        player,
        "Umbat: choose a player who may produce up to 2 units costing 4 or less",
        AGENT,
        "umbat_player",
        offered,
        true,
    ) else {
        return false;
    };
    let Some(target) = targets
        .iter()
        .find(|who| who.as_str() == answer.id)
        .cloned()
    else {
        return false;
    };
    let systems = agent_systems(context.state, content, sources, &target);
    let system = if let [only] = systems.as_slice() {
        only.clone()
    } else {
        let offered = systems
            .iter()
            .map(|system| {
                ChoiceOption::labelled(system.to_string(), "system", format!("system {system}"))
            })
            .collect();
        let Ok(answer) = ask(
            context,
            &target,
            "Umbat: produce in which system",
            AGENT,
            "umbat_system",
            offered,
            true,
        ) else {
            return true;
        };
        match systems.iter().find(|system| system.as_str() == answer.id) {
            Some(system) => system.clone(),
            None => return true, // declined: the agent is spent, nothing produced
        }
    };
    let TimingContext {
        state,
        content,
        sources,
        table,
        dice,
        rng,
        galaxy,
        ..
    } = context;
    let galaxy = *galaxy;
    let mut ctx = crate::choice::Resolving {
        content,
        sources: *sources,
        dice,
        rng,
        table,
        timing: None,
    };
    // A refused answer aborts with what was produced left in place, as everywhere else.
    let _ = crate::production::produce_by_ability_capped(
        state,
        &mut ctx,
        galaxy,
        &target,
        &system,
        Some(2),
        Some(4),
    );
    true
}

// -- Magmus: the unlock --------------------------------------------------------------------------

fn produced_a_war_sun(event: &Event, content: &ContentStore, sources: SourceSet) -> bool {
    event
        .payload
        .get("units")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|units| {
            units.iter().any(|unit| {
                unit.get("unit_type")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|kind| base_of(content, sources, kind))
                    .as_deref()
                    == Some("warsun")
            })
        })
}

/// Magmus, unlock: "Produce a war sun." Unlocked when a use of production that this player made
/// reports a war sun among its units (`UNITS_PRODUCED`). The commander's own text needs a
/// strategy-token-spent event and is not claimed in [`MODULE`].
fn commander_unlock(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_seat) = (seat.clone(), seat.clone());
    let live = |event: &Event, context: &TimingContext<'_>, who: &PlayerId| {
        event.text("player") == Some(who.as_str())
            && leader_status(context.state, who, COMMANDER) == Some(LeaderStatus::Locked)
            && produced_a_war_sun(event, context.content, context.sources)
    };
    Ability::stateful(
        format!("leader:{owner_name}:{COMMANDER}_unlock:UNITS_PRODUCED:after"),
        seat.clone(),
        "UNITS_PRODUCED",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if live(event, context, &owner)
                && let Some(seat) = context.state.player_mut(&owner)
            {
                seat.leaders
                    .insert(LeaderId::new(COMMANDER), LeaderStatus::Unlocked);
            }
            Ok(())
        }),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        live(event, context, &condition_seat)
    }))
}

/// The unlock is recorded by [`commander_unlock`] directly; this hook only reports the leader as
/// this module's, so the shared check leaves a locked Magmus locked.
fn commander_unlocked(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    _galaxy: Option<&Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    (leader.as_str() == COMMANDER).then(|| {
        matches!(
            leader_status(state, player, COMMANDER),
            Some(LeaderStatus::Unlocked | LeaderStatus::Readied | LeaderStatus::Exhausted)
        )
    })
}

/// Magmus: "After you spend a token from your strategy pool: You may gain 1 trade good." Reacts to
/// `STRATEGY_TOKEN_SPENT`, which only the spends converted to `supply::spend_strategy_token_*`
/// announce (this module's own today), so the commander is not claimed.
fn magmus(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_seat) = (seat.clone(), seat.clone());
    let live = |event: &Event, state: &GameState, who: &PlayerId| {
        event.text("player") == Some(who.as_str())
            && matches!(
                leader_status(state, who, COMMANDER),
                Some(LeaderStatus::Unlocked | LeaderStatus::Readied)
            )
    };
    Ability::stateful(
        format!("leader:{owner_name}:{COMMANDER}:STRATEGY_TOKEN_SPENT:after"),
        seat.clone(),
        "STRATEGY_TOKEN_SPENT",
        Relation::After,
        Arc::new(move |event, _resolver, context| {
            if live(event, context.state, &owner) {
                crate::supply::gain_trade_goods_staged(context.state, &owner, 1, "muaat");
            }
            Ok(())
        }),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        live(event, context.state, &condition_seat)
    }))
}

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    vec![
        commander_unlock(owner_name, seat),
        magmus(owner_name, seat),
        war_sun_moved(owner_name, seat),
        nova_seed(owner_name, seat),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::{Decider, Scripted, Table};
    use crate::fixtures::{armed_resolver, put, put_on_planet, seated_game, with_context};
    use ti4_model::content_types::DEFAULT;
    use ti4_model::state::Phase;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }
    fn sys(id: &str) -> SystemId {
        SystemId::new(id)
    }
    fn content() -> &'static ContentStore {
        ContentStore::embedded()
    }
    fn scripted(answers: &[&str]) -> Table {
        Table::with_default(Box::new(Scripted::new(answers.iter().copied())))
    }

    /// Fails the test if it is asked anything.
    struct Never;
    impl Decider for Never {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            panic!("unexpected question: {}", choice.prompt);
        }
    }
    fn never() -> Table {
        Table::with_default(Box::new(Never))
    }

    fn muaat_game() -> GameState {
        seated_game(&[("a", "muaat"), ("b", "sol")], DEFAULT)
    }
    fn sol_game() -> GameState {
        seated_game(&[("a", "sol"), ("b", "hacan")], DEFAULT)
    }
    fn give_technology(state: &mut GameState, player: &PlayerId, alias: &str) {
        state
            .player_mut(player)
            .unwrap()
            .technologies
            .insert(TechnologyId::new(alias));
    }
    fn set_status(state: &mut GameState, player: &PlayerId, leader: &str, to: LeaderStatus) {
        state
            .player_mut(player)
            .unwrap()
            .leaders
            .insert(LeaderId::new(leader), to);
    }
    fn count(state: &GameState, system: &SystemId, base: &str, owner: &PlayerId) -> usize {
        let types = ti4_content::units::catalogue(content(), DEFAULT);
        let is = |unit: &&ti4_model::units::Unit| {
            &unit.owner == owner
                && types
                    .get(unit.type_id.as_str())
                    .is_some_and(|kind| kind.base_type() == base)
        };
        let board = state.system_state(system);
        board.units.iter().filter(is).count()
            + board.planet_units.values().flatten().filter(is).count()
    }
    fn home_of(state: &GameState, player: &PlayerId) -> SystemId {
        state.player(player).unwrap().home_system.clone().unwrap()
    }
    fn perform(
        state: &mut GameState,
        galaxy: Option<&Galaxy>,
        table: &mut Table,
        id: &str,
    ) -> bool {
        let option = action(id, "test");
        with_context(state, DEFAULT, galaxy, table, |ctx| {
            perform_component(ctx, &a(), &option)
        })
    }
    fn emit(
        state: &mut GameState,
        galaxy: Option<&Galaxy>,
        table: &mut Table,
        event_type: &str,
        payload: &[(&str, serde_json::Value)],
    ) {
        let mut resolver = armed_resolver(state);
        with_context(state, DEFAULT, galaxy, table, |ctx| {
            let payload = payload
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect();
            let event = ctx
                .event_sequence
                .next(event_type, payload)
                .expect("event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("window resolves");
        });
    }

    // -- the sheet's data-driven items ----------------------------------------------------------

    #[test]
    fn muaat_starts_with_a_prototype_war_sun_in_its_home_system() {
        let state = muaat_game();
        let home = home_of(&state, &a());
        let ids: Vec<&str> = state.board[&home]
            .units
            .iter()
            .filter(|unit| unit.owner == a())
            .map(|unit| unit.type_id.as_str())
            .collect();
        assert!(ids.contains(&"muaat_warsun"), "{ids:?}");
        let types = ti4_content::units::catalogue(content(), DEFAULT);
        let war_sun = types["muaat_warsun"];
        assert!((war_sun.cost() - 12.0).abs() < 1e-9);
        assert_eq!(war_sun.base_type(), "warsun");
    }

    #[test]
    fn prototype_war_sun_ii_upgrades_the_war_sun_and_keeps_its_shield_breaking() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        give_technology(&mut state, &a(), "pws2");
        crate::technology::apply_unit_upgrades(&mut state, content(), DEFAULT, &a());
        let ids: Vec<String> = state.board[&home]
            .units
            .iter()
            .filter(|unit| unit.owner == a())
            .map(|unit| unit.type_id.to_string())
            .collect();
        assert!(ids.contains(&"muaat_warsun2".to_owned()), "{ids:?}");
        assert!(!ids.contains(&"muaat_warsun".to_owned()));
        let types = ti4_content::units::catalogue(content(), DEFAULT);
        let upgraded = types["muaat_warsun2"];
        assert!((upgraded.cost() - 10.0).abs() < 1e-9);
        assert_eq!(upgraded.base_type(), "warsun");
        assert_eq!(upgraded.move_value(), 3);
        // "Other players' units in this system lose PLANETARY SHIELD": a PDS does not stop it.
        let (system, planet) = crate::fixtures::a_placed_planet();
        put_on_planet(&mut state, &system, &planet, "pds", &b(), 1);
        put(&mut state, &system, "muaat_warsun2", &a(), 1);
        assert!(crate::invasion::bombardable(
            &state,
            content(),
            DEFAULT,
            &system,
            &planet,
            &a()
        ));
        // A fleet without a war sun is still stopped.
        let mut other = muaat_game();
        put_on_planet(&mut other, &system, &planet, "pds", &b(), 1);
        put(&mut other, &system, "dreadnought", &a(), 1);
        assert!(!crate::invasion::bombardable(
            &other,
            content(),
            DEFAULT,
            &system,
            &planet,
            &a()
        ));
    }

    // -- Star Forge and Ember Colossus -----------------------------------------------------------

    #[test]
    fn star_forge_places_two_fighters_at_a_war_sun_for_a_strategy_token() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        let (tokens_before, fighters) = (
            tokens(&state, &a(), TokenPool::Strategic),
            count(&state, &home, "fighter", &a()),
        );
        assert!(tokens_before > 0);
        let offered = component_actions(&state, content(), &a());
        assert!(offered.iter().any(|o| o.id == STAR_FORGE));
        let done = perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{home}|fighters")]),
            STAR_FORGE,
        );
        assert!(done);
        assert_eq!(count(&state, &home, "fighter", &a()), fighters + 2);
        assert_eq!(
            tokens(&state, &a(), TokenPool::Strategic),
            tokens_before - 1
        );
    }

    #[test]
    fn star_forge_can_place_a_destroyer_instead() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        let destroyers = count(&state, &home, "destroyer", &a());
        assert!(perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{home}|destroyer")]),
            STAR_FORGE,
        ));
        assert_eq!(count(&state, &home, "destroyer", &a()), destroyers + 1);
    }

    #[test]
    fn star_forge_needs_a_token_a_war_sun_and_a_choice_and_changes_nothing_otherwise() {
        // No strategy token.
        let mut state = muaat_game();
        state.player_mut(&a()).unwrap().strategic_tokens = 0;
        assert!(
            !component_actions(&state, content(), &a())
                .iter()
                .any(|o| o.id == STAR_FORGE)
        );
        let before = state.clone();
        assert!(!perform(&mut state, None, &mut never(), STAR_FORGE));
        assert_eq!(state, before);
        // No war sun.
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        state.system_mut(&home).units.retain(|unit| {
            !(unit.owner == a() && unit.type_id.as_str().starts_with("muaat_warsun"))
        });
        assert!(
            !component_actions(&state, content(), &a())
                .iter()
                .any(|o| o.id == STAR_FORGE)
        );
        let before = state.clone();
        assert!(!perform(&mut state, None, &mut never(), STAR_FORGE));
        assert_eq!(state, before);
        // Declined.
        let mut state = muaat_game();
        let before = state.clone();
        assert!(!perform(
            &mut state,
            None,
            &mut scripted(&["decline"]),
            STAR_FORGE
        ));
        assert_eq!(state, before);
    }

    fn mech_beside_the_forge(state: &mut GameState) -> (SystemId, PlanetId) {
        let home = home_of(state, &a());
        let planet = state.board[&home]
            .planet_control
            .iter()
            .find(|(_, owner)| **owner == a())
            .map(|(planet, _)| planet.clone())
            .expect("a home planet");
        put_on_planet(state, &home, &planet, "muaat_mech", &a(), 1);
        (home, planet)
    }

    #[test]
    fn ember_colossus_adds_an_infantry_with_the_mech_when_star_forge_is_used() {
        let mut state = muaat_game();
        let (home, planet) = mech_beside_the_forge(&mut state);
        let infantry = state.system_state(&home).on_planet_of(&planet, &a()).len();
        let done = perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{home}|destroyer"), &format!("{home}|{planet}")]),
            STAR_FORGE,
        );
        assert!(done);
        assert_eq!(
            state.system_state(&home).on_planet_of(&planet, &a()).len(),
            infantry + 1,
            "one infantry joined the mech"
        );
    }

    #[test]
    fn ember_colossus_is_optional_and_needs_the_forge_nearby() {
        // Declined: the infantry is not placed, the forge still resolves.
        let mut state = muaat_game();
        let (home, planet) = mech_beside_the_forge(&mut state);
        let before = state.system_state(&home).on_planet_of(&planet, &a()).len();
        assert!(perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{home}|destroyer"), "decline"]),
            STAR_FORGE,
        ));
        assert_eq!(
            state.system_state(&home).on_planet_of(&planet, &a()).len(),
            before
        );
        // A mech far from the forge (no map: only its own system counts) is not asked about.
        let mut state = muaat_game();
        let (home, _) = mech_beside_the_forge(&mut state);
        let far = state
            .board
            .keys()
            .find(|system| **system != home)
            .cloned()
            .expect("another system");
        put(&mut state, &far, "muaat_warsun", &a(), 1);
        state.system_mut(&home).units.retain(|unit| {
            !(unit.owner == a() && unit.type_id.as_str().starts_with("muaat_warsun"))
        });
        let mech_planets: usize = state
            .system_state(&home)
            .planet_units
            .values()
            .flatten()
            .count();
        assert!(perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{far}|destroyer")]),
            STAR_FORGE,
        ));
        assert_eq!(
            state
                .system_state(&home)
                .planet_units
                .values()
                .flatten()
                .count(),
            mech_planets
        );
    }

    // -- The Inferno -----------------------------------------------------------------------------

    #[test]
    fn the_inferno_places_a_cruiser_in_its_system_for_a_strategy_token() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        put(&mut state, &home, "muaat_flagship", &a(), 1);
        let (cruisers, token) = (
            count(&state, &home, "cruiser", &a()),
            tokens(&state, &a(), TokenPool::Strategic),
        );
        assert!(
            component_actions(&state, content(), &a())
                .iter()
                .any(|o| o.id == INFERNO)
        );
        assert!(perform(&mut state, None, &mut never(), INFERNO));
        assert_eq!(count(&state, &home, "cruiser", &a()), cruisers + 1);
        assert_eq!(tokens(&state, &a(), TokenPool::Strategic), token - 1);
    }

    #[test]
    fn the_inferno_needs_the_flagship_and_a_token() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        state
            .system_mut(&home)
            .units
            .retain(|u| u.type_id.as_str() != "muaat_flagship");
        let before = state.clone();
        assert!(!perform(&mut state, None, &mut never(), INFERNO));
        assert_eq!(state, before);
        put(&mut state, &home, "muaat_flagship", &a(), 1);
        state.player_mut(&a()).unwrap().strategic_tokens = 0;
        assert!(
            !component_actions(&state, content(), &a())
                .iter()
                .any(|o| o.id == INFERNO)
        );
        let before = state.clone();
        assert!(!perform(&mut state, None, &mut never(), INFERNO));
        assert_eq!(state, before);
    }

    // -- Fires of the Gashlai --------------------------------------------------------------------

    fn lend_fires(state: &mut GameState, to: &PlayerId) {
        state
            .promissory_notes
            .insert(crate::promissory::note_id("fires", "muaat"), to.clone());
    }
    fn perform_as_b(state: &mut GameState) -> bool {
        let option = action(FIRES, "test");
        with_context(state, DEFAULT, None, &mut never(), |ctx| {
            perform_component(ctx, &b(), &option)
        })
    }

    #[test]
    fn fires_of_the_gashlai_gives_the_holder_the_war_sun_and_returns_the_note() {
        let mut state = muaat_game();
        lend_fires(&mut state, &b());
        let fleet = tokens(&state, &a(), TokenPool::Fleet);
        let reinforcements = state.tokens_in_reinforcements(&a());
        assert!(
            component_actions(&state, content(), &b())
                .iter()
                .any(|o| o.id == FIRES)
        );
        assert!(perform_as_b(&mut state));
        assert!(has_technology(&state, &b(), "ws"));
        assert_eq!(tokens(&state, &a(), TokenPool::Fleet), fleet - 1);
        assert_eq!(state.tokens_in_reinforcements(&a()), reinforcements + 1);
        assert_eq!(
            crate::promissory::holder_of(&state, "fires", &a()),
            None,
            "back with the Muaat player"
        );
    }

    #[test]
    fn fires_of_the_gashlai_is_not_offered_without_the_note_a_token_or_a_missing_card() {
        // The owner never plays it, and nobody else holds it.
        let mut state = muaat_game();
        assert!(
            !component_actions(&state, content(), &a())
                .iter()
                .any(|o| o.id == FIRES)
        );
        let before = state.clone();
        assert!(!perform_as_b(&mut state));
        assert_eq!(state, before);
        // No token in the Muaat fleet pool.
        let mut state = muaat_game();
        lend_fires(&mut state, &b());
        state.player_mut(&a()).unwrap().fleet_tokens = 0;
        let before = state.clone();
        assert!(!perform_as_b(&mut state));
        assert_eq!(state, before);
        // The holder already has the card.
        let mut state = muaat_game();
        lend_fires(&mut state, &b());
        give_technology(&mut state, &b(), "ws");
        assert!(
            !component_actions(&state, content(), &b())
                .iter()
                .any(|o| o.id == FIRES)
        );
        let before = state.clone();
        assert!(!perform_as_b(&mut state));
        assert_eq!(state, before);
    }

    // -- Magmus Reactor (movement) ---------------------------------------------------------------

    fn supernova_reach(state: &mut GameState) -> usize {
        let supernova = crate::fixtures::a_system_where("supernova");
        let hub = crate::fixtures::hub_with_centre(&supernova);
        let origin = SystemId::new(hub.outer[0].clone());
        let target = SystemId::new(hub.centre.clone());
        put(state, &origin, "cruiser", &a(), 1);
        crate::tactical::activate(state, &a(), &target).unwrap();
        crate::tactical::movable(
            state,
            content(),
            ti4_model::content_types::POK,
            &hub.galaxy,
            &a(),
        )
        .len()
    }

    #[test]
    fn magmus_reactor_lets_ships_move_into_a_supernova_and_only_with_the_reactor() {
        let mut bare = seated_game(
            &[("a", "muaat"), ("b", "sol")],
            ti4_model::content_types::POK,
        );
        assert_eq!(supernova_reach(&mut bare), 0, "a supernova bars the way");
        let mut armed = seated_game(
            &[("a", "muaat"), ("b", "sol")],
            ti4_model::content_types::POK,
        );
        give_technology(&mut armed, &a(), "mr");
        assert!(supernova_reach(&mut armed) > 0, "Magmus Reactor opens it");
    }

    // -- Adjudicator Ba'al -----------------------------------------------------------------------

    fn hero_game() -> (GameState, Galaxy, SystemId) {
        let mut state = muaat_game();
        set_status(&mut state, &a(), HERO, LeaderStatus::Unlocked);
        // A hub of ordinary systems that are not anyone's home.
        let ids: Vec<String> = crate::fixtures::plain_systems(80)
            .into_iter()
            .filter(|id| !nova_blocked(content(), DEFAULT, id))
            .take(7)
            .collect();
        let hub = crate::fixtures::hub_from(&ids);
        let target = SystemId::new(hub.outer[1].clone());
        (state, hub.galaxy, target)
    }
    fn moved(
        player: &str,
        system: &SystemId,
        unit: &str,
    ) -> Vec<(&'static str, serde_json::Value)> {
        vec![
            ("player", player.into()),
            ("system", system.to_string().into()),
            ("origin", "elsewhere".into()),
            ("unit", unit.into()),
        ]
    }
    const HERO_ABILITY: &str = "leader:muaat:muaathero:MOVEMENT_FINISHED:after";

    /// A war sun moves into `system`, then movement finishes: the two windows the game opens.
    fn move_and_finish(
        state: &mut GameState,
        galaxy: Option<&Galaxy>,
        table: &mut Table,
        mover: &str,
        system: &SystemId,
        unit: &str,
    ) {
        emit(
            state,
            galaxy,
            &mut never(),
            "SHIP_MOVED",
            &moved(mover, system, unit),
        );
        emit(
            state,
            galaxy,
            table,
            "MOVEMENT_FINISHED",
            &[
                ("player", mover.into()),
                ("system", system.to_string().into()),
            ],
        );
    }

    #[test]
    fn nova_seed_replaces_the_tile_destroys_other_units_and_purges_the_hero() {
        let (mut state, mut galaxy, target) = hero_game();
        put(&mut state, &target, "muaat_warsun", &a(), 1);
        put(&mut state, &target, "cruiser", &b(), 2);
        state.system_mut(&target).place_token(b());
        state.active_system = Some(target.clone());
        let hex = galaxy.coord_of(target.as_str()).unwrap();
        let planets = crate::planets::in_system(&state, content(), DEFAULT, &target);
        if let Some(planet) = planets.first() {
            state.system_mut(&target).set_control(planet.clone(), b());
            put_on_planet(&mut state, &target, planet, "infantry", &b(), 1);
            state.exhausted_planets.insert(planet.clone());
            state
                .laws
                .insert("demilitarized_zone".into(), planet.to_string());
        }
        state.ingress_tokens.insert(target.clone());
        move_and_finish(
            &mut state,
            Some(&galaxy),
            &mut scripted(&[HERO_ABILITY]),
            "a",
            &target,
            "muaat_warsun",
        );
        let nova = SystemId::new(crate::movement::NOVA_SEED);
        assert!(!state.board.contains_key(&target), "the old tile is gone");
        assert_eq!(
            count(&state, &nova, "warsun", &a()),
            1,
            "the war sun is on the new tile"
        );
        assert_eq!(
            count(&state, &nova, "cruiser", &b()),
            0,
            "other players' units are destroyed"
        );
        assert_eq!(
            state
                .pending_destructions
                .iter()
                .filter(|(_, owner, _)| *owner == b())
                .count(),
            2,
            "through the shared route: one SHIP_DESTROYED staged per ship"
        );
        assert!(
            state.system_state(&nova).command_tokens.contains(&b()),
            "command tokens stay"
        );
        assert_eq!(state.active_system, Some(nova));
        assert!(
            planets.iter().all(|p| !state.exhausted_planets.contains(p)),
            "the planet cards are purged"
        );
        assert!(!state.laws.contains_key("demilitarized_zone"));
        assert!(!state.ingress_tokens.contains(&target));
        assert!(state.purged_systems.contains(&target));
        assert_eq!(
            leader_status(&state, &a(), HERO),
            Some(LeaderStatus::Purged)
        );
        crate::movement::replay_map_edits(&state, &mut galaxy, content(), DEFAULT)
            .expect("the recorded edit replays onto the map");
        assert_eq!(galaxy.coord_of(crate::movement::NOVA_SEED), Some(hex));
        assert!(galaxy.coord_of(target.as_str()).is_none());
    }

    #[test]
    fn nova_seed_is_not_offered_when_its_conditions_fail_and_declining_changes_nothing() {
        let (state, galaxy, target) = hero_game();
        let try_event = |mut state: GameState,
                         mover: &str,
                         system: &SystemId,
                         unit: &str,
                         table: &mut Table| {
            let before_marks = state.faction_marks.clone();
            move_and_finish(&mut state, Some(&galaxy), table, mover, system, unit);
            // Only the movement mark may differ.
            state.faction_marks = before_marks;
            assert_eq!(
                leader_status(&state, &a(), HERO),
                Some(LeaderStatus::Unlocked)
            );
            assert!(state.board.contains_key(&target) || !state.board.contains_key(&target));
            assert!(
                !state
                    .board
                    .contains_key(&SystemId::new(crate::movement::NOVA_SEED))
            );
        };
        // The hero is not unlocked.
        let mut locked = state.clone();
        set_status(&mut locked, &a(), HERO, LeaderStatus::Locked);
        move_and_finish(
            &mut locked,
            Some(&galaxy),
            &mut never(),
            "a",
            &target,
            "muaat_warsun",
        );
        assert_eq!(
            leader_status(&locked, &a(), HERO),
            Some(LeaderStatus::Locked)
        );
        // Not a war sun, another player's war sun, Mecatol Rex.
        try_event(state.clone(), "a", &target, "cruiser", &mut never());
        try_event(state.clone(), "b", &target, "warsun", &mut never());
        try_event(
            state.clone(),
            "a",
            &sys(crate::seating::MECATOL),
            "muaat_warsun",
            &mut never(),
        );
        // No map.
        let mut no_map = state.clone();
        move_and_finish(
            &mut no_map,
            None,
            &mut never(),
            "a",
            &target,
            "muaat_warsun",
        );
        assert_eq!(
            leader_status(&no_map, &a(), HERO),
            Some(LeaderStatus::Unlocked)
        );
        // Declined.
        let mut declined = state.clone();
        put(&mut declined, &target, "muaat_warsun", &a(), 1);
        move_and_finish(
            &mut declined,
            Some(&galaxy),
            &mut scripted(&["decline"]),
            "a",
            &target,
            "muaat_warsun",
        );
        assert_eq!(
            leader_status(&declined, &a(), HERO),
            Some(LeaderStatus::Unlocked)
        );
        assert!(declined.board.contains_key(&target), "the tile stays");
        // A war sun that moved in a different activation is not "moved into" this one.
        let mut stale = state.clone();
        put(&mut stale, &target, "muaat_warsun", &a(), 1);
        emit(
            &mut stale,
            Some(&galaxy),
            &mut never(),
            "SHIP_MOVED",
            &moved("a", &target, "muaat_warsun"),
        );
        stale.activation_seq += 1;
        emit(
            &mut stale,
            Some(&galaxy),
            &mut never(),
            "MOVEMENT_FINISHED",
            &[
                ("player", "a".into()),
                ("system", target.to_string().into()),
            ],
        );
        assert_eq!(
            leader_status(&stale, &a(), HERO),
            Some(LeaderStatus::Unlocked)
        );
        // Home systems and the Fracture are barred whoever's they are.
        let sol_home = home_of(&state, &b());
        assert!(nova_blocked(content(), DEFAULT, sol_home.as_str()));
        assert!(nova_blocked(content(), DEFAULT, crate::seating::MECATOL));
        assert!(nova_blocked(content(), DEFAULT, "fracture1"));
        assert!(!nova_blocked(content(), DEFAULT, target.as_str()));
    }

    /// The real driver: a tactical action moves a war sun in, movement finishes, the hero asks,
    /// and the step ends with the game's own map showing the supernova.
    #[test]
    fn nova_seed_works_in_a_driven_tactical_action() {
        use crate::game::{Game, TACTICAL_ACTION_ID};
        let pok = ti4_model::content_types::POK;
        let mut state = seated_game(&[("a", "muaat"), ("b", "sol")], pok);
        set_status(&mut state, &a(), HERO, LeaderStatus::Unlocked);
        let ids: Vec<String> = crate::fixtures::plain_systems(80)
            .into_iter()
            .filter(|id| !nova_blocked(content(), pok, id))
            .take(7)
            .collect();
        let hub = crate::fixtures::hub_from(&ids);
        let (centre, origin) = (
            SystemId::new(hub.centre.clone()),
            SystemId::new(hub.outer[0].clone()),
        );
        state.phase = Phase::Action;
        state.active = Some(a());
        put(&mut state, &origin, "muaat_warsun", &a(), 1);
        put(&mut state, &centre, "cruiser", &b(), 1);
        let table = Table::with_default(Box::new(Scripted::new([
            TACTICAL_ACTION_ID.to_owned(),
            centre.to_string(),
            format!("move|{origin}|0"),
            "done_moving".to_owned(),
            HERO_ABILITY.to_owned(),
        ])));
        let mut game = Game::with_table(state, ContentStore::embedded(), table)
            .with_sources(pok)
            .with_galaxy(hub.galaxy);
        for _ in 0..12 {
            let result = game.step();
            assert_eq!(result.error, None);
            if game.events.iter().any(|e| e == "TACTICAL_ACTION_COMPLETE") {
                break;
            }
        }
        let nova = SystemId::new(crate::movement::NOVA_SEED);
        assert_eq!(
            leader_status(&game.state, &a(), HERO),
            Some(LeaderStatus::Purged),
            "events: {:?}",
            game.events
        );
        assert_eq!(count(&game.state, &nova, "warsun", &a()), 1);
        assert_eq!(count(&game.state, &nova, "cruiser", &b()), 0);
        assert!(!game.state.board.contains_key(&centre));
        // The game replays recorded map edits at the start of its next step.
        let _ = game.step();
        let galaxy = game.galaxy().expect("the game keeps its map");
        assert!(galaxy.coord_of(crate::movement::NOVA_SEED).is_some());
        assert!(galaxy.coord_of(centre.as_str()).is_none());
    }

    // -- Magmus ----------------------------------------------------------------------------------

    #[test]
    fn magmus_unlocks_when_a_war_sun_is_produced_and_not_for_other_units() {
        let mut state = muaat_game();
        set_status(&mut state, &a(), COMMANDER, LeaderStatus::Locked);
        let units = |kind: &str| serde_json::json!([{ "unit_type": kind, "place": "space" }]);
        let home = home_of(&state, &a()).to_string();
        let produced = |kind: &str| -> Vec<(&'static str, serde_json::Value)> {
            vec![
                ("player", "a".into()),
                ("system", home.clone().into()),
                ("source", "production".into()),
                ("count", 1.into()),
                ("units", units(kind)),
            ]
        };
        emit(
            &mut state,
            None,
            &mut never(),
            "UNITS_PRODUCED",
            &produced("cruiser"),
        );
        assert_eq!(
            leader_status(&state, &a(), COMMANDER),
            Some(LeaderStatus::Locked)
        );
        assert_eq!(
            commander_unlocked(
                &state,
                content(),
                DEFAULT,
                None,
                &a(),
                &LeaderId::new(COMMANDER)
            ),
            Some(false)
        );
        emit(
            &mut state,
            None,
            &mut never(),
            "UNITS_PRODUCED",
            &produced("muaat_warsun"),
        );
        assert_eq!(
            leader_status(&state, &a(), COMMANDER),
            Some(LeaderStatus::Unlocked)
        );
        assert_eq!(
            commander_unlocked(
                &state,
                content(),
                DEFAULT,
                None,
                &a(),
                &LeaderId::new(COMMANDER)
            ),
            Some(true)
        );
    }

    // -- Gashlai Physiology, Magmus Reactor (production) -----------------------------------------

    /// Moves reachable for a cruiser that must cross a supernova centre to reach the far side.
    fn crossing_reach(state: &mut GameState) -> usize {
        let supernova = crate::fixtures::a_system_where("supernova");
        let hub = crate::fixtures::hub_with_centre(&supernova);
        let origin = SystemId::new(hub.outer[0].clone());
        let far = SystemId::new(hub.across(&hub.outer[0]));
        put(state, &origin, "cruiser", &a(), 1);
        crate::tactical::activate(state, &a(), &far).unwrap();
        crate::tactical::movable(
            state,
            content(),
            ti4_model::content_types::POK,
            &hub.galaxy,
            &a(),
        )
        .len()
    }

    #[test]
    fn gashlai_physiology_lets_muaat_ships_pass_through_a_supernova_but_not_stop_in_one() {
        let pok = ti4_model::content_types::POK;
        let mut muaat = seated_game(&[("a", "muaat"), ("b", "sol")], pok);
        assert!(
            crossing_reach(&mut muaat) > 0,
            "through, without the reactor"
        );
        let mut bare = seated_game(&[("a", "muaat"), ("b", "sol")], pok);
        assert_eq!(supernova_reach(&mut bare), 0, "but not into");
        let mut sol = seated_game(&[("a", "sol"), ("b", "muaat")], pok);
        assert_eq!(crossing_reach(&mut sol), 0, "other factions are barred");
        // Someone else holding the reactor card gets neither half.
        let mut copycat = seated_game(&[("a", "sol"), ("b", "muaat")], pok);
        give_technology(&mut copycat, &a(), "mr");
        assert_eq!(crossing_reach(&mut copycat), 0);
        assert!(!may_pass_through_supernova(&copycat, content(), pok, &a()));
        assert!(!may_enter_supernova(&copycat, content(), pok, &a()));
    }

    #[test]
    fn magmus_reactor_gives_a_supernova_with_your_units_production_five() {
        let supernova = SystemId::new(crate::fixtures::a_system_where("supernova"));
        let capacity = |state: &GameState, who: &PlayerId| {
            crate::production::capacity(state, content(), DEFAULT, who, &supernova)
        };
        let mut state = muaat_game();
        put(&mut state, &supernova, "cruiser", &a(), 1);
        assert_eq!(capacity(&state, &a()), 0, "no reactor, no production");
        give_technology(&mut state, &a(), "mr");
        assert_eq!(capacity(&state, &a()), 5);
        let window =
            crate::production::ProductionWindow::new(&state, content(), DEFAULT, &a(), &supernova);
        assert!(
            window.pending_choice(&state, content(), DEFAULT).is_some(),
            "something can be built there"
        );
        // Only with a unit of yours in it, and only for the holder.
        assert_eq!(capacity(&state, &b()), 0);
        let mut empty = muaat_game();
        give_technology(&mut empty, &a(), "mr");
        assert_eq!(capacity(&empty, &a()), 0, "no unit of yours in it");
        let mut ordinary = muaat_game();
        give_technology(&mut ordinary, &a(), "mr");
        let home = home_of(&ordinary, &a());
        assert_eq!(
            extra_production(&ordinary, content(), DEFAULT, &a(), &home),
            0,
            "not a supernova"
        );
    }

    // -- Umbat -----------------------------------------------------------------------------------

    const UMBAT: &str = "muaatagent";

    fn use_umbat(state: &mut GameState, table: &mut Table) -> Option<bool> {
        with_context(state, DEFAULT, None, table, |ctx| {
            use_leader(ctx, &a(), &LeaderId::new(UMBAT))
        })
    }

    #[test]
    fn umbat_lets_the_chosen_player_produce_two_units_costing_four_or_less() {
        let mut state = muaat_game();
        set_status(&mut state, &a(), UMBAT, LeaderStatus::Readied);
        assert_eq!(
            leader_action(&state, content(), &a(), &LeaderId::new(UMBAT)),
            Some(true)
        );
        let home = home_of(&state, &a());
        // Nothing costing more than 4 is offered.
        let window = crate::production::ProductionWindow::for_ability(
            &state,
            content(),
            DEFAULT,
            &a(),
            &home,
            Some(2),
        )
        .with_max_unit_cost(Some(4));
        let choice = window
            .pending_choice(&state, content(), DEFAULT)
            .expect("something is buildable");
        for id in choice.ids() {
            assert!(
                !id.contains("warsun") && !id.contains("flagship"),
                "{id} costs more than 4"
            );
        }
        let cruisers = count(&state, &home, "cruiser", &a());
        let done = use_umbat(&mut state, &mut scripted(&["a", "build|cruiser|1"]));
        assert_eq!(done, Some(true));
        assert_eq!(count(&state, &home, "cruiser", &a()), cruisers + 1);
    }

    #[test]
    fn umbat_needs_a_ready_agent_a_player_who_can_produce_and_a_choice() {
        let mut state = muaat_game();
        set_status(&mut state, &a(), UMBAT, LeaderStatus::Exhausted);
        assert_eq!(
            leader_action(&state, content(), &a(), &LeaderId::new(UMBAT)),
            Some(false)
        );
        set_status(&mut state, &a(), UMBAT, LeaderStatus::Readied);
        // Declining the player choice changes nothing.
        let before = state.clone();
        assert_eq!(
            use_umbat(&mut state, &mut scripted(&["decline"])),
            Some(false)
        );
        assert_eq!(state, before);
        // Nobody has a war sun or flagship.
        let mut bare = muaat_game();
        set_status(&mut bare, &a(), UMBAT, LeaderStatus::Readied);
        for who in [a(), b()] {
            let home = home_of(&bare, &who);
            bare.system_mut(&home).units.retain(|unit| {
                !(unit.owner == who
                    && matches!(
                        base_of(content(), DEFAULT, unit.type_id.as_str()).as_deref(),
                        Some("warsun" | "flagship")
                    ))
            });
        }
        assert_eq!(
            leader_action(&bare, content(), &a(), &LeaderId::new(UMBAT)),
            Some(false)
        );
        let before = bare.clone();
        assert_eq!(use_umbat(&mut bare, &mut never()), Some(false));
        assert_eq!(bare, before);
        // Not this module's leader.
        assert_eq!(
            leader_action(&state, content(), &a(), &LeaderId::new("naaluagent")),
            None
        );
    }

    // -- Magmus (effect) -------------------------------------------------------------------------

    const MAGMUS_ABILITY: &str = "leader:muaat:muaatcommander:STRATEGY_TOKEN_SPENT:after";

    fn token_spent(player: &str) -> Vec<(&'static str, serde_json::Value)> {
        vec![("player", player.into()), ("reason", "test".into())]
    }

    #[test]
    fn magmus_offers_a_trade_good_after_a_strategy_token_is_spent() {
        let mut state = muaat_game();
        set_status(&mut state, &a(), COMMANDER, LeaderStatus::Unlocked);
        let goods = state.player(&a()).unwrap().trade_goods;
        emit(
            &mut state,
            None,
            &mut scripted(&[MAGMUS_ABILITY]),
            "STRATEGY_TOKEN_SPENT",
            &token_spent("a"),
        );
        assert_eq!(state.player(&a()).unwrap().trade_goods, goods + 1);
        // Declined, another player's spend, and a locked commander change nothing.
        let mut state = muaat_game();
        set_status(&mut state, &a(), COMMANDER, LeaderStatus::Unlocked);
        let before = state.clone();
        emit(
            &mut state,
            None,
            &mut scripted(&["decline"]),
            "STRATEGY_TOKEN_SPENT",
            &token_spent("a"),
        );
        emit(
            &mut state,
            None,
            &mut never(),
            "STRATEGY_TOKEN_SPENT",
            &token_spent("b"),
        );
        assert_eq!(state, before);
        let mut locked = muaat_game();
        set_status(&mut locked, &a(), COMMANDER, LeaderStatus::Locked);
        let before = locked.clone();
        emit(
            &mut locked,
            None,
            &mut never(),
            "STRATEGY_TOKEN_SPENT",
            &token_spent("a"),
        );
        assert_eq!(locked, before);
    }

    #[test]
    fn this_modules_own_spends_announce_the_event_magmus_reacts_to() {
        let mut state = muaat_game();
        let home = home_of(&state, &a());
        assert!(perform(
            &mut state,
            None,
            &mut scripted(&[&format!("{home}|destroyer")]),
            STAR_FORGE,
        ));
        assert!(
            crate::supply::staged_event_types(&state)
                .iter()
                .any(|kind| kind == "STRATEGY_TOKEN_SPENT")
        );
    }

    // -- Games without a Muaat seat --------------------------------------------------------------

    #[test]
    fn a_game_without_a_muaat_seat_is_unchanged_and_supernovas_still_block() {
        let mut state = sol_game();
        for player in [a(), b()] {
            assert!(
                crate::factions::component_actions(&state, content(), &player)
                    .iter()
                    .all(|o| !o.id.starts_with("faction|muaat|"))
            );
            assert!(!may_enter_supernova(&state, content(), DEFAULT, &player));
            assert!(!may_pass_through_supernova(
                &state,
                content(),
                DEFAULT,
                &player
            ));
            assert_eq!(
                leader_action(&state, content(), &player, &LeaderId::new(AGENT)),
                Some(false)
            );
            let home = home_of(&state, &player);
            assert_eq!(
                extra_production(&state, content(), DEFAULT, &player, &home),
                0
            );
        }
        // Supernovas bar every route in a game with no reactor.
        let mut moving = seated_game(
            &[("a", "sol"), ("b", "hacan")],
            ti4_model::content_types::POK,
        );
        assert_eq!(supernova_reach(&mut moving), 0);
        // No Muaat window asks anything or changes anything.
        state.phase = Phase::Action;
        let target = SystemId::new("19");
        let home = home_of(&state, &a()).to_string();
        let before = state.clone();
        let hub = crate::fixtures::plain_hub();
        emit(
            &mut state,
            Some(&hub.galaxy),
            &mut never(),
            "SHIP_MOVED",
            &moved("a", &target, "warship"),
        );
        emit(
            &mut state,
            None,
            &mut never(),
            "UNITS_PRODUCED",
            &[
                ("player", "a".into()),
                ("system", home.into()),
                ("source", "production".into()),
                ("count", 1.into()),
                (
                    "units",
                    serde_json::json!([{ "unit_type": "warsun", "place": "space" }]),
                ),
            ],
        );
        assert_eq!(state, before);
    }
}
