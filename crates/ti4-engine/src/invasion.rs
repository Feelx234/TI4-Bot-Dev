//! Invasion (LRR 49), with ground combat under LRR 42.
//!
//! Ported from the oracle's `engine/invasion.py`: `_bombardment`, `_bombardable`,
//! `_commit_ground_forces`, `_ground_combat`, `_roll_ground` and `_establish_control`.
//!
//! Choices are asked inline through a [`Table`], matching `combat.rs`.

use ti4_content::ContentStore;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::{Feat, FeatOccurrence, GameState, InvasionDie, InvasionStep};
use ti4_model::units::Unit;

use crate::choice::{
    Choice, ChoiceOption, DECLINE_KIND, IllegalChoice, Observed, Resolving, Table, Window,
};
use crate::combat::MAX_ROUNDS;
use crate::decision_context::{DecisionContext, DecisionSource, DecisionTarget};
use crate::dice::Dice;
use crate::preview::{Delta, Preview, Quantity, deterministic};
use crate::production::Spend;
use crate::rng::GameRng;

/// The choice kind for committing a ground force to a planet (the oracle's `commit`).
pub const COMMIT_KIND: &str = "commit";
/// The choice kind for choosing which of your own ground forces dies.
pub const GROUND_CASUALTY_KIND: &str = "ground_casualty";

/// What an invasion did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvasionReport {
    /// Planets explored on being taken from nobody (35.1), with what the card did.
    pub explored: Vec<(PlanetId, crate::exploration::Explored)>,
    /// Planets ground forces were committed to.
    pub committed: Vec<PlanetId>,
    /// Planets whose control changed, with who held them before.
    pub captured: Vec<(PlanetId, Option<PlayerId>)>,
    /// Ground forces destroyed by bombardment.
    pub bombardment_kills: usize,
    /// Whether this invasion lifted the custodians token from Mecatol Rex (27.3).
    pub custodians_removed: bool,
    /// Planets where a defending Deepwrought chose to coexist (Research Team, on defense): no
    /// ground combat starts there and the invader is offered none.
    pub coexisted: Vec<PlanetId>,
}

/// 27.2: six influence, paid before ground forces are committed.
pub const CUSTODIANS_COST: i64 = 6;

/// What `player` pays to lift the custodians token: six influence (27.2), or nothing when a
/// faction says so (Winnu `blood_ties`, via
/// [`crate::factions::hooks_ground::GroundHooks::custodians_free`]).
#[must_use]
pub fn custodians_cost(state: &GameState, content: &ContentStore, player: &PlayerId) -> i64 {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, ti4_model::content_types::DEFAULT)?.custodians_cost(player);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::custodians_cost",
        typed(),
        custodians_cost_text(state, content, player)
    )
}

fn custodians_cost_text(state: &GameState, content: &ContentStore, player: &PlayerId) -> i64 {
    if crate::factions::hooks_ground::custodians_free(state, content, player) {
        0
    } else {
        CUSTODIANS_COST
    }
}

/// Whether this invader may lift the custodians token now (27.2).
///
/// Mecatol only, once, and only by a player who can actually pay. Until this existed there was no
/// production path in the engine that removed the token: every assignment was in a test, so the
/// agenda phase -- which 8.1 gates on the token being lifted -- never ran in a simulated game, and
/// with it went every law and every agenda victory point. In 5,881 recorded human games the
/// custodians point is the single most-scored entry.
#[must_use]
pub fn custodians_removable(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> bool {
    // 27.2a: "If a player cannot commit ground forces to land on Mecatol Rex, they cannot remove
    // the custodians token." The rule reads as a restriction and it is also a gate on a victory
    // point -- without it a seat with six influence and no army could buy the point outright and
    // then commit nothing, which is the one thing 27.2 is written to prevent.
    if !custodians_gate(state, content, sources, player, system) {
        return false;
    }
    let cost = custodians_cost(state, content, player);
    cost == 0
        || crate::production::available(
            state,
            content,
            sources,
            player,
            crate::production::Spend::Influence,
        ) >= cost
}

/// The board-reading gate of [`custodians_removable`]: the token is still down, this is Mecatol Rex and the
/// player has ground forces to land.
fn custodians_gate(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &SystemId) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.custodians_gate(player, system);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::custodians_gate",
        typed(),
        !state.custodians_removed
            && crate::seating::is_mecatol(system.as_str())
            && !landable(state, content, sources, player, system).is_empty()
    )
}

/// 15.1f: Planetary Shield makes a planet immune to bombardment entirely.
///
/// A war sun ignores it — which is most of what a war sun is for, so leaving it out would make
/// the unit strictly worse than the rules give.
#[must_use]
pub fn bombardable(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    invader: &PlayerId,
) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.bombardable(state, system, planet, invader);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::bombardable",
        typed(),
        bombardable_text(state, content, sources, system, planet, invader)
    )
}

fn bombardable_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    invader: &PlayerId,
) -> bool {
    // Conventions of War outranks every reason bombardment would otherwise be allowed, war sun
    // included: the law says it cannot be used against units on a cultural planet at all.
    if crate::laws::bombardment_forbidden(state, content, sources, planet) {
        return false;
    }
    let types = catalogue(content, sources);
    let board = state.system_state(system);
    let has_warsun = board.units_of(invader).into_iter().any(|unit| {
        types
            .get(unit.type_id.as_str())
            .is_some_and(|kind| kind.base_type() == "warsun")
    });
    if has_warsun {
        return true;
    }
    // Arc Secundus, the Letnev flagship: "Other player's units in this system lose PLANETARY
    // SHIELD." Its own player's shields are untouched, and the invader is the only one bombarding.
    let arc_secundus = board.units_of(invader).into_iter().any(|unit| {
        crate::factions::has_text(
            state,
            invader,
            unit.type_id.as_str(),
            crate::factions::FlagshipText::Letnev,
        )
    });
    if arc_secundus {
        return true;
    }
    // L1Z1X's commander ignores a planetary shield outright, which is the whole card.
    if crate::leaders::ignores_planetary_shield(state, invader) {
        return true;
    }
    // Disable: "your opponents' PDS units lose PLANETARY SHIELD ... during this invasion."
    // The markers are keyed to the activation that owns the invasion, so only Disable cards
    // played for *this* invasion matter. A shield survives only while every Disable in play
    // belongs to the shield's own owner — an opponent's copy strips it.
    let disabled_holders: Vec<PlayerId> = state
        .players
        .iter()
        .filter(|seat| seat.disable_invasion.contains(&state.activation_seq))
        .map(|seat| seat.id.clone())
        .collect();
    !board.on_planet(planet).iter().any(|unit| {
        if !types
            .get(unit.type_id.as_str())
            .is_some_and(UnitType::planetary_shield)
        {
            return false;
        }
        // Quietus: a unit in an active breach beside another player's Quietus has lost every unit
        // ability, PLANETARY SHIELD among them.
        if crate::factions::crimson::abilities_lost(state, &unit.owner, system) {
            return false;
        }
        !disabled_holders.iter().any(|holder| holder != &unit.owner)
    })
}

/// 49.1: the invader's bombarding ships fire at ground forces on the planets below.
///
/// Returns how many ground forces were destroyed. On a coexisting planet the invader must
/// choose, per bombarding unit, whose units take the hits (coexistence 7, 7.1), so a decider
/// is threaded in.
///
/// # Errors
///
/// [`IllegalChoice`] when the decider answers with something not offered.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
pub fn bombardment(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    table: &mut Table,
    system: &SystemId,
    invader: &PlayerId,
) -> Result<usize, IllegalChoice> {
    let occurrence = state.begin_feat_occurrence();
    let plan = roll_bombard_plan(state, content, sources, dice, rng, system, invader);
    apply_bombard_plan(
        state, content, sources, table, system, invader, &plan, occurrence,
    )
    .map(|(killed, _)| killed)
}

/// What was rolled on one planet: each bombarding unit's hit total (roll order) and the
/// ground forces that stood there before the bombardment, so "the last ground force on a
/// planet" (Make an Example of Their World) can be judged after the hits are assigned.
#[derive(Debug, Clone)]
struct BombardPlan {
    planet: PlanetId,
    before: Vec<Unit>,
    dice: Vec<InvasionDie>,
    groups: Vec<usize>,
    held: usize,
    victims: std::collections::BTreeSet<PlayerId>,
}

/// A unit's ground-combat threshold, with the faction shift applied.
///
/// Jol-Nar's Fragile is "-1 to all combat rolls" and Sardakk's Unrelenting is "+1" -- neither says
/// "in space". `combat_modifier` has carried a `context` parameter since it was written, and its
/// only caller passed "space", so ground combat rolled the printed value and Jol-Nar infantry
/// fought at full strength everywhere.
///
/// A shift applies to the *roll*, so it moves the threshold the other way, exactly as the space
/// path does.
#[must_use]
pub fn ground_combat_value(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    unit_type: &str,
) -> Option<i64> {
    #[cfg(feature = "typed-core")]
    let typed = || {
        crate::core_invasion::InvasionView::of(state, content, sources)?
            .ground_combat_value(player, system, planet, unit_type)
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::ground_combat_value",
        typed(),
        ground_combat_value_text(state, content, sources, player, system, planet, unit_type)
    )
}

fn ground_combat_value_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    unit_type: &str,
) -> Option<i64> {
    let printed = catalogue(content, sources)
        .get(unit_type)
        .and_then(ti4_content::units::UnitType::combat_hits_on)?;
    let shift =
        crate::combat_effects::ground_roll_shift(state, content, sources, player, system, planet, unit_type);
    Some(printed - shift)
}

pub(crate) fn is_ground_force_here(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    unit: &Unit,
) -> bool {
    catalogue(content, sources)
        .get(unit.type_id.as_str())
        .is_some_and(UnitType::is_ground_force)
        || crate::factions::hooks_ground::temporary_ground_force(
            state,
            content,
            sources,
            &unit.owner,
            system,
            planet,
            unit,
        )
}

/// Whether a Jol-Nar Shield Paling mech is on this planet, shielding its owner's infantry.
pub(crate) fn shield_paling(
    state: &GameState,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> bool {
    state
        .system_state(system)
        .on_planet_of(planet, player)
        .into_iter()
        .any(|unit| unit.type_id.as_str() == "jolnar_mech")
}

/// Letnev's Dunlain Reaper mech, offered at the start of each ground-combat round.
///
/// > DEPLOY: At the start of a round of ground combat, you may spend 2 resources to replace 1 of
/// > your infantry in that combat with 1 mech from your reinforcements.
///
/// 20.3 -- a DEPLOY ability places a unit from reinforcements -- so the swap is gated on supply,
/// and 20.5 makes it once per timing window, which one round is. Offered only to a player who
/// actually has an infantry in this combat, since that is what the mech replaces.
fn dunlain_reaper(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) {
    const MECH: &str = "letnev_mech";
    const COST: i64 = 2;
    let holds_mech_unit = state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == "letnev");
    if !holds_mech_unit {
        return;
    }
    let Some(infantry) = state
        .system_state(system)
        .on_planet_of(planet, player)
        .into_iter()
        .find(|unit| unit.type_id.as_str().contains("infantry"))
        .cloned()
    else {
        return; // nothing to replace
    };
    // 31.4 and 20.4: no mech in reinforcements, no deploy.
    if crate::supply::allowed(
        state,
        ctx.content,
        ctx.sources,
        player,
        &ti4_model::id::UnitTypeId::new(MECH),
        1,
    ) == 0
    {
        return;
    }
    if crate::production::available(
        state,
        ctx.content,
        ctx.sources,
        player,
        crate::production::Spend::Resources,
    ) < COST
    {
        return; // 22.3: not offered when it cannot be paid for
    }

    let choice = crate::choice::Choice::new(
        player.clone(),
        format!("Dunlain Reaper: spend 2 resources to replace an infantry on {planet}"),
        vec![
            crate::choice::ChoiceOption::labelled(
                "deploy".to_owned(),
                "unit",
                "deploy the mech".to_owned(),
            ),
            crate::choice::ChoiceOption::decline(),
        ],
    )
    .contextualized(
        DecisionContext::new(
            player.clone(),
            DecisionSource::Content("dunlain_reaper".to_owned()),
            "deploy_mech",
            state.phase,
            state.round,
        )
        .about(DecisionTarget::Planet {
            system: system.clone(),
            planet: planet.clone(),
        }),
    );
    let Ok(answer) = ctx.ask_seeing(state, &choice) else {
        return;
    };
    if answer.is_decline() {
        return;
    }
    if !crate::production::pay(
        state,
        ctx.content,
        ctx.sources,
        ctx.table,
        player,
        COST,
        crate::production::Spend::Resources,
    )
    .unwrap_or(false)
    {
        return;
    }
    if let Some(here) = state.board.get_mut(system)
        && let Some(stack) = here.planet_units.get_mut(planet)
        && let Some(at) = stack.iter().position(|unit| *unit == infantry)
    {
        stack[at] = Unit::new(ti4_model::id::UnitTypeId::new(MECH), player.clone());
    }
}

/// The invader's units that may bombard `planet` this invasion.
///
/// Ordinarily the ships in the space area (49.1). L1Z1X's Anihilator mech is the exception:
/// "While not participating in ground combat, this unit can use its BOMBARDMENT ability on planets
/// in its system as if it were a ship." So a mech standing on some *other* planet joins the
/// bombardment, and one standing on the planet it would be shooting at does not -- that is what
/// participating in the ground combat means here.
fn bombarders(
    state: &GameState,
    invader: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> Vec<Unit> {
    let board = state.system_state(system);
    let mut found: Vec<Unit> = board.units_of(invader).into_iter().cloned().collect();
    for (where_it_stands, standing) in &board.planet_units {
        if where_it_stands == planet {
            continue; // it is in the ground combat for this planet, so it does not bombard it
        }
        found.extend(
            standing
                .iter()
                .filter(|unit| &unit.owner == invader && unit.type_id.as_str() == "l1z1x_mech")
                .cloned(),
        );
    }
    found
}

/// One planet's bombardment before any die is drawn: the planet, each bombarding unit's (type, hit
/// number, dice) in roll order, the other players' ground forces standing there and whose they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedBombard {
    pub planet: PlanetId,
    pub rolls: Vec<(String, i64, usize)>,
    pub held: usize,
    pub victims: std::collections::BTreeSet<PlayerId>,
}

/// The rolls the bombardment of `invader` makes in `system` (49.1), unrolled. The planning half of
/// [`roll_bombard_plan`], and the question package B2's core answers in `rules::invasion::bombard_plan`.
fn bombard_rolls(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    invader: &PlayerId,
) -> Vec<PlannedBombard> {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.bombard_plan(state, system, invader);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::bombard_plan",
        typed(),
        bombard_rolls_text(state, content, sources, system, invader)
    )
}

fn bombard_rolls_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    invader: &PlayerId,
) -> Vec<PlannedBombard> {
    // Entropic scars rules 2 and 4: bombardment is a unit ability, so it cannot be used by
    // ships in a scar, nor against ground forces in one. Both directions collapse to the same
    // system here, since bombardment fires from the active system onto planets in it.
    if !crate::entropic_scars::abilities_usable(content, sources, system, Some(system))
        || crate::factions::crimson::abilities_lost(state, invader, system)
    {
        return Vec::new();
    }
    let types = catalogue(content, sources);
    let planets: Vec<PlanetId> = state
        .system_state(system)
        .planet_units
        .keys()
        .cloned()
        .collect();

    let mut plan = Vec::new();
    // Blitz: "each of your non-fighter ships in the active system that do not have
    // BOMBARDMENT gain BOMBARDMENT 6 until the end of the invasion." The card's window
    // opened before this plan was built, so the marker, if present, applies to every roll
    // below; it is keyed to the activation that owns the invasion.
    let blitzed = state
        .player(invader)
        .is_some_and(|seat| seat.blitz_invasion.contains(&state.activation_seq));
    let mut plasma = crate::technology::plasma_scoring(state, invader);
    for planet in planets {
        if !bombardable(state, content, sources, system, &planet, invader) {
            continue;
        }
        let defenders: Vec<Unit> = state
            .system_state(system)
            .on_planet(&planet)
            .iter()
            .filter(|unit| {
                &unit.owner != invader
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_ground_force)
            })
            .cloned()
            .collect();
        if defenders.is_empty() {
            continue;
        }

        let mut rolls: Vec<(String, i64, usize)> = Vec::new();
        for unit in bombarders(state, invader, system, &planet) {
            let Some(kind) = types.get(unit.type_id.as_str()) else {
                continue;
            };
            let (value, count) = match kind.bombard_hits_on() {
                Some(value) => (value, usize::try_from(kind.bombard_dice()).unwrap_or(0)),
                // Blitz grants BOMBARDMENT 6 — one die — to a non-fighter ship with no
                // bombard value of its own.
                None if blitzed && kind.is_ship() && !kind.is_fighter() => (6, 1),
                None => continue,
            };
            if count == 0 {
                continue;
            }
            // Plasma Scoring: one bombarding unit rolls a die more, once for the whole bombardment.
            let count = count + usize::from(std::mem::take(&mut plasma));
            // A galvanized unit rolls 1 additional die for its unit abilities (Last Bastion).
            let count = count + crate::combat_effects::galvanized_dice(&unit);
            // Bunker: "during this invasion, apply -4 to the result of each BOMBARDMENT roll
            // against planets you control." The window that hosts these rolls is opened after
            // the driver's invasion events, so the marker is in place by the time the rolls
            // are made. One entry per copy, so two Bunkers on the same planet give -8.
            let bunker_penalty = state
                .system_state(system)
                .planet_control
                .get(&planet)
                .and_then(|controller| state.player(controller))
                .map_or(0, |seat| {
                    4 * i64::try_from(
                        seat.bunker_invasion
                            .iter()
                            .filter(|seq| **seq == state.activation_seq)
                            .count(),
                    )
                    .unwrap_or(i64::MAX)
                });
            // Assail: +1 to the invader's unit ability rolls against a puppeted player.
            let assail = crate::factions::firmament_plots::assail_against(
                state,
                invader,
                defenders.iter().map(|unit| &unit.owner),
            );
            let value = (value + bunker_penalty - assail).max(1);
            rolls.push((unit.type_id.to_string(), value, count));
        }
        plan.push(PlannedBombard {
            planet,
            rolls,
            held: defenders.len(),
            victims: defenders.iter().map(|unit| unit.owner.clone()).collect(),
        });
    }
    plan
}

/// The stream positions to check the bombardment dice against, only when the invasion area is on `Dual`.
fn dual_positions(rng: &GameRng) -> Option<crate::rng::RngPositions> {
    (crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual && !rng.sync_pending())
        .then(|| rng.positions())
}

/// The `Core` backend of the bombardment's dice: the typed rules draw them from the streams standing
/// here. `None` when the string loop must roll: another backend, a pending stream restore, test faces, or
/// something the core does not map.
fn core_bombard_dice(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &Dice,
    rng: &GameRng,
    system: &SystemId,
    invader: &PlayerId,
) -> Option<(std::collections::VecDeque<crate::core_invasion::BombardDiceRow>, crate::rng::RngPositions)> {
    if crate::dual::backend(crate::dual::Area::Invasion) != crate::dual::Backend::Core || rng.sync_pending() || dice.is_preloaded() {
        return None;
    }
    #[cfg(feature = "typed-core")]
    {
        let (rows, after) = crate::core_invasion::InvasionView::of(state, content, sources)?.roll_bombard(state, rng.seed(), &rng.positions(), system, invader)?;
        Some((rows.into(), after))
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, content, sources, system, invader);
        None
    }
}

/// Dual check of the bombardment dice just rolled: the core draws the same faces per group from the stream
/// standing before them and leaves the streams where they stand. Not run with preloaded test faces.
#[allow(clippy::too_many_arguments, reason = "one parameter per distinct input")]
fn check_bombard_dice(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &Dice,
    rng: &GameRng,
    before: Option<&crate::rng::RngPositions>,
    rolled: Vec<crate::core_invasion::BombardDiceRow>,
    system: &SystemId,
    invader: &PlayerId,
) {
    let Some(before) = before else { return };
    #[cfg(feature = "typed-core")]
    {
        let typed = || {
            if dice.is_preloaded() {
                return None;
            }
            crate::core_invasion::InvasionView::of(state, content, sources)?.roll_bombard(state, rng.seed(), before, system, invader)
        };
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Invasion, "invasion::roll_bombard", typed, || (rolled, rng.positions()));
    }
    #[cfg(not(feature = "typed-core"))]
    let _ = (state, content, sources, dice, rng, before, rolled, system, invader);
}

/// Roll the invader's bombarding ships on every planet they can reach (49.1).
///
/// The dice-consuming half of the bombardment: both the invasion window and this synchronous
/// wrapper call it, so a given seed makes the same bombardment rolls no matter how the hits
/// are assigned afterwards.
fn roll_bombard_plan(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    invader: &PlayerId,
) -> Vec<BombardPlan> {
    let planned = bombard_rolls(state, content, sources, system, invader);
    // On `invasion=dual` the dice are also drawn by the core from the stream standing here (`rules::invasion::roll_bombard`).
    let positions = dual_positions(rng);
    // On `invasion=core` the core draws the dice and the string loop only records them.
    let mut drawn = core_bombard_dice(state, content, sources, dice, rng, system, invader);
    let mut dice_rows: Vec<crate::core_invasion::BombardDiceRow> = Vec::new();
    let mut plan = Vec::new();
    // Every unit's roll, kept for the reroll windows the bombardment opens (Agnlan Oln,
    // Scramble Frequency): one entry per bombarding unit, tagged with its planet.
    let mut staged = Vec::new();
    for PlannedBombard { planet, rolls, held, victims } in planned {
        let mut groups: Vec<usize> = Vec::new();
        for (unit_type, value, count) in rolls {
            let roll = if let Some((rows, _)) = drawn.as_mut() {
                let (_, _, hits_on, faces) = rows.pop_front().expect("the core plan names every roll of the string plan");
                rng.observe_randomness();
                dice.record_drawn("bombardment", faces, Some(hits_on), std::collections::BTreeSet::new(), invader)
            } else {
                dice.roll_by(
                    rng,
                    count,
                    "bombardment",
                    Some(u32::try_from(value).unwrap_or(u32::MAX)),
                    invader,
                )
            };
            dice_rows.push((
                planet.to_string(),
                unit_type.clone(),
                u32::try_from(value).unwrap_or(u32::MAX),
                roll.faces.clone(),
            ));
            let produced = roll.hits() * x89_multiplier(state, invader);
            if produced > 0 {
                groups.push(produced);
            }
            staged.push(ti4_model::state::RerollEntry {
                unit_types: std::iter::once((unit_type.clone(), 1)).collect(),
                unit: unit_type,
                planet: Some(planet.clone()),
                hits_on: Some(u32::try_from(value).unwrap_or(u32::MAX)),
                faces: roll.faces,
                rerolled: std::collections::BTreeSet::new(),
                deltas: std::collections::BTreeMap::new(),
                // The bombarding unit sits in the system's space area; `planet` names the
                // target it hits, not where the unit stands.
            });
        }
        plan.push(BombardPlan {
            before: state.system_state(system).on_planet(&planet).to_vec(),
            dice: Vec::new(),
            planet,
            groups,
            held,
            victims,
        });
    }
    if let Some((_, after)) = drawn {
        rng.set_positions(&after);
    }
    check_bombard_dice(state, content, sources, dice, rng, positions.as_ref(), dice_rows, system, invader);
    if staged.iter().any(|roll| !roll.faces.is_empty()) {
        state.reroll_staging.insert(
            invader.clone(),
            ti4_model::state::RerollSet {
                kind: "bombardment".into(),
                system: system.clone(),
                rolls: staged,
            },
        );
        state.last_reroll_player = Some(invader.clone());
    }
    plan
}

/// Destroy up to `produced` of `owner`'s ground forces on `planet` (their deterministic
/// on-planet order) and return how many fell. Coexistence 7.2: a unit's hits stop at the
/// chosen player's own units and do not spill to anyone else's.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn take_bombard_hits(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    owner: &PlayerId,
    produced: usize,
    log: &mut Vec<PendingEvent>,
) -> usize {
    #[cfg(feature = "typed-core")]
    {
        // On `invasion=dual` the hits also land through the core's hits step (`rules::invasion::hits`): the same
        // board, the same destroyed count and the same queued events.
        let queued = std::cell::RefCell::new(Vec::new());
        let destroyed = crate::dual_window!(
            crate::dual::Area::Invasion,
            "invasion::take_bombard_hits",
            state,
            |s: &mut GameState| {
                let mut own = Vec::new();
                let destroyed = take_bombard_hits_viewed(s, content, sources, system, planet, owner, produced, &mut own);
                let rows = own.iter().map(|(n, p)| ((*n).to_owned(), p.clone())).collect();
                queued.borrow_mut().extend(own);
                crate::dual::Run::new(crate::dual_window::HitsOut { remaining: produced, destroyed, log: rows })
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                let args = crate::dual_window::HitsArgs {
                    victim: owner,
                    system,
                    planet,
                    hits: produced,
                    cause: "bombardment",
                    unit_ability: true,
                    open: false,
                    apply: true,
                    announce: false,
                };
                crate::dual_window::ground_hits(s, sources, args, script)
            }
        )
        .destroyed;
        log.extend(queued.into_inner());
        return destroyed;
    }
    #[cfg(not(feature = "typed-core"))]
    take_bombard_hits_viewed(state, content, sources, system, planet, owner, produced, log)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn take_bombard_hits_viewed(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    owner: &PlayerId,
    produced: usize,
    log: &mut Vec<PendingEvent>,
) -> usize {
    #[cfg(feature = "typed-core")]
    let before = (crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual).then(|| state.clone());
    let destroyed = take_bombard_hits_text(state, content, sources, system, planet, owner, produced, log);
    #[cfg(feature = "typed-core")]
    if let Some(before) = before {
        let rows = |gs: &GameState| -> Vec<(String, String, bool, bool)> {
            gs.system_state(system)
                .on_planet(planet)
                .iter()
                .map(|u| (u.owner.to_string(), u.type_id.to_string(), u.sustained_damage, u.galvanized))
                .collect()
        };
        crate::dual::run_dual_typed(
            crate::dual::Area::Invasion,
            "invasion::take_bombard_hits",
            || crate::core_invasion::InvasionView::of(&before, content, sources)?.take_bombard_hits(system, planet, owner, produced),
            || (destroyed, rows(state)),
        );
    }
    destroyed
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn take_bombard_hits_text(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    owner: &PlayerId,
    produced: usize,
    log: &mut Vec<PendingEvent>,
) -> usize {
    let mut destroyed = 0;
    let mut remaining = produced;
    while remaining > 0 {
        remaining -= 1;
        match ground_hit_outcome_logged(
            state,
            content,
            sources,
            system,
            planet,
            owner,
            "bombardment",
            log,
            true,
        ) {
            GroundHit::Destroyed(_) => destroyed += 1,
            GroundHit::Sustained(_) => {
                remaining = remaining.saturating_sub(sustain_extra(state, owner))
            }
            GroundHit::Wasted => {}
        }
    }
    destroyed
}

/// The coexistence 7/7.1 question: whose units on the planet take this bombarding unit's
/// hits. Only players still holding ground forces there are offered.
fn bombardment_target_question(
    invader: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
    present: &std::collections::BTreeSet<PlayerId>,
    on_planet: &[Unit],
) -> Option<Choice> {
    if present.is_empty() {
        return None;
    }
    let choice = Choice::new(
        invader.clone(),
        format!("whose units on {planet} take the bombardment's next hits ({hits} hits)"),
        present
            .iter()
            .map(|player| {
                // OBS-008b5: `system`/`planet` give the option board context through the
                // existing generic pipeline. The option's own id is the targeted seat's raw
                // identity -- necessary for the engine to route the answer, but not a fact
                // `bombardment_target_features` lets reach the policy as one; it is represented
                // as an OBS-005 opponent slot instead.
                ChoiceOption::labelled(
                    player.as_str(),
                    "bombardment_target",
                    format!("{player}'s units"),
                )
                .with("system", system.to_string())
                .with("planet", planet.to_string())
            })
            .collect(),
    );
    Some(with_bombard_offer(choice, invader, system, planet, hits, present, on_planet))
}

/// The offer card of the bombardment target question (display only): what each answer would put at risk.
pub(crate) fn with_bombard_offer(
    choice: Choice,
    invader: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
    present: &std::collections::BTreeSet<PlayerId>,
    on_planet: &[Unit],
) -> Choice {
    let forces = |player: &PlayerId| -> String {
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for unit in on_planet.iter().filter(|unit| &unit.owner == player) {
            *counts.entry(unit.type_id.to_string().replace('_', " ")).or_insert(0) += 1;
        }
        counts.into_iter().map(|(unit, n)| format!("{n} {unit}")).collect::<Vec<_>>().join(", ")
    };
    let captions: Vec<(&str, serde_json::Value)> = present
        .iter()
        .map(|player| {
            (
                player.as_str(),
                crate::choice::offer_caption_for_seat(
                    "Hit their ground forces",
                    Some(&format!("{} on {planet}", forces(player))),
                    player.as_str(),
                ),
            )
        })
        .collect();
    choice    .offered(
        crate::choice::offer_card(
            "Bombardment hits",
            "bombardment",
            Some("Invasion, step 1: bombardment"),
            Some(
                "Hits only destroy ground forces (never structures); a Mech may sustain damage to cancel one. Several players have ground forces on this planet: choose whose take these hits.",
            ),
        ),
        vec![
            crate::choice::offer_fact_planet("Planet", planet.as_str(), system.as_str()),
            crate::choice::offer_fact_seat("Bombarding", invader.as_str()),
            crate::choice::offer_fact("Hits to assign", hits.to_string()),
        ],
        &captions,
    )
}

/// Assign an already-rolled bombardment synchronously, asking the decider on coexisting
/// planets (7, 7.1) and capping each unit's hits at the chosen player's forces (7.2).
///
/// Returns how many ground forces were destroyed and whether the "last ground force on a
/// planet" feat fired. The invasion window applies the same rolls through its pause-and-
/// answer state machine instead, because there the choice crosses a step boundary.
fn apply_bombard_plan(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    system: &SystemId,
    invader: &PlayerId,
    plan: &[BombardPlan],
    occurrence: FeatOccurrence,
) -> Result<(usize, bool), IllegalChoice> {
    let mut killed = 0;
    let mut noted = false;
    for entry in plan {
        // X-89: "Exhaust each planet you use BOMBARDMENT against."
        if x89_multiplier(state, invader) > 1 {
            state.exhaust_planet(entry.planet.clone());
        }
        let mut taken = 0;
        // Proxima Targeting VI: 1 hit cancelled per galvanized unit present on the planet.
        let mut groups = entry.groups.clone();
        if let [victim] = entry.victims.iter().collect::<Vec<_>>()[..] {
            crate::factions::bastion_units::cancel_in_groups(
                state,
                victim,
                system,
                &entry.planet,
                &mut groups,
            );
        }
        for produced in &groups {
            let target = if entry.victims.len() == 1 {
                entry.victims.iter().next().expect("a single owner").clone()
            } else {
                let present: std::collections::BTreeSet<PlayerId> = state
                    .system_state(system)
                    .on_planet(&entry.planet)
                    .iter()
                    .filter(|unit| &unit.owner != invader)
                    .map(|unit| unit.owner.clone())
                    .collect();
                let Some(question) = bombardment_target_question(
                    invader,
                    system,
                    &entry.planet,
                    *produced,
                    &present,
                    state.system_state(system).on_planet(&entry.planet),
                )
                .map(|question| {
                    question.contextualized(
                        DecisionContext::new(
                            invader.clone(),
                            DecisionSource::Rule("Coexistence 7.2".to_owned()),
                            "bombardment_target",
                            state.phase,
                            state.round,
                        )
                        .about(DecisionTarget::Planet {
                            system: system.clone(),
                            planet: entry.planet.clone(),
                        }),
                    )
                }) else {
                    // 7.2: nobody left with units takes the remaining hits.
                    break;
                };
                PlayerId::new(
                    table
                        .ask_seeing(
                            &question,
                            &crate::choice::Observed::new(state, content, sources, None),
                        )?
                        .id,
                )
            };
            // The synchronous path has no resolver, so the events it queues are dropped.
            taken += take_bombard_hits(
                state,
                content,
                sources,
                system,
                &entry.planet,
                &target,
                *produced,
                &mut Vec::new(),
            );
        }
        killed += taken;
        // Make an Example of Their World asks for the last ground force on a planet and asks
        // for it during this step: counted here rather than after the invasion, because
        // ground combat clears planets too and the empty planet afterwards does not say
        // which step emptied it.
        if taken == entry.held
            && entry.victims.len() == 1
            && state
                .system_state(system)
                .on_planet(&entry.planet)
                .iter()
                .all(|unit| &unit.owner == invader)
        {
            state.record_event_feat(invader, Feat::BombardedOutTheLastGroundForces, occurrence);
            noted = true;
        }
    }
    Ok((killed, noted))
}

/// Ground forces this player has in the system's space area, available to land.
#[must_use]
pub fn landable(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<Unit> {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.landable(player, system);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::landable",
        typed(),
        landable_text(state, content, sources, player, system)
    )
}

fn landable_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<Unit> {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units_of(player)
        .into_iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force)
        })
        .cloned()
        .collect()
}

/// The planets of `system` that a ground force may land on right now.
///
/// 27.1 keeps Mecatol Rex off the table while the custodians token sits there; everything else
/// in the system is landable.
pub(crate) fn landable_planets(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> Vec<PlanetId> {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.landable_planets(system);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::landable_planets",
        typed(),
        landable_planets_text(state, content, sources, system)
    )
}

fn landable_planets_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> Vec<PlanetId> {
    // F-M08-019-1 (C1): iterate the active system record's own `planets` array — canonical
    // order. Membership is identical to `galaxy::planets_in` for every system in this corpus;
    // only the file layout of planets.json differs, and choice-option order must not follow it.
    let Some(record) = content.get(
        ti4_model::content_types::ContentType::Systems,
        system.as_str(),
    ) else {
        return Vec::new();
    };
    record
        .strings("planets")
        .into_iter()
        .map(ToOwned::to_owned)
        // Planets placed onto this tile during play (Mirage). They are not in the system record --
        // that is what "placed during play" means -- so without this an invasion cannot reach one.
        // Appended rather than merged, which keeps the record's canonical order for the printed
        // planets and puts the arrival last, where it arrived.
        .chain(
            state
                .placed_planets
                .iter()
                .filter(|(_, went)| *went == system)
                .map(|(planet, _)| planet.to_string()),
        )
        // I.I.H.Q. Modernization: the Keleres "cannot lose" Custodia Vigilia, so no one lands on it.
        .filter(|name| name != "custodiavigilia")
        .filter_map(|name| {
            let name = name.as_str();
            // Scope filter mirrors planets_in: a planet outside the active source set is not on
            // this board. (Every system's planets come from one source, so this either keeps or
            // drops the whole array — never a subset.)
            content
                .get(ti4_model::content_types::ContentType::Planets, name)
                .filter(|planet| planet.in_sources(sources))
                .map(|_| PlanetId::new(name))
        })
        .filter(|planet| planet.as_str() != "mr" || state.custodians_removed)
        // Space stations rule 5: "Structures and ground forces cannot be committed to or placed on
        // a space station." They are listed in the system's `planets` array because they carry a
        // planet card, so without this they were offered to every invasion like any other planet --
        // and taking one was worth a planet *and* a system, since three of the four sit on tiles
        // whose only other planet is real and the fourth has none. 6.2% of measured opening
        // clearances depended on it. See `plans/evidence/SPACE_STATIONS_AUDIT.md`.
        .filter(|planet| !ti4_content::galaxy::is_space_station(content, planet.as_str(), sources))
        // Demilitarized Zone: units cannot land on the elected planet.
        .filter(|planet| !crate::laws::planet_is_demilitarized(state, planet))
        .collect()
}

/// Put a committed ground force on `planet`. A dual-form unit that stood in space in its ship form
/// (Naaz Eidolon, Z-Grav Eidolon) lands in its ground form: the ship form is never on a planet
/// ([`crate::fleet::flip_form`]; the card flips as the unit lands).
fn land_unit(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    unit: Unit,
) {
    let space_form = catalogue(content, sources)
        .get(unit.type_id.as_str())
        .is_some_and(UnitType::is_ship);
    let from = unit.type_id.clone();
    state
        .system_mut(system)
        .planet_units
        .entry(planet.clone())
        .or_default()
        .push(unit);
    if space_form {
        crate::fleet::flip_form(state, content, sources, player, system, Some(planet), &from);
    }
}

/// Every ground force the invader may commit now, with where each stands: the space area of the
/// active system (`None`, the ordinary source) first, then those a faction adds through
/// [`crate::factions::hooks_ground::GroundHooks::commit_candidates`]. A candidate that is not
/// really the invader's ground force on that planet is dropped. The index into the returned
/// vectors is the `n` of `commit|<n>|<planet>`, so the ordinary sources keep their ids.
fn commit_pool(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    invader: &PlayerId,
    system: &SystemId,
    already: &[crate::factions::hooks_ground::CommitOrigin],
) -> (
    Vec<Unit>,
    Vec<Option<crate::factions::hooks_ground::CommitOrigin>>,
    Vec<bool>,
) {
    let mut troops = landable(state, content, sources, invader, system);
    let mut origins = vec![None; troops.len()];
    let mut temporary = vec![false; troops.len()];
    let types = catalogue(content, sources);
    for candidate in crate::factions::hooks_ground::temporary_space_commit_candidates(
        state, content, sources, invader, system,
    ) {
        let real = candidate.owner == *invader
            && types
                .get(candidate.type_id.as_str())
                .is_some_and(UnitType::is_ship)
            && state
                .system_state(system)
                .units_of(invader)
                .into_iter()
                .any(|unit| *unit == candidate);
        if real {
            troops.push(candidate);
            origins.push(None);
            temporary.push(true);
        }
    }
    for candidate in crate::factions::hooks_ground::commit_candidates(
        state, content, sources, galaxy, invader, system, already,
    ) {
        let real = candidate.unit.owner == *invader
            && types
                .get(candidate.unit.type_id.as_str())
                .is_some_and(UnitType::is_ground_force)
            && state
                .system_state(&candidate.system)
                .on_planet_of(&candidate.planet, invader)
                .into_iter()
                .any(|unit| *unit == candidate.unit);
        if real {
            origins.push(Some((candidate.system, candidate.planet)));
            troops.push(candidate.unit);
            temporary.push(false);
        }
    }
    (troops, origins, temporary)
}

/// One option per *distinguishable* landing — unit type, sustained damage and planet — plus the
/// terminator. Two identical undamaged infantry are one move written twice, not a choice; a
/// damaged copy of the same type is its own options.
///
/// Each landing previews the exact, immediate `GroundForcesOnPlanet` count it reaches for the
/// invader on that planet (OBS-008a4) — what LRR 49.2 itself does, placing the unit down. It
/// claims nothing about the ground combat and control transfer that may follow; those are a
/// player choice and a later resolution, not this option's own consequence.
fn commit_options(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    invader: &PlayerId,
    system: &SystemId,
    troops: &[Unit],
    origins: &[Option<crate::factions::hooks_ground::CommitOrigin>],
    planets: &[PlanetId],
) -> Vec<ChoiceOption> {
    let options = commit_options_text(state, content, sources, invader, system, troops, origins, planets);
    #[cfg(feature = "typed-core")]
    if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual && origins.iter().all(Option::is_none) {
        // The typed list: the same landings over the planets whose toll the invader can pay, rendered.
        let open: Vec<PlanetId> = planets
            .iter()
            .filter(|planet| !crate::factions::keleres_units::commit_blocked(state, content, sources, invader, system, planet))
            .cloned()
            .collect();
        let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.commit_options(system, troops, &open);
        crate::dual::run_dual_typed(
            crate::dual::Area::Invasion,
            "invasion::commit_options",
            typed,
            || {
                options
                    .iter()
                    .map(|o| (o.id.clone(), o.kind.clone(), o.label.clone(), o.payload.clone()))
                    .collect::<Vec<_>>()
            },
        );
    }
    options
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn commit_options_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    invader: &PlayerId,
    system: &SystemId,
    troops: &[Unit],
    origins: &[Option<crate::factions::hooks_ground::CommitOrigin>],
    planets: &[PlanetId],
) -> Vec<ChoiceOption> {
    let board = state.system_state(system);
    let types = catalogue(content, sources);
    let mut seen = std::collections::BTreeSet::new();
    let mut options = Vec::new();
    // Omniopiares: "Other players must spend 1 influence to commit ground forces to the planet that
    // contains this unit." A planet whose toll the invader cannot pay is not offered.
    let planets: Vec<&PlanetId> = planets
        .iter()
        .filter(|planet| {
            !crate::factions::keleres_units::commit_blocked(
                state, content, sources, invader, system, planet,
            )
        })
        .collect();
    for (index, unit) in troops.iter().enumerate() {
        let origin = origins.get(index).and_then(Option::as_ref);
        for planet in planets.iter().copied() {
            // Moving a unit onto the planet it already stands on is no landing.
            if origin.is_some_and(|(from_system, from_planet)| {
                from_system == system && from_planet == planet
            }) {
                continue;
            }
            if !seen.insert((
                unit.type_id.to_string(),
                unit.sustained_damage,
                planet.to_string(),
                origin.map(|(from_system, from_planet)| format!("{from_system}/{from_planet}")),
            )) {
                continue;
            }
            let mut label = format!("land {}", unit.type_id);
            if unit.sustained_damage {
                label.push_str(" (damaged)");
            }
            let own_ground = i64::try_from(
                board
                    .on_planet_of(planet, invader)
                    .into_iter()
                    .filter(|standing| {
                        types
                            .get(standing.type_id.as_str())
                            .is_some_and(UnitType::is_ground_force)
                    })
                    .count(),
            )
            .unwrap_or(i64::MAX);
            let mut option = ChoiceOption::labelled(
                format!("commit|{index}|{planet}"),
                COMMIT_KIND,
                format!("{label} on {planet}"),
            )
            .with("planet", planet.to_string())
            .with("unit", unit.type_id.to_string())
            .with("damaged", unit.sustained_damage);
            if let Some((from_system, from_planet)) = origin {
                option = option
                    .with("from_system", from_system.to_string())
                    .with("from_planet", from_planet.to_string());
            }
            options.push(option.previewed(Preview::certain(vec![Delta::new(
                Quantity::GroundForcesOnPlanet,
                own_ground,
                own_ground + 1,
            )])));
        }
    }
    options.push(ChoiceOption::labelled(
        "done_committing",
        DECLINE_KIND,
        "commit no more ground forces",
    ));
    options
}

/// 49.2: commit ground forces from space onto planets, one at a time.
///
/// # Errors
/// [`IllegalChoice`] when a decider answers with something not offered.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
pub fn commit_ground_forces(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    invader: &PlayerId,
    system: &SystemId,
) -> Result<Vec<PlanetId>, IllegalChoice> {
    #[cfg(feature = "typed-core")]
    {
        // On `invasion=dual` the commitment also runs as a core window in lockstep: the same questions, the same
        // answers, the same board (`rules::invasion::commit`).
        let mut outcome = None;
        let failed = std::cell::Cell::new(false);
        let table = std::cell::RefCell::new(table);
        let planets = crate::dual_window!(
            crate::dual::Area::Invasion,
            "invasion::commit_ground_forces",
            state,
            |s: &mut GameState| {
                let result = commit_ground_forces_text(s, content, sources, &mut **table.borrow_mut(), invader, system);
                failed.set(result.is_err());
                let value = result.as_ref().map_or_else(|_| Vec::new(), Clone::clone);
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                crate::dual_window::commit_ground(s, sources, invader, system, script)
            }
        );
        return outcome.unwrap_or(Ok(planets));
    }
    #[cfg(not(feature = "typed-core"))]
    commit_ground_forces_text(state, content, sources, table, invader, system)
}

fn commit_ground_forces_text(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    invader: &PlayerId,
    system: &SystemId,
) -> Result<Vec<PlanetId>, IllegalChoice> {
    if crate::planets::in_system(state, content, sources, system).is_empty() {
        return Ok(Vec::new());
    }

    let mut committed: std::collections::BTreeSet<PlanetId> = std::collections::BTreeSet::new();
    loop {
        let troops = landable(state, content, sources, invader, system);
        if troops.is_empty() {
            break;
        }

        // Re-read each iteration: the custodians token can come down mid-sequence and open
        // Mecatol Rex, exactly as in the oracle.
        let planets = landable_planets(state, content, sources, system);
        let origins = vec![None; troops.len()];
        let options = commit_options(
            state, content, sources, invader, system, &troops, &origins, &planets,
        );

        let choice = Choice::new(
            invader.clone(),
            format!("commit ground forces in {system}"),
            options,
        )
        .contextualized(
            DecisionContext::new(
                invader.clone(),
                DecisionSource::Rule("49".to_owned()),
                "commit_ground_forces",
                state.phase,
                state.round,
            )
            .about(DecisionTarget::System(system.clone())),
        );
        let answer = table.ask_seeing(&choice, &Observed::new(state, content, sources, None))?;
        if answer.is_decline() {
            break;
        }
        let mut parts = answer.id.splitn(3, '|');
        let (_, index, planet) = (parts.next(), parts.next(), parts.next());
        let (Some(index), Some(planet)) = (
            index.and_then(|i| i.parse::<usize>().ok()),
            planet.map(PlanetId::new),
        ) else {
            break;
        };
        let Some(unit) = troops.get(index).cloned() else {
            break;
        };
        // Omniopiares: the first landing on the planet pays its toll (options were filtered, so
        // it can be paid).
        if !crate::factions::keleres_units::pay_commit_toll(
            state, content, sources, None, table, invader, system, &planet,
        )? {
            break;
        }
        state.system_mut(system).remove(std::slice::from_ref(&unit));
        land_unit(state, content, sources, invader, system, &planet, unit);
        committed.insert(planet);
    }
    Ok(committed.into_iter().collect())
}

/// A side's ground roll before any die is drawn: per hit number (ascending) the dice and the unit types behind
/// them. The planning half of [`roll_ground`], and the question the core answers in
/// `rules::invasion::ground_pools`.
type GroundPools = std::collections::BTreeMap<i64, (i64, std::collections::BTreeMap<String, u32>)>;

fn ground_pools(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> GroundPools {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_invasion::InvasionView::of(state, content, sources)?.ground_pools(player, system, planet);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::ground_pools",
        typed(),
        ground_pools_text(state, content, sources, player, system, planet)
    )
}

fn ground_pools_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> GroundPools {
    let types = catalogue(content, sources);
    // Ground rolls group dice by combat value, so each staged entry is one value's pool.
    // The pool's unit types ride along too: destruction rules that name "the units that
    // rerolled" need to know what sat behind each value's dice.
    let mut fighting: GroundPools = std::collections::BTreeMap::new();
    let roster: Vec<_> = state
        .system_state(system)
        .on_planet_of(planet, player)
        .into_iter()
        .filter(|unit| is_ground_force_here(state, content, sources, system, planet, unit))
        .cloned()
        .collect();
    let mut sol_die_added = false;
    for (unit_index, unit) in roster.into_iter().enumerate() {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        let Some(value) = ground_combat_value(
            state,
            content,
            sources,
            player,
            system,
            planet,
            unit.type_id.as_str(),
        ) else {
            continue;
        };
        let _ = kind;
        let slot = fighting
            .entry(value)
            .or_insert((0, std::collections::BTreeMap::new()));
        slot.0 += crate::factions::unit_dice(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet: Some(planet),
                unit_type: unit.type_id.as_str(),
                context: "ground",
            },
            kind.combat_dice(),
        );
        // A galvanized unit rolls 1 additional die for its combat rolls (Last Bastion).
        slot.0 += i64::try_from(crate::combat_effects::galvanized_dice(&unit)).unwrap_or(0);
        // The Sol agent's mark, set by `leaders::dispatch_leader` (rules version 16): one ground
        // force of the chosen type rolls 1 additional die in the round it was used.
        if !sol_die_added
            && state.rules_version >= ti4_model::state::RULES_FACTION_EFFECTS
            && state.player(player).is_some_and(|seat| {
                seat.extra_die_round == Some(state.combat_round_seq)
                    && seat.extra_die_unit.as_ref() == Some(&unit.type_id)
            })
        {
            slot.0 += 1;
            sol_die_added = true;
        }
        slot.0 += crate::factions::borrowed_round_agents::extra_die_for(
            state,
            content,
            sources,
            system,
            Some(planet),
            player,
            unit.type_id.as_str(),
            unit_index,
            state.combat_round_seq,
        );
        *slot.1.entry(unit.type_id.to_string()).or_insert(0) += 1;
    }
    fighting
}

/// Roll one side's ground forces on a planet (42.1).
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn roll_ground(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> usize {
    let fighting = ground_pools(state, content, sources, player, system, planet);
    let mut set = ti4_model::state::RerollSet {
        kind: "ground".into(),
        system: system.clone(),
        rolls: Vec::new(),
    };
    let mut hits = 0;
    for (value, (count, unit_types)) in fighting {
        let dice_count = usize::try_from(count).unwrap_or(0);
        if dice_count == 0 {
            continue;
        }
        let roll = dice.roll_by(
            rng,
            dice_count,
            "ground combat",
            Some(u32::try_from(value).unwrap_or(u32::MAX)),
            player,
        );
        hits += roll.hits();
        set.rolls.push(ti4_model::state::RerollEntry {
            unit: format!("combat value {value}"),
            planet: Some(planet.clone()),
            hits_on: Some(u32::try_from(value).unwrap_or(u32::MAX)),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types,
        });
    }
    if set.rolls.iter().any(|roll| !roll.faces.is_empty()) {
        state.reroll_staging.insert(player.clone(), set);
        state.last_reroll_player = Some(player.clone());
    }
    hits * x89_multiplier(state, player)
}

/// X-89 Bacterial Weapon (the codex printing in play): "Double the hits produced by your units'
/// BOMBARDMENT and ground combat rolls."
fn x89_multiplier(state: &GameState, player: &PlayerId) -> usize {
    if state.player(player).is_some_and(|seat| {
        seat.technologies
            .contains(ti4_model::id::text(&"x89c4"))
    }) {
        2
    } else {
        1
    }
}

/// A typed event queued by a hit, emitted once the step that produced it has finished.
type PendingEvent = (
    &'static str,
    std::collections::BTreeMap<String, serde_json::Value>,
);

/// What one hit on a player's ground forces did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GroundHit {
    /// An undamaged unit with SUSTAIN DAMAGE absorbed it.
    Sustained(Unit),
    /// This unit was destroyed.
    Destroyed(Unit),
    /// Nothing was left to take it (15.2a).
    Wasted,
}

/// Whether `unit` may use SUSTAIN DAMAGE on `planet` right now: it has the ability, is undamaged,
/// and no faction forbids it.
fn can_sustain_here(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    unit: &Unit,
    system: &SystemId,
    planet: &PlanetId,
) -> bool {
    !unit.sustained_damage
        && catalogue(content, sources)
            .get(unit.type_id.as_str())
            .is_some_and(|kind| kind.is_ground_force() && kind.sustain_damage())
        && crate::factions::hooks_ground::may_sustain(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player: &unit.owner,
                system: Some(system),
                planet: Some(planet),
                unit_type: unit.type_id.as_str(),
                context: "ground",
            },
        )
}

/// Apply one hit to `player`'s ground forces on `planet`: an undamaged unit with SUSTAIN DAMAGE
/// (a mech) absorbs it; otherwise the cheapest ground force is destroyed, a damaged unit before a
/// fresh one of the same cost. Structures never take it (KD-2).
///
/// Shared by ground combat, bombardment, Harrow and space cannon defense, so every ground hit
/// follows one rule.
fn ground_hit_eligible(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    unit: &Unit,
    unit_ability: bool,
) -> bool {
    !unit_ability
        || !crate::factions::hooks_combat::ability_hit_immune(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player: &unit.owner,
                system: Some(system),
                planet: Some(planet),
                unit_type: unit.type_id.as_str(),
                context: "ground",
            },
        )
}

fn ground_hit_plan(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    unit_ability: bool,
) -> GroundHit {
    #[cfg(feature = "typed-core")]
    let typed = || {
        let (kind, unit) = crate::core_invasion::InvasionView::of(state, content, sources)?
            .ground_hit(system, planet, player, unit_ability)?;
        Some(match (kind, unit) {
            (1, Some(u)) => GroundHit::Sustained(u),
            (2, Some(u)) => GroundHit::Destroyed(u),
            _ => GroundHit::Wasted,
        })
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Invasion,
        "invasion::ground_hit",
        typed(),
        ground_hit_plan_text(state, content, sources, system, planet, player, unit_ability)
    )
}

fn ground_hit_plan_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    unit_ability: bool,
) -> GroundHit {
    let types = catalogue(content, sources);
    let forces: Vec<Unit> = state
        .system_state(system)
        .on_planet_of(planet, player)
        .into_iter()
        .filter(|unit| is_ground_force_here(state, content, sources, system, planet, unit))
        .filter(|unit| {
            ground_hit_eligible(state, content, sources, system, planet, unit, unit_ability)
        })
        .cloned()
        .collect();
    if let Some(sturdy) = forces
        .iter()
        .find(|unit| can_sustain_here(state, content, sources, unit, system, planet))
    {
        return GroundHit::Sustained(sturdy.clone());
    }
    let cost = |unit: &Unit| types.get(unit.type_id.as_str()).map_or(0.0, UnitType::cost);
    let Some(doomed) = forces
        .iter()
        .min_by(|a, b| {
            cost(a)
                .total_cmp(&cost(b))
                .then(b.sustained_damage.cmp(&a.sustained_damage))
                .then(a.type_id.as_str().cmp(b.type_id.as_str()))
        })
        .cloned()
    else {
        return GroundHit::Wasted;
    };
    GroundHit::Destroyed(doomed)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn apply_ground_hit_with_origin(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    unit_ability: bool,
) -> GroundHit {
    // The decision (which unit takes the hit) is the core's `rules::invasion::ground_hit`; the mutation and
    // the string-only bookkeeping stay here.
    match ground_hit_plan(state, content, sources, system, planet, player, unit_ability) {
        GroundHit::Sustained(sturdy) => {
            state
                .system_mut(system)
                .replace_planet_unit(planet, &sturdy, sturdy.sustained());
            GroundHit::Sustained(sturdy)
        }
        GroundHit::Wasted => GroundHit::Wasted,
        GroundHit::Destroyed(doomed) => destroy_ground(state, system, planet, doomed),
    }
}

fn destroy_ground(state: &mut GameState, system: &SystemId, planet: &PlanetId, doomed: Unit) -> GroundHit {
    state
        .system_mut(system)
        .remove_from_planet(planet, std::slice::from_ref(&doomed));
    crate::faction_techs::note_destroyed(state, &doomed);
    GroundHit::Destroyed(doomed)
}

/// [`apply_ground_hit`] that also queues `GROUND_FORCE_SUSTAINED` / `GROUND_FORCE_DESTROYED` in
/// `log` for the caller to emit, and returns the destroyed unit (`None` if the hit was sustained or
/// wasted) (see `factions::hooks_ground` for the payloads).
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the tests assign single logged hits; production uses the outcome"
    )
)]
fn ground_hit_logged(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    cause: &'static str,
    log: &mut Vec<PendingEvent>,
) -> Option<Unit> {
    ground_hit_logged_with_origin(
        state,
        content,
        sources,
        system,
        planet,
        player,
        cause,
        log,
        matches!(cause, "bombardment" | "space_cannon_defense" | "harrow"),
    )
}

fn ground_hit_logged_with_origin(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    cause: &'static str,
    log: &mut Vec<PendingEvent>,
    unit_ability: bool,
) -> Option<Unit> {
    match ground_hit_outcome_logged(
        state,
        content,
        sources,
        system,
        planet,
        player,
        cause,
        log,
        unit_ability,
    ) {
        GroundHit::Destroyed(unit) => Some(unit),
        GroundHit::Sustained(_) | GroundHit::Wasted => None,
    }
}

/// How many further hits one use of SUSTAIN DAMAGE by `player`'s ground force cancels: one with
/// Non-Euclidean Shielding ("cancel 2 hits instead of 1"), none otherwise. Per *use*, as in
/// space, so the caller's remaining hits shrink by this much after a sustain.
fn sustain_extra(state: &GameState, player: &PlayerId) -> usize {
    usize::from(crate::combat::non_euclidean_shielding(state, player))
}

/// [`ground_hit_logged_with_origin`] that reports what the hit did.
fn ground_hit_outcome_logged(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    cause: &'static str,
    log: &mut Vec<PendingEvent>,
    unit_ability: bool,
) -> GroundHit {
    let event = |name: &'static str, unit: &Unit, damaged: Option<bool>| -> PendingEvent {
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), system.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("player".to_owned(), player.to_string().into());
        payload.insert("unit".to_owned(), unit.type_id.to_string().into());
        if let Some(damaged) = damaged {
            payload.insert("damaged".to_owned(), damaged.into());
            // The token goes back to the supply with a destroyed unit; the event keeps the fact
            // (Last Bastion). Present only when true.
            if unit.galvanized {
                payload.insert("galvanized".to_owned(), true.into());
            }
        }
        payload.insert("cause".to_owned(), cause.into());
        (name, payload)
    };
    match apply_ground_hit_with_origin(
        state,
        content,
        sources,
        system,
        planet,
        player,
        unit_ability,
    ) {
        GroundHit::Sustained(unit) => {
            log.push(event("GROUND_FORCE_SUSTAINED", &unit, None));
            GroundHit::Sustained(unit)
        }
        GroundHit::Destroyed(unit) => {
            log.push(event(
                "GROUND_FORCE_DESTROYED",
                &unit,
                Some(unit.sustained_damage),
            ));
            GroundHit::Destroyed(unit)
        }
        GroundHit::Wasted => GroundHit::Wasted,
    }
}

/// Emit queued events in order. Cancels and errors are ignored, as at every other emit site in
/// this file: the hits they report have already happened.
fn flush_events(state: &mut GameState, ctx: &mut Resolving<'_>, log: Vec<PendingEvent>) {
    for (event_type, payload) in log {
        let _ = ctx.emit(state, event_type, payload);
    }
}

/// Assign `hits` ordinary hits to `player`'s ground forces on `planet` from inside a timing
/// ability's effect, emitting the sustain/destroy events through `resolver`.
///
/// For effects that "produce a hit" against an opponent after a trigger (Sardakk `sardakk_mech`):
/// the hit follows the same rule as every other ground hit (SUSTAIN DAMAGE first, otherwise the
/// cheapest ground force). Returns how many ground forces were destroyed. `cause` is passed
/// through to the events.
///
/// # Errors
/// [`crate::timing::TimingError`] from a nested event.
#[allow(
    dead_code,
    reason = "API for faction modules; its first consumer is a wave-C faction, tests use it now"
)]
pub(crate) fn assign_ground_hits_in_timing(
    resolver: &mut crate::timing::Resolver,
    ctx: &mut crate::timing::TimingContext<'_>,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    hits: usize,
    cause: &'static str,
) -> Result<usize, crate::timing::TimingError> {
    assign_ground_hits_with_origin_in_timing(
        resolver, ctx, system, planet, player, hits, cause, false,
    )
}

/// Hits explicitly produced by printed unit text, preserving their event cause separately.
pub(crate) fn assign_ground_unit_ability_hits_in_timing(
    resolver: &mut crate::timing::Resolver,
    ctx: &mut crate::timing::TimingContext<'_>,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    hits: usize,
    cause: &'static str,
) -> Result<usize, crate::timing::TimingError> {
    assign_ground_hits_with_origin_in_timing(
        resolver, ctx, system, planet, player, hits, cause, true,
    )
}

fn assign_ground_hits_with_origin_in_timing(
    resolver: &mut crate::timing::Resolver,
    ctx: &mut crate::timing::TimingContext<'_>,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    hits: usize,
    cause: &'static str,
    unit_ability: bool,
) -> Result<usize, crate::timing::TimingError> {
    let mut destroyed = 0;
    let mut remaining = hits;
    while remaining > 0 {
        remaining -= 1;
        let mut log = Vec::new();
        match ground_hit_outcome_logged(
            ctx.state,
            ctx.content,
            ctx.sources,
            system,
            planet,
            player,
            cause,
            &mut log,
            unit_ability,
        ) {
            GroundHit::Destroyed(_) => destroyed += 1,
            GroundHit::Sustained(_) => {
                remaining = remaining.saturating_sub(sustain_extra(ctx.state, player));
            }
            GroundHit::Wasted => {}
        }
        for (event_type, payload) in log {
            let event = ctx.event_sequence.next(event_type, payload)?;
            resolver.emit_with_context(ctx, event, |_, _| {})?;
        }
    }
    Ok(destroyed)
}

/// Assign a hit to the specific ground force selected by an effect's controller.
/// The selected unit's owner retains the optional SUSTAIN DAMAGE decision (87.4a).
///
/// Returns how many of the effect's hits this assignment used up: 0 when `unit` is not a ground
/// force there, 1 for a hit that was taken or sustained, and 2 when the owner's SUSTAIN DAMAGE
/// also cancelled a second hit (Non-Euclidean Shielding). The caller skips that many.
pub(crate) fn assign_selected_ground_hit_in_timing(
    resolver: &mut crate::timing::Resolver,
    ctx: &mut crate::timing::TimingContext<'_>,
    system: &SystemId,
    planet: &PlanetId,
    unit: &Unit,
    cause: &'static str,
) -> Result<usize, crate::timing::TimingError> {
    if !ctx
        .state
        .system_state(system)
        .on_planet(planet)
        .contains(unit)
        || !is_ground_force_here(ctx.state, ctx.content, ctx.sources, system, planet, unit)
    {
        return Ok(0);
    }
    let sustain = if can_sustain_here(ctx.state, ctx.content, ctx.sources, unit, system, planet) {
        let choice = Choice::new(
            unit.owner.clone(),
            if crate::supply::staging_enabled(ctx.state) {
                format!(
                    "sustain the hit on {} on {} (system {})",
                    unit.type_id,
                    ti4_content::galaxy::planet(ctx.content, planet.as_str(), ctx.sources)
                        .and_then(|record| record.name())
                        .unwrap_or(planet.as_str()),
                    system
                )
            } else {
                format!("sustain the hit on {}", unit.type_id)
            },
            vec![
                ChoiceOption::labelled("sustain", "ground_effect_sustain", "use SUSTAIN DAMAGE"),
                ChoiceOption::decline(),
            ],
        )
        .offered(
            crate::choice::offer_card(
                "Sustain damage",
                "ground hit",
                Some("An effect has assigned a hit to this ground force"),
                Some("SUSTAIN DAMAGE cancels the hit and the unit stays, damaged. Without it the unit is destroyed."),
            ),
            vec![
                crate::choice::offer_fact_unit("Unit", unit.type_id.as_str()),
                crate::choice::offer_fact_planet("Where", planet.as_str(), system.as_str()),
            ],
            &[
                (
                    "sustain",
                    crate::choice::offer_caption(
                        "Sustain damage",
                        Some("The unit stays on the planet, damaged"),
                    ),
                ),
                (
                    "decline",
                    crate::choice::offer_caption("Let it be destroyed", Some("The unit is removed")),
                ),
            ],
        )
        .contextualized(
            DecisionContext::new(
                unit.owner.clone(),
                DecisionSource::Rule("87".to_owned()),
                "ground_effect_sustain",
                ctx.state.phase,
                ctx.state.round,
            )
            .about(DecisionTarget::Planet {
                system: system.clone(),
                planet: planet.clone(),
            }),
        );
        ctx.ask_seeing(&choice)
            .map_err(crate::timing::TimingError::IllegalChoice)?
            .id
            == "sustain"
    } else {
        false
    };
    let mut payload = std::collections::BTreeMap::from([
        ("system".to_owned(), system.to_string().into()),
        ("planet".to_owned(), planet.to_string().into()),
        ("player".to_owned(), unit.owner.to_string().into()),
        ("unit".to_owned(), unit.type_id.to_string().into()),
        ("cause".to_owned(), cause.into()),
    ]);
    let kind = if sustain {
        ctx.state
            .system_mut(system)
            .replace_planet_unit(planet, unit, unit.sustained());
        "GROUND_FORCE_SUSTAINED"
    } else {
        ctx.state
            .system_mut(system)
            .remove_from_planet(planet, std::slice::from_ref(unit));
        crate::faction_techs::note_destroyed(ctx.state, unit);
        payload.insert("damaged".to_owned(), unit.sustained_damage.into());
        if unit.galvanized {
            payload.insert("galvanized".to_owned(), true.into());
        }
        "GROUND_FORCE_DESTROYED"
    };
    let event = ctx.event_sequence.next(kind, payload)?;
    resolver.emit_with_context(ctx, event, |_, _| {})?;
    Ok(if sustain {
        1 + sustain_extra(ctx.state, &unit.owner)
    } else {
        1
    })
}

/// Remove `hits` of one player's ground forces from a planet, the owner choosing.
fn absorb_ground(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
) -> Result<(), IllegalChoice> {
    absorb_ground_with_origin(
        state, content, sources, table, player, system, planet, hits, false,
    )
}

fn absorb_ground_with_origin(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
    unit_ability: bool,
) -> Result<(), IllegalChoice> {
    #[cfg(feature = "typed-core")]
    {
        // On `invasion=dual` the assignment also runs as a core window in lockstep (`rules::invasion::absorb`).
        let mut outcome = None;
        let failed = std::cell::Cell::new(false);
        let table = std::cell::RefCell::new(table);
        crate::dual_window!(
            crate::dual::Area::Invasion,
            "invasion::absorb_ground",
            state,
            |s: &mut GameState| {
                let result = absorb_ground_text(s, content, sources, &mut **table.borrow_mut(), player, system, planet, hits, unit_ability);
                failed.set(result.is_err());
                outcome = Some(result);
                crate::dual::Run::new(())
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                crate::dual_window::absorb_ground(s, sources, player, system, planet, hits, unit_ability, script)
            }
        );
        return outcome.unwrap_or(Ok(()));
    }
    #[cfg(not(feature = "typed-core"))]
    absorb_ground_text(state, content, sources, table, player, system, planet, hits, unit_ability)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn absorb_ground_text(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    player: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
    hits: usize,
    unit_ability: bool,
) -> Result<(), IllegalChoice> {
    let mut remaining = hits;
    while remaining > 0 {
        remaining -= 1;
        // A mech's SUSTAIN DAMAGE cancels the hit before anyone chooses a casualty.
        let sturdy = state
            .system_state(system)
            .on_planet_of(planet, player)
            .into_iter()
            .filter(|unit| {
                ground_hit_eligible(state, content, sources, system, planet, unit, unit_ability)
            })
            .find(|unit| can_sustain_here(state, content, sources, unit, system, planet))
            .cloned();
        if let Some(sturdy) = sturdy {
            state
                .system_mut(system)
                .replace_planet_unit(planet, &sturdy, sturdy.sustained());
            // Non-Euclidean Shielding: the same use cancels a second hit.
            remaining = remaining.saturating_sub(sustain_extra(state, player));
            continue;
        }
        // LRR 42: only ground forces take hits in a ground combat; structures survive the
        // fight and die when control changes hands instead (KD-2).
        let present: Vec<Unit> = state
            .system_state(system)
            .on_planet_of(planet, player)
            .into_iter()
            .filter(|unit| is_ground_force_here(state, content, sources, system, planet, unit))
            .filter(|unit| {
                ground_hit_eligible(state, content, sources, system, planet, unit, unit_ability)
            })
            .cloned()
            .collect();
        if present.is_empty() {
            return Ok(()); // 15.2a
        }
        // Neutral units rule 7: nobody chooses; the hit goes to the unit lowest on the reference
        // card.
        let neutral_pick = if crate::neutral_units::is_neutral(player) {
            crate::neutral_units::next_casualty(
                &crate::neutral_units::roster(content, sources),
                &present,
                |_| true,
            )
            .cloned()
        } else {
            None
        };
        let doomed = if let Some(unit) = neutral_pick {
            unit
        } else if let [only] = present.as_slice() {
            only.clone()
        } else {
            let mut seen = std::collections::BTreeSet::new();
            let mut options = Vec::new();
            for (index, unit) in present.iter().enumerate() {
                if !seen.insert((unit.type_id.to_string(), unit.sustained_damage)) {
                    continue;
                }
                options.push(
                    ChoiceOption::labelled(
                        format!("destroy|{index}"),
                        GROUND_CASUALTY_KIND,
                        if crate::supply::staging_enabled(state) {
                            format!(
                                "destroy {} ({})",
                                unit.type_id,
                                if unit.sustained_damage {
                                    "damaged"
                                } else {
                                    "undamaged"
                                }
                            )
                        } else {
                            format!("destroy {}", unit.type_id)
                        },
                    )
                    .with("unit", unit.type_id.to_string())
                    .with("damaged", unit.sustained_damage),
                );
            }
            let choice = Choice::new(player.clone(), format!("assign a hit on {planet}"), options)
                .contextualized(
                    DecisionContext::new(
                        player.clone(),
                        DecisionSource::Rule("42".to_owned()),
                        "assign_ground_casualty",
                        state.phase,
                        state.round,
                    )
                    .about(DecisionTarget::Planet {
                        system: system.clone(),
                        planet: planet.clone(),
                    }),
                );
            let answer =
                table.ask_seeing(&choice, &Observed::new(state, content, sources, None))?;
            let index = answer
                .id
                .strip_prefix("destroy|")
                .and_then(|rest| rest.parse::<usize>().ok())
                .unwrap_or(0);
            present.get(index).unwrap_or(&present[0]).clone()
        };
        state
            .system_mut(system)
            .remove_from_planet(planet, std::slice::from_ref(&doomed));
        crate::faction_techs::note_destroyed(state, &doomed);
    }
    Ok(())
}

/// Fight a ground combat on one planet (42).
///
/// # Errors
/// [`IllegalChoice`] when a decider answers with something not offered.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
pub fn ground_combat(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    planet: &PlanetId,
    invader: &PlayerId,
) -> Result<Option<PlayerId>, IllegalChoice> {
    // LRR 42: only rival ground forces make this a fight; structures do not (KD-2).
    let defender = ground_force_owners(state, content, sources, system, planet)
        .into_iter()
        .find(|owner| owner != invader);
    let Some(defender) = defender else {
        return Ok(Some(invader.clone()));
    };

    for _ in 1..=MAX_ROUNDS {
        state.combat_round_seq = state.combat_round_seq.saturating_add(1);
        // 42.3: the fight ends when one side has no ground forces left — structures never
        // fight (KD-2).
        let owners = ground_force_owners(state, content, sources, system, planet);
        if !owners.contains(invader) || !owners.contains(&defender) {
            break;
        }
        for who in [invader, &defender] {
            crate::factions::hooks_ground::ground_combat_round_started(
                state, content, sources, table, who, system, planet,
            );
        }

        let attacker_hits =
            roll_ground(state, content, sources, dice, rng, invader, system, planet);
        let defender_hits = roll_ground(
            state, content, sources, dice, rng, &defender, system, planet,
        );
        // 42.2: simultaneous, as in space.
        absorb_ground(
            state,
            content,
            sources,
            table,
            &defender,
            system,
            planet,
            attacker_hits,
        )?;
        absorb_ground(
            state,
            content,
            sources,
            table,
            invader,
            system,
            planet,
            defender_hits,
        )?;

        // L1Z1X's Harrow bombards again at the end of each round. The hits are assigned here
        // rather than by the faction layer, because who loses a unit is the invasion's decision.
        //
        // 63.2: "The Planetary Shield ability prevents an L1Z1X player from using their Harrow
        // faction ability." Harrow *is* a bombardment, so it answers to the same gate the printed
        // BOMBARDMENT does -- including the war sun exemption and Disable, which is the reason to
        // ask `can_bombard` rather than to re-test the shield here.
        let harrow = if bombardable(state, content, sources, system, planet, invader) {
            crate::faction_abilities::ground_combat_round_ended(
                state, content, sources, dice, rng, invader, system,
            )
        } else {
            0
        };
        let harrow = crate::factions::bastion_units::cancel_bombardment_hits(
            state, &defender, system, planet, harrow,
        );
        if harrow > 0 {
            absorb_ground_with_origin(
                state, content, sources, table, &defender, system, planet, harrow, true,
            )?;
        }
    }

    let invader_left = !state
        .system_state(system)
        .on_planet_of(planet, invader)
        .is_empty();
    Ok(invader_left.then(|| invader.clone()))
}

/// 49.5: whoever has ground forces left takes the planet.
///
/// Two details that are easy to lose and both change play:
///
/// * **49.5d** — if every committed force died, the previous holder keeps the planet. Control
///   does not fall to the invader by default.
/// * A captured planet is taken **exhausted**. Its resources and influence belong to the round
///   after the one you spent conquering it; without this a planet could be spent the same turn
///   it was invaded.
pub fn establish_control(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    invader: &PlayerId,
    committed: &[PlanetId],
) -> Vec<(PlanetId, Option<PlayerId>)> {
    let types = catalogue(content, sources);
    let mut captured = Vec::new();
    for planet in committed {
        let holds = state
            .system_state(system)
            .on_planet_of(planet, invader)
            .into_iter()
            .any(|unit| {
                types
                    .get(unit.type_id.as_str())
                    .is_some_and(UnitType::is_ground_force)
            });
        if !holds {
            continue; // 49.5d
        }
        // A coexisting invader holds no control of the planet (coexistence 2, 3.1).
        if crate::coexistence::is_coexisting(state, system, planet, invader) {
            continue;
        }
        let previous = state
            .system_state(system)
            .planet_control
            .get(planet)
            .cloned();
        if previous.as_ref() == Some(invader) {
            continue; // 49.5c
        }
        state
            .system_mut(system)
            .set_control(planet.clone(), invader.clone());
        state.exhaust_planet(planet.clone());
        // Everything that reads "when you gain control of a planet" fires here, where control
        // actually changes hands: the L1Z1X breakthrough and the Minister of Exploration.
        crate::breakthroughs::on_gain_control(state, content, sources, invader, system, planet);
        crate::laws::on_gain_control(state, invader);
        // The Crown of Emphidia changes hands when a planet in its owner's home system is taken.
        if crate::laws::owner_home_system(state, content, "crown_of_emphidia").as_ref()
            == Some(system)
        {
            crate::laws::steal_throne_card(state, "crown_of_emphidia", invader);
        }
        captured.push((planet.clone(), previous));
    }
    captured
}

/// The position inside the coexistence bombardment question ([`Stage::ChoosingBombardment`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct BombardCursor {
    planet_index: usize,
    planet: PlanetId,
    groups: Vec<usize>,
    next_group: usize,
    taken: usize,
    held: usize,
    victims: std::collections::BTreeSet<PlayerId>,
    occurrence: FeatOccurrence,
}

/// The position inside "start another ground combat?" ([`Stage::ChoosingNextCombat`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct NextCombat {
    planets: Vec<PlanetId>,
    index: usize,
    planet: PlanetId,
    remaining: Vec<PlayerId>,
}

/// What the start of a ground combat round leaves for the rest of it.
struct RoundStart {
    /// A copied round agent made the round a numbered one with its own event.
    copied_round: bool,
    /// The planet's units before anything is rolled.
    before: Vec<Unit>,
}

/// Where the invasion fights next ([`InvasionWindow::next_fight`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FightPick {
    Fight { index: usize, defender: PlayerId },
    Coexisters { index: usize, remaining: Vec<PlayerId> },
    Exhausted,
}

/// What a finished ground combat round reports ([`InvasionWindow::end_ground_round`]).
struct GroundRound {
    before: Vec<Unit>,
    dice: Vec<InvasionDie>,
    attacker_hits: usize,
    defender_hits: usize,
    harrow: usize,
    log: Vec<PendingEvent>,
}

/// Where an open invasion has reached.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    /// The invader's bombarding ships have rolled (49.1); their hits are being assigned, in
    /// roll order, before any other invasion decision. Pauses on coexisting planets for the
    /// per-unit target choice (coexistence 7, 7.1).
    Bombarding,
    /// Mid-bombardment on the planet at `planet_index` of the window's bombardment plan:
    /// `groups` holds each bombarding unit's hit total, `next_group` the next unit to be
    /// assigned, `taken` the units destroyed on the planet so far.
    ChoosingBombardment(BombardCursor),
    /// Offering to lift the custodians token from Mecatol Rex (27.2).
    Custodians,
    /// Choosing which ground forces to land, and where (49.2).
    Committing,
    /// Fighting on `planets[index]`, having already resolved the earlier ones.
    Fighting {
        planets: Vec<PlanetId>,
        index: usize,
        defender: PlayerId,
    },
    /// A combat ended and any scoring window must close before the next planet/control step.
    Advancing {
        planets: Vec<PlanetId>,
        index: usize,
    },
    /// A coexistence combat ended and the winner may start another against the next coexister
    /// (coexistence 12). Declining leaves the rest coexisting.
    ChoosingNextCombat {
        planets: Vec<PlanetId>,
        index: usize,
        planet: PlanetId,
        remaining: Vec<PlayerId>,
    },
    /// A planet changed hands and its former controller's scoring window must close before
    /// gain-control effects and exploration continue.
    FinalizingControl {
        planets: Vec<PlanetId>,
        index: usize,
        planet: PlanetId,
        previous: Option<PlayerId>,
    },
    Done,
}

/// [`ground_force_owners`], reachable from a sibling module's tests.
///
/// The coexistence tests need the same answer the fight uses; exposing it is cheaper than
/// duplicating the unit-type filter and letting the copy drift from the original.
#[doc(hidden)]
#[must_use]
pub fn ground_force_owners_for_test(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
) -> std::collections::BTreeSet<PlayerId> {
    ground_force_owners(state, content, sources, system, planet)
}

/// Display only: a breakthrough as an offer-card header: its name and the first sentence of its
/// printed text (the ability that is being offered).
fn breakthrough_card(content: &ContentStore, card: &str) -> serde_json::Value {
    let record = content.get(ti4_model::content_types::ContentType::Breakthroughs, card);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key));
    let first_sentence = field("text").map(|text| match text.find(". ") {
        Some(end) => &text[..=end],
        None => text,
    });
    crate::choice::offer_card(
        field("name").unwrap_or(card),
        "breakthrough",
        None,
        first_sentence,
    )
}

/// LRR 49/42: who has ground forces on `planet`.
///
/// Only ground forces make a planet contested and can be casualties of a ground combat.
/// Structures roll no dice; a planet holding only rival structures falls without resistance,
/// and its structures are destroyed when control changes hands (KD-2). The Titans' PDS is the
/// one structure that is also a ground force, so it fights like infantry here.
fn ground_force_owners(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
) -> std::collections::BTreeSet<PlayerId> {
    state
        .system_state(system)
        .on_planet(planet)
        .iter()
        .filter(|unit| is_ground_force_here(state, content, sources, system, planet, unit))
        .map(|unit| unit.owner.clone())
        .collect()
}

/// An invasion, resolvable one decision at a time (LRR 49).
///
/// Bombardment is rolled when the window opens: 49.1 puts it before ground forces are
/// committed, so deferring even its rolls would let a player commit knowing what a
/// bombardment they had not yet suffered was going to do. The rolled hits are then assigned
/// inside the settle loop, pausing on coexisting planets for the invader's per-unit target
/// choices (coexistence 7, 7.1).
#[derive(Debug, Clone)]
pub struct InvasionWindow {
    invader: PlayerId,
    system: SystemId,
    stage: Stage,
    report: InvasionReport,
    pending_scoring_occurrences: std::collections::VecDeque<(FeatOccurrence, bool)>,
    current_ground_occurrence: Option<FeatOccurrence>,
    notes_at_tactical_start: crate::combat::NoteHoldings,
    /// The bombardment rolled when the window opened (49.1), awaiting assignment.
    bombard_plan: Vec<BombardPlan>,
    /// Next entry of `bombard_plan` to assign or finish.
    bombard_index: usize,
    bombard_occurrence: FeatOccurrence,
    /// The bombardment's reroll windows (Agnlan Oln, Scramble Frequency) have been opened,
    /// so the hits were already recomputed from the possibly rerolled dice.
    bombard_announced: bool,
    /// Planets this invasion has committed ground forces from through
    /// [`crate::factions::hooks_ground::GroundHooks::commit_candidates`], so a hook can say "up
    /// to 1 from each planet".
    extra_commits: Vec<crate::factions::hooks_ground::CommitOrigin>,
    /// Units committed through `temporary_space_commit_candidates`, by landing planet. They act
    /// as ground forces only through the combat and then return to the active system's space
    /// area before the invasion checks control.
    temporary_space_commits: std::collections::BTreeMap<PlanetId, Vec<Unit>>,
    /// Already committed hero combats leave invasion/control continuations to their caller.
    combat_only: bool,
    strict_timing_error: Option<crate::timing::TimingError>,
    /// The map, for [`crate::factions::hooks_ground::GroundHooks::commit_candidates`]. Taken from
    /// the resolver's handle the first time one is available, or set by [`Self::with_galaxy`].
    galaxy: Option<std::sync::Arc<ti4_content::galaxy::Galaxy>>,
}

impl InvasionWindow {
    /// Keep the authoritative boundary current even when a reaction replaces the invasion offer.
    pub(crate) fn update_public_boundary(&self, state: &mut GameState) {
        let Some(active) = state.active_invasion.as_mut() else {
            return;
        };
        let (phase, planet, defender) = match &self.stage {
            Stage::Bombarding => (
                "bombardment",
                self.bombard_plan
                    .get(self.bombard_index)
                    .map(|p| p.planet.clone()),
                None,
            ),
            Stage::ChoosingBombardment(BombardCursor { planet, .. }) => {
                ("bombardment", Some(planet.clone()), None)
            }
            Stage::Custodians => ("custodians", None, None),
            Stage::Committing => ("landing", None, None),
            Stage::Fighting {
                planets,
                index,
                defender,
            } => (
                "ground_battle",
                planets.get(*index).cloned(),
                Some(defender.clone()),
            ),
            Stage::Advancing { planets, index } => {
                ("planet_result", planets.get(*index).cloned(), None)
            }
            Stage::ChoosingNextCombat { planet, .. } => {
                ("next_defender", Some(planet.clone()), None)
            }
            Stage::FinalizingControl { planet, .. } => ("control", Some(planet.clone()), None),
            Stage::Done => ("complete", None, None),
        };
        if active.planet != planet || active.defender != defender {
            active.ground_round = 0;
        }
        if active
            .last_step
            .as_ref()
            .is_some_and(|step| Some(&step.planet) != planet.as_ref())
            && planet.is_some()
        {
            active.last_step = None;
        }
        active.phase = phase.to_owned();
        active.planet = planet;
        active.defender = defender;
    }

    /// Open an invasion at its "Commit Ground Forces" step, skipping bombardment (49.1), as an
    /// effect that says "skip directly to the Commit Ground Forces step" does (Sardakk hero).
    ///
    /// Rolls no dice and assigns no hits. The custodians offer (27.2) still comes first, since the
    /// engine models it as part of committing; see `plans/evidence/BF-00c-ground.md`. The caller
    /// owns whatever follows the step (the hero's purge): watch `GROUND_COMMITMENT_FINISHED`.
    #[must_use]
    pub fn at_commit_step(state: &mut GameState, invader: &PlayerId, system: &SystemId) -> Self {
        let notes = crate::combat::note_holdings(state);
        let occurrence = state.begin_feat_occurrence();
        Self {
            invader: invader.clone(),
            system: system.clone(),
            stage: Stage::Custodians,
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: notes,
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: occurrence,
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        }
    }

    /// Resolve only the combats on an already committed planet, with normal timing events.
    /// Hero effects own placement and subsequent control; no bombardment, commitment or
    /// space-cannon defence is performed here. Every rival is fought while the invader survives.
    pub(crate) fn fight_committed_planet(
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        invader: &PlayerId,
        system: &SystemId,
        planet: &PlanetId,
    ) -> Result<(), crate::timing::TimingError> {
        let mut window = Self::at_commit_step(state, invader, system);
        window.combat_only = true;
        loop {
            let owners = ground_force_owners(state, ctx.content, ctx.sources, system, planet);
            if !owners.contains(invader) {
                break;
            }
            let Some(defender) = owners.iter().find(|owner| *owner != invader).cloned() else {
                break;
            };
            let started = window.start_ground_combat(state, ctx, planet, &defender);
            if let Some(error) = window.strict_timing_error.take() {
                return Err(error);
            }
            if !started {
                continue;
            }
            window.current_ground_occurrence = Some(state.begin_feat_occurrence());
            // Finish-ground-round may stage normal invasion continuations. This isolated
            // helper owns neither control nor those continuations; it observes surviving sides.
            loop {
                let owners = ground_force_owners(state, ctx.content, ctx.sources, system, planet);
                if !owners.contains(invader) || !owners.contains(&defender) {
                    break;
                }
                window.resolve_ground_round(state, ctx, vec![planet.clone()], 0, defender.clone());
                if let Some(error) = window.strict_timing_error.take() {
                    return Err(error);
                }
                while let Some((occurrence, combat)) = window.take_scoring_occurrence() {
                    let mut scoring = crate::objectives::ScoringWindow::for_occurrence(
                        &state.initiative_order(),
                        crate::secrets::Timing::Action,
                        occurrence,
                        if combat {
                            crate::objectives::EventScoreLimit::OnePerPlayer
                        } else {
                            crate::objectives::EventScoreLimit::AnyPerPlayer
                        },
                    );
                    if let Some(galaxy) = ctx.timing.as_ref().and_then(|handle| handle.galaxy) {
                        scoring = scoring.with_galaxy(galaxy.clone());
                    }
                    while let Some(choice) = scoring.pending_choice(state, ctx.content, ctx.sources)
                    {
                        let answer = ctx
                            .table
                            .ask_seeing(
                                &choice,
                                &Observed::new(
                                    state,
                                    ctx.content,
                                    ctx.sources,
                                    ctx.timing.as_ref().and_then(|handle| handle.galaxy),
                                ),
                            )
                            .map_err(crate::timing::TimingError::IllegalChoice)?;
                        let scored = scoring
                            .resolve(state, ctx.content, ctx.sources, answer)
                            .map_err(|error| {
                                crate::timing::TimingError::IllegalChoice(
                                    IllegalChoice::DeciderFailed {
                                        player: choice.player.clone(),
                                        prompt: choice.prompt.clone(),
                                        reason: error.to_string(),
                                    },
                                )
                            })?;
                        if let Some(alias) = scored {
                            ctx.emit(
                                state,
                                &format!("OBJECTIVE_SCORED:{alias}"),
                                std::collections::BTreeMap::new(),
                            )?;
                        }
                        if state.finished {
                            ctx.emit(state, "GAME_FINISHED", std::collections::BTreeMap::new())?;
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Give the window the map, so a commit hook that needs adjacency (Sardakk's commander) can
    /// see it before the first answer. Without this the window takes the map from the first
    /// resolver handle it is driven with.
    #[must_use]
    pub fn with_galaxy(mut self, galaxy: &ti4_content::galaxy::Galaxy) -> Self {
        self.galaxy = Some(std::sync::Arc::new(galaxy.clone()));
        self
    }

    /// Open an invasion, rolling its bombardment immediately (49.1).
    #[must_use]
    pub fn new(
        state: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        dice: &mut Dice,
        rng: &mut GameRng,
        invader: &PlayerId,
        system: &SystemId,
    ) -> Self {
        let notes = crate::combat::note_holdings(state);
        Self::new_with_notes(state, content, sources, dice, rng, invader, system, notes)
    }

    /// Open an invasion with note holdings captured at the start of its tactical action.
    #[must_use]
    pub fn new_with_notes(
        state: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        dice: &mut Dice,
        rng: &mut GameRng,
        invader: &PlayerId,
        system: &SystemId,
        notes_at_tactical_start: crate::combat::NoteHoldings,
    ) -> Self {
        let occurrence = state.begin_feat_occurrence();
        // A stale set left by the synchronous APIs can only name rolls that are not this
        // bombardment's, so the window would react to the wrong dice; the roll below
        // restages the invader's own rolls.
        state.reroll_staging.clear();
        state.last_reroll_player = None;
        let bombard_plan = roll_bombard_plan(state, content, sources, dice, rng, system, invader);
        Self {
            invader: invader.clone(),
            system: system.clone(),
            stage: Stage::Bombarding,
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start,
            bombard_plan,
            bombard_index: 0,
            bombard_occurrence: occurrence,
            bombard_announced: false,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        }
    }

    #[must_use]
    pub fn take_scoring_occurrence(&mut self) -> Option<(FeatOccurrence, bool)> {
        self.pending_scoring_occurrences.pop_front()
    }

    /// Advance the bombardment as far as it can without asking anyone: assign the hits on
    /// single-owner planets inline, and pause (stopping the loop) on the first planet that
    /// needs a per-unit target choice.
    fn step_bombardment(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        log: &mut Vec<PendingEvent>,
    ) {
        let (content, sources) = (ctx.content, ctx.sources);
        loop {
            let Some(entry) = self.bombard_plan.get(self.bombard_index) else {
                self.stage = Stage::Custodians;
                return;
            };
            if entry.groups.is_empty() {
                let (planet, held, victims) =
                    (entry.planet.clone(), entry.held, entry.victims.clone());
                self.complete_bombard_plan(
                    state,
                    &planet,
                    0,
                    held,
                    &victims,
                    self.bombard_occurrence,
                );
                self.bombard_index += 1;
                continue;
            }
            // Lift the entry's data out before touching self mutably again.
            let (planet, groups, held, victims) = (
                entry.planet.clone(),
                entry.groups.clone(),
                entry.held,
                entry.victims.clone(),
            );
            if victims.len() == 1 {
                let target = victims.iter().next().expect("a single owner");
                // Proxima Targeting VI: 1 hit cancelled per galvanized unit present on the planet.
                let mut groups = groups;
                crate::factions::bastion_units::cancel_in_groups(
                    state,
                    target,
                    &self.system,
                    &planet,
                    &mut groups,
                );
                let mut taken = 0;
                for produced in &groups {
                    let produced = ground_hits_window(
                        state,
                        ctx,
                        &self.system,
                        &planet,
                        target,
                        *produced,
                        "bombardment",
                    );
                    taken += take_bombard_hits(
                        state,
                        content,
                        sources,
                        &self.system,
                        &planet,
                        target,
                        produced,
                        log,
                    );
                }
                self.report.bombardment_kills += taken;
                self.complete_bombard_plan(
                    state,
                    &planet,
                    taken,
                    held,
                    &victims,
                    self.bombard_occurrence,
                );
                self.bombard_index += 1;
                continue;
            }
            // Coexistence 7, 7.1: the invader picks, per bombarding unit, whose units take the
            // hits. The window pauses here until the driver's decider answers.
            self.stage = Stage::ChoosingBombardment(BombardCursor {
                planet_index: self.bombard_index,
                planet,
                groups,
                next_group: 0,
                taken: 0,
                held,
                victims,
                occurrence: self.bombard_occurrence,
            });
            return;
        }
    }

    /// Finish a planet's bombardment: if the step destroyed its last ground force, fire the
    /// feat (counted during the step -- later ground combat clears planets too, and the empty
    /// planet afterwards does not say which step emptied it).
    fn complete_bombard_plan(
        &mut self,
        state: &mut GameState,
        planet: &PlanetId,
        taken: usize,
        held: usize,
        victims: &std::collections::BTreeSet<PlayerId>,
        occurrence: FeatOccurrence,
    ) {
        let after = state.system_state(&self.system).on_planet(planet).to_vec();
        if let Some(active) = state.active_invasion.as_mut()
            && let Some(plan) = self
                .bombard_plan
                .iter()
                .find(|entry| &entry.planet == planet)
        {
            active.last_step = Some(InvasionStep {
                planet: planet.clone(),
                kind: "bombardment".to_owned(),
                round: 0,
                before: plan.before.clone(),
                after,
                dice: plan.dice.clone(),
                hits: [(self.invader.clone(), plan.groups.iter().sum())]
                    .into_iter()
                    .collect(),
                harrow_hits: 0,
            });
        }
        if taken == held
            && victims.len() == 1
            && state
                .system_state(&self.system)
                .on_planet(planet)
                .iter()
                .all(|unit| unit.owner == self.invader)
        {
            state.record_event_feat(
                &self.invader,
                Feat::BombardedOutTheLastGroundForces,
                occurrence,
            );
            self.pending_scoring_occurrences
                .push_back((occurrence, false));
        }
    }

    /// The `Bombarding` and `ChoosingBombardment` arms of [`Self::settle`], which on `invasion=dual` also run as the
    /// core `Bombardment` window (`rules::invasion::bombard`): the same hits, events, `last_step`, feat and scoring
    /// occurrences, and the same place where the step stops.
    fn bombard_settle_windowed(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) {
        #[cfg(feature = "typed-core")]
        {
            let sources = ctx.sources;
            let input = self.bombard_input();
            let (invader, system, occurrence, announce) = (self.invader.clone(), self.system.clone(), self.bombard_occurrence.0, ctx.timing.is_some());
            let (kills_before, scoring_before) = (self.report.bombardment_kills, self.pending_scoring_occurrences.len());
            let cell = std::cell::RefCell::new(&mut *self);
            let ctx = std::cell::RefCell::new(ctx);
            crate::dual_window!(
                crate::dual::Area::Invasion,
                "invasion::bombard_step",
                state,
                |s: &mut GameState| {
                    cell.borrow_mut().bombard_settle(s, &mut **ctx.borrow_mut());
                    crate::dual::Run::new(cell.borrow().bombard_output(kills_before, scoring_before))
                },
                |s: &mut GameState, script: &crate::dual_window::Recording| {
                    let a = crate::dual_window::BombardIn {
                        invader: &invader,
                        system: &system,
                        occurrence,
                        announce,
                        plan: &input.0,
                        index: input.1,
                        cursor: input.2.as_ref(),
                        answering: false,
                    };
                    crate::dual_window::bombard(s, sources, &a, script)
                }
            );
        }
        #[cfg(not(feature = "typed-core"))]
        self.bombard_settle(state, ctx);
    }

    /// The bombardment arms of `settle` (without the one-shot reroll announcement).
    fn bombard_settle(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) {
        loop {
            match self.stage.clone() {
                Stage::Bombarding => {
                    let mut log = Vec::new();
                    self.step_bombardment(state, ctx, &mut log);
                    flush_events(state, ctx, log);
                    // The stage moved on (a later bombardment step, the custodians stage, or
                    // the coexistence pause); re-match it.
                }
                Stage::ChoosingBombardment(BombardCursor {
                    planet_index,
                    planet,
                    taken,
                    held,
                    victims,
                    occurrence,
                    ..
                }) => {
                    // If the planet's ground forces are already gone, the remaining hits have
                    // no target (coexistence 7.2) and the planet finishes without asking.
                    let present: std::collections::BTreeSet<PlayerId> = state
                        .system_state(&self.system)
                        .on_planet(&planet)
                        .iter()
                        .filter(|unit| unit.owner != self.invader)
                        .map(|unit| unit.owner.clone())
                        .collect();
                    if present.is_empty() {
                        self.complete_bombard_plan(
                            state, &planet, taken, held, &victims, occurrence,
                        );
                        self.bombard_index = planet_index + 1;
                        self.stage = Stage::Bombarding;
                    } else {
                        // Wait for the invader's choice (the driver presents pending_choice).
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    /// The plan and position the core bombardment window starts from.
    #[cfg(feature = "typed-core")]
    fn bombard_input(&self) -> (Vec<crate::dual_window::BombardEntryIn>, usize, Option<crate::dual_window::BombardCursorIn>) {
        let plan = self
            .bombard_plan
            .iter()
            .map(|e| crate::dual_window::BombardEntryIn {
                planet: e.planet.clone(),
                before: e.before.clone(),
                dice: e.dice.clone(),
                groups: e.groups.clone(),
                held: e.held,
                victims: e.victims.clone(),
            })
            .collect();
        (plan, self.bombard_index, self.bombard_cursor_in())
    }

    #[cfg(feature = "typed-core")]
    fn bombard_cursor_in(&self) -> Option<crate::dual_window::BombardCursorIn> {
        let Stage::ChoosingBombardment(c) = &self.stage else { return None };
        Some(crate::dual_window::BombardCursorIn {
            planet_index: c.planet_index,
            planet: c.planet.clone(),
            groups: c.groups.clone(),
            next_group: c.next_group,
            taken: c.taken,
            held: c.held,
            victims: c.victims.clone(),
        })
    }

    /// Where a bombardment step left the window, as the dual compares it.
    #[cfg(feature = "typed-core")]
    fn bombard_output(&self, kills_before: usize, scoring_before: usize) -> crate::dual_window::BombardOut {
        let stage = match &self.stage {
            Stage::Bombarding => "bombarding",
            Stage::ChoosingBombardment(_) => "choosing",
            _ => "done",
        };
        crate::dual_window::BombardOut {
            index: self.bombard_index,
            stage,
            cursor: self.bombard_cursor_in(),
            kills: self.report.bombardment_kills - kills_before,
            scoring: self.pending_scoring_occurrences.iter().skip(scoring_before).map(|(o, flag)| (o.0, *flag)).collect(),
        }
    }

    /// The one-shot windows after the bombardment rolls: Aglnlan Oln rerolls first, then the
    /// other players' Scramble Frequency. Every planet's groups are then recomputed from the
    /// possibly rerolled dice — same order, same zero-skip, as the plan was built — and the
    /// staging is spent.
    fn announce_bombard_rerolls(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) {
        if !state.reroll_staging.contains_key(&self.invader) {
            return;
        }
        crate::combat::open_reroll_windows(state, ctx, &self.invader);
        if let Some(set) = state.reroll_staging.get(&self.invader).cloned() {
            let multiplier = x89_multiplier(state, &self.invader);
            for entry in &mut self.bombard_plan {
                entry.dice = set
                    .rolls
                    .iter()
                    .filter(|roll| roll.planet.as_ref() == Some(&entry.planet))
                    .flat_map(|roll| {
                        let target = roll.hits_on.unwrap_or(u32::MAX);
                        roll.faces
                            .iter()
                            .enumerate()
                            .map(|(index, face)| InvasionDie {
                                planet: entry.planet.clone(),
                                player: self.invader.clone(),
                                group: roll.unit.clone(),
                                face: *face,
                                target,
                                hit: i64::from(*face)
                                    + i64::from(roll.deltas.get(&index).copied().unwrap_or(0))
                                    >= i64::from(target),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                entry.groups = set
                    .rolls
                    .iter()
                    .filter(|roll| roll.planet.as_ref() == Some(&entry.planet))
                    .map(|roll| roll.hits() * multiplier)
                    .filter(|hits| *hits > 0)
                    .collect();
            }
        }
        state.reroll_staging.remove(&self.invader);
        state.last_reroll_player = None;
    }

    /// The error [`Self::settle`] could not return (it returns nothing): a question the decider
    /// failed to answer. Callers that settle and can fail check this straight after.
    pub fn take_settle_error(&mut self) -> Result<(), IllegalChoice> {
        match self.strict_timing_error.take() {
            None => Ok(()),
            Some(crate::timing::TimingError::IllegalChoice(error)) => Err(error),
            Some(other) => Err(crate::combat::CombatError::Timing(other).into_illegal_choice()),
        }
    }

    pub fn settle(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) {
        loop {
            match self.stage.clone() {
                Stage::Bombarding | Stage::ChoosingBombardment(_) => {
                    if matches!(self.stage, Stage::Bombarding) && !self.bombard_announced {
                        self.bombard_announced = true;
                        self.announce_bombard_rerolls(state, ctx);
                    }
                    self.bombard_settle_windowed(state, ctx);
                    // A pause waits for the invader's choice (the driver presents pending_choice).
                    if matches!(self.stage, Stage::ChoosingBombardment(_)) {
                        return;
                    }
                }
                Stage::Advancing { planets, index } => {
                    self.advance_fighting(state, ctx, &planets, index);
                    return;
                }
                Stage::ChoosingNextCombat {
                    planets,
                    index,
                    remaining,
                    ..
                } if remaining.is_empty() => {
                    // Nobody left to fight: coexistence 12's "until there are no more coexisting
                    // players" arm, reached without asking.
                    self.stage = Stage::Advancing {
                        planets,
                        index: index + 1,
                    };
                }
                Stage::FinalizingControl {
                    planets,
                    index,
                    planet,
                    previous,
                } => {
                    self.finish_control_gain(state, ctx, &planet, previous.as_ref());
                    self.advance_control(state, ctx, &planets, index);
                    return;
                }
                Stage::Custodians
                    if !custodians_removable(
                        state,
                        ctx.content,
                        ctx.sources,
                        &self.invader,
                        &self.system,
                    ) =>
                {
                    self.stage = Stage::Committing;
                }
                Stage::Committing
                    if self
                        .landing_options(state, ctx.content, ctx.sources)
                        .is_empty() =>
                {
                    if let Err(error) = self.add_forced_combats(state, ctx) {
                        self.strict_timing_error =
                            Some(crate::timing::TimingError::IllegalChoice(error));
                        return;
                    }
                    let planets = self.report.committed.clone();
                    if planets.is_empty() {
                        self.stage = Stage::Done;
                    } else {
                        self.advance_fighting(state, ctx, &planets, 0);
                    }
                    return;
                }
                _ => return,
            }
        }
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }

    /// What the invasion did.
    #[must_use]
    pub fn into_report(self) -> InvasionReport {
        self.report
    }

    /// Ground forces still in the space area, and the planets they could land on.
    fn landing_options(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Vec<ChoiceOption> {
        let (troops, origins, _) = commit_pool(
            state,
            content,
            sources,
            self.galaxy.as_deref(),
            &self.invader,
            &self.system,
            &self.extra_commits,
        );
        let planets = landable_planets(state, content, sources, &self.system);
        // The Crimson Revenant's DEPLOY: a mech from reinforcements may be committed with no units
        // of the invader in the system at all, so it is a landing in its own right.
        let deploy = crate::factions::crimson::deploy_planets(
            state,
            content,
            sources,
            &self.invader,
            &self.system,
            &planets,
        );
        if troops.is_empty() && deploy.is_empty() {
            return Vec::new();
        }
        let mut options = commit_options(
            state,
            content,
            sources,
            &self.invader,
            &self.system,
            &troops,
            &origins,
            &planets,
        );
        let terminator = options.pop();
        for planet in deploy {
            let own_ground = i64::try_from(
                state
                    .system_state(&self.system)
                    .on_planet_of(&planet, &self.invader)
                    .len(),
            )
            .unwrap_or(i64::MAX);
            options.push(
                ChoiceOption::labelled(
                    format!("deploy_commit|{planet}"),
                    COMMIT_KIND,
                    format!("deploy a {} on {planet}", crate::factions::crimson::MECH),
                )
                .with("planet", planet.to_string())
                .with("unit", crate::factions::crimson::MECH)
                .previewed(Preview::certain(vec![Delta::new(
                    Quantity::GroundForcesOnPlanet,
                    own_ground,
                    own_ground + 1,
                )])),
            );
        }
        options.extend(terminator);
        options
    }

    /// The tail of one landing: announce `UNITS_COMMITTED` and record the planet as committed.
    fn announce_landing(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: PlanetId,
        origin: Option<crate::factions::hooks_ground::CommitOrigin>,
    ) {
        // "After another player commits units to land on a planet you control." Emitted per
        // landing, carrying the controller so `actor_is_not` can pick out the player whose planet
        // it is.
        let controller = state
            .system_state(&self.system)
            .planet_control
            .get(&planet)
            .cloned();
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("player".to_owned(), self.invader.to_string().into());
        if let Some((from_system, from_planet)) = &origin {
            payload.insert("from_system".to_owned(), from_system.to_string().into());
            payload.insert("from_planet".to_owned(), from_planet.to_string().into());
        }
        if let Some(holder) = controller {
            payload.insert("controller".to_owned(), holder.to_string().into());
            if holder != self.invader {
                crate::diplomacy::evaluate_event(
                    state,
                    &crate::diplomacy::DiplomacyEventContext::HostileEngagement {
                        attacker: self.invader.clone(),
                        victim: holder,
                        activation_seq: state.activation_seq,
                    },
                )
                .expect("validated attack promises settle deterministically");
            }
        }
        let _ = ctx.emit(state, "UNITS_COMMITTED", payload);

        if !self.report.committed.contains(&planet) {
            self.report.committed.push(planet);
        }
    }

    /// The commit-ground-forces ask, or `None` when there is nothing left to land.
    fn committing_choice(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Option<Choice> {
        let options = self.landing_options(state, content, sources);
        if options.is_empty() {
            return None;
        }
        Some(
            Choice::new(
                self.invader.clone(),
                format!("commit ground forces in {}", self.system),
                options,
            )
            .contextualized(
                DecisionContext::new(
                    self.invader.clone(),
                    DecisionSource::Rule("49".to_owned()),
                    "commit_ground_forces",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::System(self.system.clone())),
            ),
        )
    }

    fn finish_committing(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), IllegalChoice> {
        let planets = self.report.committed.clone();
        crate::dual::note_gate(
            crate::dual::Area::Invasion,
            "invasion::finish_committing",
            "the Research Team offer and the forced combats of the commitment are the string's (5b)",
        );
        // Research Team: "When ground forces are committed", before anything else follows.
        self.offer_research_team(state, ctx, &planets)?;
        self.announce_and_defend_windowed(state, ctx, &planets);
        self.add_forced_combats(state, ctx)?;
        let planets = self.report.committed.clone();
        if planets.is_empty() {
            self.stage = Stage::Done;
        } else {
            self.advance_fighting(state, ctx, &planets, 0);
        }
        Ok(())
    }

    /// The commitment announcement and the space cannon defense of every committed planet, which on
    /// `driver=dual` also run as the core `SpaceCannonDefense` window (`rules::invasion::defense`): the same
    /// event, dice, hits, step record and stream positions. Positions the adapter bails on (entropic scars,
    /// L4 Disruptors, attachment cannons, nested activity) run the string body bare.
    fn announce_and_defend_windowed(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planets: &[PlanetId]) {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let (content, sources) = (ctx.content, ctx.sources);
                let announce = ctx.timing.is_some();
                let reproducible = !ctx.dice.is_preloaded() && !ctx.rng.sync_pending() && ctx.rng.seed() == state.rng_seed;
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let args = crate::dual_window::DefenseArgs { invader: &invader, system: &system, planets, announce, reproducible };
                if let Some(why) = crate::dual_window::defense_unported(state, content, sources, &args) {
                    crate::dual::note_bail("invasion::space_cannon_defense", why);
                    self.announce_and_defend(state, ctx, planets);
                    return;
                }
                let rng_before = ctx.rng.positions();
                let history_start = ctx.dice.history().len();
                let cell = std::cell::RefCell::new(&mut *self);
                let ctx = std::cell::RefCell::new(ctx);
                crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::space_cannon_defense",
                    state,
                    |s: &mut GameState| {
                        let mut ctx = ctx.borrow_mut();
                        cell.borrow_mut().announce_and_defend(s, &mut **ctx, planets);
                        let drawn: crate::dual_window::DefenseOut = (
                            ctx.dice
                                .history()
                                .iter()
                                .skip(history_start)
                                .filter(|roll| roll.reason == "space cannon defense")
                                .map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone()))
                                .collect(),
                            ctx.rng.positions(),
                        );
                        crate::dual::Run::new(drawn)
                    },
                    |s: &mut GameState, script: &crate::dual_window::Recording| {
                        crate::dual_window::space_cannon_defense(s, sources, &args, &rng_before, script)
                    }
                );
                return;
            }
        }
        self.announce_and_defend(state, ctx, planets);
    }

    /// "After you commit ground forces": the step is over, then space cannon defense on each planet.
    fn announce_and_defend(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planets: &[PlanetId]) {
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("player".to_owned(), self.invader.to_string().into());
        payload.insert(
            "planets".to_owned(),
            planets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .into(),
        );
        let _ = ctx.emit(state, "GROUND_COMMITMENT_FINISHED", payload);
        for planet in planets {
            space_cannon_defense(state, ctx, &self.system, planet, &self.invader);
        }
    }

    /// The Deepwrought's Research Team: "When ground forces are committed: if your units on that
    /// planet are not already coexisting, you may choose for your units to coexist." Asked for each
    /// committed planet another player controls where a rival ground force stands (without one
    /// there is nothing to coexist with: the units simply take the planet). A coexisting invader
    /// starts no combat there (`advance_fighting`) and takes no control (`establish_control`).
    /// An answer the decider could not give is an error, never a silent fight.
    fn offer_research_team(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: &[PlanetId],
    ) -> Result<(), IllegalChoice> {
        for planet in planets {
            // Research Team (Deepwrought) or a Viper EX-23 (Firmament mech) on the planet.
            let ability = if crate::factions::deepwrought::research_team_open(
                state,
                ctx.content,
                &self.invader,
                &self.system,
                planet,
            ) {
                Some(("researchteam", "Research Team"))
            } else if crate::factions::firmament::viper_open(
                state,
                &self.invader,
                &self.system,
                planet,
            ) {
                Some((crate::factions::firmament::MECH, "Viper EX-23"))
            } else {
                None
            };
            let Some((source, name)) = ability else {
                continue;
            };
            if !ground_force_owners(state, ctx.content, ctx.sources, &self.system, planet)
                .iter()
                .any(|owner| *owner != self.invader)
            {
                continue;
            }
            let choice = crate::choice::Choice::new(
                self.invader.clone(),
                format!("{name}: coexist on {planet} instead of fighting"),
                vec![
                    crate::choice::ChoiceOption::labelled(
                        "fight".to_owned(),
                        "research_team",
                        format!("fight for {planet}"),
                    ),
                    crate::choice::ChoiceOption::labelled(
                        "coexist".to_owned(),
                        "research_team",
                        format!("coexist on {planet}"),
                    ),
                ],
            )
            .contextualized(
                DecisionContext::new(
                    self.invader.clone(),
                    DecisionSource::FactionAbility(source.to_owned()),
                    "research_team_coexist",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::Planet {
                    system: self.system.clone(),
                    planet: planet.clone(),
                }),
            );
            let answer = ctx.ask_seeing(state, &choice)?;
            if answer.id == "coexist" {
                // `begin` fails before it touches anything (a missing taker, which cannot happen
                // here: another player holds the planet), so a failure leaves the units to fight.
                let _ = crate::factions::deepwrought::begin_coexisting(
                    state,
                    &self.invader,
                    &self.system,
                    planet,
                    None,
                );
            }
        }
        self.offer_research_team_defense(state, ctx, planets)
    }

    /// Research Team on defense (Dane's ruling): when another player commits ground forces to a
    /// planet where a Deepwrought's units stand and those units are not already coexisting, the
    /// Deepwrought player (not the invader) may choose for them to coexist. The units coexist under
    /// the ordinary rules (`begin_coexisting`: a controller steps aside and the invader gains the
    /// planet, 3.2; a seat that does not control it just coexists, 3.1), so no ground combat starts
    /// there (`advance_fighting`) and nothing is captured. Oceanbound triggers as usual.
    fn offer_research_team_defense(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: &[PlanetId],
    ) -> Result<(), IllegalChoice> {
        for planet in planets {
            if crate::coexistence::is_coexisting(state, &self.system, planet, &self.invader) {
                continue; // the invader already avoids the fight
            }
            let defenders: Vec<PlayerId> = state
                .seating_order
                .iter()
                .filter(|seat| {
                    crate::factions::deepwrought::research_team_defender_open(
                        state,
                        ctx.content,
                        seat,
                        &self.invader,
                        &self.system,
                        planet,
                    ) || crate::factions::firmament::viper_defender_open(
                        state,
                        seat,
                        &self.invader,
                        &self.system,
                        planet,
                    )
                })
                .cloned()
                .collect();
            for defender in defenders {
                let (source, name) = if crate::factions::deepwrought::research_team_defender_open(
                    state,
                    ctx.content,
                    &defender,
                    &self.invader,
                    &self.system,
                    planet,
                ) {
                    ("researchteam", "Research Team")
                } else {
                    (crate::factions::firmament::MECH, "Viper EX-23")
                };
                let choice = crate::choice::Choice::new(
                    defender.clone(),
                    format!(
                        "{name}: coexist on {planet} with {}'s ground forces instead of fighting",
                        self.invader
                    ),
                    vec![
                        crate::choice::ChoiceOption::labelled(
                            "fight".to_owned(),
                            "research_team",
                            format!("fight for {planet}"),
                        ),
                        crate::choice::ChoiceOption::labelled(
                            "coexist".to_owned(),
                            "research_team",
                            format!("coexist on {planet}"),
                        ),
                    ],
                )
                .contextualized(
                    DecisionContext::new(
                        defender.clone(),
                        DecisionSource::FactionAbility(source.to_owned()),
                        "research_team_defend_coexist",
                        state.phase,
                        state.round,
                    )
                    .about(DecisionTarget::Planet {
                        system: self.system.clone(),
                        planet: planet.clone(),
                    }),
                );
                let answer = ctx.ask_seeing(state, &choice)?;
                if answer.id == "coexist"
                    && crate::factions::deepwrought::begin_coexisting(
                        state,
                        &defender,
                        &self.system,
                        planet,
                        Some(&self.invader),
                    )
                    .is_ok()
                {
                    self.report.coexisted.push(planet.clone());
                }
            }
        }
        Ok(())
    }

    /// Planets a faction rule makes the invader fight on without committing there (Titans of Ul
    /// Coalescence): they join the planets this invasion fights on and takes control of. Nothing
    /// was committed, so no commitment event or space cannon defense concerns them.
    fn add_forced_combats(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        // Slumberstate Computing is read before anything is added: "if you commit no other units".
        let committed_nothing = self.report.committed.is_empty();
        for planet in crate::factions::hooks_ground::forced_combat_planets(
            state,
            content,
            sources,
            &self.invader,
            &self.system,
        ) {
            if self.report.committed.contains(&planet) {
                continue;
            }
            if committed_nothing && self.coexist_instead(state, ctx, &planet)? {
                continue;
            }
            self.report.committed.push(planet);
        }
        Ok(())
    }

    /// Titans' Slumberstate Computing: "When COALESCENCE results in a ground combat, if you commit
    /// no other units, you may choose for your units to coexist instead." Asked only where a rival
    /// ground force there would make the combat, and where another player controls the planet (the
    /// controller keeps it, coexistence 3.1). Returns whether the units now coexist; an answer the
    /// decider could not give is an error, never a silent fight.
    fn coexist_instead(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
    ) -> Result<bool, IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        if !crate::breakthroughs::holds(state, &self.invader, "titansbt") {
            return Ok(false);
        }
        let holder = state
            .system_state(&self.system)
            .planet_control
            .get(planet)
            .cloned();
        if holder.is_none() || holder.as_ref() == Some(&self.invader) {
            return Ok(false);
        }
        let rival = ground_force_owners(state, content, sources, &self.system, planet)
            .iter()
            .any(|owner| owner != &self.invader);
        if !rival {
            return Ok(false);
        }
        let choice = crate::choice::Choice::new(
            self.invader.clone(),
            format!("Slumberstate Computing: coexist on {planet} instead of fighting"),
            vec![
                crate::choice::ChoiceOption::labelled(
                    "fight".to_owned(),
                    "coalescence",
                    format!("fight for {planet}"),
                ),
                crate::choice::ChoiceOption::labelled(
                    "coexist".to_owned(),
                    "coalescence",
                    format!("coexist on {planet}"),
                ),
            ],
        )
        .offered(
            breakthrough_card(content, "titansbt"),
            vec![
                crate::choice::offer_fact_planet("Planet", planet.as_str(), self.system.as_str()),
                crate::choice::offer_fact_seat(
                    "Controlled by",
                    holder.as_ref().map_or("", PlayerId::as_str),
                ),
            ],
            &[
                (
                    "fight",
                    crate::choice::offer_caption(
                        "Fight for the planet",
                        Some("Ground combat decides who controls it"),
                    ),
                ),
                (
                    "coexist",
                    crate::choice::offer_caption(
                        "Coexist",
                        Some("No combat; the controller keeps the planet"),
                    ),
                ),
            ],
        )
        .contextualized(
            DecisionContext::new(
                self.invader.clone(),
                DecisionSource::Content("titansbt".to_owned()),
                "coalescence_coexist",
                state.phase,
                state.round,
            )
            .about(DecisionTarget::Planet {
                system: self.system.clone(),
                planet: planet.clone(),
            }),
        );
        let answer = ctx.ask_seeing(state, &choice)?;
        // `begin` fails before it touches anything (a missing taker, which this path never has: the
        // holder is another player), so a failure leaves the state as it was and the units fight.
        Ok(answer.id == "coexist"
            && crate::coexistence::begin(state, &self.system, planet, &self.invader, None).is_ok())
    }

    fn emit_ground_event(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        name: &str,
        payload: std::collections::BTreeMap<String, serde_json::Value>,
    ) {
        if let Err(error) = ctx.emit(state, name, payload) {
            if self.combat_only && self.strict_timing_error.is_none() {
                self.strict_timing_error = Some(error);
            }
        }
    }

    /// The event a Fire Team hooks: named by the roller, so the window is `actor_is`.
    fn emit_ground_rolls_made(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        player: &PlayerId,
        hits: usize,
    ) {
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("player".to_owned(), player.to_string().into());
        payload.insert("hits".to_owned(), i64::try_from(hits).unwrap_or(0).into());
        self.emit_ground_event(state, ctx, "GROUND_ROLLS_MADE", payload);
    }

    /// The hits the staged dice now show after the reroll windows at the roll site — else the
    /// originals. The caller clears the staging after using these.
    fn rerolled_ground_hits(
        state: &GameState,
        invader: &PlayerId,
        invader_hits: usize,
        defender: &PlayerId,
        defender_hits: usize,
    ) -> (usize, usize) {
        // Staged dice are raw faces; X-89 doubles what they produce, as `roll_ground` did.
        (
            state
                .reroll_staging
                .get(invader)
                .map_or(invader_hits, |set| {
                    crate::combat::staged_hits(set) * x89_multiplier(state, invader)
                }),
            state
                .reroll_staging
                .get(defender)
                .map_or(defender_hits, |set| {
                    crate::combat::staged_hits(set) * x89_multiplier(state, defender)
                }),
        )
    }

    /// The start of one ground combat round (42.1): the round counter, "round started" (when a copied
    /// round agent listens), the faction round-start hooks, Proxima's watch, and the Dunlain Reaper.
    /// `None` when a handler refused (`strict_timing_error` carries it out).
    fn begin_ground_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
    ) -> Option<RoundStart> {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let copied_round = ctx.timing.is_some()
                    && (crate::factions::borrowed_round_agents::has_round_copy(state, ctx.content, "solagent")
                        || crate::factions::borrowed_round_agents::has_owner_round_agent(state));
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let args = crate::dual_window::BeginGroundArgs { invader: &invader, defender, system: &system, planet, copied_round, sources: ctx.sources };
                if let Some(why) = crate::dual_window::begin_ground_unported(state, &args) {
                    crate::dual::note_bail("invasion::begin_ground_round", why);
                    return self.begin_ground_round_string(state, ctx, planet, defender);
                }
                let ok = std::cell::Cell::new(false);
                let mut out = None;
                crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::begin_ground_round",
                    state,
                    |s: &mut GameState| {
                        out = self.begin_ground_round_string(s, ctx, planet, defender);
                        ok.set(out.is_some());
                        crate::dual::Run::new(ok.get())
                    },
                    |s: &mut GameState, script: &crate::dual_window::Recording| {
                        if !ok.get() {
                            return None;
                        }
                        crate::dual_window::begin_ground(s, &args, script)
                    }
                );
                return out;
            }
        }
        self.begin_ground_round_string(state, ctx, planet, defender)
    }

    /// The string body of [`Self::begin_ground_round`].
    fn begin_ground_round_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
    ) -> Option<RoundStart> {
        let (content, sources) = (ctx.content, ctx.sources);
        let defender = defender.clone();
        let planet = planet.clone();
        if let Some(active) = state.active_invasion.as_mut() {
            if active.planet.as_ref() == Some(&planet)
                && active.defender.as_ref() == Some(&defender)
            {
                active.ground_round += 1;
            }
        }
        let copied_round = ctx.timing.is_some()
            && (crate::factions::borrowed_round_agents::has_round_copy(state, content, "solagent")
                || crate::factions::borrowed_round_agents::has_owner_round_agent(state));
        // From rules version 16 every ground round is a numbered combat round, so what a card
        // marks "for this round" (the Sol agent's extra die) cannot outlive it.
        let numbered = state.rules_version >= ti4_model::state::RULES_FACTION_EFFECTS;
        if numbered && !copied_round {
            state.combat_round_seq = state.combat_round_seq.saturating_add(1);
        }
        if copied_round {
            state.combat_round_seq = state.combat_round_seq.saturating_add(1);
            let payload = std::collections::BTreeMap::from([
                ("system".to_owned(), self.system.to_string().into()),
                ("planet".to_owned(), planet.to_string().into()),
                ("attacker".to_owned(), self.invader.to_string().into()),
                ("defender".to_owned(), defender.to_string().into()),
                (
                    "round_seq".to_owned(),
                    i64::from(state.combat_round_seq).into(),
                ),
            ]);
            if let Err(error) = ctx.emit(state, "GROUND_COMBAT_ROUND_STARTED", payload) {
                self.strict_timing_error = Some(error);
                return None;
            }
        }
        // Faction effects at the start of the round (Naaz Supercharge), for each participant.
        for who in [self.invader.clone(), defender.clone()] {
            crate::factions::hooks_ground::ground_combat_round_started(
                state,
                content,
                sources,
                ctx.table,
                &who,
                &self.system,
                &planet,
            );
        }
        // Proxima Targeting VI: "At the start of a round of ground combat, you may resolve
        // BOMBARDMENT 8 (x3) ...". Emitted only while a side holds the technology, so every other
        // game's event stream is unchanged.
        if crate::factions::bastion_units::watches_ground_round(state, &[&self.invader, &defender])
        {
            let payload = std::collections::BTreeMap::from([
                ("system".to_owned(), self.system.to_string().into()),
                ("planet".to_owned(), planet.to_string().into()),
                ("attacker".to_owned(), self.invader.to_string().into()),
                ("defender".to_owned(), defender.to_string().into()),
            ]);
            if let Err(error) =
                ctx.emit(state, crate::factions::bastion_units::ROUND_BEGAN, payload)
            {
                self.strict_timing_error = Some(error);
                return None;
            }
        }
        // Letnev's Dunlain Reaper: "DEPLOY: At the start of a round of ground combat, you may spend
        // 2 resources to replace 1 of your infantry in that combat with 1 mech from your
        // reinforcements." Before the dice, so the mech fights the round it arrives for.
        for who in [self.invader.clone(), defender.clone()] {
            dunlain_reaper(state, ctx, &who, &self.system, &planet);
        }
        let before = state.system_state(&self.system).on_planet(&planet).to_vec();
        Some(RoundStart { copied_round, before })
    }

    /// Both sides' ground dice of a round, which on `invasion=dual` also run as the core `GroundRoll` window
    /// (`rules::invasion::roll`): the same dice from the stream, hits, reroll staging and stream positions.
    fn roll_ground_pair_windowed(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planet: &PlanetId, defender: &PlayerId) -> (usize, usize) {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let reproducible = !ctx.dice.is_preloaded() && !ctx.rng.sync_pending() && ctx.rng.seed() == state.rng_seed;
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let args = crate::dual_window::GroundRollArgs { attacker: &invader, defender, system: &system, planet, sources: ctx.sources, reproducible };
                if let Some(why) = crate::dual_window::ground_roll_unported(state, &args) {
                    crate::dual::note_bail("invasion::roll_ground", why);
                    return self.roll_ground_pair(state, ctx, planet, defender);
                }
                let rng_before = ctx.rng.positions();
                let history_start = ctx.dice.history().len();
                let cell = std::cell::RefCell::new(&mut *self);
                let ctx = std::cell::RefCell::new(ctx);
                return crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::roll_ground",
                    state,
                    |s: &mut GameState| {
                        let mut ctx = ctx.borrow_mut();
                        let hits = cell.borrow_mut().roll_ground_pair(s, &mut **ctx, planet, defender);
                        let drawn: Vec<(u32, Vec<u32>)> = ctx
                            .dice
                            .history()
                            .iter()
                            .skip(history_start)
                            .filter(|roll| roll.reason == "ground combat")
                            .map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone()))
                            .collect();
                        crate::dual::Run::new(crate::dual_window::GroundRollOut { hits, drawn, rng: ctx.rng.positions() })
                    },
                    |s: &mut GameState, script: &crate::dual_window::Recording| {
                        crate::dual_window::ground_roll(s, &args, &rng_before, script)
                    }
                )
                .hits;
            }
        }
        self.roll_ground_pair(state, ctx, planet, defender)
    }

    /// The two `roll_ground` calls: the invader rolls, then the defender.
    fn roll_ground_pair(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planet: &PlanetId, defender: &PlayerId) -> (usize, usize) {
        let (content, sources) = (ctx.content, ctx.sources);
        let attacker_hits = roll_ground(state, content, sources, ctx.dice, ctx.rng, &self.invader, &self.system, planet);
        let defender_hits = roll_ground(state, content, sources, ctx.dice, ctx.rng, defender, &self.system, planet);
        (attacker_hits, defender_hits)
    }

    /// The part of the roll step after both sides rolled: the round number, "ground rolls made" for each side
    /// (the reroll windows open there), the hits the staged dice now show, the dice as the board shows them, and
    /// the cleared staging. On `invasion=dual` it also runs as the core `GroundRollTail` window.
    fn roll_ground_tail(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planet: &PlanetId, defender: &PlayerId, copied_round: bool, hits: (usize, usize)) -> (usize, usize, Vec<InvasionDie>) {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let args = crate::dual_window::RollTailArgs { invader: &invader, defender, system: &system, planet, rolled: hits, bump: !copied_round, announce: ctx.timing.is_some() };
                if let Some(why) = crate::dual_window::roll_tail_unported(state, &args) {
                    crate::dual::note_bail("invasion::roll_ground_tail", why);
                    return self.roll_ground_tail_string(state, ctx, planet, defender, copied_round, hits);
                }
                let rng_before = ctx.rng.positions();
                let string_after = std::cell::Cell::new(None);
                let ctx = std::cell::RefCell::new(ctx);
                let this = std::cell::RefCell::new(&mut *self);
                return crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::roll_ground_tail",
                    state,
                    |s: &mut GameState| {
                        let out = this.borrow_mut().roll_ground_tail_string(s, &mut **ctx.borrow_mut(), planet, defender, copied_round, hits);
                        string_after.set(Some(ctx.borrow().rng.positions()));
                        crate::dual::Run::new(out)
                    },
                    |s: &mut GameState, script: &crate::dual_window::Recording| crate::dual_window::roll_tail(s, &args, &rng_before, &string_after, script)
                );
            }
        }
        self.roll_ground_tail_string(state, ctx, planet, defender, copied_round, hits)
    }

    /// The string body of [`Self::roll_ground_tail`].
    fn roll_ground_tail_string(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planet: &PlanetId, defender: &PlayerId, copied_round: bool, hits: (usize, usize)) -> (usize, usize, Vec<InvasionDie>) {
        let (attacker_hits, defender_hits) = hits;
        let defender = defender.clone();
        let planet = planet.clone();
        let mut rolls: Vec<InvasionDie> = Vec::new();
        if !copied_round {
            state.combat_round_seq = state.combat_round_seq.saturating_add(1);
        }
        // "After your ground forces make combat rolls during a round of ground combat." Emitted
        // between the rolls and the removals, which is what the window means: a card played
        // here acts on the hits before anyone dies of them.
        for (who, hits) in [
            (self.invader.clone(), attacker_hits),
            (defender.clone(), defender_hits),
        ] {
            self.emit_ground_rolls_made(state, ctx, &planet, &who, hits);
        }
        // Fire Team's window ("reroll any number of your dice") opened at those emits; the
        // hits that remove units are what the possibly rerolled dice now show.
        let (attacker_hits, defender_hits) = Self::rerolled_ground_hits(
            state,
            &self.invader,
            attacker_hits,
            &defender,
            defender_hits,
        );
        // The dice as the board shows them, for the invasion step's report.
        rolls.clear();
        for player in [&self.invader, &defender] {
            if let Some(set) = state.reroll_staging.get(player) {
                for roll in set
                    .rolls
                    .iter()
                    .filter(|roll| roll.planet.as_ref() == Some(&planet))
                {
                    let target = roll.hits_on.unwrap_or(u32::MAX);
                    for (index, face) in roll.faces.iter().enumerate() {
                        let adjusted = i64::from(*face)
                            + i64::from(roll.deltas.get(&index).copied().unwrap_or(0));
                        rolls.push(InvasionDie {
                            planet: planet.clone(),
                            player: player.clone(),
                            group: roll.unit.clone(),
                            face: *face,
                            target,
                            hit: adjusted >= i64::from(target),
                        });
                    }
                }
            }
        }
        state.reroll_staging.clear();
        state.last_reroll_player = None;
        (attacker_hits, defender_hits, rolls)
    }

    /// The "Roll Dice" step (42.2) with its replays: both sides roll, the roll-made windows open, the
    /// dice are read back after rerolls. `None` when a handler refused.
    fn roll_ground_step(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
        copied_round: bool,
    ) -> Option<(usize, usize, Vec<InvasionDie>)> {
        let defender = defender.clone();
        let planet = planet.clone();
        let round_before = state.combat_round_seq;
        let mut replays = 0u32;
        let mut rolls: Vec<InvasionDie>;
        let (attacker_hits, defender_hits) = loop {
            state.combat_round_seq = round_before;
            // 42.2: hits are simultaneous, so both sides roll before either loses anything.
            let (attacker_hits, defender_hits) = self.roll_ground_pair_windowed(state, ctx, &planet, &defender);
            let (attacker_hits, defender_hits, rolls_now) = self.roll_ground_tail(state, ctx, &planet, &defender, copied_round, (attacker_hits, defender_hits));
            rolls = rolls_now;
            // The end of the step: both sides' dice are final and nothing has landed.
            let mut ended = std::collections::BTreeMap::new();
            ended.insert("system".to_owned(), self.system.to_string().into());
            ended.insert("planet".to_owned(), planet.to_string().into());
            ended.insert("attacker".to_owned(), self.invader.to_string().into());
            ended.insert("defender".to_owned(), defender.to_string().into());
            state.faction_marks.remove(crate::combat::ROLL_REPLAY_MARK);
            // Emitted only when someone is listening (a readied The Thundarian). An illegal answer
            // in its window is an error, not a skipped step: `strict_timing_error` carries it out
            // of the round (the caller turns it into an illegal choice, and the driver restores
            // the snapshot it took before the first die), like Round Started's.
            if crate::factions::nomad_agents::watches_roll_step(state) {
                if let Err(error) = ctx.emit(state, "GROUND_COMBAT_ROLL_STEP_ENDED", ended) {
                    self.strict_timing_error = Some(error);
                    return None;
                }
            }
            if crate::combat::take_roll_replay(state) {
                // Bounded like the space round's: past the limit the dice stand as rolled.
                replays += 1;
                if replays <= crate::combat::MAX_ROLL_REPLAYS {
                    continue;
                }
            }
            break (attacker_hits, defender_hits);
        };
        Some((attacker_hits, defender_hits, rolls))
    }

    /// The hits of the round (42.3): added hits, the hit windows, the removal of casualties and
    /// L1Z1X's Harrow. The events of the removals are collected in `log`. Returns the hits each side
    /// finally assigned (invader, defender) and Harrow's.
    fn assign_ground_round_hits(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
        hits: (usize, usize),
        log: &mut Vec<PendingEvent>,
    ) -> (usize, usize, usize) {
        let (content, sources) = (ctx.content, ctx.sources);
        let defender = defender.clone();
        let planet = planet.clone();
        let (attacker_hits, defender_hits) = hits;
        // Hits a faction adds to its own roll once the dice are final (Valkyrie Particle Weave).
        // Assigned by the opponent like any other hit.
        // Both sides' final dice are read before either side adds anything.
        let added = |state: &GameState, who: &PlayerId, hits: usize, opponent_hits: usize| {
            hits + crate::factions::hooks_ground::ground_rolls_extra_hits(
                state,
                content,
                sources,
                who,
                &self.system,
                &planet,
                hits,
                opponent_hits,
            )
        };
        let (rolled_attacker, rolled_defender) = (attacker_hits, defender_hits);
        let attacker_hits = added(state, &self.invader, rolled_attacker, rolled_defender);
        let defender_hits = added(state, &defender, rolled_defender, rolled_attacker);
        let attacker_hits = ground_hits_window(
            state,
            ctx,
            &self.system,
            &planet,
            &defender,
            attacker_hits,
            "ground_combat",
        );
        let defender_hits = ground_hits_window(
            state,
            ctx,
            &self.system,
            &planet,
            &self.invader,
            defender_hits,
            "ground_combat",
        );
        remove_ground(
            state,
            content,
            sources,
            &self.system,
            &planet,
            &defender,
            attacker_hits,
            "ground_combat",
            log,
        );
        remove_ground(
            state,
            content,
            sources,
            &self.system,
            &planet,
            &self.invader,
            defender_hits,
            "ground_combat",
            log,
        );
        // L1Z1X's Harrow: at the end of each round of ground combat, bombard the defender again.
        // A planetary shield stops it as it stops any bombardment (63.2).
        let harrow = if bombardable(
            state,
            content,
            sources,
            &self.system,
            &planet,
            &self.invader,
        ) {
            let harrow = crate::faction_abilities::ground_combat_round_ended(
                state,
                content,
                sources,
                ctx.dice,
                ctx.rng,
                &self.invader,
                &self.system,
            );
            // Harrow is a bombardment: Proxima Targeting VI cancels hits against its holder.
            let harrow = crate::factions::bastion_units::cancel_bombardment_hits(
                state,
                &defender,
                &self.system,
                &planet,
                harrow,
            );
            let harrow = ground_hits_window(
                state,
                ctx,
                &self.system,
                &planet,
                &defender,
                harrow,
                "harrow",
            );
            remove_ground(
                state,
                content,
                sources,
                &self.system,
                &planet,
                &defender,
                harrow,
                "harrow",
                log,
            );
            harrow
        } else {
            0
        };
        (attacker_hits, defender_hits, harrow)
    }

    /// The end of the round: the step report on the invasion record, the removal events, and
    /// Alarum. `false` when a handler refused.
    fn end_ground_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
        round: GroundRound,
    ) -> bool {
        let GroundRound {
            before,
            dice: rolls,
            attacker_hits,
            defender_hits,
            harrow,
            log,
        } = round;
        let defender = defender.clone();
        let planet = planet.clone();
        let after = state.system_state(&self.system).on_planet(&planet).to_vec();
        if let Some(active) = state.active_invasion.as_mut() {
            active.last_step = Some(InvasionStep {
                planet: planet.clone(),
                kind: "ground_round".to_owned(),
                round: active.ground_round,
                before,
                after,
                dice: rolls,
                hits: [
                    (self.invader.clone(), attacker_hits),
                    (defender.clone(), defender_hits),
                ]
                .into_iter()
                .collect(),
                harrow_hits: harrow,
            });
        }
        // After the whole round, so a handler sees the board once every hit has landed.
        for (name, payload) in log {
            self.emit_ground_event(state, ctx, name, payload);
        }
        // Alarum: "At the end of a round of combat on this planet". Emitted only when a Ral Nel mech
        // stands on the planet, so every other game's event log is unchanged.
        if crate::factions::ralnel::alarum_on(state, &self.system, &planet) {
            let mut ended = std::collections::BTreeMap::new();
            ended.insert("system".to_owned(), self.system.to_string().into());
            ended.insert("planet".to_owned(), planet.to_string().into());
            ended.insert("attacker".to_owned(), self.invader.to_string().into());
            ended.insert("defender".to_owned(), defender.to_string().into());
            if let Err(error) = ctx.emit(state, "GROUND_COMBAT_ROUND_ENDED", ended) {
                self.strict_timing_error = Some(error);
                return false;
            }
        }
        true
    }

    /// The hits of a round and its step report (`assign_ground_round_hits`, `end_ground_round`), which on
    /// `invasion=dual` also run as the core `GroundRoundHits` window (`rules::invasion::round`). `false` when a
    /// handler refused (the string result).
    #[allow(clippy::too_many_arguments, reason = "the round's facts")]
    fn ground_round_hits_windowed(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
        rolled: (usize, usize),
        before: Vec<Unit>,
        dice: Vec<InvasionDie>,
    ) -> bool {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let (content, sources) = (ctx.content, ctx.sources);
                let announce = ctx.timing.is_some();
                let invader = self.invader.clone();
                let system = self.system.clone();
                let harrow = crate::faction_abilities::has(state, content, &invader, "harrow");
                let reproducible = !ctx.dice.is_preloaded() && !ctx.rng.sync_pending() && ctx.rng.seed() == state.rng_seed;
                let rng_before = ctx.rng.positions();
                let args = crate::dual_window::RoundHitsArgs { invader: &invader, defender, system: &system, planet, rolled, announce, before: &before, dice: &dice, harrow, reproducible, rng_before: &rng_before };
                if let Some(why) = crate::dual_window::round_hits_unported(state, content, sources, &args) {
                    crate::dual::note_bail("invasion::ground_round_hits", why);
                    return self.ground_round_hits(state, ctx, planet, defender, rolled, before, dice).0;
                }
                let cell = std::cell::RefCell::new(&mut *self);
                let ctx = std::cell::RefCell::new(ctx);
                let (before_s, dice_s) = (before.clone(), dice.clone());
                let history_start = ctx.borrow().dice.history().len();
                return crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::ground_round_hits",
                    state,
                    |s: &mut GameState| {
                        let mut ctx = ctx.borrow_mut();
                        let (ok, hits) = cell.borrow_mut().ground_round_hits(s, &mut **ctx, planet, defender, rolled, before_s, dice_s);
                        let drawn: crate::dual_window::DefenseOut = (
                            ctx.dice
                                .history()
                                .iter()
                                .skip(history_start)
                                .filter(|roll| roll.reason == "harrow")
                                .map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone()))
                                .collect(),
                            ctx.rng.positions(),
                        );
                        crate::dual::Run::new(((ok, hits), drawn))
                    },
                    |s: &mut GameState, script: &crate::dual_window::Recording| {
                        crate::dual_window::round_hits(s, sources, &args, script)
                    }
                )
                .0
                .0;
            }
        }
        self.ground_round_hits(state, ctx, planet, defender, rolled, before, dice).0
    }

    /// The string body: assign the hits, then report the round. The flag is `end_ground_round`'s; the pair
    /// the hits that landed on the defender and on the invader.
    #[allow(clippy::too_many_arguments, reason = "the round's facts")]
    fn ground_round_hits(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
        rolled: (usize, usize),
        before: Vec<Unit>,
        dice: Vec<InvasionDie>,
    ) -> (bool, (usize, usize)) {
        let mut log = Vec::new();
        let (attacker_hits, defender_hits, harrow) = self.assign_ground_round_hits(state, ctx, planet, defender, rolled, &mut log);
        let round = GroundRound { before, dice, attacker_hits, defender_hits, harrow, log };
        (self.end_ground_round(state, ctx, planet, defender, round), (attacker_hits, defender_hits))
    }

    fn resolve_ground_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: Vec<PlanetId>,
        index: usize,
        defender: PlayerId,
    ) {
        let planet = planets[index].clone();
        let Some(RoundStart {
            copied_round,
            before,
        }) = self.begin_ground_round(state, ctx, &planet, &defender)
        else {
            return;
        };
        let Some((rolled_attacker, rolled_defender, dice)) =
            self.roll_ground_step(state, ctx, &planet, &defender, copied_round)
        else {
            return;
        };
        if !self.ground_round_hits_windowed(state, ctx, &planet, &defender, (rolled_attacker, rolled_defender), before, dice) {
            return;
        }
        self.finish_ground_round(state, ctx, planets, index, defender, &planet);
    }

    /// 42.3: the fight ends when one side has no ground forces left on the planet — not when
    /// its last structure falls, because structures never fight (KD-2). Both sides surviving
    /// simply starts the next round on the same planet.
    fn finish_ground_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: Vec<PlanetId>,
        index: usize,
        defender: PlayerId,
        planet: &PlanetId,
    ) {
        let (content, sources) = (ctx.content, ctx.sources);
        let owners = ground_force_owners(state, content, sources, &self.system, planet);
        let invader_survives = owners.contains(&self.invader);
        let defender_survives = owners.contains(&defender);
        if invader_survives && defender_survives {
            self.stage = Stage::Fighting {
                planets,
                index,
                defender,
            };
            return;
        }

        let winner = if invader_survives {
            self.invader.clone()
        } else {
            defender.clone()
        };
        let loser = if winner == self.invader {
            defender
        } else {
            self.invader.clone()
        };
        // Only a side with forces left has won (both wiped out is nobody's win).
        if owners.contains(&winner) {
            crate::faction_techs::dacxive_animators(
                state,
                content,
                sources,
                &winner,
                &self.system,
                planet,
            );
        }
        // Matriarch's fighters return after the combat, before 49.5 decides whether the winner
        // still has a ground force to establish control with.
        self.return_temporary_space_commits(state, planet);
        // Whether control will change at 49.5: the invader won a planet it did not control and
        // still has a ground force after temporary combatants returned to space.
        let control_changed = winner == self.invader
            && owners.contains(&winner)
            && ground_force_owners(state, content, sources, &self.system, planet)
                .contains(&self.invader)
            && state.system_state(&self.system).planet_control.get(planet) != Some(&self.invader);
        let noted = self
            .current_ground_occurrence
            .take()
            .is_some_and(|occurrence| {
                let noted = note_ground_combat_win_feats(
                    state,
                    content,
                    sources,
                    &self.system,
                    &winner,
                    &loser,
                    &self.notes_at_tactical_start,
                    occurrence,
                );
                if noted {
                    self.pending_scoring_occurrences
                        .push_back((occurrence, true));
                }
                noted
            });
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("attacker".to_owned(), self.invader.to_string().into());
        let defender_id = if loser == self.invader {
            &winner
        } else {
            &loser
        };
        payload.insert("defender".to_owned(), defender_id.to_string().into());
        if owners.contains(&winner) {
            payload.insert("winner".to_owned(), winner.to_string().into());
        }
        payload.insert("control_changed".to_owned(), control_changed.into());
        self.emit_ground_event(state, ctx, "GROUND_COMBAT_ENDED", payload);
        if self.combat_only {
            self.stage = Stage::Done;
            return;
        }
        if noted {
            self.stage = Stage::Advancing {
                planets,
                index: index + 1,
            };
        } else {
            self.advance_fighting(state, ctx, &planets, index + 1);
        }
    }

    /// Announce the start of a ground combat (`GROUND_COMBAT_STARTED`, before the first round) and
    /// report whether both sides still have ground forces after whatever reacted to it.
    fn start_ground_combat(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        defender: &PlayerId,
    ) -> bool {
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("attacker".to_owned(), self.invader.to_string().into());
        payload.insert("defender".to_owned(), defender.to_string().into());
        self.emit_ground_event(state, ctx, "GROUND_COMBAT_STARTED", payload);
        let owners = ground_force_owners(state, ctx.content, ctx.sources, &self.system, planet);
        owners.contains(&self.invader) && owners.contains(defender)
    }

    /// Matriarch's fighters have been ground forces only for this invasion's combat. Return the
    /// surviving copies that this window committed before a later combat or 49.5 can treat them
    /// as permanent ground forces.
    fn return_temporary_space_commits(&mut self, state: &mut GameState, planet: &PlanetId) {
        let Some(committed) = self.temporary_space_commits.remove(planet) else {
            return;
        };
        let on_planet = state.system_state(&self.system).on_planet(planet).to_vec();
        let mut returned = Vec::new();
        for unit in committed {
            if let Some(index) = on_planet.iter().position(|standing| *standing == unit) {
                let unit = on_planet[index].clone();
                state
                    .system_mut(&self.system)
                    .remove_from_planet(planet, std::slice::from_ref(&unit));
                returned.push(unit);
            }
        }
        if !returned.is_empty() {
            state.system_mut(&self.system).add(&returned);
        }
    }

    fn return_all_temporary_space_commits(&mut self, state: &mut GameState) {
        let planets: Vec<PlanetId> = self.temporary_space_commits.keys().cloned().collect();
        for planet in planets {
            self.return_temporary_space_commits(state, &planet);
        }
    }

    /// The planet the invasion fights on next from `index` on, without starting anything. On `invasion=dual`
    /// the choice also runs as the core `Advance` window.
    fn next_fight_windowed(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, planets: &[PlanetId], index: usize) -> FightPick {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let (content, sources) = (ctx.content, ctx.sources);
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let coexisted = self.report.coexisted.clone();
                let args = crate::dual_window::AdvanceArgs { invader: &invader, system: &system, planets, index, coexisted: &coexisted, sources };
                let this = &*self;
                return crate::dual_window!(
                    crate::dual::Area::Invasion,
                    "invasion::advance_fighting",
                    state,
                    |s: &mut GameState| crate::dual::Run::new(this.next_fight(s, content, sources, planets, index)),
                    |s: &mut GameState, script: &crate::dual_window::Recording| crate::dual_window::advance(s, &args, script)
                );
            }
        }
        self.next_fight(state, ctx.content, ctx.sources, planets, index)
    }

    /// The string body of [`Self::next_fight_windowed`]: the selection of `advance_fighting`.
    fn next_fight(&self, state: &GameState, content: &ContentStore, sources: SourceSet, planets: &[PlanetId], mut index: usize) -> FightPick {
        while index < planets.len() {
            let planet = &planets[index];
            let owners = ground_force_owners(state, content, sources, &self.system, planet);
            if !owners.contains(&self.invader)
                || crate::coexistence::is_coexisting(state, &self.system, planet, &self.invader)
                || self.report.coexisted.contains(planet)
            {
                index += 1;
                continue;
            }
            // Coexistence 11: a player who had no units here and commits ground forces "must
            // start a ground combat against the player that controls that planet" -- not against
            // whichever rival happens to sort first.
            let controller = state
                .system_state(&self.system)
                .planet_control
                .get(planet)
                .cloned()
                .filter(|holder| *holder != self.invader && owners.contains(holder));
            let next = controller.or_else(|| owners.iter().find(|owner| **owner != self.invader).cloned());
            if let Some(defender) = next {
                return FightPick::Fight { index, defender };
            }
            // Coexistence 9 and 12: the invader holds the ground here and coexisters remain; the
            // rule offers *another* combat rather than forcing one, so this is a choice.
            let remaining: Vec<PlayerId> = crate::coexistence::coexisters(state, &self.system, planet)
                .into_iter()
                .filter(|player| *player != self.invader)
                .collect();
            if !remaining.is_empty() {
                return FightPick::Coexisters { index, remaining };
            }
            index += 1;
        }
        FightPick::Exhausted
    }

    /// Move to the next planet that still needs a fight, or finish and take control.
    fn advance_fighting(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: &[PlanetId],
        mut index: usize,
    ) {
        loop {
            match self.next_fight_windowed(state, ctx, planets, index) {
                FightPick::Fight { index: at, defender } => {
                    index = at;
                    let planet = &planets[index];
                    if !self.start_ground_combat(state, ctx, planet, &defender) {
                        // A handler at the start of the combat removed a side's last ground force;
                        // look at the planet again.
                        continue;
                    }
                    self.current_ground_occurrence = Some(state.begin_feat_occurrence());
                    self.stage = Stage::Fighting {
                        planets: planets.to_vec(),
                        index,
                        defender,
                    };
                    return;
                }
                FightPick::Coexisters { index: at, remaining } => {
                    self.stage = Stage::ChoosingNextCombat {
                        planets: planets.to_vec(),
                        index: at,
                        planet: planets[at].clone(),
                        remaining,
                    };
                    return;
                }
                FightPick::Exhausted => break,
            }
        }
        let committed = self.report.committed.clone();
        self.return_all_temporary_space_commits(state);
        self.advance_control(state, ctx, &committed, 0);
    }

    /// Establish and finish control one planet at a time so a control-loss occurrence is not
    /// delayed past later captures, gain-control effects, or exploration.
    fn advance_control(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: &[PlanetId],
        mut index: usize,
    ) {
        while index < planets.len() {
            let planet = planets[index].clone();
            let Some((planet, previous)) = establish_control(
                state,
                ctx.content,
                ctx.sources,
                &self.system,
                &self.invader,
                std::slice::from_ref(&planet),
            )
            .into_iter()
            .next() else {
                index += 1;
                continue;
            };
            self.report
                .captured
                .push((planet.clone(), previous.clone()));

            let lost_home = previous.as_ref().is_some_and(|holder| {
                state
                    .player(holder)
                    .and_then(|seat| seat.home_system.as_ref())
                    == Some(&self.system)
            });
            if lost_home {
                // This public trigger opens a window that inspects the former
                // holder's private secret objectives, even when none is scoreable.
                // Latch before that inspection so disposable executions cannot
                // disclose the hand through whether a scoring offer appears.
                ctx.rng.observe_hidden_information();
                let holder = previous.as_ref().expect("lost home has a former holder");
                let occurrence = state.begin_feat_occurrence();
                state.record_event_feat(holder, Feat::LostAHomePlanet, occurrence);
                self.pending_scoring_occurrences
                    .push_back((occurrence, false));
                self.stage = Stage::FinalizingControl {
                    planets: planets.to_vec(),
                    index: index + 1,
                    planet,
                    previous,
                };
                return;
            }

            self.finish_control_gain(state, ctx, &planet, previous.as_ref());
            index += 1;
        }
        self.stage = Stage::Done;
    }

    /// LRR 49: the rival structures left on a captured planet are destroyed as control changes
    /// hands — whatever Assimilate did not convert. No rival ground force can stand here: the
    /// invader holds the planet only because every one of them is dead.
    fn destroy_rival_structures(
        &self,
        state: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        planet: &PlanetId,
    ) {
        let types = catalogue(content, sources);
        let standing = state
            .system_state(&self.system)
            .planet_units
            .get(planet)
            .cloned()
            .unwrap_or_default();
        let doomed: Vec<Unit> = standing
            .into_iter()
            .filter(|unit| {
                unit.owner != self.invader
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_structure)
            })
            .collect();
        if !doomed.is_empty() {
            state
                .system_mut(&self.system)
                .remove_from_planet(planet, &doomed);
        }
    }

    fn finish_control_gain(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        previous: Option<&PlayerId>,
    ) {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
                let (invader, system) = (self.invader.clone(), self.system.clone());
                let args = crate::dual_window::ControlArgs { sources: ctx.sources, invader: &invader, system: &system, planet, previous, announce: ctx.timing.is_some() };
                if let Some(why) = crate::dual_window::control_gain_unported(state, ctx.content, &args) {
                    crate::dual::note_gate(crate::dual::Area::Invasion, "invasion::finish_control_gain", why);
                    self.control_gain_chain(state, ctx, planet, previous);
                } else {
                    {
                        let cell = std::cell::RefCell::new(&mut *self);
                        let ctx = std::cell::RefCell::new(&mut *ctx);
                        crate::dual_window!(
                            crate::dual::Area::Invasion,
                            "invasion::finish_control_gain",
                            state,
                            |s: &mut GameState| {
                                let mut ctx = ctx.borrow_mut();
                                cell.borrow_mut().control_gain_chain(s, &mut **ctx, planet, previous);
                                crate::dual::Run::new(())
                            },
                            |s: &mut GameState, script: &crate::dual_window::Recording| crate::dual_window::control_gained(s, &args, script)
                        );
                    }
                }
                self.control_gain_exploration(state, ctx, planet, previous);
                return;
            }
        }
        self.control_gain_chain(state, ctx, planet, previous);
        self.control_gain_exploration(state, ctx, planet, previous);
    }

    /// The chain of a planet that changed hands, up to the exploration (package W8, 5a: the window `dual_window::control_gained`).
    fn control_gain_chain(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        previous: Option<&PlayerId>,
    ) {
        let (content, sources) = (ctx.content, ctx.sources);
        // L1Z1X's Assimilate converts the structures on a planet as it changes hands, before
        // anything else looks at what is standing on it.
        crate::faction_abilities::control_gained(
            state,
            content,
            sources,
            &self.invader,
            &self.system,
            planet,
        );
        if state.rules_version < ti4_model::state::RULES_INFILTRATE_FIX {
            self.destroy_rival_structures(state, content, sources, planet);
        }

        // Two printed windows read "when you gain control of a planet", so a capture is
        // announced before the exploration that follows it. The former holder is named for
        // the window that reads "a planet you control": control has already changed by the
        // time the event is read, so only this frame knows who the planet was taken from.
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("player".to_owned(), self.invader.to_string().into());
        payload.insert("planet".to_owned(), planet.to_string().into());
        payload.insert("system".to_owned(), self.system.to_string().into());
        // The window that follows cannot read this event's payload, so the frame names the
        // capture for it: Infiltrate acts on the planet, Reparations on its former holder.
        state.last_control_gained = Some((
            self.system.clone(),
            planet.clone(),
            self.invader.clone(),
            previous.cloned(),
        ));
        if let Some(previous) = previous {
            payload.insert("previous_owner".to_owned(), previous.to_string().into());
        }
        let _ = ctx.emit(state, "PLANET_CONTROL_GAINED", payload);

        // This waits for the capture window to close, because Infiltrate ("when you gain
        // control") replaces the rival structures and so they must still stand while it is
        // offered (rules version 1; a version 0 game destroyed them before the window).
        if state.rules_version >= ti4_model::state::RULES_INFILTRATE_FIX {
            self.destroy_rival_structures(state, content, sources, planet);
        }

        // Technology AFTER windows resolve before exploration. Integrated Economy is the first.
        let _ = crate::technology::control_gained(
            state,
            ctx.content,
            ctx.sources,
            None,
            ctx.table,
            &self.invader,
            &self.system,
            planet,
        );
        // Jupiter Brain: taking Thunder's Edge off somebody hands the taker their breakthrough.
        crate::legendary::control_gained(state, ctx.content, ctx.sources, &self.invader, planet);

        self.control_gain_fracture_relic(state, ctx, planet, previous);
    }

    /// Fracture rule 15 (package B5): the first claim of a Fracture planet gains a relic.
    fn control_gain_fracture_relic(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        previous: Option<&PlayerId>,
    ) {
        let content = ctx.content;
        let sources = ctx.sources;
        // Fracture rule 15: the first player to take one of its planets gains a relic. `previous`
        // is captured before control changed, so it distinguishes the first claim from a later
        // conquest without trying to infer history from the now-updated control map.
        if previous.is_none()
            && crate::fracture::is_fracture_system(content, sources, &self.system)
            && let Some(relic) = crate::relics::gain(state, &self.invader)
        {
            let mut payload = std::collections::BTreeMap::new();
            payload.insert("player".to_owned(), self.invader.to_string().into());
            payload.insert("planet".to_owned(), planet.to_string().into());
            payload.insert("relic".to_owned(), relic.to_string().into());
            let _ = ctx.emit(state, "FRACTURE_RELIC_GAINED", payload);
        }
    }

    /// 35.1 on a first claim: choose the deck (two-trait planets ask) and explore the planet.
    fn control_gain_exploration(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &PlanetId,
        previous: Option<&PlayerId>,
    ) {
        // 35.1: a planet nobody controlled is explored; one taken off another player is not.
        // Only this frame knows which, which is why `captured` carries the previous holder — a
        // caller told merely that control changed would explore every conquest and draw cards
        // the rules do not give.
        //
        // `choose_deck` rather than `trait_of`: a planet with two printed traits (several
        // Thunder's Edge worlds do) is the invader's choice of deck, not a fixed pick of
        // whichever trait happens to sort first.
        if previous.is_none() {
            crate::dual::note_gate(
                crate::dual::Area::Invasion,
                "invasion::control_gain_exploration",
                "the exploration of a first claim is the string's (its steps are dual writes of the exploration area; the reveal and its rewards are W13)",
            );
        }
        if previous.is_none()
            && let Some(deck) = crate::exploration::choose_deck(ctx, state, &self.invader, planet)
            && let Some(outcome) =
                crate::exploration::explore_with(state, ctx, &self.invader, &deck, Some(planet))
        {
            self.report.explored.push((planet.clone(), outcome));
        }
    }
}

impl InvasionWindow {
    /// The coexistence target question of the bombardment stage (7, 7.1), asked once per bombarding
    /// unit and narrowed to the players still holding units on the planet (7.2).
    fn ask_bombardment_target(&self, state: &GameState, cursor: &BombardCursor) -> Option<Choice> {
        let BombardCursor {
            planet,
            groups,
            next_group,
            ..
        } = cursor;
        // Coexistence 7, 7.1, asked once per bombarding unit (7.1), with the choices
        // narrowed to the players still holding units on the planet (7.2 makes the
        // rest of a unit's hits waste if its chosen target runs out).
        let Some(next) = (*next_group..groups.len()).find(|&i| groups[i] > 0) else {
            // settle() finishes the planet; present nothing so the driver settles it.
            return None;
        };
        let present: std::collections::BTreeSet<PlayerId> = state
            .system_state(&self.system)
            .on_planet(planet)
            .iter()
            .filter(|unit| unit.owner != self.invader)
            .map(|unit| unit.owner.clone())
            .collect();
        let hits = groups[next];
        let choice = bombardment_target_question(
            &self.invader,
            &self.system,
            planet,
            hits,
            &present,
            state.system_state(&self.system).on_planet(planet),
        )
        .map(|question| {
            question.contextualized(
                DecisionContext::new(
                    self.invader.clone(),
                    DecisionSource::Rule("Coexistence 7.2".to_owned()),
                    "bombardment_target",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::Planet {
                    system: self.system.clone(),
                    planet: planet.clone(),
                }),
            )
        });
        // On `invasion=dual` the core `BombardTarget` window puts the same question.
        #[cfg(feature = "typed-core")]
        if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
            crate::dual::run_dual_ask(crate::dual::Area::Invasion, "invasion::bombard_target_ask", choice.as_ref(), || {
                crate::dual_window::bombard_target_ask(state, &self.invader, &self.system, planet, hits, &present)
            });
        }
        choice
    }

    /// Coexistence 12: the winner may start another ground combat against the next coexister.
    fn ask_next_combat(
        &self,
        state: &GameState,
        planet: &PlanetId,
        remaining: &[PlayerId],
    ) -> Option<Choice> {
        let next = remaining.first()?;
        let choice = Choice::new(
                self.invader.clone(),
                format!("start another ground combat on {planet}"),
                vec![
                    ChoiceOption::labelled(
                        format!("fight|{next}"),
                        GROUND_CASUALTY_KIND,
                        format!("fight {next} on {planet}"),
                    ),
                    ChoiceOption::labelled(
                        "decline",
                        crate::choice::DECLINE_KIND,
                        format!("leave {next} coexisting on {planet}"),
                    ),
                ],
            )
            .contextualized(
                DecisionContext::new(
                    self.invader.clone(),
                    DecisionSource::Rule("Coexistence".to_owned()),
                    "start_next_ground_combat",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::Planet {
                    system: self.system.clone(),
                    planet: planet.clone(),
                }),
            );
        // On `invasion=dual` the core `NextCombat` window puts the same question.
        #[cfg(feature = "typed-core")]
        if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
            crate::dual::run_dual_ask(crate::dual::Area::Invasion, "invasion::next_combat_ask", Some(&choice), || {
                crate::dual_window::next_combat_ask(state, &self.invader, &self.system, planet, next)
            });
        }
        Some(choice)
    }

    /// 27.2: the custodians offer; falls through to the commit question when it does not apply.
    fn ask_custodians(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Option<Choice> {
        // Falls through rather than returning None: the driver stops the moment a window
        // has no choice, so a stage that is merely inapplicable would end the invasion
        // before any ground force was committed.
        if !custodians_removable(state, content, sources, &self.invader, &self.system) {
            return self.committing_choice(state, content, sources);
        }
        // OBS-008b6: 27.3 is a payment (six influence) plus a capped victory-point gain.
        // `custodians_removable` already proved the six influence is affordable, so
        // `deterministic::spend` mirrors the same arithmetic `production::pay` will use.
        let vp = i64::from(
            state
                .player(&self.invader)
                .map_or(0, |seat| seat.victory_points),
        );
        let vp_after = (vp + 1).min(i64::from(crate::objectives::VICTORY_TARGET));
        let cost = custodians_cost(state, content, &self.invader);
        let spend = deterministic::spend(
            state,
            content,
            sources,
            &self.invader,
            cost,
            Spend::Influence,
        );
        let yes_preview = match spend.outcome {
            crate::preview::Outcome::Certain { mut deltas } => {
                deltas.push(Delta::new(Quantity::VictoryPoints, vp, vp_after));
                Preview::certain(deltas)
            }
            // Fail closed: `custodians_removable` already guards affordability, so this
            // should not be reached in practice, but an unaffordable or unmodelled plan
            // must not silently claim the victory point either.
            _ => spend,
        };
        Some(
            Choice::new(
                self.invader.clone(),
                if cost == 0 {
                    "remove the custodians token (no influence required)".to_owned()
                } else {
                    format!("spend {cost} influence to remove the custodians token")
                },
                vec![
                    ChoiceOption::labelled("no", "decline", "leave it").previewed(
                        Preview::certain(vec![Delta::new(Quantity::VictoryPoints, vp, vp)]),
                    ),
                    ChoiceOption::labelled(
                        "yes",
                        "custodians",
                        "remove it for a victory point",
                    )
                    .previewed(yes_preview),
                ],
            )
            .contextualized(
                DecisionContext::new(
                    self.invader.clone(),
                    DecisionSource::Rule("27.2".to_owned()),
                    "remove_custodians",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::System(self.system.clone())),
            ),
        )
    }

    /// One ground combat round on `planets[index]`; the roll itself happens on the answer.
    fn ask_fight_round(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        planets: &[PlanetId],
        index: usize,
    ) -> Option<Choice> {
        // One casualty decision at a time; the roll itself happens on resolve.
        let planet = planets.get(index)?;
        // Unreachable in practice — resolve_ground_round leaves the stage the moment a
        // side has no ground forces left — but guard on the same predicate as the fight.
        if !ground_force_owners(state, content, sources, &self.system, planet)
            .contains(&self.invader)
        {
            return None;
        }
        let choice = Choice::new(
            self.invader.clone(),
            format!("fight a round on {planet}"),
            vec![ChoiceOption::labelled(
                "fight",
                GROUND_CASUALTY_KIND,
                format!("fight a round on {planet}"),
            )],
        )
        .contextualized(
            DecisionContext::new(
                self.invader.clone(),
                DecisionSource::Rule("42".to_owned()),
                "fight_ground_combat_round",
                state.phase,
                state.round,
            )
            .about(DecisionTarget::Planet {
                system: self.system.clone(),
                planet: planet.clone(),
            }),
        );
        // On `invasion=dual` the core `FightRound` window puts the same question (the round itself stays string).
        #[cfg(feature = "typed-core")]
        if crate::dual::backend(crate::dual::Area::Invasion) == crate::dual::Backend::Dual {
            crate::dual::run_dual_ask(crate::dual::Area::Invasion, "invasion::fight_round_ask", Some(&choice), || {
                crate::dual_window::fight_round_ask(state, sources, &self.invader, &self.system, planet)
            });
        }
        Some(choice)
    }

    /// [`Self::answer_bombardment_target`], which on `invasion=dual` also runs as the core `Bombardment` window
    /// answering its own question: the recorded pending question and its answer, the same hits window, hits, events
    /// and place where the step stops.
    fn answer_bombardment_target_windowed(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        choice: &Choice,
        option: &ChoiceOption,
        cursor: BombardCursor,
    ) {
        #[cfg(feature = "typed-core")]
        {
            let sources = ctx.sources;
            let input = self.bombard_input();
            let (invader, system, occurrence, announce) = (self.invader.clone(), self.system.clone(), self.bombard_occurrence.0, ctx.timing.is_some());
            let (kills_before, scoring_before) = (self.report.bombardment_kills, self.pending_scoring_occurrences.len());
            let cursor_in = self.bombard_cursor_in();
            let cell = std::cell::RefCell::new(&mut *self);
            let ctx = std::cell::RefCell::new(ctx);
            let cursor = std::cell::RefCell::new(Some(cursor));
            crate::dual_window!(
                crate::dual::Area::Invasion,
                "invasion::bombard_answer",
                state,
                |s: &mut GameState| {
                    crate::dual_window::note_ask(choice, &Ok(option.clone()));
                    let cursor = cursor.borrow_mut().take().expect("run once");
                    cell.borrow_mut().answer_bombardment_target(s, &mut **ctx.borrow_mut(), option, cursor);
                    crate::dual::Run::new(cell.borrow().bombard_output(kills_before, scoring_before))
                },
                |s: &mut GameState, script: &crate::dual_window::Recording| {
                    let a = crate::dual_window::BombardIn {
                        invader: &invader,
                        system: &system,
                        occurrence,
                        announce,
                        plan: &input.0,
                        index: input.1,
                        cursor: cursor_in.as_ref(),
                        answering: true,
                    };
                    crate::dual_window::bombard(s, sources, &a, script)
                }
            );
        }
        #[cfg(not(feature = "typed-core"))]
        {
            let _ = choice;
            self.answer_bombardment_target(state, ctx, option, cursor);
        }
    }

    /// The invader's answer to the coexistence target question: that unit's hits go to `option`.
    fn answer_bombardment_target(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        option: &ChoiceOption,
        cursor: BombardCursor,
    ) {
        let BombardCursor {
            planet_index,
            planet,
            groups,
            next_group,
            taken,
            held,
            victims,
            occurrence,
        } = cursor;
        let (content, sources) = (ctx.content, ctx.sources);
        // Coexistence 7, 7.1: apply the chosen target to this bombarding unit's hits.
        // 7.2 caps the destruction at the chosen player's remaining units.
        let next = (next_group..groups.len())
            .find(|&i| groups[i] > 0)
            .expect("paused on a bombardment that had hits to assign");
        let target = PlayerId::new(option.id.clone());
        let mut log = Vec::new();
        let produced = ground_hits_window(
            state,
            ctx,
            &self.system,
            &planet,
            &target,
            groups[next],
            "bombardment",
        );
        let applied = take_bombard_hits(
            state,
            content,
            sources,
            &self.system,
            &planet,
            &target,
            produced,
            &mut log,
        );
        flush_events(state, ctx, log);
        self.report.bombardment_kills += applied;
        let taken = taken + applied;
        let exhausted = next + 1 >= groups.len()
            || state
                .system_state(&self.system)
                .on_planet(&planet)
                .iter()
                .find(|unit| unit.owner != self.invader)
                .is_none();
        if exhausted {
            self.complete_bombard_plan(state, &planet, taken, held, &victims, occurrence);
            self.bombard_index = planet_index + 1;
            self.stage = Stage::Bombarding;
        } else {
            self.stage = Stage::ChoosingBombardment(BombardCursor {
                planet_index,
                planet,
                groups,
                next_group: next + 1,
                taken,
                held,
                victims,
                occurrence,
            });
        }
    }

    /// The winner's answer to "start another ground combat?" (coexistence 12).
    fn answer_next_combat(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        option: &ChoiceOption,
        next: NextCombat,
    ) {
        let NextCombat {
            planets,
            index,
            planet,
            mut remaining,
        } = next;
        // Coexistence 12: the winner "may start an additional ground combat against
        // another coexisting player … until they decline to start another ground combat or
        // there are no more coexisting players. Any coexisting players that they do not
        // resolve a ground combat against remain coexisting."
        //
        // Declining therefore ends the chain outright rather than skipping one opponent:
        // the rule offers *another* combat, not a queue to work through.
        // Declining and having nobody left are the same outcome: the chain ends and the
        // planet is done. Rule 12 phrases them as two ways to stop, not two results.
        if option.is_decline() || remaining.is_empty() {
            self.stage = Stage::Advancing {
                planets,
                index: index + 1,
            };
        } else {
            let defender = remaining.remove(0);
            // The chosen coexister stops coexisting for the duration: they are now a
            // side in a combat, and `ground_force_owners` already sees their units.
            state
                .system_mut(&self.system)
                .coexisting
                .entry(planet.clone())
                .or_default()
                .remove(&defender);
            if self.start_ground_combat(state, ctx, &planet, &defender) {
                self.current_ground_occurrence = Some(state.begin_feat_occurrence());
                self.stage = Stage::Fighting {
                    planets,
                    index,
                    defender,
                };
            } else {
                // A start-of-combat handler removed a side's last ground force.
                self.stage = Stage::Advancing { planets, index };
            }
        }
    }

    /// [`Self::answer_custodians`], which on `invasion=dual` also runs as the core `Custodians` window: the
    /// pending question is recorded with its answer, the core puts the same question, takes the same answer and
    /// must leave the same state. The core bows out of a paid removal (the payment asks which planets to
    /// exhaust: package B3) and of anything nested; see `dual_window::custodians`.
    fn answer_custodians_windowed(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        choice: &Choice,
        option: &ChoiceOption,
    ) -> Result<(), IllegalChoice> {
        #[cfg(feature = "typed-core")]
        {
            let (content, sources) = (ctx.content, ctx.sources);
            let invader = self.invader.clone();
            let system = self.system.clone();
            let cost = custodians_cost(state, content, &invader);
            let affordable = cost == 0 || crate::production::available(state, content, sources, &invader, Spend::Influence) >= cost;
            let mut outcome = None;
            let failed = std::cell::Cell::new(false);
            crate::dual_window!(
                crate::dual::Area::Invasion,
                "invasion::answer_custodians",
                state,
                |s: &mut GameState| {
                    crate::dual_window::note_ask(choice, &Ok(option.clone()));
                    let result = self.answer_custodians(s, ctx, option);
                    failed.set(result.is_err());
                    outcome = Some(result);
                    crate::dual::Run::new(())
                },
                |s: &mut GameState, script: &crate::dual_window::Recording| {
                    if failed.get() {
                        return None;
                    }
                    crate::dual_window::custodians(s, sources, &invader, &system, affordable, script)
                }
            );
            return outcome.unwrap_or(Ok(()));
        }
        #[cfg(not(feature = "typed-core"))]
        self.answer_custodians(state, ctx, option)
    }

    /// The answer to the custodians offer (27.2, 27.3).
    fn answer_custodians(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        option: &ChoiceOption,
    ) -> Result<(), IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        if !option.is_decline() {
            // 27.3: pay six influence, take the token, gain a victory point.
            let cost = custodians_cost(state, content, &self.invader);
            if cost == 0
                || crate::production::pay(
                    state,
                    content,
                    sources,
                    ctx.table,
                    &self.invader,
                    cost,
                    crate::production::Spend::Influence,
                )?
            {
                state.custodians_removed = true;
                crate::objectives::adjust_victory_points(
                    state,
                    &self.invader,
                    1,
                    "custodians",
                );
                self.report.custodians_removed = true;
            }
        }
        self.stage = Stage::Committing;
        Ok(())
    }

    /// The answer at the commit step. `Ok(false)` is an answer that changed nothing and leaves
    /// the step as it was (the caller then skips the "nothing left to land" check).
    fn answer_commit(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        choice: &Choice,
        option: &ChoiceOption,
    ) -> Result<bool, IllegalChoice> {
        if option.is_decline() {
            self.finish_committing(state, ctx)?;
        } else if let Some(rest) = option.id.strip_prefix("commit|") {
            return self.commit_one_windowed(state, ctx, choice, option, rest, false);
        } else if let Some(planet) = option.id.strip_prefix("deploy_commit|") {
            return self.commit_one_windowed(state, ctx, choice, option, planet, true);
        }
        Ok(true)
    }

    /// [`Self::commit_one`], which on `invasion=dual` also runs as the core `LandOne` window for an ordinary
    /// candidate: the pending question is recorded with its answer, the core puts the same question, lands the
    /// same unit and announces the same `UNITS_COMMITTED`. The core bows out of a pool with an origin, a
    /// temporary space commit or the Crimson deploy (`plain`), of the rest see `dual_window::land_one`.
    fn commit_one_windowed(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        choice: &Choice,
        option: &ChoiceOption,
        rest: &str,
        deploy_answer: bool,
    ) -> Result<bool, IllegalChoice> {
        #[cfg(feature = "typed-core")]
        {
            let (content, sources) = (ctx.content, ctx.sources);
            let (invader, system) = (self.invader.clone(), self.system.clone());
            let (troops_pool, origins, temporary) = commit_pool(state, content, sources, self.galaxy.as_deref(), &invader, &system, &self.extra_commits);
            let planets = landable_planets(state, content, sources, &system);
            let deploy = crate::factions::crimson::deploy_planets(state, content, sources, &invader, &system, &planets);
            let plain: Option<&'static str> = None;
            let pool: Vec<crate::dual_window::PoolIn> = troops_pool
                .into_iter()
                .zip(origins)
                .zip(temporary)
                .map(|((unit, origin), temporary)| crate::dual_window::PoolIn { unit, origin, temporary })
                .collect();
            let announce = ctx.timing.is_some();
            let mut outcome = None;
            let failed = std::cell::Cell::new(false);
            let cell = std::cell::RefCell::new(&mut *self);
            let ctx = std::cell::RefCell::new(ctx);
            crate::dual_window!(
                crate::dual::Area::Invasion,
                "invasion::commit_one",
                state,
                |s: &mut GameState| {
                    crate::dual_window::note_ask(choice, &Ok(option.clone()));
                    let (extras, temps) = {
                        let me = cell.borrow();
                        (me.extra_commits.len(), me.temporary_space_commits.values().map(Vec::len).sum::<usize>())
                    };
                    let result = if deploy_answer {
                        cell.borrow_mut().commit_deploy(s, &mut **ctx.borrow_mut(), rest)
                    } else {
                        cell.borrow_mut().commit_one(s, &mut **ctx.borrow_mut(), rest)
                    };
                    failed.set(result.is_err() || matches!(result, Ok(false)));
                    outcome = Some(result);
                    let me = cell.borrow();
                    crate::dual::Run::new(crate::dual_window::LandOut {
                        origin: me.extra_commits.get(extras).map(|(a, b)| (a.to_string(), b.to_string())),
                        temporary: me.temporary_space_commits.values().map(Vec::len).sum::<usize>() > temps,
                    })
                },
                |s: &mut GameState, script: &crate::dual_window::Recording| {
                    if failed.get() {
                        return None;
                    }
                    crate::dual_window::land_one(s, sources, &invader, &system, plain, announce, &pool, &deploy, script)
                }
            );
            return outcome.unwrap_or(Ok(true));
        }
        #[cfg(not(feature = "typed-core"))]
        if deploy_answer {
            self.commit_deploy(state, ctx, rest)
        } else {
            self.commit_one(state, ctx, rest)
        }
    }

    /// One `commit|<index>|<planet>` answer: toll, move, land, announce.
    fn commit_one(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        rest: &str,
    ) -> Result<bool, IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
            let mut parts = rest.splitn(2, '|');
            let (Some(index), Some(planet)) = (
                parts.next().and_then(|i| i.parse::<usize>().ok()),
                parts.next().map(PlanetId::new),
            ) else {
                return Ok(false);
            };
            let (troops, origins, temporary) = commit_pool(
                state,
                content,
                sources,
                self.galaxy.as_deref(),
                &self.invader,
                &self.system,
                &self.extra_commits,
            );
            if let Some(unit) = troops.get(index).cloned() {
                // Omniopiares: the first landing on the planet pays its toll, chosen by the
                // invader like any influence spend. Unpayable landings were never offered;
                // should one arrive anyway nothing changes.
                if !crate::factions::keleres_units::pay_commit_toll(
                    state,
                    content,
                    sources,
                    self.galaxy.as_deref(),
                    ctx.table,
                    &self.invader,
                    &self.system,
                    &planet,
                )? {
                    return Ok(false);
                }
                let origin = origins.get(index).cloned().flatten();
                let temporary = temporary.get(index).copied().unwrap_or(false);
                // Parley reads the landing back through this marker: the emission's
                // AFTER window resolves before the commit step continues, and the
                // marker is the one fact the effect can trust.
                state.last_committed_unit = Some((
                    self.invader.clone(),
                    self.system.clone(),
                    planet.clone(),
                    unit.clone(),
                ));
                // The engine does the move: out of the space area (the ordinary source) or
                // off the planet a faction's hook named, then onto the target planet.
                match &origin {
                    None => {
                        state
                            .system_mut(&self.system)
                            .remove(std::slice::from_ref(&unit));
                    }
                    Some((from_system, from_planet)) => {
                        state
                            .system_mut(from_system)
                            .remove_from_planet(from_planet, std::slice::from_ref(&unit));
                    }
                }
                land_unit(
                    state,
                    content,
                    sources,
                    &self.invader,
                    &self.system,
                    &planet,
                    unit,
                );
                if temporary {
                    self.temporary_space_commits
                        .entry(planet.clone())
                        .or_default()
                        .push(troops[index].clone());
                }
                if let Some(origin) = origin.clone() {
                    self.extra_commits.push(origin);
                }
                self.announce_landing(state, ctx, planet, origin);
            }
        Ok(true)
    }

    /// The Crimson Revenant's `deploy_commit|<planet>` answer.
    fn commit_deploy(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planet: &str,
    ) -> Result<bool, IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
            // The Crimson Revenant's DEPLOY: "you may commit 1 mech, even if you have no
            // units in the system". The mech comes from reinforcements and lands as a
            // committed ground force.
            let planet = PlanetId::new(planet);
            let landable = landable_planets(state, content, sources, &self.system);
            if !crate::factions::crimson::deploy_planets(
                state,
                content,
                sources,
                &self.invader,
                &self.system,
                &landable,
            )
            .contains(&planet)
            {
                return Ok(false);
            }
            if !crate::factions::keleres_units::pay_commit_toll(
                state,
                content,
                sources,
                self.galaxy.as_deref(),
                ctx.table,
                &self.invader,
                &self.system,
                &planet,
            )? {
                return Ok(false);
            }
            let unit = Unit::new(
                ti4_model::id::UnitTypeId::new(crate::factions::crimson::MECH),
                self.invader.clone(),
            );
            state.last_committed_unit = Some((
                self.invader.clone(),
                self.system.clone(),
                planet.clone(),
                unit.clone(),
            ));
            land_unit(
                state,
                content,
                sources,
                &self.invader,
                &self.system,
                &planet,
                unit,
            );
            crate::factions::crimson::note_deployed(state, &self.invader);
            self.announce_landing(state, ctx, planet, None);
        Ok(true)
    }

    /// The answer to "fight a round": roll and resolve one ground combat round.
    fn answer_fight(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        planets: Vec<PlanetId>,
        index: usize,
        defender: PlayerId,
    ) -> Result<(), IllegalChoice> {
        self.resolve_ground_round(state, ctx, planets, index, defender);
        if let Some(error) = self.strict_timing_error.take() {
            return Err(crate::combat::CombatError::Timing(error).into_illegal_choice());
        }
        Ok(())
    }
}

impl Window for InvasionWindow {
    /// A fresh window starts on its bombardment, and bombardment advances by
    /// [`Self::settle`] rather than by any player choice, so driving one settles before the
    /// first question and after every answer that leaves nothing to ask.
    ///
    /// Drive stops once a scoring occurrence is queued: that pause belongs to the caller (the
    /// driver opens the occurrence's scoring window there), exactly as between its own steps.
    fn drive(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), IllegalChoice> {
        self.settle(state, ctx);
        self.take_settle_error()?;
        while self.pending_scoring_occurrences.is_empty() && !self.is_done() {
            if let Some(choice) = self.pending_choice(state, ctx.content, ctx.sources) {
                let answer = ctx.ask_seeing(state, &choice)?;
                self.resolve(state, ctx, answer)?;
            } else {
                self.settle(state, ctx);
                self.take_settle_error()?;
            }
        }
        Ok(())
    }

    fn pending_choice(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Option<Choice> {
        match &self.stage {
            Stage::Done
            | Stage::Bombarding
            | Stage::Advancing { .. }
            | Stage::FinalizingControl { .. } => None,
            Stage::ChoosingBombardment(cursor) => self.ask_bombardment_target(state, cursor),
            Stage::ChoosingNextCombat {
                planet, remaining, ..
            } => self.ask_next_combat(state, planet, remaining),
            Stage::Custodians => self.ask_custodians(state, content, sources),
            Stage::Committing => self.committing_choice(state, content, sources),
            Stage::Fighting { planets, index, .. } => {
                self.ask_fight_round(state, content, sources, planets, *index)
            }
        }
    }

    fn resolve(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        answer: ChoiceOption,
    ) -> Result<(), IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        if self.galaxy.is_none()
            && let Some(galaxy) = ctx.timing.as_ref().and_then(|handle| handle.galaxy)
        {
            self.galaxy = Some(std::sync::Arc::new(galaxy.clone()));
        }
        let Some(choice) = self.pending_choice(state, content, sources) else {
            return Ok(());
        };
        let option = crate::choice::validate(&choice, answer)?;

        match self.stage.clone() {
            Stage::Done
            | Stage::Bombarding
            | Stage::Advancing { .. }
            | Stage::FinalizingControl { .. } => {}
            Stage::ChoosingBombardment(cursor) => {
                self.answer_bombardment_target_windowed(state, ctx, &choice, &option, cursor);
            }
            Stage::ChoosingNextCombat {
                planets,
                index,
                planet,
                remaining,
            } => {
                self.answer_next_combat(
                    state,
                    ctx,
                    &option,
                    NextCombat {
                        planets,
                        index,
                        planet,
                        remaining,
                    },
                );
            }
            Stage::Custodians
                if !custodians_removable(state, content, sources, &self.invader, &self.system) =>
            {
                // The ask that was actually answered was the commit one, reached by fall-through.
                self.stage = Stage::Committing;
                return self.resolve(state, ctx, option);
            }
            Stage::Custodians => self.answer_custodians_windowed(state, ctx, &choice, &option)?,
            Stage::Committing => {
                if !self.answer_commit(state, ctx, &choice, &option)? {
                    return Ok(());
                }
            }
            Stage::Fighting {
                planets,
                index,
                defender,
            } => self.answer_fight(state, ctx, planets, index, defender)?,
        }

        // Committing settles when nothing is left to land.
        if matches!(self.stage, Stage::Committing)
            && self.landing_options(state, content, sources).is_empty()
        {
            self.finish_committing(state, ctx)?;
        }
        Ok(())
    }
}

fn note_ground_combat_win_feats(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    winner: &PlayerId,
    loser: &PlayerId,
    notes_at_tactical_start: &crate::combat::NoteHoldings,
    occurrence: FeatOccurrence,
) -> bool {
    let mut noted = false;
    if ti4_content::galaxy::all_systems(content, sources)
        .get(system.as_str())
        .is_some_and(ti4_content::galaxy::System::is_anomaly)
    {
        state.record_event_feat(winner, Feat::WonInAnAnomaly, occurrence);
        noted = true;
    }
    if crate::combat::is_rival_home_system(state, winner, system) {
        state.record_event_feat(winner, Feat::WonInARivalHome, occurrence);
        noted = true;
    }
    let most = state
        .players
        .iter()
        .map(|seat| seat.victory_points)
        .max()
        .unwrap_or(0);
    if state
        .player(loser)
        .is_some_and(|seat| seat.victory_points == most)
    {
        state.record_event_feat(winner, Feat::WonAgainstThePointsLeader, occurrence);
        noted = true;
    }
    let holds_note = notes_at_tactical_start
        .get(winner)
        .is_some_and(|issuers| issuers.contains(loser));
    if holds_note {
        state.record_event_feat(winner, Feat::WonAgainstANoteHolder, occurrence);
        noted = true;
    }
    noted
}

/// Remove `hits` of a player's ground forces from a planet, weakest-first.
///
/// No choice is offered: every ground force on a planet is interchangeable in this model, so
/// asking would be a decision between identical options.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn remove_ground(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    hits: usize,
    cause: &'static str,
    log: &mut Vec<PendingEvent>,
) {
    #[cfg(feature = "typed-core")]
    {
        // On `invasion=dual` the removal also lands through the core's hits step (`rules::invasion::hits`).
        let queued = std::cell::RefCell::new(Vec::new());
        crate::dual_window!(
            crate::dual::Area::Invasion,
            "invasion::remove_ground",
            state,
            |s: &mut GameState| {
                let mut own = Vec::new();
                remove_ground_text(s, content, sources, system, planet, player, hits, cause, &mut own);
                let rows = own.iter().map(|(n, p)| ((*n).to_owned(), p.clone())).collect();
                queued.borrow_mut().extend(own);
                crate::dual::Run::new(crate::dual_window::HitsOut { remaining: hits, destroyed: 0, log: rows })
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                let args = crate::dual_window::HitsArgs {
                    victim: player,
                    system,
                    planet,
                    hits,
                    cause,
                    unit_ability: matches!(cause, "bombardment" | "space_cannon_defense" | "harrow"),
                    open: false,
                    apply: true,
                    announce: false,
                };
                crate::dual_window::ground_hits(s, sources, args, script).map(|mut w| {
                    w.run.value.destroyed = 0;
                    w
                })
            }
        );
        log.extend(queued.into_inner());
        return;
    }
    #[cfg(not(feature = "typed-core"))]
    remove_ground_text(state, content, sources, system, planet, player, hits, cause, log)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
fn remove_ground_text(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    planet: &PlanetId,
    player: &PlayerId,
    hits: usize,
    cause: &'static str,
    log: &mut Vec<PendingEvent>,
) {
    let mut remaining = hits;
    while remaining > 0 {
        remaining -= 1;
        let sustained = ground_hit_outcome_logged(
            state,
            content,
            sources,
            system,
            planet,
            player,
            cause,
            log,
            matches!(cause, "bombardment" | "space_cannon_defense" | "harrow"),
        );
        if matches!(sustained, GroundHit::Sustained(_)) {
            remaining = remaining.saturating_sub(sustain_extra(state, player));
        }
    }
}

/// "Before a hit would be assigned" to `victim`'s ground forces on `planet`: open the window and
/// spend what a card played in it (Titans' Tellurian) cancelled. Returns the hits that remain.
///
/// One window per batch of hits (a combat round's, a bombardment unit's, a space cannon
/// defense's), shaped like the space combat `HITS_TO_ASSIGN`. Without a timing handle no window
/// opens and nothing is cancelled, as everywhere else in this file.
fn ground_hits_window(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    planet: &PlanetId,
    victim: &PlayerId,
    hits: usize,
    cause: &'static str,
) -> usize {
    if hits == 0 {
        return 0;
    }
    #[cfg(feature = "typed-core")]
    {
        // On `invasion=dual` the window also opens in the core (`rules::invasion::hits`): the same event, the same
        // cancellations spent. A card played in it (nested activity) is left to the string answer.
        let (sources, announce) = (ctx.sources, ctx.timing.is_some());
        return crate::dual_window!(
            crate::dual::Area::Invasion,
            "invasion::ground_hits_window",
            state,
            |s: &mut GameState| {
                let remaining = ground_hits_window_text(s, ctx, system, planet, victim, hits, cause);
                crate::dual::Run::new(crate::dual_window::HitsOut { remaining, destroyed: 0, log: Vec::new() })
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                let args = crate::dual_window::HitsArgs { victim, system, planet, hits, cause, unit_ability: false, open: true, apply: false, announce };
                crate::dual_window::ground_hits(s, sources, args, script)
            }
        )
        .remaining;
    }
    #[cfg(not(feature = "typed-core"))]
    ground_hits_window_text(state, ctx, system, planet, victim, hits, cause)
}

fn ground_hits_window_text(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    planet: &PlanetId,
    victim: &PlayerId,
    hits: usize,
    cause: &'static str,
) -> usize {
    let payload = std::collections::BTreeMap::from([
        ("system".to_owned(), system.to_string().into()),
        ("planet".to_owned(), planet.to_string().into()),
        ("player".to_owned(), victim.to_string().into()),
        ("hits".to_owned(), i64::try_from(hits).unwrap_or(0).into()),
        ("cause".to_owned(), cause.into()),
    ]);
    crate::combat::open_hits_window(state, ctx, GROUND_HITS_TO_ASSIGN, payload, victim, hits)
}

/// Event opened before a batch of hits lands on ground forces (ground combat, bombardment, space
/// cannon defense, Harrow); payload `system`, `planet`, `player` (the victim), `hits`, `cause`.
pub const GROUND_HITS_TO_ASSIGN: &str = "GROUND_HITS_TO_ASSIGN";

/// Space cannon defense: after ground forces are committed, the other players' SPACE CANNON
/// units on the planet fire at them (PDS, and the Xxcha mech). Disable strips an opponent's PDS
/// of SPACE CANNON for the invasion, as it does for space cannon offense.
fn space_cannon_defense(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    planet: &PlanetId,
    invader: &PlayerId,
) {
    let (content, sources) = (ctx.content, ctx.sources);
    if !crate::entropic_scars::abilities_usable(content, sources, system, Some(system)) {
        return;
    }
    // L4 Disruptors (Letnev): "During an invasion, units cannot use SPACE CANNON against your
    // units." Space cannon defense is the only cannon fire an invasion has.
    if crate::technology::has_technology_text(state, invader, "l4") {
        return;
    }
    let types = catalogue(content, sources);
    let guns: Vec<Unit> = state
        .system_state(system)
        .on_planet(planet)
        .iter()
        .filter(|unit| &unit.owner != invader)
        // Quietus: a gun in an active breach beside another player's Quietus has lost SPACE CANNON.
        .filter(|unit| !crate::factions::crimson::abilities_lost(state, &unit.owner, system))
        .cloned()
        .collect();
    let before = state.system_state(system).on_planet(planet).to_vec();
    let mut rolled = Vec::new();
    let mut hits = 0;
    let mut plasma = crate::combat::plasma_picks(
        state,
        guns.iter().map(|unit| {
            (
                unit,
                types
                    .get(unit.type_id.as_str())
                    .and_then(|kind| kind.space_cannon_hits_on()),
            )
        }),
    );
    for unit in guns {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        let Some(value) = kind.space_cannon_hits_on() else {
            continue;
        };
        if kind.base_type() == "pds"
            && state.players.iter().any(|seat| {
                seat.id != unit.owner && seat.disable_invasion.contains(&state.activation_seq)
            })
        {
            continue;
        }
        let count = usize::try_from(kind.space_cannon_dice()).unwrap_or(0);
        if count == 0 {
            continue;
        }
        let count = count + crate::combat::take_plasma(&mut plasma, &unit.owner, value);
        // A galvanized unit rolls 1 additional die for its unit abilities (Last Bastion).
        let count = count + crate::combat_effects::galvanized_dice(&unit);
        // Assail: +1 to the gunner's unit ability rolls against a puppeted invader.
        let assail =
            crate::factions::firmament_plots::assail_against(state, &unit.owner, [invader]);
        let value = (value - assail).max(1);
        let roll = ctx.dice.roll_by(
            ctx.rng,
            count,
            "space cannon defense",
            Some(u32::try_from(value).unwrap_or(u32::MAX)),
            &unit.owner,
        );
        ctx.dice.annotate_source(crate::dice::RollSource {
            unit: unit.type_id.to_string(),
            planet: Some(planet.to_string()),
            system: None,
        });
        hits += roll.hits();
        for face in roll.faces {
            rolled.push(InvasionDie {
                planet: planet.clone(),
                player: unit.owner.clone(),
                group: unit.type_id.to_string(),
                face,
                target: u32::try_from(value).unwrap_or(u32::MAX),
                hit: i64::from(face) >= value,
            });
        }
    }
    // SPACE CANNON an attachment gives the planet "as if it were a unit" (Titans' Geoform).
    // Disable and Plasma Scoring do not apply to these dice: the attachment is not a PDS unit.
    for (owner, value, dice) in crate::planets::attachment_cannons(state, content, system, planet) {
        if &owner == invader {
            continue;
        }
        let roll = ctx
            .dice
            .roll_by(ctx.rng, dice, "space cannon defense", Some(value), &owner);
        ctx.dice.annotate_source(crate::dice::RollSource {
            unit: "attachment".to_owned(),
            planet: Some(planet.to_string()),
            system: None,
        });
        hits += roll.hits();
    }
    let hits = ground_hits_window(
        state,
        ctx,
        system,
        planet,
        invader,
        hits,
        "space_cannon_defense",
    );
    let mut log = Vec::new();
    remove_ground(
        state,
        content,
        sources,
        system,
        planet,
        invader,
        hits,
        "space_cannon_defense",
        &mut log,
    );
    flush_events(state, ctx, log);
    if !rolled.is_empty() {
        let after = state.system_state(system).on_planet(planet).to_vec();
        let mut produced = std::collections::BTreeMap::new();
        for die in &rolled {
            if die.hit {
                *produced.entry(die.player.clone()).or_insert(0) += 1;
            }
        }
        if let Some(active) = state.active_invasion.as_mut() {
            active.last_step = Some(InvasionStep {
                planet: planet.clone(),
                kind: "space_cannon_defense".to_owned(),
                round: 0,
                before,
                after,
                dice: rolled,
                hits: produced,
                harrow_hits: 0,
            });
        }
    }
}

/// Run a whole invasion for the active player (LRR 49).
///
/// # Errors
/// [`IllegalChoice`] when a decider answers with something not offered.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per distinct input"
)]
pub fn resolve(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    invader: &PlayerId,
) -> Result<InvasionReport, IllegalChoice> {
    let mut window = InvasionWindow::new(state, content, sources, dice, rng, invader, system);
    let mut ctx = Resolving {
        content,
        sources,
        dice,
        rng,
        table,
        timing: None,
    };
    while !window.is_done() {
        window.drive(state, &mut ctx)?;
        // The synchronous API cannot present secret-objective choices. It still records each
        // exact occurrence, then consumes the pause and completes the invasion.
        while window.take_scoring_occurrence().is_some() {}
        window.settle(state, &mut ctx);
    }
    Ok(window.into_report())
}

#[cfg(test)]
mod tests {
    /// The coexist question's card header is the breakthrough's name and its first sentence.
    #[test]
    fn a_breakthrough_card_is_its_name_and_first_sentence() {
        let card = breakthrough_card(ContentStore::embedded(), "titansbt");
        assert_eq!(card["title"], "Slumberstate Computing");
        assert_eq!(card["tag"], "breakthrough");
        assert_eq!(
            card["text"],
            "When COALESCENCE results in a ground combat, if you commit no other units, you may choose for your units to coexist instead."
        );
    }

    fn apply_ground_hit(
        state: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        system: &SystemId,
        planet: &PlanetId,
        player: &PlayerId,
    ) -> GroundHit {
        apply_ground_hit_with_origin(state, content, sources, system, planet, player, false)
    }

    /// 27.2a: no ground forces, no custodians token — and so no victory point.
    ///
    /// The token is worth a point, so a seat with six influence and an empty fleet could otherwise
    /// buy that point and commit nothing. The rule says they cannot remove the token at all.
    #[test]
    fn the_custodians_token_needs_troops_not_just_influence() {
        let content = ContentStore::embedded();
        let sources = POK;
        let player = invader();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.board.entry(mecatol.clone()).or_default();

        // Influence enough and no army: the token stays.
        if let Some(seat) = state.player_mut(&player) {
            seat.trade_goods = 0;
        }
        crate::fixtures::put(&mut state, &mecatol, "cruiser", &player, 1);
        assert!(
            !custodians_removable(&state, content, sources, &player, &mecatol),
            "a fleet with nothing to land cannot lift the token"
        );

        crate::fixtures::put(&mut state, &mecatol, "infantry", &player, 1);
        // Still needs the influence, which this seat does not have — so the guard under test is
        // proven by giving it the troops and watching the *other* condition decide.
        let with_troops = custodians_removable(&state, content, sources, &player, &mecatol);
        if let Some(seat) = state.player_mut(&player) {
            seat.trade_goods = 0;
        }
        assert_eq!(
            with_troops,
            crate::production::available(
                &state,
                content,
                sources,
                &player,
                crate::production::Spend::Influence,
            ) >= 6,
            "with troops present, only the influence decides"
        );
    }

    /// Jol-Nar's Fragile applies on the ground, not only in space.
    ///
    /// "-1 to all combat rolls" is not a space rule, and `combat_modifier` has carried a `context`
    /// parameter for exactly this. Its only caller passed "space", so ground combat rolled the
    /// printed value: Jol-Nar infantry fought at full strength on every planet in the game.
    #[test]
    fn jol_nar_infantry_are_fragile_on_the_ground_too() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena();
        let player = invader();
        if let Some(seat) = state.player_mut(&player) {
            seat.faction = ti4_model::id::FactionId::new("jolnar");
        }
        on_planet(&mut state, &system, &planet, "infantry", &player, 1);

        let printed = catalogue(content, POK)
            .get("infantry")
            .and_then(ti4_content::units::UnitType::combat_hits_on)
            .expect("infantry have a combat value");
        assert_eq!(
            ground_combat_value(&state, content, POK, &player, &system, &planet, "infantry"),
            Some(printed + 1),
            "Fragile is -1 to the roll, so the threshold rises by one"
        );

        if let Some(seat) = state.player_mut(&player) {
            seat.faction = ti4_model::id::FactionId::new("sol");
        }
        assert_eq!(
            ground_combat_value(&state, content, POK, &player, &system, &planet, "infantry"),
            Some(printed),
            "and another faction rolls the printed value"
        );
    }

    /// L1Z1X's Anihilator mech bombards from the ground, but not the planet it is standing on.
    ///
    /// "While not participating in ground combat, this unit can use its BOMBARDMENT ability on
    /// planets in its system as if it were a ship." Standing on the planet it would shoot at *is*
    /// participating, so the mech joins the bombardment of every other planet and not that one.
    #[test]
    fn an_anihilator_mech_bombards_other_planets_in_its_system() {
        let (mut state, system, planet) = arena();
        let player = invader();
        let elsewhere = PlanetId::new("a_different_planet");
        on_planet(&mut state, &system, &elsewhere, "l1z1x_mech", &player, 1);

        let from_elsewhere = bombarders(&state, &player, &system, &planet);
        assert!(
            from_elsewhere
                .iter()
                .any(|unit| unit.type_id.as_str() == "l1z1x_mech"),
            "a mech on another planet joins the bombardment"
        );

        let at_home = bombarders(&state, &player, &system, &elsewhere);
        assert!(
            !at_home
                .iter()
                .any(|unit| unit.type_id.as_str() == "l1z1x_mech"),
            "but it does not bombard the planet it is standing on"
        );
    }

    /// Jol-Nar's Shield Paling mech lifts Fragile from the infantry standing with it.
    ///
    /// One of four in-scope mech abilities that no coverage helper counted, so none of them showed
    /// up as a gap. Only infantry, only that planet: the mech itself still rolls Fragile, and
    /// infantry on the next planet over are unhelped.
    #[test]
    fn a_shield_paling_lifts_fragile_from_the_infantry_beside_it() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena();
        let player = invader();
        if let Some(seat) = state.player_mut(&player) {
            seat.faction = ti4_model::id::FactionId::new("jolnar");
        }
        on_planet(&mut state, &system, &planet, "infantry", &player, 1);
        let printed = catalogue(content, POK)
            .get("infantry")
            .and_then(ti4_content::units::UnitType::combat_hits_on)
            .expect("infantry have a combat value");

        assert_eq!(
            ground_combat_value(&state, content, POK, &player, &system, &planet, "infantry"),
            Some(printed + 1),
            "Fragile applies without the mech"
        );

        on_planet(&mut state, &system, &planet, "jolnar_mech", &player, 1);
        assert_eq!(
            ground_combat_value(&state, content, POK, &player, &system, &planet, "infantry"),
            Some(printed),
            "and the mech lifts it"
        );
        let mech_printed = catalogue(content, POK)
            .get("jolnar_mech")
            .and_then(ti4_content::units::UnitType::combat_hits_on)
            .expect("the mech has a combat value");
        assert_eq!(
            ground_combat_value(
                &state,
                content,
                POK,
                &player,
                &system,
                &planet,
                "jolnar_mech"
            ),
            Some(mech_printed + 1),
            "but only for infantry: the mech itself is still Fragile"
        );
    }

    /// 63.2: a planetary shield stops Harrow, as it stops any other bombardment.
    ///
    /// Harrow is a bombardment that fires again at the end of each ground-combat round, and the
    /// shield names it explicitly. It was firing through the shield because the faction layer asks
    /// only "does this seat have Harrow" -- the gate lives in the invasion, where the planet is
    /// known, so Harrow now goes through the same `bombardable` every other bombardment does.
    #[test]
    fn a_planetary_shield_stops_harrow() {
        let (mut state, system, planet) = arena();
        if let Some(seat) = state.player_mut(&invader()) {
            seat.faction = ti4_model::id::FactionId::new("l1z1x");
        }
        in_space(&mut state, &system, "dreadnought", &invader(), 1);

        assert!(
            bombardable(
                &state,
                ContentStore::embedded(),
                POK,
                &system,
                &planet,
                &invader()
            ),
            "with no shield the bombardment is allowed"
        );

        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        assert!(
            !bombardable(
                &state,
                ContentStore::embedded(),
                POK,
                &system,
                &planet,
                &invader()
            ),
            "and the shield stops it -- which is the gate Harrow now asks"
        );
    }
    /// Harrow is a window of its own after the removals of a round: its dice come after them, in the dice stream
    /// order of the string code, and the step report holds its hits.
    #[test]
    fn harrow_runs_as_a_core_window_after_the_removals_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        let mut harrow_dice = 0;
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "l1z1x"), ("b", "sol")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = 9;
            let (system, planet) = crate::fixtures::a_placed_planet();
            crate::fixtures::put(&mut state, &system, "warsun", &PlayerId::new("a"), 1);
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 10);
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let mut dice = Dice::new();
            let mut rng = GameRng::new(9);
            resolve(&mut state, ContentStore::embedded(), POK, &mut table, &mut dice, &mut rng, &system, &PlayerId::new("a")).unwrap();
            harrow_dice = dice.history().iter().filter(|r| r.reason == "harrow").count();
        });
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(harrow_dice > 0, "Harrow rolled");
        assert!(w.compared >= 1, "the round with Harrow is compared: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Infantry II is an ability of the core table: the defender's destroyed Infantry II opens the AFTER window of
    /// `GROUND_FORCE_DESTROYED`, the core resolver (armed from the table) finds the ability eligible and rolls the
    /// same die from the same stream, and the window is compared, not bailed.
    #[test]
    fn infantry_ii_rolls_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut rolled = 0;
        let mut on_card = 0;
        with_mask(mask, || {
            for seed in 1..=6_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                crate::technology::grant(&mut state, &PlayerId::new("b"), &ti4_model::id::TechnologyId::new("inf2"));
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry2", &PlayerId::new("b"), 6);
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let content = ContentStore::embedded();
                let mut resolver = armed_resolver(&state, vec![PlayerId::new("a"), PlayerId::new("b")]);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                drop(ctx);
                rolled += dice.history().iter().filter(|r| r.reason == "infantry II").count();
                on_card += crate::infantry_ii::units_on_card(&state, &PlayerId::new("b"));
            }
        });
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(rolled > 0, "Infantry II rolled");
        assert!(on_card > 0, "a six went on the card in one of the seeds");
        assert!(w.compared >= 1, "the rounds with Infantry II are compared: {:?}", r.windows);
        assert!(!r.windows.keys().any(|k| k.contains("unported ability technology:*:inf2")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// The standing reaction-card slot is in the core table (eligibility, the card list of the option, the label):
    /// a defender holding Fire Team is asked after the ground rolls, declines, and the window that opened the
    /// question is compared rather than bailed. Playing the card is not ported: that run is not followed.
    #[test]
    fn a_reaction_slot_that_is_declined_is_followed_by_the_core_window() {
        declined_slot_lockstep("hacan");
    }

    /// A Yssaril seat holding a reaction card is followed too: Transparasteel Plating is the kernel's hook
    /// (`economy.action_cards_forbidden`), no longer a bail of the seat.
    #[test]
    fn a_yssaril_seat_with_a_reaction_card_is_followed_by_the_core_window() {
        declined_slot_lockstep("yssaril");
    }

    fn declined_slot_lockstep(faction: &str) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut asked = 0;
        with_mask(mask, || {
            for seed in 1..=4_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", faction)], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("b")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("fire_team")];
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 5);
                let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                struct DeclineSlots(std::rc::Rc<std::cell::RefCell<Vec<Vec<String>>>>);
                impl crate::choice::Decider for DeclineSlots {
                    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                        self.0.borrow_mut().push(choice.ids().into_iter().map(String::from).collect());
                        if choice.ids().iter().any(|id| id.starts_with("reaction:")) {
                            return crate::choice::AlwaysDecline.choose(choice);
                        }
                        crate::choice::FirstOption.choose(choice)
                    }
                }
                let mut table = Table::with_default(Box::new(DeclineSlots(seen.clone())));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let content = ContentStore::embedded();
                let mut resolver = armed_resolver(&state, vec![PlayerId::new("a"), PlayerId::new("b")]);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                drop(ctx);
                asked += seen.borrow().iter().filter(|options| options.iter().any(|o| o.starts_with("reaction:"))).count();
            }
        });
        let r = report();
        assert!(asked > 0, "the slot was offered");
        assert!(r.windows.keys().all(|k| !k.contains("unported ability reaction:") && !k.contains("Transparasteel")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// The Cabal's Reanimator, Devour and The Terror Between are abilities of the core table: the ground losses of a
    /// fight the Cabal takes part in are captured by the core resolver exactly as the string one does.
    #[test]
    fn the_cabal_captures_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut captured = 0;
        for flagship in [false, true] {
            with_mask(mask, || {
                for seed in 1..=5_u64 {
                    let mut state = crate::fixtures::seated_game(&[("a", "cabal"), ("b", "sol")], POK);
                    state.promissory_notes.clear();
                    state.promissory_faceup.clear();
                    state.rng_seed = seed;
                    let (system, planet) = crate::fixtures::a_placed_planet();
                    crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                    if flagship {
                        crate::fixtures::put(&mut state, &system, "cabal_flagship", &PlayerId::new("a"), 1);
                    }
                    crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 5);
                    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 5);
                    crate::fixtures::put_on_planet(&mut state, &system, &planet, "cabal_mech", &PlayerId::new("a"), 1);
                    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                    let mut dice = Dice::new();
                    let mut rng = GameRng::new(seed);
                    let content = ContentStore::embedded();
                    let mut resolver = armed_resolver(&state, vec![PlayerId::new("a"), PlayerId::new("b")]);
                    let mut sequence = crate::event::EventSequence::new();
                    let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                    let mut ctx = Resolving {
                        content,
                        sources: POK,
                        dice: &mut dice,
                        rng: &mut rng,
                        table: &mut table,
                        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                    };
                    while !window.is_done() {
                        window.drive(&mut state, &mut ctx).unwrap();
                        while window.take_scoring_occurrence().is_some() {}
                        window.settle(&mut state, &mut ctx);
                    }
                    drop(ctx);
                    captured += state.player(&PlayerId::new("a")).unwrap().captured_units.len();
                }
            });
        }
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(captured > 0, "the Cabal captured something");
        assert!(w.compared >= 1, "{:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability ability:*:devour") && !k.contains("unported ability unit:*:cabal")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// The Terror Between captures the units destroyed in its system but never a neutral's (Thunder's Edge): a bombardment
    /// of neutral infantry under the Cabal flagship leaves the Cabal's sheet as it was, in the string and in the core window.
    #[test]
    fn the_cabal_does_not_capture_the_neutral_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
        let mut destroyed = 0;
        with_mask(mask, || {
            for seed in 1..=8_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "cabal"), ("b", "sol")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "cabal_flagship", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "dreadnought", &PlayerId::new("a"), 3);
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 3);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &neutral, 6);
                let before = state.system_state(&system).planet_units.values().flatten().filter(|u| u.owner == neutral).count();
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let content = ContentStore::embedded();
                let mut resolver = armed_resolver(&state, vec![PlayerId::new("a"), PlayerId::new("b")]);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                drop(ctx);
                let after = state.system_state(&system).planet_units.values().flatten().filter(|u| u.owner == neutral).count();
                destroyed += before.saturating_sub(after);
                assert!(state.player(&PlayerId::new("a")).unwrap().captured_units.iter().all(|(owner, _)| *owner != neutral), "seed {seed}");
            }
        });
        let r = report();
        assert!(destroyed > 0, "neutral infantry was destroyed");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// A Crimson Legionnaire destroyed by a bombardment pays its trade good as a staged event (the string state keeps it as a mark):
    /// the core step writes what the ability of the table staged back, instead of dropping it; found by the soak.
    #[test]
    fn a_legionnaire_destroyed_by_a_bombardment_stages_its_trade_good_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut staged = 0;
        with_mask(mask, || {
            for seed in 1..=8_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "muaat"), ("b", "mahact")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "dreadnought", &PlayerId::new("a"), 3);
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 2);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "mahact_infantry", &PlayerId::new("b"), 5);
                // Only the conversion is possible: the commodities are at the limit.
                state.player_mut(&PlayerId::new("b")).unwrap().commodities = 3;
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let content = ContentStore::embedded();
                let mut resolver = armed_resolver(&state, vec![PlayerId::new("a"), PlayerId::new("b")]);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                drop(ctx);
                staged += state.faction_marks.iter().filter(|(k, v)| k.starts_with("private:#staged:event:") && v.contains("TRADE_GOODS_GAINED")).count();
            }
        });
        let r = report();
        assert!(staged > 0, "a Legionnaire paid a trade good");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Run the invasion of `system` by "a" to its end with the first option everywhere (the reaction slots
    /// declined), the resolver armed for "a" and "b". For the lockstep tests of the ability table.
    fn run_invasion_first_option(state: &mut GameState, seed: u64, system: &SystemId) {
        run_invasion_with(state, seed, system, Box::new(crate::choice::FirstOption));
    }

    fn run_invasion_with(state: &mut GameState, seed: u64, system: &SystemId, decider: Box<dyn crate::choice::Decider>) {
        let mut table = Table::with_default(decider);
        let mut dice = Dice::new();
        let mut rng = GameRng::new(seed);
        let content = ContentStore::embedded();
        let order: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
        let mut resolver = armed_resolver(state, order);
        let mut sequence = crate::event::EventSequence::new();
        let mut window = InvasionWindow::new(state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), system);
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        while !window.is_done() {
            window.drive(state, &mut ctx).unwrap();
            while window.take_scoring_occurrence().is_some() {}
            window.settle(state, &mut ctx);
        }
    }

    /// Evelyn De Louis (the Sol agent) is an ability of the core table: at the start of each ground combat round the
    /// readied agent offers the units of the system, the chosen one is marked for the round and the card is
    /// exhausted. The core resolver asks the same question (options, captions) and writes the same mark.
    #[test]
    fn the_sol_agent_chooses_a_unit_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut marked = 0;
        with_mask(mask, || {
            for seed in 1..=4_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solagent"), ti4_model::state::LeaderStatus::Readied);
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
                run_invasion_first_option(&mut state, seed, &system);
                marked += state.faction_marks.keys().filter(|k| k.starts_with("yssaril:ssruu-round-agent:b:solagent:")).count();
            }
        });
        let r = report();
        let w = r.windows.get("invasion::begin_ground_round").copied().unwrap_or_default();
        assert!(marked > 0, "the agent marked a unit");
        assert!(w.compared >= 1, "the round start with the agent is compared: {:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:solagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// A ground combat with a third seat holding Ssruu (Yssaril, readied) next to a lender: the borrower takes the copy of the
    /// lender's agent (`prefer`: a substring of the ability id) and answers the questions with the option `seed` picks.
    fn ssruu_ground(lender: &str, agent: &str, prefer: &'static str) -> (usize, usize, crate::dual::DualReport) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        struct Pick(&'static str, u64);
        impl crate::choice::Decider for Pick {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
                let i = choice.options.iter().position(|o| o.id.contains(self.0)).unwrap_or(if choice.prompt.starts_with("Ssruu copying") { self.1 as usize % choice.options.len() } else { 0 });
                Ok(choice.options[i].clone())
            }
        }
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let (mut copied, mut placed) = (0, 0);
        with_mask(mask, || {
            for seed in 1..=6_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", lender), ("c", "yssaril")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("c")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("yssarilagent"), ti4_model::state::LeaderStatus::Readied);
                state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new(agent), ti4_model::state::LeaderStatus::Exhausted);
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
                let before: usize = state.system_state(&system).planet_units.values().flatten().count();
                let mut table = Table::with_default(Box::new(Pick(prefer, seed)));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let content = ContentStore::embedded();
                let order: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
                let mut resolver = armed_resolver(&state, order);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                copied += usize::from(state.player(&PlayerId::new("c")).unwrap().leaders[&ti4_model::id::LeaderId::new("yssarilagent")] == ti4_model::state::LeaderStatus::Exhausted);
                placed += usize::from(state.system_state(&system).planet_units.values().flatten().count() + 20 > before);
            }
        });
        let r = report();
        let _ = take_report();
        (copied, placed, r)
    }

    /// Ssruu copies Evelyn De Louis (Sol): the copy chooses one ground force of the system for an extra die, the mark names
    /// borrower and lender and the dice of the round read it from the core state.
    #[test]
    fn ssruu_copies_the_sol_agent_in_the_core_window_in_lockstep() {
        let (copied, _, r) = ssruu_ground("sol", "solagent", "ssruu-copy");
        assert!(copied > 0, "Ssruu was used");
        assert!(r.windows.keys().all(|k| !k.contains("[bail:")), "{:?}", r.windows);
        assert!(r.windows.get("invasion::begin_ground_round").is_some_and(|w| w.compared >= 1), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    }

    /// Ssruu copies Brother Milor's ground branch: the owner of the destroyed ground force places 2 infantry.
    #[test]
    fn ssruu_copies_brother_milor_on_the_ground_in_the_core_window_in_lockstep() {
        let (copied, _, r) = ssruu_ground("yin", "yinagent", "yssarilagent:yinagent");
        assert!(copied > 0, "Ssruu was used");
        assert!(r.windows.keys().all(|k| !k.contains("[bail:")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    }

    /// A Nomad that listens to the exhaustion of an agent (Temporal Command Suite) hears the Sol agent's exhaustion; the
    /// core does not announce it, so the window that follows the agent is the string's (`Unknown`), counted under the
    /// window's label with the Nomad as the reason.
    #[test]
    fn bail_the_sol_agent_with_a_listening_nomad_is_counted() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol"), ("c", "nomad")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = 2;
            crate::technology::grant(&mut state, &PlayerId::new("c"), &ti4_model::id::TechnologyId::new("tcs"));
            state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solagent"), ti4_model::state::LeaderStatus::Readied);
            let (system, planet) = crate::fixtures::a_placed_planet();
            crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
            run_invasion_first_option(&mut state, 2, &system);
        });
        let r = report();
        assert!(r.windows.keys().any(|k| k.contains("invasion::begin_ground_round") && k.contains("Nomad Temporal Command Suite")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// With diplomacy promises on, a leader's use is an event of the promises; the core does not evaluate them, so
    /// the window that follows the agent is the string's, with the ability named.
    #[test]
    fn bail_the_sol_agent_with_diplomacy_on_is_counted() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = 2;
            state.diplomacy.enabled = true;
            state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("solagent"), ti4_model::state::LeaderStatus::Readied);
            let (system, planet) = crate::fixtures::a_placed_planet();
            crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
            run_invasion_first_option(&mut state, 2, &system);
        });
        let r = report();
        assert!(r.windows.keys().any(|k| k.contains("invasion::begin_ground_round") && k.contains("diplomacy promises on a leader")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Self-Assembly Routines pays a trade good for each mech lost: the destroyed mech's AFTER window runs the ability of
    /// the core table, which gains the good and announces `TRADE_GOODS_GAINED` through the resolver.
    #[test]
    fn self_assembly_routines_pays_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut paid = 0;
        with_mask(mask, || {
            for seed in 1..=5_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                crate::technology::grant(&mut state, &PlayerId::new("b"), &ti4_model::id::TechnologyId::new("sar"));
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "hacan_mech", &PlayerId::new("b"), 2);
                let before = state.player(&PlayerId::new("b")).unwrap().trade_goods;
                run_invasion_first_option(&mut state, seed, &system);
                paid += state.player(&PlayerId::new("b")).unwrap().trade_goods - before;
            }
        });
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(paid > 0, "a mech was lost and paid");
        assert!(w.compared >= 1, "{:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability tech:*:sar")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Letani Warrior II rolls like Infantry II: each destroyed unit of the Arborec rolls a die from the same stream and a
    /// six puts it on the card (`arborec:lw2:{seat}` counts them).
    #[test]
    fn letani_warrior_ii_rolls_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut on_card = 0_u32;
        let mut rolled = 0;
        with_mask(mask, || {
            for seed in 1..=8_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "arborec")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                crate::technology::grant(&mut state, &PlayerId::new("b"), &ti4_model::id::TechnologyId::new("lw2"));
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "arborec_infantry2", &PlayerId::new("b"), 6);
                run_invasion_first_option(&mut state, seed, &system);
                on_card += state.faction_marks.get("arborec:lw2:b").and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
                rolled += 1;
            }
        });
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(rolled > 0 && on_card > 0, "a six went on the card in one of the seeds");
        assert!(w.compared >= 1, "{:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability unit:*:arborec_infantry2")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// The Crimson Legionnaire and its upgrade: after the unit is destroyed its owner gains a commodity or converts one into
    /// a trade good (a question when both are possible, with the commodities in the details); the II waits on the card.
    #[test]
    fn the_crimson_legionnaire_pays_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let (mut gained, mut on_card) = (0, 0);
        with_mask(mask, || {
            for (seed, commodities) in (1..=10_u64).zip([0, 1, 2, 0, 1, 2, 5, 0, 1, 2]) {
                let second = seed > 5;
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "mahact")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                if second {
                    crate::technology::grant(&mut state, &PlayerId::new("b"), &ti4_model::id::TechnologyId::new("cl2"));
                }
                state.player_mut(&PlayerId::new("b")).unwrap().commodities = commodities;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, if second { "mahact_infantry2" } else { "mahact_infantry" }, &PlayerId::new("b"), 4);
                let before = state.player(&PlayerId::new("b")).unwrap().trade_goods + state.player(&PlayerId::new("b")).unwrap().commodities;
                run_invasion_first_option(&mut state, seed, &system);
                let now = state.player(&PlayerId::new("b")).unwrap().trade_goods + state.player(&PlayerId::new("b")).unwrap().commodities;
                gained += i64::from(now - before).max(0);
                on_card += state.faction_marks.get("mahact:cl2:b").and_then(|n| n.parse::<u32>().ok()).unwrap_or(0);
            }
        });
        let r = report();
        let w = r.windows.get("invasion::ground_round_hits").copied().unwrap_or_default();
        assert!(gained > 0, "a commodity or a trade good was gained");
        assert!(on_card > 0, "a Legionnaire II waits on the card");
        assert!(w.compared >= 1, "{:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability unit:*:mahact_infantry")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Technological Singularity in a ground combat: the Nekro invades a planet; after a ground force of the defender is
    /// destroyed by the combat the ability asks which technology to take. The marks are keyed by system and planet.
    #[test]
    fn the_nekro_singularity_runs_in_a_ground_combat_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        struct Rotate(usize);
        impl crate::choice::Decider for Rotate {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
                if choice.prompt == "gain a technology, or assimilate instead" {
                    return Ok(choice.options[self.0 % choice.options.len()].clone());
                }
                crate::choice::FirstOption.choose(choice)
            }
        }
        let (mut gained, mut tokens) = (0, 0);
        with_mask(mask, || {
            for seed in 1..=10_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "hacan")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                for tech in ["vax", "vay"] {
                    state.player_mut(&PlayerId::new("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(tech));
                }
                state.player_mut(&PlayerId::new("b")).unwrap().technologies.clear();
                for tech in ["gd", "ie", "qdn"] {
                    state.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(tech));
                }
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
                let mut table = Table::with_default(Box::new(Rotate(seed as usize)));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let content = ContentStore::embedded();
                let order: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
                let mut resolver = armed_resolver(&state, order);
                let mut sequence = crate::event::EventSequence::new();
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &PlayerId::new("a"), &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap();
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
                let nekro = state.player(&PlayerId::new("a")).unwrap();
                gained += usize::from(nekro.technologies.contains(ti4_model::id::text(&"gd")) || nekro.technologies.contains(ti4_model::id::text(&"ie")));
                tokens += nekro.assimilated_technologies.len();
                assert!(state.faction_marks.keys().all(|k| !k.starts_with("nekro:ts:")), "the marks are closed with the combat");
            }
        });
        let r = report();
        assert!(gained > 0, "a technology was gained");
        assert!(tokens > 0, "a token was placed");
        assert!(r.windows.keys().all(|k| !k.contains("technological_singularity")), "{:?}", r.windows);
        assert!(r.windows.get("invasion::ground_round_hits").is_some_and(|w| w.compared >= 1), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Research Team (Deepwrought) and Slumberstate Computing (Titans) are string windows of `finish_committing`
    /// (`offer_research_team`, `add_forced_combats`): the eligibility queries are compared hook by hook, the questions and the
    /// coexistence writes are not core windows. A Deepwrought invader coexisting with the controller of a planet runs inside the
    /// dual run without a difference, and the ground combat windows are not entered for the coexisting planet.
    #[test]
    fn bail_the_research_team_coexistence_is_the_strings_inside_the_dual_run() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        struct Coexist;
        impl crate::choice::Decider for Coexist {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
                let i = choice.options.iter().position(|o| o.id == "coexist").unwrap_or(0);
                Ok(choice.options[i].clone())
            }
        }
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut coexisted = 0;
        with_mask(mask, || {
            for seed in 1..=3_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "deepwrought"), ("b", "hacan")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 3);
                state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 2);
                run_invasion_with(&mut state, seed, &system, Box::new(Coexist));
                coexisted += usize::from(crate::coexistence::is_coexisting(&state, &system, &planet, &PlayerId::new("a")));
            }
        });
        let r = report();
        assert!(coexisted > 0, "the invader coexisted");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Package W8, 5a: `FinalizingControl` (`finish_control_gain`: Assimilate, `PLANET_CONTROL_GAINED`, the destruction of the rival structures,
    /// up to the exploration) is the core window `ControlGained`, compared step by step: a capture of a planet holding a rival PDS runs
    /// without a difference, the structure is destroyed (or converted by an L1Z1X invader) and the control changes. Fails on the pre-W8
    /// code, which counted the chain as a gate and compared no window (`compared >= 1` and `bailed == 0` below).
    #[test]
    fn the_control_gain_chain_runs_as_a_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let (mut captured, mut converted) = (0, 0);
        with_mask(mask, || {
            for (seed, invader) in (1..=8_u64).zip(["hacan", "l1z1x", "hacan", "sol", "l1z1x", "sol", "l1z1x", "naaz"]) {
                let mut state = crate::fixtures::seated_game(&[("a", invader), ("b", "sardakk")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 1);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &PlayerId::new("b"), 1);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "spacedock", &PlayerId::new("b"), 1);
                run_invasion_first_option(&mut state, seed, &system);
                let took = state.system_state(&system).planet_control.get(&planet) == Some(&PlayerId::new("a"));
                captured += usize::from(took);
                if took {
                    let own = |id: &str| state.system_state(&system).planet_units.get(&planet).is_some_and(|u| u.iter().any(|x| x.type_id.as_str() == id && x.owner == PlayerId::new("a")));
                    converted += usize::from(invader == "l1z1x" && own("pds") && own("spacedock"));
                }
            }
        });
        let r = report();
        assert!(captured > 0, "a planet changed hands");
        assert!(converted > 0, "an L1Z1X invader assimilated the PDS and the space dock");
        let w = r.windows.get("invasion::finish_control_gain").copied().unwrap_or_default();
        assert!(w.compared >= u64::try_from(captured).unwrap() && w.bailed == 0, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Assimilate with the reinforcements spent (the rival PDS is destroyed, not converted), the same capture in a rules version 0 game (the
    /// rival structures are destroyed BEFORE the announcement) and a first claim of an unowned planet (the exploration follows the window):
    /// each runs in lockstep on the core window.
    #[test]
    fn the_control_gain_chain_runs_in_lockstep_with_empty_reinforcements_version_zero_and_a_first_claim() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut runs = 0;
        with_mask(mask, || {
            for (seed, case) in (1..=6_u64).zip(["spent", "spent", "version0", "version0", "unowned", "unowned"]) {
                let mut state = crate::fixtures::seated_game(&[("a", "l1z1x"), ("b", "sardakk")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                if case != "unowned" {
                    state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
                    crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 1);
                    crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &PlayerId::new("b"), 1);
                }
                if case == "spent" {
                    // Every PDS of the reinforcements is on the board somewhere else.
                    let home = state.player(&PlayerId::new("a")).unwrap().home_system.clone().unwrap();
                    let planet_at_home = state.system_state(&home).planet_control.keys().next().cloned().unwrap();
                    crate::fixtures::put_on_planet(&mut state, &home, &planet_at_home, "pds", &PlayerId::new("a"), 6);
                }
                if case == "version0" {
                    state.rules_version = 0;
                }
                run_invasion_first_option(&mut state, seed, &system);
                runs += 1;
            }
        });
        let r = report();
        let w = r.windows.get("invasion::finish_control_gain").copied().unwrap_or_default();
        assert!(runs == 6 && w.compared >= 3 && w.bailed == 0, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// What the chain still hands to the string: Integrated Economy (a production nested in the chain) is a counted gate, the window is not
    /// opened and the run has no difference.
    #[test]
    fn bail_the_control_gain_chain_of_integrated_economy_is_a_gate() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sardakk")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.player_mut(&PlayerId::new("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ie"));
            let (system, planet) = crate::fixtures::a_placed_planet();
            crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
            state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 1);
            run_invasion_first_option(&mut state, 1, &system);
        });
        let r = report();
        let gated: u64 = r.windows.iter().filter(|(k, _)| k.starts_with("invasion::finish_control_gain [gate: Integrated Economy")).map(|(_, w)| w.bailed).sum();
        assert!(gated >= 1, "{:?}", r.windows);
        assert_eq!(r.windows.get("invasion::finish_control_gain").map_or(0, |w| w.compared), 0, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Valkyrie Exoskeleton (the Sardakk mech): when it sustains in a ground combat it produces 1 hit against the opponent's ground
    /// forces; the core applies the invasion's own hits step and announces the events through the resolver.
    #[test]
    fn the_sardakk_mech_hits_back_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            for seed in 1..=24_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sardakk"), ("b", "hacan")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "sardakk_mech", &PlayerId::new("a"), 3);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 3);
                state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 6);
                run_invasion_first_option(&mut state, seed, &system);
            }
        });
        let r = report();
        let followed: u64 = r.nested.iter().filter(|(k, _)| k.contains("unit:*:sardakk_mech:GROUND_FORCE_SUSTAINED:after")).map(|(_, n)| *n).sum();
        assert!(followed > 0, "a mech sustained and hit back: {:?}", r.nested.keys().collect::<Vec<_>>());
        assert!(r.windows.keys().all(|k| !k.contains("sardakk_mech")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Fire Team ("reroll any number of your dice" after your ground forces roll): the card is played from the reaction slot of
    /// `GROUND_ROLLS_MADE`, one optional question per staged die, the chosen dice are drawn again from the dice stream. The core
    /// window rolls the same faces: the hits and the dice of the report and the position of the stream are compared.
    #[test]
    fn fire_team_rerolls_the_staged_dice_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        struct Play(u64, usize);
        impl crate::choice::Decider for Play {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, crate::choice::IllegalChoice> {
                self.1 += 1;
                if choice.prompt.starts_with("reroll die") {
                    let want = (self.0 as usize + self.1) % 2 == 0;
                    let i = choice.options.iter().position(|o| o.id.starts_with("reroll|") == want).unwrap_or(0);
                    return Ok(choice.options[i].clone());
                }
                let i = choice.options.iter().position(|o| o.id.starts_with("reaction:")).unwrap_or(0);
                Ok(choice.options[i].clone())
            }
        }
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            for seed in 1..=8_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("a")).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("fire_team")];
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
                run_invasion_with(&mut state, seed, &system, Box::new(Play(seed, 0)));
            }
        });
        let r = report();
        let played: u64 = r.nested.iter().filter(|(k, _)| k.contains("GROUND_ROLLS_MADE")).map(|(_, n)| *n).sum();
        assert!(played > 0, "Fire Team was offered inside the roll windows: {:?}", r.nested.keys().collect::<Vec<_>>());
        assert!(r.nested.keys().any(|k| k.contains("ask:reroll die")), "the dice were asked about one by one: {:?}", r.nested.keys().collect::<Vec<_>>());
        assert!(r.windows.keys().all(|k| !k.contains("[bail:")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// A Firmament seat's Siphon watches the commodities a Legionnaire gains: the core does not stage that event, so the
    /// window bails (named, counted) and nothing differs.
    #[test]
    fn bail_the_crimson_legionnaire_with_a_siphoning_firmament_seat_is_counted() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        with_mask(mask, || {
            for seed in 1..=4_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "mahact"), ("c", "firmament")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 8);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "mahact_infantry", &PlayerId::new("b"), 4);
                run_invasion_first_option(&mut state, seed, &system);
            }
        });
        let r = report();
        assert!(r.windows.keys().any(|k| k.contains("a fact the adapter did not compute")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Brother Milor (the Yin agent), ground branch: after a ground force is destroyed in a ground combat the readied
    /// agent lets its owner place 2 infantry on the planet; the core resolver places the same units (form, count)
    /// and exhausts the card.
    #[test]
    fn brother_milor_places_infantry_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Driver, Backend::Dual);
        let mut exhausted = 0;
        with_mask(mask, || {
            for (seed, who) in (1..=4_u64).zip(["hacan", "arborec", "sol", "naalu"]) {
                let mut state = crate::fixtures::seated_game(&[("a", who), ("b", "yin")], POK);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("yinagent"), ti4_model::state::LeaderStatus::Readied);
                let (system, planet) = crate::fixtures::a_placed_planet();
                crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
                crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
                run_invasion_first_option(&mut state, seed, &system);
                exhausted += usize::from(state.player(&PlayerId::new("b")).unwrap().leaders.get(ti4_model::id::text(&"yinagent")) == Some(&ti4_model::state::LeaderStatus::Exhausted));
            }
        });
        let r = report();
        assert!(exhausted > 0, "the agent was used");
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:yinagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    use ti4_model::content_types::DEFAULT as ALL_SOURCES;
    use ti4_model::content_types::POK;

    /// Space stations rule 5: ground forces cannot be committed to a space station.
    ///
    /// Checked on real tiles rather than an invented one. 117 carries only The Watchtower, so the
    /// whole tile must offer nothing; 109 carries Bellatrix *and* Tsion Station, so it must offer
    /// exactly the real planet -- which is the case that distinguishes "filtered" from "skipped
    /// the tile".
    #[test]
    fn ground_forces_cannot_land_on_a_space_station() {
        let content = ti4_content::ContentStore::embedded();
        let state = crate::fixtures::game(&["a"]);

        let station_only = landable_planets(&state, content, ALL_SOURCES, &SystemId::new("117"));
        assert!(
            station_only.is_empty(),
            "The Watchtower is the only planet on 117 and is a station, so nothing may land: {station_only:?}"
        );

        let mixed = landable_planets(&state, content, ALL_SOURCES, &SystemId::new("109"));
        assert_eq!(
            mixed,
            vec![PlanetId::new("bellatrix")],
            "109 must offer its real planet and not Tsion Station"
        );
    }

    use ti4_model::id::UnitTypeId;

    use super::*;
    use crate::setup::start_game;

    #[test]
    fn maximum_ground_immunity_filters_bombardment_and_defence_before_sustain() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let system = SystemId::new("18");
        let planet = PlanetId::new("mr");
        let owner = PlayerId::new("a");
        for cause in ["bombardment", "space_cannon_defense", "harrow"] {
            let mut state = crate::fixtures::seated_game(&[("a", "naaz"), ("b", "sol")], sources);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "naaz_voltron", &owner, 1);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &owner, 1);
            let mut log = Vec::new();
            let destroyed = ground_hit_logged(
                &mut state, content, sources, &system, &planet, &owner, cause, &mut log,
            )
            .unwrap();
            assert_eq!(destroyed.type_id.as_str(), "infantry", "{cause}");
            assert!(
                ground_hit_logged(
                    &mut state, content, sources, &system, &planet, &owner, cause, &mut log
                )
                .is_none()
            );
            assert_eq!(
                log.len(),
                1,
                "the unassignable hit emits no sustain/destruction"
            );
            assert!(!state.system_state(&system).on_planet(&planet)[0].sustained_damage);
            assert!(
                matches!(
                    apply_ground_hit(&mut state, content, sources, &system, &planet, &owner),
                    GroundHit::Sustained(_)
                ),
                "ordinary combat hits remain eligible"
            );
        }
    }

    #[test]
    fn maximum_does_not_use_sustain_for_a_harrow_hit_in_owner_choice_path() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let system = SystemId::new("18");
        let planet = PlanetId::new("mr");
        let owner = PlayerId::new("a");
        let mut state = crate::fixtures::seated_game(&[("a", "naaz"), ("b", "sol")], sources);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "naaz_voltron", &owner, 1);
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        absorb_ground_with_origin(
            &mut state, content, sources, &mut table, &owner, &system, &planet, 1, true,
        )
        .unwrap();
        assert!(!state.system_state(&system).on_planet(&planet)[0].sustained_damage);
        absorb_ground(
            &mut state, content, sources, &mut table, &owner, &system, &planet, 1,
        )
        .unwrap();
        assert!(state.system_state(&system).on_planet(&planet)[0].sustained_damage);
    }

    /// Exercise the bombardment route from an actual invasion window. A war sun rolls three
    /// hits; the infantry is the only eligible casualty, so the remaining hits are wasted while
    /// the Maximum stays on the planet, undamaged.
    #[test]
    fn invasion_window_bombardment_skips_maximum_and_discards_excess_hits() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let (mut state, system, planet) = naaz_invasion_arena("sol");
        let invader = PlayerId::new("b");
        let defender = PlayerId::new("a");
        isolate_invasion_system(&mut state, &system, &planet, &defender);
        in_space(&mut state, &system, "warsun", &invader, 1);
        on_planet(&mut state, &system, &planet, "naaz_voltron", &defender, 1);
        on_planet(&mut state, &system, &planet, "infantry", &defender, 1);

        let mut dice = Dice::from_faces([10u32, 10, 10]);
        let mut rng = GameRng::new(3);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(FirstOptionCapturing { seen: seen.clone() }));
        let mut window = InvasionWindow::new(
            &mut state, content, sources, &mut dice, &mut rng, &invader, &system,
        );
        let mut ctx = Resolving {
            content,
            sources,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        window.drive(&mut state, &mut ctx).unwrap();

        assert_eq!(window.report.bombardment_kills, 1);
        let here = state.system_state(&system);
        let standing = here.on_planet(&planet);
        assert_eq!(standing.len(), 1, "only the Maximum should remain");
        assert_eq!(standing[0].type_id.as_str(), "naaz_voltron");
        assert!(!standing[0].sustained_damage);
        assert!(
            seen.borrow()
                .iter()
                .all(|choice| !choice.prompt.to_ascii_lowercase().contains("sustain")),
            "bombardment must not offer the Maximum's sustain"
        );
    }

    /// The space-cannon-defense step after a real commitment must apply the hit to an eligible
    /// infantry unit instead of offering the invading Naaz player's Maximum as a casualty.
    #[test]
    fn invasion_window_space_cannon_defense_skips_maximum_for_infantry() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let (mut state, system, planet) = naaz_invasion_arena("sol");
        let invader = PlayerId::new("a");
        let defender = PlayerId::new("b");
        isolate_invasion_system(&mut state, &system, &planet, &defender);
        on_planet(&mut state, &system, &planet, "pds", &defender, 1);
        on_planet(&mut state, &system, &planet, "naaz_voltron", &invader, 1);
        in_space(&mut state, &system, "infantry", &invader, 1);

        let mut dice = Dice::from_faces([10u32]);
        let mut rng = GameRng::new(4);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(FirstOptionCapturing { seen: seen.clone() }));
        let mut window = InvasionWindow::at_commit_step(&mut state, &invader, &system);
        let mut ctx = Resolving {
            content,
            sources,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        window.drive(&mut state, &mut ctx).unwrap();

        let here = state.system_state(&system);
        let standing = here.on_planet_of(&planet, &invader);
        assert_eq!(standing.len(), 1, "the PDS hit should remove the infantry");
        assert_eq!(standing[0].type_id.as_str(), "naaz_voltron");
        assert!(!standing[0].sustained_damage);
        assert!(
            seen.borrow()
                .iter()
                .all(|choice| !choice.prompt.to_ascii_lowercase().contains("sustain")),
            "the Maximum must never be offered sustain for the cannon hit"
        );
    }

    /// Drive one real ground-combat round through Harrow. The six low faces are the Maximum's
    /// four dice plus the two infantry dice; the dreadnought then hits on Harrow. Only the
    /// defender's infantry may take that unit-ability hit.
    #[test]
    fn invasion_window_harrow_skips_maximum_for_eligible_infantry() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let (mut state, system, planet) = naaz_invasion_arena("l1z1x");
        let defender = PlayerId::new("a");
        let invader = PlayerId::new("b");
        isolate_invasion_system(&mut state, &system, &planet, &defender);
        on_planet(&mut state, &system, &planet, "naaz_voltron", &defender, 1);
        on_planet(&mut state, &system, &planet, "infantry", &defender, 1);
        in_space(&mut state, &system, "dreadnought", &invader, 1);
        in_space(&mut state, &system, "infantry", &invader, 1);

        let mut dice = Dice::from_faces([1u32, 1, 1, 1, 1, 1, 10, 10, 10]);
        let mut rng = GameRng::new(5);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(FirstOptionCapturing { seen: seen.clone() }));
        let mut window = InvasionWindow::at_commit_step(&mut state, &invader, &system);
        let mut ctx = Resolving {
            content,
            sources,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        window.settle(&mut state, &mut ctx);
        let commit = window
            .pending_choice(&state, content, sources)
            .expect("the invading infantry can commit")
            .options
            .into_iter()
            .find(|option| option.kind == COMMIT_KIND)
            .expect("choose the native commit option");
        window.resolve(&mut state, &mut ctx, commit).unwrap();
        window.settle(&mut state, &mut ctx);
        let fight = window
            .pending_choice(&state, content, sources)
            .expect("the committed infantry starts ground combat")
            .options[0]
            .clone();
        window.resolve(&mut state, &mut ctx, fight).unwrap();

        let here = state.system_state(&system);
        let standing = here.on_planet_of(&planet, &defender);
        assert_eq!(
            standing.len(),
            1,
            "Harrow should remove the eligible infantry"
        );
        assert_eq!(standing[0].type_id.as_str(), "naaz_voltron");
        assert!(!standing[0].sustained_damage);
        assert!(
            seen.borrow()
                .iter()
                .all(|choice| !choice.prompt.to_ascii_lowercase().contains("sustain")),
            "Harrow must not offer the Maximum's sustain"
        );
    }

    fn naaz_invasion_arena(opponent_faction: &str) -> (GameState, SystemId, PlanetId) {
        let sources = ti4_model::content_types::DEFAULT;
        let state =
            crate::fixtures::seated_game(&[("a", "naaz"), ("b", opponent_faction)], sources);
        let planets = ti4_content::galaxy::all_planets(ContentStore::embedded(), sources);
        let (id, planet) = planets
            .iter()
            .find(|(_, planet)| {
                planet.system_id().is_some()
                    && !planet.is_placed_during_play()
                    && planet.system_id() != Some(crate::seating::MECATOL)
            })
            .expect("the corpus has an invasion planet outside Mecatol Rex");
        (
            state,
            SystemId::new(planet.system_id().unwrap()),
            PlanetId::new(*id),
        )
    }

    fn isolate_invasion_system(
        state: &mut GameState,
        system: &SystemId,
        planet: &PlanetId,
        controller: &PlayerId,
    ) {
        let board = state.system_mut(system);
        board.units.clear();
        board.planet_units.clear();
        board.planet_control.clear();
        board
            .planet_control
            .insert(planet.clone(), controller.clone());
    }

    fn invader() -> PlayerId {
        PlayerId::new("a")
    }
    fn holder() -> PlayerId {
        PlayerId::new("b")
    }

    /// A system the corpus gives planets, so landing has somewhere to go.
    fn arena() -> (GameState, SystemId, PlanetId) {
        let state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
            .iter()
            .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
            .expect("the corpus has a placed planet");
        (state, system, planet)
    }

    fn on_planet(
        state: &mut GameState,
        system: &SystemId,
        planet: &PlanetId,
        kind: &str,
        owner: &PlayerId,
        count: usize,
    ) {
        for _ in 0..count {
            state
                .system_mut(system)
                .planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new(kind), owner.clone()));
        }
    }

    fn in_space(
        state: &mut GameState,
        system: &SystemId,
        kind: &str,
        owner: &PlayerId,
        count: usize,
    ) {
        for _ in 0..count {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new(kind), owner.clone()));
        }
    }

    fn kit() -> (Table, Dice, GameRng) {
        (Table::new(), Dice::new(), GameRng::new(5))
    }

    #[test]
    fn a_planetary_shield_blocks_bombardment_and_a_war_sun_ignores_it() {
        // 15.1f, and the exception that is most of what a war sun is for.
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        in_space(&mut state, &system, "dreadnought", &invader(), 1);

        assert!(
            !bombardable(
                &state,
                ContentStore::embedded(),
                POK,
                &system,
                &planet,
                &invader()
            ),
            "a PDS shields the planet"
        );

        in_space(&mut state, &system, "warsun", &invader(), 1);
        assert!(
            bombardable(
                &state,
                ContentStore::embedded(),
                POK,
                &system,
                &planet,
                &invader()
            ),
            "a war sun ignores the shield"
        );
    }

    #[test]
    fn bombardment_kills_defenders_and_spares_your_own() {
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 4);
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
        in_space(&mut state, &system, "dreadnought", &invader(), 6);
        let (mut table, mut dice, mut rng) = kit();

        bombardment(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &mut table,
            &system,
            &invader(),
        )
        .expect("single owner: no choice to refuse");

        assert_eq!(
            state
                .system_state(&system)
                .on_planet_of(&planet, &invader())
                .len(),
            2,
            "your own troops are never bombarded"
        );
    }

    #[test]
    fn separate_bombardments_record_separate_noncombat_occurrences() {
        let (mut state, system, planet) = arena();
        in_space(&mut state, &system, "dreadnought", &invader(), 1);
        let mut dice = Dice::from_faces([10, 10]);
        let mut rng = GameRng::new(1);
        let mut table = Table::new();

        for _ in 0..2 {
            on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
            assert_eq!(
                bombardment(
                    &mut state,
                    ContentStore::embedded(),
                    POK,
                    &mut dice,
                    &mut rng,
                    &mut table,
                    &system,
                    &invader(),
                )
                .expect("single owner: no choice to refuse"),
                1
            );
        }

        let occurrences: Vec<_> = state
            .player(&invader())
            .unwrap()
            .event_feats
            .iter()
            .filter_map(|(feat, occurrence)| {
                (*feat == Feat::BombardedOutTheLastGroundForces).then_some(*occurrence)
            })
            .collect();
        assert_eq!(occurrences.len(), 2);
        assert_ne!(occurrences[0], occurrences[1]);
    }

    #[test]
    fn ground_combat_in_any_rivals_home_records_darkening_the_skies() {
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        let c = PlayerId::new("c");
        let mut state = start_game(
            ContentStore::embedded(),
            &[a.clone(), b.clone(), c.clone()],
            POK,
            None,
        )
        .unwrap();
        let (_, system, _) = arena();
        state.player_mut(&b).unwrap().home_system = Some(system.clone());
        let occurrence = state.begin_feat_occurrence();
        let notes = crate::combat::note_holdings(&state);

        assert!(note_ground_combat_win_feats(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &a,
            &c,
            &notes,
            occurrence,
        ));
        assert!(state.did_at_occurrence(&a, Feat::WonInARivalHome, occurrence));
    }

    #[test]
    fn ground_combat_uses_note_holdings_from_tactical_action_start() {
        let (mut state, system, _) = arena();
        // Production-format key: the suffix is the owner's faction name, resolved to that
        // faction's seat. holder b plays Hacan and owns the Trade Convoys note; the invader places it
        // faceup in its play area with the card's ACTION (the corpus marks convoys playArea).
        state.player_mut(&holder()).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        crate::promissory::take(
            &mut state,
            ContentStore::embedded(),
            &invader(),
            "convoys:hacan",
        );
        // Its text opens ACTION, so the holder places it faceup deliberately.
        assert!(crate::promissory::play_convoys(&mut state, &invader()));
        let notes = crate::combat::note_holdings(&state);
        state.promissory_notes.clear();
        let occurrence = state.begin_feat_occurrence();

        assert!(note_ground_combat_win_feats(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &invader(),
            &holder(),
            &notes,
            occurrence,
        ));
        assert!(state.did_at_occurrence(&invader(), Feat::WonAgainstANoteHolder, occurrence));
    }

    #[test]
    fn ground_combat_resolves_production_note_keys_to_seated_issuers() {
        // Betray a Friend on the ground path: the winner holds the loser's note faceup in its
        // play area, and the production key "terraform:titans" must resolve to the Titans' seat —
        // not to a PlayerId built from the faction name.
        let (mut state, system, _) = arena();
        let content = ContentStore::embedded();
        let a = invader(); // titans: the issuer, and the loser
        let b = holder(); // hacan: holds a's note, and wins
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("titans");
        state.player_mut(&b).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        crate::promissory::take(&mut state, content, &b, "terraform:titans");
        let notes = crate::combat::note_holdings(&state);
        let occurrence = state.begin_feat_occurrence();

        assert!(note_ground_combat_win_feats(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &b,
            &a,
            &notes,
            occurrence,
        ));
        assert!(state.did_at_occurrence(&b, Feat::WonAgainstANoteHolder, occurrence));
    }

    #[test]
    fn an_undefended_planet_is_not_bombarded() {
        let (mut state, system, _) = arena();
        in_space(&mut state, &system, "dreadnought", &invader(), 4);
        let (mut table, mut dice, mut rng) = kit();

        let killed = bombardment(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &mut table,
            &system,
            &invader(),
        )
        .expect("no defenders: no choice to refuse");

        assert_eq!(killed, 0);
        assert_eq!(dice.count(), 0, "nothing to shoot at, so no dice");
    }

    /// Answers like [`Scripted`] but records what every question offered, so a test can
    /// assert not only the outcome but what the engine was allowed to say.
    struct RecordingScripted {
        inner: crate::choice::Scripted,
        seen: std::rc::Rc<std::cell::RefCell<Vec<Vec<String>>>>,
    }

    impl crate::choice::Decider for RecordingScripted {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            self.seen
                .borrow_mut()
                .push(choice.ids().into_iter().map(String::from).collect());
            self.inner.choose(choice)
        }
    }

    #[test]
    fn a_coexisting_planets_bombardment_is_chosen_per_unit_and_capped_at_each_target() {
        // Coexistence 7, 7.1, 7.2: the invader chooses, per bombarding unit, whose units on
        // the planet take the hits, and a unit's hits stop at the chosen player's own units
        // rather than spilling to a different player's forces.
        //
        // Invader's fleet: a dreadnought (one hit), a Sardakkian dreadnought (two hits),
        // another dreadnought (one hit). B holds one infantry on the planet, C holds two.
        // The first unit names C, the second names B (destroying B's only infantry and
        // wasting its surplus hit, which must not spill to C), the third names C again,
        // which empties the planet. The answers deliberately differ from the first offered
        // option, so a decider that ignored them would visibly change the result.
        let (system, planet) = crate::fixtures::a_placed_planet();
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        let c = PlayerId::new("c");
        let content = ContentStore::embedded();

        let mut state = crate::fixtures::game(&["a", "b", "c"]);
        state
            .system_mut(&system)
            .set_control(planet.clone(), b.clone());
        on_planet(&mut state, &system, &planet, "infantry", &b, 1);
        on_planet(&mut state, &system, &planet, "infantry", &c, 2);
        in_space(&mut state, &system, "dreadnought", &a, 1);
        in_space(&mut state, &system, "sardakk_dreadnought", &a, 1);
        in_space(&mut state, &system, "dreadnought", &a, 1);

        let mut dice = Dice::from_faces([8, 6, 6, 8]);
        let mut rng = GameRng::new(7);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let decider = RecordingScripted {
            inner: crate::choice::Scripted::new(["c", "b", "c"]),
            seen: seen.clone(),
        };
        let mut table = Table::with_default(Box::new(decider));

        let mut window =
            InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &a, &system);
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        // The window's own decisions run against the table, exactly as the public `resolve`
        // wrapper does, and the driver does through its step boundary.
        while !window.is_done() {
            window
                .drive(&mut state, &mut ctx)
                .expect("the script only picks what is offered");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
        }

        let report = window.into_report();
        assert_eq!(
            report.bombardment_kills, 3,
            "1 + 1 (the second unit's other hit wasted on B) + 1"
        );
        let expected: Vec<Vec<String>> = vec![
            vec!["b".to_owned(), "c".to_owned()],
            vec!["b".to_owned(), "c".to_owned()],
            vec!["c".to_owned()],
        ];
        assert_eq!(
            seen.borrow().as_slice(),
            expected.as_slice(),
            "each unit is asked separately, and B stops being offered once B holds nothing there"
        );

        let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert!(
            left.is_empty(),
            "every infantry was named: C twice, B once, B's surplus hit wasted"
        );
        assert_eq!(dice.count(), 3, "one roll per bombarding ship");
    }

    /// The bombardment question is an offer card: planet, hits and, per answer, what that
    /// player has on the planet. Display only, so option ids and kinds are as before.
    #[test]
    fn bombardment_target_question_is_an_offer_card_naming_each_players_forces() {
        let system = SystemId::new("18");
        let planet = PlanetId::new("mecatolrex");
        let present: std::collections::BTreeSet<PlayerId> =
            [PlayerId::new("b"), PlayerId::new("c")]
                .into_iter()
                .collect();
        let units = vec![
            Unit::new(UnitTypeId::new("infantry"), PlayerId::new("b")),
            Unit::new(UnitTypeId::new("infantry"), PlayerId::new("b")),
            Unit::new(UnitTypeId::new("mech"), PlayerId::new("b")),
            Unit::new(UnitTypeId::new("infantry"), PlayerId::new("c")),
        ];
        let choice =
            bombardment_target_question(&PlayerId::new("a"), &system, &planet, 2, &present, &units)
                .expect("two seats present");
        assert_eq!(choice.ids(), vec!["b", "c"]);
        assert_eq!(
            choice
                .details
                .get("kind")
                .and_then(serde_json::Value::as_str),
            Some("offer")
        );
        let captions = choice.details.get("captions").expect("captions");
        assert_eq!(captions["b"]["hint"], "2 infantry, 1 mech on mecatolrex");
        assert_eq!(captions["c"]["seat"], "c");
        let facts = choice
            .details
            .get("facts")
            .and_then(serde_json::Value::as_array)
            .expect("facts");
        assert!(
            facts
                .iter()
                .any(|f| f["label"] == "Hits to assign" && f["value"] == "2")
        );
    }

    /// OBS-008b5: a bombardment-target option names the system and planet it concerns, board
    /// context it previously carried none of at all.
    #[test]
    fn obs008b5_bombardment_target_options_carry_their_system_and_planet() {
        let system = SystemId::new("18");
        let planet = PlanetId::new("mecatolrex");
        let present: std::collections::BTreeSet<PlayerId> =
            [PlayerId::new("b"), PlayerId::new("c")]
                .into_iter()
                .collect();

        let choice =
            bombardment_target_question(&PlayerId::new("a"), &system, &planet, 2, &present, &[])
                .expect("two seats present");

        for option in &choice.options {
            assert_eq!(
                option
                    .payload
                    .get("system")
                    .and_then(serde_json::Value::as_str),
                Some("18")
            );
            assert_eq!(
                option
                    .payload
                    .get("planet")
                    .and_then(serde_json::Value::as_str),
                Some("mecatolrex")
            );
        }
        assert_eq!(
            choice
                .options
                .iter()
                .map(|option| option.id.clone())
                .collect::<std::collections::BTreeSet<_>>(),
            ["b".to_owned(), "c".to_owned()].into_iter().collect(),
            "the option id is still the targeted seat, for the engine to route the answer"
        );
    }

    /// OBS-008b6: three more producers closed out in one pass. A ground casualty names its unit
    /// and damage state, the same payload space combat's casualty already carries; "fight this
    /// coexisting player next" still routes on the seat's raw id (the policy-side leak fix is
    /// tested in `features.rs`); removing the custodians token previews the capped victory-point
    /// gain and, when declined, no change at all.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one batched package's three fixtures stay together rather than splitting artificially"
    )]
    fn obs008b6_ground_casualty_and_custodians_previews() {
        let (system, planet) = crate::fixtures::a_placed_planet();
        let content = ContentStore::embedded();
        let player = invader();

        // A ground casualty names its unit.
        let mut state = crate::fixtures::game(&["a", "b"]);
        // Two kinds of infantry: a mech would sustain the hit and nothing would be asked.
        on_planet(&mut state, &system, &planet, "infantry", &player, 1);
        on_planet(&mut state, &system, &planet, "infantry2", &player, 1);
        let (decider, seen) = crate::choice::Capturing::new(Box::new(crate::choice::FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        absorb_ground(
            &mut state, content, POK, &mut table, &player, &system, &planet, 1,
        )
        .unwrap();
        let asked = seen.borrow();
        let choice = asked.first().expect("two ground forces means a real ask");
        for option in &choice.options {
            assert!(
                option.payload.contains_key("unit"),
                "every ground casualty option names its unit: {option:?}"
            );
        }

        // "fight this coexisting player next" still routes on the seat's own raw id.
        let mut window = InvasionWindow {
            invader: player.clone(),
            system: system.clone(),
            stage: Stage::ChoosingNextCombat {
                planets: vec![planet.clone()],
                index: 0,
                planet: planet.clone(),
                remaining: vec![PlayerId::new("b")],
            },
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let next_combat = window
            .pending_choice(&state, content, POK)
            .expect("a coexisting player remains");
        assert!(
            next_combat
                .options
                .iter()
                .any(|option| option.id == "fight|b"),
            "the engine still routes on the raw seat id: {:?}",
            next_combat.options
        );
        window.stage = Stage::ChoosingNextCombat {
            planets: vec![planet.clone()],
            index: 0,
            planet: planet.clone(),
            remaining: vec![],
        };

        // Removing the custodians token previews the capped victory-point gain; declining
        // previews no change.
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let mut mecatol_state = crate::fixtures::game(&["a", "b"]);
        mecatol_state.board.entry(mecatol.clone()).or_default();
        crate::fixtures::put(&mut mecatol_state, &mecatol, "infantry", &player, 1);
        if let Some(seat) = mecatol_state.player_mut(&player) {
            seat.trade_goods = i32::try_from(CUSTODIANS_COST).unwrap();
        }
        let custodians_window = InvasionWindow {
            invader: player.clone(),
            system: mecatol,
            stage: Stage::Custodians,
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&mecatol_state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: mecatol_state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let removal = custodians_window
            .pending_choice(&mecatol_state, content, POK)
            .expect("removable");
        let yes = removal
            .options
            .iter()
            .find(|option| option.id == "yes")
            .expect("the removal option");
        match &yes.preview.as_ref().expect("previewed").outcome {
            crate::preview::Outcome::Certain { deltas } => {
                let vp = deltas
                    .iter()
                    .find(|delta| delta.quantity == Quantity::VictoryPoints)
                    .expect("a victory-point delta");
                assert_eq!((vp.before, vp.after), (0, 1));
            }
            other => panic!("removal preview is certain, got {other:?}"),
        }
        let no = removal
            .options
            .iter()
            .find(|option| option.id == "no")
            .expect("the decline option");
        match &no.preview.as_ref().expect("previewed").outcome {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(deltas, &[Delta::new(Quantity::VictoryPoints, 0, 0)]);
            }
            other => panic!("decline preview is certain, got {other:?}"),
        }
    }

    #[test]
    fn ground_forces_land_from_space_onto_a_planet() {
        let (mut state, system, planet) = arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        let (mut table, _, _) = kit();

        let committed = commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        assert!(!committed.is_empty());
        assert!(
            landable(&state, ContentStore::embedded(), POK, &invader(), &system).is_empty(),
            "they left the space area"
        );
        let _ = planet;
    }

    #[test]
    fn an_uncontested_landing_takes_the_planet_exhausted() {
        // A captured planet is taken exhausted: its resources belong to the round after the
        // one you spent conquering it.
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);

        let captured = establish_control(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &invader(),
            std::slice::from_ref(&planet),
        );

        assert_eq!(captured, vec![(planet.clone(), None)]);
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&invader())
        );
        assert!(
            state.exhausted_planets.contains(&planet),
            "taken exhausted, not ready to spend"
        );
    }

    #[test]
    fn losing_a_home_planet_records_a_separate_occurrence_for_its_previous_holder() {
        let (mut state, system, planet) = arena();
        state.player_mut(&holder()).unwrap().home_system = Some(system.clone());
        state
            .system_mut(&system)
            .set_control(planet.clone(), holder());
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);
        let mut window = InvasionWindow {
            invader: invader(),
            system: system.clone(),
            stage: Stage::Done,
            report: InvasionReport {
                committed: vec![planet.clone()],
                ..InvasionReport::default()
            },
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut table = Table::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        window.advance_fighting(&mut state, &mut ctx, std::slice::from_ref(&planet), 0);

        let (occurrence, combat) = window
            .take_scoring_occurrence()
            .expect("the control loss creates a scoring occurrence");
        assert!(!combat, "control loss is not a combat occurrence");
        assert!(state.did_at_occurrence(&holder(), Feat::LostAHomePlanet, occurrence));
        assert!(!state.did_at_occurrence(&invader(), Feat::LostAHomePlanet, occurrence));
        assert!(
            matches!(window.stage, Stage::FinalizingControl { .. }),
            "gain-control effects wait until the loss-scoring pause closes"
        );

        window.settle(&mut state, &mut ctx);
        assert!(
            window.is_done(),
            "the retained invasion resumes after scoring"
        );
    }

    #[test]
    fn taking_an_unowned_planet_explores_it_and_taking_one_off_a_rival_does_not() {
        // 35.1. A caller told merely that control changed would explore every conquest and
        // draw cards the rules do not give.
        let explore_once = |previous_holder: Option<PlayerId>| {
            let (mut state, system, planet) = arena();
            // A planet with a trait, so it has a deck to explore into.
            let deck = crate::exploration::trait_of(ContentStore::embedded(), POK, &planet)?;
            state
                .exploration_decks
                .insert(deck, vec!["minent".to_owned()]);
            if let Some(holder) = previous_holder {
                state
                    .system_mut(&system)
                    .set_control(planet.clone(), holder);
            }
            on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);

            let mut window = InvasionWindow {
                invader: invader(),
                system: system.clone(),
                stage: Stage::Done,
                report: InvasionReport {
                    committed: vec![planet.clone()],
                    ..InvasionReport::default()
                },
                pending_scoring_occurrences: std::collections::VecDeque::new(),
                current_ground_occurrence: None,
                notes_at_tactical_start: crate::combat::note_holdings(&state),
                bombard_plan: Vec::new(),
                bombard_index: 0,
                bombard_occurrence: state.begin_feat_occurrence(),
                bombard_announced: true,
                extra_commits: Vec::new(),
                temporary_space_commits: std::collections::BTreeMap::new(),
                combat_only: false,
                strict_timing_error: None,
                galaxy: None,
            };
            let mut dice = Dice::new();
            let mut rng = GameRng::new(1);
            let mut inner = Table::new();
            let mut ctx = crate::choice::Resolving {
                content: ContentStore::embedded(),
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut inner,
                timing: None,
            };
            window.advance_fighting(&mut state, &mut ctx, &[planet], 0);
            Some(window.into_report().explored.len())
        };

        if let Some(unowned) = explore_once(None) {
            assert_eq!(unowned, 1, "a planet nobody held is explored");
        }
        if let Some(conquered) = explore_once(Some(holder())) {
            assert_eq!(conquered, 0, "a planet taken off a rival is not");
        }
    }

    /// A dual-trait planet explores into whichever deck the invader picks, not whichever trait
    /// happens to print first.
    ///
    /// Lazul Rex prints INDUSTRIAL before CULTURAL. Before `trait_of` was replaced with
    /// `choose_deck` at the capture site, the `find_map` over the planet's own trait order
    /// always stopped at INDUSTRIAL -- an invader who wanted the cultural deck could never reach
    /// it, no matter what they answered, because nothing ever asked.
    #[test]
    fn a_dual_trait_planet_explores_into_the_invaders_chosen_deck() {
        let content = ContentStore::embedded();
        let planet = PlanetId::new("lazulrex");
        assert_eq!(
            crate::exploration::traits_of(content, ALL_SOURCES, &planet),
            vec!["INDUSTRIAL".to_owned(), "CULTURAL".to_owned()],
            "Lazul Rex prints industrial before cultural"
        );
        let system = ti4_content::galaxy::planet(content, planet.as_str(), ALL_SOURCES)
            .and_then(|record| record.system_id())
            .map(SystemId::new)
            .expect("Lazul Rex sits on a tile");

        let mut state = start_game(content, &[invader(), holder()], ALL_SOURCES, None).unwrap();
        state
            .exploration_decks
            .insert("INDUSTRIAL".to_owned(), vec!["minent".to_owned()]);
        state
            .exploration_decks
            .insert("CULTURAL".to_owned(), vec!["ent".to_owned()]);
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);

        let mut window = InvasionWindow {
            invader: invader(),
            system: system.clone(),
            stage: Stage::Done,
            report: InvasionReport {
                committed: vec![planet.clone()],
                ..InvasionReport::default()
            },
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(["CULTURAL"])));
        let mut ctx = Resolving {
            content,
            sources: ALL_SOURCES,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        window.advance_fighting(&mut state, &mut ctx, std::slice::from_ref(&planet), 0);

        assert_eq!(
            window.into_report().explored.len(),
            1,
            "the planet was explored"
        );
        assert_eq!(
            state.exploration_decks.get("CULTURAL").map(Vec::len),
            Some(0),
            "the chosen deck was drawn from"
        );
        assert_eq!(
            state.exploration_decks.get("INDUSTRIAL").map(Vec::len),
            Some(1),
            "the deck the invader did not choose was left untouched"
        );
    }

    #[test]
    fn a_wiped_out_invasion_leaves_the_planet_with_its_holder() {
        // 49.5d: everything died, so the defender keeps what they had.
        let (mut state, system, planet) = arena();
        state
            .system_mut(&system)
            .set_control(planet.clone(), holder());

        let captured = establish_control(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &invader(),
            std::slice::from_ref(&planet),
        );

        assert!(captured.is_empty());
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&holder()),
            "control did not fall to the invader by default"
        );
    }

    #[test]
    fn recapturing_your_own_planet_changes_nothing() {
        // 49.5c.
        let (mut state, system, planet) = arena();
        state
            .system_mut(&system)
            .set_control(planet.clone(), invader());
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);

        let captured = establish_control(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &invader(),
            std::slice::from_ref(&planet),
        );

        assert!(captured.is_empty());
        assert!(
            !state.exhausted_planets.contains(&planet),
            "it was not re-taken, so it was not exhausted"
        );
    }

    #[test]
    fn ground_combat_ends_with_one_side_holding_the_planet() {
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 4);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        let (mut table, mut dice, mut rng) = kit();

        ground_combat(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
            &planet,
            &invader(),
        )
        .unwrap();

        let board = state.system_state(&system);
        let both = !board.on_planet_of(&planet, &invader()).is_empty()
            && !board.on_planet_of(&planet, &holder()).is_empty();
        assert!(
            !both,
            "a ground combat does not end with both sides standing"
        );
    }

    #[test]
    fn each_defended_planet_gets_a_distinct_ground_combat_occurrence() {
        let mut state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        state.player_mut(&holder()).unwrap().victory_points = 1;
        let planets = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK);
        let (system, pair) = planets
            .iter()
            .filter_map(|(id, record)| {
                record
                    .system_id()
                    .map(|system| (SystemId::new(system), PlanetId::new(*id)))
            })
            .fold(
                std::collections::BTreeMap::<SystemId, Vec<PlanetId>>::new(),
                |mut grouped, (system, planet)| {
                    grouped.entry(system).or_default().push(planet);
                    grouped
                },
            )
            .into_iter()
            .find_map(|(system, planets)| {
                (planets.len() >= 2).then(|| (system, planets[..2].to_vec()))
            })
            .expect("the corpus has a two-planet system");
        for planet in &pair {
            on_planet(&mut state, &system, planet, "infantry", &invader(), 1);
            on_planet(&mut state, &system, planet, "infantry", &holder(), 1);
        }
        let mut window = InvasionWindow {
            invader: invader(),
            system,
            stage: Stage::Done,
            report: InvasionReport {
                committed: pair.clone(),
                ..InvasionReport::default()
            },
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let mut dice = Dice::from_faces([10, 1, 10, 1]);
        let mut rng = GameRng::new(1);
        let mut table = Table::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        window.advance_fighting(&mut state, &mut ctx, &pair, 0);
        let first_answer = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .unwrap()
            .options[0]
            .clone();
        window.resolve(&mut state, &mut ctx, first_answer).unwrap();
        let (first, first_is_combat) = window.take_scoring_occurrence().unwrap();
        assert!(first_is_combat);

        window.settle(&mut state, &mut ctx);
        let second_answer = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .unwrap()
            .options[0]
            .clone();
        window.resolve(&mut state, &mut ctx, second_answer).unwrap();
        let (second, second_is_combat) = window.take_scoring_occurrence().unwrap();
        assert!(second_is_combat);
        assert_ne!(
            first, second,
            "rule 61.7 caps each ground combat separately"
        );
    }

    #[test]
    fn an_empty_planet_needs_no_ground_combat() {
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
        let (mut table, mut dice, mut rng) = kit();

        let winner = ground_combat(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
            &planet,
            &invader(),
        )
        .unwrap();

        assert_eq!(winner, Some(invader()));
        assert_eq!(dice.count(), 0, "nobody to fight");
    }

    /// Like [`arena`], but never on Mecatol Rex, where the custodians token would change the
    /// commit flow.
    fn arena_off_mecatol() -> (GameState, SystemId, PlanetId) {
        let state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        let planets = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK);
        let (id, p) = planets
            .iter()
            .find(|(_, p)| {
                p.system_id().is_some()
                    && !p.is_placed_during_play()
                    && p.system_id() != Some(crate::seating::MECATOL)
            })
            .expect("the corpus has a placed planet outside Mecatol Rex");
        (
            state,
            SystemId::new(p.system_id().unwrap()),
            PlanetId::new(*id),
        )
    }

    /// Commits every ground force to one planet, records every prompt it sees, and answers any
    /// other prompt with its first option — enough to run a pre-fix engine through the spurious
    /// fight without panicking.
    struct CommitAndRecord {
        planet: PlanetId,
        seen: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl crate::choice::Decider for CommitAndRecord {
        fn choose(
            &mut self,
            choice: &crate::choice::Choice,
        ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            self.seen.borrow_mut().push(choice.prompt.clone());
            let wanted = format!("commit|0|{}", self.planet.as_str());
            if let Some(option) = choice.options.iter().find(|o| o.id == wanted) {
                return Ok(option.clone());
            }
            if let Some(done) = choice.options.iter().find(|o| o.id == "done_committing") {
                return Ok(done.clone());
            }
            choice
                .options
                .first()
                .cloned()
                .ok_or(crate::choice::IllegalChoice::NoOptions {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                })
        }
    }

    fn planet_units(
        state: &GameState,
        system: &SystemId,
        planet: &PlanetId,
    ) -> Vec<(String, bool)> {
        let mut units: Vec<(String, bool)> = state
            .system_state(system)
            .on_planet(planet)
            .iter()
            .map(|unit| (unit.type_id.to_string(), unit.sustained_damage))
            .collect();
        units.sort();
        units
    }

    #[test]
    fn a_ground_hit_is_sustained_then_takes_the_cheapest_force_and_never_a_structure() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "mech", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        let mut hit =
            || match apply_ground_hit(&mut state, content, POK, &system, &planet, &holder()) {
                GroundHit::Destroyed(unit) => Some(unit),
                GroundHit::Sustained(_) | GroundHit::Wasted => None,
            };
        assert!(hit().is_none(), "the mech sustains the first hit");
        assert_eq!(
            hit().map(|unit| unit.type_id.to_string()),
            Some("infantry".to_owned()),
            "then the cheapest force falls"
        );
        assert_eq!(
            hit().map(|unit| unit.type_id.to_string()),
            Some("mech".to_owned())
        );
        assert!(hit().is_none(), "nothing is left to take a hit");
        assert_eq!(
            planet_units(&state, &system, &planet),
            vec![("pds".to_owned(), false)],
            "the structure is untouched"
        );
    }

    #[test]
    fn bombardment_hits_ground_forces_only() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "spacedock", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        let taken = take_bombard_hits(
            &mut state,
            content,
            POK,
            &system,
            &planet,
            &holder(),
            3,
            &mut Vec::new(),
        );
        assert_eq!(taken, 1);
        assert_eq!(
            planet_units(&state, &system, &planet),
            vec![("spacedock".to_owned(), false)]
        );
    }

    /// A defender with a mech (SUSTAIN DAMAGE) and an infantry; `nes` gives it Non-Euclidean
    /// Shielding.
    fn nes_defender(nes: bool) -> (GameState, SystemId, PlanetId) {
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "mech", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        if nes {
            state
                .player_mut(&holder())
                .unwrap()
                .technologies
                .insert(ti4_model::id::TechnologyId::new("nes"));
        }
        (state, system, planet)
    }

    #[test]
    fn non_euclidean_shielding_cancels_two_ground_hits_per_sustain() {
        let content = ContentStore::embedded();
        for (nes, survivors) in [
            (false, vec![("mech", true)]),
            (true, vec![("infantry", false), ("mech", true)]),
        ] {
            let (mut state, system, planet) = nes_defender(nes);
            // Without the technology the second hit asks which force falls: the infantry.
            let mut table =
                Table::with_default(Box::new(crate::choice::Scripted::new(["destroy|1"])));
            absorb_ground(
                &mut state,
                content,
                POK,
                &mut table,
                &holder(),
                &system,
                &planet,
                2,
            )
            .unwrap();
            let mut left = planet_units(&state, &system, &planet);
            left.sort();
            let mut expected: Vec<(String, bool)> = survivors
                .into_iter()
                .map(|(id, damaged)| (id.to_owned(), damaged))
                .collect();
            expected.sort();
            assert_eq!(left, expected, "nes = {nes}");
        }
    }

    #[test]
    fn non_euclidean_shielding_applies_per_use_not_per_hit() {
        // Three hits: the mech sustains (cancelling two), the third hit kills the infantry.
        let content = ContentStore::embedded();
        let (mut state, system, planet) = nes_defender(true);
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(["destroy|1"])));
        absorb_ground(
            &mut state,
            content,
            POK,
            &mut table,
            &holder(),
            &system,
            &planet,
            3,
        )
        .unwrap();
        assert_eq!(
            planet_units(&state, &system, &planet),
            vec![("mech".to_owned(), true)]
        );
    }

    #[test]
    fn non_euclidean_shielding_cancels_two_bombardment_hits_per_sustain() {
        let content = ContentStore::embedded();
        for (nes, destroyed) in [(false, 1), (true, 0)] {
            let (mut state, system, planet) = nes_defender(nes);
            let taken = take_bombard_hits(
                &mut state,
                content,
                POK,
                &system,
                &planet,
                &holder(),
                2,
                &mut Vec::new(),
            );
            assert_eq!(taken, destroyed, "nes = {nes}");
            assert_eq!(
                planet_units(&state, &system, &planet).len(),
                2 - destroyed,
                "nes = {nes}"
            );
        }
    }

    #[test]
    fn non_euclidean_shielding_cancels_two_removed_ground_hits_per_sustain() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = nes_defender(true);
        remove_ground(
            &mut state,
            content,
            POK,
            &system,
            &planet,
            &holder(),
            2,
            "space_cannon_defense",
            &mut Vec::new(),
        );
        assert_eq!(planet_units(&state, &system, &planet).len(), 2);
    }

    #[test]
    fn arc_secundus_strips_the_defenders_planetary_shield() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        in_space(&mut state, &system, "dreadnought", &invader(), 1);
        assert!(!bombardable(
            &state,
            content,
            POK,
            &system,
            &planet,
            &invader()
        ));
        in_space(&mut state, &system, "letnev_flagship", &invader(), 1);
        assert!(bombardable(
            &state,
            content,
            POK,
            &system,
            &planet,
            &invader()
        ));
    }

    #[test]
    fn space_cannon_defense_fires_at_the_forces_that_landed() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::from_faces([10u32]);
        let mut rng = GameRng::new(7);
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        space_cannon_defense(&mut state, &mut ctx, &system, &planet, &invader());
        let landed = state
            .system_state(&system)
            .on_planet_of(&planet, &invader())
            .len();
        assert_eq!(landed, 1, "the PDS hit one of the two infantry");
    }

    /// Titans' Geoform (an attachment) gives its planet "SPACE CANNON 5 (x3) as if it were a unit":
    /// it fires at the invader in space cannon defense, and only for a controller who is not the
    /// invader.
    #[test]
    fn a_geoform_attachment_fires_in_space_cannon_defense() {
        let content = ContentStore::embedded();
        let landed_after = |attach: bool, controller: PlayerId| {
            let (mut state, system, planet) = arena_off_mecatol();
            on_planet(&mut state, &system, &planet, "infantry", &invader(), 3);
            state
                .system_mut(&system)
                .planet_control
                .insert(planet.clone(), controller);
            if attach {
                state
                    .planet_attachments
                    .entry(planet.clone())
                    .or_default()
                    .push("titanshero".to_owned());
            }
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let mut dice = Dice::from_faces([10u32, 10, 10]);
            let mut rng = GameRng::new(7);
            let mut ctx = crate::choice::Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: None,
            };
            space_cannon_defense(&mut state, &mut ctx, &system, &planet, &invader());
            state
                .system_state(&system)
                .on_planet_of(&planet, &invader())
                .len()
        };
        assert_eq!(landed_after(false, holder()), 3, "no attachment, no gun");
        assert_eq!(landed_after(true, holder()), 0, "three dice, three hits");
        assert_eq!(landed_after(true, invader()), 3, "the invader's own planet");
    }

    /// Tellurian before a ground hit: the window opens in space cannon defense and the cancelled
    /// hit is not assigned.
    #[test]
    fn tellurian_cancels_a_space_cannon_defense_hit_on_ground_forces() {
        let content = ContentStore::embedded();
        let run = |ready: bool| {
            let (mut state, system, planet) = arena_off_mecatol();
            on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
            on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
            if ready {
                state.player_mut(&invader()).unwrap().leaders.insert(
                    ti4_model::id::LeaderId::new("titansagent"),
                    ti4_model::state::LeaderStatus::Readied,
                );
            }
            let name = crate::promissory::faction_name(&state, &invader());
            let id = format!("leader:{name}:titansagent:GROUND_HITS_TO_ASSIGN:when");
            let mut resolver = armed_resolver(&state, vec![invader(), holder()]);
            let mut sequence = crate::event::EventSequence::new();
            let mut table = Table::with_default(Box::new(crate::choice::Scripted::new([id])));
            let mut dice = Dice::from_faces([10u32]);
            let mut rng = GameRng::new(7);
            let mut ctx = crate::choice::Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle {
                    resolver: &mut resolver,
                    sequence: &mut sequence,
                    galaxy: None,
                }),
            };
            space_cannon_defense(&mut state, &mut ctx, &system, &planet, &invader());
            let left = state
                .system_state(&system)
                .on_planet_of(&planet, &invader())
                .len();
            let agent = state
                .player(&invader())
                .unwrap()
                .leaders
                .get(ti4_model::id::text(&"titansagent"))
                .copied();
            (left, agent)
        };
        assert_eq!(run(false).0, 1, "without the card the hit lands");
        let (left, agent) = run(true);
        assert_eq!(left, 2, "the hit was cancelled");
        assert_eq!(agent, Some(ti4_model::state::LeaderStatus::Exhausted));
    }

    #[test]
    fn x89_doubles_ground_combat_hits() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 1);
        let mut rng = GameRng::new(7);
        let mut dice = Dice::from_faces([10u32]);
        let plain = roll_ground(
            &mut state,
            content,
            POK,
            &mut dice,
            &mut rng,
            &invader(),
            &system,
            &planet,
        );
        state.reroll_staging.clear();
        state
            .player_mut(&invader())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("x89c4"));
        let mut dice = Dice::from_faces([10u32]);
        let doubled = roll_ground(
            &mut state,
            content,
            POK,
            &mut dice,
            &mut rng,
            &invader(),
            &system,
            &planet,
        );
        assert_eq!((plain, doubled), (1, 2));
    }

    #[test]
    fn l4_disruptors_silence_space_cannon_defense() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
        state
            .player_mut(&invader())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("l4"));
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::from_faces([10u32]);
        let mut rng = GameRng::new(7);
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        space_cannon_defense(&mut state, &mut ctx, &system, &planet, &invader());
        assert_eq!(
            state
                .system_state(&system)
                .on_planet_of(&planet, &invader())
                .len(),
            2,
            "no cannon fired at the Letnev forces"
        );
        assert!(dice.rolled("space cannon defense").is_empty());
    }

    #[test]
    fn an_assimilated_l4_disruptors_silence_space_cannon_defense_for_the_nekro() {
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &invader(), 2);
        state.player_mut(&invader()).unwrap().faction = ti4_model::id::FactionId::new("nekro");
        state
            .player_mut(&holder())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("l4"));
        let nekro = state.player_mut(&invader()).unwrap();
        nekro
            .technologies
            .insert(ti4_model::id::TechnologyId::new("vax"));
        nekro
            .assimilated_technologies
            .insert("vax".to_owned(), ti4_model::id::TechnologyId::new("l4"));
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::from_faces([10u32]);
        let mut rng = GameRng::new(7);
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        space_cannon_defense(&mut state, &mut ctx, &system, &planet, &invader());
        assert_eq!(
            state
                .system_state(&system)
                .on_planet_of(&planet, &invader())
                .len(),
            2,
            "the assimilated text silences the cannon"
        );
        assert!(dice.rolled("space cannon defense").is_empty());
    }

    #[test]
    fn a_structure_only_planet_falls_without_resistance() {
        // LRR 49 (KD-2): structures are not ground forces, so a planet holding only rival
        // structures is uncontested — it falls without resistance and its structures are
        // destroyed when control changes hands. No fight prompt may ever be offered.
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        state.player_mut(&invader()).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        state
            .system_mut(&system)
            .set_control(planet.clone(), holder());
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "spacedock", &holder(), 1);
        in_space(&mut state, &system, "infantry", &invader(), 2);

        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(CommitAndRecord {
            planet: planet.clone(),
            seen: seen.clone(),
        }));
        // The PDS fires space cannon defense at the landing infantry first; a 1 misses, so both
        // land. Pre-fix, the spurious fight consumed the next two faces in round one.
        let mut dice = Dice::from_faces([1u32, 10, 8]);
        let mut rng = GameRng::new(7);
        let mut window = InvasionWindow::new(
            &mut state,
            content,
            POK,
            &mut dice,
            &mut rng,
            &invader(),
            &system,
        );
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        while !window.is_done() {
            crate::choice::Window::drive(&mut window, &mut state, &mut ctx).unwrap();
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
        }
        let report = window.into_report();

        assert!(
            seen.borrow().iter().all(|prompt| !prompt.contains("fight")),
            "no ground combat may be offered on a structure-only planet: {:?}",
            seen.borrow()
        );
        assert!(
            dice.rolled("ground combat").is_empty(),
            "the spurious fight consumes no dice once it stops happening"
        );
        assert_eq!(report.captured, vec![(planet.clone(), Some(holder()))]);
        let units = state
            .system_state(&system)
            .planet_units
            .get(&planet)
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            units.len(),
            2,
            "only the invader's ground forces remain: {units:?}"
        );
        assert!(
            units
                .iter()
                .all(|unit| unit.owner == invader() && unit.type_id.as_str() == "infantry"),
            "the rival structures are destroyed when control changes hands: {units:?}"
        );
    }

    #[test]
    fn structures_survive_a_legitimate_ground_fight_and_die_when_control_changes() {
        // LRR 42 (KD-2): in a real ground combat only ground forces fight and take hits. The
        // rival PDS must still stand when the last rival infantry dies; it is destroyed later,
        // when control changes hands — not by a combat hit that could never target it.
        let content = ContentStore::embedded();
        let (mut state, system, planet) = arena_off_mecatol();
        state.player_mut(&invader()).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        // The combat win must be a rival-home win so its occurrence pauses the invasion at a
        // point where the fight is over but control has not yet transferred.
        state.player_mut(&holder()).unwrap().home_system = Some(system.clone());
        state
            .system_mut(&system)
            .set_control(planet.clone(), holder());
        // The PDS is stored first, so a pre-fix casualty pool (any unit) kills it on the
        // first hit.
        on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
        in_space(&mut state, &system, "infantry", &invader(), 2);

        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(CommitAndRecord {
            planet: planet.clone(),
            seen: seen.clone(),
        }));
        // Round one only: the invader's two infantry hit (10, 8), the rival infantry misses
        // (1). Both pre-fix and post-fix the fight ends after this round.
        let mut dice = Dice::from_faces([10u32, 8, 1]);
        let mut rng = GameRng::new(7);
        let mut window = InvasionWindow::new(
            &mut state,
            content,
            POK,
            &mut dice,
            &mut rng,
            &invader(),
            &system,
        );
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };

        // Commit and fight to the combat-win pause: the win occurrence is queued before
        // control transfers, so this is the moment to inspect what the fight itself destroyed.
        crate::choice::Window::drive(&mut window, &mut state, &mut ctx).unwrap();
        let (occurrence, is_combat) = window
            .take_scoring_occurrence()
            .expect("the combat win queues a scoring occurrence");
        assert!(is_combat, "the pause holds the ground-combat win");
        assert!(state.did_at_occurrence(&invader(), Feat::WonInARivalHome, occurrence));

        let mid: Vec<Unit> = state
            .system_state(&system)
            .planet_units
            .get(&planet)
            .cloned()
            .unwrap_or_default();
        assert!(
            mid.iter()
                .any(|unit| unit.type_id.as_str() == "pds" && unit.owner == holder()),
            "the PDS survived the ground combat itself: {mid:?}"
        );

        while !window.is_done() {
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
        }
        let report = window.into_report();

        assert_eq!(report.captured, vec![(planet.clone(), Some(holder()))]);
        let units = state
            .system_state(&system)
            .planet_units
            .get(&planet)
            .cloned()
            .unwrap_or_default();
        assert!(
            units
                .iter()
                .all(|unit| unit.owner == invader() && unit.type_id.as_str() == "infantry"),
            "the PDS dies when control changes hands, not in the fight: {units:?}"
        );
    }

    #[test]
    fn an_invasion_with_no_troops_commits_nothing() {
        // 49.2c: straight on to Production.

        let (mut state, system, _) = arena();
        let (mut table, mut dice, mut rng) = kit();

        let report = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
            &invader(),
        )
        .unwrap();

        assert!(report.committed.is_empty());
        assert!(report.captured.is_empty());
    }

    #[test]
    fn a_whole_invasion_takes_an_undefended_planet() {
        let (mut state, system, _) = arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        let (mut table, mut dice, mut rng) = kit();

        let report = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
            &invader(),
        )
        .unwrap();

        assert!(!report.captured.is_empty(), "the planet changed hands");
        for (planet, _) in &report.captured {
            assert!(state.exhausted_planets.contains(planet));
        }
    }

    type RecordedAsk = (String, Vec<(String, String, String)>);

    /// A decider that records every choice it is asked to answer, answering from a queue of ids.
    struct CommitRecording {
        wanted: std::collections::VecDeque<String>,
        seen: std::rc::Rc<std::cell::RefCell<Vec<RecordedAsk>>>,
    }

    impl CommitRecording {
        fn new(wanted: &[String]) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<RecordedAsk>>>) {
            let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    wanted: wanted.iter().cloned().collect(),
                    seen: seen.clone(),
                },
                seen,
            )
        }

        fn record(&self, choice: &crate::choice::Choice) {
            self.seen.borrow_mut().push((
                choice.prompt.clone(),
                choice
                    .options
                    .iter()
                    .map(|option| (option.id.clone(), option.kind.clone(), option.label.clone()))
                    .collect(),
            ));
        }
    }

    impl crate::choice::Decider for CommitRecording {
        fn choose(
            &mut self,
            choice: &crate::choice::Choice,
        ) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            self.record(choice);
            let Some(wanted) = self.wanted.pop_front() else {
                return Err(crate::choice::IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted: "<script exhausted>".to_owned(),
                    offered: choice.ids().into_iter().map(str::to_owned).collect(),
                });
            };
            choice.option(&wanted).cloned().ok_or_else(|| {
                crate::choice::IllegalChoice::ScriptDiverged {
                    player: choice.player.clone(),
                    wanted,
                    offered: choice.ids().into_iter().map(str::to_owned).collect(),
                }
            })
        }
    }

    /// A system the corpus places at least two planets in, so a landing is a real choice.
    fn two_planet_arena() -> (GameState, SystemId, PlanetId, PlanetId) {
        let state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        let content = ContentStore::embedded();
        let systems: std::collections::BTreeSet<&str> =
            ti4_content::galaxy::all_planets(content, POK)
                .iter()
                .filter_map(|(_, planet)| planet.system_id())
                .collect();
        for system in &systems {
            // F-M08-019-1 (C1): pa/pb follow the system record's own `planets` array
            // (canonical order), so option-order assertions track the oracle rather than the
            // planets.json layout. System 09 is selected: record [maaluuk, druua] vs file
            // order [druaa, maaluuk].
            if let Some(record) =
                content.get(ti4_model::content_types::ContentType::Systems, system)
            {
                let planets: Vec<PlanetId> = record
                    .strings("planets")
                    .into_iter()
                    .map(PlanetId::new)
                    .collect();
                if planets.len() >= 2 {
                    return (
                        state,
                        SystemId::new(*system),
                        planets[0].clone(),
                        planets[1].clone(),
                    );
                }
            }
        }
        panic!("the corpus has no two-planet system")
    }

    #[test]
    fn commit_ground_forces_offers_the_oracle_identity() {
        // engine/invasion.py:253–324 asks "commit ground forces in {system}" with ids
        // commit|{i}|{planet}, kind "commit", labels "land infantry on {p}", and the terminator
        // ("done_committing", "decline", "commit no more ground forces"). Two identical undamaged
        // infantry over two planets are one move each, so unit 1 contributes no options.
        let (mut state, system, pa, pb) = two_planet_arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        let script = vec![format!("commit|0|{pa}"), "done_committing".to_owned()];
        let (recorder, seen) = CommitRecording::new(&script);
        let mut table = Table::with_default(Box::new(recorder));

        let committed = commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        assert_eq!(committed, vec![pa.clone()]);
        let asks = seen.borrow();
        assert_eq!(asks.len(), 2, "one landing, then the decline ask");
        assert_eq!(asks[0].0, format!("commit ground forces in {system}"));
        assert_eq!(
            asks[0].1,
            vec![
                (
                    format!("commit|0|{pa}"),
                    "commit".to_owned(),
                    format!("land infantry on {pa}")
                ),
                (
                    format!("commit|0|{pb}"),
                    "commit".to_owned(),
                    format!("land infantry on {pb}")
                ),
                (
                    "done_committing".to_owned(),
                    "decline".to_owned(),
                    "commit no more ground forces".to_owned()
                )
            ]
        );
        assert_eq!(asks[1].0, format!("commit ground forces in {system}"));
    }

    /// A decider that answers the first offered option and keeps every `Choice` it was asked,
    /// context included -- `CommitRecording` deliberately does not, so this is separate rather
    /// than widening a shared, already-approved test helper.
    struct FirstOptionCapturing {
        seen: std::rc::Rc<std::cell::RefCell<Vec<crate::choice::Choice>>>,
    }

    impl crate::choice::Decider for FirstOptionCapturing {
        fn choose(
            &mut self,
            choice: &crate::choice::Choice,
        ) -> Result<ChoiceOption, IllegalChoice> {
            self.seen.borrow_mut().push(choice.clone());
            choice
                .options
                .first()
                .cloned()
                .ok_or_else(|| IllegalChoice::NoOptions {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                })
        }
    }

    /// OBS-003d: committing ground forces names its rule and the system landed in.
    #[test]
    fn obs003d_commit_ground_forces_carries_its_typed_context() {
        let (mut state, system, planet, _) = two_planet_arena();
        in_space(&mut state, &system, "infantry", &invader(), 1);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(FirstOptionCapturing { seen: seen.clone() }));

        commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        let asked = seen.borrow();
        let context = asked[0].context.as_ref().expect("typed context");
        assert_eq!(context.source, DecisionSource::Rule("49".to_owned()));
        assert_eq!(context.subtype, "commit_ground_forces");
        assert_eq!(context.target, Some(DecisionTarget::System(system)));
        let _ = planet;
    }

    /// OBS-008a4: each landing option previews the invader's own ground-force count on that
    /// planet reaching one more, immediately -- what 49.2 itself does -- and claims nothing about
    /// the ground combat and control transfer that may follow. The decline option has no preview.
    #[test]
    fn obs008a4_commit_options_preview_the_invaders_own_ground_count_rising_by_one() {
        let (mut state, system, pa, pb) = two_planet_arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        // A defender already standing on `pa` proves the preview counts only the invader's own.
        state
            .system_mut(&system)
            .planet_units
            .entry(pa.clone())
            .or_default()
            .push(Unit::new(UnitTypeId::new("infantry"), PlayerId::new("b")));
        // A PDS is owned by the invader but is a structure, not a ground force. It must not
        // inflate the previewed count (unlike the Titans' PDS II, which is a ground force).
        on_planet(&mut state, &system, &pa, "pds", &invader(), 1);

        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(FirstOptionCapturing { seen: seen.clone() }));
        commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        let asked = seen.borrow();
        let first = &asked[0];
        for option in &first.options {
            if option.is_decline() {
                assert!(option.preview.is_none(), "declining to land has no preview");
                continue;
            }
            assert!(
                option.id.ends_with(&format!("|{pa}")) || option.id.ends_with(&format!("|{pb}")),
                "unexpected option {}",
                option.id
            );
            match &option
                .preview
                .as_ref()
                .expect("a landing is previewed")
                .outcome
            {
                crate::preview::Outcome::Certain { deltas } => {
                    // 0 -> 1 on both planets: the invader has no ground forces on either yet, on
                    // pa the defender's own presence notwithstanding -- this previews only the
                    // invader's own count.
                    assert_eq!(deltas, &[Delta::new(Quantity::GroundForcesOnPlanet, 0, 1)]);
                }
                other => panic!("a landing preview is certain, got {other:?}"),
            }
        }

        // `FirstOptionCapturing` answered the first offered option -- whichever planet that
        // named -- so the second ask's option for that same planet previews 1 -> 2: the first
        // preview's `after` is what the next choice's `before` actually is.
        let landed_on = first.options[0]
            .payload
            .get("planet")
            .and_then(serde_json::Value::as_str)
            .expect("the answered option names a planet")
            .to_owned();
        let second = &asked[1];
        let landed_again = second
            .options
            .iter()
            .find(|option| {
                option
                    .payload
                    .get("planet")
                    .and_then(serde_json::Value::as_str)
                    == Some(landed_on.as_str())
            })
            .expect("the same planet is still landable");
        match &landed_again.preview.as_ref().unwrap().outcome {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(deltas, &[Delta::new(Quantity::GroundForcesOnPlanet, 1, 2)]);
            }
            other => panic!("a landing preview is certain, got {other:?}"),
        }
    }

    /// OBS-003d: removing the custodians token is typed distinctly from committing ground
    /// forces, though both arise from the same invasion of Mecatol Rex.
    #[test]
    fn obs003d_custodians_removal_carries_its_typed_context() {
        let content = ContentStore::embedded();
        let player = invader();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.board.entry(mecatol.clone()).or_default();
        crate::fixtures::put(&mut state, &mecatol, "infantry", &player, 1);
        if let Some(seat) = state.player_mut(&player) {
            seat.trade_goods = i32::try_from(CUSTODIANS_COST).unwrap();
        }
        assert!(custodians_removable(
            &state, content, POK, &player, &mecatol
        ));

        let window = InvasionWindow {
            invader: player,
            system: mecatol,
            stage: Stage::Custodians,
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let choice = window
            .pending_choice(&state, content, POK)
            .expect("the custodians question is asked before landing");
        let context = choice.context.as_ref().expect("typed context");
        assert_eq!(context.source, DecisionSource::Rule("27.2".to_owned()));
        assert_eq!(context.subtype, "remove_custodians");
    }

    #[test]
    fn commit_options_follow_the_system_record_planet_order() {
        // F-M08-019-1 (C1): landing options must follow the system record's own `planets`
        // array, not the file layout of planets.json. System 09 prints [maaluuk, druua]; in
        // file order druaa precedes maaluuk — under the old code the option ids came out
        // swapped. Verified RED before the fix.
        let mut state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        let system = SystemId::new("09");
        in_space(&mut state, &system, "infantry", &invader(), 1);
        let (recorder, seen) = CommitRecording::new(&["done_committing".to_owned()]);
        let mut table = Table::with_default(Box::new(recorder));

        commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        let asks = seen.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(
            asks[0].1,
            vec![
                (
                    "commit|0|maaluuk".to_owned(),
                    "commit".to_owned(),
                    "land infantry on maaluuk".to_owned()
                ),
                (
                    "commit|0|druaa".to_owned(),
                    "commit".to_owned(),
                    "land infantry on druaa".to_owned()
                ),
                (
                    "done_committing".to_owned(),
                    "decline".to_owned(),
                    "commit no more ground forces".to_owned()
                )
            ]
        );
    }

    #[test]
    fn commit_options_distinguish_sustained_damage() {
        // engine/choice.py:96 unit_label shows damage rather than folding it away; the dedup key
        // is (type, sustained damage, planet), so a damaged infantry is its own options.
        let (mut state, system, pa, pb) = two_planet_arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        state
            .system_mut(&system)
            .units
            .last_mut()
            .expect("two troops are in space")
            .sustained_damage = true;
        let (recorder, seen) = CommitRecording::new(&["done_committing".to_owned()]);
        let mut table = Table::with_default(Box::new(recorder));

        commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        let asks = seen.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(
            asks[0].1,
            vec![
                (
                    format!("commit|0|{pa}"),
                    "commit".to_owned(),
                    format!("land infantry on {pa}")
                ),
                (
                    format!("commit|0|{pb}"),
                    "commit".to_owned(),
                    format!("land infantry on {pb}")
                ),
                (
                    format!("commit|1|{pa}"),
                    "commit".to_owned(),
                    format!("land infantry (damaged) on {pa}")
                ),
                (
                    format!("commit|1|{pb}"),
                    "commit".to_owned(),
                    format!("land infantry (damaged) on {pb}")
                ),
                (
                    "done_committing".to_owned(),
                    "decline".to_owned(),
                    "commit no more ground forces".to_owned()
                )
            ]
        );
    }

    #[test]
    fn lifting_the_custodians_token_costs_six_influence_and_pays_a_point() {
        // 27.2/27.3. Until this existed, every assignment to `custodians_removed` in the whole
        // codebase was inside a test, so the agenda phase -- gated on the token by 8.1 -- never
        // ran in a simulated game, and every law and agenda victory point was unreachable.
        let content = ContentStore::embedded();
        let mut state = start_game(content, &[invader(), holder()], POK, None).unwrap();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        assert!(!state.custodians_removed);

        // A freshly started game controls no planets, so the seat is funded with trade goods --
        // spendable as influence -- rather than by hand-placing planet control.
        if let Some(seat) = state.player_mut(&invader()) {
            seat.trade_goods = 6;
        }
        // 27.2a: the token cannot be lifted by a seat with nothing to commit, so the invasion that
        // lifts it has an army in the system. Funding influence alone used to be enough here,
        // which is the gap that rule closes.
        state.board.entry(mecatol.clone()).or_default();
        crate::fixtures::put(&mut state, &mecatol, "infantry", &invader(), 1);
        let influence = crate::production::available(
            &state,
            content,
            POK,
            &invader(),
            crate::production::Spend::Influence,
        );
        assert!(
            influence >= CUSTODIANS_COST,
            "a starting seat should be able to afford the token, had {influence}"
        );
        assert!(custodians_removable(
            &state,
            content,
            POK,
            &invader(),
            &mecatol
        ));

        let before = state
            .player(&invader())
            .map_or(0, |seat| seat.victory_points);
        let mut window = InvasionWindow {
            invader: invader(),
            system: mecatol.clone(),
            stage: Stage::Custodians,
            report: InvasionReport::default(),
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(&state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let choice = window
            .pending_choice(&state, content, POK)
            .expect("the custodians ask is offered on Mecatol");
        assert!(
            choice.prompt.contains("custodians"),
            "got {}",
            choice.prompt
        );

        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        let yes = choice
            .options
            .iter()
            .find(|option| option.id == "yes")
            .cloned()
            .expect("the accepting option exists");
        window.resolve(&mut state, &mut ctx, yes).unwrap();

        assert!(state.custodians_removed, "the token comes off");
        assert_eq!(
            state
                .player(&invader())
                .map_or(0, |seat| seat.victory_points),
            before + 1,
            "27.3 pays a victory point"
        );
        let after = crate::production::available(
            &state,
            content,
            POK,
            &invader(),
            crate::production::Spend::Influence,
        );
        assert!(
            after <= influence - CUSTODIANS_COST,
            "six influence was spent"
        );
    }

    #[test]
    fn mecatol_is_not_offered_while_the_custodians_token_is_present() {
        // 27.1: nobody lands on Mecatol Rex while the custodians token sits there.
        // System 18 holds exactly one planet ("mr"), so with the token up the commit ask
        // offers nothing but the terminator; without it, the landing is offered.
        let content = ContentStore::embedded();
        let mut state = start_game(content, &[invader(), holder()], POK, None).unwrap();
        assert!(!state.custodians_removed);
        let mecatol_system = SystemId::new("18");
        in_space(&mut state, &mecatol_system, "infantry", &invader(), 1);

        let (recorder, seen) = CommitRecording::new(&["done_committing".to_owned()]);
        let mut table = Table::with_default(Box::new(recorder));
        commit_ground_forces(
            &mut state,
            content,
            POK,
            &mut table,
            &invader(),
            &mecatol_system,
        )
        .unwrap();
        let asks = seen.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(
            asks[0].1, // the token keeps Mecatol Rex off the table
            vec![(
                "done_committing".to_owned(),
                "decline".to_owned(),
                "commit no more ground forces".to_owned()
            )]
        );

        state.custodians_removed = true;
        let (recorder, seen) = CommitRecording::new(&["done_committing".to_owned()]);
        let mut table = Table::with_default(Box::new(recorder));
        commit_ground_forces(
            &mut state,
            content,
            POK,
            &mut table,
            &invader(),
            &mecatol_system,
        )
        .unwrap();
        let asks = seen.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(
            asks[0].1, // without the token, Mecatol Rex lands like any planet
            vec![
                (
                    "commit|0|mr".to_owned(),
                    "commit".to_owned(),
                    "land infantry on mr".to_owned()
                ),
                (
                    "done_committing".to_owned(),
                    "decline".to_owned(),
                    "commit no more ground forces".to_owned()
                )
            ]
        );
    }

    #[test]
    fn the_invasion_window_commit_ask_uses_the_oracle_identity() {
        // The staged window is the real-game path (armed from game.rs); it builds its own copy of
        // the option list, so the surface is asserted here rather than assumed shared.
        let (mut state, system, pa, pb) = two_planet_arena();
        in_space(&mut state, &system, "infantry", &invader(), 2);
        let (mut table, mut dice, mut rng) = kit();

        let mut window = InvasionWindow::new(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &invader(),
            &system,
        );
        let content = ContentStore::embedded();
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        // A fresh window sits on its (choice-free) bombardment until it settles.
        window.settle(&mut state, &mut ctx);
        let choice = window
            .pending_choice(&state, content, POK)
            .expect("troops in space mean a commit ask");

        assert_eq!(choice.prompt, format!("commit ground forces in {system}"));
        assert_eq!(
            choice
                .options
                .iter()
                .map(|o| o.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                format!("commit|0|{pa}"),
                format!("commit|0|{pb}"),
                "done_committing".to_owned()
            ]
        );
        assert!(
            choice
                .options
                .iter()
                .all(|o| o.kind == "commit" || o.id == "done_committing")
        );
    }

    // -- reroll windows: Fire Team, Scramble Frequency, Aglnlan Oln ----------------
    //
    // These drive the real window machinery (armed reaction slots, staged dice, recompute
    // after the window) rather than calling the card effects directly, so a reroll that only
    // worked on the direct path cannot pass here.

    /// A resolver armed exactly as the driver arms one: a standing slot per player per
    /// window, the hand read at resolution time.
    fn armed_resolver(state: &GameState, players: Vec<PlayerId>) -> crate::timing::Resolver {
        let mut resolver = crate::timing::Resolver::new(
            players,
            Some(PlayerId::new("a")),
            crate::choice::Table::default(),
        );
        crate::reactions::arm(&mut resolver, state);
        resolver
    }

    /// Builds the manual invasion window the card tests drive, and runs it to completion.
    /// `script` is the exact answer to every question the run makes (window offers, commit
    /// offers, card offers); `dice` and `rng` pin the rolls. Returns the questions that were
    /// offered, in order.
    fn drive_invasion(
        state: &mut GameState,
        system: &SystemId,
        stage: Stage,
        report: InvasionReport,
        script: Vec<String>,
        dice: &mut Dice,
        rng: &mut GameRng,
    ) -> Vec<Vec<String>> {
        let a = invader();
        let b = holder();
        let content = ContentStore::embedded();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let decider = RecordingScripted {
            inner: crate::choice::Scripted::new(script),
            seen: seen.clone(),
        };
        let mut table = Table::with_default(Box::new(decider));
        let mut resolver = armed_resolver(state, vec![a.clone(), b.clone()]);
        let mut event_sequence = crate::event::EventSequence::new();
        let mut window = InvasionWindow {
            invader: a,
            system: system.clone(),
            stage,
            report,
            pending_scoring_occurrences: std::collections::VecDeque::new(),
            current_ground_occurrence: None,
            notes_at_tactical_start: crate::combat::note_holdings(state),
            bombard_plan: Vec::new(),
            bombard_index: 0,
            bombard_occurrence: state.begin_feat_occurrence(),
            bombard_announced: true,
            extra_commits: Vec::new(),
            temporary_space_commits: std::collections::BTreeMap::new(),
            combat_only: false,
            strict_timing_error: None,
            galaxy: None,
        };
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice,
            rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut event_sequence,
                galaxy: None,
            }),
        };
        while !window.is_done() {
            window
                .drive(state, &mut ctx)
                .unwrap_or_else(|e| panic!("the script only picks what is offered: {e:?}; the questions so far: {:?}", seen.borrow()));
            while window.take_scoring_occurrence().is_some() {}
            window.settle(state, &mut ctx);
        }
        seen.borrow().clone()
    }

    #[test]
    fn fire_team_rerolls_your_own_ground_dice_before_anyone_is_removed() {
        // Fire Team: "After your ground forces make combat rolls during a round of ground
        // combat: Reroll any number of your dice." One infantry each (both hit on 8): the
        // invader's face 8 kills the defender's only infantry, and the defender's 3 kills
        // nothing. The reroll draws from the seeded stream (seed 4's first DICE draw is an 8),
        // so it kills the invader's only infantry too: both sides die in the same round and
        // the defender holds the planet.
        let a = invader();
        let b = holder();
        let (system, planet) = {
            let (_, s, p) = arena();
            (s, p)
        };

        let run = |with_card: bool| -> (Vec<Vec<String>>, GameState, Dice) {
            let (mut state, system, planet) = arena();
            state
                .system_mut(&system)
                .set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &a, 1);
            on_planet(&mut state, &system, &planet, "infantry", &b, 1);
            if with_card {
                state.player_mut(&b).unwrap().action_cards =
                    vec![ti4_model::id::ActionCardId::new("fire_team")];
            }
            // The preload feeds the two initial rolls only; a reroll re-draws from the seeded
            // stream (see Dice::reroll), and seed 4's first DICE-domain draw is an 8.
            let mut dice = Dice::from_faces([8u32, 3]);
            let mut rng = GameRng::new(4);
            let script: Vec<String> = if with_card {
                vec![
                    "fight".to_owned(),
                    "reaction:generic:GROUND_ROLLS_MADE:after".to_owned(),
                    "reroll|0:0".to_owned(),
                ]
            } else {
                vec!["fight".to_owned()]
            };
            let asks = drive_invasion(
                &mut state,
                &system,
                Stage::Fighting {
                    planets: vec![planet.clone()],
                    index: 0,
                    defender: b.clone(),
                },
                InvasionReport {
                    // The manual window models a committed landing, so a surviving invader can
                    // establish control the way the Committing stage would record it.
                    committed: vec![planet.clone()],
                    ..InvasionReport::default()
                },
                script,
                &mut dice,
                &mut rng,
            );
            (asks, state, dice)
        };

        // The card-less fight: the 8 kills the defender's only infantry, the 3 kills nothing,
        // the invader takes the planet, and nothing was ever rerolled.
        let (asks, state, dice) = run(false);
        assert_eq!(
            asks,
            vec![vec!["fight".to_owned()]],
            "no card in hand, so no reaction window opens; got {asks:?}"
        );
        let units: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(units.len(), 1, "the invader's infantry is all that is left");
        assert_eq!(units[0].owner, a);
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&a),
            "the surviving invader holds the planet"
        );
        assert!(dice.rolled("fire team").is_empty(), "no card, no reroll");

        // With Fire Team: the defender rerolls the 3; the re-draw is an 8 (seed 4), so the
        // defender kills the invader's only infantry as well. Both sides die in the same
        // round, and the defender holds the planet.
        let (asks, state, dice) = run(true);
        assert_eq!(
            asks,
            vec![
                vec!["fight".to_owned()],
                vec![
                    "reaction:generic:GROUND_ROLLS_MADE:after".to_owned(),
                    "decline".to_owned()
                ],
                vec!["reroll|0:0".to_owned(), "decline".to_owned()],
            ],
            // The single playable card is auto-selected, so no inner card ask is recorded.
            "the round ask, the reaction offer, then one die question; got {asks:?}"
        );
        assert!(
            state.reroll_staging.is_empty(),
            "the staging is spent with the round"
        );
        let units: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert!(
            units.is_empty(),
            "the reroll let the defender kill the invader's last infantry: both sides died"
        );
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b),
            "the invader died, so the defender keeps the planet"
        );
        assert!(
            state.player(&b).unwrap().action_cards.is_empty(),
            "the card was spent"
        );
        // The reroll ran through the game's own roller, recorded under the card's name.
        assert_eq!(
            dice.rolled("fire team").len(),
            1,
            "the die the decider named was the only one re-drawn"
        );
    }

    /// Fire Team played from the reaction slot of a real ground combat round (the dice come from the game's own generator, the staged
    /// set is the one the roll step staged), with `Area::ActionCards` on `Dual` and the invasion on the string: the announcement is
    /// the top-level window, so the core's Fire Team window runs on the position in flight and is compared with the string.
    #[cfg(feature = "typed-core")]
    #[test]
    fn fire_team_played_from_the_slot_of_a_real_ground_combat_agrees_with_the_core_window() {
        let _serial = crate::dual::tests::serial();
        let (a, b) = (invader(), holder());
        for answers in [&["reroll|0:0", "reroll|0:1"][..], &["decline", "reroll|0:1"][..], &["reroll|0:0", "decline"][..], &["decline", "decline"][..]] {
            let (mut state, system, planet) = arena();
            for (seat, faction) in state.players.iter_mut().zip(["arborec", "hacan"]) {
                seat.faction = ti4_model::id::FactionId::new(faction);
            }
            state.promissory_notes.clear();
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &a, 2);
            on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            state.player_mut(&b).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("fire_team")];
            state.rng_seed = 4;
            let mut dice = Dice::new();
            let mut rng = GameRng::new(4);
            let slot = format!("reaction:{}:GROUND_ROLLS_MADE:after", crate::promissory::faction_name(&state, &b));
            let mut script = vec!["fight".to_owned(), slot];
            script.extend(answers.iter().map(|a| (*a).to_owned()));
            let _ = crate::dual::take_report();
            let mask = crate::dual::AreaMask::STRING.with(crate::dual::Area::ActionCards, crate::dual::Backend::Dual);
            let asks = crate::dual::with_mask(mask, || {
                drive_invasion(
                    &mut state,
                    &system,
                    Stage::Fighting { planets: vec![planet.clone()], index: 0, defender: b.clone() },
                    InvasionReport { committed: vec![planet.clone()], ..InvasionReport::default() },
                    script,
                    &mut dice,
                    &mut rng,
                )
            });
            let report = crate::dual::report();
            let shown: String = format!("{:?}", report.differences).chars().take(1200).collect();
            assert!(report.differences.is_empty(), "{answers:?}: {shown}");
            let w = report.windows.get("action_cards::announce").cloned().unwrap_or_default();
            assert_eq!((w.compared, w.bailed), (1, 0), "{answers:?}: the core window ran on the position in flight: {:?} / {asks:?}", report.windows);
            assert!(asks.iter().any(|q| q.iter().any(|o| o.starts_with("reroll|"))), "{answers:?}: the die questions were asked");
            let _ = crate::dual::take_report();
        }
    }

    /// The Nomad's The Thundarian: "After the \"Roll Dice\" step of combat: You may exhaust this
    /// card. If you do, hits are not assigned to either player's units. Return to the start of this
    /// combat round's \"Roll Dice\" step." One infantry each (both hit on 8). The first roll would
    /// kill both; with the card it is discarded and the step is rolled again.
    #[test]
    fn the_thundarian_replays_the_ground_roll_without_assigning_its_hits() {
        let (a, b) = (invader(), holder());
        let run = |used: bool| {
            let (mut state, system, planet) = arena();
            state
                .system_mut(&system)
                .set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &a, 1);
            on_planet(&mut state, &system, &planet, "infantry", &b, 1);
            state.player_mut(&a).unwrap().leaders.insert(
                ti4_model::id::LeaderId::new("nomadagentthundarian"),
                ti4_model::state::LeaderStatus::Readied,
            );
            let name = crate::promissory::faction_name(&state, &a);
            let window =
                format!("leader:{name}:nomadagentthundarian:GROUND_COMBAT_ROLL_STEP_ENDED:after");
            let script = vec![
                "fight".to_owned(),
                if used { window } else { "decline".to_owned() },
            ];
            // Round 1's roll: both hit. The replayed roll: the invader hits, the defender misses.
            let mut dice = Dice::from_faces([8u32, 8, 8, 3]);
            let mut rng = GameRng::new(4);
            let asks = drive_invasion(
                &mut state,
                &system,
                Stage::Fighting {
                    planets: vec![planet.clone()],
                    index: 0,
                    defender: b.clone(),
                },
                InvasionReport {
                    committed: vec![planet.clone()],
                    ..InvasionReport::default()
                },
                script,
                &mut dice,
                &mut rng,
            );
            let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
            let agent = state
                .player(&a)
                .unwrap()
                .leaders
                .get(ti4_model::id::text(&"nomadagentthundarian"))
                .copied();
            (
                asks,
                left,
                state,
                system,
                planet,
                dice.history().len(),
                agent,
            )
        };

        let (asks, left, state, system, planet, rolls, agent) = run(true);
        assert_eq!(rolls, 4, "two rolls, discarded, then two more");
        assert_eq!(left.len(), 1, "only the defender's infantry fell");
        assert_eq!(left[0].owner, a);
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&a)
        );
        assert_eq!(agent, Some(ti4_model::state::LeaderStatus::Exhausted));
        assert!(
            !state
                .faction_marks
                .contains_key(crate::combat::ROLL_REPLAY_MARK),
            "the request is spent"
        );
        assert_eq!(asks.len(), 2, "the round, then the one offer; got {asks:?}");

        // Declined, the same first roll kills both sides and the defender keeps the planet.
        let (_, left, state, system, planet, rolls, agent) = run(false);
        assert_eq!(rolls, 2);
        assert!(left.is_empty());
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b)
        );
        assert_eq!(agent, Some(ti4_model::state::LeaderStatus::Readied));
    }

    /// Package W1: a readied The Thundarian does not stop the tail of the ground roll step from running as a core window. The card
    /// listens to the event that ends the step (outside the tail), and a replay rolls the pair and runs the tail again. The same
    /// fight as `the_thundarian_replays_the_ground_roll_without_assigning_its_hits`, with the invasion on `Dual`.
    #[cfg(feature = "typed-core")]
    #[test]
    fn the_thundarian_watching_does_not_stop_the_tail_window_in_lockstep() {
        let _serial = crate::dual::tests::serial();
        let (a, b) = (invader(), holder());
        for (holder_of_card, used) in [(false, false), (true, false), (true, true)] {
            for nomad_seat in [a.clone(), b.clone()] {
                let (mut state, system, planet) = arena();
                // Real faction ids: the codec refuses the fixture faction, and the core half would silently bail.
                for (seat, faction) in state.players.iter_mut().zip(if nomad_seat == a { ["nomad", "hacan"] } else { ["arborec", "nomad"] }) {
                    seat.faction = ti4_model::id::FactionId::new(faction);
                }
                state.promissory_notes.clear();
                state.system_mut(&system).set_control(planet.clone(), b.clone());
                on_planet(&mut state, &system, &planet, "infantry", &a, 1);
                on_planet(&mut state, &system, &planet, "infantry", &b, 1);
                if holder_of_card {
                    state.player_mut(&nomad_seat).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nomadagentthundarian"), ti4_model::state::LeaderStatus::Readied);
                }
                let name = crate::promissory::faction_name(&state, &nomad_seat);
                let window = format!("leader:{name}:nomadagentthundarian:GROUND_COMBAT_ROLL_STEP_ENDED:after");
                let script = vec!["fight".to_owned(), if used { window } else { "decline".to_owned() }];
                let mut dice = Dice::new();
                let mut rng = GameRng::new(4);
                state.rng_seed = 4;
                let _ = crate::dual::take_report();
                let mask = crate::dual::AreaMask::STRING.with(crate::dual::Area::Invasion, crate::dual::Backend::Dual);
                let asks = crate::dual::with_mask(mask, || {
                    drive_invasion(
                        &mut state,
                        &system,
                        Stage::Fighting { planets: vec![planet.clone()], index: 0, defender: b.clone() },
                        InvasionReport { committed: vec![planet.clone()], ..InvasionReport::default() },
                        script,
                        &mut dice,
                        &mut rng,
                    )
                });
                let report = crate::dual::report();
                let shown: String = format!("{:?}", report.differences).chars().take(1200).collect();
                assert!(report.differences.is_empty(), "{holder_of_card}/{used}/{nomad_seat}: {shown}");
                let w = report.windows.get("invasion::roll_ground_tail").cloned().unwrap_or_default();
                assert!(w.compared > 0, "{holder_of_card}/{used}/{nomad_seat}: the tail ran in the core: {:?} / {asks:?}", report.windows);
                assert!(
                    report.windows.keys().all(|k| !k.contains("invasion::roll_ground_tail [bail: The Thundarian")),
                    "{holder_of_card}/{used}/{nomad_seat}: {:?}",
                    report.windows
                );
                let _ = crate::dual::take_report();
            }
        }
    }

    /// Package W1: Naaz's Supercharge and the repair of the Eidolon Maximum at the start of a ground round run in the core window
    /// (`ground_combat_round_started` of the Naaz crate; the question rides on the tape of the step). Both answers, both sides
    /// holding the technology, a damaged Maximum, with the invasion on `Dual`: no difference, the window ran in the core and the
    /// +1 shows in the dice.
    #[cfg(feature = "typed-core")]
    #[test]
    fn naaz_supercharge_and_the_maximum_repair_run_in_the_core_begin_window_in_lockstep() {
        let _serial = crate::dual::tests::serial();
        let (a, b) = (invader(), holder());
        for (answer, seed) in [("sc", 4_u64), ("decline", 4), ("sc", 9), ("decline", 9)] {
            let (mut state, system, planet) = arena();
            for (seat, faction) in state.players.iter_mut().zip(["naaz", "hacan"]) {
                seat.faction = ti4_model::id::FactionId::new(faction);
            }
            state.promissory_notes.clear();
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &a, 2);
            on_planet(&mut state, &system, &planet, "naaz_voltron", &a, 1);
            on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            for unit in state.system_mut(&system).planet_units.get_mut(&planet).unwrap() {
                if unit.type_id.as_str() == "naaz_voltron" {
                    unit.sustained_damage = true;
                }
            }
            state.player_mut(&a).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("sc"));
            // As in a game: the invasion is active, so the question names it (`invasion=<seq>` in the decision context).
            state.active_invasion = Some(ti4_model::state::ActiveInvasion {
                system: system.clone(),
                invader: a.clone(),
                seq: 88,
                phase: "fighting".to_owned(),
                planet: None,
                defender: None,
                ground_round: 0,
                last_step: None,
            });
            state.rng_seed = seed;
            let mut dice = Dice::new();
            let mut rng = GameRng::new(seed);
            let script = vec!["fight".to_owned(), answer.to_owned()];
            let _ = crate::dual::take_report();
            let mask = crate::dual::AreaMask::STRING.with(crate::dual::Area::Invasion, crate::dual::Backend::Dual);
            let asks = crate::dual::with_mask(mask, || {
                drive_invasion(
                    &mut state,
                    &system,
                    Stage::Fighting { planets: vec![planet.clone()], index: 0, defender: b.clone() },
                    InvasionReport { committed: vec![planet.clone()], ..InvasionReport::default() },
                    script,
                    &mut dice,
                    &mut rng,
                )
            });
            let report = crate::dual::report();
            let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
            assert!(report.differences.is_empty(), "{answer}/{seed}: {shown}");
            let w = report.windows.get("invasion::begin_ground_round").cloned().unwrap_or_default();
            assert!(w.compared > 0 && w.bailed == 0, "{answer}/{seed}: the opening ran in the core: {:?}", report.windows);
            assert!(asks.iter().any(|q| q.iter().any(|o| o == "sc")), "{answer}/{seed}: Supercharge was offered: {asks:?}");
            // (A later round of the fight asks again and the scripted decider has run out of answers: only a used card is certain.)
            if answer == "sc" {
                assert!(state.player(&a).unwrap().exhausted_technologies.contains(ti4_model::id::text(&"sc")), "{answer}/{seed}: the card is exhausted when it was used");
            }
            let _ = crate::dual::take_report();
        }
    }

    /// A system with two non-station planets, so a relocation has somewhere to go.
    fn dual_arena() -> (GameState, SystemId, PlanetId, PlanetId) {
        let content = ContentStore::embedded();
        let (system, planets) = ti4_content::galaxy::all_systems(content, POK)
            .iter()
            .find_map(|(name, _)| {
                let record = content.get(ti4_model::content_types::ContentType::Systems, name)?;
                let planets: Vec<PlanetId> = record
                    .strings("planets")
                    .into_iter()
                    .map(PlanetId::new)
                    .filter(|planet| {
                        !ti4_content::galaxy::is_space_station(content, planet.as_str(), POK)
                    })
                    .collect();
                (planets.len() >= 2).then(|| (SystemId::new(*name), planets))
            })
            .expect("the corpus has a two-planet system");
        let mut state = start_game(content, &[invader(), holder()], POK, None).unwrap();
        // The driver activates the system before opening the invasion window; card effects
        // read the state's active system, as in play.
        state.active_system = Some(system.clone());
        (state, system, planets[0].clone(), planets[1].clone())
    }
    /// The parley test arena: the invader holds one infantry in space, the holder two on the
    /// planet he controls. The cardless arm ends in a ground round (one die for the
    /// invader's infantry, two for the defender's, all hitting on 8); the parley arm rolls
    /// nothing and is asked to commit twice — once before the landing, then again after
    /// Parley puts the unit back in space, where it can be committed again.
    fn parley_run(with_card: bool) -> (Vec<Vec<String>>, GameState) {
        let a = invader();
        let b = holder();
        let (mut state, system, planet) = arena();
        state
            .system_mut(&system)
            .set_control(planet.clone(), b.clone());
        on_planet(&mut state, &system, &planet, "infantry", &b, 2);
        in_space(&mut state, &system, "infantry", &a, 1);
        if with_card {
            state.player_mut(&b).unwrap().action_cards =
                vec![ti4_model::id::ActionCardId::new("parley")];
        }
        let (mut dice, mut rng) = if with_card {
            (Dice::new(), GameRng::new(5))
        } else {
            (Dice::from_faces([8u32, 8, 8]), GameRng::new(5))
        };
        let script: Vec<String> = if with_card {
            vec![
                format!("commit|0|{planet}"),
                "reaction:generic:UNITS_COMMITTED:after".to_owned(),
                "done_committing".to_owned(),
            ]
        } else {
            vec![format!("commit|0|{planet}"), "fight".to_owned()]
        };
        let asks = drive_invasion(
            &mut state,
            &system,
            Stage::Committing,
            InvasionReport::default(),
            script,
            &mut dice,
            &mut rng,
        );
        (asks, state)
    }

    /// Package W3: Parley played by the holder returns a Naaz mech the invader landed to the space area in its SHIP form
    /// (`naaz_mech_space`: a ground form is never in a space area). The card is played inside the window of the landing
    /// (`invasion::commit_one`), where the core ability table plays it (`cards::parley`, `fx_strategy::flip_form`) and the result
    /// is compared with the string's `flip_form` (R-bail-11 closed).
    #[cfg(feature = "typed-core")]
    #[test]
    fn parley_flips_a_landed_naaz_mech_in_the_core_commit_window_in_lockstep() {
        let _serial = crate::dual::tests::serial();
        let (a, b) = (invader(), holder());
        let (mut state, system, planet) = arena();
        for (seat, faction) in state.players.iter_mut().zip(["naaz", "hacan"]) {
            seat.faction = ti4_model::id::FactionId::new(faction);
        }
        state.promissory_notes.clear();
        state.system_mut(&system).set_control(planet.clone(), b.clone());
        on_planet(&mut state, &system, &planet, "infantry", &b, 2);
        in_space(&mut state, &system, "naaz_mech", &a, 1);
        state.player_mut(&b).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("parley")];
        state.rng_seed = 5;
        let mut dice = Dice::new();
        let mut rng = GameRng::new(5);
        let slot = format!("reaction:{}:UNITS_COMMITTED:after", crate::promissory::faction_name(&state, &b));
        let script = vec![format!("commit|0|{planet}"), slot, "done_committing".to_owned()];
        let _ = crate::dual::take_report();
        let mask = crate::dual::AreaMask::STRING.with(crate::dual::Area::Invasion, crate::dual::Backend::Dual);
        let _asks = crate::dual::with_mask(mask, || {
            drive_invasion(&mut state, &system, Stage::Committing, InvasionReport::default(), script, &mut dice, &mut rng)
        });
        let report = crate::dual::report();
        let shown: String = format!("{:?}", report.differences).chars().take(1500).collect();
        assert!(report.differences.is_empty(), "{shown}");
        let w = report.windows.get("invasion::commit_one").cloned().unwrap_or_default();
        assert!(w.compared > 0 && w.bailed == 0, "the landing ran in the core with Parley in its ability table: {:?}", report.windows);
        let space: Vec<String> = state.system_state(&system).units_of(&a).into_iter().map(|u| u.type_id.to_string()).collect();
        assert_eq!(space, ["naaz_mech_space"], "back in space in its ship form");
        let _ = crate::dual::take_report();
    }

    #[test]
    fn parley_returns_the_committed_unit_to_space() {
        // Parley: "Return the committed units to the space area." The invader lands one
        // infantry on the holder's planet, which stands over the holder's two. With Parley
        // the landing unit returns to space before any combat — both defender infantry stand
        // and the invader's infantry survives; without the card both sides roll 8 and die.
        let a = invader();
        let b = holder();
        let run = |with_card: bool| parley_run(with_card);

        // The cardless landing: both 8s kill and the invader's unit dies on the planet. Both
        // arms ran on the same deterministic arena, so its coordinates can be re-derived.
        let (asks, state) = run(false);
        let (_, system, planet) = arena();
        let commit_offer = vec![format!("commit|0|{planet}"), "done_committing".to_owned()];
        let on_planet = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(
            on_planet.len(),
            1,
            "the invader's unit died; one defender infantry stands"
        );
        assert_eq!(on_planet[0].owner, b);
        assert_eq!(
            asks,
            vec![commit_offer.clone(), vec!["fight".to_owned()]],
            "commit, then the fight"
        );

        // With Parley: the landing unit is back in space, nothing is fought, both defender
        // infantry stand, the planet is still the holder's, and the invader took nothing.
        let (asks, state) = run(true);
        let on_planet = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(
            on_planet.len(),
            2,
            "both defender infantry stand; the committed unit never landed"
        );
        assert!(on_planet.iter().all(|unit| unit.owner == b));
        let in_space = state
            .system_state(&system)
            .units_of(&a)
            .into_iter()
            .filter(|unit| unit.type_id.as_str() == "infantry")
            .count();
        assert_eq!(in_space, 1, "the committed unit is back in the space area");
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b),
            "the invader took nothing"
        );
        assert!(
            state.player(&b).unwrap().action_cards.is_empty(),
            "the card was spent"
        );
        assert_eq!(
            state.last_committed_unit, None,
            "Parley cleared the marker it acted on"
        );
        let reaction_offer = vec![
            "reaction:generic:UNITS_COMMITTED:after".to_owned(),
            "decline".to_owned(),
        ];
        assert_eq!(
            asks,
            vec![commit_offer.clone(), reaction_offer, commit_offer],
            "commit, the reaction offer, then the commit offer again and the pass"
        );
    }

    /// The ghost squad test arena: the holder controls two planets with two of his infantry
    /// on the first; the invader holds one infantry in space. The cardless arm ends in a
    /// ground round (one die for the invader, two for the defender, all hitting on 8); the
    /// card arm commits, answers the reaction, makes the move, then declines the reverse.
    fn ghost_run(with_card: bool) -> (Vec<Vec<String>>, GameState, SystemId, PlanetId, PlanetId) {
        let a = invader();
        let b = holder();
        let (mut state, system, planet, other) = dual_arena();
        state
            .system_mut(&system)
            .set_control(planet.clone(), b.clone());
        state
            .system_mut(&system)
            .set_control(other.clone(), b.clone());
        on_planet(&mut state, &system, &planet, "infantry", &b, 2);
        in_space(&mut state, &system, "infantry", &a, 1);
        if with_card {
            state.player_mut(&b).unwrap().action_cards =
                vec![ti4_model::id::ActionCardId::new("ghost_squad")];
        }
        let (mut dice, mut rng) = if with_card {
            (Dice::new(), GameRng::new(5))
        } else {
            (Dice::from_faces([8u32, 8, 8]), GameRng::new(5))
        };
        let commit = format!("commit|0|{planet}");
        let script: Vec<String> = if with_card {
            vec![
                commit,
                "reaction:generic:UNITS_COMMITTED:after".to_owned(),
                format!("move|{planet}|{other}|infantry"),
                "decline".to_owned(),
            ]
        } else {
            vec![commit, "fight".to_owned()]
        };
        let asks = drive_invasion(
            &mut state,
            &system,
            Stage::Committing,
            InvasionReport::default(),
            script,
            &mut dice,
            &mut rng,
        );
        (asks, state, system, planet, other)
    }

    #[test]
    // The offer used to come twice: after moving the infantry from one planet to the other, the
    // reverse move was offered again, and this test asserted that sequence with a decider that
    // declined it. A decider that does NOT decline -- any greedy policy, since argmax is
    // deterministic -- oscillates instead, and the repeats sit inside one `Game::step` where the
    // run-level step limit cannot see them. A planet that has received forces is no longer offered
    // as a source, so there is one move and the effect ends.
    fn ghost_squad_relocates_the_holders_forces_before_the_fight() {
        // Ghost Squad: "Move any number of your ground forces from any planet you control
        // in the active system to any other planet you control in the active system." The
        // invader lands on the planet holding the holder's two infantry. With the card the
        // holder moves both infantry to his other planet before any combat: the invader's
        // lone infantry takes the first planet alone. Without it, both sides roll 8, the
        // invader's unit dies, and the holder keeps the first planet.
        let a = invader();
        let b = holder();
        let run = |with_card: bool| ghost_run(with_card);

        // The cardless landing: both 8s kill, the invader's unit dies on the first planet,
        // one defender infantry stands, and the holder keeps both planets.
        let (asks, state, system, planet, other) = run(false);
        let on_planet = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(
            on_planet.len(),
            1,
            "the invader's unit died; one defender infantry stands"
        );
        assert_eq!(on_planet[0].owner, b);
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b),
            "the invader held nothing of its own on the planet, so it took nothing"
        );
        assert!(
            state.system_state(&system).on_planet(&other).is_empty(),
            "nothing was moved"
        );
        let commit_offer = vec![
            format!("commit|0|{planet}"),
            format!("commit|0|{other}"),
            "done_committing".to_owned(),
        ];
        assert_eq!(
            asks,
            vec![commit_offer.clone(), vec!["fight".to_owned()]],
            "commit, then the fight; got {asks:?}"
        );

        // With Ghost Squad: both infantry move to the second planet before any combat; the
        // invader's infantry stands on the first planet alone and takes it.
        let (asks, state, _, _, _) = run(true);
        let on_planet = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(
            on_planet.len(),
            1,
            "only the invader's infantry stands on the first planet"
        );
        assert_eq!(on_planet[0].owner, a);
        let moved: Vec<Unit> = state.system_state(&system).on_planet(&other).to_vec();
        assert_eq!(moved.len(), 2, "both of the holder's infantry relocated");
        assert!(moved.iter().all(|unit| unit.owner == b));
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&a),
            "the invader holds the first planet: no defender ground was there to fight"
        );
        assert_eq!(
            state.system_state(&system).planet_control.get(&other),
            Some(&b),
            "the holder still controls the planet the forces moved to"
        );
        assert!(
            state.player(&b).unwrap().action_cards.is_empty(),
            "the card was spent"
        );
        let move_offer = |from: &PlanetId, to: &PlanetId| {
            vec![format!("move|{from}|{to}|infantry"), "decline".to_owned()]
        };
        let reaction_offer = vec![
            "reaction:generic:UNITS_COMMITTED:after".to_owned(),
            "decline".to_owned(),
        ];
        assert_eq!(
            asks,
            vec![commit_offer, reaction_offer, move_offer(&planet, &other)],
            "commit, the reaction offer, then the one move; got {asks:?}"
        );
    }
    #[test]
    fn scramble_frequency_rerolls_all_of_the_rollers_dice() {
        // Scramble Frequency: "After another player makes a BOMBARDMENT, SPACE CANNON, or
        // ANTI-FIGHTER BARRAGE roll: That player rerolls all of their dice." The invader's
        // dreadnought (hits on 5) rolls an 8 (one kill); the holder scrambles it, and the
        // re-draw from the seeded stream (seed 1's first DICE draw is a 4) kills nothing.
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        let c = PlayerId::new("c");
        let (system, planet) = crate::fixtures::a_placed_planet();

        let run = |play: bool| -> (Vec<Vec<String>>, GameState, Dice) {
            let mut state = crate::fixtures::game(&["a", "b", "c"]);
            state
                .system_mut(&system)
                .set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            crate::fixtures::put(&mut state, &system, "dreadnought", &a, 1);
            state.player_mut(&c).unwrap().action_cards =
                vec![ti4_model::id::ActionCardId::new("scramble")];
            let content = ContentStore::embedded();
            let mut dice = Dice::from_faces([8u32, 2]);
            let mut rng = GameRng::new(1);
            let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let script: Vec<String> = if play {
                vec!["reaction:generic:UNIT_ABILITY_ROLLED:after".to_owned()]
            } else {
                vec!["decline".to_owned()]
            };
            let decider = RecordingScripted {
                inner: crate::choice::Scripted::new(script),
                seen: seen.clone(),
            };
            let mut table = Table::with_default(Box::new(decider));
            let mut resolver = armed_resolver(&state, vec![a.clone(), b.clone(), c.clone()]);
            let mut event_sequence = crate::event::EventSequence::new();
            let mut window =
                InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &a, &system);
            let mut ctx = Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle {
                    resolver: &mut resolver,
                    sequence: &mut event_sequence,
                    galaxy: None,
                }),
            };
            while !window.is_done() {
                window
                    .drive(&mut state, &mut ctx)
                    .expect("the script only picks what is offered");
                while window.take_scoring_occurrence().is_some() {}
                window.settle(&mut state, &mut ctx);
            }
            let asks = seen.borrow().clone();
            let _ = window;
            (asks, state, dice)
        };

        // Declined: the 8 lands its one kill.
        let (asks, state, dice) = run(false);
        assert_eq!(
            asks,
            vec![vec![
                "reaction:generic:UNIT_ABILITY_ROLLED:after".to_owned(),
                "decline".to_owned()
            ]],
            "the roller is not offered the window, only the other players; got {asks:?}"
        );
        let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(left.len(), 1, "one of the two infantry was killed by the 8");
        assert_eq!(dice.count(), 1, "a declined scramble consumes no dice");
        assert!(dice.rolled("scramble frequency").is_empty());

        // Played: every die the invader made is rerolled, and the 2 kills nothing.
        let (asks, state, dice) = run(true);
        assert_eq!(
            asks,
            vec![vec![
                "reaction:generic:UNIT_ABILITY_ROLLED:after".to_owned(),
                "decline".to_owned()
            ]],
            "one reaction offer; the lone card auto-plays and asks no die question, all dice reroll; got {asks:?}"
        );
        assert!(state.reroll_staging.is_empty());
        let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(left.len(), 2, "the scramble saved both infantry");
        assert_eq!(
            dice.rolled("scramble frequency").len(),
            1,
            "the forced reroll ran through the game's roller"
        );
        assert!(
            state.player(&c).unwrap().action_cards.is_empty(),
            "the card was spent even though it saved the day"
        );
    }

    /// Scramble Frequency played from the reaction slot of a real bombardment (the dice come from the game's own generator), with
    /// `Area::ActionCards` on `Dual`: the core's window runs on the position in flight and is compared with the string, for the
    /// holder who plays it and for the one who declines (nothing is drawn then).
    #[cfg(feature = "typed-core")]
    #[test]
    fn scramble_frequency_played_from_the_slot_of_a_real_bombardment_agrees_with_the_core_window() {
        let _serial = crate::dual::tests::serial();
        let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
        for play in [true, false] {
            let mut state = start_game(ContentStore::embedded(), &[a.clone(), b.clone(), c.clone()], POK, None).unwrap();
            for (seat, faction) in state.players.iter_mut().zip(["arborec", "hacan", "sol"]) {
                seat.faction = ti4_model::id::FactionId::new(faction);
            }
            state.promissory_notes.clear();
            let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
                .iter()
                .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
                .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
                .expect("the corpus has a placed planet");
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            in_space(&mut state, &system, "dreadnought", &a, 2);
            state.player_mut(&c).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("scramble")];
            state.rng_seed = 1;
            let content = ContentStore::embedded();
            let mut dice = Dice::new();
            let mut rng = GameRng::new(1);
            let slot = format!("reaction:{}:UNIT_ABILITY_ROLLED:after", crate::promissory::faction_name(&state, &c));
            let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let decider = RecordingScripted { inner: crate::choice::Scripted::new(vec![if play { slot } else { "decline".to_owned() }]), seen: seen.clone() };
            let mut table = Table::with_default(Box::new(decider));
            let mut resolver = armed_resolver(&state, vec![a.clone(), b.clone(), c.clone()]);
            let mut event_sequence = crate::event::EventSequence::new();
            let _ = crate::dual::take_report();
            let mask = crate::dual::AreaMask::STRING.with(crate::dual::Area::ActionCards, crate::dual::Backend::Dual);
            crate::dual::with_mask(mask, || {
                let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &a, &system);
                let mut ctx = Resolving {
                    content,
                    sources: POK,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut event_sequence, galaxy: None }),
                };
                while !window.is_done() {
                    window.drive(&mut state, &mut ctx).unwrap_or_else(|e| panic!("the script only picks what is offered: {e:?}; {:?}", seen.borrow()));
                    while window.take_scoring_occurrence().is_some() {}
                    window.settle(&mut state, &mut ctx);
                }
            });
            let report = crate::dual::report();
            let shown: String = format!("{:?}", report.differences).chars().take(1200).collect();
            assert!(report.differences.is_empty(), "play {play}: {shown}");
            let w = report.windows.get("action_cards::announce").cloned().unwrap_or_default();
            assert_eq!((w.compared, w.bailed), (u64::from(play), 0), "play {play}: {:?}", report.windows);
            assert_eq!(dice.rolled("scramble frequency").len(), if play { 2 } else { 0 }, "play {play}: both dreadnoughts' dice were drawn again");
            let _ = crate::dual::take_report();
        }
    }

    #[test]
    fn the_jolnar_commander_may_reroll_any_ability_die() {
        // Aglnlan Oln's commander: "After you roll dice for a unit ability: You may reroll
        // any of those dice." The invader's dreadnought (hits on 5) bombs with a 3 (no hits);
        // rerolled, the re-draw from the seeded stream (seed 4's first DICE draw is an 8)
        // takes one of the defender's two infantry. Declining rerolls nothing.
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        let (system, planet) = crate::fixtures::a_placed_planet();

        let run = |reroll: bool| -> (Vec<Vec<String>>, GameState, Dice) {
            let mut state = crate::fixtures::game(&["a", "b"]);
            state
                .system_mut(&system)
                .set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            crate::fixtures::put(&mut state, &system, "dreadnought", &a, 1);
            state.player_mut(&a).unwrap().leaders.insert(
                ti4_model::id::LeaderId::new("jolnarcommander"),
                ti4_model::state::LeaderStatus::Unlocked,
            );
            let content = ContentStore::embedded();
            let mut dice = Dice::from_faces([3u32]);
            let mut rng = GameRng::new(4);
            let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let script: Vec<String> = if reroll {
                vec!["reroll|0:0".to_owned()]
            } else {
                vec!["decline".to_owned()]
            };
            let decider = RecordingScripted {
                inner: crate::choice::Scripted::new(script),
                seen: seen.clone(),
            };
            let mut table = Table::with_default(Box::new(decider));
            let mut resolver = armed_resolver(&state, vec![a.clone(), b.clone()]);
            let mut event_sequence = crate::event::EventSequence::new();
            let mut window =
                InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &a, &system);
            let mut ctx = Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle {
                    resolver: &mut resolver,
                    sequence: &mut event_sequence,
                    galaxy: None,
                }),
            };
            while !window.is_done() {
                window
                    .drive(&mut state, &mut ctx)
                    .expect("the script only picks what is offered");
                while window.take_scoring_occurrence().is_some() {}
                window.settle(&mut state, &mut ctx);
            }
            let asks = seen.borrow().clone();
            let _ = window;
            (asks, state, dice)
        };

        // Rerolled: the 3 is re-drawn to an 8 (seed 4) and takes one infantry.
        let (asks, state, dice) = run(true);
        assert_eq!(
            asks,
            vec![vec!["reroll|0:0".to_owned(), "decline".to_owned()]],
            "one optional die question, asked of the roller; got {asks:?}"
        );
        let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(left.len(), 1, "the reroll landed its one hit");
        assert_eq!(dice.rolled("jolnar commander").len(), 1);
        assert!(state.reroll_staging.is_empty());

        // Declined: the 3 rolls no hits and nothing is rerolled.
        let (asks, state, dice) = run(false);
        assert_eq!(
            asks,
            vec![vec!["reroll|0:0".to_owned(), "decline".to_owned()]],
            "the question is offered either way; got {asks:?}"
        );
        let left: Vec<Unit> = state.system_state(&system).on_planet(&planet).to_vec();
        assert_eq!(left.len(), 2, "a declined reroll changes nothing");
        assert!(dice.rolled("jolnar commander").is_empty());
        assert_eq!(dice.count(), 1);
    }

    #[test]
    fn a_nekro_flagship_with_the_letnev_z_token_strips_the_defenders_planetary_shield() {
        let content = ContentStore::embedded();
        let run = |lent: &[&str]| {
            let (_, system, planet) = arena_off_mecatol();
            let mut state = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "sol")], lent);
            on_planet(&mut state, &system, &planet, "pds", &holder(), 1);
            on_planet(&mut state, &system, &planet, "infantry", &holder(), 1);
            in_space(&mut state, &system, "dreadnought", &invader(), 1);
            in_space(&mut state, &system, "nekro_flagship", &invader(), 1);
            bombardable(&state, content, POK, &system, &planet, &invader())
        };
        assert!(!run(&[]), "off by default");
        assert!(run(&["letnev"]));
    }
}

#[cfg(test)]
mod every_committed_force_lands {
    use super::*;
    use crate::setup::start_game;
    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;

    fn invader() -> PlayerId {
        PlayerId::new("a")
    }
    fn holder() -> PlayerId {
        PlayerId::new("b")
    }
    fn in_space(state: &mut GameState, system: &SystemId, kind: &str, owner: &PlayerId, n: usize) {
        for _ in 0..n {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new(kind), owner.clone()));
        }
    }

    /// Four ground forces committed to two planets must produce four ground forces on those
    /// planets. Observed in review of seed 501: Sol committed two units to each of Kraag and Siig,
    /// and the board ended with two on Kraag and ONE on Siig. A committed unit vanished, and
    /// control of both planets was taken anyway.
    ///
    /// The commit path itself looks clean -- `landable` reads only the space area and
    /// `SystemState::remove` touches only the space area -- so this pins the outcome rather than
    /// the mechanism.
    #[test]
    fn four_forces_committed_to_two_planets_all_arrive() {
        let mut state =
            start_game(ContentStore::embedded(), &[invader(), holder()], POK, None).unwrap();
        let system = SystemId::new("09");
        in_space(&mut state, &system, "infantry", &invader(), 4);

        let script = vec![
            "commit|0|maaluuk".to_owned(),
            "commit|0|druaa".to_owned(),
            "commit|0|maaluuk".to_owned(),
            "commit|0|druaa".to_owned(),
            "done_committing".to_owned(),
        ];
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(script)));

        commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &invader(),
            &system,
        )
        .unwrap();

        let board = state.system_state(&system);
        let on = |planet: &str| {
            board
                .planet_units
                .get(ti4_model::id::text(&planet))
                .map_or(0, Vec::len)
        };
        let left_in_space = board
            .units
            .iter()
            .filter(|unit| unit.owner == invader())
            .count();

        assert_eq!(
            (on("maaluuk"), on("druaa"), left_in_space),
            (2, 2, 0),
            "four committed, four must land: maaluuk {}, druaa {}, still in space {}",
            on("maaluuk"),
            on("druaa"),
            left_in_space
        );
    }
}

/// BF-00c-ground: the typed events and faction hooks the base-faction modules hang on.
///
/// The hook tests install a plain-`fn` hook set through `hooks_ground::test_support`, since
/// `MODULES` is a constant; every one of them also pins that the same scenario without the hook is
/// the ordinary rules, which is the neutrality the empty modules rely on.
#[cfg(test)]
mod ground_routes {
    use super::*;
    use crate::factions::hooks_ground::{
        CommitCandidate, CommitOrigin, GroundHooks, with_test_hooks,
    };
    use crate::setup::start_game;
    use crate::timing::{Ability, Relation, Resolver, TimingContext};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;

    type Recorded = Vec<(String, BTreeMap<String, serde_json::Value>)>;
    type Seen = Arc<Mutex<Recorded>>;

    const EVENTS: [&str; 6] = [
        "GROUND_FORCE_SUSTAINED",
        "GROUND_FORCE_DESTROYED",
        "GROUND_COMBAT_STARTED",
        "GROUND_COMBAT_ENDED",
        "GROUND_COMMITMENT_FINISHED",
        "UNITS_COMMITTED",
    ];

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    /// A resolver that records every event in [`EVENTS`] as it is emitted.
    fn listening() -> (Resolver, Seen) {
        let seen: Seen = Arc::default();
        let mut resolver = Resolver::new(vec![a(), b()], Some(a()), Table::default());
        resolver.register(EVENTS.iter().map(|event| {
            let seen = Arc::clone(&seen);
            Ability::new(
                format!("test:{event}"),
                a(),
                *event,
                Relation::After,
                Arc::new(move |event, _| {
                    seen.lock()
                        .unwrap()
                        .push((event.event_type.clone(), event.payload.clone()));
                    Ok(())
                }),
            )
        }));
        (resolver, seen)
    }

    fn text(payload: &BTreeMap<String, serde_json::Value>, key: &str) -> String {
        payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("payload has no text {key}: {payload:?}"))
            .to_owned()
    }

    fn on_planet(
        state: &mut GameState,
        system: &SystemId,
        planet: &PlanetId,
        kind: &str,
        who: &PlayerId,
    ) {
        state
            .system_mut(system)
            .planet_units
            .entry(planet.clone())
            .or_default()
            .push(Unit::new(UnitTypeId::new(kind), who.clone()));
    }

    fn arena() -> (GameState, SystemId, PlanetId) {
        let state = start_game(ContentStore::embedded(), &[a(), b()], POK, None).unwrap();
        let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
            .iter()
            .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
            .expect("the corpus has a placed planet");
        (state, system, planet)
    }

    /// Records the option ids of every question, then answers from a script.
    struct Recording(crate::choice::Scripted, Arc<Mutex<Vec<Vec<String>>>>);

    impl crate::choice::Decider for Recording {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            self.1
                .lock()
                .unwrap()
                .push(choice.options.iter().map(|o| o.id.clone()).collect());
            self.0.choose(choice)
        }
    }

    /// Drive `window` to its end with the scripted answers; returns the asks' option ids and every
    /// recorded event.
    fn drive(
        state: &mut GameState,
        window: &mut InvasionWindow,
        script: &[&str],
        faces: &[u32],
    ) -> (Vec<Vec<String>>, Recorded) {
        let (mut resolver, seen) = listening();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let mut table = Table::with_default(Box::new(Recording(
            crate::choice::Scripted::new(script.iter().copied()),
            Arc::clone(&asked),
        )));
        let mut dice = Dice::from_faces(faces.iter().copied());
        let mut rng = GameRng::new(7);
        let mut sequence = crate::event::EventSequence::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        while !window.is_done() {
            window.drive(state, &mut ctx).expect("only offered answers");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(state, &mut ctx);
        }
        let asks = asked.lock().unwrap().clone();
        let events = seen.lock().unwrap().clone();
        (asks, events)
    }

    fn fight_window(state: &mut GameState, system: &SystemId, planet: &PlanetId) -> InvasionWindow {
        let mut window = InvasionWindow::at_commit_step(state, &a(), system);
        window.stage = Stage::Advancing {
            planets: vec![planet.clone()],
            index: 0,
        };
        window.report.committed = vec![planet.clone()];
        window
    }

    /// A mech defends two invader infantry (hit on 8; the mech hits on 6).
    fn mech_defends() -> (GameState, SystemId, PlanetId) {
        let (mut state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        on_planet(&mut state, &system, &planet, "infantry", &a());
        on_planet(&mut state, &system, &planet, "infantry", &a());
        on_planet(&mut state, &system, &planet, "mech", &b());
        (state, system, planet)
    }

    /// Mahact's Edict through a real ground combat (not a hand-emitted event): the Mahact seat `a`
    /// wins as invader or as defender, and the fleet pool owners afterwards are returned.
    fn edict_after_real_combat(mahact_invades: bool) -> Vec<PlayerId> {
        let mut state = crate::fixtures::seated_game(&[("a", "mahact"), ("b", "xxcha")], POK);
        let (_state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        on_planet(&mut state, &system, &planet, "infantry", &a());
        on_planet(&mut state, &system, &planet, "infantry", &a());
        on_planet(&mut state, &system, &planet, "infantry", &b());
        let invader = if mahact_invades { a() } else { b() };
        let mut window = InvasionWindow::at_commit_step(&mut state, &invader, &system);
        window.stage = Stage::Advancing {
            planets: vec![planet.clone()],
            index: 0,
        };
        window.report.committed = vec![planet.clone()];
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(
            std::iter::repeat_n("fight", 8),
        )));
        // Every die hits: the Mahact's two infantry beat the one that opposes them.
        let mut dice = Dice::from_faces(std::iter::repeat_n(10, 40));
        let mut rng = GameRng::new(7);
        let mut sequence = crate::event::EventSequence::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        let mut rounds = 0;
        while !window.is_done() && rounds < 50 {
            rounds += 1;
            window
                .drive(&mut state, &mut ctx)
                .expect("only offered answers");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
        }
        crate::factions::mahact::fleet_pool_owners(&state, &a())
    }

    #[test]
    fn edict_takes_a_token_when_the_mahact_wins_a_real_ground_combat() {
        assert_eq!(edict_after_real_combat(true), vec![b()], "as the invader");
        assert_eq!(edict_after_real_combat(false), vec![b()], "as the defender");
    }

    #[test]
    fn the_nekro_mech_rolls_two_better_once_on_the_core_backend() {
        // `invasion=core` answers the ground value from the typed rows: the Mordred row is native and the string
        // bridge of the combat kernel must not add it again (bench-v1 seed 25 rolled on 2 instead of 4).
        use crate::dual::{Area, AreaMask, Backend, with_mask};
        let _guard = crate::dual::tests::serial();
        let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "sol")], POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
            .iter()
            .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
            .expect("a placed planet");
        // Sol owns Mageon Implants, which carries Nekro's X token.
        crate::technology_effects_tests::give(&mut state, "b", "mi");
        state.player_mut(&a()).unwrap().assimilated_technologies.insert("vax".to_owned(), ti4_model::id::TechnologyId::new("mi"));
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "nekro_mech", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b(), 1);
        let value = |backend: Backend| {
            with_mask(AreaMask::STRING.with(Area::Invasion, backend), || ground_combat_value(&state, ContentStore::embedded(), POK, &a(), &system, &planet, "nekro_mech"))
        };
        let printed = value(Backend::String);
        assert_eq!(printed, Some(4), "6 with Mordred's +2");
        assert_eq!(value(Backend::Core), printed);
    }

    #[test]
    fn a_landing_runs_as_a_core_window_in_lockstep() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
                .iter()
                .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
                .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
                .expect("a placed planet");
            crate::fixtures::put(&mut state, &system, "infantry", &a(), 2);
            crate::fixtures::put(&mut state, &system, "mech", &a(), 1);
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let mut dice = Dice::new();
            let mut rng = GameRng::new(4);
            let report = resolve(&mut state, ContentStore::embedded(), POK, &mut table, &mut dice, &mut rng, &system, &a()).unwrap();
            assert_eq!(report.committed, vec![planet.clone()]);
        });
        let r = report();
        let w = r.windows.get("invasion::commit_one").copied().unwrap_or_default();
        assert!(w.compared >= 2, "both landings are compared: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_next_combat_question_is_put_by_the_core_window_in_lockstep() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
                .iter()
                .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
                .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
                .expect("a placed planet");
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &system);
            window.stage = Stage::ChoosingNextCombat { planets: vec![planet.clone()], index: 0, planet: planet.clone(), remaining: vec![b()] };
            let choice = window.pending_choice(&state, ContentStore::embedded(), POK).expect("asked");
            assert_eq!(choice.options.len(), 2);
        });
        let r = report();
        let w = r.windows.get("invasion::next_combat_ask").copied().unwrap_or_default();
        assert_eq!(w.compared, 1, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_bombardment_target_question_is_put_by_the_core_window_in_lockstep() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev"), ("c", "hacan")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            let (system, planet) = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK)
                .iter()
                .find(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
                .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
                .expect("a placed planet");
            on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"));
            on_planet(&mut state, &system, &planet, "mech", &PlayerId::new("c"));
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &system);
            window.stage = Stage::ChoosingBombardment(BombardCursor {
                planet_index: 0,
                planet: planet.clone(),
                groups: vec![2, 1],
                next_group: 0,
                taken: 0,
                held: 2,
                victims: [PlayerId::new("b"), PlayerId::new("c")].into_iter().collect(),
                occurrence: state.begin_feat_occurrence(),
            });
            let choice = window.pending_choice(&state, ContentStore::embedded(), POK).expect("asked");
            assert_eq!(choice.options.len(), 2);
        });
        let r = report();
        let w = r.windows.get("invasion::bombard_target_ask").copied().unwrap_or_default();
        assert_eq!(w.compared, 1, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_bombardment_steps_and_answers_run_as_a_core_window_in_lockstep() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            // The coexisting bombardment of `a_coexisting_planets_bombardment_is_chosen_per_unit_...`, then a plain
            // one-owner planet: pauses, answers, hits, events and the end of the plan.
            let (_, system, planet) = arena();
            let (a, b, c) = (PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c"));
            let content = ContentStore::embedded();
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev"), ("c", "hacan")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            on_planet(&mut state, &system, &planet, "infantry", &b);
            on_planet(&mut state, &system, &planet, "infantry", &c);
            on_planet(&mut state, &system, &planet, "infantry", &c);
            for kind in ["dreadnought", "sardakk_dreadnought", "dreadnought"] {
                state.system_mut(&system).units.push(Unit::new(UnitTypeId::new(kind), a.clone()));
            }
            let mut dice = Dice::from_faces([8, 6, 6, 8]);
            let mut rng = GameRng::new(7);
            let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(["c", "b", "c"])));
            let mut window = InvasionWindow::new(&mut state, content, POK, &mut dice, &mut rng, &a, &system);
            let mut ctx = Resolving { content, sources: POK, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
            while !window.is_done() {
                window.drive(&mut state, &mut ctx).expect("the script only picks what is offered");
                while window.take_scoring_occurrence().is_some() {}
                window.settle(&mut state, &mut ctx);
            }
            assert_eq!(window.into_report().bombardment_kills, 3);
        });
        let r = report();
        for label in ["invasion::bombard_step", "invasion::bombard_answer"] {
            let w = r.windows.get(label).copied().unwrap_or_default();
            assert!(w.compared >= 1, "{label}: {:?}", r.windows);
        }
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_fight_round_question_is_put_by_the_core_window_in_lockstep() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let (mut state, system, planet) = mech_defends();
            let mut window = fight_window(&mut state, &system, &planet);
            let (asks, _) = drive(&mut state, &mut window, &["fight", "fight"], &[10, 1, 1, 10, 1, 1]);
            assert_eq!(asks.len(), 2, "two rounds were fought: {asks:?}");
        });
        let r = report();
        let w = r.windows.get("invasion::fight_round_ask").copied().unwrap_or_default();
        assert!(w.compared >= 2, "every question of the fight is compared: {:?}", r.windows);
        assert_eq!(w.bailed, 0);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn a_ground_combat_announces_its_start_sustains_destruction_and_end() {
        let (mut state, system, planet) = mech_defends();
        let mut window = fight_window(&mut state, &system, &planet);
        // Round 1: invader [10, 1] (one hit), mech [1]. Round 2: the same.
        let (asks, events) = drive(
            &mut state,
            &mut window,
            &["fight", "fight"],
            &[10, 1, 1, 10, 1, 1],
        );
        assert_eq!(asks.len(), 2, "two rounds were fought: {asks:?}");
        let names: Vec<&str> = events.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "GROUND_COMBAT_STARTED",
                "GROUND_FORCE_SUSTAINED",
                "GROUND_FORCE_DESTROYED",
                "GROUND_COMBAT_ENDED"
            ]
        );

        let started = &events[0].1;
        assert_eq!(text(started, "system"), system.to_string());
        assert_eq!(text(started, "planet"), planet.to_string());
        assert_eq!(text(started, "attacker"), "a");
        assert_eq!(text(started, "defender"), "b");

        let sustained = &events[1].1;
        assert_eq!(text(sustained, "player"), "b");
        assert_eq!(text(sustained, "unit"), "mech");
        assert_eq!(text(sustained, "cause"), "ground_combat");
        assert_eq!(text(sustained, "planet"), planet.to_string());
        assert_eq!(text(sustained, "system"), system.to_string());

        let destroyed = &events[2].1;
        assert_eq!(text(destroyed, "player"), "b");
        assert_eq!(text(destroyed, "unit"), "mech");
        assert_eq!(destroyed["damaged"], serde_json::Value::Bool(true));
        assert_eq!(text(destroyed, "cause"), "ground_combat");

        let ended = &events[3].1;
        assert_eq!(text(ended, "winner"), "a");
        assert_eq!(text(ended, "attacker"), "a");
        assert_eq!(text(ended, "defender"), "b");
        assert_eq!(ended["control_changed"], serde_json::Value::Bool(true));
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&a()),
            "and 49.5 then does what the event said"
        );
    }

    /// Self-Assembly Routines' second ability through a real ground combat: seat `b` defends with
    /// `mechs` mechs against `attackers` infantry, every die hitting, and its trade goods after
    /// the fight are returned.
    fn goods_after_a_mech_fight(
        rules_version: u32,
        owns_the_card: bool,
        mechs: usize,
        attackers: usize,
    ) -> i32 {
        let (mut state, system, planet) = arena();
        state.rules_version = rules_version;
        state.system_mut(&system).set_control(planet.clone(), b());
        for _ in 0..attackers {
            on_planet(&mut state, &system, &planet, "infantry", &a());
        }
        for _ in 0..mechs {
            on_planet(&mut state, &system, &planet, "mech", &b());
        }
        if owns_the_card {
            state
                .player_mut(&b())
                .unwrap()
                .technologies
                .insert(ti4_model::id::TechnologyId::new("sar"));
        }
        let before = state.player(&b()).unwrap().trade_goods;
        let mut window = fight_window(&mut state, &system, &planet);
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(
            std::iter::repeat_n("fight", 8),
        )));
        let mut dice = Dice::from_faces(std::iter::repeat_n(10, 60));
        let mut rng = GameRng::new(7);
        let mut sequence = crate::event::EventSequence::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        let mut rounds = 0;
        while !window.is_done() && rounds < 50 {
            rounds += 1;
            window
                .drive(&mut state, &mut ctx)
                .expect("only offered answers");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
        }
        assert!(
            !state
                .system_state(&system)
                .planet_units
                .get(&planet)
                .is_some_and(|units| units.iter().any(|unit| unit.owner == b())),
            "every defender fell"
        );
        state.player(&b()).unwrap().trade_goods - before
    }

    #[test]
    fn self_assembly_routines_pays_a_trade_good_for_a_mech_lost_in_ground_combat() {
        let now = ti4_model::state::RULES_SELF_ASSEMBLY;
        assert_eq!(goods_after_a_mech_fight(now, true, 1, 2), 1);
        assert_eq!(
            goods_after_a_mech_fight(now, true, 2, 4),
            2,
            "one trade good per destroyed mech"
        );
        assert_eq!(
            goods_after_a_mech_fight(now, false, 1, 2),
            0,
            "no card, no goods"
        );
        assert_eq!(
            goods_after_a_mech_fight(now - 1, true, 1, 2),
            0,
            "a game from before the gate is unchanged"
        );
    }

    /// The card pays for mechs only: infantry lost beside the card's owner earn nothing, and a
    /// mech that only sustains is not destroyed.
    #[test]
    fn self_assembly_routines_ignores_infantry_and_a_sustained_mech() {
        let (mut state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        on_planet(&mut state, &system, &planet, "infantry", &b());
        state
            .player_mut(&b())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("sar"));
        crate::factions::hooks_ground::stage_ground_force_destroyed(
            &mut state,
            &system,
            &planet,
            &Unit::new(UnitTypeId::new("infantry"), b()),
            "ground_combat",
        );
        let before = state.player(&b()).unwrap().trade_goods;
        flush_staged_with_armed_resolver(&mut state);
        assert_eq!(state.player(&b()).unwrap().trade_goods, before);
    }

    /// An action card (or agenda, or ability) stages its kills; the flush announces them and the
    /// card pays.
    #[test]
    fn self_assembly_routines_pays_for_a_mech_destroyed_by_an_action_card() {
        let (mut state, system, planet) = arena();
        state
            .player_mut(&b())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("sar"));
        for _ in 0..2 {
            crate::factions::hooks_ground::stage_ground_force_destroyed(
                &mut state,
                &system,
                &planet,
                &Unit::new(UnitTypeId::new("mech"), b()),
                "action_card:test",
            );
        }
        let before = state.player(&b()).unwrap().trade_goods;
        flush_staged_with_armed_resolver(&mut state);
        assert_eq!(state.player(&b()).unwrap().trade_goods, before + 2);
    }

    fn flush_staged_with_armed_resolver(state: &mut GameState) {
        let mut resolver = crate::fixtures::armed_resolver(state);
        let mut table = Table::default();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(7);
        let mut sequence = crate::event::EventSequence::new();
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        crate::factions::hooks_ground::announce_staged_events(state, &mut ctx);
    }

    #[test]
    fn a_defender_who_holds_is_the_winner_and_control_does_not_change() {
        let (mut state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        on_planet(&mut state, &system, &planet, "infantry", &a());
        on_planet(&mut state, &system, &planet, "infantry", &b());
        on_planet(&mut state, &system, &planet, "infantry", &b());
        let mut window = fight_window(&mut state, &system, &planet);
        // The invader misses; the defender's pool (hit on 8) kills the lone attacker.
        let (_, events) = drive(&mut state, &mut window, &["fight"], &[1, 10, 1]);
        let ended = events
            .iter()
            .find(|(name, _)| name == "GROUND_COMBAT_ENDED")
            .expect("the combat ended");
        assert_eq!(text(&ended.1, "winner"), "b");
        assert_eq!(ended.1["control_changed"], serde_json::Value::Bool(false));
        assert!(
            events
                .iter()
                .any(|(name, p)| name == "GROUND_FORCE_DESTROYED" && text(p, "player") == "a"),
            "the invader's infantry died: {events:?}"
        );
    }

    fn nobody_may_sustain(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: &crate::factions::CombatUnit<'_>,
    ) -> bool {
        false
    }

    #[test]
    fn a_prohibition_stops_sustain_damage_and_the_unit_dies_to_the_first_hit() {
        let (mut state, system, planet) = mech_defends();
        let mut window = fight_window(&mut state, &system, &planet);
        let hooks = GroundHooks {
            may_sustain: Some(nobody_may_sustain),
            ..GroundHooks::NONE
        };
        let (asks, events) = with_test_hooks(hooks, || {
            drive(&mut state, &mut window, &["fight"], &[10, 1, 1])
        });
        assert_eq!(asks.len(), 1, "one round suffices: {asks:?}");
        let names: Vec<&str> = events.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "GROUND_COMBAT_STARTED",
                "GROUND_FORCE_DESTROYED",
                "GROUND_COMBAT_ENDED"
            ],
            "no sustain event: the mech could not use it"
        );
        assert_eq!(
            events[1].1["damaged"],
            serde_json::Value::Bool(false),
            "destroyed undamaged"
        );
    }

    fn weave(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        player: &PlayerId,
        _: &SystemId,
        _: &PlanetId,
        _: usize,
        opponent_hits: usize,
    ) -> usize {
        // Valkyrie Particle Weave, for the test seat "a" only.
        usize::from(player.as_str() == "a" && opponent_hits >= 1)
    }

    fn weave_both(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: &PlayerId,
        _: &SystemId,
        _: &PlanetId,
        _: usize,
        opponent_hits: usize,
    ) -> usize {
        usize::from(opponent_hits >= 1)
    }

    #[test]
    fn the_pre_hook_hits_of_the_opponent_are_what_a_weave_reads() {
        // Both seats have the ability. Round 1: a rolls 1 hit, b none. b sees a hit and adds one;
        // a sees b rolled zero (not b added hit), so a adds nothing: one casualty each way.
        // Round 2: both roll a hit, both add one; each side last infantry dies.
        let (mut state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        for _ in 0..2 {
            on_planet(&mut state, &system, &planet, "infantry", &a());
            on_planet(&mut state, &system, &planet, "infantry", &b());
        }
        let mut window = fight_window(&mut state, &system, &planet);
        let hooks = GroundHooks {
            ground_rolls_extra_hits: Some(weave_both),
            ..GroundHooks::NONE
        };
        let (asks, events) = with_test_hooks(hooks, || {
            drive(
                &mut state,
                &mut window,
                &["fight", "fight"],
                &[10, 1, 1, 1, 10, 10],
            )
        });
        assert_eq!(asks.len(), 2, "{asks:?}");
        let who: Vec<String> = events
            .iter()
            .filter(|(name, _)| name == "GROUND_FORCE_DESTROYED")
            .map(|(_, p)| text(p, "player"))
            .collect();
        assert_eq!(who, ["b", "a", "b", "a"]);
    }

    #[test]
    fn an_added_hit_is_assigned_by_the_opponent_like_any_other() {
        // One invader infantry misses; the defenders hit once. With a Valkyrie-Particle-Weave-style
        // hook the invader (whose opponent hit) produces 1 additional hit that the defender assigns.
        let run = |hooked: bool| {
            let (mut state, system, planet) = arena();
            state.system_mut(&system).set_control(planet.clone(), b());
            on_planet(&mut state, &system, &planet, "infantry", &a());
            on_planet(&mut state, &system, &planet, "infantry", &b());
            on_planet(&mut state, &system, &planet, "infantry", &b());
            let mut window = fight_window(&mut state, &system, &planet);
            let faces = [1, 10, 1];
            let (asks, events) = if hooked {
                let hooks = GroundHooks {
                    ground_rolls_extra_hits: Some(weave),
                    ..GroundHooks::NONE
                };
                with_test_hooks(hooks, || {
                    drive(&mut state, &mut window, &["fight", "fight"], &faces)
                })
            } else {
                drive(&mut state, &mut window, &["fight", "fight"], &faces)
            };
            let destroyed = events
                .iter()
                .filter(|(name, _)| name == "GROUND_FORCE_DESTROYED")
                .count();
            (asks.len(), destroyed)
        };
        assert_eq!(run(false), (1, 1), "ordinary rules: only the invader dies");
        assert_eq!(
            run(true),
            (1, 2),
            "the invader missed but adds a hit, because b hit"
        );
    }

    /// Invader `a` may also commit its infantry standing on planet `maaluuk` of system 09, once.
    fn commit_from_09(
        state: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        invader: &PlayerId,
        _: &SystemId,
        already: &[CommitOrigin],
    ) -> Vec<CommitCandidate> {
        let (system, planet) = (SystemId::new("09"), PlanetId::new("maaluuk"));
        if invader.as_str() != "a" || already.contains(&(system.clone(), planet.clone())) {
            return Vec::new();
        }
        state
            .system_state(&system)
            .on_planet_of(&planet, invader)
            .into_iter()
            .take(1)
            .map(|unit| CommitCandidate {
                system: system.clone(),
                planet: planet.clone(),
                unit: unit.clone(),
            })
            .collect()
    }

    #[test]
    fn extra_commit_sources_follow_the_ordinary_ones_and_the_engine_moves_the_unit() {
        let content = ContentStore::embedded();
        let mut state = start_game(content, &[a(), b()], POK, None).unwrap();
        let home = SystemId::new("09");
        let record = content
            .get(ti4_model::content_types::ContentType::Systems, "09")
            .unwrap();
        let planets: Vec<PlanetId> = record
            .strings("planets")
            .into_iter()
            .map(PlanetId::new)
            .collect();
        assert!(planets.contains(&ti4_model::id::PlanetId::new("maaluuk")) && planets.len() >= 2);
        let target = planets
            .iter()
            .find(|planet| planet.as_str() != "maaluuk")
            .cloned()
            .unwrap();
        on_planet(
            &mut state,
            &home,
            &PlanetId::new("maaluuk"),
            "infantry",
            &a(),
        );
        crate::fixtures::put(&mut state, &home, "infantry", &a(), 1);

        let ids = |state: &GameState, window: &InvasionWindow| -> Vec<String> {
            window
                .pending_choice(state, content, POK)
                .expect("something to commit")
                .options
                .iter()
                .map(|o| o.id.clone())
                .collect()
        };
        let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &home);
        window.stage = Stage::Committing;
        // Neutral: only the space infantry (index 0) to every landable planet, then the decline.
        let plain = ids(&state, &window);
        assert!(
            plain.iter().all(|id| !id.starts_with("commit|1|")),
            "{plain:?}"
        );

        let hooks = GroundHooks {
            commit_candidates: Some(commit_from_09),
            ..GroundHooks::NONE
        };
        with_test_hooks(hooks, || {
            let extended = ids(&state, &window);
            // The ordinary options keep their ids; the extra source follows, and never offers the
            // planet it already stands on.
            assert_eq!(&extended[..plain.len() - 1], &plain[..plain.len() - 1]);
            assert_eq!(extended.last().map(String::as_str), Some("done_committing"));
            let extras: Vec<&String> = extended
                .iter()
                .filter(|id| id.starts_with("commit|1|"))
                .collect();
            assert_eq!(extras, vec![&format!("commit|1|{target}")]);
            let option = window
                .pending_choice(&state, content, POK)
                .unwrap()
                .options
                .into_iter()
                .find(|o| o.id.starts_with("commit|1|"))
                .unwrap();
            assert_eq!(option.payload["from_planet"], "maaluuk");
            assert_eq!(option.payload["from_system"], "09");

            // Commit it through the window.
            let pick = format!("commit|1|{target}");
            let (_, events) = drive(&mut state, &mut window, &[&pick, "done_committing"], &[]);
            let committed = events
                .iter()
                .find(|(name, _)| name == "UNITS_COMMITTED")
                .expect("the commitment is announced");
            assert_eq!(text(&committed.1, "from_planet"), "maaluuk");
            assert_eq!(text(&committed.1, "planet"), target.to_string());
        });
        let board = state.system_state(&home);
        assert!(
            board
                .on_planet_of(&PlanetId::new("maaluuk"), &a())
                .is_empty(),
            "the unit left its planet"
        );
        assert_eq!(
            board.on_planet_of(&target, &a()).len(),
            1,
            "and landed on the target"
        );
        assert_eq!(
            window.extra_commits,
            vec![(home.clone(), PlanetId::new("maaluuk"))],
            "the engine remembers where it moved from, so `already` limits the hook"
        );
    }

    fn liar(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        invader: &PlayerId,
        _: &SystemId,
        _: &[CommitOrigin],
    ) -> Vec<CommitCandidate> {
        // Nothing stands on maaluuk.
        vec![CommitCandidate {
            system: SystemId::new("09"),
            planet: PlanetId::new("maaluuk"),
            unit: Unit::new(UnitTypeId::new("infantry"), invader.clone()),
        }]
    }

    fn temporary_fighter(
        state: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        invader: &PlayerId,
        system: &SystemId,
    ) -> Vec<Unit> {
        state
            .system_state(system)
            .units_of(invader)
            .into_iter()
            .filter(|unit| unit.type_id.as_str() == "fighter")
            .cloned()
            .collect()
    }

    fn temporary_fighter_on_planet(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: &PlayerId,
        _: &SystemId,
        _: &PlanetId,
        unit: &Unit,
    ) -> bool {
        unit.type_id.as_str() == "fighter"
    }

    #[test]
    fn temporary_space_commit_survivors_return_before_control() {
        let (mut state, system, planet) = arena();
        state.system_mut(&system).set_control(planet.clone(), b());
        crate::fixtures::put(&mut state, &system, "fighter", &a(), 1);
        on_planet(&mut state, &system, &planet, "infantry", &b());
        let hooks = GroundHooks {
            temporary_space_commit_candidates: Some(temporary_fighter),
            temporary_ground_force: Some(temporary_fighter_on_planet),
            ..GroundHooks::NONE
        };
        let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &system);
        let commit = format!("commit|0|{planet}");
        let (_, events) = with_test_hooks(hooks, || {
            drive(&mut state, &mut window, &[&commit, "fight"], &[10, 1])
        });
        assert!(
            state
                .system_state(&system)
                .on_planet_of(&planet, &a())
                .is_empty(),
            "the surviving fighter has returned from the planet"
        );
        assert_eq!(
            state
                .system_state(&system)
                .units_of(&a())
                .into_iter()
                .filter(|unit| unit.type_id.as_str() == "fighter")
                .count(),
            1,
            "the survivor returns to the space area"
        );
        assert_eq!(
            state.system_state(&system).planet_control.get(&planet),
            Some(&b()),
            "a fighter-only win cannot establish control"
        );
        let ended = events
            .iter()
            .find(|(name, _)| name == "GROUND_COMBAT_ENDED")
            .expect("the landing fought");
        assert_eq!(ended.1["control_changed"], serde_json::Value::Bool(false));
    }

    #[test]
    fn a_candidate_that_is_not_really_there_is_not_offered() {
        let content = ContentStore::embedded();
        let mut state = start_game(content, &[a(), b()], POK, None).unwrap();
        let home = SystemId::new("09");
        crate::fixtures::put(&mut state, &home, "infantry", &a(), 1);
        let hooks = GroundHooks {
            commit_candidates: Some(liar),
            ..GroundHooks::NONE
        };
        let (troops, origins, _) = with_test_hooks(hooks, || {
            commit_pool(&state, content, POK, None, &a(), &home, &[])
        });
        assert_eq!(troops.len(), 1);
        assert_eq!(origins, vec![None]);
    }

    fn free_for_a(_: &GameState, _: &ContentStore, player: &PlayerId) -> bool {
        player.as_str() == "a"
    }

    #[test]
    fn the_custodians_token_can_come_down_free_from_a_window_opened_at_the_commit_step() {
        let content = ContentStore::embedded();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let setup = || {
            let mut state = start_game(content, &[a(), b()], POK, None).unwrap();
            state.board.entry(mecatol.clone()).or_default();
            crate::fixtures::put(&mut state, &mecatol, "infantry", &a(), 1);
            // A fresh game controls no planets, so with no trade goods there is no influence.
            state.player_mut(&a()).unwrap().trade_goods = 0;
            state
        };

        let state = setup();
        assert_eq!(custodians_cost(&state, content, &a()), CUSTODIANS_COST);
        assert!(
            !custodians_removable(&state, content, POK, &a(), &mecatol),
            "neutral: a broke seat cannot lift it"
        );

        let hooks = GroundHooks {
            custodians_free: Some(free_for_a),
            ..GroundHooks::NONE
        };
        with_test_hooks(hooks, || {
            let mut state = setup();
            assert_eq!(custodians_cost(&state, content, &a()), 0);
            assert_eq!(custodians_cost(&state, content, &b()), CUSTODIANS_COST);
            assert!(custodians_removable(&state, content, POK, &a(), &mecatol));
            let vp = state.player(&a()).unwrap().victory_points;

            // Item 9 (Sardakk hero): no bombardment, no dice.
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
            let choice = window.pending_choice(&state, content, POK).expect("asked");
            assert!(
                choice.prompt.contains("no influence required"),
                "{}",
                choice.prompt
            );
            let (asks, events) = drive(&mut state, &mut window, &["yes", "done_committing"], &[]);
            assert_eq!(asks.len(), 2, "{asks:?}");
            assert!(state.custodians_removed);
            assert_eq!(state.player(&a()).unwrap().victory_points, vp + 1);
            assert_eq!(
                state.player(&a()).unwrap().trade_goods,
                0,
                "nothing was paid"
            );
            let finished = events
                .iter()
                .find(|(name, _)| name == "GROUND_COMMITMENT_FINISHED")
                .expect("the commit step announces its end");
            assert_eq!(text(&finished.1, "player"), "a");
            assert_eq!(text(&finished.1, "system"), mecatol.to_string());
            assert_eq!(finished.1["planets"], serde_json::json!([]));
        });
    }

    #[test]
    fn the_custodians_stage_runs_as_a_core_window_in_lockstep() {
        // Declining, the paid removal (through B3's PayStep) and the free removal (Winnu) are compared: the core
        // puts the same questions, takes the same answers and leaves the same state.
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let setup = |faction: &str| {
            let mut state = crate::fixtures::seated_game(&[("a", faction), ("b", "letnev")], POK);
            state.board.entry(mecatol.clone()).or_default();
            crate::fixtures::put(&mut state, &mecatol, "infantry", &a(), 1);
            state.player_mut(&a()).unwrap().trade_goods = 4;
            // The seats leave placeholder-faction notes the card slice cannot map.
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state
        };
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let mut state = setup("sol");
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
            let (asks, _) = drive(&mut state, &mut window, &["no", "done_committing"], &[]);
            assert_eq!(asks.len(), 2, "{asks:?}");
            assert!(!state.custodians_removed);

            let mut state = setup("sol");
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
            let paid = ["yes", "exhaust|jord", "done_committing"];
            let (_, _) = drive(&mut state, &mut window, &paid, &[]);
            assert!(state.custodians_removed, "the paid removal still happened on the string side");

            // Winnu's blood ties lift the token for nothing.
            let mut state = setup("winnu");
            let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
            let vp = state.player(&a()).unwrap().victory_points;
            let (_, _) = drive(&mut state, &mut window, &["yes", "done_committing"], &[]);
            assert_eq!(state.player(&a()).unwrap().victory_points, vp + 1);
        });
        let r = report();
        let w = r.windows.get("invasion::answer_custodians").copied().unwrap_or_default();
        assert!(w.compared >= 3, "decline, the paid and the free removal are compared: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Package W5: the paid removal of the custodians token at a table with a Naaz, Nomad, Yssaril or Mentak seat is a core window
    /// (the pre-W5 guard `unported_window` bailed it: "paid removal at a table with a faction the production window does not carry").
    #[test]
    fn the_paid_custodians_removal_runs_in_the_core_at_tables_with_the_tier_w_seats() {
        use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
        let _guard = crate::dual::tests::serial();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        for seats in [[("a", "naaz"), ("b", "letnev")], [("a", "sol"), ("b", "nomad")], [("a", "sol"), ("b", "yssaril")], [("a", "mentak"), ("b", "naaz")]] {
            let _ = take_report();
            with_mask(mask, || {
                let mut state = crate::fixtures::seated_game(&seats, POK);
                state.board.entry(mecatol.clone()).or_default();
                crate::fixtures::put(&mut state, &mecatol, "infantry", &a(), 1);
                state.player_mut(&a()).unwrap().trade_goods = 8;
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
                let (_, _) = drive(&mut state, &mut window, &["yes", "trade_good", "trade_good", "trade_good", "trade_good", "trade_good", "trade_good", "done_committing"], &[]);
                assert!(state.custodians_removed, "{seats:?}");
            });
            let r = report();
            let w = r.windows.get("invasion::answer_custodians").copied().unwrap_or_default();
            assert!(w.compared >= 1 && w.bailed == 0, "{seats:?}: {:?}", r.windows);
            assert_eq!(r.total_differences(), 0, "{seats:?}: {:?}", r.differences);
        }
        let _ = take_report();
    }

    /// Lifting the custodians token pays its six influence through the ordinary payment question:
    /// typed `pay_influence` with the whole bill outstanding, one `pay` option per planet (`exhaust|<planet>`)
    /// with its worth, so the client renders it with the payment panel (both printed
    /// values beside every planet) instead of a bare list of "exhaust" buttons.
    #[test]
    fn the_custodians_payment_is_an_ordinary_influence_payment_question() {
        let content = ContentStore::embedded();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        let mut state = start_game(content, &[a(), b()], POK, None).unwrap();
        state.board.entry(mecatol.clone()).or_default();
        crate::fixtures::put(&mut state, &mecatol, "infantry", &a(), 1);
        state.player_mut(&a()).unwrap().trade_goods = 0;
        // Planets worth 2+ influence each, until there are at least six: several planets, so the
        // payment is a real question.
        let mut influence = 0;
        for (id, planet) in ti4_content::galaxy::all_planets(content, POK) {
            if influence >= 7 {
                break;
            }
            if planet.homeworld_of().is_some()
                || planet.is_placed_during_play()
                || planet.influence() < 2
                || id == "mr"
            {
                continue;
            }
            let system = SystemId::new(planet.system_id().unwrap());
            state
                .system_mut(&system)
                .set_control(PlanetId::new(id), a());
            influence += planet.influence();
        }
        assert!(custodians_removable(&state, content, POK, &a(), &mecatol));

        let (capturing, seen) =
            crate::choice::Capturing::new(Box::new(crate::choice::Scripted::new(["yes"])));
        let mut table = Table::with_default(Box::new(capturing));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(7);
        let mut window = InvasionWindow::at_commit_step(&mut state, &a(), &mecatol);
        {
            let mut ctx = Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: None,
            };
            let first = window.pending_choice(&state, content, POK).expect("asked");
            assert_eq!(first.context.as_ref().unwrap().subtype, "remove_custodians");
            let answer = ctx.ask_seeing(&state, &first).unwrap();
            window.resolve(&mut state, &mut ctx, answer).unwrap();
        }
        assert!(state.custodians_removed);
        let seen = seen.borrow();
        let pay = seen
            .iter()
            .find(|choice| {
                choice
                    .context
                    .as_ref()
                    .is_some_and(|context| context.subtype == "pay_influence")
            })
            .expect("the six influence were asked for");
        let owed = &pay.context.as_ref().unwrap().outstanding[0];
        assert_eq!((owed.amount, owed.paid), (CUSTODIANS_COST, 0));
        assert!(pay.options.len() > 1, "{:?}", pay.ids());
        for option in &pay.options {
            assert_eq!(option.kind, crate::production::PAY_KIND);
            assert!(
                option.id.starts_with("exhaust|") || option.id == "trade_good",
                "{option:?}"
            );
            assert!(option.payload.get("worth").is_some(), "{option:?}");
        }
    }

    #[test]
    fn a_timing_effect_can_assign_ordinary_hits_and_the_events_follow() {
        let (mut state, system, planet) = arena();
        on_planet(&mut state, &system, &planet, "mech", &b());
        on_planet(&mut state, &system, &planet, "infantry", &b());
        let (mut resolver, seen) = listening();
        let (mut table, mut dice, mut rng) = (Table::default(), Dice::new(), GameRng::new(3));
        let mut sequence = crate::event::EventSequence::new();
        let destroyed = {
            let mut ctx = TimingContext {
                state: &mut state,
                content: ContentStore::embedded(),
                sources: POK,
                table: &mut table,
                dice: &mut dice,
                rng: &mut rng,
                event_sequence: &mut sequence,
                galaxy: None,
            };
            assign_ground_hits_in_timing(&mut resolver, &mut ctx, &system, &planet, &b(), 2, "test")
                .unwrap()
        };
        assert_eq!(destroyed, 1, "the mech sustained, the infantry died");
        let seen = seen.lock().unwrap();
        let names: Vec<&str> = seen.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["GROUND_FORCE_SUSTAINED", "GROUND_FORCE_DESTROYED"]);
        assert_eq!(text(&seen[1].1, "unit"), "infantry");
        assert_eq!(text(&seen[1].1, "cause"), "test");
        let left = state
            .system_state(&system)
            .on_planet_of(&planet, &b())
            .len();
        assert_eq!(left, 1, "the damaged mech stands");
    }

    fn mark_round(
        state: &mut GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: &mut Table,
        who: &PlayerId,
        _: &SystemId,
        _: &PlanetId,
    ) {
        let key = format!("test:round:{who}");
        let seen: u32 = state
            .faction_marks
            .get(&key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        state.faction_marks.insert(key, (seen + 1).to_string());
    }

    #[test]
    fn the_ground_round_start_hook_runs_once_per_participant_per_round() {
        let rounds = |hooked: bool| {
            let (mut state, system, planet) = mech_defends();
            let mut window = fight_window(&mut state, &system, &planet);
            let run = |state: &mut GameState, window: &mut InvasionWindow| {
                drive(state, window, &["fight", "fight"], &[10, 1, 1, 10, 1, 1]);
            };
            if hooked {
                let hooks = GroundHooks {
                    ground_combat_round_started: Some(mark_round),
                    ..GroundHooks::NONE
                };
                with_test_hooks(hooks, || run(&mut state, &mut window));
            } else {
                run(&mut state, &mut window);
            }
            (
                state.faction_marks.get("test:round:a").cloned(),
                state.faction_marks.get("test:round:b").cloned(),
                state
                    .system_state(&system)
                    .planet_control
                    .get(&planet)
                    .cloned(),
            )
        };
        let (a_rounds, b_rounds, controller) = rounds(true);
        assert_eq!(
            a_rounds.as_deref(),
            Some("2"),
            "invader, each of two rounds"
        );
        assert_eq!(
            b_rounds.as_deref(),
            Some("2"),
            "defender, each of two rounds"
        );
        let (none_a, none_b, plain_controller) = rounds(false);
        assert_eq!((none_a, none_b), (None, None), "neutral without the hook");
        assert_eq!(controller, plain_controller, "the hook changed no outcome");
    }

    #[test]
    fn the_synchronous_ground_combat_also_starts_each_round_through_the_hook() {
        let (mut state, system, planet) = mech_defends();
        let hooks = GroundHooks {
            ground_combat_round_started: Some(mark_round),
            ..GroundHooks::NONE
        };
        let mut table = Table::default();
        let mut dice = Dice::from_faces([10, 1, 1, 10, 1, 1]);
        let mut rng = GameRng::new(7);
        with_test_hooks(hooks, || {
            let _ = ground_combat(
                &mut state,
                ContentStore::embedded(),
                POK,
                &mut table,
                &mut dice,
                &mut rng,
                &system,
                &planet,
                &a(),
            );
        });
        let rounds: u32 = state.faction_marks["test:round:a"].parse().unwrap();
        assert!(rounds >= 1);
        assert_eq!(
            state.faction_marks["test:round:a"], state.faction_marks["test:round:b"],
            "both participants, every round"
        );
    }

    #[test]
    fn a_ship_form_unit_lands_in_its_ground_form() {
        let (mut state, system, planet) = arena();
        let content = ContentStore::embedded();
        let eidolon = Unit::new(UnitTypeId::new("naaz_mech_space"), a());
        land_unit(&mut state, content, POK, &a(), &system, &planet, eidolon);
        let infantry = Unit::new(UnitTypeId::new("infantry"), a());
        land_unit(&mut state, content, POK, &a(), &system, &planet, infantry);
        let board = state.system_state(&system);
        let landed: Vec<&str> = board
            .on_planet_of(&planet, &a())
            .iter()
            .map(|unit| unit.type_id.as_str())
            .collect();
        assert_eq!(
            landed,
            ["naaz_mech", "infantry"],
            "only the dual-form unit flipped"
        );
    }

    #[test]
    fn committing_a_space_form_mech_lands_it_as_the_ground_form() {
        let (mut state, system, planet) = arena();
        // Nothing of ours or anybody else's stands there; clear the arena for the commit.
        state.system_mut(&system).planet_units.clear();
        state.system_mut(&system).units.clear();
        crate::fixtures::put(&mut state, &system, "naaz_mech_space", &a(), 1);
        let script = vec![format!("commit|0|{planet}"), "done_committing".to_owned()];
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(script)));
        let committed = commit_ground_forces(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &a(),
            &system,
        )
        .unwrap();
        assert_eq!(committed, vec![planet.clone()]);
        let board = state.system_state(&system);
        let landed: Vec<&str> = board
            .on_planet_of(&planet, &a())
            .iter()
            .map(|unit| unit.type_id.as_str())
            .collect();
        assert_eq!(landed, ["naaz_mech"]);
        assert!(state.system_state(&system).units_of(&a()).is_empty());
    }

    thread_local! {
        static SAW_GALAXY: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    fn note_galaxy(
        _: &GameState,
        _: &ContentStore,
        _: ti4_model::content_types::SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        _: &PlayerId,
        _: &SystemId,
        _: &[CommitOrigin],
    ) -> Vec<CommitCandidate> {
        SAW_GALAXY.with(|cell| cell.set(Some(galaxy.is_some())));
        Vec::new()
    }

    #[test]
    fn the_commit_hook_receives_the_map_the_window_was_given() {
        let content = ContentStore::embedded();
        let (mut state, system, _) = arena();
        crate::fixtures::put(&mut state, &system, "infantry", &a(), 1);
        let hub = crate::fixtures::plain_hub();
        let hooks = GroundHooks {
            commit_candidates: Some(note_galaxy),
            ..GroundHooks::NONE
        };
        let window = InvasionWindow::at_commit_step(&mut state, &a(), &system);
        with_test_hooks(hooks, || {
            let _ = window.landing_options(&state, content, POK);
        });
        assert_eq!(
            SAW_GALAXY.with(std::cell::Cell::get),
            Some(false),
            "no map yet"
        );
        SAW_GALAXY.with(|cell| cell.set(None));
        let window = window.with_galaxy(&hub.galaxy);
        with_test_hooks(hooks, || {
            let _ = window.landing_options(&state, content, POK);
        });
        assert_eq!(SAW_GALAXY.with(std::cell::Cell::get), Some(true));
    }
}

/// Package B2: the bombardment rules answered by `rules::invasion` in lockstep with the string rules.
#[cfg(all(test, feature = "typed-core"))]
mod core_bombardment {
    use super::*;
    use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;

    fn invader() -> PlayerId {
        PlayerId::new("a")
    }
    fn holder() -> PlayerId {
        PlayerId::new("b")
    }

    /// A system with a planet of its own that is not a scar.
    fn arena() -> (GameState, SystemId, PlanetId) {
        arena_of("letnev", "sol")
    }

    fn arena_of(fa: &str, fb: &str) -> (GameState, SystemId, PlanetId) {
        let mut state = crate::fixtures::seated_game(&[("a", fa), ("b", fb)], POK);
        // The fixture deals the notes before the factions are seated ("cf:generic"): no note for the codec.
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        let all = ti4_content::galaxy::all_planets(ContentStore::embedded(), POK);
        let (system, planet) = all
            .iter()
            .find(|(_, p)| {
                p.system_id().is_some_and(|s| {
                    !p.is_placed_during_play() && ti4_content::galaxy::planets_in(ContentStore::embedded(), s, POK).len() >= 2
                })
            })
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
            .expect("the corpus has a system with two planets");
        (state, system, planet)
    }

    fn put(state: &mut GameState, system: &SystemId, planet: Option<&PlanetId>, kind: &str, owner: &PlayerId, n: usize) {
        for _ in 0..n {
            let unit = Unit::new(UnitTypeId::new(kind), owner.clone());
            match planet {
                Some(p) => state.system_mut(system).planet_units.entry(p.clone()).or_default().push(unit),
                None => state.system_mut(system).units.push(unit),
            }
        }
    }

    fn plan_of(state: &GameState, system: &SystemId) -> Vec<PlannedBombard> {
        bombard_rolls(state, ContentStore::embedded(), POK, system, &invader())
    }

    #[test]
    fn the_bombardment_rules_run_in_the_core_in_lockstep() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            // A PDS shields the planet; a war sun ignores it; a dreadnought alone rolls on 5.
            let (mut state, system, planet) = arena();
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 3);
            put(&mut state, &system, Some(&planet), "pds", &holder(), 1);
            put(&mut state, &system, None, "dreadnought", &invader(), 2);
            assert!(plan_of(&state, &system).is_empty(), "the shield holds");
            assert!(!bombardable(&state, ContentStore::embedded(), POK, &system, &planet, &invader()));
            put(&mut state, &system, None, "warsun", &invader(), 1);
            let plan = plan_of(&state, &system);
            assert_eq!(plan.len(), 1);
            assert_eq!(plan[0].held, 3 + 1 - 1, "the PDS is not a ground force");
            assert_eq!(plan[0].rolls.len(), 3);

            // Blitz gives a carrier BOMBARDMENT 6; Plasma Scoring adds a die to the first roller; a
            // galvanized dreadnought rolls one more; the Bunker lowers each result by four.
            put(&mut state, &system, None, "carrier", &invader(), 1);
            let act = state.activation_seq;
            state.player_mut(&invader()).unwrap().blitz_invasion.push(act);
            state.player_mut(&invader()).unwrap().technologies.insert(ti4_model::TechnologyId::new("ps"));
            if let Some(u) = state.system_mut(&system).units.iter_mut().find(|u| u.type_id.as_str() == "dreadnought") {
                u.galvanized = true;
            }
            let plan = plan_of(&state, &system);
            assert!(plan[0].rolls.iter().any(|r| r.0 == "carrier" && r.1 == 6));
            state.system_mut(&system).planet_control.insert(planet.clone(), holder());
            let act = state.activation_seq;
            state.player_mut(&holder()).unwrap().bunker_invasion.push(act);
            let plan = plan_of(&state, &system);
            assert!(plan[0].rolls.iter().all(|r| r.1 >= 1));

            // Disable: an opponent's copy strips the shield again.
            let (mut state, system, planet) = arena();
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 1);
            put(&mut state, &system, Some(&planet), "pds", &holder(), 1);
            put(&mut state, &system, None, "dreadnought", &invader(), 1);
            assert!(!bombardable(&state, ContentStore::embedded(), POK, &system, &planet, &invader()));
            let act = state.activation_seq;
            state.player_mut(&invader()).unwrap().disable_invasion.push(act);
            assert!(bombardable(&state, ContentStore::embedded(), POK, &system, &planet, &invader()));

            // The Anihilator bombards from another planet of its system.
            let others: Vec<PlanetId> = ti4_content::galaxy::planets_in(ContentStore::embedded(), system.as_str(), POK)
                .into_iter()
                .map(|p| PlanetId::new(p.id()))
                .filter(|p| p != &planet)
                .collect();
            if let Some(other) = others.first() {
                put(&mut state, &system, Some(other), "l1z1x_mech", &invader(), 1);
                put(&mut state, &system, Some(&planet), "infantry", &holder(), 1);
                let plan = plan_of(&state, &system);
                assert!(plan.iter().any(|p| p.planet == planet && p.rolls.iter().any(|r| r.0 == "l1z1x_mech")), "{plan:?}");
            }
        });
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 8, "the core answered: {counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_bombardment_dice_are_drawn_by_the_core_in_lockstep() {
        // The core draws the same faces per bombarding unit, from the same stream, and leaves the streams
        // where the string loop does (several planets' worth of groups: a plasma-scored dreadnought pair and
        // a war sun, two seeds).
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            for seed in [3_u64, 11] {
                let (mut state, system, planet) = arena();
                put(&mut state, &system, Some(&planet), "infantry", &holder(), 3);
                put(&mut state, &system, None, "dreadnought", &invader(), 2);
                put(&mut state, &system, None, "warsun", &invader(), 1);
                state.player_mut(&invader()).unwrap().technologies.insert(ti4_model::TechnologyId::new("ps"));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let plan = roll_bombard_plan(&mut state, ContentStore::embedded(), POK, &mut dice, &mut rng, &system, &invader());
                assert_eq!(plan.len(), 1);
                assert!(dice.history().len() >= 3, "every bombarding unit rolled");
            }
        });
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 2, "the core drew the dice: {counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_core_backend_draws_the_same_bombardment_dice_as_the_string_loop() {
        let _guard = crate::dual::tests::serial();
        let run = |backend: Backend, seed: u64| {
            let mask = AreaMask::STRING.with(Area::Invasion, backend);
            with_mask(mask, || {
                let (mut state, system, planet) = arena();
                put(&mut state, &system, Some(&planet), "infantry", &holder(), 3);
                put(&mut state, &system, None, "dreadnought", &invader(), 2);
                put(&mut state, &system, None, "warsun", &invader(), 1);
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let plan = roll_bombard_plan(&mut state, ContentStore::embedded(), POK, &mut dice, &mut rng, &system, &invader());
                let groups: Vec<Vec<usize>> = plan.iter().map(|p| p.groups.clone()).collect();
                let faces: Vec<Vec<u32>> = dice.history().iter().map(|r| r.faces.clone()).collect();
                (groups, faces, rng.positions(), serde_json::to_string(&state.reroll_staging).unwrap())
            })
        };
        for seed in [5_u64, 17, 23] {
            let string = run(Backend::String, seed);
            let core = run(Backend::Core, seed);
            assert!(!string.1.is_empty());
            assert_eq!(string, core, "seed {seed}");
        }
    }

    #[test]
    fn ground_hits_are_decided_by_the_core_in_lockstep() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let (mut state, system, planet) = arena();
            // A mech sustains first, then the damaged infantry goes before the fresh one, then the mech.
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 2);
            put(&mut state, &system, Some(&planet), "sol_mech", &holder(), 1);
            put(&mut state, &system, Some(&planet), "pds", &holder(), 1);
            let (c, s) = (ContentStore::embedded(), POK);
            for _ in 0..5 {
                let mut log = Vec::new();
                let _ = ground_hit_outcome_logged(&mut state, c, s, &system, &planet, &holder(), "ground_combat", &mut log, false);
            }
            assert!(state.system_state(&system).on_planet(&planet).iter().all(|u| u.type_id.as_str() == "pds"));
            // Bombardment hits in a row: the mech sustains, then infantry fall.
            let (mut state, system, planet) = arena();
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 2);
            put(&mut state, &system, Some(&planet), "sol_mech", &holder(), 1);
            let mut log = Vec::new();
            assert_eq!(take_bombard_hits(&mut state, c, s, &system, &planet, &holder(), 4, &mut log), 3);
            let (mut state, system, planet) = arena();
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 1);
            // A hit of a unit ability skips a Naaz Maximum.
            put(&mut state, &system, Some(&planet), "naaz_mech_space", &holder(), 1);
            let mut log = Vec::new();
            let _ = ground_hit_outcome_logged(&mut state, c, s, &system, &planet, &holder(), "bombardment", &mut log, true);
            // The same batches through the core's hits step (`rules::invasion::hits`): the events queued for the
            // caller are compared next to the board and the destroyed count.
            let (mut state, system, planet) = arena();
            put(&mut state, &system, Some(&planet), "infantry", &holder(), 2);
            put(&mut state, &system, Some(&planet), "sol_mech", &holder(), 1);
            let mut log = Vec::new();
            remove_ground(&mut state, c, s, &system, &planet, &holder(), 2, "ground_combat", &mut log);
            assert_eq!(log.len(), 2);
            let mut log = Vec::new();
            remove_ground(&mut state, c, s, &system, &planet, &holder(), 3, "harrow", &mut log);
        });
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 6, "{counts:?}");
        for label in ["invasion::take_bombard_hits", "invasion::remove_ground"] {
            let w = r.windows.get(label).unwrap_or_else(|| panic!("{label} not seen: {:?}", r.windows.keys().collect::<Vec<_>>()));
            assert!(w.compared >= 1, "{label}: {w:?}");
        }
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn ground_rolls_are_planned_by_the_core_in_lockstep() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual).with(Area::Factions, Backend::Dual);
        with_mask(mask, || {
            for (fa, fb) in [("jolnar", "sardakk"), ("letnev", "sol"), ("nekro", "mentak")] {
                let (mut state, system, planet) = arena_of(fa, fb);
                put(&mut state, &system, Some(&planet), "infantry", &invader(), 2);
                put(&mut state, &system, Some(&planet), &format!("{fa}_mech"), &invader(), 1);
                put(&mut state, &system, Some(&planet), "infantry2", &holder(), 3);
                put(&mut state, &system, Some(&planet), &format!("{fb}_mech"), &holder(), 1);
                if let Some(u) = state.system_mut(&system).planet_units.get_mut(&planet).and_then(|v| v.first_mut()) {
                    u.galvanized = true;
                }
                for who in [invader(), holder()] {
                    let _ = ground_pools(&state, ContentStore::embedded(), POK, &who, &system, &planet);
                    let _ = ground_combat_value(&state, ContentStore::embedded(), POK, &who, &system, &planet, "infantry");
                }
            }
        });
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 12, "{counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn committing_ground_forces_runs_as_a_core_window_in_lockstep() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        let (state, system, planet) = arena();
        let mut results = Vec::new();
        with_mask(mask, || {
            for answers in [vec!["done_committing".to_owned()], vec!["commit|0|PLANET".to_owned(), "commit|0|PLANET".to_owned()]] {
                let mut state = state.clone();
                put(&mut state, &system, None, "infantry", &invader(), 1);
                put(&mut state, &system, None, "sol_mech", &invader(), 1);
                let answers: Vec<String> = answers.into_iter().map(|a| a.replace("PLANET", planet.as_str())).collect();
                let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(answers)));
                let committed = commit_ground_forces(&mut state, ContentStore::embedded(), POK, &mut table, &invader(), &system);
                results.push((committed.map(|c| c.len()), state.system_state(&system).on_planet(&planet).len()));
            }
        });
        assert_eq!(results[0], (Ok(0), 0));
        assert_eq!(results[1].0, Ok(1));
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 2, "{counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn assigning_ground_hits_runs_as_a_core_window_in_lockstep() {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        let (state, system, planet) = arena();
        with_mask(mask, || {
            for (hits, answers) in [(1, vec!["destroy|1"]), (2, vec!["destroy|0", "destroy|0"]), (5, vec![])] {
                let mut state = state.clone();
                put(&mut state, &system, Some(&planet), "infantry", &holder(), 2);
                put(&mut state, &system, Some(&planet), "infantry2", &holder(), 1);
                put(&mut state, &system, Some(&planet), "sol_mech", &holder(), 1);
                let mut table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(answers.into_iter().map(str::to_owned))));
                let _ = absorb_ground(&mut state, ContentStore::embedded(), POK, &mut table, &holder(), &system, &planet, hits);
            }
        });
        let r = report();
        let counts = &r.areas[&Area::Invasion];
        assert!(counts.calls >= 3, "{counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }
}

/// Package B2: the pre-checks (bails) of the ground round's start and roll tail windows, which the golden games do
/// not all reach.
#[cfg(all(test, feature = "typed-core"))]
mod core_ground_round_prechecks {
    use super::*;
    use crate::dual_window::{begin_ground_unported, roll_tail_unported, BeginGroundArgs, RollTailArgs};
    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;

    fn game(fa: &str, fb: &str) -> (GameState, SystemId, PlanetId) {
        let state = crate::fixtures::seated_game(&[("a", fa), ("b", fb)], POK);
        (state, SystemId::new("18"), PlanetId::new("mr"))
    }

    fn begin<'a>(a: &'a PlayerId, b: &'a PlayerId, s: &'a SystemId, p: &'a PlanetId, copied: bool) -> BeginGroundArgs<'a> {
        BeginGroundArgs { invader: a, defender: b, system: s, planet: p, copied_round: copied, sources: POK }
    }

    #[test]
    fn the_round_start_bails_name_what_the_core_window_does_not_carry() {
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let (state, s, p) = game("sol", "hacan");
        assert_eq!(begin_ground_unported(&state, &begin(&a, &b, &s, &p, false)), None);
        // A copied round agent is carried (numbered and announced).
        assert_eq!(begin_ground_unported(&state, &begin(&a, &b, &s, &p, true)), None);
        // A Naaz side is carried since package W1 (Supercharge and the Maximum's repair are a hook of the core window).
        let (state, s, p) = game("naaz", "hacan");
        assert_eq!(begin_ground_unported(&state, &begin(&a, &b, &s, &p, false)), None);
        // Letnev only offers the Dunlain Reaper with infantry on the planet.
        let (mut state, s, p) = game("sol", "letnev");
        assert_eq!(begin_ground_unported(&state, &begin(&a, &b, &s, &p, false)), None);
        state.system_mut(&s).planet_units.entry(p.clone()).or_default().push(Unit::new(UnitTypeId::new("infantry"), b.clone()));
        assert_eq!(begin_ground_unported(&state, &begin(&a, &b, &s, &p, false)), Some("Dunlain Reaper"));
        let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
        assert_eq!(begin_ground_unported(&state, &begin(&neutral, &b, &s, &p, false)), Some("neutral side"));
    }

    /// Dice an ability rolled need the streams (`Sources::rng`, a seeded driver); a window that is lent none cannot
    /// follow the roll: it is counted under the window's label (it was not: the roll came out of a stream at position
    /// zero and showed as a `nested.ability_dice` difference). The same recording without dice is followed.
    #[test]
    fn bail_dice_rolled_by_an_ability_in_a_window_lent_no_stream_is_silent() {
        use crate::dual::{report, take_report};
        use crate::dual_window::{begin_ground, Recording, ResolverCfg};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let (mut state, s, p) = game("sol", "hacan");
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        crate::fixtures::put_on_planet(&mut state, &s, &p, "infantry", &b, 1);
        let cfg = ResolverCfg {
            initiative: vec!["a".to_owned(), "b".to_owned()],
            seating: vec!["a".to_owned(), "b".to_owned()],
            active: Some("a".to_owned()),
            speaker: None,
            phase: ti4_model::state::Phase::Action,
            round: 1,
            turn: 1,
            max_depth: 16,
            used: Vec::new(),
            sources: POK,
            stack: Vec::new(),
        };
        let mut script = Recording::default();
        script.nested = true;
        script.cfg = Some(cfg);
        let args = begin(&a, &b, &s, &p, false);
        assert!(begin_ground(&mut state.clone(), &args, &script).is_some(), "nested activity with no dice is followed");
        script.dice = vec![(Some(6), vec![3])];
        assert!(begin_ground(&mut state.clone(), &args, &script).is_none(), "dice with no stream lent: the string result stands");
        let r = report();
        assert!(r.windows.keys().any(|k| k.contains("lent no stream")), "the bail is counted: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    fn advance_args<'a>(a: &'a PlayerId, s: &'a SystemId, planets: &'a [PlanetId], none: &'a [PlanetId]) -> crate::dual_window::AdvanceArgs<'a> {
        crate::dual_window::AdvanceArgs { invader: a, system: s, planets, index: 0, coexisted: none, sources: POK }
    }

    fn advance_bail(state: &mut GameState, rec: &crate::dual_window::Recording, reason: &str) {
        use crate::dual::{report, take_report};
        let _ = take_report();
        let (a, s, p) = (PlayerId::new("a"), SystemId::new("18"), [PlanetId::new("mr")]);
        assert!(crate::dual_window::advance(state, &advance_args(&a, &s, &p, &[]), rec).is_none());
        let r = report();
        assert!(r.windows.keys().any(|k| k.contains(reason)), "{:?}", r.windows);
        let _ = take_report();
    }

    #[test]
    fn the_advance_bails_nested_and_seats_out_of_id_order() {
        let _guard = crate::dual::tests::serial();
        let (mut state, _, _) = game("sol", "hacan");
        let nested = { let mut r = crate::dual_window::Recording::default(); r.nested = true; r };
        advance_bail(&mut state, &nested, "nested");
        // The owner table is sorted by id (a BTreeSet), so "seats not in id order" cannot arise: the guard is a
        // defensive check and has no reachable case.
        let swapped = crate::fixtures::seated_game(&[("b", "sol"), ("a", "hacan")], POK);
        let (cx, r) = crate::dual_window::tables(&swapped).unwrap();
        let _ = cx;
        assert!(r.seats.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_choice_of_the_next_fight_runs_as_a_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mask = AreaMask::STRING.with(Area::Invasion, Backend::Dual);
        with_mask(mask, || {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = 9;
            let (system, planet) = crate::fixtures::a_placed_planet();
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 3);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(9));
            resolve(&mut state, ContentStore::embedded(), POK, &mut table, &mut dice, &mut rng, &system, &PlayerId::new("a")).unwrap();
        });
        let r = report();
        let w = r.windows.get("invasion::advance_fighting").copied().unwrap_or_default();
        assert!(w.compared >= 1, "the choice of the fight is compared: {:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_roll_tail_bails_name_what_the_core_window_does_not_carry() {
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let (mut state, s, p) = game("sol", "hacan");
        let args = RollTailArgs { invader: &a, defender: &b, system: &s, planet: &p, rolled: (1, 1), bump: true, announce: true };
        assert_eq!(roll_tail_unported(&state, &args), None);
        state.player_mut(&a).unwrap().technologies.insert(ti4_model::TechnologyId::new("x89c4"));
        assert_eq!(roll_tail_unported(&state, &args), Some("X-89 Bacterial Weapon"));
        state.player_mut(&a).unwrap().technologies.clear();
        state.faction_marks.insert(crate::combat::ROLL_REPLAY_MARK.to_owned(), String::new());
        assert_eq!(roll_tail_unported(&state, &args), Some("roll replay mark"));
        state.faction_marks.clear();
        let neutral = PlayerId::new(crate::neutral_units::NEUTRAL);
        let args = RollTailArgs { invader: &neutral, ..args };
        assert_eq!(roll_tail_unported(&state, &args), Some("neutral side"));
    }
}
