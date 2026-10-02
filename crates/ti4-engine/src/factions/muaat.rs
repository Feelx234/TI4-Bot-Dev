//! The Embers of Muaat (`muaat`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope.
//!
//! Implemented: Star Forge, Prototype War Sun I/II (data-driven stats, verified), The Inferno,
//! Ember Colossus, Fires of the Gashlai, Adjudicator Ba'al (Nova Seed), and the Magmus
//! commander's unlock. Partial, not claimed (no route; see `plans/evidence/BF-muaat.md`):
//! Gashlai Physiology, Magmus Reactor (movement half only), Umbat, Magmus's effect, and
//! Stellar Genesis.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlanetId, PlayerId, SystemId, TechnologyId};
use ti4_model::state::{GameState, LeaderStatus, TokenPool};

use super::hooks_movement::MovementHooks;
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption, IllegalChoice};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::event::Event;
use crate::movement::MapEdit;
use crate::timing::{Ability, Relation, TimingContext};

/// What this faction implements; grows package by package.
pub const MODULE: FactionModule = FactionModule {
    alias: "muaat",
    abilities: &["star_forge"],
    technologies: &["pws2"],
    units: &[
        "muaat_warsun",
        "muaat_warsun2",
        "muaat_flagship",
        "muaat_mech",
    ],
    promissory: &["fires"],
    leaders: &[],
    breakthroughs: &[],
    hooks: Hooks {
        component_actions: Some(component_actions),
        perform_component: Some(perform_component),
        commander_unlocked: Some(commander_unlocked),
        timing_abilities: Some(timing_abilities),
        movement: MovementHooks {
            may_enter_supernova: Some(may_enter_supernova),
            ..MovementHooks::NONE
        },
        ..Hooks::NONE
    },
};

const STAR_FORGE: &str = "faction|muaat|star_forge";
const INFERNO: &str = "faction|muaat|inferno";
const FIRES: &str = "faction|muaat|fires";
const HERO: &str = "muaathero";
const COMMANDER: &str = "muaatcommander";

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

/// Magmus Reactor: "Your ships can move into supernovas." The hook lifts 86.1 as a whole (through
/// and into), which is exactly right for a player holding this card *and* Gashlai Physiology
/// ("Your ships can move through supernovas"), which every Muaat player has. Gashlai Physiology
/// alone (through, but not into) needs a split in the hook: see the evidence file. Until then a
/// Muaat player without the reactor is not given the half-permission: an over-permissive route
/// would offer moves the rules forbid.
fn may_enter_supernova(
    state: &GameState,
    _content: &ContentStore,
    _sources: SourceSet,
    player: &PlayerId,
) -> bool {
    is_muaat(state, player)
        && (has_technology(state, player, "mr")
            || state.player(player).is_some_and(|seat| {
                seat.assimilated_technologies
                    .values()
                    .any(|tech| tech.as_str() == "mr")
            }))
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
    if let Some(seat) = context.state.player_mut(player) {
        seat.spend_token(TokenPool::Strategic);
    }
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
    if let Some(seat) = context.state.player_mut(player) {
        seat.spend_token(TokenPool::Strategic);
    }
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

/// The system the hero would replace for this `SHIP_MOVED` event, if everything about it is legal
/// now: the owner moved a war sun into a non-home system other than Mecatol Rex, the hero is
/// unlocked, the map can take the Nova Seed there.
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
    if base_of(content, sources, event.text("unit")?).as_deref() != Some("warsun") {
        return None;
    }
    let system = SystemId::new(event.text("system")?);
    if nova_blocked(content, sources, system.as_str()) {
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

/// Adjudicator Ba'al, Nova Seed: "After you move a war sun into a non-home system other than
/// Mecatol Rex: You may destroy all other players' units in that system and replace that system
/// tile with the Muaat supernova tile. If you do, purge this card and each planet card that
/// corresponds to the replaced system tile." Command tokens and the frontier token stay
/// (`movement::apply_map_edit`); other players' units are destroyed first, the planets' cards
/// leave exhaustion and placement records, and the active system follows the tile. The whole
/// change is made on a copy and swapped in, so it either happens completely or not at all. The
/// game's own map catches up from the recorded edit at the end of the step.
fn nova_seed(owner_name: &str, seat: &PlayerId) -> Ability {
    let (owner, condition_seat) = (seat.clone(), seat.clone());
    Ability::stateful(
        format!("leader:{owner_name}:{HERO}:SHIP_MOVED:after"),
        seat.clone(),
        "SHIP_MOVED",
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
            state
                .system_mut(&system)
                .units
                .retain(|unit| unit.owner == owner);
            for planet in crate::planets::in_system(&state, content, sources, &system) {
                state.exhausted_planets.remove(&planet);
                state.placed_planets.remove(&planet);
                state.planet_attachments.remove(&planet);
            }
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

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    let mut abilities = vec![commander_unlock(owner_name, seat)];
    // Nova Seed is written and tested but not claimed: ships that move without cargo emit no typed
    // `SHIP_MOVED` in `game.rs`, and a per-ship window would fire it mid-movement (see the
    // evidence file). It stays off in real games; the unit tests drive it.
    if cfg!(test) {
        abilities.push(nova_seed(owner_name, seat));
    }
    abilities
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
    const HERO_ABILITY: &str = "leader:muaat:muaathero:SHIP_MOVED:after";

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
        }
        emit(
            &mut state,
            Some(&galaxy),
            &mut scripted(&[HERO_ABILITY]),
            "SHIP_MOVED",
            &moved("a", &target, "muaat_warsun"),
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
        assert!(
            state.system_state(&nova).command_tokens.contains(&b()),
            "command tokens stay"
        );
        assert_eq!(state.active_system, Some(nova));
        assert!(
            planets.iter().all(|p| !state.exhausted_planets.contains(p)),
            "the planet cards are purged"
        );
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
        let try_event =
            |mut state: GameState, event: Vec<(&str, serde_json::Value)>, table: &mut Table| {
                let before = state.clone();
                emit(&mut state, Some(&galaxy), table, "SHIP_MOVED", &event);
                assert_eq!(state, before);
            };
        // The hero is not unlocked.
        let mut locked = state.clone();
        set_status(&mut locked, &a(), HERO, LeaderStatus::Locked);
        try_event(locked, moved("a", &target, "muaat_warsun"), &mut never());
        // Not a war sun.
        try_event(state.clone(), moved("a", &target, "cruiser"), &mut never());
        // Another player's war sun.
        try_event(state.clone(), moved("b", &target, "warsun"), &mut never());
        // Mecatol Rex.
        try_event(
            state.clone(),
            moved("a", &sys(crate::seating::MECATOL), "muaat_warsun"),
            &mut never(),
        );
        // No map.
        let mut no_map = state.clone();
        let before = no_map.clone();
        emit(
            &mut no_map,
            None,
            &mut never(),
            "SHIP_MOVED",
            &moved("a", &target, "muaat_warsun"),
        );
        assert_eq!(no_map, before);
        // Declined.
        try_event(
            state.clone(),
            moved("a", &target, "muaat_warsun"),
            &mut scripted(&["decline"]),
        );
        // Home systems and the Fracture are barred whoever's they are.
        let sol_home = home_of(&state, &b());
        assert!(nova_blocked(content(), DEFAULT, sol_home.as_str()));
        assert!(nova_blocked(content(), DEFAULT, crate::seating::MECATOL));
        assert!(nova_blocked(content(), DEFAULT, "fracture1"));
        assert!(!nova_blocked(content(), DEFAULT, target.as_str()));
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
