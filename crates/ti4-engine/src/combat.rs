//! Space combat (LRR 78, with 87 governing Sustain Damage).
//!
//! Ported from the oracle's `engine/combat.py`: `combatants`, `effective_hits_on`,
//! `_roll_combat`, `absorb_hits`, `_offer_sustain`, `_choose_casualty` and the round loop in
//! `resolve`.
//!
//! Choices are asked inline through a [`Table`], as the oracle does, rather than being exposed
//! as a resumable window. The step driver therefore does not run combat yet — the same shape
//! movement had before its driver landed, and recorded as an open finding.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, RelicId, SystemId};
use ti4_model::state::{CombatRollRecord, Feat, FeatOccurrence, GameState, RerollEntry, RerollSet};
use ti4_model::units::Unit;

use crate::choice::{Choice, ChoiceOption, IllegalChoice, Observed, Resolving, Table, Window};
use crate::decision_context::{
    ConstraintKind, DecisionContext, DecisionSource, DecisionTarget, HitCause, HitDetail,
    HitRestriction, OutstandingConstraint,
};
use crate::dice::Dice;
use crate::factions::hooks_combat::{AfbExcess, CombatMoment, HitSite, ProducedHits};
use crate::preview::{Delta, Preview, Quantity, stochastic};
use crate::rng::GameRng;

/// A bound on the round loop, so an unresolvable fight fails loudly instead of hanging.
pub const MAX_ROUNDS: u32 = 50;

/// The choice kind for cancelling a hit with Sustain Damage.
pub const SUSTAIN_KIND: &str = "sustain";
/// The choice kind for assigning a hit to one of your own units.
pub const CASUALTY_KIND: &str = "casualty";

/// What produced hits that are being assigned.  Immunity to unit abilities must not leak onto
/// ordinary combat rolls, which share the casualty machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitOrigin {
    CombatRoll,
    UnitAbility,
}

/// The display cause of hits whose engine `cause` string is given (see `absorb_hits_seeing_with_context`).
pub(crate) fn hit_cause_of(cause: &str) -> HitCause {
    if cause.contains("anti_fighter_barrage") {
        HitCause::AntiFighterBarrage
    } else if cause.contains("space_cannon") {
        HitCause::SpaceCannon
    } else {
        HitCause::Other
    }
}

/// A combat could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CombatError {
    #[error("space combat in {0} did not finish within {MAX_ROUNDS} rounds")]
    Unresolved(SystemId),
    #[error(transparent)]
    IllegalChoice(#[from] IllegalChoice),
    #[error(transparent)]
    Timing(#[from] crate::timing::TimingError),
}

/// How a space combat ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatOutcome {
    /// The last player with ships; `None` when both sides were wiped out, or when the combat
    /// was declared a draw by Skilled Retreat.
    pub winner: Option<PlayerId>,
    /// Rounds fought.
    pub rounds: u32,
}

/// Players with ships in a system, in seating order (78.1).
#[must_use]
pub fn combatants(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> Vec<PlayerId> {
    let types = catalogue(content, sources);
    let mut found = Vec::new();
    for player in &state.seating_order {
        let has_ship = !ships_of(state, content, sources, player, system).is_empty();
        if has_ship {
            found.push(player.clone());
        }
    }
    // Thunder's Edge neutral units, rule 2: "If a player has ships in a space area that contains
    // neutral units, they will resolve a space combat against those units." The neutral owner is
    // never seated (rule 6), so the loop above cannot find it, and every combat against the
    // Fracture's garrison used to end before it began with the player declared the winner.
    let neutral = crate::neutral_units::owner();
    if state.system_state(system).units.iter().any(|unit| {
        unit.owner == neutral
            && types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
    }) {
        found.push(neutral);
    }
    found
}

/// This player's ships in the system, in board order.
#[must_use]
pub fn ships_of(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<Unit> {
    let types = catalogue(content, sources);
    let mut ships: Vec<Unit> = state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
        })
        .cloned()
        .collect();
    ships.extend(
        planet_combatants(state, content, sources, player, system)
            .into_iter()
            .map(|(_, _, unit)| unit),
    );
    ships
}

/// Units standing on a planet that fight this system's space combat as ships, in planet order and
/// then list order, each with its planet and its index in that planet's unit list.
///
/// Two kinds: a Naaz Eidolon Maximum (see [`planetary_maximum_participates`]) and the ground
/// forces a Nekro Alastor chose "to participate in that combat as if they were ships"
/// (`factions::nekro_units::alastor_participants`).
pub(crate) fn planet_combatants(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<(ti4_model::id::PlanetId, usize, Unit)> {
    let maximum = planetary_maximum_participates(state, content, sources, player, system);
    let chosen = crate::factions::nekro_units::alastor_participants(state, player, system);
    let mut found = Vec::new();
    for (planet, units) in &state.system_state(system).planet_units {
        for (index, unit) in units.iter().enumerate() {
            if &unit.owner != player {
                continue;
            }
            let voltron = maximum && unit.type_id.as_str() == "naaz_voltron";
            if voltron || chosen.contains(&(planet.clone(), index)) {
                found.push((planet.clone(), index, unit.clone()));
            }
        }
    }
    found
}

/// Whether a planet-standing combatant can use SUSTAIN DAMAGE against a space-combat hit: a ship
/// by the ship rule, a ground force chosen by the Alastor by its own printed ability.
fn planet_unit_sustains(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    player: &PlayerId,
    unit: &Unit,
    in_combat: bool,
) -> bool {
    let types = catalogue(content, sources);
    let Some(kind) = types.get(unit.type_id.as_str()) else {
        return false;
    };
    if kind.is_ship() {
        return sustains_in_space(
            state, content, sources, system, player, unit, kind, in_combat,
        );
    }
    &unit.owner == player
        && !unit.sustained_damage
        && kind.sustain_damage()
        && !crate::laws::sustain_suppressed(state, kind.base_type())
        && crate::factions::hooks_combat::may_sustain(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet: None,
                unit_type: unit.type_id.as_str(),
                context: "space",
            },
        )
}

/// The label of a planet-standing sustain option: the Maximum keeps its name, anything else is
/// named by its unit type.
fn planet_sustain_label(
    content: &ContentStore,
    sources: SourceSet,
    planet: &str,
    unit: &Unit,
) -> String {
    let place = ti4_content::galaxy::planet(content, planet, sources)
        .and_then(|record| record.name())
        .unwrap_or(planet);
    if unit.type_id.as_str() == "naaz_voltron" {
        let form = if unit.galvanized {
            "galvanized"
        } else {
            "plain"
        };
        format!("sustain damage on Maximum on {place} ({form})")
    } else {
        format!("sustain damage on {} on {place}", unit.type_id)
    }
}

/// The corpus clarification for Eidolon Maximum permits a planet-standing unit to join
/// its owner's fleet in space combat. Cargo alone is not a fleet.
pub(crate) fn planetary_maximum_participates(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> bool {
    let types = catalogue(content, sources);
    state.system_state(system).units.iter().any(|unit| {
        &unit.owner == player
            && types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
    })
}

/// Remove one combat ship from where it actually stands.  Almost every ship is in the space
/// area; Eidolon Maximum is the sole current exception and remains on its planet while joining
/// the battle.
pub(crate) fn remove_combat_ship(state: &mut GameState, system: &SystemId, unit: &Unit) {
    crate::factions::observe(
        state,
        crate::factions::Obs::ShipLost {
            system,
            unit,
        },
    );
    let voltron = unit.type_id.as_str() == "naaz_voltron";
    // An Alastor participant is a ground force standing on its planet: it leaves from there, and
    // the choice that made it a participant shrinks by one.
    let alastor = !voltron
        && crate::factions::nekro_units::alastor_covers(
            state,
            system,
            &unit.owner,
            unit.type_id.as_str(),
        );
    if voltron || alastor {
        let removed_from =
            state
                .system_mut(system)
                .planet_units
                .iter_mut()
                .find_map(|(planet, units)| {
                    let index = units.iter().position(|found| found == unit)?;
                    units.remove(index);
                    Some(planet.clone())
                });
        if let Some(planet) = removed_from {
            if alastor {
                crate::factions::observe(
                    state,
                    crate::factions::Obs::GroundForceLost {
                        system,
                        owner: &unit.owner,
                        planet: &planet,
                        unit: unit.type_id.as_str(),
                    },
                );
            }
            return;
        }
    }
    state.system_mut(system).remove(std::slice::from_ref(unit));
}

/// The value a unit needs to roll, or `None` if it does not fight.
///
/// A printed combat value of zero means "does not fight" rather than "hits on 0", which is why
/// this is an `Option` and not a number with a sentinel.
#[must_use]
pub fn hits_on(content: &ContentStore, sources: SourceSet, unit: &Unit) -> Option<i64> {
    catalogue(content, sources)
        .get(unit.type_id.as_str())
        .and_then(UnitType::combat_hits_on)
}

/// The threshold a unit needs in this combat round after its round-scoped effects.
///
/// Effects store the sequence in which they were created rather than a flag that a later combat
/// exit path must remember to clear. That is load-bearing: a combat can end after barrage,
/// casualties, or retreat, and a flag leaks from whichever exit forgot it. A sequence marker
/// simply stops matching when the next round starts.
#[must_use]
pub fn effective_hits_on(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    unit: &Unit,
) -> Option<i64> {
    effective_from(
        state,
        content,
        sources,
        player,
        unit,
        hits_on(content, sources, unit)?,
    )
}

/// [`effective_hits_on`] for a unit rolling on `threshold` instead of its own printed combat value
/// (a ship that borrows another card's combat value, `hooks_combat::borrowed_stats`).
fn effective_from(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    unit: &Unit,
    threshold: i64,
) -> Option<i64> {
    let morale_is_current = state
        .player(player)
        .is_some_and(|seat| seat.combat_bonus_round == Some(state.combat_round_seq));
    let is_fighter = catalogue(content, sources)
        .get(unit.type_id.as_str())
        .is_some_and(UnitType::is_fighter);
    let shift = crate::combat_effects::space_roll_shift(
        state, content, sources, player, unit, is_fighter,
    );
    Some(threshold - i64::from(morale_is_current) - shift)
}

/// The roll bonus this seat's fighters carry in the current combat round (Fighter Prototype):
/// two per copy held, counted only while the round the cards were played in is the live one.
pub(crate) fn fighter_bonus_now(state: &GameState, player: &PlayerId) -> i64 {
    let prototypes = state.player(player).map_or(0, |seat| {
        2 * i64::try_from(
            seat.fighter_bonus_round
                .iter()
                .filter(|round| **round == state.combat_round_seq)
                .count(),
        )
        .unwrap_or(i64::MAX)
    });
    // Prophecy of Ixth is the same shape and stacks with the card: "+1 to the result of their
    // fighter's combat rolls", for as long as its owner still holds the law.
    prototypes + crate::laws::fighter_combat_bonus(state, player)
}

/// Replace the missed dice in one space-combat batch when Munitions Reserves is current.
///
/// Paying for the ability and opening its reaction window belong to the faction/reaction layer.
/// Combat owns the other half: applying the already-recorded marker at the one place a space
/// combat roll becomes final. The original is already in `Dice` history; `reroll` deliberately
/// records the replacement beside it so a replay preserves both draws.
fn reroll_munitions_misses(
    state: &GameState,
    dice: &mut Dice,
    rng: &mut GameRng,
    player: &PlayerId,
    roll: &crate::dice::Roll,
) -> crate::dice::Roll {
    let active = state
        .player(player)
        .is_some_and(|seat| seat.munitions_round == Some(state.combat_round_seq));
    if !active {
        return roll.clone();
    }
    let misses = roll.missed(None);
    if misses.is_empty() {
        return roll.clone();
    }
    let reason = format!("munitions:{player}");
    dice.reroll(rng, roll, misses, Some(&reason))
}

// -- reroll windows -----------------------------------------------------------
//
// Fire Team, Scramble Frequency, and Aglnlan Oln all act on dice that have been rolled
// but not yet applied. Each roll site stages its rolls in `GameState::reroll_staging`,
// opens the window, and then recomputes the hits from whatever faces remain when the
// window closes.

/// Display only: the whole staged roll, one entry per die in the order the dice are asked about,
/// so a client can show every die and which one a question is about.
#[must_use]
pub fn staged_dice_view(set: &RerollSet) -> Vec<serde_json::Value> {
    set.rolls
        .iter()
        .enumerate()
        .flat_map(|(unit, entry)| {
            entry.faces.iter().enumerate().map(move |(die, face)| {
                let bonus = entry.deltas.get(&die).copied().unwrap_or(0);
                serde_json::json!({
                    "unit": entry.unit,
                    "unit_index": unit,
                    "die": die,
                    "face": face,
                    "bonus": bonus,
                    "hits_on": entry.hits_on,
                    "rerolled": entry.rerolled.contains(&die),
                })
            })
        })
        .collect()
}

/// The staged dice that currently miss, as `(roll entry, die)` positions.
#[must_use]
pub fn staged_misses(set: &RerollSet) -> Vec<(usize, usize)> {
    set.rolls
        .iter()
        .enumerate()
        .flat_map(|(unit, entry)| {
            entry
                .faces
                .iter()
                .enumerate()
                .filter(move |(die, face)| {
                    entry.hits_on.is_some_and(|on| {
                        i64::from(**face) + entry.deltas.get(die).map_or(0, |d| i64::from(*d))
                            < i64::from(on)
                    })
                })
                .map(move |(die, _)| (unit, die))
        })
        .collect()
}

/// Ask the roller, one optional question per die, which dice to reroll.
///
/// A die whose question fails (the decider answered with something not offered) is kept
// as-is, the same way the Letnev Munitions ask degrades, so an optional reroll can never
/// wedge the combat.
#[must_use]
pub fn choose_reroll_dice(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    table: &mut Table,
    player: &PlayerId,
    source: &DecisionSource,
    subtype: &str,
) -> Vec<(usize, usize)> {
    let Some(set) = state.reroll_staging.get(player) else {
        return Vec::new();
    };
    let observed = Observed::new(state, content, sources, galaxy);
    let mut picks = Vec::new();
    let dice = staged_dice_view(set);
    let mut asked = 0usize;
    for (unit, entry) in set.rolls.iter().enumerate() {
        for (die, face) in entry.faces.iter().enumerate() {
            // OBS-008b2: the die's own current hit status (its face plus whatever adjustment --
            // Thalnos's +1 -- still sits on this position), the same per-die arithmetic
            // `RerollEntry::hits` sums over the whole entry.
            let adjusted = i64::from(*face)
                + entry
                    .deltas
                    .get(&die)
                    .map_or(0, |offset| i64::from(*offset));
            let current_hit = entry.hits_on.map(|on| i64::from(adjusted >= i64::from(on)));

            let mut reroll = ChoiceOption::labelled(
                format!("reroll|{unit}:{die}"),
                "reroll_die",
                format!("reroll die {} of {} (shows {})", die + 1, entry.unit, face),
            )
            .with("unit", entry.unit.clone())
            .with("face", i64::from(*face));
            let mut decline = ChoiceOption::decline().with("unit", entry.unit.clone());
            if let (Some(hits_on), Some(current_hit)) = (entry.hits_on, current_hit) {
                reroll = reroll
                    .with("hits_on", i64::from(hits_on))
                    // A fresh d10 draw replaces the face *and* any per-die adjustment it carried
                    // (`RerollEntry::deltas`'s own doc: "the adjustment dies with the die"), so
                    // the redraw is exactly the uniform, unmodified threshold roll
                    // `stochastic::hit_count_preview` already models.
                    .previewed(stochastic::hit_count_preview(1, hits_on, |hits| {
                        vec![Delta::new(Quantity::Hits, current_hit, i64::from(hits))]
                    }));
                // Declining leaves this die exactly as it stands -- a certain, zero-chance fact,
                // not the reroll's distribution.
                decline = decline.previewed(Preview::certain(vec![Delta::new(
                    Quantity::Hits,
                    current_hit,
                    current_hit,
                )]));
            }

            let mut context = DecisionContext::new(
                player.clone(),
                source.clone(),
                subtype,
                state.phase,
                state.round,
            );
            context = context.about(DecisionTarget::System(set.system.clone()));
            let choice = Choice::new(
                player.clone(),
                format!("reroll die {} of {}", die + 1, entry.unit),
                vec![reroll, decline],
            )
            .detailed("dice", dice.clone())
            .detailed("current", asked)
            .contextualized(context);
            asked += 1;
            let Ok(answer) = table.ask_seeing(&choice, &observed) else {
                continue;
            };
            if !answer.is_decline() {
                picks.push((unit, die));
            }
        }
    }
    picks
}

/// Re-draw the chosen dice of the staged set through the game's roller, in place.
///
/// A re-drawn die replaces whatever the previous faces — and any Thalnos +1 sitting on
/// that position — said: the offset is removed with the die it adjusted.
pub fn apply_reroll_dice(
    dice: &mut Dice,
    rng: &mut GameRng,
    set: &mut RerollSet,
    picks: &[(usize, usize)],
    reason: &str,
) {
    for (unit, die) in picks {
        let Some(entry) = set.rolls.get_mut(*unit) else {
            continue;
        };
        if *die >= entry.faces.len() {
            continue;
        }
        let original = crate::dice::Roll {
            reason: set.kind.clone(),
            faces: entry.faces.clone(),
            hits_on: entry.hits_on,
            rerolled: std::collections::BTreeSet::new(),
            by: None,
            system: None,
            outcome: None,
            source: None,
        };
        let again = dice.reroll(rng, &original, [*die], Some(reason));
        entry.faces = again.faces;
        entry.rerolled.insert(*die);
        entry.deltas.remove(die);
    }
}

/// The hits a staged set currently produces from its (possibly rerolled) faces.
#[must_use]
pub fn staged_hits(set: &RerollSet) -> usize {
    set.rolls.iter().map(RerollEntry::hits).sum()
}

/// Open the reroll window for the roll `side` just made: the roller's own commander reroll
/// first (Agnlan Oln), then the relics that watch the dice, then the event other players
/// react to (Scramble Frequency).
///
/// The event goes through the timing resolver so reaction windows actually fire. Called at
/// the space cannon, anti-fighter barrage, and bombardment sites; the caller recomputes
/// the hits from the staging afterwards and clears it.
///
/// # Panics
/// If the commander hook re-enters the staging it just read and finds it gone (a card effect
/// ran in the meantime and removed it), which cannot happen on the call sites: the staged set
/// is only removed by the caller after this returns.
pub fn open_reroll_windows(state: &mut GameState, ctx: &mut Resolving<'_>, side: &PlayerId) {
    reroll_window(state, ctx, side, true);
}

/// The fleet's own window. A fleet roll is not a unit-ability roll, so Aglnlan Oln and
/// Scramble Frequency stand down and no `UNIT_ABILITY_ROLLED` event goes out; the relics
/// that watch *any* die do not.
pub fn open_fleet_reroll_windows(state: &mut GameState, ctx: &mut Resolving<'_>, side: &PlayerId) {
    reroll_window(state, ctx, side, false);
}

/// The window both entry points share: the rerolls the dice's owner may make, the units
/// those rerolls destroy, and (for unit-ability rolls only) the event other players react
/// to, after which the Heart of Ixth may bend whatever faces survive.
fn reroll_window(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    side: &PlayerId,
    unit_ability: bool,
) {
    let Some(set) = state.reroll_staging.get(side) else {
        return;
    };
    let (kind, system) = (set.kind.clone(), set.system.clone());
    let has_dice = set.rolls.iter().any(|entry| !entry.faces.is_empty());
    // Aglnlan Oln: "After you roll dice for a unit ability: You may reroll any of those
    // dice." Ground rolls are not unit ability rolls, so the commander stands down there,
    // and fleet rolls are not either.
    let commander_reroll = unit_ability
        && has_dice
        && kind != "ground"
        && crate::promissory::has_commander_ability(state, side, "jolnarcommander");
    if commander_reroll {
        let picks = choose_reroll_dice(
            state,
            ctx.content,
            ctx.sources,
            None,
            ctx.table,
            side,
            &DecisionSource::Content("jolnarcommander".to_owned()),
            "jolnar_commander_reroll",
        );
        if !picks.is_empty() {
            let set = state.reroll_staging.get_mut(side).expect("checked above");
            apply_reroll_dice(ctx.dice, ctx.rng, set, &picks, "jolnar commander");
        }
    }
    // Munitions Reserves, decided after the dice (rules version 2): the Letnev player has seen the
    // fleet's roll and may now pay 2 trade goods to re-roll any of it.
    if kind == "fleet"
        && has_dice
        && state.rules_version >= ti4_model::state::RULES_MUNITIONS_AFTER_ROLL
    {
        let picks = crate::faction_abilities::munitions_after_roll(state, ctx, side);
        if !picks.is_empty() {
            let set = state.reroll_staging.get_mut(side).expect("checked above");
            apply_reroll_dice(ctx.dice, ctx.rng, set, &picks, &format!("munitions:{side}"));
        }
    }
    // The Crown of Thalnos (relic): "this card's owner may reroll any number of their dice,
    // applying +1 to the results".
    // Both Thalnos cards read "During each combat round", and only two of the five staged roll
    // kinds are one: the fleet roll of a space combat round, and a ground combat round. Space
    // cannon, anti-fighter barrage and bombardment are steps of a tactical action or of an
    // invasion, not rounds -- so neither card reaches them, however tempting the dice look.
    let combat_round = matches!(kind.as_str(), "fleet" | "ground");
    let thalnos_picks = if combat_round && has_dice && holds_crown_relic(state, side) {
        choose_reroll_dice(
            state,
            ctx.content,
            ctx.sources,
            None,
            ctx.table,
            side,
            &DecisionSource::Content("thalnos".to_owned()),
            "crown_of_thalnos_relic_reroll",
        )
    } else {
        Vec::new()
    };
    if !thalnos_picks.is_empty() {
        let set = state.reroll_staging.get_mut(side).expect("checked above");
        apply_reroll_dice(ctx.dice, ctx.rng, set, &thalnos_picks, "thalnos");
        for (unit, die) in &thalnos_picks {
            if let Some(entry) = set.rolls.get_mut(*unit) {
                entry.deltas.insert(*die, 1);
            }
        }
    }
    // The Crown of Thalnos (law): the elected player "may reroll any number of dice" —
    // the destruction clause below does not forgive the misses the reroll leaves.
    let crown_picks = if combat_round && has_dice && owns_crown_law(state, side) {
        choose_reroll_dice(
            state,
            ctx.content,
            ctx.sources,
            None,
            ctx.table,
            side,
            &DecisionSource::Agenda("crown_of_thalnos".to_owned()),
            "crown_of_thalnos_law_reroll",
        )
    } else {
        Vec::new()
    };
    if !crown_picks.is_empty() {
        let set = state.reroll_staging.get_mut(side).expect("checked above");
        apply_reroll_dice(ctx.dice, ctx.rng, set, &crown_picks, "crown of thalnos");
    }
    destroy_reroll_casualties(
        state,
        ctx,
        &kind,
        &system,
        side,
        &thalnos_picks,
        &crown_picks,
    );
    if unit_ability {
        let hits = staged_hits(state.reroll_staging.get(side).expect("checked above"));
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("kind".to_owned(), kind.into());
        payload.insert("player".to_owned(), side.to_string().into());
        payload.insert("system".to_owned(), system.to_string().into());
        payload.insert("hits".to_owned(), i64::try_from(hits).unwrap_or(0).into());
        let _ = ctx.emit(state, "UNIT_ABILITY_ROLLED", payload);
    }
    heart_ixth(state, ctx, side);
}

/// Whether `player` holds the Crown of Thalnos relic.
fn holds_crown_relic(state: &GameState, player: &PlayerId) -> bool {
    crate::relics::holds(state, player, &RelicId::new("thalnos"))
}

/// Whether `player` owns the Crown of Thalnos law — the elected player is its owner.
fn owns_crown_law(state: &GameState, player: &PlayerId) -> bool {
    crate::laws::elected(state, "crown_of_thalnos").is_some_and(|owner| owner == player.as_str())
}

/// The Crown of Thalnos' destruction clauses, judged from the faces every reroll left behind.
///
/// The relic is wide and the +1 is part of the result: "any units that reroll dice but do
/// not produce at least 1 hit are destroyed" looks at the entry's whole total, adjusted.
/// The law is narrow and unforgiving: "they must destroy each of their units that did not
/// produce a hit **with its reroll**" looks only at the dice the law's own reroll re-drew.
/// A die the later Crown re-drew replaced the die the relic blessed, so both clauses read
/// the final faces and agree on what the entry now shows.
fn destroy_reroll_casualties(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    kind: &str,
    system: &SystemId,
    player: &PlayerId,
    thalnos_picks: &[(usize, usize)],
    crown_picks: &[(usize, usize)],
) {
    if thalnos_picks.is_empty() && crown_picks.is_empty() {
        return;
    }
    let set = state
        .reroll_staging
        .get_mut(player)
        .expect("opened from it");
    let doomed: Vec<usize> = set
        .rolls
        .iter()
        .enumerate()
        .filter_map(|(entry, roll)| {
            let thalnos_doomed =
                thalnos_picks.iter().any(|(unit, _)| *unit == entry) && roll.hits() == 0;
            let crown_doomed =
                entry_reroll_hits(roll, crown_picks, entry).is_some_and(|hits| hits == 0);
            (thalnos_doomed || crown_doomed).then_some(entry)
        })
        .collect();
    if doomed.is_empty() {
        return;
    }
    // The casualties are lifted out of the staging before the board is touched: the set
    // is a mutable borrow of the state the removals run through.
    let doomed_units: Vec<RerollEntry> = {
        let mut out = Vec::new();
        for index in doomed.iter().rev() {
            let entry = set.rolls[*index].clone();
            set.rolls.remove(*index);
            out.push(entry);
        }
        out
    };
    for entry in doomed_units {
        remove_casualty_units(state, ctx, kind, system, player, &entry);
    }
}

/// If `picks` re-drew any die of entry `entry`, the hits those dice now show — the plain
/// faces, since a re-drawn die carries no adjustment left to count.
fn entry_reroll_hits(entry: &RerollEntry, picks: &[(usize, usize)], which: usize) -> Option<usize> {
    let dice: Vec<usize> = picks
        .iter()
        .filter(|(unit, _)| *unit == which)
        .map(|(_, die)| *die)
        .collect();
    if dice.is_empty() {
        return None;
    }
    Some(entry.hits_on.map_or(0, |on| {
        dice.iter().filter(|die| entry.faces[**die] >= on).count()
    }))
}

/// Remove the units a reroll casualty stands for.
///
/// A unit carries no identity past its type and owner, so "the unit that made this roll"
/// is "a unit of these types": that many are removed in deterministic order — space first,
/// then the system's planets in order, which is where a planet-mounted cannon sits. The
/// entry's `unit_types` says how many of each type pooled into the roll; a pooled entry
/// that produced no hits loses every unit in the pool, because every one of them rerolled
/// a die and none of them produced a hit. Fleet dice are the one case where the loss is
/// announced: the ships die mid-combat, and cards read "when your last ship is destroyed".
fn remove_casualty_units(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    kind: &str,
    system: &SystemId,
    player: &PlayerId,
    entry: &RerollEntry,
) {
    if entry.unit_types.is_empty() {
        // A relic's own dice, not a unit's (Metali Void Armaments' extra barrage): there
        // is no unit to lose.
        return;
    }
    let announce = kind == "fleet";
    for (type_id, count) in &entry.unit_types {
        let mut due = usize::try_from(*count).unwrap_or(0);
        if due == 0 {
            continue;
        }
        let board = state.system_state(system);
        let matching: Vec<Unit> = board
            .units
            .iter()
            .filter(|unit| unit.owner == *player && unit.type_id.as_str() == type_id)
            .cloned()
            .chain(
                board
                    .planet_units
                    .values()
                    .flatten()
                    .filter(|unit| unit.owner == *player && unit.type_id.as_str() == type_id)
                    .cloned(),
            )
            .take(due)
            .collect();
        for unit in matching {
            due -= 1;
            remove_combat_ship(state, system, &unit);
            if announce {
                announce_ship_destroyed(state, ctx, system, player, &unit);
            }
        }
    }
}

/// Heart of Ixth: "After any die is rolled, you may exhaust this card to add or subtract
/// 1 from its result."
///
/// A relic cannot hold a reaction slot, so the driver asks instead: after the roll and
/// every reroll of it are done, each holder who still has the card ready — in seat order,
/// so two holders may each bend a die of the same roll — gets one question. The die may
/// be any die `side` just rolled, which is what lets a holder bend an opponent's result
/// as well as their own. Each use exhausts the card until the status phase readies it.
// Four lines over the limit, and every one of them is the offer: a die, a sign, and the label that
// tells the holder what the shift would do. Splitting it would put a boundary inside one question.
#[expect(
    clippy::too_many_lines,
    reason = "one offer, built per die and per sign"
)]
fn heart_ixth(state: &mut GameState, ctx: &mut Resolving<'_>, side: &PlayerId) {
    const RELIC: &str = "heartofixth";
    let Some(set) = state.reroll_staging.get(side) else {
        return;
    };
    let dice: Vec<(usize, usize)> = set
        .rolls
        .iter()
        .enumerate()
        .flat_map(|(entry, roll)| {
            roll.faces
                .iter()
                .enumerate()
                .map(move |(die, _face)| (entry, die))
        })
        .collect();
    if dice.is_empty() {
        return;
    }
    let relic = RelicId::new(RELIC);
    // The question is identical for every holder, so it is built once, before any of
    // the answers below takes a mutable borrow of the state.
    let options: Vec<ChoiceOption> = if dice.len() == 1 {
        let (_, die) = dice[0];
        let unit = &set.rolls[0].unit;
        vec![
            ChoiceOption::labelled(
                format!("hfix|0:{die}:add"),
                "heart_ixth",
                format!("add 1 to the die of {unit}"),
            ),
            ChoiceOption::labelled(
                format!("hfix|0:{die}:sub"),
                "heart_ixth",
                format!("subtract 1 from the die of {unit}"),
            ),
        ]
    } else {
        dice.iter()
            .flat_map(|(entry, die)| {
                let unit = &set.rolls[*entry].unit;
                vec![
                    ChoiceOption::labelled(
                        format!("hfix|{entry}:{die}:add"),
                        "heart_ixth",
                        format!("add 1 to die {} of {die} of {unit}", die + 1),
                    ),
                    ChoiceOption::labelled(
                        format!("hfix|{entry}:{die}:sub"),
                        "heart_ixth",
                        format!("subtract 1 from die {} of {die} of {unit}", die + 1),
                    ),
                ]
            })
            .collect()
    };
    // The eligible holders, in seat order, gathered before any answer below takes a
    // mutable borrow of the state.
    let candidates: Vec<PlayerId> = state
        .players
        .iter()
        .map(|seat| seat.id.clone())
        .filter(|id| {
            crate::relics::holds(state, id, &relic)
                && !state
                    .player(id)
                    .is_some_and(|seat| seat.exhausted_relics.contains(&relic))
        })
        .collect();
    for seat_id in candidates {
        let mut options = options.clone();
        options.push(ChoiceOption::decline());
        let choice = Choice::new(
            seat_id.clone(),
            "Heart of Ixth: add or subtract 1 from a die that was just rolled",
            options,
        )
        .contextualized(DecisionContext::new(
            seat_id.clone(),
            DecisionSource::Content("heartofixth".to_owned()),
            "heart_ixth_die_adjust",
            state.phase,
            state.round,
        ));
        let picked = {
            let observed = Observed::new(state, ctx.content, ctx.sources, None);
            match ctx.table.ask_seeing(&choice, &observed) {
                Ok(answer) if !answer.is_decline() => Some(answer.id),
                _ => None,
            }
        };
        let Some(id) = picked else {
            continue;
        };
        // The id is the option's: "hfix|{entry}:{die}:{sign}". Anything else is a decider
        // that answered with something not offered, and the card stays ready for the next
        // roll rather than being spent on a die nobody chose.
        let mut parts = id.split('|');
        let (Some(head), Some(entry), Some(die), Some(sign)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if head != "hfix" {
            continue;
        }
        let (Ok(entry), Ok(die)) = (entry.parse::<usize>(), die.parse::<usize>()) else {
            continue;
        };
        let delta: i8 = match sign {
            "add" => 1,
            "sub" => -1,
            _ => continue,
        };
        let set = state.reroll_staging.get_mut(side).expect("checked above");
        if entry >= set.rolls.len() || die >= set.rolls[entry].faces.len() {
            continue;
        }
        set.rolls[entry].deltas.insert(die, delta);
        if let Some(seat) = state.player_mut(&seat_id) {
            seat.exhausted_relics.insert(relic.clone());
        }
    }
}

/// One combat value's pooled fleet dice: how many, and which unit types sit behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FleetGroup {
    dice: i64,
    types: std::collections::BTreeMap<String, u32>,
}

/// Whether `player` has 0.0.1 among its ships in `system`.
fn forces_non_fighters(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> bool {
    ships_of(state, content, sources, player, system)
        .iter()
        .any(|unit| {
            crate::factions::has_text(
                state,
                player,
                unit.type_id.as_str(),
                crate::factions::FlagshipText::L1z1x,
            )
        })
}

/// Whether a ship's hits are ones 0.0.1 forces onto non-fighters.
fn forced_hull(kind: UnitType<'_>) -> bool {
    matches!(kind.base_type(), "flagship" | "dreadnought")
}

/// A ship's roll group beyond its combat value. A ship whose hits follow a rule the others' do not
/// rolls on its own, so the staged entry says whose dice they were: 1 for J.N.S. Hylarim, 2 for
/// the hulls 0.0.1 binds.
fn roll_tag(kind: UnitType<'_>, jolnar_text: bool, forced: bool) -> u8 {
    if jolnar_text {
        1
    } else if forced && forced_hull(kind) {
        2
    } else {
        0
    }
}

/// A staged fleet roll's hits: those free to land anywhere, and those 0.0.1 forces onto
/// non-fighters. J.N.S. Hylarim's extra hits are counted from the natural faces.
fn fleet_hits(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    set: &RerollSet,
) -> (usize, usize) {
    let types = catalogue(content, sources);
    let binding = forces_non_fighters(state, content, sources, player, system);
    let (mut free, mut forced) = (0, 0);
    for entry in &set.rolls {
        let only = |test: &dyn Fn(&str) -> bool| {
            !entry.unit_types.is_empty() && entry.unit_types.keys().all(|id| test(id))
        };
        let mut hits = entry.hits();
        if only(&|id| crate::factions::has_text(state, player, id, crate::factions::FlagshipText::Jolnar)) {
            hits += 2 * entry.faces.iter().filter(|face| **face >= 9).count();
        }
        if binding && only(&|id| types.get(id).is_some_and(|kind| forced_hull(*kind))) {
            forced += hits;
        } else {
            free += hits;
        }
    }
    (free, forced)
}

/// Duranium Armor: "During each combat round, after you assign hits to your units, repair 1 of
/// your damaged units that did not use SUSTAIN DAMAGE during this combat round."
///
/// `before` lists the types of this side's ships that were already damaged when the round's hits
/// were queued; a unit of such a type still damaged now cannot be one that sustained this round
/// (a sustain damages an undamaged unit). Damaged units of one type are interchangeable, so the
/// first such unit is repaired.
fn duranium_armor(state: &mut GameState, system: &SystemId, player: &PlayerId, before: &[String]) {
    if before.is_empty()
        || !state.player(player).is_some_and(|seat| {
            seat.technologies
                .contains(ti4_model::id::text(&"da"))
        })
    {
        return;
    }
    let Some(unit) = state
        .system_state(system)
        .units
        .iter()
        .find(|unit| {
            &unit.owner == player
                && unit.sustained_damage
                && before.iter().any(|kind| kind == unit.type_id.as_str())
        })
        .cloned()
    else {
        return;
    };
    let mut repaired = unit.clone();
    repaired.sustained_damage = false;
    state.system_mut(system).replace_unit(&unit, repaired);
}

/// Arc Secundus repairs itself at the start of each space combat round.
fn repair_self_repairing(state: &mut GameState, system: &SystemId, player: &PlayerId) {
    let damaged: Vec<Unit> = state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| {
            &unit.owner == player
                && crate::factions::has_text(
                    state,
                    player,
                    unit.type_id.as_str(),
                    crate::factions::FlagshipText::Letnev,
                )
                && unit.sustained_damage
        })
        .cloned()
        .collect();
    for unit in damaged {
        let mut repaired = unit.clone();
        repaired.sustained_damage = false;
        state.system_mut(system).replace_unit(&unit, repaired);
    }
}

/// The first other player with ships in `system`: whom the active player's space cannon
/// offense fires at.
pub fn opponent_with_ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    active: &PlayerId,
    system: &SystemId,
) -> Option<PlayerId> {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .find(|unit| {
            &unit.owner != active
                && types
                    .get(unit.type_id.as_str())
                    .is_some_and(UnitType::is_ship)
        })
        .map(|unit| unit.owner.clone())
}

/// The fleet's rolls, grouped by combat value in the ascending order 78.5b/78.5c fixes.
///
/// Three destroyers are one roll of three dice, not three rolls of one: the number of
/// draws from the seeded stream is part of what a seed reproduces, so rolling them apart
/// would silently renumber every later draw. `BTreeMap` gives the ascending order for free.
fn fleet_groups(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> std::collections::BTreeMap<(i64, u8), FleetGroup> {
    #[cfg(feature = "typed-core")]
    let typed = || {
        let rows = crate::core_combat::CombatView::of(state, content, sources)?.fleet_groups(player, system)?;
        Some(
            rows.into_iter()
                .map(|(value, tag, dice, types)| ((value, tag), FleetGroup { dice, types: types.into_iter().collect() }))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::fleet_groups",
        typed(),
        fleet_groups_text(state, content, sources, player, system)
    )
}

fn fleet_groups_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> std::collections::BTreeMap<(i64, u8), FleetGroup> {
    let types = catalogue(content, sources);
    let forced = forces_non_fighters(state, content, sources, player, system);
    let extra_die = state.player(player).and_then(|seat| {
        (seat.extra_die_round == Some(state.combat_round_seq))
            .then_some(seat.extra_die_unit.as_ref())
            .flatten()
    });
    let mut extra_die_added = false;
    // One ship may roll on another card's combat value this combat (The Cavalry): the first ship
    // equal to the borrower in board order, since identical ships are interchangeable.
    let borrower =
        crate::factions::hooks_combat::borrowed_stats(state, content, sources, player, system)
            .and_then(|borrowed| borrowed.hits_on.map(|value| (borrowed.unit, value)));
    let mut borrower_used = false;
    let mut groups: std::collections::BTreeMap<(i64, u8), FleetGroup> =
        std::collections::BTreeMap::new();
    // (hit number, roll tag, dice, ship type) per ship, grouped once Gravleash Maneuvers is in.
    let mut contributions: Vec<(i64, u8, i64, String)> = Vec::new();
    for (unit_index, unit) in ships_of(state, content, sources, player, system)
        .into_iter()
        .enumerate()
    {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        let lent = borrower
            .as_ref()
            .filter(|(ship, _)| !borrower_used && *ship == unit)
            .map(|(_, value)| *value);
        borrower_used |= lent.is_some();
        let Some(value) = (match lent {
            Some(base) => effective_from(state, content, sources, player, &unit, base),
            None => effective_hits_on(state, content, sources, player, &unit),
        }) else {
            continue;
        };
        let mut dice = crate::factions::unit_dice(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet: None,
                unit_type: unit.type_id.as_str(),
                context: "space",
            },
            kind.combat_dice(),
        );
        if !extra_die_added && extra_die.is_some_and(|selected| selected == &unit.type_id) {
            dice += 1;
            extra_die_added = true;
        }
        // A galvanized unit rolls 1 additional die for its combat rolls (Last Bastion).
        dice += i64::try_from(crate::combat_effects::galvanized_dice(&unit)).unwrap_or(0);
        dice += crate::factions::borrowed_round_agents::extra_die_for(
            state,
            content,
            sources,
            system,
            None,
            player,
            unit.type_id.as_str(),
            unit_index,
            state.combat_round_seq,
        );
        let jolnar_text = crate::factions::has_text(
            state,
            player,
            unit.type_id.as_str(),
            crate::factions::FlagshipText::Jolnar,
        );
        let tag = roll_tag(*kind, jolnar_text, forced);
        contributions.push((value, tag, dice, unit.type_id.to_string()));
    }
    gravleash_maneuvers(state, content, sources, player, system, &mut contributions);
    for (value, tag, dice, type_id) in contributions {
        let group = groups.entry((value, tag)).or_insert(FleetGroup {
            dice: 0,
            types: std::collections::BTreeMap::new(),
        });
        group.dice += dice;
        *group.types.entry(type_id).or_insert(0) += 1;
    }
    groups
}

/// Letnev, Gravleash Maneuvers (`letnevbt`, rules version 16): "Before you roll dice during
/// space combat, apply +X to the results of 1 of your ship's rolls, where X is the number of
/// ship types you have in the combat." The bonus is applied to one die of one ship, kept apart
/// from its group with a hit number lowered by X (never below 1), so the staged roll, the
/// rerolls and the hit count all see it.
///
/// Which ship is the choice the card gives the Letnev player; every die is equal before the
/// roll, so it goes to the die the bonus helps most (a hit number above 10 gains less, a
/// number of 1 nothing) and, among equals, to the first ship in board order.
fn gravleash_maneuvers(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    contributions: &mut Vec<(i64, u8, i64, String)>,
) {
    if state.rules_version < ti4_model::state::RULES_FACTION_EFFECTS
        || !state
            .player(player)
            .and_then(|seat| seat.breakthrough.as_ref())
            .is_some_and(|alias| alias.as_str() == "letnevbt")
    {
        return;
    }
    let kinds: std::collections::BTreeSet<String> =
        ships_of(state, content, sources, player, system)
            .into_iter()
            .map(|unit| unit.type_id.to_string())
            .collect();
    let bonus = i64::try_from(kinds.len()).unwrap_or(0);
    let chance = |hit_on: i64| (11 - hit_on).clamp(0, 10);
    let mut best: Option<(usize, i64)> = None;
    for (index, (value, _, dice, _)) in contributions.iter().enumerate() {
        if *dice <= 0 {
            continue;
        }
        let gain = chance((value - bonus).max(1)) - chance(*value);
        if best.is_none_or(|(_, top)| gain > top) {
            best = Some((index, gain));
        }
    }
    let Some((index, gain)) = best else { return };
    if bonus <= 0 || gain <= 0 {
        return;
    }
    let (value, tag, dice, type_id) = contributions[index].clone();
    if dice > 1 {
        contributions[index].2 = dice - 1;
        contributions.push(((value - bonus).max(1), tag, 1, type_id));
    } else {
        contributions[index].0 = (value - bonus).max(1);
    }
}


/// Dual check of the rolls just made by the fleet loop: the typed roll from the stream positions
/// before them must leave the same dice history and the streams where they stand.
#[allow(clippy::too_many_arguments)]
fn check_fleet_rolls(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &Dice,
    rng: &GameRng,
    before: &Option<crate::rng::RngPositions>,
    start: usize,
    player: &PlayerId,
    system: &SystemId,
    barrage: bool,
) {
    let Some(before) = before else { return };
    let rows: Vec<crate::core_combat::DiceRow> =
        dice.history()[start..].iter().map(crate::core_combat::dice_row).collect();
    let scene = dice.history().get(start).and_then(|r| r.system.clone());
    #[cfg(feature = "typed-core")]
    let typed = || {
        if dice.is_preloaded() {
            return None;
        }
        crate::core_combat::CombatView::of(state, content, sources)?.roll_fleet(
            rng.seed(),
            before,
            player,
            system,
            scene.as_deref(),
            barrage,
        )
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<(Vec<crate::core_combat::DiceRow>, crate::rng::RngPositions)>;
    let after = rng.positions();
    let label = if barrage { "combat::roll_barrage" } else { "combat::roll_fleet" };
    let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, label, typed, || (rows, after));
}

/// The `Core` backend of the fleet's rolls: the typed rules draw them, and the dice history and the
/// streams take the result (the string loop is not run). `None` when the string loop must run: another
/// backend, a pending stream restore, test faces, or something the core does not map.
///
/// Returns each group's final roll as (hit number, faces): the reroll's when there was one.
fn core_fleet_rolls(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    player: &PlayerId,
    system: &SystemId,
) -> Option<Vec<(u32, Vec<u32>)>> {
    if crate::dual::backend(crate::dual::Area::Combat) != crate::dual::Backend::Core
        || rng.sync_pending()
        || dice.is_preloaded()
    {
        return None;
    }
    #[cfg(feature = "typed-core")]
    {
        let positions = rng.positions();
        let (rolls, after) = crate::core_combat::CombatView::of(state, content, sources)?.roll_fleet_raw(
            rng.seed(),
            &positions,
            player,
            system,
        )?;
        let mut finals: Vec<(u32, Vec<u32>)> = Vec::new();
        for roll in rolls {
            rng.observe_randomness();
            let reroll = !roll.rerolled.is_empty();
            let reason = if reroll { format!("munitions:{player}") } else { "space combat".to_owned() };
            dice.record_drawn(
                &reason,
                roll.faces.clone(),
                Some(roll.hits_on),
                roll.rerolled.iter().map(|i| *i as usize).collect(),
                player,
            );
            if reroll {
                if let Some(last) = finals.last_mut() {
                    last.1 = roll.faces;
                }
            } else {
                finals.push((roll.hits_on, roll.faces));
            }
        }
        rng.set_positions(&after);
        Some(finals)
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, content, sources, system);
        None
    }
}

/// The `Core` backend of one side's anti-fighter barrage dice: the typed plan's rolls drawn from the core's
/// copy of the dice stream, in plan order; the streams take the result at once and the string sites then
/// record each entry in the dice history instead of rolling. `None` when the string rolls must run (as
/// [`core_fleet_rolls`]).
fn core_barrage_rolls(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    rng: &mut GameRng,
    dice: &Dice,
    player: &PlayerId,
    system: &SystemId,
) -> Option<std::vec::IntoIter<(u32, Vec<u32>)>> {
    if crate::dual::backend(crate::dual::Area::Combat) != crate::dual::Backend::Core
        || rng.sync_pending()
        || dice.is_preloaded()
    {
        return None;
    }
    #[cfg(feature = "typed-core")]
    {
        let positions = rng.positions();
        let (rolls, after) = crate::core_combat::CombatView::of(state, content, sources)?.roll_barrage_raw(
            rng.seed(),
            &positions,
            player,
            system,
        )?;
        let mut out = Vec::new();
        for roll in rolls {
            rng.observe_randomness();
            out.push((roll.hits_on, roll.faces));
        }
        rng.set_positions(&after);
        Some(out.into_iter())
    }
    #[cfg(not(feature = "typed-core"))]
    {
        let _ = (state, content, sources, rng, player, system);
        None
    }
}

/// One barrage roll of `count` dice needing `value`: the next typed one when `drawn` is on, else a roll.
fn barrage_roll(
    drawn: &mut Option<std::vec::IntoIter<(u32, Vec<u32>)>>,
    dice: &mut Dice,
    rng: &mut GameRng,
    count: usize,
    reason: &str,
    value: u32,
    player: &PlayerId,
) -> crate::dice::Roll {
    match drawn.as_mut() {
        Some(drawn) => {
            let (_, faces) = drawn.next().expect("the typed barrage rolls follow the string plan");
            dice.record_drawn(reason, faces, Some(value), std::collections::BTreeSet::new(), player)
        }
        None => dice.roll_by(rng, count, reason, Some(value), player),
    }
}

/// The stream positions to check a roll against, only when the combat area is on `Dual`.
fn dual_positions(rng: &GameRng) -> Option<crate::rng::RngPositions> {
    (crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual && !rng.sync_pending())
        .then(|| rng.positions())
}

/// Roll one player's fleet and count the hits (78.5).
///
/// Rolled in **ascending order of combat value**, per 78.5b/78.5c, so the sequence a seed
/// reproduces does not depend on the order units happen to sit in the system.
pub fn roll_fleet(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    player: &PlayerId,
    system: &SystemId,
) -> usize {
    if let Some(groups) = core_fleet_rolls(state, content, sources, dice, rng, player, system) {
        return groups.iter().map(|(on, faces)| faces.iter().filter(|f| **f >= *on).count()).sum();
    }
    let (before, start) = (dual_positions(rng), dice.history().len());
    let mut hits = 0;
    for ((value, _), group) in fleet_groups(state, content, sources, player, system) {
        let dice_count = usize::try_from(group.dice).unwrap_or(0);
        if dice_count == 0 {
            continue;
        }
        let threshold = u32::try_from(value).unwrap_or(u32::MAX);
        let roll = dice.roll_by(rng, dice_count, "space combat", Some(threshold), player);
        hits += reroll_munitions_misses(state, dice, rng, player, &roll).hits();
    }
    check_fleet_rolls(state, content, sources, dice, rng, &before, start, player, system, false);
    hits
}

/// Roll one player's fleet, stage the dice, and open the window that may still touch
/// them — the two Crowns of Thalnos and the Heart of Ixth — before any hit lands.
///
/// The staged faces are what the hits are read from: a die rerolled with a +1 is a new
/// result, and a unit the rerolls destroy is gone before the opponent's dice leave the
/// bag. A seed's draw sequence is untouched: a reroll is a fresh draw from the same
/// roller, and a window nobody answers changes nothing.
pub fn roll_fleet_and_open(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
) -> usize {
    roll_fleet_and_open_staged(state, ctx, player, system).0
}

/// [`roll_fleet_and_open`], also handing back the staged dice the hits were read from.
///
/// War Funding acts after *both* sides have rolled, so the dice have to outlive the staging this
/// function clears.
pub fn roll_fleet_and_open_staged(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
) -> (usize, Option<RerollSet>) {
    let (content, sources) = (ctx.content, ctx.sources);
    let mut set = RerollSet {
        kind: "fleet".into(),
        system: system.clone(),
        rolls: Vec::new(),
    };
    let mut drawn =
        core_fleet_rolls(state, content, sources, ctx.dice, ctx.rng, player, system).map(Vec::into_iter);
    let (before, start) = (dual_positions(ctx.rng), ctx.dice.history().len());
    let mut hits = 0;
    for ((value, _), group) in fleet_groups(state, content, sources, player, system) {
        let dice_count = usize::try_from(group.dice).unwrap_or(0);
        if dice_count == 0 {
            continue;
        }
        let threshold = u32::try_from(value).unwrap_or(u32::MAX);
        let roll = if let Some(drawn) = drawn.as_mut() {
            let (hits_on, faces) = drawn.next().expect("the typed rolls follow the string groups");
            crate::dice::Roll {
                reason: "space combat".to_owned(),
                faces,
                hits_on: Some(hits_on),
                rerolled: std::collections::BTreeSet::new(),
                by: Some(player.to_string()),
                system: None,
                outcome: None,
                source: None,
            }
        } else {
            let roll = ctx
                .dice
                .roll_by(ctx.rng, dice_count, "space combat", Some(threshold), player);
            reroll_munitions_misses(state, ctx.dice, ctx.rng, player, &roll)
        };
        hits += roll.hits();
        set.rolls.push(RerollEntry {
            unit: group.types.keys().cloned().collect::<Vec<_>>().join("+"),
            planet: None,
            hits_on: Some(threshold),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: group.types,
        });
    }
    check_fleet_rolls(state, content, sources, ctx.dice, ctx.rng, &before, start, player, system, false);
    if set.rolls.iter().any(|roll| !roll.faces.is_empty()) {
        state.reroll_staging.insert(player.clone(), set);
        state.last_reroll_player = Some(player.clone());
    }
    open_fleet_reroll_windows(state, ctx, player);
    wrath_of_kenara(state, ctx, player, system);
    let staged = state.reroll_staging.remove(player);
    let hits = staged.as_ref().map_or(hits, |set| {
        let (free, forced) = fleet_hits(state, content, sources, player, system, set);
        free + forced
    });
    state.last_reroll_player = None;
    (hits, staged)
}

/// Wrath of Kenara, the Hacan flagship: "After you roll a die during a space combat in this system,
/// you may spend 1 trade good to apply +1 to the result."
///
/// Offered once per roll, after any rerolls, and only for dice the +1 would turn into hits: a +1
/// on a die that already hits, or still misses, buys nothing (22.3). The player picks how many to
/// buy; the lowest-positioned near misses take them, which is indistinguishable in effect.
fn wrath_of_kenara(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
) {
    let has_flagship =
        crate::factions::has_text_in(state, player, system, crate::factions::FlagshipText::Hacan);
    if !has_flagship || crate::supply::potential_goods(state, player) <= 0 {
        return;
    }
    // Xander Alexin Victori III (Keleres): the agent may let commodities be spent as trade goods,
    // offered before the question so its options are generated against what can really be paid.
    let Ok(opened) = crate::supply::open_goods_window(
        state,
        ctx.content,
        ctx.sources,
        None,
        ctx.table,
        player,
        1,
    ) else {
        return;
    };
    wrath_of_kenara_buy(state, ctx, player);
    crate::supply::close_goods_window(state, player, opened);
}

/// The purchase half of [`wrath_of_kenara`], inside its goods window.
fn wrath_of_kenara_buy(state: &mut GameState, ctx: &mut Resolving<'_>, player: &PlayerId) {
    let goods = i32::try_from(crate::supply::spendable_goods(state, player)).unwrap_or(i32::MAX);
    if goods <= 0 {
        return;
    }
    let Some(set) = state.reroll_staging.get(player) else {
        return;
    };
    let near: Vec<(usize, usize)> = set
        .rolls
        .iter()
        .enumerate()
        .flat_map(|(entry, roll)| {
            roll.faces
                .iter()
                .enumerate()
                .filter_map(move |(die, face)| {
                    let on = i64::from(roll.hits_on?);
                    let adjusted = i64::from(*face)
                        + roll.deltas.get(&die).map_or(0, |offset| i64::from(*offset));
                    (adjusted + 1 == on).then_some((entry, die))
                })
        })
        .collect();
    let most = near.len().min(usize::try_from(goods).unwrap_or(0));
    if most == 0 {
        return;
    }
    let mut options: Vec<ChoiceOption> = (1..=most)
        .map(|count| {
            ChoiceOption::labelled(
                format!("kenara|{count}"),
                "trade_goods",
                format!("spend {count} trade good(s): +1 to {count} die/dice, {count} more hit(s)"),
            )
            .with("count", count)
        })
        .collect();
    options.push(ChoiceOption::decline());
    let choice = Choice::new(
        player.clone(),
        "Wrath of Kenara: spend trade goods to add 1 to dice",
        options,
    )
    .contextualized(DecisionContext::new(
        player.clone(),
        DecisionSource::Content("hacan_flagship".to_owned()),
        "wrath_of_kenara_bump",
        state.phase,
        state.round,
    ));
    let Ok(answer) = ctx.ask_seeing(state, &choice) else {
        return;
    };
    let Some(count) = answer
        .id
        .strip_prefix("kenara|")
        .and_then(|count| count.parse::<usize>().ok())
        .filter(|count| *count <= most)
    else {
        return;
    };
    if !crate::supply::spend_goods(state, player, i32::try_from(count).unwrap_or(0)) {
        return;
    }
    if let Some(set) = state.reroll_staging.get_mut(player) {
        for (entry, die) in near.into_iter().take(count) {
            *set.rolls[entry].deltas.entry(die).or_insert(0) += 1;
        }
    }
}

/// The War Funding note this combatant holds from another player, if any.
fn war_funding_note(state: &GameState, holder: &PlayerId) -> Option<String> {
    state
        .promissory_notes
        .iter()
        .find(|(note, held_by)| {
            *held_by == holder
                && crate::promissory::alias_of(note) == "war_funding"
                // By faction, as in `promissory::holder_of`: seats sharing a faction share the id.
                && crate::promissory::owner_of(note)
                    .is_some_and(|name| name != crate::promissory::faction_name(state, holder))
        })
        .map(|(note, _)| note.clone())
}

/// 78.3: anti-fighter barrage — simultaneous, first round only, and hits fall only on fighters.
///
/// Both barrages are rolled **before** either removes a fighter. Rolling and resolving one side
/// at a time would let the first barrage destroy fighters that had already earned their return
/// fire, which is the same simultaneity 78.6 requires of ordinary combat.
///
/// The argument list is long because a combat step needs the state, the corpus it is read
/// against, both halves of the pinned random source, and both sides. Bundling them into a
/// context struct would hide which of them this step actually mutates.
///
/// # Errors
///
/// A Waylay casualty answer that the table rejects, or a dice source that runs dry.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per genuinely distinct input"
)]
pub fn anti_fighter_barrage(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    attacker: &PlayerId,
    defender: &PlayerId,
) -> Result<Vec<(PlayerId, usize)>, CombatError> {
    let occurrence = state.begin_feat_occurrence();
    anti_fighter_barrage_at(
        state, content, sources, dice, rng, system, attacker, defender, occurrence,
    )
    .map(|(resolved, _)| resolved)
}

/// Iconoclast Omega (`naalu_mech_omega`): "Other players cannot use ANTI-FIGHTER BARRAGE against
/// your units in this system." Rules version 17.
fn omega_forbids_barrage(state: &GameState, system: &SystemId, player: &PlayerId) -> bool {
    if state.rules_version < ti4_model::state::RULES_FACTION_BUGFIXES {
        return false;
    }
    let board = state.system_state(system);
    board
        .units
        .iter()
        .chain(board.planet_units.values().flatten())
        .any(|unit| {
            unit.owner != *player
                && unit.type_id.as_str() == "naalu_mech_omega"
                && !crate::factions::crimson::abilities_lost(state, &unit.owner, system)
        })
}

/// Roll one side's anti-fighter barrage and stage the roll for the reroll windows.
///
/// The dice-consuming half: both the combat window and the synchronous wrapper below call
/// this, so a given seed makes the same barrage rolls however the hits are later assigned.
pub fn roll_barrage_side(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    player: &PlayerId,
) -> usize {
    let (before, start) = (dual_positions(rng), dice.history().len());
    let mut plan: Vec<(Option<String>, i64, usize)> = Vec::new();
    if omega_forbids_barrage(state, system, player) {
        check_barrage_plan(state, content, sources, player, system, &plan);
        return 0;
    }
    let mut drawn = core_barrage_rolls(state, content, sources, rng, dice, player, system);
    let types = catalogue(content, sources);
    let mut set = RerollSet {
        kind: "anti_fighter_barrage".into(),
        system: system.clone(),
        rolls: Vec::new(),
    };
    let mut hits = 0;
    // Metali Void Armaments fires once for its holder, not once per ship: the card grants the
    // barrage to the player.
    if let Some((value, count)) = crate::relics::extra_barrage(state, player) {
        plan.push((None, i64::from(value), count));
        let roll = barrage_roll(&mut drawn, dice, rng, count, "anti_fighter_barrage", value, player);
        hits += roll.hits();
        set.rolls.push(RerollEntry {
            unit: "extra barrage".into(),
            planet: None,
            hits_on: Some(value),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            // The relic's own dice, not a unit's: nothing here can be a casualty.
            unit_types: std::collections::BTreeMap::new(),
        });
    }
    // The ship that borrows another card's ANTI-FIGHTER BARRAGE (The Cavalry) fires that instead.
    let borrower =
        crate::factions::hooks_combat::borrowed_stats(state, content, sources, player, system)
            .and_then(|borrowed| borrowed.barrage.map(|barrage| (borrowed.unit, barrage)));
    let mut borrower_used = false;
    // Quietus: units in an active breach beside another player's Quietus have lost every unit
    // ability, ANTI-FIGHTER BARRAGE among them.
    let abilities_lost = crate::factions::crimson::abilities_lost(state, player, system);
    // Hits fall only on fighters (a Waylay widens them to every ship).
    let waylay_in_play = state
        .player(player)
        .and_then(|seat| seat.waylay_barrage_round)
        .is_some_and(|played| played == state.combat_round_seq);
    let no_fighter_target = !waylay_in_play
        && state
            .system_state(system)
            .units
            .iter()
            .map(|unit| &unit.owner)
            .filter(|owner| *owner != player)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .all(|owner| fighters_of(state, content, sources, owner, system) == 0);
    for unit in ships_of(state, content, sources, player, system) {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        if abilities_lost {
            continue;
        }
        let lent = borrower
            .as_ref()
            .filter(|(ship, _)| !borrower_used && *ship == unit)
            .map(|(_, barrage)| *barrage);
        borrower_used |= lent.is_some();
        let Some(value) = lent.map(|(value, _)| value).or_else(|| kind.afb_hits_on()) else {
            continue;
        };
        let count =
            usize::try_from(lent.map_or_else(|| kind.afb_dice(), |(_, dice)| dice)).unwrap_or(0);
        if count == 0 {
            continue;
        }
        // A galvanized unit rolls 1 additional die for its unit abilities (Last Bastion).
        let count = count + crate::combat_effects::galvanized_dice(&unit);
        let value = crate::combat_effects::barrage_value(
            state,
            content,
            sources,
            player,
            system,
            kind.is_fighter(),
            value,
        );
        plan.push((Some(unit.type_id.to_string()), value, count));
        let roll = barrage_roll(
            &mut drawn,
            dice,
            rng,
            count,
            "anti-fighter barrage",
            u32::try_from(value).unwrap_or(u32::MAX),
            player,
        );
        hits += roll.hits();
        // Presentation only: a barrage against a fleet without fighters still rolls (the dice
        // are drawn as ever), but the log says its hits have nothing to hit.
        if no_fighter_target {
            let hit_count = roll.hits();
            let outcome = match hit_count {
                0 => "no hits".to_owned(),
                1 => "1 hit, but no enemy fighters to hit".to_owned(),
                n => format!("{n} hits, but no enemy fighters to hit"),
            };
            dice.annotate_last(Some(system.as_str()), Some(&outcome));
        }
        set.rolls.push(RerollEntry {
            unit: unit.type_id.to_string(),
            planet: None,
            hits_on: Some(u32::try_from(value).unwrap_or(u32::MAX)),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: std::iter::once((unit.type_id.to_string(), 1)).collect(),
        });
    }
    check_barrage_plan(state, content, sources, player, system, &plan);
    check_fleet_rolls(state, content, sources, dice, rng, &before, start, player, system, true);
    if set.rolls.iter().any(|roll| !roll.faces.is_empty()) {
        state.reroll_staging.insert(player.clone(), set);
        state.last_reroll_player = Some(player.clone());
    }
    hits
}

/// Dual check of one side's barrage: the typed plan must list the same rolls (ship, hit number, dice).
fn check_barrage_plan(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    plan: &[(Option<String>, i64, usize)],
) {
    if crate::dual::backend(crate::dual::Area::Combat) != crate::dual::Backend::Dual {
        return;
    }
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::CombatView::of(state, content, sources)?.barrage_plan(player, system);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<Vec<(Option<String>, i64, usize)>>;
    let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::barrage_plan", typed, || plan.to_vec());
}

/// The barrage hits that take fighters in unit order, and their announcements, as a core window on
/// `driver=dual` (`rules::combat::destroy_fighters`).
fn destroy_fighters_windowed(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    ctx: &mut Resolving<'_>,
    target: &PlayerId,
    system: &SystemId,
    hits: usize,
) -> Result<(), CombatError> {
    const CAUSE: &str = "unit_ability:anti_fighter_barrage";
    let failed = std::cell::Cell::new(false);
    let mut outcome = None;
    let args = crate::dual_window::FightersArgs {
        target,
        system,
        hits,
        cause: CAUSE,
        staging: crate::supply::staging_enabled(state),
        announce: ctx.timing.is_some(),
    };
    crate::dual_window!(
        crate::dual::Area::Driver,
        "combat::destroy_fighters",
        state,
        |s: &mut GameState| {
            let result = (|| {
                let destroyed = destroy_fighters(s, content, sources, target, system, hits);
                if crate::supply::staging_enabled(s) {
                    for fighter in destroyed {
                        try_announce_ship_destroyed_type(s, ctx, system, target, &fighter.type_id, CAUSE, true)?;
                    }
                }
                Ok(())
            })();
            failed.set(result.is_err());
            outcome = Some(result);
            crate::dual::Run::new(!failed.get())
        },
        |s: &mut GameState, script: &crate::dual_window::Recording| {
            if failed.get() {
                return None;
            }
            crate::dual_window::destroy_fighters(s, sources, args, script)
        }
    );
    outcome.unwrap_or(Ok(()))
}

/// Resolve both barrages: remove the target's fighters for each side's (possibly rerolled)
/// hits, and keep the event-scoped feats that creates.
fn apply_barrage(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    attacker: &PlayerId,
    defender: &PlayerId,
    results: &[(PlayerId, usize)],
    occurrence: FeatOccurrence,
) -> Result<Vec<PlayerId>, CombatError> {
    let mut feat_players = Vec::new();
    for (player, hits) in results {
        let target = if player == attacker {
            defender
        } else {
            attacker
        };
        // Fight with Precision asks for the last fighter specifically, and specifically during
        // this step, so the count is taken either side of the removal rather than after the
        // combat: by then ordinary combat rounds have taken fighters too, and nothing would say
        // which step emptied the system.
        let before = fighters_of(state, content, sources, target, system);
        // "Before a hit would be assigned" (Titans' Tellurian): the barrage's hits are a batch.
        // The window opens only when something can be hit: the target has fighters, or a Waylay
        // widens the hits to every ship.
        let waylay_in_play = state
            .player(player)
            .and_then(|seat| seat.waylay_barrage_round)
            .is_some_and(|played| played == state.combat_round_seq);
        let hits = &if *hits > 0 && (before > 0 || waylay_in_play) {
            let payload = std::collections::BTreeMap::from([
                ("system".to_owned(), system.to_string().into()),
                ("player".to_owned(), target.to_string().into()),
                ("gunner".to_owned(), player.to_string().into()),
                ("hits".to_owned(), i64::try_from(*hits).unwrap_or(0).into()),
            ]);
            open_hits_window(
                state,
                ctx,
                "ANTI_FIGHTER_BARRAGE_HITS",
                payload,
                target,
                *hits,
            )
        } else {
            *hits
        };
        if *hits > 0 {
            // Waylay ("hits from this roll are produced against all ships (not just
            // fighters)", played before this side's barrage roll): the hits are assigned
            // like any other combat hits — SUSTAIN DAMAGE first, then the target's owner
            // chooses the losses — instead of taking fighters in unit order.
            let waylay = state
                .player(player)
                .and_then(|seat| seat.waylay_barrage_round)
                .is_some_and(|played| played == state.combat_round_seq);
            if waylay {
                absorb_hits_seeing_with_context(
                    state,
                    content,
                    sources,
                    galaxy,
                    ctx,
                    target,
                    system,
                    player,
                    *hits,
                    false,
                    HitOrigin::UnitAbility,
                    "unit_ability:anti_fighter_barrage",
                    true,
                )?;
            } else {
                destroy_fighters_windowed(state, content, sources, ctx, target, system, *hits)?;
                // Hits beyond the target's fighters are discarded (15.2a) unless a faction's
                // ability redirects them (`hooks_combat::afb_excess`, Raid Formation).
                let excess = hits.saturating_sub(before);
                if excess > 0 {
                    let site = AfbExcess {
                        producer: player,
                        target,
                        system,
                        excess,
                    };
                    // On `driver=dual` Raid Formation also runs as a core window (`rules::combat::raid_formation`).
                    let failed = std::cell::Cell::new(false);
                    let mut outcome = None;
                    crate::dual_window!(
                        crate::dual::Area::Driver,
                        "combat::raid_formation",
                        state,
                        |s: &mut GameState| {
                            let made = with_timing(s, ctx, |timing| {
                                crate::factions::hooks_combat::afb_excess(timing, &site)
                            });
                            announce_staged_destructions(s, ctx);
                            failed.set(made.is_err());
                            outcome = Some(made);
                            crate::dual::Run::new(!failed.get())
                        },
                        |s: &mut GameState, script: &crate::dual_window::Recording| {
                            if failed.get() {
                                return None;
                            }
                            let args = crate::dual_window::RaidArgs { producer: player, target, system, excess };
                            crate::dual_window::raid_formation(s, sources, args, script)
                        }
                    );
                    if let Some(made) = outcome {
                        made?;
                    }
                }
            }
        }
        if before > 0 && fighters_of(state, content, sources, target, system) == 0 {
            state.record_event_feat(player, Feat::BarrageTookTheLastFighters, occurrence);
            feat_players.push(player.clone());
        }
    }
    Ok(feat_players)
}

/// Resolve anti-fighter barrage without the reroll windows: roll both sides and apply. The
/// synchronous API the tests and standalone callers use; the combat window instead rolls the
/// same [`roll_barrage_side`] with the windows opened in between, so the two paths cannot
/// disagree about the dice. The staging left behind is stale by construction and is cleared
/// at the start of the next windowed roll.
///
/// Without the caller's timing machinery there is no resolver to ask a Waylay casualty
/// choice to, so the synchronous path answers its own questions with the table's default
/// decider. The stale-staging and dice guarantees above are unaffected.
#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per genuinely distinct input"
)]
#[allow(
    clippy::type_complexity,
    reason = "hits and feat players are one result pair, not a new type"
)]
fn anti_fighter_barrage_at(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    attacker: &PlayerId,
    defender: &PlayerId,
    occurrence: FeatOccurrence,
) -> Result<(Vec<(PlayerId, usize)>, Vec<PlayerId>), CombatError> {
    let mut pending = Vec::new();
    for player in [attacker, defender] {
        let hits = roll_barrage_side(state, content, sources, dice, rng, system, player);
        if hits > 0 {
            pending.push((player.clone(), hits));
        }
    }
    let resolved = pending.clone();
    let mut table = Table::new();
    let mut ctx = Resolving {
        content,
        sources,
        dice,
        rng,
        table: &mut table,
        timing: None,
    };
    let feat_players = apply_barrage(
        state, content, sources, None, &mut ctx, system, attacker, defender, &pending, occurrence,
    )?;
    Ok((resolved, feat_players))
}

/// Fighters this player has in the space area of a system.
fn fighters_of(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> usize {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_fighter)
        })
        .count()
}

/// Non-fighter ships this player has in the space area of a system.
pub fn non_fighter_ships_of(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> usize {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.is_ship() && !kind.is_fighter())
        })
        .count()
}

/// War suns and flagships this player has in the space area of a system.
///
/// The two ships Destroy Their Greatest Ship names. Read by base type rather than by alias so a
/// faction's upgraded flagship counts as the flagship it is.
fn capital_ships_of(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> usize {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == player)
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| matches!(kind.base_type(), "warsun" | "flagship"))
        })
        .count()
}

/// Remove up to `hits` fighters, and nothing else. Excess hits have no effect (15.2a).
///
/// The owner is not asked which fighter dies: fighters carry no damage and no other
/// distinguishing state, so every choice would be between identical options.
fn destroy_fighters(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    hits: usize,
) -> Vec<Unit> {
    let types = catalogue(content, sources);
    let mut destroyed = Vec::new();
    for _ in 0..hits {
        let fighter = state
            .system_state(system)
            .units
            .iter()
            .find(|unit| {
                &unit.owner == player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_fighter)
            })
            .cloned();
        let Some(fighter) = fighter else {
            break;
        };
        remove_combat_ship(state, system, &fighter);
        destroyed.push(fighter);
    }
    destroyed
}

/// Units in the active system fire on the active player's ships, before combat.
///
/// Guns come from both the space area and every planet in the system: a PDS sits on a planet and
/// shoots into space, which is the ordinary case, while some faction units carry the ability in
/// the space area itself.
///
/// Returns the hits produced per firing player, for the caller to absorb. They are kept separate
/// from combat hits because the two are answered by different cards, which is the distinction
/// [`absorb_hits`] exists to preserve.
/// Guns in *adjacent* systems that may fire into this one.
///
/// PDS II ("you may use this unit's SPACE CANNON against ships that are in adjacent systems") and
/// Xxcha's Indomitus mech, which prints the same clause. Both stand on planets, so only planets are
/// searched; and both belong to somebody other than the active player, since space cannon offence
/// fires at the seat that activated the system.
/// Does this unit's own card let its SPACE CANNON reach an adjacent system?
///
/// Read from the unit's ability text rather than matched against a list of ids. The list this
/// replaces held `["pds2", "xxcha_mech"]` and had gone stale: four units in the corpus carry the
/// clause -- PDS II, Hel-Titan II, Xxcha's Indomitus mech and the Xxcha flagship Loncara Ssodu --
/// and two of them never fired. A list is exactly the wrong shape for "every unit whose card says
/// this", because adding the fifth unit means editing code that looks unrelated to the content
/// change. `every_unit_that_reaches_an_adjacent_system_is_found` pins the derived set so a reworded
/// card fails a test instead of silently disarming a gun.
/// The unit ids whose card lets their SPACE CANNON reach an adjacent system (the `reaching` list the core offense is given).
pub(crate) fn reaching_units(content: &ContentStore, sources: SourceSet) -> Vec<String> {
    catalogue(content, sources).iter().filter(|(_, kind)| reaches_adjacent(**kind)).map(|(id, _)| (*id).to_owned()).collect()
}

fn reaches_adjacent(kind: ti4_content::units::UnitType<'_>) -> bool {
    let Some(ability) = kind.record().text("ability") else {
        return false;
    };
    let ability = ability.to_ascii_lowercase();
    // Both phrasings in the corpus: "against ships that are in adjacent systems" (PDS II, the mech
    // and the flagship) and "against ships that are adjacent to this unit's systems" (Hel-Titan II).
    ability.contains("space cannon against ships that are") && ability.contains("adjacent")
}

/// Guns in neighbouring systems whose cards let them fire into this one.
///
/// Scans the space area as well as the planets. The planet-only version could not see the Xxcha
/// flagship, which is a ship and never stands on a planet, so that gun was unreachable even before
/// the id list is considered. `fires` says whose guns count.
fn reaching_guns_by(
    state: &GameState,
    types: &std::collections::BTreeMap<&str, ti4_content::units::UnitType<'_>>,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    system: &SystemId,
    fires: impl Fn(&PlayerId) -> bool,
    silenced_in_space: impl Fn(&Unit) -> bool,
) -> Vec<Unit> {
    let Some(galaxy) = galaxy else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for neighbour in galaxy.adjacent(system.as_str()) {
        let board = state.system_state(&SystemId::new(neighbour));
        // Miniaturization: a Ral Nel structure in the space area has no unit abilities.
        let standing = board
            .planet_units
            .values()
            .flat_map(|units| units.iter())
            .chain(board.units.iter().filter(|unit| !silenced_in_space(unit)));
        let here = SystemId::new(neighbour);
        found.extend(
            standing
                .filter(|unit| fires(&unit.owner))
                // Quietus: a gun in an active breach beside another player's Quietus has lost it.
                .filter(|unit| !crate::factions::crimson::abilities_lost(state, &unit.owner, &here))
                .filter(|unit| {
                    types
                        .get(unit.type_id.as_str())
                        .is_some_and(|kind| reaches_adjacent(*kind))
                })
                .cloned(),
        );
    }
    found
}

/// Faction bars (Argent flagship: "Other players cannot use SPACE CANNON against your ships in
/// this system"): whether `owner`'s gun may not fire. The active player's own guns fire at the
/// `opponent`, every other gun at the active player. Applies to adjacent-reaching guns too.
fn cannon_barred(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
    active: &PlayerId,
    opponent: Option<&PlayerId>,
    system: &SystemId,
) -> bool {
    let target = if owner == active {
        opponent
    } else {
        Some(active)
    };
    target.is_some_and(|target| {
        crate::factions::hooks_combat::space_cannon_barred(
            state, content, sources, owner, target, system,
        )
    })
}

/// The guns that roll in the space cannon offense of `system`, when `active` activates it.
pub struct CannonGuns {
    /// The units, in the order in which they roll.
    pub units: Vec<Unit>,
    /// SPACE CANNON of an attachment: the owner, the planet, the value and the number of dice.
    pub attachments: Vec<(PlayerId, ti4_model::id::PlanetId, u32, usize)>,
}

/// Who may fire in the space cannon offense of `system`. It rolls nothing and changes nothing:
/// [`space_cannon_offense`] rolls for these guns, and a client asks it before the activation.
#[must_use]
pub fn space_cannon_guns(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    active: &PlayerId,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> CannonGuns {
    // Solar Flare: during the named tactical action, other players cannot use SPACE CANNON
    // against the active player's ships. Every other player's gun fires at the active player, so
    // the card silences all of them; the active player's own guns are untouched. The marker is
    // activation-scoped, like the card's "this tactical action" wording.
    // Myru Vos (Firmament agent) silences SPACE CANNON against the ships that moved the same way.
    let solar_flare = state
        .player(active)
        .is_some_and(|seat| seat.solar_flare.contains(&state.activation_seq))
        || crate::factions::firmament::space_cannon_silenced(
            state, content, sources, active, system,
        );
    let types = catalogue(content, sources);
    let board = state.system_state(system);
    // The active player's guns fire too (as ti4calc has it; user ruling 2026-09-17), at the ships
    // of the player being attacked. With no such ships there is nothing to roll at.
    let opponent = opponent_with_ships(state, content, sources, active, system);
    let targets = opponent.is_some();
    let barred = |owner: &PlayerId| {
        cannon_barred(
            state,
            content,
            sources,
            owner,
            active,
            opponent.as_ref(),
            system,
        )
    };
    let may_fire = |owner: &PlayerId| {
        if owner == active {
            targets && !barred(owner)
        } else {
            !solar_flare && !barred(owner)
        }
    };

    // Quietus: a unit in an active breach beside another player's Quietus has lost SPACE CANNON.
    let may_fire_here = |owner: &PlayerId| {
        may_fire(owner) && !crate::factions::crimson::abilities_lost(state, owner, system)
    };
    // Miniaturization: a Ral Nel structure in the space area cannot use its own SPACE CANNON.
    let mut guns: Vec<Unit> = board
        .units
        .iter()
        .filter(|unit| {
            may_fire_here(&unit.owner)
                && !crate::factions::ralnel::silenced_in_space(state, content, sources, unit)
        })
        .cloned()
        .collect();
    for planet in board.planet_units.keys() {
        guns.extend(
            board
                .on_planet(planet)
                .iter()
                .filter(|unit| may_fire_here(&unit.owner))
                .cloned(),
        );
    }

    // PDS II and Xxcha's Indomitus mech both read "You may use this unit's SPACE CANNON against
    // ships that are in adjacent systems." Two cards, one clause, and neither reached the active
    // system before: `space_cannon_offense` read only the system being activated, so an upgraded
    // PDS next door -- a technology every faction can research -- never fired at all.
    guns.extend(reaching_guns_by(
        state,
        &types,
        galaxy,
        system,
        &may_fire,
        |unit| crate::factions::ralnel::silenced_in_space(state, content, sources, unit),
    ));
    // Linkship I / II: the SPACE CANNON of a Ral Nel structure in the space area, borrowed.
    guns.extend(crate::factions::ralnel::linkship_guns(
        state,
        content,
        sources,
        system,
        &may_fire_here,
    ));
    // SPACE CANNON an attachment gives its planet "as if it were a unit" (Titans' Geoform).
    // Disable and Plasma Scoring do not apply to these dice: the attachment is not a PDS unit.
    let attachment_guns: Vec<(PlayerId, ti4_model::id::PlanetId, u32, usize)> = board
        .planet_control
        .keys()
        .flat_map(|planet| {
            crate::planets::attachment_cannons(state, content, system, planet)
                .into_iter()
                .map(move |(owner, value, count)| (owner, planet.clone(), value, count))
        })
        .filter(|(owner, ..)| may_fire(owner))
        .collect();
    if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
        #[cfg(feature = "typed-core")]
        let typed = || {
            let reaching: Vec<String> =
                types.iter().filter(|(_, kind)| reaches_adjacent(**kind)).map(|(id, _)| (*id).to_owned()).collect();
            crate::core_combat::CombatView::of(state, content, sources)?.unit_cannon_guns(system, active, galaxy, &reaching)
        };
        #[cfg(not(feature = "typed-core"))]
        let typed = || None::<Vec<String>>;
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::unit_cannon_guns", typed, || {
            guns.iter().map(|unit| unit.type_id.to_string()).collect::<Vec<_>>()
        });
    }
    if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
        #[cfg(feature = "typed-core")]
        let typed = || crate::core_combat::CombatView::of(state, content, sources)?.attachment_cannon_guns(system, active);
        #[cfg(not(feature = "typed-core"))]
        let typed = || None::<Vec<(String, String, u32, usize)>>;
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::attachment_cannon_guns", typed, || {
            attachment_guns.iter().map(|(who, planet, value, count)| (who.to_string(), planet.to_string(), *value, *count)).collect::<Vec<_>>()
        });
    }
    CannonGuns {
        units: guns,
        attachments: attachment_guns,
    }
}

/// Where each unit that could fire in the space cannon offense of `system` stands: the planet
/// (none in space) and the system when it is not `system` itself. Presentation only.
fn gun_places(
    state: &GameState,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    system: &SystemId,
) -> Vec<(Unit, Option<String>, Option<String>)> {
    let mut found = Vec::new();
    let mut scan = |id: &SystemId, from: Option<String>| {
        let board = state.system_state(id);
        found.extend(board.units.iter().map(|u| (u.clone(), None, from.clone())));
        for (planet, units) in &board.planet_units {
            found.extend(
                units
                    .iter()
                    .map(|u| (u.clone(), Some(planet.to_string()), from.clone())),
            );
        }
    };
    scan(system, None);
    if let Some(galaxy) = galaxy {
        for neighbour in galaxy.adjacent(system.as_str()) {
            scan(&SystemId::new(neighbour), Some(neighbour.to_owned()));
        }
    }
    found
}

/// Takes the place of `unit` out of `places`, so two identical guns get two places.
fn claim_place(
    places: &mut Vec<(Unit, Option<String>, Option<String>)>,
    unit: &Unit,
) -> crate::dice::RollSource {
    let at = places.iter().position(|(candidate, ..)| candidate == unit);
    let (planet, system) = at.map_or((None, None), |at| {
        let (_, planet, system) = places.remove(at);
        (planet, system)
    });
    crate::dice::RollSource {
        unit: unit.type_id.to_string(),
        planet,
        system,
    }
}

pub fn space_cannon_offense(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    dice: &mut Dice,
    rng: &mut GameRng,
    system: &SystemId,
    active: &PlayerId,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Vec<(PlayerId, usize, Vec<RerollEntry>)> {
    let CannonGuns {
        units: guns,
        attachments: attachment_guns,
    } = space_cannon_guns(state, content, sources, system, active, galaxy);
    let types = catalogue(content, sources);
    let mut places = gun_places(state, galaxy, system);
    // The player the active player's guns fire at (Assail reads it below).
    let opponent = opponent_with_ships(state, content, sources, active, system);

    let mut by_player: std::collections::BTreeMap<PlayerId, (usize, Vec<RerollEntry>)> =
        std::collections::BTreeMap::new();
    // Plasma Scoring: one of a player's firing units rolls a die more. The player picks which; the
    // engine gives it to the unit that hits most easily, the pick nobody would refuse.
    let mut plasma = plasma_picks(
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
    let gun_list: Vec<(PlayerId, String)> =
        guns.iter().map(|unit| (unit.owner.clone(), unit.type_id.to_string())).collect();
    let mut plan: Vec<(String, String, i64, usize)> = Vec::new();
    for unit in guns {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        let Some(value) = kind.space_cannon_hits_on() else {
            continue;
        };
        // Disable strips SPACE CANNON from opponents' PDS during the invasion. In the driven
        // game the cannon step precedes the invasion window, so this binds only callers that
        // fire the guns after the card has played; it keeps the two PDS effects of the card
        // in lockstep either way.
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
        let count = count + take_plasma(&mut plasma, &unit.owner, value);
        // A galvanized unit rolls 1 additional die for its unit abilities (Last Bastion).
        let count = count + crate::combat_effects::galvanized_dice(&unit);
        // Assail: +1 to the gunner's unit ability rolls against a puppeted player (the active
        // player's guns fire at the opponent, every other gun at the active player).
        let target = if unit.owner == *active {
            opponent.clone()
        } else {
            Some(active.clone())
        };
        let value = (value
            - crate::factions::firmament_plots::assail_against(state, &unit.owner, target.iter()))
        .max(1);
        // Antimass Deflectors: -1 to other players' SPACE CANNON rolls against the owner's units,
        // the same as asking 1 more of the die.
        let value = value
            + i64::from(
                state.rules_version >= ti4_model::state::RULES_TECH_EFFECTS
                    && target.as_ref().is_some_and(|victim| {
                        *victim != unit.owner
                            && crate::technology::owns_technology(state, victim, "amd")
                    }),
            );
        plan.push((unit.owner.to_string(), unit.type_id.to_string(), value, count));
        let roll = dice.roll_by(
            rng,
            count,
            "space cannon",
            Some(u32::try_from(value).unwrap_or(u32::MAX)),
            &unit.owner,
        );
        dice.annotate_source(claim_place(&mut places, &unit));
        let entry = RerollEntry {
            unit: unit.type_id.to_string(),
            planet: None,
            hits_on: Some(u32::try_from(value).unwrap_or(u32::MAX)),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: std::iter::once((unit.type_id.to_string(), 1)).collect(),
        };
        let slot = by_player
            .entry(unit.owner.clone())
            .or_insert_with(|| (0, Vec::new()));
        slot.0 += entry.hits();
        slot.1.push(entry);
    }
    if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
        #[cfg(feature = "typed-core")]
        let typed = || {
            crate::core_combat::CombatView::of(state, content, sources)?.cannon_plan(
                system,
                active,
                opponent.as_ref(),
                &gun_list,
            )
        };
        #[cfg(not(feature = "typed-core"))]
        let typed = || None::<Vec<(String, String, i64, usize)>>;
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::cannon_plan", typed, || plan.clone());
    }
    for (owner, planet, value, count) in attachment_guns {
        let roll = dice.roll_by(rng, count, "space cannon", Some(value), &owner);
        dice.annotate_source(crate::dice::RollSource {
            unit: "attachment".to_owned(),
            planet: Some(planet.to_string()),
            system: None,
        });
        let entry = RerollEntry {
            unit: format!("planet cannon {planet}"),
            planet: None,
            hits_on: Some(value),
            faces: roll.faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            // An attachment's own dice, not a unit's: nothing here can be a casualty.
            unit_types: std::collections::BTreeMap::new(),
        };
        let slot = by_player.entry(owner).or_insert_with(|| (0, Vec::new()));
        slot.0 += entry.hits();
        slot.1.push(entry);
    }
    // Stage each gunner's rolls for the reroll windows; the caller opens one window per
    // gunner and names them with `last_reroll_player`.
    for (player, (_, rolls)) in &by_player {
        if rolls.iter().any(|roll| !roll.faces.is_empty()) {
            state.reroll_staging.insert(
                player.clone(),
                RerollSet {
                    kind: "space_cannon".into(),
                    system: system.clone(),
                    rolls: rolls.clone(),
                },
            );
        }
    }
    by_player
        .into_iter()
        .filter(|(_, (hits, _))| *hits > 0)
        .map(|(player, (hits, rolls))| (player, hits, rolls))
        .collect()
}

/// Hits a card has let this seat cancel in the current combat round (Shields Holding).
///
/// Consumed as they are used, so "cancel up to 2 hits" is two hits across the round rather than two
/// per assignment. Scoped to the round the card names.
#[must_use]
pub fn cancellable_hits(state: &GameState, player: &PlayerId) -> usize {
    state
        .player(player)
        .and_then(|seat| seat.cancel_hits_round)
        .filter(|(round, _)| *round == state.combat_round_seq)
        .map_or(0, |(_, hits)| hits)
}

/// Grant this seat cancellable hits for the current combat round.
pub fn grant_hit_cancellation(state: &mut GameState, player: &PlayerId, hits: usize) {
    let round = state.combat_round_seq;
    if let Some(seat) = state.player_mut(player) {
        let running = match seat.cancel_hits_round {
            Some((held, had)) if held == round => had,
            _ => 0,
        };
        seat.cancel_hits_round = Some((round, running + hits));
    }
}

/// Open a "before a hit would be assigned" window for `victim` and spend what a card played in it
/// cancelled, returning the hits that remain.
///
/// Only the cancellations granted *during* the window (the growth of the round-scoped pool while
/// the event resolves) are spent here, capped at `hits`; a cancellation granted earlier (Shields
/// Holding played before this batch) stays in the pool for the hit assignment that spends it.
/// The space-combat hit queue differs: after its HITS_TO_ASSIGN window it spends the whole pool
/// (earlier grants included) before the sustain offer, the order a round-scoped grant implies.
pub(crate) fn open_hits_window(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    event: &'static str,
    payload: std::collections::BTreeMap<String, serde_json::Value>,
    victim: &PlayerId,
    hits: usize,
) -> usize {
    if hits == 0 {
        return 0;
    }
    let before = cancellable_hits(state, victim);
    let _ = ctx.emit(state, event, payload);
    let gained = cancellable_hits(state, victim).saturating_sub(before);
    hits - spend_cancellations(state, victim, gained.min(hits))
}

/// Spend up to `wanted` of this seat's cancellations, returning how many were spent.
pub(crate) fn spend_cancellations(
    state: &mut GameState,
    player: &PlayerId,
    wanted: usize,
) -> usize {
    let available = cancellable_hits(state, player);
    let spent = available.min(wanted);
    if spent > 0 {
        let round = state.combat_round_seq;
        if let Some(seat) = state.player_mut(player) {
            seat.cancel_hits_round = Some((round, available - spent));
        }
    }
    spent
}

/// Most "Roll Dice" step replays one combat round may take.
///
/// Each replay needs an effect to exhaust a readied card (The Thundarian), and a card readies
/// again only through another effect (the Nomad's Temporal Command Suite, once per exhaustion of
/// its own, or a Ssruu copy), so the rules allow a handful at most and no real game reaches
/// this. It exists so a pathological ready/exhaust loop ends: past the limit the dice stand as
/// rolled and the step proceeds instead of recursing without end. Chosen as 8: far above any
/// legal chain (one agent, one TCS, one Ssruu copy per round), small enough to bound the stack.
pub(crate) const MAX_ROLL_REPLAYS: u32 = 8;

/// `faction_marks` row an effect sets to send the combat round back to the start of its "Roll Dice"
/// step. Invisible to every seat (`private:#`).
pub(crate) const ROLL_REPLAY_MARK: &str = "private:#combat:replay_roll";

/// Ask for the current space or ground combat round's "Roll Dice" step to be played again: the
/// dice already rolled are discarded without producing a hit. Honoured at the next
/// `SPACE_COMBAT_ROLL_STEP_ENDED` / `GROUND_COMBAT_ROLL_STEP_ENDED`, once.
pub(crate) fn request_roll_replay(state: &mut GameState) {
    state
        .faction_marks
        .insert(ROLL_REPLAY_MARK.to_owned(), String::new());
}

/// Whether a replay was requested; clears the request.
pub(crate) fn take_roll_replay(state: &mut GameState) -> bool {
    state.faction_marks.remove(ROLL_REPLAY_MARK).is_some()
}

/// Whether a card has barred this seat from retreating this combat round (Intercept).
#[must_use]
pub fn retreat_barred(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .and_then(|seat| seat.retreat_barred_round)
        .is_some_and(|round| round == state.combat_round_seq)
}

/// Bar this seat from retreating for the current combat round.
pub fn bar_retreat(state: &mut GameState, player: &PlayerId) {
    let round = state.combat_round_seq;
    if let Some(seat) = state.player_mut(player) {
        seat.retreat_barred_round = Some(round);
    }
}

/// 87.1: each undamaged sustaining unit may cancel one hit. Always optional.
///
/// Returns the hits still to be absorbed.
/// Rear Admiral Farran: a trade good each time one of this player's units sustains.
///
/// Paid where the sustain happens rather than at the card, so it cannot be honoured in one
/// hit-assignment path and forgotten in another.
fn pay_sustain_commander(state: &mut GameState, content: &ContentStore, player: &PlayerId) {
    if crate::leaders::pays_on_sustain(state, content, player)
        && let Some(seat) = state.player_mut(player)
    {
        seat.trade_goods += 1;
        crate::supply::note_trade_goods_gained(state, player, 1, "letnevcommander");
    }
}

/// Rear Admiral Farran from rules version 16: "After 1 of your units uses SUSTAIN DAMAGE: You may
/// gain 1 trade good." Asked once per use, only of an unlocked commander's holder.
fn ask_sustain_commander_windowed(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
) -> Result<(), IllegalChoice> {
    // The window is also a core `Window` (`driver::windows::sustain_commander`); on `driver=dual` both run in
    // lockstep from this state and are compared (docs/ENGINE_CORE_DRIVER.md).
    let mut outcome = Ok(());
    let string_pays = crate::leaders::pays_on_sustain(state, content, player);
    crate::dual_window!(
        crate::dual::Area::Driver,
        "combat::ask_sustain_commander",
        state,
        |s: &mut GameState| {
            outcome = ask_sustain_commander(s, ctx, content, sources, galaxy, player);
            crate::dual::Run::new(outcome.is_ok())
        },
        |s: &mut GameState, script: &crate::dual_window::Recording| {
            crate::dual_window::sustain_commander(s, player, string_pays, script)
        }
    );
    outcome
}

fn ask_sustain_commander(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
) -> Result<(), IllegalChoice> {
    if !crate::leaders::pays_on_sustain(state, content, player) || state.player(player).is_none() {
        return Ok(());
    }
    let choice = Choice::new(
        player.clone(),
        "Rear Admiral Farran: gain 1 trade good for the sustained damage",
        vec![
            ChoiceOption::labelled("gain", "letnev_commander_trade_good", "Gain 1 trade good"),
            ChoiceOption::decline(),
        ],
    )
    .contextualized(DecisionContext::new(
        player.clone(),
        DecisionSource::Content("letnevcommander".to_owned()),
        "letnev_commander_trade_good",
        state.phase,
        state.round,
    ));
    if crate::neutral_units::is_neutral(player) {
        return Ok(());
    }
    let answer = ctx
        .table
        .ask_seeing(&choice, &Observed::new(state, content, sources, galaxy))?;
    if answer.id == "gain"
        && let Some(seat) = state.player_mut(player)
    {
        seat.trade_goods += 1;
        crate::supply::note_trade_goods_gained(state, player, 1, "letnevcommander");
    }
    Ok(())
}

/// Barony of Letnev, Non-Euclidean Shielding: each use of SUSTAIN DAMAGE cancels two hits.
pub(crate) fn non_euclidean_shielding(state: &GameState, player: &PlayerId) -> bool {
    crate::technology::has_technology_text(state, player, "nes")
}

/// Whether this unit may use SUSTAIN DAMAGE against a hit in space.
///
/// Only ships can be assigned space hits, so a mech carried in the space area never sustains
/// one, whatever its card says.
///
/// Every faction module may forbid it (`hooks_combat::may_sustain`; Mentak's flagship: "Other
/// player's ships in this system cannot use SUSTAIN DAMAGE"), asked last so the shared rules
/// answer first.
///
/// `in_combat` is whether the hit is part of a space combat (rolls, start-of-combat and
/// after-round effects, anti-fighter barrage) rather than a SPACE CANNON OFFENSE hit; a module may
/// give a unit that is not a ship the ability for combat only (`hooks_combat::grants_sustain`,
/// Nomad's Quantum Manipulator).
#[allow(
    clippy::too_many_arguments,
    reason = "the unit, its type, its owner, the system and whether a combat is being fought"
)]
fn sustains_in_space(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    player: &PlayerId,
    unit: &Unit,
    kind: &UnitType,
    in_combat: bool,
) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || {
        crate::core_combat::CombatView::of(state, content, sources)?.sustains_in_space(system, player, unit, in_combat)
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::sustains_in_space",
        typed(),
        sustains_in_space_text(state, content, sources, system, player, unit, kind, in_combat)
    )
}

#[allow(clippy::too_many_arguments, reason = "the string side of `sustains_in_space`")]
fn sustains_in_space_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    player: &PlayerId,
    unit: &Unit,
    kind: &UnitType,
    in_combat: bool,
) -> bool {
    &unit.owner == player
        && !unit.sustained_damage
        // Publicize Weapon Schematics: all war suns lose SUSTAIN DAMAGE. Asked of the unit here
        // rather than removed from the type, so repealing the law gives it back.
        && !crate::laws::sustain_suppressed(state, kind.base_type())
        // Metali Void Shielding grants the ability to a non-fighter ship that lacks it. Asked here
        // rather than of the unit type, so a dreadnought is not given a second sustain it never
        // had.
        && ((kind.is_ship()
            && (kind.sustain_damage()
                || (crate::relics::grants_sustain(state, player) && !kind.is_fighter())))
            || crate::factions::hooks_combat::grants_sustain(
                state,
                content,
                sources,
                &crate::factions::CombatUnit {
                    player,
                    system: Some(system),
                    planet: None,
                    unit_type: unit.type_id.as_str(),
                    context: if in_combat { "space" } else { "space_cannon" },
                },
            ))
        && crate::factions::hooks_combat::may_sustain(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet: None,
                unit_type: unit.type_id.as_str(),
                context: "space",
            },
        )
}

/// Whether a hit of `origin` may be assigned to a unit of `player`'s at `system` at all.
///
/// SUSTAIN DAMAGE cancels a hit before assignment, but 87.4a requires assignability: a unit that cannot be
/// assigned a hit from a unit ability (Naaz Eidolon Maximum, "it cannot be assigned hits from unit
/// abilities") is not asked to spend its sustain on that hit either. A hit nothing can take is
/// unused once no eligible unit remains; 15.2a specifically describes bombardment.
fn assignable_to_hit(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    unit: &Unit,
    planet: Option<&ti4_model::id::PlanetId>,
    origin: HitOrigin,
) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || {
        crate::core_combat::CombatView::of(state, content, sources)?.assignable_to_hit(
            player,
            system,
            unit.type_id.as_str(),
            planet,
            origin == HitOrigin::UnitAbility,
        )
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::assignable_to_hit",
        typed(),
        assignable_to_hit_text(state, content, sources, player, system, unit, planet, origin)
    )
}

#[allow(clippy::too_many_arguments, reason = "the string side of `assignable_to_hit`")]
fn assignable_to_hit_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    unit: &Unit,
    planet: Option<&ti4_model::id::PlanetId>,
    origin: HitOrigin,
) -> bool {
    origin != HitOrigin::UnitAbility
        || !crate::factions::hooks_combat::ability_hit_immune(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet,
                unit_type: unit.type_id.as_str(),
                context: "space",
            },
        )
}

/// How many sustains of each unit type the seat can use in a row, for the display-only
/// `sustainers` detail of a sustain question. The question lists one option per unit *type*, so a
/// client cannot tell two cruisers from one cruiser that The Cavalry lets sustain (once one of
/// the type is damaged no other can); without the count it plans a sustain on both.
///
/// Types that sustain by themselves count every ship. A type that only a card grants it is played
/// out on a copy of the game, ship by ship, until none is offered any more.
fn sustainer_counts(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    player: &PlayerId,
    in_combat: bool,
    available: &[(String, Unit)],
) -> serde_json::Value {
    #[cfg(feature = "typed-core")]
    let typed = || {
        crate::core_combat::CombatView::of(state, content, sources)?
            .sustainer_counts(system, player, in_combat, available)
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    let counts = crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::sustainer_counts",
        typed(),
        sustainer_counts_text(state, content, sources, system, player, in_combat, available)
    );
    serde_json::to_value(counts).unwrap_or(serde_json::Value::Null)
}

fn sustainer_counts_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    player: &PlayerId,
    in_combat: bool,
    available: &[(String, Unit)],
) -> std::collections::BTreeMap<String, u64> {
    let types = catalogue(content, sources);
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for (_, unit) in available {
        *counts.entry(unit.type_id.to_string()).or_insert(0) += 1;
    }
    for (kind_id, count) in &mut counts {
        let Some(kind) = types.get(kind_id.as_str()) else {
            continue;
        };
        let by_itself = kind.is_ship()
            && (kind.sustain_damage()
                || (crate::relics::grants_sustain(state, player) && !kind.is_fighter()));
        let in_space = available
            .iter()
            .filter(|(_, unit)| unit.type_id.as_str() == kind_id.as_str())
            .all(|(location, _)| !location.starts_with("planet:"));
        if *count < 2 || by_itself || !in_space {
            continue;
        }
        let mut played = state.clone();
        let mut used = 0_u64;
        loop {
            let next = played.system_state(system).units.iter().position(|unit| {
                unit.type_id.as_str() == kind_id.as_str()
                    && sustains_in_space(
                        &played, content, sources, system, player, unit, kind, in_combat,
                    )
            });
            let Some(index) = next else { break };
            played.system_mut(system).units[index].sustained_damage = true;
            used += 1;
        }
        *count = used.max(1);
    }
    counts
}

fn player_prompt(system: &SystemId) -> String {
    format!("cancel a hit at {system}")
}

/// Dual check of the sustain offer: the typed ask must list the same options, worded the same.
#[allow(clippy::too_many_arguments)]
fn check_sustain_options(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    hits: usize,
    non_fighters_first: bool,
    ability: bool,
    in_combat: bool,
    prompt: &str,
    options: &[ChoiceOption],
) {
    #[cfg(feature = "typed-core")]
    let typed = || {
        use ti4_engine_core::generated::System as CoreSystem;
        let ask = ti4_engine_core::choice::combat::Ask::CancelHit {
            system: <CoreSystem as crate::generated::IdStr>::from_id(system.as_str())?,
            hits: u16::try_from(hits).unwrap_or(u16::MAX),
            non_fighters_first,
            staging: crate::supply::staging_enabled(state),
            ability,
            in_combat,
            window: None,
        };
        crate::core_combat::CombatView::of(state, content, sources)?.sustain_options(player, ask)
    };
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<(String, Vec<crate::core_combat::OptionRow>)>;
    let rows = || {
        (
            prompt.to_owned(),
            options
                .iter()
                .map(|o| (o.id.clone(), o.kind.clone(), o.label.clone(), o.payload.clone()))
                .collect::<Vec<_>>(),
        )
    };
    let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::sustain_options", typed, rows);
}

/// [`offer_sustain`] as a core window on `driver=dual`: both sides run from this state in lockstep and are
/// compared (`rules::combat::sustain_offer`).
#[allow(clippy::too_many_arguments, reason = "offer_sustain's own")]
fn offer_sustain_windowed(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
    origin: HitOrigin,
    during_space_combat: bool,
    non_fighters_first: bool,
) -> Result<usize, CombatError> {
    let mut outcome = None;
    let failed = std::cell::Cell::new(false);
    let args = crate::dual_window::SustainArgs {
        player,
        system,
        producer,
        hits,
        ability: origin == HitOrigin::UnitAbility,
        in_combat: during_space_combat,
        non_fighters_first: non_fighters_first
            && !non_fighter_ships(state, content, sources, player, system).is_empty(),
        string_pays: crate::leaders::pays_on_sustain(state, content, player),
        announce: ctx.timing.is_some(),
    };
    crate::dual_window!(
        crate::dual::Area::Driver,
        "combat::offer_sustain",
        state,
        |s: &mut GameState| {
            let result = offer_sustain(
                s, content, sources, galaxy, ctx, player, system, producer, hits, origin, during_space_combat,
                non_fighters_first,
            );
            failed.set(result.is_err());
            let value = result.as_ref().map_or(usize::MAX, |left| *left);
            outcome = Some(result);
            crate::dual::Run::new(value)
        },
        |s: &mut GameState, script: &crate::dual_window::Recording| {
            if failed.get() {
                return None;
            }
            crate::dual_window::sustain_offer(s, sources, args, script)
        }
    );
    outcome.unwrap_or(Ok(hits))
}

/// Offer every available SUSTAIN DAMAGE until the hits run out or the player takes them.
///
/// Only units that could actually be assigned a hit of `origin` are offered.
/// `non_fighters_first` marks "take the hit" when the loss is bound to non-fighter ships
/// (Graviton Laser System). Display only: a client planning the loss ahead must not pick a
/// fighter the next question will not offer.
#[allow(
    clippy::too_many_arguments,
    reason = "the combat position, the decider and the one binding"
)]
fn offer_sustain(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    mut hits: usize,
    origin: HitOrigin,
    during_space_combat: bool,
    non_fighters_first: bool,
) -> Result<usize, CombatError> {
    let types = catalogue(content, sources);
    while hits > 0 {
        let mut available: Vec<(String, Unit)> = state
            .system_state(system)
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                types.get(unit.type_id.as_str()).is_some_and(|kind| {
                    sustains_in_space(
                        state,
                        content,
                        sources,
                        system,
                        player,
                        unit,
                        kind,
                        during_space_combat,
                    )
                }) && assignable_to_hit(state, content, sources, player, system, unit, None, origin)
            })
            .map(|(index, unit)| (format!("space:{index}"), unit.clone()))
            .collect();
        // A Maximum on a planet is a combat ship only while a normal ship shares the space area;
        // mirror `ships_of` here so it can actually cancel a hit it was allowed to join.
        // A ground force an Alastor brought into the battle is mirrored the same way.
        let mut distinct_fresh: std::collections::BTreeMap<ti4_model::id::PlanetId, Vec<Unit>> =
            std::collections::BTreeMap::new();
        for (planet, index, unit) in planet_combatants(state, content, sources, player, system) {
            if !planet_unit_sustains(
                state,
                content,
                sources,
                system,
                player,
                &unit,
                during_space_combat,
            ) || !assignable_to_hit(
                state,
                content,
                sources,
                player,
                system,
                &unit,
                Some(&planet),
                origin,
            ) {
                continue;
            }
            let seen_here = distinct_fresh.entry(planet.clone()).or_default();
            if seen_here.contains(&unit) {
                continue;
            }
            let location = if unit.type_id.as_str() != "naaz_voltron"
                || (crate::supply::staging_enabled(state) && !seen_here.is_empty())
            {
                format!("planet:{planet}:variant:{index}")
            } else {
                format!("planet:{planet}")
            };
            seen_here.push(unit.clone());
            available.push((location, unit));
        }
        if available.is_empty() {
            return Ok(hits);
        }

        // OBS-008b4: sustaining keeps the ship, damaged; declining loses it. Either way every
        // sustain-or-decline option in this ask shares the same before count and the same two
        // possible afters, so it is read once per ask rather than once per option.
        let own_ships = i64::try_from(ships_of(state, content, sources, player, system).len())
            .unwrap_or(i64::MAX);

        // One option per unit *type*. Every unit here is undamaged by the filter above, so two
        // of the same type are the same decision written twice — and the copies skew it,
        // because a sampling decider would sustain on whichever type it happened to own more of.
        let mut seen = std::collections::BTreeSet::new();
        let mut options = Vec::new();
        for (location, unit) in &available {
            if !(crate::supply::staging_enabled(state) && location.starts_with("planet:"))
                && !seen.insert(unit.type_id.to_string())
            {
                continue;
            }
            let label = if crate::supply::staging_enabled(state) {
                planetary_sustain_location(location).map_or_else(
                    || format!("sustain damage on {}", unit.type_id),
                    |(planet, _)| planet_sustain_label(content, sources, planet, unit),
                )
            } else {
                format!("sustain damage on {}", unit.type_id)
            };
            options.push(
                ChoiceOption::labelled(format!("sustain|{location}"), SUSTAIN_KIND, label)
                    .with("unit", unit.type_id.to_string())
                    .previewed(Preview::certain(vec![Delta::new(
                        Quantity::ShipsInSystem,
                        own_ships,
                        own_ships,
                    )])),
            );
        }
        let mut take = ChoiceOption::labelled(
            crate::choice::DECLINE_ID,
            crate::choice::DECLINE_KIND,
            "take the hit",
        )
        .previewed(Preview::certain(vec![Delta::new(
            Quantity::ShipsInSystem,
            own_ships,
            own_ships - 1,
        )]));
        if non_fighters_first
            && !non_fighter_ships(state, content, sources, player, system).is_empty()
        {
            take = take.with("non_fighters_first", true);
        }
        options.push(take);
        if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
            check_sustain_options(
                state,
                content,
                sources,
                player,
                system,
                hits,
                non_fighters_first
                    && !non_fighter_ships(state, content, sources, player, system).is_empty(),
                origin == HitOrigin::UnitAbility,
                during_space_combat,
                &player_prompt(system),
                &options,
            );
        }

        let sustainers = sustainer_counts(
            state,
            content,
            sources,
            system,
            player,
            during_space_combat,
            &available,
        );
        let choice = Choice::new(player.clone(), format!("cancel a hit at {system}"), options)
            .detailed("sustainers", sustainers)
            .contextualized(
                DecisionContext::new(
                    player.clone(),
                    DecisionSource::Rule("82".to_owned()),
                    "sustain_damage",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::System(system.clone()))
                .owing(OutstandingConstraint::new(
                    ConstraintKind::UnitsToRemove,
                    i64::try_from(hits).unwrap_or(0),
                    0,
                )),
            );
        // Neutral units rule 5: they use every ability they can, so they always sustain and are
        // never asked.
        let answer = if crate::neutral_units::is_neutral(player) {
            match choice
                .options
                .iter()
                .find(|option| !option.is_decline())
                .cloned()
            {
                Some(option) => option,
                None => return Ok(hits),
            }
        } else {
            ctx.table
                .ask_seeing(&choice, &Observed::new(state, content, sources, galaxy))?
        };
        if answer.is_decline() {
            return Ok(hits);
        }
        let Some(location) = answer.id.strip_prefix("sustain|") else {
            return Ok(hits);
        };
        let planet_side: std::collections::BTreeSet<(ti4_model::id::PlanetId, usize)> =
            planet_combatants(state, content, sources, player, system)
                .into_iter()
                .map(|(planet, index, _)| (planet, index))
                .collect();
        let expected_rows = if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
            #[cfg(feature = "typed-core")]
            {
                crate::core_combat::CombatView::of(state, content, sources)
                    .and_then(|v| v.sustained_rows(player, system, location))
            }
            #[cfg(not(feature = "typed-core"))]
            {
                None
            }
        } else {
            None
        };
        let sustained = if let Some(index) = location
            .strip_prefix("space:")
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            state.system_mut(system).units.get_mut(index)
        } else if let Some((planet, variant_index)) = planetary_sustain_location(location) {
            let planet_id = ti4_model::id::PlanetId::new(planet);
            state
                .system_mut(system)
                .planet_units
                .get_mut(&planet_id)
                .and_then(|units| {
                    if let Some(index) = variant_index {
                        units.get_mut(index).filter(|unit| {
                            unit.owner == *player
                                && planet_side.contains(&(planet_id.clone(), index))
                                && !unit.sustained_damage
                        })
                    } else {
                        units.iter_mut().find(|unit| {
                            unit.owner == *player
                                && unit.type_id.as_str() == "naaz_voltron"
                                && !unit.sustained_damage
                        })
                    }
                })
        } else {
            None
        };
        if let Some(unit) = sustained {
            let kind = unit.type_id.to_string();
            *unit = unit.sustained();
            let exact_target = unit.clone();
            remember_sustain_target(state, system, location, &exact_target);
            if let Some(expected) = expected_rows {
                let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::apply_sustain", || Some(expected), || {
                    crate::core_combat::board_rows(state, system)
                });
            }
            if state.rules_version >= ti4_model::state::RULES_FACTION_EFFECTS {
                ask_sustain_commander_windowed(state, ctx, content, sources, galaxy, player)?;
            } else {
                pay_sustain_commander(state, content, player);
            }
            emit_sustain_used(
                state,
                ctx,
                galaxy,
                system,
                player,
                &kind,
                producer,
                during_space_combat,
            )?;
        }
        // Non-Euclidean Shielding: "When 1 of your units uses SUSTAIN DAMAGE, cancel 2 hits
        // instead of 1." Per *use*, so a second hit costs the same one damaged ship -- which is
        // why it is a subtraction here rather than a second pass through the offer.
        #[cfg(feature = "typed-core")]
        let typed = || crate::core_combat::CombatView::of(state, content, sources)?.cancel_per_sustain(player);
        #[cfg(not(feature = "typed-core"))]
        let typed = || None;
        let cancels = crate::dual_typed!(
            crate::dual::Area::Combat,
            "combat::cancel_per_sustain",
            typed(),
            if non_euclidean_shielding(state, player) { 2 } else { 1 }
        );
        hits = hits.saturating_sub(cancels);
    }
    Ok(hits)
}

/// 78.6: cancel what Sustain Damage can, then lose one ship per remaining hit.
///
/// Excess hits beyond the units available simply have no effect (15.2a).
///
/// # Errors
/// [`CombatError::IllegalChoice`] when a decider answers with something not offered.
pub fn absorb_hits(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    table: &mut Table,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
) -> Result<(), CombatError> {
    // The windowless path: the resolver carries the table the questions go to but no timing
    // machinery, so a sustained hit announces nothing and cannot be reacted to. The dice and
    // roller are never touched on this path and exist only to satisfy the handle's shape.
    let mut dice = crate::dice::Dice::new();
    let mut rng = crate::rng::GameRng::new(0);
    let mut ctx = Resolving {
        content,
        sources,
        dice: &mut dice,
        rng: &mut rng,
        table,
        timing: None,
    };
    absorb_hits_seeing(
        state, content, sources, None, &mut ctx, player, system, producer, hits,
    )
}

/// Absorb hits with the public map attached to every learned casualty decision.
///
/// # Errors
/// [`CombatError::IllegalChoice`] when a decider answers with something not offered.
#[allow(
    clippy::too_many_arguments,
    reason = "hit assignment needs the combat position and optional map observation"
)]
pub fn absorb_hits_seeing(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
) -> Result<(), CombatError> {
    absorb_hits_seeing_with_origin(
        state,
        content,
        sources,
        galaxy,
        ctx,
        player,
        system,
        producer,
        hits,
        HitOrigin::CombatRoll,
    )
}

/// Assign hits with their source classified for unit-ability immunities.
///
/// # Errors
/// As [`absorb_hits_seeing`].
#[allow(clippy::too_many_arguments)]
pub fn absorb_hits_seeing_with_origin(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
    origin: HitOrigin,
) -> Result<(), CombatError> {
    absorb_hits_seeing_with(
        state, content, sources, galaxy, ctx, player, system, producer, hits, false, origin,
    )
}

/// [`absorb_hits_seeing`], with the hits bound to non-fighter ships while any are left
/// (Graviton Laser System: "hits produced by those units must be assigned to non-fighter ships
/// if able").
///
/// # Errors
/// As [`absorb_hits_seeing`].
#[allow(
    clippy::too_many_arguments,
    reason = "absorb_hits_seeing's inputs plus the one binding"
)]
pub fn absorb_hits_seeing_with(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
    non_fighters_first: bool,
    origin: HitOrigin,
) -> Result<(), CombatError> {
    let cause = match origin {
        HitOrigin::CombatRoll => "effect_hit",
        HitOrigin::UnitAbility => "unit_ability:space_cannon",
    };
    absorb_hits_seeing_with_context(
        state,
        content,
        sources,
        galaxy,
        ctx,
        player,
        system,
        producer,
        hits,
        non_fighters_first,
        origin,
        cause,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn absorb_hits_seeing_with_context(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    hits: usize,
    non_fighters_first: bool,
    origin: HitOrigin,
    cause: &str,
    during_space_combat: bool,
) -> Result<(), CombatError> {
    // Shields Holding and Maneuvering Jets friends cancel hits before any are assigned: the
    // round-scoped pool they grant into is spent here, the way the combat window's queue
    // spends it, so a cancelled cannon or barrage hit is one nobody has to absorb.
    let hits = hits.saturating_sub(spend_cancellations(state, player, hits));
    let remaining = offer_sustain_windowed(
        state,
        content,
        sources,
        galaxy,
        ctx,
        player,
        system,
        producer,
        hits,
        origin,
        during_space_combat,
        non_fighters_first,
    )?;

    assign_remaining_windowed(
        state,
        content,
        sources,
        galaxy,
        ctx,
        player,
        system,
        producer,
        remaining,
        non_fighters_first,
        origin,
        cause,
        during_space_combat,
    )
}

/// [`assign_remaining`] as a core window on `driver=dual` (`rules::combat::assign`).
#[allow(clippy::too_many_arguments, reason = "assign_remaining's own")]
fn assign_remaining_windowed(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    remaining: usize,
    non_fighters_first: bool,
    origin: HitOrigin,
    cause: &str,
    during_space_combat: bool,
) -> Result<(), CombatError> {
    let failed = std::cell::Cell::new(false);
    let mut outcome = None;
    let args = crate::dual_window::AssignArgs {
        player,
        system,
        producer,
        hits: remaining,
        ability: origin == HitOrigin::UnitAbility,
        non_fighters_first,
        cause,
        cause_kind: hit_cause_of(cause),
        during_space_combat,
        announce_staging: crate::supply::staging_enabled(state),
        announce: ctx.timing.is_some(),
        neutral: crate::neutral_units::is_neutral(player),
    };
    crate::dual_window!(
        crate::dual::Area::Driver,
        "combat::assign_hits",
        state,
        |s: &mut GameState| {
            let result = assign_remaining(
                s, content, sources, galaxy, ctx, player, system, producer, remaining, non_fighters_first,
                origin, cause, during_space_combat,
            );
            failed.set(result.is_err());
            outcome = Some(result);
            crate::dual::Run::new(!failed.get())
        },
        |s: &mut GameState, script: &crate::dual_window::Recording| {
            if failed.get() {
                return None;
            }
            crate::dual_window::assign_hits(s, sources, args, script)
        }
    );
    outcome.unwrap_or(Ok(()))
}

/// The loop of [`absorb_hits_seeing_with_context`] that assigns `remaining` hits one casualty at a time.
#[allow(clippy::too_many_arguments, reason = "the absorb call's own")]
fn assign_remaining(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    producer: &PlayerId,
    mut remaining: usize,
    non_fighters_first: bool,
    origin: HitOrigin,
    cause: &str,
    during_space_combat: bool,
) -> Result<(), CombatError> {
    while remaining > 0 {
        let mut alive = ships_of(state, content, sources, player, system);
        alive.retain(|unit| {
            let board = state.system_state(system);
            assignable_to_hit(
                state,
                content,
                sources,
                player,
                system,
                unit,
                if board.units.contains(unit) {
                    None
                } else {
                    board
                        .planet_units
                        .iter()
                        .find_map(|(planet, units)| units.contains(unit).then_some(planet))
                },
                origin,
            )
        });
        if non_fighters_first && !alive.is_empty() {
            let mut bound = non_fighter_ships(state, content, sources, player, system);
            bound.retain(|unit| alive.contains(unit));
            if !bound.is_empty() {
                alive = bound;
            }
        }
        if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
            #[cfg(feature = "typed-core")]
            let typed = || {
                crate::core_combat::CombatView::of(state, content, sources)?.hit_targets(
                    player,
                    system,
                    origin == HitOrigin::UnitAbility,
                    non_fighters_first,
                )
            };
            #[cfg(not(feature = "typed-core"))]
            let typed = || None::<Vec<Unit>>;
            let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::hit_targets", typed, || alive.clone());
        }
        if alive.is_empty() {
            return Ok(()); // 15.2a
        }
        let casualty = choose_casualty_owing(
            state,
            content,
            sources,
            galaxy,
            ctx.table,
            player,
            &alive,
            &DecisionSource::Rule("78.4".to_owned()),
            "assign_casualty",
            Some(system),
            Some(remaining),
            Some(HitDetail {
                cause: hit_cause_of(cause),
                destroy: false,
                restriction: if non_fighters_first {
                    HitRestriction::NonFighter
                } else {
                    HitRestriction::Any
                },
                producer: Some(producer.clone()),
            }),
        )?;
        remove_combat_ship(state, system, &casualty);
        if crate::supply::staging_enabled(state) {
            try_announce_ship_destroyed_type(
                state,
                ctx,
                system,
                player,
                &casualty.type_id,
                cause,
                during_space_combat,
            )?;
        }
        remaining -= 1;
    }
    Ok(())
}

/// Graviton Laser System: "You may exhaust this card before 1 or more of your units uses SPACE
/// CANNON; hits produced by those units must be assigned to non-fighter ships if able."
///
/// Used (and exhausted) only when it can change something: hits to assign, and a target fleet
/// with both fighters and non-fighters. Returns whether the hits are bound to non-fighters.
pub fn use_graviton(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    gunner: &PlayerId,
    victim: &PlayerId,
    system: &SystemId,
    hits: usize,
) -> bool {
    let card = ti4_model::id::TechnologyId::new("gls");
    let ready = state.player(gunner).is_some_and(|seat| {
        seat.technologies.contains(&card) && !seat.exhausted_technologies.contains(&card)
    });
    let ships = ships_of(state, content, sources, victim, system).len();
    let non_fighters = non_fighter_ships(state, content, sources, victim, system).len();
    if !ready || hits == 0 || non_fighters == 0 || non_fighters == ships {
        return false;
    }
    if let Some(seat) = state.player_mut(gunner) {
        seat.exhausted_technologies.insert(card);
    }
    true
}

/// This player's ships in the system other than fighters.
fn non_fighter_ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<Unit> {
    let types = catalogue(content, sources);
    ships_of(state, content, sources, player, system)
        .into_iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| !kind.is_fighter())
        })
        .collect()
}

/// Generic cause used for an ordinary space-combat casualty.
///
/// `cause` names what removed the ship. Whether the removal happened during a space combat is a
/// separate `during_space_combat` fact: Direct Hit, Courageous, Devotion, and Impulse Core retain
/// their real effect source even when they act inside a fight.
pub const SHIP_CAUSE_COMBAT: &str = "space_combat";

/// Whether a `SHIP_DESTROYED` event is a ship lost during a space combat.
///
/// An event recorded before this key existed is not assumed to be a combat loss.
pub fn destroyed_during_combat(event: &crate::event::Event) -> bool {
    event.boolean("during_space_combat") == Some(true)
}

/// The `SHIP_DESTROYED` payload: the removal, whether it left the owner with nothing in the
/// system, and what caused it.
///
/// Shared by the combat window's own emissions and by the card-play path that drains
/// [`GameState::pending_destructions`], so the two cannot drift apart on a key a listener reads.
pub(crate) fn ship_destroyed_payload(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    owner: &PlayerId,
    unit: &ti4_model::id::UnitTypeId,
    cause: &str,
    during_space_combat: bool,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let remaining = ships_of(state, content, sources, owner, system).len();
    let mut payload = std::collections::BTreeMap::new();
    // The galvanize token went back to the supply with the ship; the event is the only record that
    // it was galvanized (Last Bastion). Present only when true, so other games' events are unchanged.
    if crate::factions::observe(
        state,
        crate::factions::Obs::ShipLostTaken {
            system,
            owner,
            unit,
        },
    ) {
        payload.insert("galvanized".to_owned(), true.into());
    }
    payload.insert("system".to_owned(), system.to_string().into());
    payload.insert("player".to_owned(), owner.to_string().into());
    payload.insert("unit".to_owned(), unit.to_string().into());
    payload.insert("last".to_owned(), (remaining == 0).into());
    payload.insert("cause".to_owned(), cause.to_owned().into());
    payload.insert("during_space_combat".to_owned(), during_space_combat.into());
    payload
}

/// Announce one destroyed ship, and whether it was the owner's last in the system.
///
/// Two printed windows read this: "after 1 of your ships is destroyed during a space combat", and
/// "when your last ship in the active system is destroyed". The second is not a separate moment --
/// it is the first with a fact attached -- so one event carries `last` rather than two events
/// racing to describe the same removal.
///
/// Called after the unit is off the board, so `last` is read from the position a reacting card
/// would see. The handoff [`GameState::last_ship_destroyed`] is recorded first: an effect that
/// needs to know *which* ship was destroyed cannot read the event once the window has run.
fn announce_ship_destroyed(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    owner: &PlayerId,
    destroyed: &Unit,
) {
    announce_ship_destroyed_type(
        state,
        ctx,
        system,
        owner,
        &destroyed.type_id,
        SHIP_CAUSE_COMBAT,
        true,
    );
}

/// [`announce_ship_destroyed`] for a ship already off the board, named by its type.
///
/// `cause` names what removed it; see [`SHIP_CAUSE_COMBAT`].
fn announce_ship_destroyed_type(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    owner: &PlayerId,
    unit: &ti4_model::id::UnitTypeId,
    cause: &str,
    during_space_combat: bool,
) {
    let _ = try_announce_ship_destroyed_type(
        state,
        ctx,
        system,
        owner,
        unit,
        cause,
        during_space_combat,
    );
}

fn try_announce_ship_destroyed_type(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    owner: &PlayerId,
    unit: &ti4_model::id::UnitTypeId,
    cause: &str,
    during_space_combat: bool,
) -> Result<(), CombatError> {
    state.last_ship_destroyed = Some((system.clone(), owner.clone(), unit.clone()));
    let payload = ship_destroyed_payload(
        state,
        ctx.content,
        ctx.sources,
        system,
        owner,
        unit,
        cause,
        during_space_combat,
    );
    ctx.emit(state, "SHIP_DESTROYED", payload)?;
    Ok(())
}

/// The retreat is named by the declaring player, so the window guard is `actor_is_not`.
fn emit_retreat_declared(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    player: &PlayerId,
    round: u32,
) {
    let mut payload = std::collections::BTreeMap::new();
    payload.insert("system".to_owned(), system.to_string().into());
    payload.insert("player".to_owned(), player.to_string().into());
    payload.insert("round".to_owned(), i64::from(round).into());
    let _ = ctx.emit(state, "RETREAT_DECLARED", payload);
}

impl CombatError {
    /// Windows mid-answer only ever surface a choice failure; a combat that merely ran past
    /// the round limit is reported by the standalone driver, never by a window. The second
    /// arm maps that impossible case to an honest "no options" refusal rather than a panic.
    pub(crate) fn into_illegal_choice(self) -> IllegalChoice {
        match self {
            CombatError::IllegalChoice(error) => error,
            CombatError::Timing(error) => match error {
                crate::timing::TimingError::IllegalChoice(error) => error,
                other => IllegalChoice::NoOptions {
                    player: PlayerId::new(""),
                    prompt: other.to_string(),
                },
            },
            CombatError::Unresolved(system) => IllegalChoice::NoOptions {
                player: PlayerId::new(""),
                prompt: format!("combat in {system} did not settle"),
            },
        }
    }
}

/// Both sustain windows read the moment, and both need the unit, so the event names it.
/// The `producer` is the player whose unit or ability produced the cancelled hit — Direct
/// Hit's window only opens for the producer, not for any sustained hit on a rival's fleet.
/// The handoff [`GameState::last_sustain`] is recorded first: the effect cannot see the
/// event once the window has run.
///
/// Reflective Shielding, played in the window the emission just opened, stages hits against
/// the sustained hit's producer ("your opponent"). The window has closed by the time this
/// runs, so the victim's absorption — with its own sustain offers and loss choices — can now
/// run against the same resolver. The drain lives here rather than at the two sustain sites
/// because the combat window's queue and the absorption path both apply a sustain through
/// this one emission, and a reflective hit must answer either.
///
/// # Errors
///
/// [`CombatError::IllegalChoice`] when the decider refuses the victim's loss choice.
fn planetary_sustain_location(location: &str) -> Option<(&str, Option<usize>)> {
    let planet = location.strip_prefix("planet:")?;
    if let Some((planet, index)) = planet.rsplit_once(":variant:") {
        Some((planet, Some(index.parse::<usize>().ok()?)))
    } else {
        Some((planet, None))
    }
}
fn remember_sustain_target(state: &mut GameState, system: &SystemId, location: &str, unit: &Unit) {
    if crate::supply::staging_enabled(state) {
        let planet = planetary_sustain_location(location).map(|(planet, _)| planet);
        state.faction_marks.insert(
            "combat:sustain_target".to_owned(),
            serde_json::json!({"system": system, "planet": planet, "unit": unit}).to_string(),
        );
    }
}

fn emit_sustain_used(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    system: &SystemId,
    player: &PlayerId,
    unit: &str,
    producer: &PlayerId,
    during_space_combat: bool,
) -> Result<(), CombatError> {
    state.last_sustain = Some((
        system.clone(),
        player.clone(),
        ti4_model::id::UnitTypeId::new(unit),
        producer.clone(),
        during_space_combat,
    ));
    let mut payload = std::collections::BTreeMap::new();
    payload.insert("system".to_owned(), system.to_string().into());
    payload.insert("player".to_owned(), player.to_string().into());
    payload.insert("unit".to_owned(), unit.to_owned().into());
    payload.insert("producer".to_owned(), producer.to_string().into());
    payload.insert("during_space_combat".to_owned(), during_space_combat.into());
    // Direct Hit's window reads this: Dreadnought II and its faction versions "cannot be destroyed
    // by 'Direct Hit' action cards", which the corpus carries as the absence of `canBeDirectHit`.
    // A faction module may add an immunity of its own (`hooks_combat::direct_hit_immune`).
    payload.insert(
        "direct_hittable".to_owned(),
        direct_hittable_at(state, ctx.content, ctx.sources, player, system, unit).into(),
    );
    if crate::supply::staging_enabled(state) {
        ctx.emit(state, "SUSTAIN_DAMAGE_USED", payload)?;
    } else {
        let _ = ctx.emit(state, "SUSTAIN_DAMAGE_USED", payload);
    }
    if let Some((sys, victim, hits)) = std::mem::take(&mut state.pending_reflective_hits)
        && victim != *player
    {
        absorb_hits_seeing_with_context(
            state,
            ctx.content,
            ctx.sources,
            galaxy,
            ctx,
            &victim,
            &sys,
            player,
            hits,
            false,
            HitOrigin::CombatRoll,
            "action_card:reflective_shielding",
            during_space_combat,
        )?;
    }
    Ok(())
}

/// Whether a Direct Hit may destroy a ship of this unit type. A type the corpus does not know is
/// treated as hittable, the card's printed default.
#[must_use]
pub fn direct_hittable(content: &ContentStore, sources: SourceSet, unit: &str) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::direct_hittable(sources, unit);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::direct_hittable",
        typed(),
        direct_hittable_text(content, sources, unit)
    )
}

fn direct_hittable_text(content: &ContentStore, sources: SourceSet, unit: &str) -> bool {
    catalogue(content, sources)
        .get(unit)
        .is_none_or(ti4_content::units::UnitType::can_be_direct_hit)
}

/// Whether a Direct Hit may destroy this player's ship of type `unit` in `system` now: the
/// printed rule ([`direct_hittable`]) and no faction module's immunity
/// (`hooks_combat::direct_hit_immune`; Exotrireme II carries its own printed flag, a module covers
/// what a flag cannot).
///
/// The `direct_hittable` payload of `SUSTAIN_DAMAGE_USED` is this value. The Direct Hit effect
/// (`action_cards::direct_hit`) still asks the printed [`direct_hittable`] only: see the request
/// in `plans/evidence/BF-00c-space.md`.
#[must_use]
pub fn direct_hittable_at(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    unit: &str,
) -> bool {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::CombatView::of(state, content, sources)?.direct_hittable_at(player, system, unit);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::direct_hittable_at",
        typed(),
        direct_hittable_at_text(state, content, sources, player, system, unit)
    )
}

fn direct_hittable_at_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    unit: &str,
) -> bool {
    direct_hittable(content, sources, unit)
        && !crate::factions::hooks_combat::direct_hit_immune(
            state,
            content,
            sources,
            &crate::factions::CombatUnit {
                player,
                system: Some(system),
                planet: None,
                unit_type: unit,
                context: "space",
            },
        )
}

/// Destroy specific ships of `player` in `system` as an effect, not a combat hit.
///
/// For a card or faction ability that says "destroy" (Exotrireme II: "you may destroy this unit
/// to destroy up to 2 ships in this system"; Van Hauge: "When this ship is destroyed, destroy all
/// ships in this system"). Each `victim` names one ship by value (type and damage) and must be a
/// ship of `player` in the space area; a unit that is not there stops the whole call before any
/// change (atomic), returning `0`. Units carry no identity, so two victims of one type and state
/// are one stack's worth: name each copy once.
///
/// This only changes the board and stages one announcement per ship in
/// [`GameState::pending_destructions`], the route a window effect (which holds a
/// [`crate::timing::TimingContext`] but no resolver) already uses. The announcements -- the same
/// `SHIP_DESTROYED` (payload `system`, `player`, `unit`, `last`) the combat emits for a casualty --
/// are emitted by [`announce_staged_destructions`], which the combat window runs right after every
/// event and hook where an effect can stage one. Callers that hold a [`Resolving`] use
/// [`destroy_ships_announced`] instead.
///
/// `cause` is the provenance the staged announcements carry; see [`SHIP_CAUSE_COMBAT`] for the
/// vocabulary. A caller that is resolving a space combat passes [`SHIP_CAUSE_COMBAT`].
///
/// Returns how many ships were destroyed.
pub fn destroy_units(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    victims: &[Unit],
    cause: &str,
) -> usize {
    destroy_units_with_context(
        state,
        content,
        sources,
        player,
        system,
        victims,
        cause,
        cause == SHIP_CAUSE_COMBAT,
    )
}

/// [`destroy_units`] with the combat-window fact supplied independently from the effect source.
#[allow(clippy::too_many_arguments)]
pub fn destroy_units_with_context(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    victims: &[Unit],
    cause: &str,
    during_space_combat: bool,
) -> usize {
    // Every victim must be there: count what the board holds per value against what is asked.
    let present = ships_of(state, content, sources, player, system);
    let mut remaining = present;
    for victim in victims {
        let Some(index) = remaining.iter().position(|unit| unit == victim) else {
            return 0;
        };
        remaining.remove(index);
    }
    for victim in victims {
        remove_combat_ship(state, system, victim);
        state.pending_destructions.push((
            system.clone(),
            player.clone(),
            victim.type_id.clone(),
            cause.to_owned(),
            during_space_combat,
        ));
    }
    victims.len()
}

/// [`destroy_units`], announced at once for a caller that holds a [`Resolving`].
pub fn destroy_ships_announced(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    player: &PlayerId,
    system: &SystemId,
    victims: &[Unit],
) -> usize {
    let destroyed = destroy_units(
        state,
        ctx.content,
        ctx.sources,
        player,
        system,
        victims,
        SHIP_CAUSE_COMBAT,
    );
    announce_staged_destructions(state, ctx);
    destroyed
}

/// Announce every destruction an effect staged in [`GameState::pending_destructions`] as
/// `SHIP_DESTROYED`, in staging order. An announcement can stage more (a ship that destroys others
/// when it dies), so this runs until nothing is left; it ends because every staged destruction has
/// already removed a ship. A no-op when nothing is staged.
/// Strict drain for Game callers: preserve the staged batch and real services on a failed reaction.
pub(crate) fn try_announce_staged_destructions(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
) -> Result<(), CombatError> {
    let checkpoint = (
        state.clone(),
        ctx.dice.clone(),
        ctx.rng.clone(),
        ctx.table.log.clone(),
    );
    let timing = ctx
        .timing
        .as_ref()
        .map(|handle| (handle.sequence.clone(), handle.resolver.checkpoint()));
    let result = (|| {
        while !state.pending_destructions.is_empty() {
            for (system, owner, unit, cause, during_space_combat) in
                std::mem::take(&mut state.pending_destructions)
            {
                try_announce_ship_destroyed_type(
                    state,
                    ctx,
                    &system,
                    &owner,
                    &unit,
                    &cause,
                    during_space_combat,
                )?;
            }
        }
        Ok(())
    })();
    if result.is_err() {
        *state = checkpoint.0;
        *ctx.dice = checkpoint.1;
        *ctx.rng = checkpoint.2;
        ctx.table.log = checkpoint.3;
        if let Some((sequence, resolver)) = timing
            && let Some(handle) = ctx.timing.as_mut()
        {
            *handle.sequence = sequence;
            handle.resolver.restore(resolver);
        }
    }
    result
}

pub fn announce_staged_destructions(state: &mut GameState, ctx: &mut Resolving<'_>) {
    while !state.pending_destructions.is_empty() {
        for (system, owner, unit, cause, during_space_combat) in
            std::mem::take(&mut state.pending_destructions)
        {
            announce_ship_destroyed_type(
                state,
                ctx,
                &system,
                &owner,
                &unit,
                &cause,
                during_space_combat,
            );
        }
    }
}

/// 78.6: the owning player chooses which of their own units dies.
#[cfg(test)]
pub(crate) fn choose_casualty(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    table: &mut Table,
    player: &PlayerId,
    units: &[Unit],
    source: &DecisionSource,
    subtype: &str,
    target: Option<&SystemId>,
) -> Result<Unit, CombatError> {
    choose_casualty_owing(
        state, content, sources, galaxy, table, player, units, source, subtype, target, None, None,
    )
}

/// [`choose_casualty`], telling the decision what kind of hit it assigns (display only), for the
/// one-off losses whose kind a client cannot read from the subtype alone.
#[allow(
    clippy::too_many_arguments,
    reason = "choose_casualty's inputs plus the hit's description"
)]
pub(crate) fn choose_casualty_as(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    table: &mut Table,
    player: &PlayerId,
    units: &[Unit],
    source: &DecisionSource,
    subtype: &str,
    target: Option<&SystemId>,
    hit: HitDetail,
) -> Result<Unit, CombatError> {
    choose_casualty_owing(
        state,
        content,
        sources,
        galaxy,
        table,
        player,
        units,
        source,
        subtype,
        target,
        None,
        Some(hit),
    )
}

/// [`choose_casualty`], telling the decision how many hits are still owed (itself included), so
/// a client can stage them together instead of guessing.
pub(crate) fn choose_casualty_owing(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    table: &mut Table,
    player: &PlayerId,
    units: &[Unit],
    source: &DecisionSource,
    subtype: &str,
    target: Option<&SystemId>,
    owed: Option<usize>,
    hit: Option<HitDetail>,
) -> Result<Unit, CombatError> {
    if let [only] = units {
        return Ok(only.clone());
    }
    // Neutral units rule 7: nobody chooses; the hit goes to the unit lowest on the reference card.
    if crate::neutral_units::is_neutral(player)
        && let Some(unit) = crate::neutral_units::next_casualty(
            &crate::neutral_units::roster(content, sources),
            units,
            |_| true,
        )
    {
        return Ok(unit.clone());
    }
    // One option per distinguishable loss. Five fighters are one decision, not five, and
    // offering it five times mattered: a sampling decider draws per option, so with five
    // fighters and one dreadnought it destroyed a fighter five times in six whatever it thought
    // of the trade — the count decided, not the scoring.
    //
    // Damage is part of what distinguishes a unit, and goes in the label as well as the key:
    // losing an already-damaged dreadnought is a different proposition from losing a fresh one.
    let mut seen = std::collections::BTreeSet::new();
    let mut options = Vec::new();
    for (index, unit) in units.iter().enumerate() {
        if !seen.insert((unit.type_id.to_string(), unit.sustained_damage)) {
            continue;
        }
        let damaged = if unit.sustained_damage {
            " (damaged)"
        } else {
            ""
        };
        options.push(
            ChoiceOption::labelled(
                format!("destroy|{index}"),
                CASUALTY_KIND,
                format!("destroy {}{damaged}", unit.type_id),
            )
            .with("unit", unit.type_id.to_string())
            .with("damaged", unit.sustained_damage),
        );
    }
    if crate::dual::backend(crate::dual::Area::Combat) == crate::dual::Backend::Dual {
        #[cfg(feature = "typed-core")]
        let typed = || crate::core_combat::CombatView::of(state, content, sources)?.casualty_options(units);
        #[cfg(not(feature = "typed-core"))]
        let typed = || None::<Vec<crate::core_combat::OptionRow>>;
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::casualty_options", typed, || {
            options
                .iter()
                .map(|o| (o.id.clone(), o.kind.clone(), o.label.clone(), o.payload.clone()))
                .collect::<Vec<_>>()
        });
    }
    let mut context = DecisionContext::new(
        player.clone(),
        source.clone(),
        subtype,
        state.phase,
        state.round,
    );
    if let Some(system) = target {
        context = context.about(DecisionTarget::System(system.clone()));
    }
    if let Some(owed) = owed {
        context = context.owing(OutstandingConstraint::new(
            ConstraintKind::UnitsToRemove,
            i64::try_from(owed).unwrap_or(0),
            0,
        ));
    }
    if let Some(hit) = hit {
        context = context.with_hit(hit);
    }
    let choice = Choice::new(player.clone(), "assign a hit", options).contextualized(context);
    let answer = table.ask_seeing(&choice, &Observed::new(state, content, sources, galaxy))?;
    let index = answer
        .id
        .strip_prefix("destroy|")
        .and_then(|rest| rest.parse::<usize>().ok())
        .unwrap_or(0);
    Ok(units.get(index).unwrap_or(&units[0]).clone())
}

/// Whether a retreat may end in `destination` under the movement rules: no asteroid field
/// without Antimass Deflectors (11.1), no supernova (86.1) except where a faction lifts it. A
/// nebula is fine (a retreat is one step into the "active" system, 59.1). Older games
/// (`rules_version` below `RULES_RETREAT_HAZARDS`) never checked, so they replay unchanged.
fn retreat_may_enter(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    destination: &SystemId,
) -> bool {
    if state.rules_version < ti4_model::state::RULES_RETREAT_HAZARDS {
        return true;
    }
    let board = crate::movement::Board::for_player(state, content, sources, player);
    crate::movement::MovementRules::with_laws(
        galaxy,
        content,
        sources,
        destination.as_str(),
        board,
        Some(state),
    )
    .can_enter(destination.as_str())
}

/// The choice kind for announcing a retreat.
pub const RETREAT_KIND: &str = "retreat";
/// The choice kind for picking where to retreat to.
pub const RETREAT_TO_KIND: &str = "retreat_to";

/// 78.7c: adjacent systems that hold this player's units or a planet they control, and no
/// other player's ships.
#[must_use]
pub fn eligible_retreats(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::eligible_retreats(state, sources, galaxy, player, system);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<Vec<SystemId>>;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::eligible_retreats",
        typed(),
        eligible_retreats_text(state, content, sources, galaxy, player, system)
    )
}

fn eligible_retreats_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    let types = catalogue(content, sources);
    // A retreat is a move: Sundered and the sever token close the wormholes it may not use.
    let galaxy = crate::factions::hooks_movement::galaxy_for_mover(state, player, galaxy);
    galaxy
        .adjacent(system.as_str())
        .into_iter()
        .filter(|adjacent| !galaxy.is_hyperlane(adjacent)) // not a system: never a retreat
        .map(SystemId::new)
        .filter(|adjacent| retreat_may_enter(state, content, sources, &galaxy, player, adjacent))
        .filter(|adjacent| {
            let board = state.system_state(adjacent);
            let enemy_ship = board.units.iter().any(|unit| {
                &unit.owner != player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_ship)
            });
            if enemy_ship {
                return false;
            }
            !board.units_of(player).is_empty() || board.controls_a_planet(player)
        })
        .collect()
}

/// Whether this combat round was declared a draw (Skilled Retreat).
///
/// Checked at both exits. The window concludes through `conclude` when a fight is fought out, but
/// a fleet that retreats leaves one side empty, and the next look at the system takes the
/// "fewer than two fleets" exit instead — so applying the draw in only one of them would make the
/// card work or not depending on which path the retreat happened to land on.
fn declared_draw(state: &GameState) -> bool {
    state.combat_draw_round == Some(state.combat_round_seq)
}

/// Where Skilled Retreat may send a fleet.
///
/// Deliberately not [`eligible_retreats`]. That enforces 78.7c, which also demands the
/// destination hold your units or a planet you control; the card asks only for "an adjacent
/// system that does not contain another player's ships". Reusing the stricter function would
/// quietly make the card weaker than it is printed.
#[must_use]
pub fn skilled_retreat_destinations(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    // Mahact hero: neither player can resolve an ability that would move their ships.
    if crate::factions::mahact_units::ship_movement_barred(state) {
        return Vec::new();
    }
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::open_retreat_destinations(state, sources, galaxy, player, system, false);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<Vec<SystemId>>;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::skilled_retreat_destinations",
        typed(),
        skilled_retreat_text(state, content, sources, galaxy, player, system)
    )
}

fn skilled_retreat_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    let types = catalogue(content, sources);
    galaxy
        .adjacent(system.as_str())
        .into_iter()
        .filter(|adjacent| !galaxy.is_hyperlane(adjacent)) // not a system: never a retreat
        .map(SystemId::new)
        .filter(|adjacent| retreat_may_enter(state, content, sources, galaxy, player, adjacent))
        .filter(|adjacent| {
            !state.system_state(adjacent).units.iter().any(|unit| {
                &unit.owner != player
                    && types
                        .get(unit.type_id.as_str())
                        .is_some_and(UnitType::is_ship)
            })
        })
        .collect()
}

/// Where Dark Energy Tap may send a fleet: adjacent systems that hold no other players'
/// units — ships in space *or* ground forces on planets, because the technology prints
/// "units", unlike 78.7c's "ships".
///
/// Deliberately the complement of [`eligible_retreats`], not a copy of it: the technology
/// waives 78.7c's own-presence clause ("even if you do not have units or control planets in
/// that system") and nothing else, so the two destination sets are united, not replaced. A
/// neighbour with an enemy garrison but no enemy ships is 78.7c-legal yet DET-illegal.
#[must_use]
pub fn det_retreat_destinations(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    #[cfg(feature = "typed-core")]
    let typed = || crate::core_combat::open_retreat_destinations(state, sources, galaxy, player, system, true);
    #[cfg(not(feature = "typed-core"))]
    let typed = || None::<Vec<SystemId>>;
    crate::dual_typed!(
        crate::dual::Area::Combat,
        "combat::det_retreat_destinations",
        typed(),
        det_retreat_text(state, content, sources, galaxy, player, system)
    )
}

fn det_retreat_text(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: &ti4_content::galaxy::Galaxy,
    player: &PlayerId,
    system: &SystemId,
) -> Vec<SystemId> {
    galaxy
        .adjacent(system.as_str())
        .into_iter()
        .filter(|adjacent| !galaxy.is_hyperlane(adjacent)) // not a system: never a retreat
        .map(SystemId::new)
        .filter(|adjacent| retreat_may_enter(state, content, sources, galaxy, player, adjacent))
        .filter(|adjacent| {
            let board = state.system_state(adjacent);
            !board.units.iter().any(|unit| &unit.owner != player)
                && !board
                    .planet_units
                    .values()
                    .any(|units| units.iter().any(|unit| &unit.owner != player))
        })
        .collect()
}

/// 78.7b: move a player's fleet to `destination`, and lose what it cannot carry.
///
/// Only ships with a move value leave under their own power. Anything consuming capacity comes
/// along only if there is room; the rest is destroyed, which is the cost of a retreat rather
/// than an oversight.
pub fn retreat_to(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    destination: &SystemId,
) -> usize {
    crate::dual_window!(
        crate::dual::Area::Driver,
        "combat::retreat_to",
        state,
        |s: &mut GameState| crate::dual::Run::new(retreat_to_string(s, content, sources, player, system, destination)),
        |s: &mut GameState, script: &crate::dual_window::Recording| {
            let args = crate::dual_window::RetreatArgs { player, system, destination };
            crate::dual_window::retreat_to(s, content, sources, args, script)
        }
    )
}

/// The string body of [`retreat_to`].
fn retreat_to_string(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
    destination: &SystemId,
) -> usize {
    let types = catalogue(content, sources);
    // Ground forces an Alastor brought into the battle stay on their planets and leave it with the
    // fleet: they are no longer in this combat.
    crate::factions::observe(
        state,
        crate::factions::Obs::ParticipantsCleared {
            system,
            player: Some(player),
        },
    );
    let mut own: Vec<Unit> = state
        .system_state(system)
        .units_of(player)
        .into_iter()
        .cloned()
        .collect();

    let planetary: Vec<Unit> =
        if planetary_maximum_participates(state, content, sources, player, system) {
            state
                .system_state(system)
                .planet_units
                .values()
                .flatten()
                .filter(|unit| &unit.owner == player && unit.type_id.as_str() == "naaz_voltron")
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
    own.extend(planetary.iter().cloned());

    let mut movers = Vec::new();
    let mut carried = Vec::new();
    for unit in own {
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        // A Floating Factory "can retreat as if it were a ship" (`moves_as_ship`).
        if (kind.is_ship() || kind.moves_as_ship()) && kind.move_value() > 0 {
            movers.push(unit);
        } else if kind.consumes_capacity() {
            carried.push(unit);
        }
    }
    let room: i64 = movers
        .iter()
        .filter_map(|unit| types.get(unit.type_id.as_str()))
        .map(UnitType::capacity)
        .sum();
    let room = usize::try_from(room.max(0)).unwrap_or(0);
    let stranded = carried.split_off(room.min(carried.len()));

    let mut leaving = movers;
    leaving.extend(carried);
    // The ordinary move primitive removes from the space pool only. Remove the participating
    // planet model explicitly before adding it at the retreat destination, so it is not duplicated.
    for unit in &planetary {
        if leaving.contains(unit) {
            remove_combat_ship(state, system, unit);
            // Leaving is not destroying: forget the galvanized-loss note the removal made.
            let _ =
                crate::factions::observe(
                    state,
                    crate::factions::Obs::ShipLostTaken {
                        system,
                        owner: &unit.owner,
                        unit: &unit.type_id,
                    },
                );
        }
    }
    let ships_leaving: Vec<Unit> = leaving
        .iter()
        .filter(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
        })
        .cloned()
        .collect();
    crate::factions::observe(
        state,
        crate::factions::Obs::Retreat {
            system,
            destination,
            owner: player,
            ships: &ships_leaving,
        },
    );
    state.move_units(system, destination, &leaving);
    // A dual-form unit is a ship only in the active system during its battle (Z-Grav Eidolon):
    // one that retreats takes its ground form again where it lands.
    crate::fleet::flip_to_ground_forms(
        state,
        content,
        sources,
        std::slice::from_ref(player),
        destination,
    );
    for unit in &stranded {
        remove_combat_ship(state, system, unit);
        if crate::supply::staging_enabled(state)
            && types
                .get(unit.type_id.as_str())
                .is_some_and(UnitType::is_ship)
        {
            state.pending_destructions.push((
                system.clone(),
                player.clone(),
                unit.type_id.clone(),
                "retreat_capacity".to_owned(),
                true,
            ));
        }
    }

    // 78.7d: a command token goes to the destination.
    state.system_mut(destination).place_token(player.clone());
    crate::tokens::stage_command_token_placed(
        state,
        player,
        destination,
        crate::factions::hooks_cards::TokenPool::Reinforcements,
    );
    stranded.len()
}

/// Run `run` with the [`crate::timing::TimingContext`] a faction hook needs, built from the
/// combat's own [`Resolving`]: the same state, table, dice and random stream, and the game's
/// event allocator when the combat has timing machinery.
///
/// Without it (a standalone combat, a test) the hook gets a detached allocator and no map. Hooks
/// never emit typed events themselves (they stage destructions, which the combat announces), so
/// the detached numbering is never used.
fn with_timing<T>(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    run: impl FnOnce(&mut crate::timing::TimingContext<'_>) -> T,
) -> T {
    let mut detached = crate::event::EventSequence::new();
    let (sequence, galaxy) = match ctx.timing.as_mut() {
        Some(handle) => (&mut *handle.sequence, handle.galaxy),
        None => (&mut detached, None),
    };
    let mut timing = crate::timing::TimingContext {
        state,
        content: ctx.content,
        sources: ctx.sources,
        table: &mut *ctx.table,
        dice: &mut *ctx.dice,
        rng: &mut *ctx.rng,
        event_sequence: sequence,
        galaxy,
    };
    run(&mut timing)
}

/// Hits still to be absorbed by one player.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    player: PlayerId,
    hits: usize,
    /// Whose roll produced these hits: the `producer` of the SUSTAIN DAMAGE use that
    /// cancels them, and what Direct Hit keys on.
    producer: PlayerId,
    /// 0.0.1: these hits must be assigned to non-fighter ships if able.
    non_fighters_only: bool,
    /// The producing player, not the owner, picks which ship each hit lands on (Devotion:
    /// "produce 1 hit and assign it to 1 of your opponent's ships"). The owner may still use
    /// SUSTAIN DAMAGE first.
    producer_assigns: bool,
}

/// Where an open space combat has reached.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stage {
    /// A round opens: its start-of-round events and, in round 1, the anti-fighter barrage
    /// (78.3, step 1), before anyone announces a retreat.
    Opening {
        round: u32,
    },
    /// 78.4 (step 2): retreats are announced after the barrage and before any combat dice, and
    /// the defender decides first.
    Announcing {
        round: u32,
        asking: PlayerId,
        announced: Vec<PlayerId>,
    },
    /// Retreats announced: roll, then queue both sides' hits.
    Rolling {
        round: u32,
    },
    /// Anti-fighter barrage scored a secret; the round resumes at the announcement once the
    /// scoring window has closed.
    RollingAfterBarrage {
        round: u32,
    },
    /// Offering Sustain Damage against the hits at the front of the queue (87.1).
    Sustaining {
        queue: Vec<Pending>,
        round: u32,
    },
    /// Assigning a casualty for one unabsorbed hit (78.6).
    Assigning {
        queue: Vec<Pending>,
        round: u32,
    },
    /// 78.7: those who announced are leaving, and must pick where.
    Retreating {
        round: u32,
        leaving: Vec<PlayerId>,
    },
    Done(CombatOutcome),
}

/// A space combat, resolvable one decision at a time (LRR 78).
///
/// The queue is what makes 78.6's simultaneity survive being stepped: both sides' hits are
/// computed and queued *before* either is absorbed, so a casualty can never reduce return fire
/// that had already been earned. Resolving one side to completion and then rolling the other —
/// the obvious way to write a resumable version — would break exactly that rule.
#[derive(Debug, Clone)]
pub struct CombatWindow {
    system: SystemId,
    attacker: PlayerId,
    defender: PlayerId,
    stage: Stage,
    /// The map, when the caller has one. Without it there is nowhere to retreat to.
    galaxy: Option<ti4_content::galaxy::Galaxy>,
    /// Players who announced a retreat this round and will leave once it ends (78.7).
    pending_retreats: Vec<PlayerId>,
    /// One before-assignment reaction window per seat and combat round, even when
    /// a card cancels some (but not all) of that seat's incoming hits.
    hit_reaction_offered: std::collections::BTreeSet<(u32, PlayerId)>,
    /// One identity spans the barrage and resolution of this combat (61.7).
    combat_occurrence: Option<FeatOccurrence>,
    /// A timing pause that the game driver has not yet opened a scoring window for.
    pending_scoring_occurrence: Option<FeatOccurrence>,
    /// Each side's ships already damaged when this round's hits were queued, by unit type: the
    /// ones Duranium Armor may repair, since they did not use SUSTAIN DAMAGE this round.
    damaged_before_round: std::collections::BTreeMap<PlayerId, Vec<String>>,
    /// "Roll Dice" step replays taken in the current round (The Thundarian); see
    /// [`MAX_ROLL_REPLAYS`].
    roll_replays: u32,
    /// Which stretch of hits the queue at the front is (module-produced hits resolve in queues
    /// of their own, before round 1's dice and after a round's hits).
    hit_phase: HitPhase,
    /// Each side's ships when the combat opened, by type, for the `destroyed` list of
    /// `SPACE_COMBAT_ENDED`.
    fleet_at_start: std::collections::BTreeMap<PlayerId, Vec<String>>,
    /// Who carried out a retreat, in order, with the ship types that arrived at the destination.
    fled: Vec<(PlayerId, Vec<String>)>,
    /// `SPACE_COMBAT_ENDED` has been emitted (or the window never fought).
    end_announced: bool,
    /// Round 1's start-of-combat hits were assigned; the next `open_round` resumes at the
    /// anti-fighter barrage instead of opening the round again.
    resuming_round: bool,
    /// Depth of `settle` (roll rounds settle inside themselves): only the outermost is compared as one.
    settle_depth: u32,
    /// The settle did something the composed core machine does not carry (see `settle_args`).
    machine_bail: bool,
    /// Why (the census reason of the bail-out of the core half; the first one of the settle).
    machine_bail_why: &'static str,
}

/// What the "Roll Dice" step leaves for [`CombatWindow::roll_round`]: whether the window settles afterwards
/// (not when a reroll ended the fight), and whether a module produced hits after the roll.
struct RollEnd {
    settle: bool,
    produced: bool,
}

/// What a stage helper tells `settle_stages`: go round the loop again, stop at a pending decision, or
/// answer on behalf of a neutral player.
enum Flow {
    Continue,
    Return,
    Resolve(ChoiceOption),
}

/// What the hit queue at the front of a [`CombatWindow`] is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitPhase {
    /// A round's own dice hits.
    Round,
    /// Hits modules produced "at the start of a space combat" (`CombatMoment::CombatStart`).
    CombatStart,
    /// Hits modules produced "after each space combat round" (`CombatMoment::RoundEnded`).
    RoundEnd,
}

impl CombatWindow {
    /// Open a combat, or finish immediately if fewer than two players have ships (78.1).
    ///
    /// The roles are anchored to the active player (LRR 13 / 29.1), not to seating order:
    /// whoever is active when the combat opens is the attacker.
    #[must_use]
    pub fn new(
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        system: &SystemId,
    ) -> Self {
        let mut sides = combatants(state, content, sources, system);
        if sides.len() == 2
            && state
                .active
                .as_ref()
                .is_some_and(|active| &sides[1] == active)
        {
            // LRR 13: during combat the active player is the attacker. `combatants` lists
            // the sides in seating order, so move the active seat to the front when it is
            // the second of the two; a combat with no active player (a test state the
            // rules never produce) keeps the seating order.
            sides.swap(0, 1);
        }
        let [attacker, defender] = sides.as_slice() else {
            return Self {
                system: system.clone(),
                attacker: PlayerId::new(""),
                defender: PlayerId::new(""),
                stage: Stage::Done(CombatOutcome {
                    winner: if declared_draw(state) {
                        None
                    } else {
                        sides.first().cloned()
                    },
                    rounds: 0,
                }),
                galaxy: None,
                pending_retreats: Vec::new(),
                hit_reaction_offered: Default::default(),
                combat_occurrence: None,
                pending_scoring_occurrence: None,
                damaged_before_round: std::collections::BTreeMap::new(),
                roll_replays: 0,
                hit_phase: HitPhase::Round,
                fleet_at_start: std::collections::BTreeMap::new(),
                fled: Vec::new(),
                // No fight happened, so there is no end to announce.
                end_announced: true,
                resuming_round: false,
                settle_depth: 0,
                machine_bail: false,
                machine_bail_why: "",
            };
        };
        let fleet_at_start = [attacker, defender]
            .into_iter()
            .map(|side| {
                let mut fleet: Vec<String> = ships_of(state, content, sources, side, system)
                    .iter()
                    .map(|unit| unit.type_id.to_string())
                    .collect();
                fleet.sort();
                (side.clone(), fleet)
            })
            .collect();
        Self {
            system: system.clone(),
            attacker: attacker.clone(),
            defender: defender.clone(),
            stage: Stage::Opening { round: 1 },
            galaxy: None,
            pending_retreats: Vec::new(),
            hit_reaction_offered: Default::default(),
            combat_occurrence: None,
            pending_scoring_occurrence: None,
            damaged_before_round: std::collections::BTreeMap::new(),
            roll_replays: 0,
            hit_phase: HitPhase::Round,
            fleet_at_start,
            fled: Vec::new(),
            end_announced: false,
            resuming_round: false,
            settle_depth: 0,
            machine_bail: false,
            machine_bail_why: "",
        }
    }

    /// The settle did something the composed core machine does not carry (`why`: the census reason; the first one stays).
    fn bail_machine(&mut self, why: &'static str) {
        if !self.machine_bail {
            self.machine_bail_why = why;
        }
        self.machine_bail = true;
    }

    /// Give the fight a map, which is what makes retreat possible (78.7c).
    #[must_use]
    pub fn with_galaxy(mut self, galaxy: ti4_content::galaxy::Galaxy) -> Self {
        self.galaxy = Some(galaxy);
        self
    }

    /// Where this player could retreat to right now.
    fn retreats(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
    ) -> Vec<SystemId> {
        // Neutral units rule 6: they cannot retreat, so they are never asked to announce one.
        if crate::neutral_units::is_neutral(player) && !crate::neutral_units::may_retreat() {
            return Vec::new();
        }
        // Intercept: "your opponent cannot retreat during this round of space combat." A seat with
        // nowhere to go is not asked (78.4c), so barring is expressed as having nowhere to go.
        if retreat_barred(state, player) {
            return Vec::new();
        }
        // Mahact hero: "neither player can retreat" from the combat it starts.
        if crate::factions::mahact_units::ship_movement_barred(state) {
            return Vec::new();
        }
        self.galaxy.as_ref().map_or_else(Vec::new, |galaxy| {
            let mut spots =
                eligible_retreats(state, content, sources, galaxy, player, &self.system);
            // Dark Energy Tap: "Your ships can retreat into adjacent systems that do not
            // contain other players' units, even if you do not have units or control planets
            // in that system." The holder alone gets the union with the technology's looser
            // set; everyone else still retreats by 78.7c only.
            if crate::technology::owns_det(state, player) {
                for spot in
                    det_retreat_destinations(state, content, sources, galaxy, player, &self.system)
                {
                    if !spots.contains(&spot) {
                        spots.push(spot);
                    }
                }
            }
            spots
        })
    }

    /// The result, once the fight is over.
    #[must_use]
    pub fn outcome(&self) -> Option<CombatOutcome> {
        match &self.stage {
            Stage::Done(outcome) => Some(outcome.clone()),
            _ => None,
        }
    }

    #[must_use]
    pub fn take_scoring_occurrence(&mut self) -> Option<FeatOccurrence> {
        self.pending_scoring_occurrence.take()
    }

    #[must_use]
    pub fn combat_occurrence(&self) -> Option<FeatOccurrence> {
        self.combat_occurrence
    }

    fn ensure_combat_occurrence(&mut self, state: &mut GameState) -> FeatOccurrence {
        *self
            .combat_occurrence
            .get_or_insert_with(|| state.begin_feat_occurrence())
    }

    fn over(&self, state: &GameState, content: &ContentStore, sources: SourceSet) -> bool {
        finished(
            state,
            content,
            sources,
            &self.system,
            &self.attacker,
            &self.defender,
        )
    }

    fn conclude(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        rounds: u32,
    ) -> Stage {
        // Skilled Retreat ends the combat in a draw. Counting ships would hand the win to
        // whoever stayed, which is the opposite of what the card says and would also score
        // "win a space combat" for a fight nobody won.
        Stage::Done(CombatOutcome {
            winner: if declared_draw(state) {
                None
            } else {
                winner(
                    state,
                    content,
                    sources,
                    &self.system,
                    &self.attacker,
                    &self.defender,
                )
            },
            rounds,
        })
    }

    /// Units of the player at the front of the queue that could still sustain a hit.
    fn sustainers(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        player: &PlayerId,
    ) -> Vec<(String, Unit)> {
        let types = catalogue(content, sources);
        let mut found: Vec<(String, Unit)> = state
            .system_state(&self.system)
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                types.get(unit.type_id.as_str()).is_some_and(|kind| {
                    sustains_in_space(
                        state,
                        content,
                        sources,
                        &self.system,
                        player,
                        unit,
                        kind,
                        true,
                    )
                })
            })
            .map(|(index, unit)| (index.to_string(), unit.clone()))
            .collect();
        let mut distinct_fresh: std::collections::BTreeMap<ti4_model::id::PlanetId, Vec<Unit>> =
            std::collections::BTreeMap::new();
        for (planet, index, unit) in
            planet_combatants(state, content, sources, player, &self.system)
        {
            if !planet_unit_sustains(state, content, sources, &self.system, player, &unit, true) {
                continue;
            }
            let seen_here = distinct_fresh.entry(planet.clone()).or_default();
            if seen_here.contains(&unit) {
                continue;
            }
            let location = if unit.type_id.as_str() != "naaz_voltron"
                || (crate::supply::staging_enabled(state) && !seen_here.is_empty())
            {
                format!("planet:{planet}:variant:{index}")
            } else {
                format!("planet:{planet}")
            };
            seen_here.push(unit.clone());
            found.push((location, unit));
        }
        found
    }

    /// The answer the neutral-unit rules give at a pending sustain or casualty decision.
    ///
    /// Nobody decides for neutral units. Rule 5: they use every ability they can, so a sustain is
    /// always taken. Rule 7: a hit is assigned to the unit lowest on the neutral reference card.
    /// Answering through [`Window::resolve`] keeps every effect of the ordinary path (events,
    /// hit counting) rather than re-implementing it for one owner.
    fn neutral_answer(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Option<ChoiceOption> {
        let choice = self.pending_choice(state, content, sources)?;
        match &self.stage {
            Stage::Sustaining { .. } => choice
                .options
                .iter()
                .find(|option| !option.is_decline())
                .cloned(),
            Stage::Assigning { queue, .. } => {
                let front = queue.first()?;
                let units = ships_of(state, content, sources, &front.player, &self.system);
                let pool: Vec<Unit> = self
                    .casualty_candidates(state, content, sources, front)
                    .into_iter()
                    .map(|(_, unit)| unit)
                    .collect();
                let order = crate::neutral_units::roster(content, sources);
                let doomed = crate::neutral_units::next_casualty(&order, &pool, |_| true)
                    .or_else(|| pool.first())?;
                choice
                    .options
                    .iter()
                    .find(|option| {
                        option
                            .id
                            .strip_prefix("destroy|")
                            .and_then(|id| units.get(id.parse::<usize>().ok()?))
                            .is_some_and(|unit| {
                                unit.type_id == doomed.type_id
                                    && unit.sustained_damage == doomed.sustained_damage
                            })
                    })
                    .or_else(|| choice.options.first())
                    .cloned()
            }
            _ => None,
        }
    }

    /// The ships a pending hit may destroy, with their `ships_of` index: every ship, or only the
    /// non-fighters when 0.0.1 forced the hit and a non-fighter is left.
    fn casualty_candidates(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        front: &Pending,
    ) -> Vec<(String, Unit)> {
        let types = catalogue(content, sources);
        let units: Vec<(String, Unit)> =
            ships_of(state, content, sources, &front.player, &self.system)
                .into_iter()
                .enumerate()
                .map(|(index, unit)| (index.to_string(), unit))
                .collect();
        let non_fighter = |unit: &Unit| {
            types
                .get(unit.type_id.as_str())
                .is_none_or(|kind| !kind.is_fighter())
        };
        if front.non_fighters_only && units.iter().any(|(_, unit)| non_fighter(unit)) {
            units
                .into_iter()
                .filter(|(_, unit)| non_fighter(unit))
                .collect()
        } else {
            units
        }
    }

    /// Carry out a retreat (78.7b), noting which ships arrived for `SPACE_COMBAT_ENDED`.
    fn retreat(
        &mut self,
        state: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        ctx: &mut Resolving<'_>,
        player: &PlayerId,
        destination: &SystemId,
    ) {
        let there = |state: &GameState| -> Vec<String> {
            ships_of(state, content, sources, player, destination)
                .iter()
                .map(|unit| unit.type_id.to_string())
                .collect()
        };
        let mut arrived = there(state);
        let before = std::mem::take(&mut arrived);
        retreat_to(state, content, sources, player, &self.system, destination);
        announce_staged_destructions(state, ctx);
        let mut now = there(state);
        for kind in before {
            if let Some(index) = now.iter().position(|held| *held == kind) {
                now.remove(index);
            }
        }
        // Last Dispatch: "When this unit retreats". Only a flagship that arrived counts.
        if crate::factions::ralnel::flagship_among(state, player, &now) {
            let mut payload = std::collections::BTreeMap::new();
            payload.insert("system".to_owned(), self.system.to_string().into());
            payload.insert("player".to_owned(), player.to_string().into());
            payload.insert("destination".to_owned(), destination.to_string().into());
            let _ = ctx.emit(state, "FLAGSHIP_RETREATED", payload);
            self.bail_machine("a Last Dispatch flagship retreat (Ral Nel, non-lobby)");
        }
        self.fled.push((player.clone(), now));
    }

    /// Ask every module for the hits it produces for both participants at `moment`
    /// (`hooks_combat::produced_hits`), announcing any ship a module destroyed to pay for them.
    /// `[attacker, defender]`.
    fn produce_hits(
        &self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
        moment: CombatMoment,
    ) -> Result<[ProducedHits; 2], CombatError> {
        let sources = ctx.sources;
        let announce = ctx.timing.is_some();
        let reproducible = !ctx.dice.is_preloaded() && !ctx.rng.sync_pending() && ctx.rng.seed() == state.rng_seed;
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        let rng_before = ctx.rng.positions();
        let history_start = ctx.dice.history().len();
        let mut outcome = None;
        let ctx = std::cell::RefCell::new(ctx);
        let failed = std::cell::Cell::new(false);
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::produce_hits",
            state,
            |s: &mut GameState| {
                let mut ctx = ctx.borrow_mut();
                let result = self.produce_hits_string(s, &mut **ctx, round, moment);
                failed.set(result.is_err());
                let value: crate::dual_window::ProduceOut = match &result {
                    Ok(made) => (
                        [(made[0].any_ship, made[0].producer_assigned, made[0].non_fighter), (made[1].any_ship, made[1].producer_assigned, made[1].non_fighter)],
                        ctx.dice
                            .history()
                            .iter()
                            .skip(history_start)
                            .filter(|roll| roll.reason == "ambush")
                            .map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone()))
                            .collect(),
                        ctx.rng.positions(),
                    ),
                    Err(_) => ([(0, 0, 0); 2], Vec::new(), ctx.rng.positions()),
                };
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                let args = crate::dual_window::ProduceArgs { attacker: &attacker, defender: &defender, system: &system, at_start: moment == CombatMoment::CombatStart, reproducible, announce };
                crate::dual_window::produce_hits(s, sources, args, &rng_before, script)
            }
        );
        outcome.unwrap_or_else(|| Ok([ProducedHits::NONE; 2]))
    }

    /// The string body of [`Self::produce_hits`].
    fn produce_hits_string(
        &self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
        moment: CombatMoment,
    ) -> Result<[ProducedHits; 2], CombatError> {
        let mut produced = [ProducedHits::NONE; 2];
        let sides = [
            (&self.attacker, &self.defender),
            (&self.defender, &self.attacker),
        ];
        for (slot, (player, opponent)) in produced.iter_mut().zip(sides) {
            let site = HitSite {
                player,
                opponent,
                system: &self.system,
                round,
                moment,
            };
            let made = with_timing(state, ctx, |timing| {
                crate::factions::hooks_combat::produced_hits(timing, &site)
            });
            // Whatever the hook staged before failing is announced; the error then stops the
            // step like any other combat choice failure.
            announce_staged_destructions(state, ctx);
            *slot = made?;
        }
        Ok(produced)
    }

    /// The hits queued for assignment from what each side produced, `[attacker, defender]`:
    /// forced non-fighter hits first, like a round's own, then the producer-assigned ones.
    fn produced_queue(&self, produced: [ProducedHits; 2]) -> Vec<Pending> {
        let mut queue: Vec<Pending> = [
            (
                &self.defender,
                produced[0].non_fighter,
                &self.attacker,
                true,
            ),
            (&self.defender, produced[0].any_ship, &self.attacker, false),
            (
                &self.attacker,
                produced[1].non_fighter,
                &self.defender,
                true,
            ),
            (&self.attacker, produced[1].any_ship, &self.defender, false),
        ]
        .into_iter()
        .filter(|(_, hits, _, _)| *hits > 0)
        .map(|(player, hits, producer, non_fighters_only)| Pending {
            player: player.clone(),
            hits,
            producer: producer.clone(),
            non_fighters_only,
            producer_assigns: false,
        })
        .collect();
        queue.extend(self.producer_assigned_queue(&produced));
        queue
    }

    /// The producer-assigned hits of `produced` (`[attacker, defender]`).
    fn producer_assigned_queue(&self, produced: &[ProducedHits; 2]) -> Vec<Pending> {
        [
            (
                &self.defender,
                produced[0].producer_assigned,
                &self.attacker,
            ),
            (
                &self.attacker,
                produced[1].producer_assigned,
                &self.defender,
            ),
        ]
        .into_iter()
        .filter(|(_, hits, _)| *hits > 0)
        .map(|(player, hits, producer)| Pending {
            player: player.clone(),
            hits,
            producer: producer.clone(),
            non_fighters_only: false,
            producer_assigns: true,
        })
        .collect()
    }

    /// "At the start of a space combat" hit production: after Assault Cannon (which is the same
    /// moment), before anti-fighter barrage. Returns whether it queued hits, which then resolve
    /// before the round resumes at the barrage ([`HitPhase::CombatStart`]).
    fn open_start_hits(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<bool, CombatError> {
        let produced = self.produce_hits(state, ctx, round, CombatMoment::CombatStart)?;
        let queue = self.produced_queue(produced);
        if queue.is_empty() {
            return Ok(false);
        }
        self.hit_phase = HitPhase::CombatStart;
        self.stage = Stage::Sustaining { queue, round };
        Ok(true)
    }

    /// "After a round of space combat": hits modules produce once the round, retreats included,
    /// is over. Not asked once the fight is already decided. Returns whether a queue was opened
    /// ([`HitPhase::RoundEnd`]).
    fn open_round_end_hits(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<bool, CombatError> {
        if self.over(state, ctx.content, ctx.sources) {
            return Ok(false);
        }
        let produced = self.produce_hits(state, ctx, round, CombatMoment::RoundEnded)?;
        let queue = self.produced_queue(produced);
        if queue.is_empty() {
            return Ok(false);
        }
        self.hit_phase = HitPhase::RoundEnd;
        // Hits produced after the round are their own "before you assign hits" moment, so the
        // once-per-round guard on the ordinary assignment must not swallow their announcement.
        self.hit_reaction_offered.retain(|(seen, _)| *seen != round);
        self.stage = Stage::Sustaining { queue, round };
        Ok(true)
    }

    /// Announce a finished round as `SPACE_COMBAT_ROUND_ENDED` (`system`, `round`, `attacker`,
    /// `defender`), then announce whatever an effect on it destroyed.
    fn announce_round_ended(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32) {
        self.hit_phase = HitPhase::Round;
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("round".to_owned(), i64::from(round).into());
        payload.insert("attacker".to_owned(), self.attacker.to_string().into());
        payload.insert("defender".to_owned(), self.defender.to_string().into());
        let _ = ctx.emit(state, "SPACE_COMBAT_ROUND_ENDED", payload);
        announce_staged_destructions(state, ctx);
    }

    /// After a round is announced over: the next round, or the end of the fight.
    fn next_round_or_end(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        round: u32,
    ) -> Stage {
        if self.over(state, content, sources) || round >= MAX_ROUNDS {
            self.conclude(state, content, sources, round)
        } else {
            Stage::Opening { round: round + 1 }
        }
    }

    /// Announce the end of the fight once, as `SPACE_COMBAT_ENDED`.
    ///
    /// Payload: `system`, `attacker`, `defender`, `rounds` (int), `winner` (absent for a draw or a
    /// mutual wipe-out), `losers` (array: the sides that did not win, empty without a winner),
    /// `reason` (`"destruction"`, `"retreat"`, `"draw"` for Skilled Retreat, or `"round_limit"`),
    /// `retreated` (array of players who left) and `destroyed` (array of `{player, unit}`: every
    /// ship either side had when the fight opened that is neither still there nor arrived with a
    /// retreating fleet, which includes fighters lost to barrage and ships an effect destroyed).
    fn announce_end(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) {
        let Stage::Done(outcome) = &self.stage else {
            return;
        };
        if std::mem::replace(&mut self.end_announced, true) {
            return;
        }
        let (content, sources) = (ctx.content, ctx.sources);
        let outcome = outcome.clone();
        let sides = [self.attacker.clone(), self.defender.clone()];
        let payload = self.end_payload(state, content, sources, &outcome);
        // The rest runs on `driver=dual` as a core window too (`rules::combat::end_announce`); the string body
        // notes what the window does not carry.
        let bail: std::cell::Cell<Option<&'static str>> = std::cell::Cell::new((!state.pending_destructions.is_empty()).then_some("staged destructions pending at the end of the fight"));
        let marks_changed = std::cell::Cell::new(false);
        let facts = crate::dual_window::EndArgs {
            attacker: &self.attacker,
            defender: &self.defender,
            system: &self.system,
            rounds: outcome.rounds,
            winner: outcome.winner.as_ref(),
            declared_draw: declared_draw(state),
            fled: &self.fled,
            at_start: &self.fleet_at_start,
            announce: ctx.timing.is_some(),
        };
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::announce_end",
            state,
            |state: &mut GameState| {
                let marks = state.faction_marks.clone();
        // "At the end of a space battle in the active system, flip this card" (Z-Grav Eidolon):
        // the survivors in this system's space area return to their ground form. Measured after
        // the losses above, which count ships, so a flipped mech is not reported destroyed.
        crate::fleet::flip_to_ground_forms(state, content, sources, &sides, &self.system);
        let _ = ctx.emit(state, "SPACE_COMBAT_ENDED", payload.clone());
        crate::factions::observe(
            state,
            crate::factions::Obs::ParticipantsCleared {
                system: &self.system,
                player: None,
            },
        );
        crate::factions::observe(
            state,
            crate::factions::Obs::RetreatsForgotten {
                system: &self.system,
            },
        );
        // A mobile space dock left in a space area with another player's ships is destroyed
        // (Floating Factory); checked when the combat that could have cleared them ends.
        if !crate::production::destroy_blockaded_mobile_docks(state, ctx.content, ctx.sources, &self.system).is_empty() {
            bail.set(bail.get().or(Some("a blockaded mobile dock destroyed at the end of the fight (Floating Factory)")));
        }
        // The notes the announcement leaves (Cabal's marks, a staged event) are the core's when the string run had
        // nested activity: the abilities of the table write them, and the state comparison tells.
        // (Technological Singularity's combat mark, `nekro:ts:`, is one of those: the ability that closes it is in the table.)
        let own = |marks: &std::collections::BTreeMap<String, String>| marks.iter().filter(|(k, _)| !k.starts_with("nekro:ts:")).map(|(k, v)| (k.clone(), v.clone())).collect::<Vec<_>>();
        marks_changed.set(own(&state.faction_marks) != own(&marks));
        announce_staged_destructions(state, ctx);
                crate::dual::Run::new(true)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if let Some(why) = bail.get() {
                    crate::dual::note_bail_here(why);
                    return None;
                }
                if marks_changed.get() && !script.nested {
                    crate::dual::note_bail_here("notes of the end announcement (Cabal marks, a staged event) written by the string");
                    return None;
                }
                crate::dual_window::announce_end(s, content, sources, facts, script)
            }
        );
        if let Some(why) = bail.get() {
            self.bail_machine(why);
        } else if marks_changed.get() {
            self.bail_machine("notes of the end announcement (Cabal marks, a staged event) written by the string");
        }
    }

    /// The `SPACE_COMBAT_ENDED` payload (see [`Self::announce_end`]): pure over the position, so the
    /// winner, reason and fleet diff can be ported on their own.
    fn end_payload(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        outcome: &CombatOutcome,
    ) -> std::collections::BTreeMap<String, serde_json::Value> {
        let sides = [self.attacker.clone(), self.defender.clone()];
        let reason = if !self.fled.is_empty() {
            "retreat"
        } else if declared_draw(state) {
            "draw"
        } else if self.over(state, content, sources) {
            "destruction"
        } else {
            "round_limit"
        };
        let mut destroyed = Vec::new();
        for side in &sides {
            let mut pool: Vec<String> = ships_of(state, content, sources, side, &self.system)
                .iter()
                .map(|unit| unit.type_id.to_string())
                .collect();
            for (who, arrived) in &self.fled {
                if who == side {
                    pool.extend(arrived.iter().cloned());
                }
            }
            for kind in self.fleet_at_start.get(side).into_iter().flatten() {
                if let Some(index) = pool.iter().position(|held| held == kind) {
                    pool.remove(index);
                } else {
                    destroyed.push(serde_json::json!({"player": side.to_string(), "unit": kind}));
                }
            }
        }
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("attacker".to_owned(), self.attacker.to_string().into());
        payload.insert("defender".to_owned(), self.defender.to_string().into());
        payload.insert("rounds".to_owned(), i64::from(outcome.rounds).into());
        if let Some(winner) = &outcome.winner {
            payload.insert("winner".to_owned(), winner.to_string().into());
        }
        payload.insert(
            "losers".to_owned(),
            sides
                .iter()
                .filter(|side| outcome.winner.as_ref().is_some_and(|won| won != *side))
                .map(|side| serde_json::Value::String(side.to_string()))
                .collect::<Vec<_>>()
                .into(),
        );
        payload.insert("reason".to_owned(), reason.into());
        payload.insert(
            "retreated".to_owned(),
            self.fled
                .iter()
                .map(|(who, _)| serde_json::Value::String(who.to_string()))
                .collect::<Vec<_>>()
                .into(),
        );
        payload.insert("destroyed".to_owned(), destroyed.clone().into());
        self.check_end_report(state, content, sources, &outcome, reason, &sides, &destroyed);
        payload
    }

    /// Dual check of the end report (`rules::combat::end`): reason, losers, retreats and lost ships.
    fn check_end_report(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
        outcome: &CombatOutcome,
        reason: &str,
        sides: &[PlayerId; 2],
        destroyed: &[serde_json::Value],
    ) {
        if crate::dual::backend(crate::dual::Area::Combat) != crate::dual::Backend::Dual {
            return;
        }
        let string = || {
            let text = |v: &serde_json::Value, key: &str| v[key].as_str().unwrap_or_default().to_owned();
            (
                reason.to_owned(),
                sides.iter().filter(|side| outcome.winner.as_ref().is_some_and(|won| won != *side)).map(ToString::to_string).collect::<Vec<_>>(),
                self.fled.iter().map(|(who, _)| who.to_string()).collect::<Vec<_>>(),
                destroyed.iter().map(|d| (text(d, "player"), text(d, "unit"))).collect::<Vec<_>>(),
            )
        };
        #[cfg(feature = "typed-core")]
        let typed = || {
            let fled: Vec<(PlayerId, Vec<String>)> = self.fled.clone();
            let start: Vec<(PlayerId, Vec<String>)> = self.fleet_at_start.iter().map(|(p, f)| (p.clone(), f.clone())).collect();
            crate::core_combat::CombatView::of(state, content, sources)?.end_report(
                &self.system,
                [&self.attacker, &self.defender],
                outcome.winner.as_ref(),
                declared_draw(state),
                &fled,
                &start,
            )
        };
        #[cfg(not(feature = "typed-core"))]
        let typed = || None::<(String, Vec<String>, Vec<String>, Vec<(String, String)>)>;
        let _ = crate::dual::run_dual_typed(crate::dual::Area::Combat, "combat::end_report", typed, string);
    }

    /// Roll a round and queue both sides' hits, or finish.
    #[allow(
        clippy::too_many_lines,
        reason = "one windowed step per side, read as a table"
    )]
    fn open_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        let resumed = std::mem::take(&mut self.resuming_round);
        if !resumed {
            self.begin_round(state, ctx, round)?;
        }

        if round == 1 {
            // Assault Cannon: "At the start of a space combat in a system that contains 3 or more
            // of your non-fighter ships, your opponent must destroy 1 of their non-fighter ships."
            // Both sides' eligibility is read before either loss, so the two resolve as one moment.
            if !resumed {
                self.assault_cannon(state, ctx)?;
                if self.over(state, content, sources) {
                    self.stage = self.conclude(state, content, sources, round);
                    return Ok(());
                }
                // "At the start of a space combat" is one moment with Assault Cannon, and it
                // precedes anti-fighter barrage: modules' hits are produced and assigned here, and
                // the round resumes at the barrage once they are.
                if self.open_start_hits(state, ctx, round)? {
                    return Ok(());
                }
            }
            let occurrence = self.ensure_combat_occurrence(state);
            let feat_players = self.barrage_step(state, ctx, round)?;
            if !feat_players.is_empty() {
                self.pending_scoring_occurrence = Some(occurrence);
                self.stage = Stage::RollingAfterBarrage { round };
                return Ok(());
            }
            if self.over(state, content, sources) {
                // 78.3a: a barrage can end the fight before any combat die is rolled.
                self.stage = self.conclude(state, content, sources, round);
                return Ok(());
            }
            state.combat_presentation.phase = "pre_roll".to_owned();
        }
        state.combat_presentation.round_start =
            ships_of(state, content, sources, &self.attacker, &self.system)
                .into_iter()
                .chain(ships_of(
                    state,
                    content,
                    sources,
                    &self.defender,
                    &self.system,
                ))
                .collect();
        self.stage = Stage::Announcing {
            round,
            asking: self.defender.clone(),
            announced: Vec::new(),
        };
        Ok(())
    }

    /// The opening of a round that is not resumed: the round counter, the presentation, the round's
    /// announcements and the faction offers made before any dice (string `open_round`, first part).
    ///
    /// On `driver=dual` the step also runs as a core window (`rules::combat::begin_round`) for the positions
    /// `dual_window::round_opening_unported` does not exclude.
    fn begin_round(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        let failed = std::cell::Cell::new(false);
        let failed_why = std::cell::Cell::new("");
        let mut outcome = None;
        let announce = ctx.timing.is_some();
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::begin_round",
            state,
            |s: &mut GameState| {
                let result = self.begin_round_string(s, ctx, round);
                failed.set(result.is_err());
                failed_why.set("the string opening ended in an error");
                outcome = Some(result);
                crate::dual::Run::new(!failed.get())
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    crate::dual::note_bail_here(failed_why.get());
                    return None;
                }
                let args = crate::dual_window::BeginRoundArgs {
                    attacker: &attacker,
                    defender: &defender,
                    system: &system,
                    round,
                    announce,
                };
                crate::dual_window::begin_round(s, content, sources, args, script)
            }
        );
        outcome.unwrap_or(Ok(()))
    }

    /// The string body of [`Self::begin_round`].
    fn begin_round_string(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        state.combat_round_seq = state.combat_round_seq.saturating_add(1);
        state.combat_presentation.phase = "pre_roll".to_owned();
        state.combat_presentation.round = round;
        state.combat_presentation.remaining_hits.clear();
        state.combat_round_hits.clear();
        state.combat_round_dice.clear();
        state.combat_presentation.round_start =
            ships_of(state, content, sources, &self.attacker, &self.system)
                .into_iter()
                .chain(ships_of(
                    state,
                    content,
                    sources,
                    &self.defender,
                    &self.system,
                ))
                .collect();
        if round == 1 {
            state.combat_presentation.barrage_start =
                state.combat_presentation.round_start.clone();
            state.combat_presentation.barrage_hits.clear();
            state.combat_presentation.barrage_dice.clear();
        }

        // Announced before anything is rolled, because eight action cards read "at the start of
        // a combat round" and Morale Boost scopes its bonus to `combat_round_seq`. Emitting after
        // the round would apply it to the next one.
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        // Who is fighting, so the round's card window can admit only them: every card that
        // hooks it acts on the player's own units in this combat.
        payload.insert("attacker".to_owned(), self.attacker.to_string().into());
        payload.insert("defender".to_owned(), self.defender.to_string().into());
        payload.insert("round".to_owned(), i64::from(round).into());
        if round == 1 {
            // "At the start of a space combat", a ground-form dual-form unit (Naaz Eidolon)
            // in the space area of the active system takes its ship form, so it fights. It
            // is added to the fleet the end announcement measures losses against.
            let sides = [self.attacker.clone(), self.defender.clone()];
            for (owner, form) in
                crate::fleet::flip_to_ship_forms(state, content, sources, &sides, &self.system)
            {
                if let Some(fleet) = self.fleet_at_start.get_mut(&owner) {
                    fleet.push(form.to_string());
                    fleet.sort();
                }
            }
            crate::diplomacy::evaluate_event(
                state,
                &crate::diplomacy::DiplomacyEventContext::HostileEngagement {
                    attacker: self.attacker.clone(),
                    victim: self.defender.clone(),
                    activation_seq: state.activation_seq,
                },
            )
            .expect("validated attack promises settle deterministically");
            let mut opening = payload.clone();
            opening.insert("player".to_owned(), self.attacker.to_string().into());
            let _ = ctx.emit(state, "SPACE_COMBAT_STARTED", opening);
        }
        if crate::factions::borrowed_round_agents::has_round_copy(state, content, "letnevagent")
            || crate::factions::borrowed_round_agents::has_owner_space_round_agent(state)
        {
            payload.insert(
                "round_seq".to_owned(),
                i64::from(state.combat_round_seq).into(),
            );
            ctx.emit(state, "COMBAT_ROUND_STARTED", payload)?;
        } else {
            let _ = ctx.emit(state, "COMBAT_ROUND_STARTED", payload);
        }

        // Faction offers made at the round's opening window, before any dice: Letnev pays for
        // Munitions Reserves here, and `reroll_munitions_misses` reads the marker below.
        for side in [self.attacker.clone(), self.defender.clone()] {
            crate::faction_abilities::space_combat_round_started(
                state, content, sources, ctx.table, &side,
            );
            repair_self_repairing(state, &self.system, &side);
        }
        Ok(())
    }

    /// Assault Cannon (round 1, before the barrage): "your opponent must destroy 1 of their non-fighter
    /// ships" for each holder of 3 or more.
    ///
    /// On `driver=dual` the whole step also runs as a core window (`rules::combat::assault_cannon`).
    fn assault_cannon(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) -> Result<(), CombatError> {
        let sources = ctx.sources;
        let failed = std::cell::Cell::new(false);
        let mut outcome = None;
        let announce = ctx.timing.is_some();
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::assault_cannon",
            state,
            |s: &mut GameState| {
                let result = self.assault_cannon_string(s, ctx);
                failed.set(result.is_err());
                outcome = Some(result);
                crate::dual::Run::new(!failed.get())
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                let args = crate::dual_window::AssaultCannonArgs {
                    attacker: &attacker,
                    defender: &defender,
                    system: &system,
                    announce,
                };
                crate::dual_window::assault_cannon(s, sources, args, script)
            }
        );
        outcome.unwrap_or(Ok(()))
    }

    /// The string body of [`Self::assault_cannon`].
    fn assault_cannon_string(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        let firing: Vec<(PlayerId, PlayerId)> = [
            (self.attacker.clone(), self.defender.clone()),
            (self.defender.clone(), self.attacker.clone()),
        ]
        .into_iter()
        .filter(|(holder, _)| {
            state.player(holder).is_some_and(|seat| {
                seat.technologies
                    .contains(ti4_model::id::text(&"asc"))
            }) && non_fighter_ships(state, content, sources, holder, &self.system).len()
                >= 3
        })
        .collect();
        for (holder, victim) in firing {
            let targets = non_fighter_ships(state, content, sources, &victim, &self.system);
            if targets.is_empty() {
                continue;
            }
            let casualty = choose_casualty_as(
                state,
                content,
                sources,
                self.galaxy.as_ref(),
                ctx.table,
                &victim,
                &targets,
                &DecisionSource::Content("asc".to_owned()),
                "assault_cannon_destroy",
                Some(&self.system),
                HitDetail {
                    cause: HitCause::AssaultCannon,
                    destroy: true,
                    restriction: HitRestriction::NonFighter,
                    producer: Some(holder),
                },
            )?;
            remove_combat_ship(state, &self.system, &casualty);
            announce_ship_destroyed_type(
                state,
                ctx,
                &self.system,
                &victim,
                &casualty.type_id,
                "technology:assault_cannon",
                true,
            );
        }
        Ok(())
    }

    /// Both anti-fighter barrages (78.3): rolled one side at a time with its reroll windows, then applied.
    /// The players that scored a barrage feat.
    ///
    /// On `driver=dual` the step also runs as a core window (`rules::combat::barrage`) for the positions
    /// `dual_window::barrage_unported` does not exclude.
    fn barrage_step(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32) -> Result<Vec<PlayerId>, CombatError> {
        let sources = ctx.sources;
        let failed = std::cell::Cell::new(false);
        let mut outcome = None;
        let announce = ctx.timing.is_some();
        let reproducible = crate::product::reproducible(ctx.dice.is_preloaded(), ctx.rng.sync_pending(), ctx.rng.seed(), state.rng_seed); // real games always match: Game::new and Game::from_state seed the rng from state.rng_seed (game.rs); only hand-built test rigs differ
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        let occurrence = self.ensure_combat_occurrence(state).0;
        let staging = crate::supply::staging_enabled(state);
        let probe = crate::dual_window::BarrageArgs {
            attacker: &attacker,
            defender: &defender,
            system: &system,
            round,
            occurrence,
            staging,
            announce,
            reproducible,
        };
        // Positions the window does not carry run bare, so the windows inside (fighter removal, Raid
        // Formation) are compared on their own.
        if crate::dual::backend(crate::dual::Area::Driver) == crate::dual::Backend::Dual {
            if let Some(why) = crate::dual_window::barrage_step_why(state, &probe) {
                crate::dual::note_gate(crate::dual::Area::Driver, "combat::barrage_step", why);
                return self.barrage_step_string(state, ctx, round);
            }
        }
        let rng_before = ctx.rng.positions();
        let history_start = ctx.dice.history().len();
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::barrage_step",
            state,
            |s: &mut GameState| {
                let result = self.barrage_step_string(s, ctx, round);
                failed.set(result.is_err());
                let value: crate::dual_window::BarrageOut = (
                    result.as_ref().map(Clone::clone).unwrap_or_default(),
                    ctx.dice
                        .history()
                        .iter()
                        .skip(history_start)
                        .filter(|roll| roll.reason.starts_with("anti"))
                        .map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone()))
                        .collect(),
                    ctx.rng.positions(),
                );
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                let args = crate::dual_window::BarrageArgs {
                    attacker: &attacker,
                    defender: &defender,
                    system: &system,
                    round,
                    occurrence,
                    staging,
                    announce,
                    reproducible,
                };
                crate::dual_window::barrage_step(s, sources, args, &rng_before, script)
            }
        );
        outcome.unwrap_or_else(|| Ok(Vec::new()))
    }

    /// The string body of [`Self::barrage_step`].
    fn barrage_step_string(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32) -> Result<Vec<PlayerId>, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
    let occurrence = self.ensure_combat_occurrence(state);
    // Both barrages are rolled before either is applied (78.3), one side at a time:
    // each side's reroll windows (Agnlan Oln, Scramble Frequency) open between its
    // roll and either side's removals, and the hits are read from the possibly
    // rerolled dice only afterwards.
    let mut results: Vec<(PlayerId, usize)> = Vec::new();
    for side in [self.attacker.clone(), self.defender.clone()] {
        // "Before you roll dice for ANTI-FIGHTER BARRAGE": Waylay binds here,
        // per side, before that side's dice leave the bag. The dice are rolled
        // only after the window, so the card cannot see or shape them.
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("system".to_owned(), self.system.to_string().into());
        payload.insert("player".to_owned(), side.to_string().into());
        payload.insert("round".to_owned(), i64::from(round).into());
        let _ = ctx.emit(state, "ANTI_FIGHTER_BARRAGE_STARTED", payload);
        state.combat_presentation.phase = "barrage".to_owned();
        let _ = roll_barrage_side(
            state,
            content,
            sources,
            ctx.dice,
            ctx.rng,
            &self.system,
            &side,
        );
        open_reroll_windows(state, ctx, &side);
        if let Some(set) = state.reroll_staging.get(&side).cloned() {
            let hits = staged_hits(&set);
            state
                .combat_presentation
                .barrage_hits
                .insert(side.clone(), hits as u32);
            for entry in &set.rolls {
                for (index, face) in entry.faces.iter().enumerate() {
                    let target = entry.hits_on.unwrap_or(0);
                    let adjusted = i64::from(*face)
                        + i64::from(entry.deltas.get(&index).copied().unwrap_or(0));
                    state
                        .combat_presentation
                        .barrage_dice
                        .push(CombatRollRecord {
                            player: side.clone(),
                            unit: entry.unit.clone(),
                            roll: *face,
                            target,
                            hit: target > 0 && adjusted >= i64::from(target),
                        });
                }
            }
            if hits > 0 {
                results.push((side.clone(), hits));
            }
        }
        state.reroll_staging.remove(&side);
    }
    state.last_reroll_player = None;
    state.combat_presentation.phase = "barrage".to_owned();
    let feat_players = apply_barrage(
        state,
        content,
        sources,
        self.galaxy.as_ref(),
        ctx,
        &self.system,
        &self.attacker,
        &self.defender,
        &results,
        occurrence,
    )?;
        Ok(feat_players)
    }

    /// Roll a round's combat dice and queue both sides' hits.
    #[allow(
        clippy::too_many_lines,
        reason = "one windowed step per side, read as a table"
    )]
    fn roll_round(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<(), CombatError> {
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        let (content, sources) = (ctx.content, ctx.sources);
        let unported = self.roll_unported(state, ctx.content);
        let ctx_announce = ctx.timing.is_some();
        let munitions_after = [&attacker, &defender].map(|side| {
            state.rules_version >= ti4_model::state::RULES_MUNITIONS_AFTER_ROLL
                && crate::faction_abilities::has(state, content, side, "munitions")
        });
        let reproducible = crate::product::reproducible(ctx.dice.is_preloaded(), ctx.rng.sync_pending(), ctx.rng.seed(), state.rng_seed);
        // Positions the window does not carry run bare, so the windows inside (the reroll questions) are
        // compared on their own.
        if crate::dual::backend(crate::dual::Area::Driver) == crate::dual::Backend::Dual && unported {
            crate::dual::note_gate(crate::dual::Area::Driver, "combat::roll_round", self.roll_why(state, content).unwrap_or("a gate of the roll"));
            let end = self.roll_round_string(state, ctx, round)?;
            return if end.settle { self.settle(state, ctx) } else { Ok(()) };
        }
        let rng_before = ctx.rng.positions();
        let history_start = ctx.dice.history().len();
        let failed = std::cell::Cell::new(false);
        let mut outcome = None;
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::roll_round",
            state,
            |s: &mut GameState| {
                let result = self.roll_round_string(s, ctx, round);
                let queue: Vec<(PlayerId, usize, PlayerId, bool, bool)> = match &self.stage {
                    Stage::Sustaining { queue, .. } => queue
                        .iter()
                        .map(|p| (p.player.clone(), p.hits, p.producer.clone(), p.non_fighters_only, p.producer_assigns))
                        .collect(),
                    _ => Vec::new(),
                };
                failed.set(result.as_ref().map_or(true, |end| end.produced) || !matches!(self.stage, Stage::Sustaining { .. }));
                let mut damaged: Vec<(PlayerId, Vec<String>)> = self
                    .damaged_before_round
                    .iter()
                    .map(|(side, kinds)| {
                        let mut kinds = kinds.clone();
                        kinds.sort();
                        (side.clone(), kinds)
                    })
                    .collect();
                damaged.sort();
                let value: crate::dual_window::RollOut = (
                    queue,
                    damaged,
                    ctx.dice.history().iter().skip(history_start).map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone())).collect(),
                    ctx.rng.positions(),
                );
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                let args = crate::dual_window::RollArgs { round, attacker: &attacker, defender: &defender, system: &system, reproducible, munitions_after, announce: ctx_announce };
                crate::dual_window::roll_step(s, content, sources, args, &rng_before, script)
            }
        );
        let end = outcome.unwrap_or(Ok(RollEnd { settle: false, produced: false }))?;
        if end.settle { self.settle(state, ctx) } else { Ok(()) }
    }

    /// Whether the "Roll Dice" step can hold something the core roll window does not carry: neutral sides,
    /// a roll-step reaction (The Thundarian), the Keleres agent granting commodities as trade goods to a
    /// side that may pay for Munitions Reserves or Wrath of Kenara. Everything else (the Crowns of
    /// Thalnos, the Heart of Ixth, Munitions Reserves after the roll, War Funding, Wrath of Kenara) is a
    /// stage of the window.
    fn roll_unported(&self, state: &GameState, content: &ContentStore) -> bool {
        self.roll_why(state, content).is_some()
    }

    /// [`Self::roll_unported`] with the reason (the census text).
    fn roll_why(&self, state: &GameState, content: &ContentStore) -> Option<&'static str> {
        let sides = [&self.attacker, &self.defender];
        if crate::neutral_units::is_neutral(&self.attacker) || crate::neutral_units::is_neutral(&self.defender) {
            return Some("a neutral side");
        }
        // A readied The Thundarian is an ability of the core table since package W2 (its window and the replay of the step are the
        // core roll step's); a Yssaril copy of it is not, and bails as "nested (unported ability ...)" when it is offered.
        sides
            .into_iter()
            .any(|side| {
                crate::supply::potential_goods(state, side) > crate::supply::spendable_goods(state, side)
                    && (crate::factions::has_text_in(state, side, &self.system, crate::factions::FlagshipText::Hacan)
                        || (state.rules_version >= ti4_model::state::RULES_MUNITIONS_AFTER_ROLL
                            && crate::faction_abilities::has(state, content, side, "munitions")))
            })
            .then_some("trade goods a Hacan flagship or Munitions Reserves could spend")
    }

    /// The string body of [`Self::roll_round`], up to (not including) the settle.
    fn roll_round_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<RollEnd, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        state.combat_presentation.phase = "resolving_hits".to_owned();
        state.combat_presentation.round_start =
            ships_of(state, content, sources, &self.attacker, &self.system)
                .into_iter()
                .chain(ships_of(
                    state,
                    content,
                    sources,
                    &self.defender,
                    &self.system,
                ))
                .collect();
        // 78.5f: the attacker rolls everything first. 78.6: both sides' hits are computed
        // before either is absorbed. Each side's window opens before the next side's dice
        // are drawn, like the barrage's, so a reroll can empty a fleet mid-roll and end
        // the fight where it stands.
        let Some((attacker_hits, attacker_dice, defender_hits, defender_dice)) =
            self.roll_both_sides(state, ctx, round)
        else {
            self.bail_machine("a roll that ends the fight inside the roll step");
            return Ok(RollEnd { settle: false, produced: false });
        };
        let mut sets = [attacker_dice, defender_dice];
        self.war_funding_step(state, ctx, &mut sets)?;
        if self.roll_step_end(state, ctx, round)? {
            return self.roll_round_string(state, ctx, round);
        }
        let produced = self.queue_round_hits(state, ctx, round, &sets, [attacker_hits, defender_hits])?;
        Ok(RollEnd { settle: true, produced })
    }

    /// The "Roll Dice" step's dice (78.5f), the attacker's first. `None` when a reroll window emptied a
    /// fleet mid-roll: the fight is over and the stage is already `Done`.
    fn roll_both_sides(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Option<(usize, Option<RerollSet>, usize, Option<RerollSet>)> {
        let (content, sources) = (ctx.content, ctx.sources);
        let (attacker_hits, attacker_dice) =
            roll_fleet_and_open_staged(state, ctx, &self.attacker, &self.system);
        if self.over(state, content, sources) {
            self.stage = self.conclude(state, content, sources, round);
            return None;
        }
        let (defender_hits, defender_dice) =
            roll_fleet_and_open_staged(state, ctx, &self.defender, &self.system);
        if self.over(state, content, sources) {
            self.stage = self.conclude(state, content, sources, round);
            return None;
        }
        Some((attacker_hits, attacker_dice, defender_hits, defender_dice))
    }

    /// War Funding after both sides rolled; rerolls `sets` in place.
    fn war_funding_step(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        sets: &mut [Option<RerollSet>],
    ) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        // War Funding (Letnev): "After you and your opponent roll dice during space combat: You may
        // reroll all of your opponent's dice. You may reroll any number of your dice. Then, return
        // this card to the Letnev player." Both sides' dice are in hand here and no hit has landed.
        // The holder rerolls every opposing die and their own misses -- rerolling a hit can only
        // lose it. Priced for trading since the port, the card was never offered.
        let sides = [self.attacker.clone(), self.defender.clone()];
        for (holder_index, opponent_index) in [(0usize, 1usize), (1, 0)] {
            let holder = sides[holder_index].clone();
            let Some(note) = war_funding_note(state, &holder) else {
                continue;
            };
            let opponent_dice: Vec<(usize, usize)> = sets[opponent_index]
                .as_ref()
                .map(|set| {
                    set.rolls
                        .iter()
                        .enumerate()
                        .flat_map(|(unit, entry)| {
                            (0..entry.faces.len()).map(move |die| (unit, die))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let own_misses: Vec<(usize, usize)> = sets[holder_index]
                .as_ref()
                .map(|set| {
                    set.rolls
                        .iter()
                        .enumerate()
                        .flat_map(|(unit, entry)| {
                            entry
                                .faces
                                .iter()
                                .enumerate()
                                .filter(|&(die, face)| {
                                    let adjusted = i64::from(*face)
                                        + entry
                                            .deltas
                                            .get(&die)
                                            .map_or(0, |offset| i64::from(*offset));
                                    entry.hits_on.is_some_and(|on| adjusted < i64::from(on))
                                })
                                .map(move |(die, _)| (unit, die))
                                .collect::<Vec<_>>()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let opponent_hits = sets[opponent_index].as_ref().map_or(0, staged_hits);
            if opponent_hits == 0 && own_misses.is_empty() {
                continue; // nothing to gain from it this round
            }
            let choice = Choice::new(
                holder.clone(),
                "War Funding: reroll all of your opponent's dice and your misses",
                vec![
                    ChoiceOption::labelled("use", "promissory_note", "use War Funding"),
                    ChoiceOption::decline(),
                ],
            )
            .contextualized(
                DecisionContext::new(
                    holder.clone(),
                    DecisionSource::Content("war_funding".to_owned()),
                    "war_funding",
                    state.phase,
                    state.round,
                )
                .about(DecisionTarget::System(self.system.clone())),
            );
            let answer = ctx.table.ask_seeing(
                &choice,
                &Observed::new(state, content, sources, self.galaxy.as_ref()),
            )?;
            if answer.is_decline() {
                continue;
            }
            if let Some(set) = sets[opponent_index].as_mut() {
                apply_reroll_dice(ctx.dice, ctx.rng, set, &opponent_dice, "war_funding");
            }
            if let Some(set) = sets[holder_index].as_mut() {
                apply_reroll_dice(ctx.dice, ctx.rng, set, &own_misses, "war_funding");
            }
            crate::promissory::give_back(state, &note);
        }
        Ok(())
    }

    /// The end of the "Roll Dice" step: the reaction window, and whether the step is rolled afresh.
    fn roll_step_end(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
    ) -> Result<bool, CombatError> {
        // The end of the "Roll Dice" step (78.5): both sides' dice are final and nothing has landed.
        // An effect that "returns to the start of this round's Roll Dice step" (the Nomad's
        // The Thundarian) asks for it here; no hit is produced or assigned for the discarded roll,
        // and the whole step is rolled afresh from the game's own dice stream.
        {
            let mut payload = std::collections::BTreeMap::new();
            payload.insert("system".to_owned(), self.system.to_string().into());
            payload.insert("round".to_owned(), i64::from(round).into());
            payload.insert("attacker".to_owned(), self.attacker.to_string().into());
            payload.insert("defender".to_owned(), self.defender.to_string().into());
            state.faction_marks.remove(ROLL_REPLAY_MARK);
            // Emitted only when someone is listening (a readied The Thundarian), so a game without
            // one allocates no event id for it. An illegal answer in its window is an error, not a
            // skipped step: the driver restores the snapshot it took before the first die, like
            // every other `?` in this round.
            if crate::factions::nomad_agents::watches_roll_step(state) {
                ctx.emit(state, "SPACE_COMBAT_ROLL_STEP_ENDED", payload)?;
            }
            if take_roll_replay(state) {
                // Bounded: a replay re-enters this function, so a pathological loop of re-readied
                // agents would recurse without end. See `MAX_ROLL_REPLAYS`.
                self.roll_replays += 1;
                if self.roll_replays <= MAX_ROLL_REPLAYS {
                    return Ok(true);
                }
            }
            self.roll_replays = 0;
        }
        Ok(false)
    }

    /// The round's hits from its final dice (and the modules' after-roll hits), the dice record, the
    /// pre-assignment damage note, and the assignment queue (forced hits first).
    fn queue_round_hits(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
        sets: &[Option<RerollSet>],
        rolled: [usize; 2],
    ) -> Result<bool, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        let [attacker_hits, defender_hits] = rolled;
        let split = |set: &Option<RerollSet>, rolled: usize, side: &PlayerId| {
            set.as_ref().map_or((rolled, 0), |set| {
                fleet_hits(state, content, sources, side, &self.system, set)
            })
        };
        let (mut attacker_free, mut attacker_forced) =
            split(&sets[0], attacker_hits, &self.attacker);
        let (mut defender_free, mut defender_forced) =
            split(&sets[1], defender_hits, &self.defender);
        // Hits a faction module adds to this roll (`CombatMoment::AfterRoll`) join the round's
        // simultaneous hits (78.6), assigned by the opponent like the dice's own.
        let [by_attacker, by_defender] =
            self.produce_hits(state, ctx, round, CombatMoment::AfterRoll)?;
        let produced = [by_attacker, by_defender].iter().any(|made| *made != ProducedHits::NONE);
        attacker_free += by_attacker.any_ship;
        attacker_forced += by_attacker.non_fighter;
        defender_free += by_defender.any_ship;
        defender_forced += by_defender.non_fighter;

        state.combat_round_hits.insert(
            self.attacker.clone(),
            (attacker_free + attacker_forced) as u32,
        );
        state.combat_round_hits.insert(
            self.defender.clone(),
            (defender_free + defender_forced) as u32,
        );

        #[cfg(feature = "typed-core")]
        let typed = || {
            crate::core_combat::CombatView::of(state, content, sources)?
                .round_dice([(&self.attacker, &sets[0]), (&self.defender, &sets[1])])
        };
        #[cfg(not(feature = "typed-core"))]
        let typed = || None;
        let round_dice = crate::dual_typed!(
            crate::dual::Area::Combat,
            "combat::round_dice",
            typed(),
            {
        let mut round_dice = Vec::new();
        for (side, set) in [(&self.attacker, &sets[0]), (&self.defender, &sets[1])] {
            if let Some(s) = set {
                for entry in &s.rolls {
                    let threshold = entry.hits_on.unwrap_or(0);
                    for (die_idx, face) in entry.faces.iter().enumerate() {
                        let delta = entry.deltas.get(&die_idx).copied().unwrap_or(0);
                        let final_val = (*face as i64 + delta as i64).max(0) as u32;
                        let hit = threshold > 0 && final_val >= threshold;
                        round_dice.push(CombatRollRecord {
                            player: (*side).clone(),
                            unit: entry.unit.clone(),
                            roll: *face,
                            target: threshold,
                            hit,
                        });
                    }
                }
            }
        }
        round_dice
            }
        );
        state.combat_round_dice = round_dice;

        // Forced hits first, so a free hit can still take a fighter the forced ones had to spare.
        let mut queue: Vec<Pending> = [
            (&self.defender, attacker_forced, &self.attacker, true),
            (&self.defender, attacker_free, &self.attacker, false),
            (&self.attacker, defender_forced, &self.defender, true),
            (&self.attacker, defender_free, &self.defender, false),
        ]
        .into_iter()
        .map(|(player, hits, producer, non_fighters_only)| Pending {
            player: player.clone(),
            hits,
            producer: producer.clone(),
            non_fighters_only,
            producer_assigns: false,
        })
        .into_iter()
        .filter(|pending| pending.hits > 0)
        .collect();
        // Hits a module's player assigns themselves ride after the round's own.
        queue.extend(self.producer_assigned_queue(&[by_attacker, by_defender]));

        self.damaged_before_round = [self.attacker.clone(), self.defender.clone()]
            .into_iter()
            .map(|side| {
                let damaged = state
                    .system_state(&self.system)
                    .units
                    .iter()
                    .filter(|unit| unit.owner == side && unit.sustained_damage)
                    .map(|unit| unit.type_id.to_string())
                    .collect();
                (side, damaged)
            })
            .collect();
        self.stage = Stage::Sustaining { queue, round };
        Ok(produced)
    }

    /// Settle a freshly opened window, so a fight that is already over reports so.
    ///
    /// # Errors
    ///
    /// A Waylay casualty answer that the table rejects, or a dice source that runs dry.
    pub fn settle_open(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), CombatError> {
        self.settle(state, ctx)
    }

    /// Advance past anything with no decision left in it, and announce the end of the fight
    /// (`SPACE_COMBAT_ENDED`) once it is reached.
    fn settle(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), CombatError> {
        if self.settle_depth > 0
            || !crate::dual_window::composed()
            || crate::dual::backend(crate::dual::Area::Driver) != crate::dual::Backend::Dual
        {
            return self.settle_string(state, ctx);
        }
        self.settle_machine_dual(state, ctx)
    }

    /// The string body of [`Self::settle`].
    fn settle_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), CombatError> {
        self.settle_depth += 1;
        let result = self.settle_stages(state, ctx);
        self.settle_depth -= 1;
        result?;
        self.announce_end(state, ctx);
        Ok(())
    }

    /// `settle` on `driver=dual`: the string loop is compared with the composed core machine
    /// (`rules::combat::machine`) from the stage it starts in to the next decision or the end of the fight.
    /// The windows inside the string loop run bare then (the outer window compares the whole).
    fn settle_machine_dual(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), CombatError> {
        let sources = ctx.sources;
        // A position the machine is known not to carry runs the windows inside one by one instead.
        let args = self.settle_args(state, ctx);
        if let Some(why) = args.as_ref().and_then(|a| a.gate_why) {
            crate::dual::note_gate(crate::dual::Area::Driver, "combat::settle", why);
        }
        let args = args.filter(|a| !(a.opening_unported || a.barrage_unported || a.roll_unported || a.retreat_unported));
        let Some(args) = args else { return self.settle_string(state, ctx) };
        let args = Some(args);
        let rng_before = ctx.rng.positions();
        let history_start = ctx.dice.history().len();
        let failed = std::cell::Cell::new(false);
        let failed_why = std::cell::Cell::new("");
        let mut outcome = None;
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::settle",
            state,
            |s: &mut GameState| {
                self.machine_bail = false;
                self.machine_bail_why = "";
                let result = self.settle_string(s, ctx);
                failed.set(result.is_err() || self.machine_bail);
                failed_why.set(if result.is_err() { "the string settle ended in an error" } else { self.machine_bail_why });
                let dice: Vec<(u32, Vec<u32>)> = ctx.dice.history().iter().skip(history_start).map(|roll| (roll.hits_on.unwrap_or(0), roll.faces.clone())).collect();
                let value = self.settle_out(dice, ctx.rng.positions());
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    crate::dual::note_bail_here(failed_why.get());
                    return None;
                }
                args.and_then(|args| crate::dual_window::settle_machine(s, sources, args, &rng_before, script))
            }
        );
        outcome.unwrap_or(Ok(()))
    }

    /// What a settle left, as the dual compares it.
    fn settle_out(&self, dice: Vec<(u32, Vec<u32>)>, rng: crate::rng::RngPositions) -> crate::dual_window::SettleOut {
        let mut damaged: Vec<(PlayerId, Vec<String>)> = self
            .damaged_before_round
            .iter()
            .map(|(side, kinds)| {
                let mut kinds = kinds.clone();
                kinds.sort();
                (side.clone(), kinds)
            })
            .collect();
        damaged.sort();
        let outcome = match &self.stage {
            Stage::Done(o) => Some((o.winner.clone(), o.rounds)),
            _ => None,
        };
        let phase = match self.hit_phase {
            HitPhase::Round => 0,
            HitPhase::CombatStart => 1,
            HitPhase::RoundEnd => 2,
        };
        (
            Self::flow_step_of(&self.stage),
            outcome,
            (phase, self.resuming_round, self.pending_retreats.clone(), self.hit_reaction_offered.iter().cloned().collect()),
            (damaged, self.fled.clone(), self.end_announced),
            dice,
            rng,
        )
    }

    /// The window fields as plain data: what `Window::Combat` holds in the core (D1b2).
    pub(crate) fn fields_in(&self) -> crate::dual_window::FieldsIn {
        crate::dual_window::FieldsIn {
            step: Self::flow_step_of(&self.stage),
            phase: match self.hit_phase {
                HitPhase::Round => 0,
                HitPhase::CombatStart => 1,
                HitPhase::RoundEnd => 2,
            },
            resuming: self.resuming_round,
            pending_retreats: self.pending_retreats.clone(),
            offered: self.hit_reaction_offered.iter().cloned().collect(),
            damaged: self.damaged_before_round.iter().map(|(p, k)| (p.clone(), k.clone())).collect(),
            fled: self.fled.clone(),
            at_start: self.fleet_at_start.iter().map(|(p, k)| (p.clone(), k.clone())).collect(),
            end_announced: self.end_announced,
            outcome: match &self.stage {
                Stage::Done(o) => Some((o.winner.clone(), o.rounds)),
                _ => None,
            },
        }
    }

    /// The facts and gates the composed machine starts from. `None`: no map to read the retreats from.
    fn settle_args(&self, state: &GameState, ctx: &Resolving<'_>) -> Option<crate::dual_window::SettleArgs> {
        let (content, sources) = (ctx.content, ctx.sources);
        let sides = [&self.attacker, &self.defender];
        let announce = ctx.timing.is_some();
        let reproducible = crate::product::reproducible(ctx.dice.is_preloaded(), ctx.rng.sync_pending(), ctx.rng.seed(), state.rng_seed);
        let galaxy = self.galaxy.as_ref();
        let neighbors = sides.map(|side| match galaxy {
            Some(g) => crate::core_combat::retreat_neighbors(state, sources, g, side, &self.system),
            None => Some(Vec::new()),
        });
        let [Some(n0), Some(n1)] = neighbors else { return None };
        let has_tech = |side: &PlayerId, id: &str| state.player(side).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&id)));
        let round_probe = crate::dual_window::BeginRoundArgs { attacker: &self.attacker, defender: &self.defender, system: &self.system, round: 1, announce };
        let opening_why = crate::dual_window::round_opening_why(state, content, &round_probe).or_else(|| {
            sides.iter().find_map(|side| {
                if state.player(side).is_none_or(|p| p.faction.as_str() == "mentak") {
                    return Some("a Mentak side (Ambush at the start of the combat)");
                }
                ["asc", "ds", "ic"].iter().any(|t| has_tech(side, t)).then_some("Assault Cannon, Dimensional Splicer or Impulse Core (start-of-combat hits)")
            })
        });
        let opening_unported = opening_why.is_some();
        let occurrence = self.combat_occurrence.map(|o| o.0);
        let barrage_probe = crate::dual_window::BarrageArgs {
            attacker: &self.attacker,
            defender: &self.defender,
            system: &self.system,
            round: 1,
            occurrence: occurrence.unwrap_or(0),
            staging: crate::supply::staging_enabled(state),
            announce,
            reproducible,
        };
        let barrage_why = if reproducible { crate::dual_window::barrage_why(state, &barrage_probe) } else { Some("irreproducible dice (test faces or a stream restore)") };
        let barrage_unported = barrage_why.is_some();
        let retreat_why = sides.iter().find_map(|side| {
            [&n0, &n1].iter().find_map(|list| {
                list.iter().find_map(|(dest, _)| crate::dual_window::retreat_why(state, content, sources, side, &self.system, dest))
            })
        });
        let retreat_unported = retreat_why.is_some();
        let roll_why = if reproducible { self.roll_why(state, content) } else { Some("irreproducible dice (test faces or a stream restore)") };
        let may_retreat = sides.map(|side| {
            !(crate::neutral_units::is_neutral(side) && !crate::neutral_units::may_retreat())
                && !crate::factions::mahact_units::ship_movement_barred(state)
        });
        let begin_occurrence = occurrence.is_none()
            && matches!(self.stage, Stage::Opening { round: 1 })
            && !self.resuming_round
            && !self.over(state, content, sources);
        Some(crate::dual_window::SettleArgs {
            attacker: self.attacker.clone(),
            defender: self.defender.clone(),
            system: self.system.clone(),
            step: Self::flow_step_of(&self.stage),
            phase: match self.hit_phase {
                HitPhase::Round => 0,
                HitPhase::CombatStart => 1,
                HitPhase::RoundEnd => 2,
            },
            resuming: self.resuming_round,
            pending_retreats: self.pending_retreats.clone(),
            offered: self.hit_reaction_offered.iter().cloned().collect(),
            damaged: self.damaged_before_round.iter().map(|(p, k)| (p.clone(), k.clone())).collect(),
            fled: self.fled.clone(),
            at_start: self.fleet_at_start.iter().map(|(p, k)| (p.clone(), k.clone())).collect(),
            end_announced: self.end_announced,
            announce,
            staging: crate::supply::staging_enabled(state),
            occurrence,
            begin_occurrence,
            munitions_after: sides.map(|side| {
                state.rules_version >= ti4_model::state::RULES_MUNITIONS_AFTER_ROLL && crate::faction_abilities::has(state, content, side, "munitions")
            }),
            opening_unported,
            round_seq_listens: crate::factions::borrowed_round_agents::has_round_copy(state, content, "letnevagent")
                || crate::factions::borrowed_round_agents::has_owner_space_round_agent(state),
            barrage_unported,
            roll_unported: roll_why.is_some(),
            retreat_unported,
            gate_why: opening_why.or(barrage_why).or(roll_why).or(retreat_why),
            neighbors: [n0, n1],
            may_retreat,
            det: sides.map(|side| crate::technology::owns_det(state, side)),
            duranium: sides.map(|side| has_tech(side, "da")),
            devotion: state.players.iter().any(|p| crate::factions::yin::is_yin(state, &p.id)),
        })
    }

    /// The state machine behind [`Self::settle`].
    ///
    /// One long match rather than several helpers: every arm is a transition in the same state
    /// machine, and splitting them would hide the fact that each arm's job is to fall through
    /// to the next stage rather than to do work of its own.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per combat stage, read as a table"
    )]
    fn settle_stages(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
    ) -> Result<(), CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
        loop {
            state.combat_presentation.phase = match &self.stage {
                Stage::Opening { .. } | Stage::Announcing { .. } => "pre_roll",
                Stage::RollingAfterBarrage { .. } => "barrage",
                Stage::Sustaining { .. } | Stage::Assigning { .. } | Stage::Rolling { .. } => {
                    "resolving_hits"
                }
                Stage::Retreating { .. } => "retreating",
                Stage::Done(_) => "complete",
            }
            .to_owned();
            state.combat_presentation.remaining_hits = match &self.stage {
                Stage::Sustaining { queue, .. } | Stage::Assigning { queue, .. } => {
                    let mut remaining = std::collections::BTreeMap::new();
                    for pending in queue {
                        *remaining.entry(pending.player.clone()).or_insert(0) += pending.hits;
                    }
                    remaining
                }
                _ => Default::default(),
            };
            match self.stage.clone() {
                Stage::Sustaining { queue, round } | Stage::Assigning { queue, round } => {
                    match self.hit_queue_head(state, ctx, queue, round)? {
                        Flow::Continue => {}
                        Flow::Return => return Ok(()),
                        Flow::Resolve(answer) => {
                            return self.resolve(state, ctx, answer).map_err(CombatError::from);
                        }
                    }
                }
                Stage::Announcing {
                    round,
                    asking,
                    announced,
                } => {
                    if let Flow::Return = self.announcing_step(state, ctx, round, asking, announced)? {
                        return Ok(());
                    }
                }
                Stage::Retreating { round, leaving } => {
                    if let Flow::Return = self.retreating_step(state, ctx, round, leaving)? {
                        return Ok(());
                    }
                }
                Stage::Opening { round } => {
                    if self.over(state, content, sources) {
                        self.stage = self.conclude(state, content, sources, round - 1);
                        return Ok(());
                    }
                    self.open_round(state, ctx, round)?;
                    if matches!(self.stage, Stage::RollingAfterBarrage { .. }) {
                        return Ok(());
                    }
                }
                Stage::Rolling { round } => {
                    if self.over(state, content, sources) {
                        self.stage = self.conclude(state, content, sources, round - 1);
                        return Ok(());
                    }
                    self.roll_round(state, ctx, round)?;
                    return Ok(());
                }
                Stage::RollingAfterBarrage { round } => {
                    self.stage = Stage::Announcing {
                        round,
                        asking: self.defender.clone(),
                        announced: Vec::new(),
                    };
                }
                Stage::Done(_) => return Ok(()),
            }
        }
    }

    /// [`Self::hit_queue_head_string`] with its bookkeeping checked against the core flow window.
    fn hit_queue_head(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, queue: Vec<Pending>, round: u32) -> Result<Flow, CombatError> {
        self.flow_dual(state, ctx, |me, s, c| me.hit_queue_head_string(s, c, queue, round))
    }

    /// [`Self::announcing_step_string`] checked against the core flow window.
    fn announcing_step(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32, asking: PlayerId, announced: Vec<PlayerId>) -> Result<Flow, CombatError> {
        self.flow_dual(state, ctx, |me, s, c| me.announcing_step_string(s, c, round, asking, announced))
    }

    /// [`Self::retreating_step_string`] checked against the core flow window.
    fn retreating_step(&mut self, state: &mut GameState, ctx: &mut Resolving<'_>, round: u32, leaving: Vec<PlayerId>) -> Result<Flow, CombatError> {
        self.flow_dual(state, ctx, |me, s, c| me.retreating_step_string(s, c, round, leaving))
    }

    /// The stage as the core flow window sees it.
    fn flow_step_of(stage: &Stage) -> crate::dual_window::StepS {
        use crate::dual_window::{PendS, StepS};
        let queue = |queue: &[Pending]| {
            queue
                .iter()
                .map(|p| PendS { player: p.player.clone(), hits: p.hits, producer: p.producer.clone(), non_fighters_only: p.non_fighters_only, producer_assigns: p.producer_assigns })
                .collect()
        };
        match stage {
            Stage::Opening { round } => StepS::Opening(*round),
            Stage::Announcing { round, asking, announced } => StepS::Announcing(*round, asking.clone(), announced.clone()),
            Stage::Rolling { round } => StepS::Rolling(*round),
            Stage::RollingAfterBarrage { round } => StepS::AfterBarrage(*round),
            Stage::Sustaining { queue: q, round } => StepS::Queue(false, *round, queue(q)),
            Stage::Assigning { queue: q, round } => StepS::Queue(true, *round, queue(q)),
            Stage::Retreating { round, leaving } => StepS::Retreating(*round, leaving.clone()),
            Stage::Done(_) => StepS::Done,
        }
    }

    /// Run one stage transition of `settle_stages` (the string body `run`), compared with the core flow
    /// window on `driver=dual`. Positions the core window does not carry run bare.
    fn flow_dual(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        run: impl FnOnce(&mut Self, &mut GameState, &mut Resolving<'_>) -> Result<Flow, CombatError>,
    ) -> Result<Flow, CombatError> {
        use crate::dual_window::StepS;
        if crate::dual::backend(crate::dual::Area::Driver) != crate::dual::Backend::Dual {
            return run(self, state, ctx);
        }
        let (content, sources) = (ctx.content, ctx.sources);
        let (attacker, defender, system) = (self.attacker.clone(), self.defender.clone(), self.system.clone());
        let step = Self::flow_step_of(&self.stage);
        let over = self.over(state, content, sources);
        let (mut asking_can_retreat, mut leaver_destinations, mut front_alive, mut offered_before, mut sustainers) = (false, 0, 0, false, false);
        let mut bare = !state.pending_destructions.is_empty();
        match &self.stage {
            Stage::Announcing { asking, .. } => asking_can_retreat = !self.retreats(state, content, sources, asking).is_empty(),
            Stage::Retreating { leaving, .. } => {
                if let Some(first) = leaving.first() {
                    leaver_destinations = self.retreats(state, content, sources, first).len();
                    bare |= leaver_destinations == 1;
                } else {
                    // hits modules produce after the round (Devotion) ask their own questions
                    bare |= !over && state.players.iter().any(|p| crate::factions::yin::is_yin(state, &p.id));
                }
            }
            Stage::Sustaining { queue, round } | Stage::Assigning { queue, round } => {
                let assigning = matches!(self.stage, Stage::Assigning { .. });
                if let Some(front) = queue.first() {
                    front_alive = ships_of(state, content, sources, &front.player, &self.system).len();
                    offered_before = self.hit_reaction_offered.contains(&(*round, front.player.clone()));
                    sustainers = !self.sustainers(state, content, sources, &front.player).is_empty();
                    let producer_picks = front.producer_assigns && assigning;
                    bare |= front.hits > 0
                        && front_alive > 0
                        && (cancellable_hits(state, &front.player) > 0
                            || (assigning && self.casualty_candidates(state, content, sources, front).len() == 1)
                            || (crate::neutral_units::is_neutral(&front.player) && !producer_picks));
                } else if self.hit_phase == HitPhase::Round {
                    bare |= [&self.attacker, &self.defender].into_iter().any(|side| {
                        self.damaged_before_round.get(side).is_some_and(|l| !l.is_empty())
                            && state.player(side).is_some_and(|seat| seat.technologies.contains(ti4_model::id::text(&"da")))
                    });
                }
            }
            _ => bare = true,
        }
        if bare || crate::neutral_units::is_neutral(&attacker) || crate::neutral_units::is_neutral(&defender) {
            return run(self, state, ctx);
        }
        let announce = ctx.timing.is_some();
        let pending_retreats = self.pending_retreats.clone();
        let phase = match self.hit_phase {
            HitPhase::Round => 0,
            HitPhase::CombatStart => 1,
            HitPhase::RoundEnd => 2,
        };
        let resuming = self.resuming_round;
        let before_offered = self.hit_reaction_offered.clone();
        let failed = std::cell::Cell::new(false);
        let mut outcome = None;
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::flow_step",
            state,
            |s: &mut GameState| {
                let result = run(self, s, ctx);
                let returned = matches!(result, Ok(Flow::Return));
                failed.set(!matches!(result, Ok(Flow::Continue | Flow::Return)));
                let next = Self::flow_step_of(&self.stage);
                // Hits a module queued after the round (not carried): the window left the flow.
                failed.set(failed.get() || (matches!(step, StepS::Retreating(..)) && matches!(next, StepS::Queue(..))));
                let offered_now = self.hit_reaction_offered.len() > before_offered.len();
                let phase_now = match self.hit_phase {
                    HitPhase::Round => 0,
                    HitPhase::CombatStart => 1,
                    HitPhase::RoundEnd => 2,
                };
                let value: crate::dual_window::FlowOut = (next, returned, phase_now, self.resuming_round, self.pending_retreats.clone(), offered_now);
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    return None;
                }
                let args = crate::dual_window::FlowArgs {
                    attacker: &attacker,
                    defender: &defender,
                    system: &system,
                    announce,
                    step: step.clone(),
                    over,
                    asking_can_retreat,
                    leaver_destinations,
                    front_alive,
                    offered_before,
                    sustainers,
                    pending_retreats: &pending_retreats,
                    phase,
                    resuming,
                };
                crate::dual_window::flow_step(s, sources, &args, script)
            }
        );
        outcome.unwrap_or(Ok(Flow::Return))
    }

    /// The head of the hit queue (`Sustaining`/`Assigning`): the end of the queue, empty and
    /// unanswerable entries, cancellations, the HITS_TO_ASSIGN reaction, the sustain/casualty
    /// shortcuts, and the neutral answer. `Flow::Return` is a pending decision.
    #[allow(clippy::too_many_lines, reason = "one arm of the old table, moved whole")]
    fn hit_queue_head_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        queue: Vec<Pending>,
        round: u32,
    ) -> Result<Flow, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
            let Some(front) = queue.first().cloned() else {
                // Hits modules produced at the start of the combat are assigned; the
                // round resumes at its anti-fighter barrage.
                if self.hit_phase == HitPhase::CombatStart {
                    self.hit_phase = HitPhase::Round;
                    if self.over(state, content, sources) {
                        self.stage = self.conclude(state, content, sources, round);
                        return Ok(Flow::Return);
                    }
                    self.resuming_round = true;
                    self.stage = Stage::Opening { round };
                    return Ok(Flow::Continue);
                }
                // "After a round" hits are assigned: the round is announced over.
                if self.hit_phase == HitPhase::RoundEnd {
                    self.announce_round_ended(state, ctx, round);
                    self.stage = self.next_round_or_end(state, content, sources, round);
                    return Ok(Flow::Continue);
                }
                // Both sides absorbed: Duranium Armor repairs now, "after you assign
                // hits to your units".
                for side in [self.attacker.clone(), self.defender.clone()] {
                    let before =
                        self.damaged_before_round.remove(&side).unwrap_or_default();
                    duranium_armor(state, &self.system, &side, &before);
                }
                if self.over(state, content, sources) || round >= MAX_ROUNDS {
                    self.announce_round_ended(state, ctx, round);
                    self.stage = self.conclude(state, content, sources, round);
                    return Ok(Flow::Return);
                }
                // 78.7: those who announced now leave, before the next round.
                let leaving = std::mem::take(&mut self.pending_retreats);
                self.stage = Stage::Retreating { round, leaving };
                return Ok(Flow::Continue);
            };
            if front.hits == 0 {
                let rest = queue[1..].to_vec();
                self.stage = Stage::Sustaining { queue: rest, round };
                return Ok(Flow::Continue);
            }
            let alive =
                ships_of(state, content, sources, &front.player, &self.system).len();
            if alive == 0 {
                // 15.2a: hits beyond the units available have no effect.
                let rest = queue[1..].to_vec();
                self.stage = Stage::Sustaining { queue: rest, round };
                return Ok(Flow::Continue);
            }
            // Shields Holding and friends cancel hits before any are assigned. Spent here,
            // after the window that grants them has had its chance to fire and before the
            // sustain offer, because a cancelled hit is one nobody has to absorb.
            let cancelled = spend_cancellations(state, &front.player, front.hits);
            if cancelled > 0 {
                let mut rest = queue.clone();
                rest[0].hits -= cancelled;
                self.stage = match self.stage {
                    Stage::Assigning { .. } => Stage::Assigning { queue: rest, round },
                    _ => Stage::Sustaining { queue: rest, round },
                };
                return Ok(Flow::Continue);
            }
            // "Before you assign hits to your ships during a space combat." Emitted as the
            // first of a player's hits is about to land: `front` still carries its full
            // count here and the stage has consumed none of it.
            if matches!(self.stage, Stage::Sustaining { .. })
                && self
                    .hit_reaction_offered
                    .insert((round, front.player.clone()))
            {
                let mut payload = std::collections::BTreeMap::new();
                payload.insert("system".to_owned(), self.system.to_string().into());
                payload.insert("player".to_owned(), front.player.to_string().into());
                payload.insert(
                    "hits".to_owned(),
                    i64::try_from(front.hits).unwrap_or(0).into(),
                );
                payload.insert("round".to_owned(), i64::from(round).into());
                let _ = ctx.emit(state, "HITS_TO_ASSIGN", payload);
                // The reaction window may have granted cancellations while `emit`
                // was waiting for its players. Apply them before offering sustain or
                // casualties; the earlier spend only covers grants from prior windows.
                let cancelled = spend_cancellations(state, &front.player, front.hits);
                if cancelled > 0 {
                    let mut rest = queue.clone();
                    rest[0].hits -= cancelled;
                    self.stage = Stage::Sustaining { queue: rest, round };
                    return Ok(Flow::Continue);
                }
            }
            // A sustain is only offered when something can take one.
            if matches!(self.stage, Stage::Sustaining { .. })
                && self
                    .sustainers(state, content, sources, &front.player)
                    .is_empty()
            {
                self.stage = Stage::Assigning { queue, round };
                return Ok(Flow::Continue);
            }
            // A single possible casualty is not a decision.
            let candidates = self.casualty_candidates(state, content, sources, &front);
            if matches!(self.stage, Stage::Assigning { .. }) && candidates.len() == 1 {
                let only = candidates[0].1.clone();
                remove_combat_ship(state, &self.system, &only);
                announce_ship_destroyed(state, ctx, &self.system, &front.player, &only);
                let mut rest = queue;
                rest[0].hits -= 1;
                self.stage = Stage::Sustaining { queue: rest, round };
                return Ok(Flow::Continue);
            }
            // A decision is pending. Neutral units never wait on a decider (rules 5, 7):
            // answer for them and let `resolve` carry the combat on from there.
            // A producer-assigned hit (Devotion) has the *producer* pick the ship, so the
            // shortcut is skipped only for that pick; the neutral side's SUSTAIN DAMAGE
            // answer is still automatic.
            let producer_picks =
                front.producer_assigns && matches!(self.stage, Stage::Assigning { .. });
            if crate::neutral_units::is_neutral(&front.player)
                && !producer_picks
                && let Some(answer) = self.neutral_answer(state, content, sources)
            {
                return Ok(Flow::Resolve(answer));
            }
            return Ok(Flow::Return);
        
    }

    /// The `Announce Retreats` step for `asking`: the step-start reaction, the end check, and who is
    /// asked next. `Flow::Return` is a pending announcement question.
    fn announcing_step_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
        asking: PlayerId,
        announced: Vec<PlayerId>,
    ) -> Result<Flow, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
            // "At the start of the 'Announce Retreats' step of space combat, if you are the
            // defender." The defender is asked first (78.4b), so the step starts when they
            // are the one being asked and nobody has announced yet.
            if asking == self.defender && announced.is_empty() {
                let mut payload = std::collections::BTreeMap::new();
                payload.insert("system".to_owned(), self.system.to_string().into());
                payload.insert("player".to_owned(), self.defender.to_string().into());
                payload.insert("round".to_owned(), i64::from(round).into());
                let _ = ctx.emit(state, "RETREAT_STEP_STARTED", payload);
            }
            if self.over(state, content, sources) {
                self.stage = self.conclude(state, content, sources, round - 1);
                return Ok(Flow::Return);
            }
            // 78.4c: a player with nowhere to go is not asked.
            if !self.retreats(state, content, sources, &asking).is_empty() {
                return Ok(Flow::Return);
            }
            if asking == self.defender && !announced.contains(&self.defender) {
                self.stage = Stage::Announcing {
                    round,
                    asking: self.attacker.clone(),
                    announced,
                };
                return Ok(Flow::Continue);
            }
            self.pending_retreats = announced;
            self.stage = Stage::Rolling { round };
        
        Ok(Flow::Continue)
    }

    /// The `Retreating` stage: the next leaver (moved without a question when there is one
    /// destination), or the end of the round.
    fn retreating_step_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        round: u32,
        leaving: Vec<PlayerId>,
    ) -> Result<Flow, CombatError> {
        let (content, sources) = (ctx.content, ctx.sources);
            let Some(player) = leaving.first().cloned() else {
                // Everyone who announced has gone: the round, retreats included, is
                // over. Modules' "after a round" hits come first (not once the fight is
                // decided), then the round is announced over.
                if self.open_round_end_hits(state, ctx, round)? {
                    return Ok(Flow::Continue);
                }
                self.announce_round_ended(state, ctx, round);
                self.stage = self.next_round_or_end(state, content, sources, round);
                return Ok(Flow::Continue);
            };
            let destinations = self.retreats(state, content, sources, &player);
            match destinations.as_slice() {
                // The destination stopped qualifying during the round.
                [] => {
                    let rest = leaving[1..].to_vec();
                    self.stage = Stage::Retreating {
                        round,
                        leaving: rest,
                    };
                }
                [only] => {
                    let only = only.clone();
                    self.retreat(state, content, sources, ctx, &player, &only);
                    let rest = leaving[1..].to_vec();
                    self.stage = Stage::Retreating {
                        round,
                        leaving: rest,
                    };
                }
                _ => return Ok(Flow::Return),
            }
        
        Ok(Flow::Continue)
    }
}

impl Window for CombatWindow {
    #[allow(clippy::too_many_lines, reason = "one arm per stage, read as a table")]
    fn pending_choice(
        &self,
        state: &GameState,
        content: &ContentStore,
        sources: SourceSet,
    ) -> Option<Choice> {
        match &self.stage {
            Stage::Done(_)
            | Stage::Opening { .. }
            | Stage::Rolling { .. }
            | Stage::RollingAfterBarrage { .. } => None,
            Stage::Announcing { asking, .. } => {
                if self.retreats(state, content, sources, asking).is_empty() {
                    return None; // 78.4c: nothing to retreat to, so nothing to announce
                }
                // Rout ("your opponent must announce a retreat, if able", played at the start
                // of this step by the defender): the opponent's retreat is forced, so
                // "stay" is not on the table for them this round. The defender's marker is
                // scoped to `combat_round_seq` and cannot match any later round or combat.
                let forced = asking == &self.attacker
                    && state
                        .player(&self.defender)
                        .and_then(|seat| seat.rout_round)
                        .is_some_and(|played| played == state.combat_round_seq);
                // OBS-008b3: 78.7 retreats the *whole* remaining fleet together, so "retreat"
                // previews it leaving this system entirely; "stay" previews no change. Both carry
                // `system` so the existing generic board-fact pipeline (`option-system:*`) also
                // reaches this decision, which previously carried no payload at all.
                let own_ships =
                    i64::try_from(ships_of(state, content, sources, asking, &self.system).len())
                        .unwrap_or(i64::MAX);
                let here = self.system.to_string();
                let options = if forced {
                    vec![
                        ChoiceOption::labelled("retreat", RETREAT_KIND, "retreat (forced)")
                            .with("system", here)
                            .previewed(Preview::certain(vec![Delta::new(
                                Quantity::ShipsInSystem,
                                own_ships,
                                0,
                            )])),
                    ]
                } else {
                    vec![
                        ChoiceOption::labelled("stay", RETREAT_KIND, "stay and fight")
                            .with("system", here.clone())
                            .previewed(Preview::certain(vec![Delta::new(
                                Quantity::ShipsInSystem,
                                own_ships,
                                own_ships,
                            )])),
                        ChoiceOption::labelled("retreat", RETREAT_KIND, "announce a retreat")
                            .with("system", here)
                            .previewed(Preview::certain(vec![Delta::new(
                                Quantity::ShipsInSystem,
                                own_ships,
                                0,
                            )])),
                    ]
                };
                Some(
                    Choice::new(
                        asking.clone(),
                        format!("announce a retreat from {}", self.system),
                        options,
                    )
                    .contextualized(
                        DecisionContext::new(
                            asking.clone(),
                            DecisionSource::Rule("78.9".to_owned()),
                            "announce_retreat",
                            state.phase,
                            state.round,
                        )
                        .about(DecisionTarget::System(self.system.clone())),
                    ),
                )
            }
            Stage::Retreating { leaving, .. } => {
                let player = leaving.first()?;
                let destinations = self.retreats(state, content, sources, player);
                if destinations.len() < 2 {
                    return None; // 78.7b: one destination is not a decision
                }
                // OBS-008b3: 78.7 retreats the whole remaining fleet together, so every
                // destination previews the same arrival count -- what changes between options is
                // the destination itself, carried by `system` for the existing generic
                // board-fact pipeline (`option-system:*`).
                let fleet_size =
                    i64::try_from(ships_of(state, content, sources, player, &self.system).len())
                        .unwrap_or(i64::MAX);
                Some(
                    Choice::new(
                        player.clone(),
                        "retreat to which system",
                        destinations
                            .iter()
                            .map(|id| {
                                let already_there = i64::try_from(
                                    ships_of(state, content, sources, player, id).len(),
                                )
                                .unwrap_or(i64::MAX);
                                ChoiceOption::labelled(
                                    id.to_string(),
                                    RETREAT_TO_KIND,
                                    format!("retreat to {id}"),
                                )
                                .with("system", id.to_string())
                                .previewed(Preview::certain(vec![Delta::new(
                                    Quantity::ShipsInSystem,
                                    already_there,
                                    already_there + fleet_size,
                                )]))
                            })
                            .collect(),
                    )
                    .contextualized(
                        DecisionContext::new(
                            player.clone(),
                            DecisionSource::Rule("78.7".to_owned()),
                            "retreat_to",
                            state.phase,
                            state.round,
                        )
                        .about(DecisionTarget::System(self.system.clone())),
                    ),
                )
            }
            Stage::Sustaining { queue, .. } => {
                let front = queue.first()?;
                let available = self.sustainers(state, content, sources, &front.player);
                if available.is_empty() {
                    return None;
                }
                // OBS-008b4: sustaining keeps the ship, damaged; declining loses it -- the same
                // shared before/after every sustain-or-decline option in this ask carries.
                let own_ships = i64::try_from(
                    ships_of(state, content, sources, &front.player, &self.system).len(),
                )
                .unwrap_or(i64::MAX);

                // Space choices preserve the one-option-per-type stream. Planetary Naaz variants
                // retain their exact canonical location so different fresh unit states remain
                // selectable without changing the first variant's legacy id.
                let sustainers = sustainer_counts(
                    state,
                    content,
                    sources,
                    &self.system,
                    &front.player,
                    true,
                    &available,
                );
                let mut seen = std::collections::BTreeSet::new();
                let mut options = Vec::new();
                for (index, unit) in available {
                    if !(crate::supply::staging_enabled(state) && index.starts_with("planet:"))
                        && !seen.insert(unit.type_id.to_string())
                    {
                        continue;
                    }
                    let label = if crate::supply::staging_enabled(state) {
                        planetary_sustain_location(&index).map_or_else(
                            || format!("sustain damage on {}", unit.type_id),
                            |(planet, _)| planet_sustain_label(content, sources, planet, &unit),
                        )
                    } else {
                        format!("sustain damage on {}", unit.type_id)
                    };
                    options.push(
                        ChoiceOption::labelled(format!("sustain|{index}"), SUSTAIN_KIND, label)
                            .with("unit", unit.type_id.to_string())
                            .previewed(Preview::certain(vec![Delta::new(
                                Quantity::ShipsInSystem,
                                own_ships,
                                own_ships,
                            )])),
                    );
                }
                let mut take = ChoiceOption::labelled(
                    crate::choice::DECLINE_ID,
                    crate::choice::DECLINE_KIND,
                    "take the hit",
                )
                .previewed(Preview::certain(vec![Delta::new(
                    Quantity::ShipsInSystem,
                    own_ships,
                    own_ships - 1,
                )]));
                // Display only, as in `offer_sustain`: the loss this hit forces is a
                // non-fighter while one is left.
                if front.non_fighters_only
                    && !non_fighter_ships(state, content, sources, &front.player, &self.system)
                        .is_empty()
                {
                    take = take.with("non_fighters_first", true);
                }
                options.push(take);
                Some(
                    Choice::new(
                        front.player.clone(),
                        format!("cancel a hit at {}", self.system),
                        options,
                    )
                    .detailed("sustainers", sustainers)
                    .contextualized(
                        DecisionContext::new(
                            front.player.clone(),
                            DecisionSource::Rule("82".to_owned()),
                            "sustain_damage",
                            state.phase,
                            state.round,
                        )
                        .about(DecisionTarget::System(self.system.clone()))
                        .owing(OutstandingConstraint::new(
                            ConstraintKind::UnitsToRemove,
                            i64::try_from(front.hits).unwrap_or(0),
                            0,
                        )),
                    ),
                )
            }
            Stage::Assigning { queue, .. } => {
                let front = queue.first()?;
                let candidates = self.casualty_candidates(state, content, sources, front);
                if candidates.len() < 2 {
                    return None;
                }
                // One option per distinguishable loss; damage is part of what distinguishes.
                let mut seen = std::collections::BTreeSet::new();
                let mut options = Vec::new();
                for (index, unit) in &candidates {
                    let index = index.clone();
                    if !seen.insert((unit.type_id.to_string(), unit.sustained_damage)) {
                        continue;
                    }
                    let damaged = if unit.sustained_damage {
                        " (damaged)"
                    } else {
                        ""
                    };
                    options.push(
                        ChoiceOption::labelled(
                            format!("destroy|{index}"),
                            CASUALTY_KIND,
                            format!("destroy {}{damaged}", unit.type_id),
                        )
                        .with("unit", unit.type_id.to_string())
                        .with("damaged", unit.sustained_damage),
                    );
                }
                // A producer-assigned hit (Devotion) is the producer's choice of the victim's ship.
                let chooser = if front.producer_assigns {
                    front.producer.clone()
                } else {
                    front.player.clone()
                };
                Some(
                    Choice::new(chooser.clone(), "assign a hit", options).contextualized(
                        DecisionContext::new(
                            chooser,
                            DecisionSource::Rule("78.4".to_owned()),
                            "assign_casualty",
                            state.phase,
                            state.round,
                        )
                        .about(DecisionTarget::System(self.system.clone()))
                        .owing(OutstandingConstraint::new(
                            ConstraintKind::UnitsToRemove,
                            i64::try_from(front.hits).unwrap_or(0),
                            0,
                        )),
                    ),
                )
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per answer kind, read as a table"
    )]
    fn resolve(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        answer: ChoiceOption,
    ) -> Result<(), IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        let Some(choice) = self.pending_choice(state, content, sources) else {
            return Ok(());
        };
        let option = crate::choice::validate(&choice, answer)?;
        if !self.resolve_answer(state, ctx, &choice, &option)? {
            return Ok(());
        }
        self.settle(state, ctx)
            .map_err(CombatError::into_illegal_choice)?;
        Ok(())
    }
}

impl CombatWindow {
    /// [`Self::resolve_answer_string`], which on `driver=dual` also runs as the core machine awaiting the answer:
    /// the pending question is recorded with its answer, the core puts the same question, takes the same answer and
    /// must leave the same state, fields and events. `Ok(false)`: no hit left to settle for.
    fn resolve_answer(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        choice: &Choice,
        option: &ChoiceOption,
    ) -> Result<bool, IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        let composed = crate::dual_window::composed() && crate::dual::backend(crate::dual::Area::Driver) == crate::dual::Backend::Dual;
        let args = if composed { self.settle_args(state, ctx) } else { None };
        let Some(args) = args else { return self.resolve_answer_string(state, ctx, option) };
        let string_pays = match &self.stage {
            Stage::Sustaining { queue, .. } => queue.first().is_some_and(|front| crate::leaders::pays_on_sustain(state, content, &front.player)),
            _ => false,
        };
        let rng_before = ctx.rng.positions();
        let failed = std::cell::Cell::new(false);
        let failed_why = std::cell::Cell::new("");
        let mut outcome = None;
        crate::dual_window!(
            crate::dual::Area::Driver,
            "combat::answer",
            state,
            |s: &mut GameState| {
                crate::dual_window::note_ask(choice, &Ok(option.clone()));
                self.machine_bail = false;
                self.machine_bail_why = "";
                let result = self.resolve_answer_string(s, ctx, option);
                failed.set(result.is_err() || self.machine_bail);
                failed_why.set(if result.is_err() { "the string answer was refused" } else { self.machine_bail_why });
                let value = self.settle_out(Vec::new(), ctx.rng.positions());
                outcome = Some(result);
                crate::dual::Run::new(value)
            },
            |s: &mut GameState, script: &crate::dual_window::Recording| {
                if failed.get() {
                    crate::dual::note_bail_here(failed_why.get());
                    return None;
                }
                crate::dual_window::answer_machine(s, sources, args, &rng_before, script, string_pays)
            }
        );
        outcome.unwrap_or(Ok(true))
    }

    /// The answer to the pending decision (without the settle that follows). `Ok(false)`: nothing to settle.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per answer kind, read as a table"
    )]
    fn resolve_answer_string(
        &mut self,
        state: &mut GameState,
        ctx: &mut Resolving<'_>,
        option: &ChoiceOption,
    ) -> Result<bool, IllegalChoice> {
        let (content, sources) = (ctx.content, ctx.sources);
        match self.stage.clone() {
            Stage::Done(_)
            | Stage::Opening { .. }
            | Stage::Rolling { .. }
            | Stage::RollingAfterBarrage { .. } => {}
            Stage::Announcing {
                round,
                asking,
                mut announced,
            } => {
                if option.id == "retreat" {
                    announced.push(asking.clone());
                    // "After your opponent declares a retreat during a space combat." Named by the
                    // declaring player, so `actor_is_not` gives it to the opponent.
                    emit_retreat_declared(state, ctx, &self.system, &asking, round);
                }
                // 78.4b: the defender announcing silences the attacker.
                let next = (asking == self.defender && !announced.contains(&self.defender))
                    .then(|| self.attacker.clone());
                self.stage = next.map_or(Stage::Rolling { round }, |asking| Stage::Announcing {
                    round,
                    asking,
                    announced: announced.clone(),
                });
                if matches!(self.stage, Stage::Rolling { .. }) && !announced.is_empty() {
                    self.pending_retreats = announced;
                }
            }
            Stage::Retreating { round, mut leaving } => {
                if let Some(player) = leaving.first().cloned() {
                    let destination = SystemId::new(option.id.clone());
                    self.retreat(state, content, sources, ctx, &player, &destination);
                    leaving.remove(0);
                }
                self.stage = Stage::Retreating { round, leaving };
            }
            Stage::Sustaining { mut queue, round } => {
                let front_player = queue
                    .first()
                    .map_or_else(|| self.defender.clone(), |front| front.player.clone());
                // Whose roll produced the hit being cancelled: Direct Hit keys on it.
                let front_producer = queue
                    .first()
                    .map_or_else(|| self.defender.clone(), |front| front.producer.clone());
                if option.is_decline() {
                    self.stage = Stage::Assigning { queue, round };
                } else if let Some(location) = option.id.strip_prefix("sustain|") {
                    let planet_side: std::collections::BTreeSet<(ti4_model::id::PlanetId, usize)> =
                        planet_combatants(state, content, sources, &front_player, &self.system)
                            .into_iter()
                            .map(|(planet, index, _)| (planet, index))
                            .collect();
                    let sustained = if let Ok(index) = location.parse::<usize>() {
                        state.system_mut(&self.system).units.get_mut(index)
                    } else if let Some((planet, variant_index)) =
                        planetary_sustain_location(location)
                    {
                        let planet_id = ti4_model::id::PlanetId::new(planet);
                        state
                            .system_mut(&self.system)
                            .planet_units
                            .get_mut(&planet_id)
                            .and_then(|units| {
                                if let Some(index) = variant_index {
                                    units.get_mut(index).filter(|unit| {
                                        unit.owner == front_player
                                            && planet_side.contains(&(planet_id.clone(), index))
                                            && !unit.sustained_damage
                                    })
                                } else {
                                    units.iter_mut().find(|unit| {
                                        unit.owner == front_player
                                            && unit.type_id.as_str() == "naaz_voltron"
                                            && !unit.sustained_damage
                                    })
                                }
                            })
                    } else {
                        None
                    }
                    .map(|unit| {
                        *unit = unit.sustained();
                        unit.clone()
                    });
                    // Two printed windows read this moment -- "when one of your ships uses SUSTAIN
                    // DAMAGE" and "after another player's ship uses SUSTAIN DAMAGE to cancel a hit
                    // produced by your units". Both need the *unit*, so the event names it.
                    if let Some(unit) = sustained {
                        let kind = unit.type_id.to_string();
                        remember_sustain_target(state, &self.system, location, &unit);
                        // Rear Admiral Farran in a real combat (rules version 17; the window's
                        // sustain step never asked before).
                        if state.rules_version >= ti4_model::state::RULES_FACTION_BUGFIXES {
                            ask_sustain_commander_windowed(
                                state,
                                ctx,
                                content,
                                sources,
                                self.galaxy.as_ref(),
                                &front_player,
                            )?;
                        }
                        emit_sustain_used(
                            state,
                            ctx,
                            self.galaxy.as_ref(),
                            &self.system,
                            &front_player,
                            &kind,
                            &front_producer,
                            true,
                        )
                        .map_err(CombatError::into_illegal_choice)?;
                    }
                    // Non-Euclidean Shielding: "cancel 2 hits instead of 1" per use (version 17).
                    let cancelled = if state.rules_version
                        >= ti4_model::state::RULES_FACTION_BUGFIXES
                        && non_euclidean_shielding(state, &front_player)
                    {
                        2
                    } else {
                        1
                    };
                    if let Some(front) = queue.first_mut() {
                        front.hits = front.hits.saturating_sub(cancelled);
                    }
                    self.stage = Stage::Sustaining { queue, round };
                }
            }
            Stage::Assigning { mut queue, round } => {
                let Some(front) = queue.first().cloned() else {
                    return Ok(false);
                };
                let units = ships_of(state, content, sources, &front.player, &self.system);
                let index = option
                    .id
                    .strip_prefix("destroy|")
                    .and_then(|rest| rest.parse::<usize>().ok())
                    .unwrap_or(0);
                if let Some(doomed) = units.get(index) {
                    let doomed = doomed.clone();
                    remove_combat_ship(state, &self.system, &doomed);
                    announce_ship_destroyed(state, ctx, &self.system, &front.player, &doomed);
                }
                if let Some(front) = queue.first_mut() {
                    front.hits = front.hits.saturating_sub(1);
                }
                // Back to sustaining: the next hit may be cancellable even if this one was not.
                self.stage = Stage::Sustaining { queue, round };
            }
        }
        Ok(true)
    }
}

/// Fight a space combat to its end (LRR 78).
///
/// Returns immediately when fewer than two players have ships (78.1).
///
/// # Errors
/// [`CombatError::IllegalChoice`] when a decider answers with something not offered.
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
) -> Result<CombatOutcome, CombatError> {
    let mut ctx = Resolving {
        content,
        sources,
        dice,
        rng,
        table,
        timing: None,
    };
    resolve_resolving(state, &mut ctx, system, None)
}

/// [`resolve`] inside a caller's [`Resolving`], so a combat started by an effect (the Mahact hero)
/// opens the same typed windows a tactical action's combat does when `ctx` carries a timing handle.
///
/// # Errors
/// As [`resolve`].
pub(crate) fn resolve_resolving(
    state: &mut GameState,
    ctx: &mut Resolving<'_>,
    system: &SystemId,
    galaxy: Option<&Galaxy>,
) -> Result<CombatOutcome, CombatError> {
    let (content, sources) = (ctx.content, ctx.sources);
    let before = before_combat(state, content, sources, system);
    let mut window = CombatWindow::new(state, content, sources, system);
    if let Some(galaxy) = galaxy {
        window = window.with_galaxy(galaxy.clone());
    }
    // Opening does not roll; settle once so a fight that is already over reports so.
    window.settle(state, ctx)?;
    while window.outcome().is_none() {
        window.drive(state, ctx)?;
        if window.outcome().is_some() {
            break;
        }
        // The synchronous API has no outer Game scoring window. Preserve the occurrence facts,
        // consume the pause, and continue the same combat to completion.
        let _ = window.take_scoring_occurrence();
        window.settle_open(state, ctx)?;
    }
    complete_window(state, content, sources, system, &before, &window)
        .ok_or_else(|| CombatError::Unresolved(system.clone()))
}

/// Complete a driven combat window: return its outcome and record event feats at completion.
/// Both the synchronous API and the stepped test harness call this so they cannot drift apart
/// (M07-022). The Game driver keeps its own inline bookkeeping, because there a noted occurrence
/// pauses for scoring before the fight is over.
fn complete_window(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &BeforeCombat,
    window: &CombatWindow,
) -> Option<CombatOutcome> {
    let outcome = window.outcome()?;
    if let Some(occurrence) = window.combat_occurrence() {
        note_combat_event_feats(
            state, content, sources, system, before, &outcome, occurrence,
        );
    }
    Some(outcome)
}

/// What a system held before a space combat, for the feats that ask what changed.
///
/// Taken before the first die because every fact in it is destroyed by the fight itself: the war
/// sun that was killed is gone, and the loser's fleet is gone with it. A card that asks "did you
/// destroy their flagship" cannot be answered from the wreckage.
#[derive(Debug, Clone, Default)]
pub struct BeforeCombat {
    /// The players with ships when the fight opened, in seating order.
    sides: Vec<PlayerId>,
    /// War suns and flagships each side had, to see which were destroyed.
    capitals: std::collections::BTreeMap<PlayerId, usize>,
    /// Whose promissory notes each side held when the tactical action started. Standalone combat
    /// callers snapshot when their combat opens because they have no enclosing tactical window.
    notes: NoteHoldings,
}

impl BeforeCombat {
    /// The players with ships when the fight opened, in seating order. A Salvage played into
    /// the win window knows its opponents only from this list — by then the losers' ships are
    /// off the board and the board itself answers with the winner alone.
    #[must_use]
    pub fn sides(&self) -> &[PlayerId] {
        &self.sides
    }

    /// Whose promissory notes each side held when the tactical action started.
    #[must_use]
    pub fn notes(&self) -> &NoteHoldings {
        &self.notes
    }

    /// The war suns and flagships each side had when the fight opened, in the map's (player id) order.
    pub fn capitals(&self) -> impl Iterator<Item = (&PlayerId, usize)> {
        self.capitals.iter().map(|(player, n)| (player, *n))
    }
}

/// Promissory-note issuers held by each player at a defined timing boundary.
pub type NoteHoldings = std::collections::BTreeMap<PlayerId, std::collections::BTreeSet<PlayerId>>;

/// Snapshot note holdings for objectives that name the start of a tactical action.
///
/// Betray a Friend counts only notes in the holder's **play area** (faceup), not hand-held
/// notes: "'In your play area' is not the same as 'in your hand'." Support for the Throne is
/// play-area by construction, which is why it lives in `support_holders` and needs no filter.
#[must_use]
pub fn note_holdings(state: &GameState) -> NoteHoldings {
    let mut notes = NoteHoldings::new();
    for seat in &state.players {
        let mut issuers = std::collections::BTreeSet::new();
        for (note, holder) in &state.promissory_notes {
            // The key is note_id(alias, owner_faction): the suffix names the issuer's faction,
            // so the issuer is that faction's seat. A PlayerId built from the faction name
            // matches no seat and could never fire; an unseated owner resolves to nothing.
            // Unlike rival_note_issuers_count there is deliberately no own-faction guard: baf
            // tests notes[winner].contains(loser) with winner != loser, and a seat can never hold
            // its own faction's note — deal keeps each seat's own notes in hand (never face-up),
            // and factions are unique per seat, so no transfer can deliver one.
            if holder == &seat.id
                && state.promissory_faceup.contains(note)
                && let Some(owner_faction) = crate::promissory::owner_of(note)
                && let Some(issuer_seat) = crate::promissory::seat_of(state, &owner_faction)
            {
                issuers.insert(issuer_seat);
            }
        }
        for (owner, holder) in &state.support_holders {
            if holder == &seat.id {
                issuers.insert(owner.clone());
            }
        }
        notes.insert(seat.id.clone(), issuers);
    }
    notes
}

/// Whether `system` is another player's home system from `winner`'s perspective.
#[must_use]
pub fn is_rival_home_system(state: &GameState, winner: &PlayerId, system: &SystemId) -> bool {
    state
        .players
        .iter()
        .any(|seat| seat.id != *winner && seat.home_system.as_ref() == Some(system))
}

/// Snapshot a system before a space combat.
#[must_use]
pub fn before_combat(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> BeforeCombat {
    before_combat_with_notes(state, content, sources, system, note_holdings(state))
}

/// Snapshot combat-only facts while retaining note holdings from the tactical-action boundary.
#[must_use]
pub fn before_combat_with_notes(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    notes: NoteHoldings,
) -> BeforeCombat {
    let sides = combatants(state, content, sources, system);
    let mut capitals = std::collections::BTreeMap::new();
    for side in &sides {
        capitals.insert(
            side.clone(),
            capital_ships_of(state, content, sources, side, system),
        );
    }
    BeforeCombat {
        sides,
        capitals,
        notes,
    }
}

/// Record what a finished space combat did, for the secrets that ask about the event.
///
/// Six of the thirteen unimplemented secrets are decided here. None of them can be read off the
/// board afterwards, which is why they had no requirement: "win a combat in an anomaly" leaves
/// the same board as losing one somewhere else.
pub fn note_combat_feats(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &BeforeCombat,
    outcome: &CombatOutcome,
) {
    let occurrence = state.begin_feat_occurrence();
    let _ = note_combat_feats_at(state, content, sources, system, before, outcome, occurrence);
}

/// Record finished-combat feats against the concrete combat that caused them.
///
/// Returns whether the resolution created at least one event feat, so callers can skip opening
/// an empty scoring window.
pub fn note_combat_event_feats(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &BeforeCombat,
    outcome: &CombatOutcome,
    occurrence: FeatOccurrence,
) -> bool {
    note_combat_feats_at(state, content, sources, system, before, outcome, occurrence)
}

fn record_combat_feat(
    state: &mut GameState,
    player: &PlayerId,
    feat: Feat,
    occurrence: FeatOccurrence,
) {
    state.record_event_feat(player, feat, occurrence);
}

fn note_combat_feats_at(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &BeforeCombat,
    outcome: &CombatOutcome,
    occurrence: FeatOccurrence,
) -> bool {
    // `rules::combat::feats` runs beside the string body on `driver=dual` and the two are compared (state and result).
    crate::dual::write(
        crate::dual::Area::Driver,
        state,
        "combat::feats",
        |s| note_combat_feats_text(s, content, sources, system, before, outcome, occurrence),
        |s| crate::cannon_dual::combat_feats(s, content, sources, system, before, outcome, occurrence),
    )
}

fn note_combat_feats_text(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &BeforeCombat,
    outcome: &CombatOutcome,
    occurrence: FeatOccurrence,
) -> bool {
    if outcome.rounds == 0 {
        // Nobody fought, so nothing was won and nothing was destroyed.
        return false;
    }
    let mut noted = false;

    // Destroy Their Greatest Ship. In a two-sided fight whoever is not the owner did the
    // destroying, so a drop in one side's count is the other side's feat — including when both
    // sides lost one and both score.
    for side in &before.sides {
        let had = before.capitals.get(side).copied().unwrap_or(0);
        if had == 0 {
            continue;
        }
        if capital_ships_of(state, content, sources, side, system) < had {
            for other in &before.sides {
                if other != side {
                    record_combat_feat(state, other, Feat::DestroyedACapitalShip, occurrence);
                    noted = true;
                }
            }
        }
    }

    // Demonstrate Your Power asks about the fleet at the end of the combat and says nothing
    // about winning, so it is offered to whoever is still standing there.
    for side in &before.sides {
        if non_fighter_ships_of(state, content, sources, side, system) >= 3 {
            record_combat_feat(
                state,
                side,
                Feat::HeldThreeShipsAfterASpaceCombat,
                occurrence,
            );
            noted = true;
        }
    }

    let Some(winner) = outcome.winner.clone() else {
        // A draw wins nothing. Skilled Retreat exists to produce exactly this, and treating the
        // survivor as the winner would score four cards off a fight nobody won.
        return noted;
    };

    if ti4_content::galaxy::all_systems(content, sources)
        .get(system.as_str())
        .is_some_and(ti4_content::galaxy::System::is_anomaly)
    {
        record_combat_feat(state, &winner, Feat::WonInAnAnomaly, occurrence);
        noted = true;
    }

    if is_rival_home_system(state, &winner, system) {
        record_combat_feat(state, &winner, Feat::WonInARivalHome, occurrence);
        noted = true;
    }

    if capital_ships_of(state, content, sources, &winner, system) > 0
        && has_surviving_flagship(state, content, sources, &winner, system)
    {
        record_combat_feat(
            state,
            &winner,
            Feat::WonBesideASurvivingFlagship,
            occurrence,
        );
        noted = true;
    }

    // "The most victory points" includes a tie: nothing in the card breaks one, and the leader
    // board holding two names does not make either of them not the leader.
    let most = state
        .players
        .iter()
        .map(|seat| seat.victory_points)
        .max()
        .unwrap_or(0);
    for loser in &before.sides {
        if loser == &winner {
            continue;
        }
        if state
            .player(loser)
            .is_some_and(|seat| seat.victory_points == most)
        {
            record_combat_feat(state, &winner, Feat::WonAgainstThePointsLeader, occurrence);
            noted = true;
        }
        if before
            .notes
            .get(&winner)
            .is_some_and(|issuers| issuers.contains(loser))
        {
            record_combat_feat(state, &winner, Feat::WonAgainstANoteHolder, occurrence);
            noted = true;
        }
    }
    noted
}

/// Whether this player still has a flagship in the system.
fn has_surviving_flagship(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &SystemId,
) -> bool {
    let types = catalogue(content, sources);
    state
        .system_state(system)
        .units
        .iter()
        .filter(|unit| &unit.owner == player)
        .any(|unit| {
            types
                .get(unit.type_id.as_str())
                .is_some_and(|kind| kind.base_type() == "flagship")
        })
}

fn finished(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    attacker: &PlayerId,
    defender: &PlayerId,
) -> bool {
    ships_of(state, content, sources, attacker, system).is_empty()
        || ships_of(state, content, sources, defender, system).is_empty()
}

fn winner(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    attacker: &PlayerId,
    defender: &PlayerId,
) -> Option<PlayerId> {
    let attacker_alive = !ships_of(state, content, sources, attacker, system).is_empty();
    let defender_alive = !ships_of(state, content, sources, defender, system).is_empty();
    match (attacker_alive, defender_alive) {
        (true, false) => Some(attacker.clone()),
        (false, true) => Some(defender.clone()),
        // Both wiped out is a draw, and both alive cannot happen at a decision point.
        _ => None,
    }
}

#[cfg(test)]
mod tests {

    /// PDS II fires into the next system, and PDS I does not.
    ///
    /// "You may use this unit's SPACE CANNON against ships that are in adjacent systems" is printed
    /// on PDS II and on Xxcha's Indomitus mech. `space_cannon_offense` read only the activated
    /// system, so neither ever fired -- an upgraded PDS next door, from a technology every faction
    /// can research, did nothing at all.
    #[test]
    fn an_upgraded_pds_fires_from_an_adjacent_system() {
        let hub = crate::fixtures::plain_hub();
        let active = SystemId::new(&hub.centre);
        let next_door = SystemId::new(&hub.outer[0]);
        let planet = ti4_model::id::PlanetId::new("a_planet_next_door");

        let mut state = crate::fixtures::game(&["a", "b"]);
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            state.board.entry(SystemId::new(id)).or_default();
        }
        put(&mut state, &active, "cruiser", &attacker(), 1);

        let types = catalogue(ContentStore::embedded(), POK);
        let guns = |state: &GameState| {
            reaching_guns_by(
                state,
                &types,
                Some(&hub.galaxy),
                &active,
                |owner| owner != &attacker(),
                |_| false,
            )
            .len()
        };

        if let Some(here) = state.board.get_mut(&next_door) {
            here.planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new("pds"), defender()));
        }
        assert_eq!(guns(&state), 0, "a PDS I stays at home");

        if let Some(here) = state.board.get_mut(&next_door) {
            here.planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new("pds2"), defender()));
        }
        assert_eq!(guns(&state), 1, "an upgraded one reaches next door");

        if let Some(here) = state.board.get_mut(&next_door) {
            here.planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new("xxcha_mech"), defender()));
        }
        assert_eq!(guns(&state), 2, "and so does Xxcha's Indomitus mech");

        // The flagship is a ship: it is never on a planet, and the planet-only scan could not see
        // it however the unit list was written.
        if let Some(here) = state.board.get_mut(&next_door) {
            here.units
                .push(Unit::new(UnitTypeId::new("xxcha_flagship"), defender()));
        }
        assert_eq!(
            guns(&state),
            3,
            "Loncara Ssodu reaches next door from the space area"
        );

        if let Some(here) = state.board.get_mut(&next_door) {
            here.planet_units
                .entry(planet)
                .or_default()
                .push(Unit::new(UnitTypeId::new("titans_pds2"), defender()));
        }
        assert_eq!(guns(&state), 4, "and so does Hel-Titan II");
    }

    /// Every unit whose card grants the adjacent-system clause is found, and nothing else is.
    ///
    /// The predicate reads prose, so it is pinned against the corpus rather than trusted. A
    /// reworded card or a newly added unit fails here instead of quietly losing a gun, which is
    /// how the previous hard-coded `["pds2", "xxcha_mech"]` list came to be missing half the units
    /// that carry the clause.
    #[test]
    fn every_unit_that_reaches_an_adjacent_system_is_found() {
        let types = catalogue(ContentStore::embedded(), POK);
        let mut reaching: Vec<&str> = types
            .iter()
            .filter(|(_, kind)| reaches_adjacent(**kind))
            .map(|(id, _)| *id)
            .collect();
        reaching.sort_unstable();
        assert_eq!(
            reaching,
            vec!["pds2", "titans_pds2", "xxcha_flagship", "xxcha_mech"],
            "the corpus units carrying \"SPACE CANNON against ships that are ... adjacent\""
        );
        for id in &reaching {
            assert!(
                types
                    .get(id)
                    .is_some_and(|kind| kind.space_cannon_hits_on().is_some()),
                "{id} reaches an adjacent system but has no SPACE CANNON to fire"
            );
        }
    }

    /// OBS-003d: the retreat-announce choice a fresh combat opens into names its rule and the
    /// system fought over.
    #[test]
    fn obs003d_announce_retreat_carries_its_typed_context() {
        let hub = crate::fixtures::plain_hub();
        let system = SystemId::new(&hub.centre);
        let mut state = crate::fixtures::game(&["a", "b"]);
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            state.board.entry(SystemId::new(id)).or_default();
        }
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);
        // A destination: the defender must already hold a system to retreat to (78.7c), and the
        // outer ring starts empty.
        let refuge = SystemId::new(&hub.outer[0]);
        state.board.entry(refuge.clone()).or_default();
        put(&mut state, &refuge, "fighter", &defender(), 1);

        let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system)
            .with_galaxy(hub.galaxy);
        // As it stands once round 1 has opened (no barrage between two fighters).
        window.stage = Stage::Announcing {
            round: 1,
            asking: defender(),
            announced: Vec::new(),
        };
        let choice = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("round 1 asks the defender to announce");
        let context = choice.context.as_ref().expect("typed context");
        assert_eq!(context.source, DecisionSource::Rule("78.9".to_owned()));
        assert_eq!(context.subtype, "announce_retreat");
        assert_eq!(context.target, Some(DecisionTarget::System(system)));
    }

    /// OBS-003d: casualty assignment and sustain damage are typed distinctly from each other and
    /// from the retreat announcement above, though all three arise from the same combat.
    #[test]
    fn obs003d_assigning_and_sustaining_are_typed_distinctly() {
        let (mut state, system) = arena();
        // Two distinguishable ships, so Assigning has a real choice (a lone one resolves without
        // asking), and a dreadnought, which has SUSTAIN DAMAGE, for Sustaining to offer.
        put(&mut state, &system, "cruiser", &defender(), 1);
        put(&mut state, &system, "destroyer", &defender(), 1);
        put(&mut state, &system, "dreadnought", &defender(), 1);
        let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);

        window.stage = Stage::Assigning {
            queue: vec![Pending {
                player: defender(),
                hits: 1,
                producer: attacker(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let assigning = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("a hit is queued");
        assert_eq!(
            assigning.context.as_ref().unwrap().subtype,
            "assign_casualty"
        );

        window.stage = Stage::Sustaining {
            queue: vec![Pending {
                player: defender(),
                hits: 1,
                producer: attacker(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let sustaining = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("a hit is queued");
        assert_eq!(
            sustaining.context.as_ref().unwrap().subtype,
            "sustain_damage"
        );
        assert_ne!(
            assigning.context.as_ref().unwrap().subtype,
            sustaining.context.as_ref().unwrap().subtype
        );
    }

    /// Forced hits (L1Z1X dreadnoughts) go to non-fighter ships; the window's sustain question
    /// says so on "take the hit", as the cannon path does, so a client never plans a fighter
    /// loss the next question will not offer.
    #[test]
    fn a_windowed_forced_hit_marks_its_sustain_question_bound_to_non_fighters() {
        for forced in [true, false] {
            let (mut state, system) = arena();
            put(&mut state, &system, "dreadnought", &defender(), 1);
            put(&mut state, &system, "fighter", &defender(), 1);
            let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
            window.stage = Stage::Sustaining {
                queue: vec![Pending {
                    player: defender(),
                    hits: 1,
                    producer: attacker(),
                    non_fighters_only: forced,
                    producer_assigns: false,
                }],
                round: 1,
            };
            let sustaining = window
                .pending_choice(&state, ContentStore::embedded(), POK)
                .expect("a hit is queued");
            let take = sustaining
                .options
                .iter()
                .find(|option| option.is_decline())
                .expect("taking the hit is offered");
            assert_eq!(
                take.payload
                    .get("non_fighters_first")
                    .and_then(serde_json::Value::as_bool),
                forced.then_some(true),
                "forced = {forced}"
            );
        }
    }

    /// Hits outside a combat window (space cannon, barrage) are absorbed one ask at a time. Each
    /// ask must say how many hits are still owed, or a client cannot stage them together and
    /// has to guess the amount.
    #[test]
    fn absorbing_hits_outside_a_window_states_the_hits_still_owed() {
        let content = ContentStore::embedded();
        let (mut state, system) = arena();
        let player = defender();
        put(&mut state, &system, "dreadnought", &player, 1);
        put(&mut state, &system, "cruiser", &player, 2);
        put(&mut state, &system, "fighter", &player, 2);

        let (capturing, seen) = crate::choice::Capturing::new(Box::new(crate::choice::FirstOption));
        let mut table = Table::with_default(Box::new(capturing));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        absorb_hits_seeing(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &player,
            &system,
            &attacker(),
            3,
        )
        .expect("the hits resolve");

        let owed: Vec<(String, i64)> = seen
            .borrow()
            .iter()
            .filter_map(|choice| {
                let context = choice.context.as_ref()?;
                let amount = context.outstanding.first()?.amount;
                Some((context.subtype.clone(), amount))
            })
            .collect();
        assert_eq!(
            owed.first(),
            Some(&("sustain_damage".to_owned(), 3)),
            "the sustain ask owes every hit: {owed:?}"
        );
        let casualties: Vec<_> = owed
            .iter()
            .filter(|(subtype, _)| subtype == "assign_casualty")
            .collect();
        assert!(!casualties.is_empty(), "a casualty ask followed: {owed:?}");
        assert!(
            seen.borrow().iter().all(|choice| choice
                .context
                .as_ref()
                .is_some_and(|c| !c.outstanding.is_empty())),
            "no ask leaves the amount unstated"
        );
    }

    /// Non-Euclidean Shielding cancels two hits per sustain, not two sustains.
    ///
    /// "When 1 of your units uses SUSTAIN DAMAGE, cancel 2 hits instead of 1" -- so one dreadnought
    /// absorbs two hits and takes one point of damage. A reading that cancelled two hits by
    /// damaging two ships would cost the holder twice what the technology is for.
    #[test]
    fn non_euclidean_shielding_cancels_two_hits_for_one_damaged_ship() {
        let content = ContentStore::embedded();
        let (mut state, system) = arena();
        let player = defender();
        put(&mut state, &system, "dreadnought", &player, 1);

        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut inner = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut inner,
            timing: None,
        };

        let left = offer_sustain(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &player,
            &system,
            &attacker(),
            2,
            HitOrigin::CombatRoll,
            false,
            false,
        )
        .expect("the offer resolves");
        assert_eq!(
            left, 1,
            "without the technology one sustain cancels one hit"
        );

        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &player, 1);
        if let Some(seat) = state.player_mut(&player) {
            seat.technologies
                .insert(ti4_model::id::TechnologyId::new("nes"));
        }
        let left = offer_sustain(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &player,
            &system,
            &attacker(),
            2,
            HitOrigin::CombatRoll,
            false,
            false,
        )
        .expect("the offer resolves");
        assert_eq!(left, 0, "with it, the same one ship cancels both");
        assert_eq!(
            state
                .system_state(&system)
                .units_of(&player)
                .iter()
                .filter(|unit| unit.sustained_damage)
                .count(),
            1,
            "and only one ship is damaged"
        );
    }

    /// The Thalnos cards reach combat rounds and nothing else.
    ///
    /// Both read "During each combat round". A space cannon roll is a step of a tactical action, a
    /// barrage is a step of a space combat, and a bombardment is a step of an invasion -- none of
    /// them is a round, so neither card may touch their dice. They stage through the same window as
    /// the fleet roll, which is exactly why the scope has to be checked rather than assumed.
    #[test]
    fn thalnos_does_not_reach_a_space_cannon_roll() {
        let (mut state, system) = arena();
        let player = attacker();
        put(&mut state, &system, "cruiser", &player, 1);
        if let Some(seat) = state.player_mut(&player) {
            seat.relics = vec![ti4_model::id::RelicId::new("thalnos")];
        }
        let staged = |state: &mut GameState, kind: &str| {
            state.reroll_staging.insert(
                player.clone(),
                RerollSet {
                    kind: kind.to_owned(),
                    system: system.clone(),
                    rolls: vec![RerollEntry {
                        unit: "pds".to_owned(),
                        unit_types: std::collections::BTreeMap::new(),
                        planet: None,
                        hits_on: Some(5),
                        faces: vec![1],
                        rerolled: std::collections::BTreeSet::new(),
                        deltas: std::collections::BTreeMap::new(),
                    }],
                },
            );
        };
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut inner = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut ctx = crate::choice::Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut inner,
            timing: None,
        };

        staged(&mut state, "space_cannon");
        reroll_window(&mut state, &mut ctx, &player, true);
        assert_eq!(
            state.reroll_staging.get(&player).expect("staged").rolls[0].faces,
            vec![1],
            "a space cannon roll is not a combat round"
        );

        // A fleet roll *is* a combat round, so the relic takes it. Either the die was rerolled or
        // the unit that rerolled and still missed was destroyed with its entry -- both prove the
        // card fired, and which one happens depends on the draw.
        staged(&mut state, "fleet");
        reroll_window(&mut state, &mut ctx, &player, false);
        let after = state.reroll_staging.get(&player).expect("staged");
        assert!(
            after
                .rolls
                .first()
                .is_none_or(|entry| entry.faces != vec![1]),
            "but a fleet roll is, and the relic takes it"
        );
    }

    /// 59.5: the defender rolls one better in a nebula, and the attacker does not.
    ///
    /// Applied to the threshold because that is where every other roll modifier lives -- +1 to the
    /// die is the same as needing one less, and keeping them in one place is what stops two
    /// modifiers disagreeing about which direction they push.
    #[test]
    fn a_nebula_helps_the_defender_only() {
        let content = ContentStore::embedded();
        let nebula = SystemId::new(crate::fixtures::a_system_where("nebula"));
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.board.entry(nebula.clone()).or_default();
        state.active = Some(attacker());
        state.active_system = Some(nebula.clone());

        let cruiser = Unit::new(UnitTypeId::new("cruiser"), attacker());
        let printed = hits_on(content, POK, &cruiser).expect("a cruiser has a combat value");

        assert_eq!(
            effective_hits_on(&state, content, POK, &attacker(), &cruiser),
            Some(printed),
            "the attacker gets nothing from the nebula"
        );
        assert_eq!(
            effective_hits_on(&state, content, POK, &defender(), &cruiser),
            Some(printed - 1),
            "the defender needs one less"
        );

        // And nowhere else: the same fleet in an ordinary system is unmodified.
        let plain = SystemId::new(crate::fixtures::plain_systems(1)[0].clone());
        state.active_system = Some(plain);
        assert_eq!(
            effective_hits_on(&state, content, POK, &defender(), &cruiser),
            Some(printed),
            "an ordinary system helps nobody"
        );
    }

    /// Shields Holding cancels hits across the round, not per assignment.
    ///
    /// "Cancel up to 2 hits" is two hits in that combat round. Granting two and spending them one
    /// at a time must leave none, which is what `spend_cancellations` returning the smaller of
    /// wanted and available buys.
    #[test]
    fn granted_hit_cancellations_are_spent_once_each() {
        let player = PlayerId::new("a");
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.combat_round_seq = 4;

        grant_hit_cancellation(&mut state, &player, 2);
        assert_eq!(cancellable_hits(&state, &player), 2);

        assert_eq!(spend_cancellations(&mut state, &player, 1), 1);
        assert_eq!(cancellable_hits(&state, &player), 1);
        assert_eq!(
            spend_cancellations(&mut state, &player, 5),
            1,
            "capped by what is left"
        );
        assert_eq!(cancellable_hits(&state, &player), 0);
    }

    /// A cancellation belongs to the round it was granted in.
    #[test]
    fn hit_cancellations_expire_with_their_round() {
        let player = PlayerId::new("a");
        let mut state = crate::fixtures::game(&["a"]);
        state.combat_round_seq = 4;
        grant_hit_cancellation(&mut state, &player, 2);

        state.combat_round_seq = 5;
        assert_eq!(
            cancellable_hits(&state, &player),
            0,
            "the card names this combat round"
        );
    }

    /// Intercept bars a retreat for its round and no longer.
    #[test]
    fn a_barred_retreat_lasts_one_round() {
        let player = PlayerId::new("a");
        let mut state = crate::fixtures::game(&["a"]);
        state.combat_round_seq = 2;
        assert!(!retreat_barred(&state, &player));

        bar_retreat(&mut state, &player);
        assert!(retreat_barred(&state, &player));

        state.combat_round_seq = 3;
        assert!(!retreat_barred(&state, &player));
    }

    #[test]
    fn skilled_retreat_may_go_where_an_ordinary_retreat_may_not() {
        // 78.7c demands the destination hold your units or a planet you control. The card asks
        // only for "an adjacent system that does not contain another player's ships", so reusing
        // the stricter rule would quietly make it weaker than printed.
        let hub = crate::fixtures::plain_hub();
        let mine = PlayerId::new("a");
        let system = SystemId::new(hub.centre.clone());
        let state = crate::fixtures::game(&["a", "b"]);

        let ordinary = eligible_retreats(
            &state,
            ContentStore::embedded(),
            POK,
            &hub.galaxy,
            &mine,
            &system,
        );
        let skilled = skilled_retreat_destinations(
            &state,
            ContentStore::embedded(),
            POK,
            &hub.galaxy,
            &mine,
            &system,
        );

        assert!(
            ordinary.is_empty(),
            "an empty ring holds none of this player's units"
        );
        assert_eq!(
            skilled.len(),
            6,
            "but every ring seat is free of enemy ships"
        );
    }

    #[test]
    fn skilled_retreat_will_not_go_where_an_enemy_fleet_sits() {
        let hub = crate::fixtures::plain_hub();
        let mine = PlayerId::new("a");
        let theirs = PlayerId::new("b");
        let system = SystemId::new(hub.centre.clone());
        let occupied = SystemId::new(hub.outer[0].clone());

        let mut state = crate::fixtures::game(&["a", "b"]);
        crate::fixtures::put(&mut state, &occupied, "cruiser", &theirs, 1);

        let open = skilled_retreat_destinations(
            &state,
            ContentStore::embedded(),
            POK,
            &hub.galaxy,
            &mine,
            &system,
        );

        assert_eq!(open.len(), 5);
        assert!(!open.contains(&occupied), "another player's ships bar it");
    }

    /// Drive a windowed combat on a map: `stepped_fight`'s loop, plus the galaxy, because
    /// 78.7's retreat rules have no destinations to speak of without one.
    fn stepped_fight_on_map(
        state: &mut GameState,
        system: &SystemId,
        galaxy: ti4_content::galaxy::Galaxy,
        table: &mut Table,
        dice: &mut Dice,
        rng: &mut GameRng,
    ) -> CombatOutcome {
        let content = ContentStore::embedded();
        let before = crate::combat::before_combat(state, content, POK, system);
        let mut window = CombatWindow::new(state, content, POK, system).with_galaxy(galaxy);
        let mut inner = Table::new();
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice,
            rng,
            table: &mut inner,
            timing: None,
        };
        window.settle(state, &mut ctx).unwrap();
        while window.outcome().is_none() {
            if let Some(choice) = window.pending_choice(state, content, POK) {
                let answer = table.ask(&choice).unwrap();
                window.resolve(state, &mut ctx, answer).unwrap();
            } else {
                let _ = window.take_scoring_occurrence();
                window.settle_open(state, &mut ctx).unwrap();
            }
        }
        crate::combat::complete_window(state, content, POK, system, &before, &window)
            .expect("the fight resolved")
    }

    #[test]
    fn a_det_owner_may_retreat_into_an_empty_adjacent_system() {
        // DET: "Your ships can retreat into adjacent systems that do not contain other
        // players' units, even if you do not have units or control planets in that system."
        // The technology waives 78.7c's own-presence demand, so an empty neighbour is a
        // legal destination for the holder, and the window must offer it. Preloaded all-
        // miss dice against a zero-dice Winnu flagship make the 50-round stalemate
        // choice-free, so nothing reaches the table before the retreat announcement.
        let hub = crate::fixtures::plain_hub();
        let centre = SystemId::new(hub.centre.clone());
        let empty = SystemId::new(hub.outer[0].clone());

        let mut state = crate::fixtures::game(&["a", "b"]);
        state
            .player_mut(&attacker())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("det"));
        put(&mut state, &centre, "flagship", &attacker(), 1);
        put(&mut state, &centre, "winnu_flagship", &defender(), 1);

        let mut table = Table::with_default(Box::new(Scripted::new([
            "retreat".to_owned(),
            hub.outer[0].clone(),
        ])));
        let mut dice = Dice::from_faces(vec![1u32; 120]);
        let mut rng = GameRng::new(7);
        stepped_fight_on_map(
            &mut state,
            &centre,
            hub.galaxy.clone(),
            &mut table,
            &mut dice,
            &mut rng,
        );

        assert!(
            state
                .system_state(&centre)
                .units
                .iter()
                .any(|unit| unit.owner == defender()),
            "the defender's fleet stayed"
        );
        assert!(
            !state
                .system_state(&centre)
                .units
                .iter()
                .any(|unit| unit.owner == attacker()),
            "the DET holder's fleet left the combat system"
        );
        assert!(
            state
                .system_state(&empty)
                .units
                .iter()
                .any(|unit| unit.owner == attacker()),
            "it went to the empty neighbour the technology opens up"
        );
    }

    #[test]
    fn without_det_a_fleet_with_nowhere_to_go_is_not_asked_to_retreat() {
        // The relaxation belongs to the DET holder: without it, 78.7c still demands a
        // destination that holds your units or a planet you control, so a fleet with
        // nowhere to go is not asked at all (78.4c) and simply stays.
        let hub = crate::fixtures::plain_hub();
        let centre = SystemId::new(hub.centre.clone());

        let mut state = crate::fixtures::game(&["a", "b"]);
        put(&mut state, &centre, "flagship", &attacker(), 1);
        put(&mut state, &centre, "winnu_flagship", &defender(), 1);

        let mut table = Table::new();
        let mut dice = Dice::from_faces(vec![1u32; 120]);
        let mut rng = GameRng::new(7);
        stepped_fight_on_map(
            &mut state,
            &centre,
            hub.galaxy.clone(),
            &mut table,
            &mut dice,
            &mut rng,
        );

        assert!(
            state
                .system_state(&centre)
                .units
                .iter()
                .any(|unit| unit.owner == attacker()),
            "with no legal destination the fleet has to stay"
        );
        assert!(
            state
                .system_state(&centre)
                .units
                .iter()
                .any(|unit| unit.owner == defender()),
            "and so does the defender's"
        );
    }

    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;

    use super::*;
    use crate::choice::{FirstOption, Scripted};
    use crate::setup::start_game;

    fn attacker() -> PlayerId {
        PlayerId::new("a")
    }
    fn defender() -> PlayerId {
        PlayerId::new("b")
    }

    fn arena() -> (GameState, SystemId) {
        let players = [attacker(), defender()];
        let state = start_game(ContentStore::embedded(), &players, POK, None).unwrap();
        (state, SystemId::new("18"))
    }

    fn put(state: &mut GameState, system: &SystemId, kind: &str, owner: &PlayerId, n: usize) {
        for _ in 0..n {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new(kind), owner.clone()));
        }
    }

    fn kit() -> (Table, Dice, GameRng) {
        (Table::new(), Dice::new(), GameRng::new(7))
    }

    /// A unit type that carries anti-fighter barrage, chosen from the corpus by property.
    fn a_barrage_unit() -> String {
        ti4_content::units::catalogue(ContentStore::embedded(), POK)
            .iter()
            .find(|(_, kind)| kind.has_anti_fighter_barrage() && kind.is_ship())
            .map(|(id, _)| (*id).to_owned())
            .expect("the corpus has a barrage ship")
    }

    /// A unit type that carries space cannon.
    fn a_cannon_unit() -> String {
        ti4_content::units::catalogue(ContentStore::embedded(), POK)
            .iter()
            .find(|(_, kind)| kind.space_cannon_hits_on().is_some())
            .map(|(id, _)| (*id).to_owned())
            .expect("the corpus has a space cannon unit")
    }

    #[test]
    fn the_core_backend_draws_the_barrage_dice_the_string_backend_rolls() {
        // `combat=core`: the barrage's dice come from the typed plan and the core's copy of the stream; the
        // hits, the dice history, the staged dice and the stream positions are the string roll's.
        use crate::dual::{with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let run = |backend: Backend| {
            let mut seen = Vec::new();
            for seed in 0..12_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                let system = SystemId::new("18");
                put(&mut state, &system, &a_barrage_unit(), &attacker(), 3);
                put(&mut state, &system, "fighter", &defender(), 3);
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let hits = with_mask(AreaMask::STRING.with(Area::Combat, backend), || {
                    roll_barrage_side(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &mut dice, &mut rng, &system, &attacker())
                });
                seen.push(format!("{hits} {:?} {:?} {:?}", dice.history(), rng.positions(), state.reroll_staging));
            }
            seen
        };
        let (string, core) = (run(Backend::String), run(Backend::Core));
        assert!(string.iter().any(|s| !s.starts_with("0 ")), "some seed hits");
        assert_eq!(string, core);
    }

    #[test]
    fn a_barrage_kills_only_fighters() {
        // 78.3: hits fall on fighters and nothing else, so a cruiser standing beside them is
        // untouched however well the barrage rolls. Swept across seeds rather than pinned to
        // one, so the test does not depend on a particular roll going well.
        let mut fighters_ever_died = false;
        for seed in 0..40_u64 {
            let (mut state, system) = arena();
            put(&mut state, &system, &a_barrage_unit(), &attacker(), 4);
            put(&mut state, &system, "fighter", &defender(), 3);
            put(&mut state, &system, "cruiser", &defender(), 1);
            let mut dice = Dice::new();
            let mut rng = GameRng::new(seed);

            anti_fighter_barrage(
                &mut state,
                ContentStore::embedded(),
                POK,
                &mut dice,
                &mut rng,
                &system,
                &attacker(),
                &defender(),
            )
            .unwrap();

            let left = ships_of(&state, ContentStore::embedded(), POK, &defender(), &system);
            assert!(
                left.iter().any(|unit| unit.type_id.as_str() == "cruiser"),
                "seed {seed}: the cruiser is not a legal target"
            );
            if left
                .iter()
                .filter(|u| u.type_id.as_str() == "fighter")
                .count()
                < 3
            {
                fighters_ever_died = true;
            }
        }
        assert!(
            fighters_ever_died,
            "a barrage that never hits anything is not testing the hit path"
        );
    }

    #[test]
    fn a_fleet_with_no_barrage_rolls_nothing() {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &attacker(), 3);
        put(&mut state, &system, "fighter", &defender(), 3);
        let (_, mut dice, mut rng) = kit();

        let fired = anti_fighter_barrage(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            &defender(),
        )
        .unwrap();

        assert!(fired.is_empty());
        assert_eq!(dice.count(), 0);
        assert_eq!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).len(),
            3,
            "nothing was destroyed"
        );
    }

    #[test]
    fn a_barrage_against_no_fighters_still_rolls_but_the_log_says_it_hit_nothing() {
        // Feedback fb_1792391288_b1b8a1e9: a destroyer's barrage showed "(1 hit)" against a
        // fleet without fighters. The dice are drawn as ever (replays), the line explains.
        let mut roll_counts = Vec::new();
        for with_fighters in [false, true] {
            let mut said_nothing_to_hit = false;
            for seed in 0..40_u64 {
                let (mut state, system) = arena();
                put(&mut state, &system, &a_barrage_unit(), &attacker(), 8);
                put(&mut state, &system, "cruiser", &defender(), 1);
                if with_fighters {
                    put(&mut state, &system, "fighter", &defender(), 1);
                }
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                anti_fighter_barrage(
                    &mut state,
                    ContentStore::embedded(),
                    POK,
                    &mut dice,
                    &mut rng,
                    &system,
                    &attacker(),
                    &defender(),
                )
                .unwrap();
                roll_counts.push(dice.count());
                for roll in dice.history() {
                    let outcome = roll.outcome.as_deref().unwrap_or("");
                    let flagged = outcome.contains("no enemy fighters");
                    if with_fighters {
                        assert!(!flagged, "fighters were there: {outcome}");
                    } else if flagged {
                        said_nothing_to_hit = true;
                    }
                }
            }
            if !with_fighters {
                assert!(said_nothing_to_hit, "some seed hits and says so");
            }
        }
        assert!(
            roll_counts.iter().all(|n| *n == roll_counts[0]),
            "same dice either way"
        );
    }

    #[test]
    fn barrages_are_simultaneous() {
        // Both are rolled before either removes a fighter. Resolving one side first would let
        // it destroy fighters that had already earned their return barrage.
        let (mut state, system) = arena();
        let barrager = a_barrage_unit();
        put(&mut state, &system, &barrager, &attacker(), 6);
        put(&mut state, &system, &barrager, &defender(), 6);
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);
        let (_, mut dice, mut rng) = kit();

        let fired = anti_fighter_barrage(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            &defender(),
        );

        // Whatever the dice said, both sides' rolls were taken before any fighter was removed:
        // each side that scored a hit is recorded, even if it lost its own fighter.
        assert!(
            fired.unwrap().len() <= 2,
            "at most one entry per side, and both were rolled"
        );
    }

    #[test]
    fn excess_barrage_hits_have_no_effect() {
        // 15.2a, on a target with a single fighter and a large barrage.
        let (mut state, system) = arena();
        put(&mut state, &system, &a_barrage_unit(), &attacker(), 8);
        put(&mut state, &system, "fighter", &defender(), 1);
        let (_, mut dice, mut rng) = kit();

        anti_fighter_barrage(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            &defender(),
        )
        .unwrap();

        assert!(ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).len() <= 1);
    }

    #[test]
    fn morale_changes_only_its_owners_current_combat_round() {
        let (mut state, _) = arena();
        let cruiser = Unit::new(UnitTypeId::new("cruiser"), attacker());
        let printed = hits_on(ContentStore::embedded(), POK, &cruiser).unwrap();
        state.combat_round_seq = 7;
        state.player_mut(&attacker()).unwrap().combat_bonus_round = Some(7);

        assert_eq!(
            effective_hits_on(&state, ContentStore::embedded(), POK, &attacker(), &cruiser),
            Some(printed - 1),
            "Morale Boost is +1 to this player's combat roll"
        );
        assert_eq!(
            effective_hits_on(&state, ContentStore::embedded(), POK, &defender(), &cruiser),
            Some(printed),
            "the opponent's threshold is untouched"
        );

        state.combat_round_seq = 8;
        assert_eq!(
            effective_hits_on(&state, ContentStore::embedded(), POK, &attacker(), &cruiser),
            Some(printed),
            "the marker lapses without a cleanup path"
        );
    }

    #[test]
    fn the_selected_unit_rolls_exactly_one_extra_die_in_its_round() {
        let (mut plain, system) = arena();
        let mut marked = plain.clone();
        put(&mut plain, &system, "cruiser", &attacker(), 2);
        put(&mut marked, &system, "cruiser", &attacker(), 2);
        marked.combat_round_seq = 4;
        let seat = marked.player_mut(&attacker()).unwrap();
        seat.extra_die_round = Some(4);
        seat.extra_die_unit = Some(UnitTypeId::new("cruiser"));

        let (_, mut plain_dice, mut plain_rng) = kit();
        let (_, mut marked_dice, mut marked_rng) = kit();
        roll_fleet(
            &plain,
            ContentStore::embedded(),
            POK,
            &mut plain_dice,
            &mut plain_rng,
            &attacker(),
            &system,
        );
        roll_fleet(
            &marked,
            ContentStore::embedded(),
            POK,
            &mut marked_dice,
            &mut marked_rng,
            &attacker(),
            &system,
        );

        assert_eq!(
            plain_dice.history()[0].faces.len() + 1,
            marked_dice.history()[0].faces.len()
        );
    }

    #[test]
    fn duranium_armor_repairs_a_ship_damaged_in_an_earlier_round() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &attacker(), 1);
        let damaged = state.system_state(&system).units[0].clone();
        state
            .system_mut(&system)
            .replace_unit(&damaged, damaged.sustained());
        state
            .player_mut(&attacker())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("da"));

        // Damaged before this round: repaired.
        duranium_armor(
            &mut state,
            &system,
            &attacker(),
            &["dreadnought".to_owned()],
        );
        assert!(!state.system_state(&system).units[0].sustained_damage);

        // Damaged this round (nothing was damaged before it): left as it is.
        let fresh = state.system_state(&system).units[0].clone();
        state
            .system_mut(&system)
            .replace_unit(&fresh, fresh.sustained());
        duranium_armor(&mut state, &system, &attacker(), &[]);
        assert!(state.system_state(&system).units[0].sustained_damage);
    }

    #[test]
    fn non_fighter_priority_never_reintroduces_an_ability_immune_maximum() {
        let (mut state, system) = arena();
        state.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("naaz");
        put(&mut state, &system, "naaz_voltron", &defender(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);
        let content = ContentStore::embedded();
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        absorb_hits_seeing_with(
            &mut state,
            content,
            ti4_model::content_types::DEFAULT,
            None,
            &mut ctx,
            &defender(),
            &system,
            &attacker(),
            1,
            true,
            HitOrigin::UnitAbility,
        )
        .unwrap();
        let board = state.system_state(&system);
        let survivors = board.units_of(&defender());
        assert_eq!(survivors.len(), 1);
        assert_eq!(survivors[0].type_id.as_str(), "naaz_voltron");
        assert!(!survivors[0].sustained_damage);
    }

    #[test]
    fn graviton_binds_cannon_hits_to_non_fighters_once_per_readying() {
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &defender(), 2);
        put(&mut state, &system, "destroyer", &defender(), 1);
        state
            .player_mut(&attacker())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("gls"));
        let content = ContentStore::embedded();
        assert!(use_graviton(
            &mut state,
            content,
            POK,
            &attacker(),
            &defender(),
            &system,
            1
        ));
        assert!(
            !use_graviton(
                &mut state,
                content,
                POK,
                &attacker(),
                &defender(),
                &system,
                1
            ),
            "exhausted after one use"
        );

        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        absorb_hits_seeing_with(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &defender(),
            &system,
            &attacker(),
            1,
            true,
            HitOrigin::UnitAbility,
        )
        .unwrap();
        assert_eq!(
            non_fighter_ships_of(&state, content, POK, &defender(), &system),
            0,
            "the destroyer took the hit, not a fighter"
        );
        assert_eq!(
            ships_of(&state, content, POK, &defender(), &system).len(),
            2
        );
    }

    #[test]
    fn graviton_bound_sustain_question_says_the_hit_goes_to_a_non_fighter() {
        // Nightly runs 07-2221 and 37-0225: the sustain question did not say the hits were
        // bound, so the client planned "take the hit, lose a fighter" and the follow-up
        // casualty question (non-fighters only) rejected it.
        for bound in [true, false] {
            let (mut state, system) = arena();
            put(&mut state, &system, "dreadnought", &defender(), 1);
            put(&mut state, &system, "destroyer", &defender(), 1);
            put(&mut state, &system, "fighter", &defender(), 1);
            let content = ContentStore::embedded();
            let mut dice = Dice::new();
            let mut rng = GameRng::new(1);
            let (decider, seen) = crate::choice::Capturing::new(Box::new(FirstOption));
            let mut table = Table::with_default(Box::new(decider));
            let mut ctx = Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: None,
            };
            absorb_hits_seeing_with(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &defender(),
                &system,
                &attacker(),
                1,
                bound,
                HitOrigin::CombatRoll,
            )
            .unwrap();
            let asked = seen.borrow();
            let take = asked[0]
                .options
                .iter()
                .find(|option| option.is_decline())
                .expect("the sustain question offers taking the hit");
            assert_eq!(
                take.payload
                    .get("non_fighters_first")
                    .and_then(serde_json::Value::as_bool),
                bound.then_some(true),
                "bound = {bound}"
            );
        }
    }

    #[test]
    fn assault_cannon_costs_the_opponent_a_non_fighter_ship_before_any_die() {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &attacker(), 3);
        put(&mut state, &system, "destroyer", &defender(), 1);
        state
            .player_mut(&attacker())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("asc"));
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(3);
        resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();
        assert!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).is_empty(),
            "the destroyer went to the cannon"
        );
        assert_eq!(
            ships_of(&state, ContentStore::embedded(), POK, &attacker(), &system).len(),
            3
        );
        assert!(
            dice.rolled("space combat").is_empty(),
            "the fight ended before any die"
        );
    }

    #[test]
    fn assault_cannon_runs_as_a_core_window_in_lockstep() {
        // The cannon with `driver=dual`: the string step and the core `AssaultCannon` window put the same
        // question, take the same answer and leave the same state (one victim ship: no question; several:
        // the victim chooses).
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        for victims in [1usize, 2] {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "letnev")], ti4_model::content_types::DEFAULT);
            // The fixture seats leave placeholder-faction notes the card slice cannot map.
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "destroyer", &defender(), 1);
            if victims == 2 {
                put(&mut state, &system, "dreadnought", &defender(), 1);
                put(&mut state, &system, "fighter", &defender(), 2);
            }
            state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("asc"));
            let mask = AreaMask::STRING.with(Area::Driver, Backend::Dual);
            with_mask(mask, || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(3);
                resolve(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &mut table, &mut dice, &mut rng, &system).unwrap();
            });
        }
        let r = report();
        let counts = &r.areas[&Area::Driver];
        assert!(counts.calls >= 2, "both openings were compared: {counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn a_round_opening_runs_as_a_core_window_in_lockstep() {
        // Several rounds of a plain fight with `driver=dual`: the string `begin_round` and the core
        // `BeginRound` window leave the same counters, presentation and events.
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        let system = SystemId::new("18");
        put(&mut state, &system, "cruiser", &attacker(), 3);
        put(&mut state, &system, "cruiser", &defender(), 3);
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(5));
            resolve(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &mut table, &mut dice, &mut rng, &system).unwrap();
        });
        let r = report();
        let counts = &r.areas[&Area::Driver];
        assert!(counts.calls >= 3, "the round openings and the idle barrage were compared: {counts:?}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Viscount Unlenn (the Letnev agent) is an ability of the core table: a Letnev seat outside the fight that holds
    /// the readied agent is offered every ship of the system at the start of each round, the choice is marked and
    /// the card exhausted, and the core `BeginRound` window announces the round with the `round_seq` the agent reads.
    #[test]
    fn the_letnev_agent_chooses_a_ship_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut marked = 0;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            for seed in 1..=3_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "letnev")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("c")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("letnevagent"), ti4_model::state::LeaderStatus::Readied);
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &attacker(), 3);
                put(&mut state, &system, "cruiser", &defender(), 3);
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                marked += state.faction_marks.keys().filter(|k| k.starts_with("yssaril:ssruu-round-agent:c:letnevagent:")).count();
            }
        });
        let r = report();
        let w = r.windows.get("combat::begin_round").copied().unwrap_or_default();
        assert!(marked > 0, "the agent marked a ship");
        assert!(w.compared >= 1, "the round opening with the agent is compared: {:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:letnevagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Tellurian (the Titans agent) is an ability of the core table: in the `HITS_TO_ASSIGN` window of a round the
    /// readied agent is offered, the core resolver exhausts it and grants the cancellation the next hit spends, and
    /// the window is compared rather than bailed.
    #[test]
    fn tellurian_cancels_a_hit_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut used = 0;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            for seed in 1..=4_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "titans"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &attacker(), 3);
                put(&mut state, &system, "cruiser", &defender(), 3);
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                used += usize::from(state.player(&attacker()).unwrap().leaders.get(ti4_model::id::text(&"titansagent")) == Some(&ti4_model::state::LeaderStatus::Exhausted));
            }
        });
        let r = report();
        assert!(used > 0, "the agent was used");
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:titansagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Brother Milor, ship branch: after a ship is destroyed in a space combat the readied agent lets its owner place
    /// 2 fighters in the system (the destroyed ship's owner's form).
    #[test]
    fn brother_milor_places_fighters_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut exhausted = 0;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            for (seed, who) in (1..=4_u64).zip(["hacan", "naalu", "sol", "letnev"]) {
                let mut state = crate::fixtures::seated_game(&[("a", who), ("b", "yin")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&defender()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("yinagent"), ti4_model::state::LeaderStatus::Readied);
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &attacker(), 3);
                put(&mut state, &system, "cruiser", &defender(), 3);
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                exhausted += usize::from(state.player(&defender()).unwrap().leaders.get(ti4_model::id::text(&"yinagent")) == Some(&ti4_model::state::LeaderStatus::Exhausted));
            }
        });
        let r = report();
        assert!(exhausted > 0, "the agent was used");
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:yinagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// Salvage Operations (Mentak), the trade good: after a space combat it took part in and did not draw, the holder
    /// gains 1 trade good (staged, announced by the end window's adapter).
    #[test]
    fn salvage_operations_pays_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut paid = 0;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            for seed in 1..=4_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "mentak"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                crate::technology::grant(&mut state, &attacker(), &ti4_model::id::TechnologyId::new("so"));
                // Nothing to pay the produce half with: it is another ability and stays unoffered.
                state.player_mut(&attacker()).unwrap().trade_goods = 0;
                let held: Vec<_> = state.controlled_planets(&attacker()).into_iter().map(|(_, planet)| planet.clone()).collect();
                state.exhausted_planets.extend(held);
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &attacker(), 4);
                put(&mut state, &system, "cruiser", &defender(), 1);
                let before = state.player(&attacker()).unwrap().trade_goods;
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                paid += state.player(&attacker()).unwrap().trade_goods - before;
            }
        });
        let r = report();
        let ended = r.windows.get("combat::announce_end").copied().unwrap_or_default();
        assert!(paid > 0, "the holder was paid");
        assert!(ended.compared >= 1, "the end announcement with the ability is compared: {:?}", r.windows);
        assert!(r.windows.keys().all(|k| !k.contains("unported ability technology:*:so")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// A reaction card is played from the standing slot: the slot asks which card when they differ, the card is spent
    /// into the declared plays, and at the close of the window the core announces `ACTION_CARD_PLAYED`, plays the stamp
    /// of its effect and discards it (`ACTION_CARD_DISCARDED`), as the string `reactions::announce` does.
    #[test]
    fn declared_reaction_cards_are_played_in_the_core_window_in_lockstep() {
        let mut played = 0;
        let mut asked = 0;
        for (seed, hand) in (1..=6_u64).zip([&["mb1"][..], &["mb1", "emergency"], &["sh1", "mb2"], &["mb1", "mb2"], &["emergency", "mb3", "sh3"], &["sh1", "sh2"]]) {
            let (p, a, bails) = declared_cards_fight(seed, hand, true);
            assert!(bails.iter().all(|k| !k.contains("(card ") && !k.contains("(a card played at once)")), "{hand:?}: {bails:?}");
            played += p;
            asked += a;
        }
        assert!(played > 0 && asked > 0, "cards were played ({played}) and a choice of card was asked ({asked})");
    }

    /// Sabotage is played at once, in the window of `ACTION_CARD_PLAYED`: the core announces the Sabotage inside the
    /// slot's effect and cancels the event that opened the window (the declared Morale Boost is spent, its effect is not).
    #[test]
    fn sabotage_cancels_a_declared_play_in_the_core_window_in_lockstep() {
        let mut sabotaged = 0;
        for seed in 1..=6_u64 {
            let (_, _, bails, plays) = cards_fight(seed, ["sol", "hacan"], &["sabo1"], &["mb1"], true);
            assert!(bails.iter().all(|k| !k.contains("(card ") && !k.contains("(a card played at once)")), "{bails:?}");
            sabotaged += usize::from(plays.iter().any(|c| c == "sabo1"));
        }
        assert!(sabotaged > 0, "a Sabotage was played in the window of a declared card");
    }

    /// Skilled Retreat: the holder withdraws the whole fleet to a free neighbour at the start of a round; the core asks which
    /// system when there are several (the destinations are the map's, noted by the string card), moves the fleet through the
    /// retreat window and ends the combat in a draw.
    #[test]
    fn skilled_retreat_withdraws_the_fleet_in_the_core_window_in_lockstep() {
        let mut withdrew = 0;
        for factions in [["sol", "hacan"], ["sol", "mentak"]] {
            for seed in 1..=4_u64 {
                let (_, asked, bails, plays) = cards_fight_at(seed, factions, &[], &["s_retreat1"], true, true);
                assert!(bails.iter().all(|k| !k.contains("(card s_retreat")), "{bails:?}");
                withdrew += usize::from(plays.iter().any(|c| c == "s_retreat1"));
                let _ = asked;
            }
        }
        assert!(withdrew > 0, "a Skilled Retreat was played");
    }

    /// W4: Reveal Prototype is played from the standing slot at the start of the combat: its four resources are judged (by the adapter, the
    /// string's own judge), the unit upgrade of a line of the holder's units in the system is researched and the bill paid, all in the core.
    #[test]
    fn reveal_prototype_is_played_from_the_slot_in_the_core_window_in_lockstep() {
        let mut played = 0;
        for seed in 1..=4_u64 {
            let (_, _, bails, plays) = cards_fight_with(seed, ["hacan", "sol"], &[], &["reveal_prototype"], false, false, &|state, system| {
                put(state, system, "fighter", &defender(), 2);
            });
            assert!(bails.iter().all(|k| !k.contains("nested") && !k.contains("(card ") && !k.contains("cost the core does not judge")), "the card is followed: {bails:?}");
            played += usize::from(plays.iter().any(|c| c == "reveal_prototype"));
        }
        assert!(played > 0, "a Reveal Prototype was played");
    }

    /// The promissory notes played at the start of a space combat: Antivirus (placed faceup) and The Cavalry (a ship type
    /// takes the flagship's abilities, the note goes home when the combat ends), both held by the defender.
    #[test]
    fn the_antivirus_and_the_cavalry_are_played_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let (mut faceup, mut home) = (0, 0);
        for (note, third) in [("antivirus:nekro", "nekro"), ("cavalry:nomad", "nomad")] {
            let _ = take_report();
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                for seed in 1..=4_u64 {
                    let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol"), ("c", third)], ti4_model::content_types::DEFAULT);
                    state.promissory_notes.clear();
                    state.promissory_faceup.clear();
                    state.rng_seed = seed;
                    crate::promissory::take(&mut state, ContentStore::embedded(), &defender(), note);
                    let system = SystemId::new("18");
                    put(&mut state, &system, "cruiser", &attacker(), 3);
                    put(&mut state, &system, "cruiser", &defender(), 2);
                    if seed % 2 == 0 {
                        put(&mut state, &system, "dreadnought", &defender(), 1);
                    }
                    state.active_system = Some(system.clone());
                    state.active = Some(attacker());
                    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                    let mut resolver = crate::fixtures::armed_resolver(&state);
                    let mut sequence = crate::event::EventSequence::new();
                    let mut ctx = Resolving {
                        content: ContentStore::embedded(),
                        sources: ti4_model::content_types::DEFAULT,
                        dice: &mut dice,
                        rng: &mut rng,
                        table: &mut table,
                        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                    };
                    resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                    faceup += usize::from(state.promissory_faceup.contains(note));
                    home += usize::from(state.promissory_notes.get(note) == Some(&PlayerId::new("c")));
                }
            });
            let r = report();
            assert!(r.windows.keys().all(|k| !k.contains("unported ability promissory:*:")), "{note}: {:?}", r.windows);
            if note.starts_with("cavalry") {
                assert!(r.windows.get("combat::announce_end").is_some_and(|w| w.compared > 0), "the end of the combat, where the note goes home, is compared: {:?}", r.windows);
            }
            assert_eq!(r.total_differences(), 0, "{note}: {:?}", r.differences);
        }
        assert!(faceup > 0, "Antivirus was placed faceup");
        assert!(home > 0, "The Cavalry went home at the end of the combat");
        let _ = take_report();
    }

    /// A fight that is over when the roll of a round is next (every ship of a side gone since the stage was entered) ends in the
    /// `Rolling` stage, which the string leaves without another turn of its loop: the presentation keeps that stage's phase
    /// (`resolving_hits`); the core machine used to take one more turn and say `complete`. Found by the soak.
    #[test]
    fn a_fight_over_at_the_roll_keeps_the_phase_of_its_stage_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || crate::dual_window::with_composed(true, || {
            let content = ContentStore::embedded();
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = 1;
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 2);
            put(&mut state, &system, "cruiser", &defender(), 1);
            state.active_system = Some(system.clone());
            state.active = Some(attacker());
            let mut window = CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system);
            // The defender's ships are gone when the round is about to roll.
            state.system_mut(&system).units.retain(|u| u.owner != defender());
            window.stage = Stage::Rolling { round: 1 };
            let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(1));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content,
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
            };
            window.settle_open(&mut state, &mut ctx).unwrap();
            assert_ne!(state.combat_presentation.phase, "complete", "the string leaves the stage's phase");
        }));
        let r = report();
        assert!(r.windows.get("combat::settle").is_some_and(|w| w.compared > 0), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// The ship The Cavalry lends the Nomad flagship's ANTI-FIGHTER BARRAGE (Memoria II has one) rolls it in the round-1 barrage that
    /// follows the play; the note is played inside the window, so the lent stats are read from the core state, not from the position
    /// the window started in; found by the soak (a Hacan carrier, game 10 of the Thunder's Edge slice).
    #[test]
    fn the_cavalry_ship_rolls_the_lent_barrage_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut barrages = 0;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            for seed in 1..=6_u64 {
                let mut state = crate::fixtures::seated_game(&[("a", "hacan"), ("b", "sol"), ("c", "nomad")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                state.player_mut(&PlayerId::new("c")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("m2"));
                crate::promissory::take(&mut state, ContentStore::embedded(), &defender(), "cavalry:nomad");
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &attacker(), 3);
                put(&mut state, &system, "carrier", &defender(), 1);
                put(&mut state, &system, "fighter", &attacker(), 2);
                state.active_system = Some(system.clone());
                state.active = Some(attacker());
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                barrages += dice.history().iter().filter(|r| r.reason.starts_with("anti")).count();
            }
        });
        let r = report();
        assert!(barrages > 0, "the lent barrage was rolled");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Ipswitch, Loose Cannon (the Mentak hero): purged at the start of the combat, it places a ship of the type of every
    /// other player's ship destroyed in it, and the record of the combat is cleared at its end.
    #[test]
    fn the_mentak_hero_places_ships_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        for composed in [false, true] {
            let _ = take_report();
            let (mut purged, mut gained) = (0, 0_i64);
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || crate::dual_window::with_composed(composed, || {
                for seed in 1..=6_u64 {
                    let mut state = crate::fixtures::seated_game(&[("a", "mentak"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
                    state.promissory_notes.clear();
                    state.promissory_faceup.clear();
                    state.rng_seed = seed;
                    state.player_mut(&attacker()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("mentakhero"), ti4_model::state::LeaderStatus::Unlocked);
                    let system = SystemId::new("18");
                    put(&mut state, &system, "cruiser", &attacker(), 3);
                    put(&mut state, &system, "cruiser", &defender(), 2);
                    put(&mut state, &system, "destroyer", &defender(), 2);
                    if seed % 2 == 0 {
                        // The hero's owner loses ships too: those place nothing.
                        put(&mut state, &system, "destroyer", &attacker(), 4);
                        put(&mut state, &system, "dreadnought", &defender(), 2);
                    }
                    state.active_system = Some(system.clone());
                    state.active = Some(attacker());
                    let before = state.system_state(&system).units.iter().filter(|u| u.owner == attacker()).count() as i64;
                    let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                    let mut resolver = crate::fixtures::armed_resolver(&state);
                    let mut sequence = crate::event::EventSequence::new();
                    let mut ctx = Resolving {
                        content: ContentStore::embedded(),
                        sources: ti4_model::content_types::DEFAULT,
                        dice: &mut dice,
                        rng: &mut rng,
                        table: &mut table,
                        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                    };
                    resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                    purged += usize::from(state.player(&attacker()).unwrap().leaders.get(ti4_model::id::text(&"mentakhero")) == Some(&ti4_model::state::LeaderStatus::Purged));
                    assert!(!state.faction_marks.contains_key("mentak:hero:18"), "the record of the combat is cleared");
                    if seed % 2 == 1 {
                        gained += state.system_state(&system).units.iter().filter(|u| u.owner == attacker()).count() as i64 - before;
                    }
                }
            }));
            let r = report();
            assert!(purged > 0, "composed {composed}: the hero was purged");
            assert!(gained > 0, "composed {composed}: ships were placed ({gained})");
            assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:mentakhero")), "composed {composed}: {:?}", r.windows);
            assert!(r.windows.keys().all(|k| !k.contains("a fact the adapter did not compute")), "composed {composed}: a destroyed cruiser's form is the core's (package W3): {:?}", r.windows);
            assert_eq!(r.total_differences(), 0, "composed {composed}: {:?}", r.differences);
        }
        let _ = take_report();
    }

    /// One space fight of a Nekro (`a`) against a Hacan (`b`) who owns `theirs`; the Nekro owns the Valefar Assimilators.
    /// The question of Technological Singularity is answered with the option `seed` picks (the other questions: the first).
    /// `(the state after, the keys of the report's windows, the differences)`.
    fn nekro_fight(seed: u64, theirs: &[&str], prepare: impl Fn(&mut GameState)) -> (GameState, Vec<String>, u64) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        struct Rotate(usize);
        impl crate::choice::Decider for Rotate {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                if choice.prompt == "gain a technology, or assimilate instead" {
                    let pick = choice.options[self.0 % choice.options.len()].clone();
                    return Ok(pick);
                }
                crate::choice::FirstOption.choose(choice)
            }
        }
        let mut out = None;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || crate::dual_window::with_composed(true, || {
            let mut state = crate::fixtures::seated_game(&[("a", "nekro"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            for tech in ["vax", "vay"] {
                state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(tech));
            }
            state.player_mut(&defender()).unwrap().technologies.clear();
            for tech in theirs {
                state.player_mut(&defender()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(*tech));
            }
            prepare(&mut state);
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "destroyer", &defender(), 3);
            put(&mut state, &system, "cruiser", &defender(), 1);
            state.active_system = Some(system.clone());
            state.active = Some(attacker());
            let mut table = Table::with_default(Box::new(Rotate(seed as usize)));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
            };
            resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
            out = Some(state);
        }));
        let r = report();
        let bails = r.windows.keys().cloned().collect();
        let differences = r.total_differences();
        // The end announcement of a fight whose combat mark the table closes is no reason to keep the settle in the string.
        if r.windows.keys().all(|k| !k.contains("[bail:")) {
            assert_eq!(r.windows.get("combat::settle").map_or(0, |w| w.bailed), 0, "{:?}", r.windows);
        }
        if differences > 0 {
            for d in r.differences.iter().take(8) {
                eprintln!("{d:?}");
            }
        }
        let _ = take_report();
        (out.unwrap(), bails, differences)
    }

    /// Technological Singularity (the Nekro): the marks open and close with the combat, the ability is offered after a
    /// destroyed opponent ship and asks which technology to gain or which token to place; the core follows all of it.
    #[test]
    fn the_nekro_singularity_runs_in_the_core_window_in_lockstep() {
        let (mut gained, mut tokens) = (0, 0);
        for seed in 0..12_u64 {
            let (state, bails, differences) = nekro_fight(seed, &["gd", "ie", "qdn", "pm"], |_| {});
            assert_eq!(differences, 0, "seed {seed}");
            assert!(bails.iter().all(|k| !k.contains("technological_singularity")), "seed {seed}: {bails:?}");
            let nekro = state.player(&attacker()).unwrap();
            gained += usize::from(nekro.technologies.contains(ti4_model::id::text(&"gd")) || nekro.technologies.contains(ti4_model::id::text(&"ie")));
            tokens += nekro.assimilated_technologies.len();
            assert!(state.faction_marks.keys().all(|k| !k.starts_with("nekro:ts:")), "the marks are closed with the combat");
        }
        assert!(gained > 0, "a technology was gained");
        assert!(tokens > 0, "a token was placed");
    }

    /// Technological Singularity is barred by a faceup Antivirus in the opponent's area, and the X and Y tokens are not offered
    /// on the technologies of a Last Bastion player whose commander (Nip and Tuck) is unlocked: the options the core derives
    /// equal the string's (a difference in either would differ in the options of the question).
    #[test]
    fn the_singularity_respects_antivirus_and_nip_and_tuck_in_lockstep() {
        let mut barred_runs = 0;
        for seed in 0..8_u64 {
            let (state, bails, differences) = nekro_fight(seed, &["gd", "qdn"], |s| {
                let note = crate::promissory::note_id("antivirus", &crate::promissory::faction_name(s, &attacker()));
                s.promissory_notes.insert(note.clone(), defender());
                s.promissory_faceup.insert(note);
            });
            assert_eq!(differences, 0, "antivirus, seed {seed}");
            assert!(bails.iter().all(|k| !k.contains("technological_singularity")), "{bails:?}");
            let nekro = state.player(&attacker()).unwrap();
            barred_runs += usize::from(!nekro.technologies.contains(ti4_model::id::text(&"gd")) && nekro.assimilated_technologies.is_empty());
            let (state, _, differences) = nekro_fight(seed, &["gd", "qdn"], |s| {
                s.player_mut(&defender()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("bastioncommander"), ti4_model::state::LeaderStatus::Unlocked);
            });
            assert_eq!(differences, 0, "nip and tuck, seed {seed}");
            assert!(state.player(&attacker()).unwrap().assimilated_technologies.is_empty(), "no token goes on the technologies of that player");
        }
        assert_eq!(barred_runs, 8, "the faceup Antivirus bars the ability");
    }

    /// The tokens are placed once on a technology (one token per technology, one per card), and a token lends only a faction
    /// technology that is not a unit upgrade nor Executive Order: all of it equal to the string's options.
    #[test]
    fn the_singularity_options_equal_the_strings_for_every_token_state() {
        for seed in 0..10_u64 {
            for theirs in [&["qdn", "pm", "gd"][..], &["executiveorder", "qdn"][..], &["hcf2", "gd", "ie"][..]] {
                let (_, _, differences) = nekro_fight(seed, theirs, |s| {
                    if seed % 3 == 0 {
                        s.player_mut(&attacker()).unwrap().assimilated_technologies.insert("vax".to_owned(), ti4_model::id::TechnologyId::new("qdn"));
                    }
                    if seed % 5 == 0 {
                        s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gd"));
                    }
                });
                assert_eq!(differences, 0, "seed {seed} {theirs:?}");
            }
        }
    }

    /// Gaining a unit upgrade through Technological Singularity reaches the units on the board (`technology::gain`): the Nekro's
    /// cruisers become Cruiser II in the core window as in the string.
    #[test]
    fn the_singularity_gaining_a_unit_upgrade_swaps_the_units_in_lockstep() {
        let (mut swapped, mut gained) = (0, 0);
        for seed in 0..6_u64 {
            let (state, windows, differences) = nekro_fight(seed, &["cr2"], |_| {});
            assert_eq!(differences, 0, "seed {seed}");
            assert!(windows.iter().all(|k| !k.contains("[bail:")), "seed {seed}: {windows:?}");
            gained += usize::from(state.player(&attacker()).unwrap().technologies.contains(ti4_model::id::text(&"cr2")));
            swapped += state.system_state(&SystemId::new("18")).units.iter().filter(|u| u.owner == attacker() && u.type_id.as_str() == "cruiser2").count();
        }
        assert!(gained > 0 && swapped > 0, "the upgrade was gained ({gained}) and reached the board ({swapped})");
    }

    /// Documented bail of Technological Singularity: a Nekro who holds the Z token breakthrough (a flagship's text is lent).
    #[test]
    fn bail_the_singularity_with_the_z_token() {
        let mut bailed = 0;
        for seed in 0..6_u64 {
            let (_, bails, differences) = nekro_fight(seed, &["gd", "qdn"], |s| {
                s.player_mut(&attacker()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("nekrobt"));
            });
            assert_eq!(differences, 0, "seed {seed}");
            bailed += usize::from(bails.iter().any(|k| k.contains("a fact the adapter did not compute")));
        }
        assert!(bailed > 0, "the Z token is the string's");
    }

    /// The ground forces an Alastor brought into the fight stop taking part when it ends: the marks that name them are cleared by the
    /// announcement of the end, in the core window as in the string; found by the soak.
    #[test]
    fn the_end_of_a_fight_clears_the_alastor_participants_in_lockstep() {
        let mut cleared = 0;
        for seed in 0..8_u64 {
            let (state, _bails, differences) = nekro_fight(seed, &["gd"], |s| {
                let system = SystemId::new("18");
                put(s, &system, "nekro_flagship", &attacker(), 1);
                crate::fixtures::put_on_planet(s, &system, &ti4_model::id::PlanetId::new("mecatol_rex"), "nekro_mech", &attacker(), 2);
            });
            assert_eq!(differences, 0, "seed {seed}");
            cleared += usize::from(state.faction_marks.keys().all(|k| !k.starts_with("nekro:alastor")));
        }
        assert!(cleared > 0, "the participants were cleared");
    }

    /// The Z token is an option of the Singularity even when the opponent owns nothing the Nekro could gain: the core cannot tell
    /// whether the string offers it, so the window bails there too (counted); found by the soak.
    #[test]
    fn bail_the_singularity_with_the_z_token_and_nothing_else_to_take() {
        let mut bailed = 0;
        for seed in 0..6_u64 {
            let (_, bails, differences) = nekro_fight(seed, &[], |s| {
                s.player_mut(&attacker()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("nekrobt"));
            });
            assert_eq!(differences, 0, "seed {seed}");
            bailed += usize::from(bails.iter().any(|k| k.contains("a fact the adapter did not compute")));
        }
        assert!(bailed > 0, "the Z token is the string's");
    }

    /// The string registers The Terror Between for a Nekro seat (its flagship carries the text through the Z token): the core
    /// registers it too (the registry test) and bails when a Nekro flagship holding the Z token stands in the system.
    #[test]
    fn bail_the_terror_between_of_a_nekro_flagship_with_the_z_token() {
        let (mut bailed, mut plain_bailed) = (0, 0);
        for seed in 0..6_u64 {
            let (_, bails, differences) = nekro_fight(seed, &[], |s| {
                put(s, &SystemId::new("18"), "nekro_flagship", &attacker(), 1);
                s.player_mut(&attacker()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("nekrobt"));
            });
            assert_eq!(differences, 0, "seed {seed}");
            bailed += usize::from(bails.iter().any(|k| k.contains("a fact the adapter did not compute")));
            // Without the Z token the flagship carries nothing: the window is followed.
            let (_, bails, differences) = nekro_fight(seed, &[], |s| put(s, &SystemId::new("18"), "nekro_flagship", &attacker(), 1));
            assert_eq!(differences, 0, "seed {seed}");
            plain_bailed += usize::from(bails.iter().any(|k| k.contains("a fact the adapter did not compute")));
        }
        assert!(bailed > 0, "the Z token may lend the text: the string decides");
        assert_eq!(plain_bailed, 0, "no Z token, no bail");
    }

    /// One space fight of `a` against `b` (real factions, the fixture's notes cleared) at the centre of a plain hub whose map is
    /// lent to the combat, after `prepare`; every question is answered by `pick(seed, nth question of its prompt kind, choice)`.
    /// `(the state after, the windows of the report, the number of differences)`.
    fn hub_fight(
        seed: u64,
        factions: &[&str],
        prepare: impl Fn(&mut GameState, &SystemId),
        pick: fn(u64, usize, &Choice) -> usize,
    ) -> (GameState, std::collections::BTreeMap<String, crate::dual::WindowCounts>, u64, Vec<String>) {
        hub_fight_composed(true, seed, factions, prepare, pick)
    }

    /// [`hub_fight`] with the windows of the fight compared one by one (`composed` false) or as the composed machine.
    fn hub_fight_composed(
        composed: bool,
        seed: u64,
        factions: &[&str],
        prepare: impl Fn(&mut GameState, &SystemId),
        pick: fn(u64, usize, &Choice) -> usize,
    ) -> (GameState, std::collections::BTreeMap<String, crate::dual::WindowCounts>, u64, Vec<String>) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        struct Rotate(u64, usize, fn(u64, usize, &Choice) -> usize);
        impl crate::choice::Decider for Rotate {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                self.1 += 1;
                let i = (self.2)(self.0, self.1, choice).min(choice.options.len().saturating_sub(1));
                choice.options.get(i).cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
            }
        }
        let hub = crate::fixtures::plain_hub();
        let mut out = None;
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || crate::dual_window::with_composed(composed, || {
            let names = ["a", "b", "c", "d"];
            let seats: Vec<(&str, &str)> = factions.iter().enumerate().map(|(i, f)| (names[i], *f)).collect();
            let mut state = crate::fixtures::seated_game(&seats, ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new(hub.centre.clone());
            prepare(&mut state, &system);
            state.active_system = Some(system.clone());
            state.active = Some(attacker());
            // As `Game` runs a fight: the position names the battle, so the questions asked inside it carry it (`about_battle`).
            state.active_space_combat = Some((system.clone(), attacker(), defender()));
            let mut table = Table::with_default(Box::new(Rotate(seed, 0, pick)));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: Some(&hub.galaxy) }),
            };
            resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
            out = Some(state);
        }));
        let r = report();
        let differences = r.total_differences();
        if differences > 0 {
            for d in r.differences.iter().take(8) {
                eprintln!("{d:?}");
            }
        }
        let windows = r.windows.clone();
        let nested = r.nested.keys().cloned().collect();
        let _ = take_report();
        (out.unwrap(), windows, differences, nested)
    }

    /// Dynamo (the Empyrean flagship): after a defender's dreadnought sustains in the fight the Empyrean may spend 2 influence
    /// to repair it; the planets are the payment's own questions, answered in turn.
    #[test]
    fn dynamo_repairs_in_the_core_window_in_lockstep() {
        let (mut offered, mut paid_questions) = (0, 0);
        for seed in 0..10_u64 {
            let (_state, windows, differences, nested) = hub_fight(
                seed,
                &["empyrean", "hacan"],
                |s, system| {
                    // Half of the fights have the flagship in an adjacent system: the map is an input of the ability.
                    let home = if s.rng_seed % 2 == 1 { SystemId::new(crate::fixtures::plain_hub().outer[0].clone()) } else { system.clone() };
                    put(s, &home, "empyrean_flagship", &attacker(), 1);
                    put(s, system, "cruiser", &attacker(), 2);
                    put(s, system, "dreadnought", &defender(), 3);
                    put(s, system, "cruiser", &defender(), 1);
                    // Trade goods beside the planets make the payment a real choice.
                    s.player_mut(&attacker()).unwrap().trade_goods = 6;
                },
                |seed, nth, c| if c.prompt.starts_with("pay ") { (seed as usize + nth) % 3 } else { 0 },
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("[bail:")), "seed {seed}: {windows:?}");
            offered += nested.iter().filter(|k| k.contains("unit:*:empyrean_flagship:SUSTAIN_DAMAGE_USED:after")).count();
            paid_questions += nested.iter().filter(|k| k.contains("ask:pay ")).count();
        }
        assert!(offered > 0, "Dynamo was offered and followed");
        assert!(paid_questions > 0, "a payment question was put inside the ability");
    }

    /// The Watcher (the Empyrean mech): the opponent plays a reaction card in the fight; the Empyrean removes a mech in reach to
    /// cancel it (a question when several mechs are in reach).
    #[test]
    fn the_watcher_cancels_a_card_in_the_core_window_in_lockstep() {
        let (mut offered, mut asked) = (0, 0);
        for seed in 0..8_u64 {
            let (state, windows, differences, nested) = hub_fight(
                seed,
                &["empyrean", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 2);
                    put(s, system, "empyrean_mech", &attacker(), 2);
                    put(s, system, "dreadnought", &defender(), 2);
                    let outer = SystemId::new(crate::fixtures::plain_hub().outer[0].clone());
                    put(s, &outer, "empyrean_mech", &attacker(), 1);
                    s.player_mut(&defender()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("mb1")];
                },
                |seed, _, c| {
                    if let Some(i) = c.options.iter().position(|o| o.id.contains("empyrean_mech")) {
                        return i;
                    }
                    if c.prompt.starts_with("Watcher") {
                        return seed as usize;
                    }
                    c.options.iter().position(|o| o.id.starts_with("reaction:")).unwrap_or(0)
                },
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("[bail:")), "seed {seed}: {windows:?}");
            offered += nested.iter().filter(|k| k.contains("unit:*:empyrean_mech:ACTION_CARD_PLAYED:when")).count();
            asked += nested.iter().filter(|k| k.contains("ask:Watcher")).count();
            let _ = state;
        }
        assert!(offered > 0, "the Watcher was offered and followed");
        assert!(asked > 0, "several mechs were in reach: the Empyrean was asked which");
    }

    /// Salvage Operations, the produce half (Mentak): after a won fight the owner may pay the cost of a ship type destroyed in
    /// it and place one; the payment is the pay loop inside the ability. A cruiser or a flagship is the string's (Corsair form).
    #[test]
    fn salvage_operations_produces_a_ship_in_the_core_window_in_lockstep() {
        let (mut offered, mut paid_questions, mut built) = (0, 0, 0);
        for seed in 0..12_u64 {
            let (state, windows, differences, nested) = hub_fight(
                seed,
                &["mentak", "hacan"],
                |s, system| {
                    s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("so"));
                    put(s, system, "dreadnought", &attacker(), 3);
                    put(s, system, "carrier", &attacker(), 1);
                    put(s, system, "destroyer", &defender(), 2);
                    put(s, system, "fighter", &defender(), 2);
                    s.player_mut(&attacker()).unwrap().trade_goods = 5;
                },
                |seed, nth, c| if c.prompt.starts_with("Salvage") { 0 } else if c.prompt.starts_with("pay ") { (seed as usize + nth) % 3 } else { 0 },
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            offered += nested.iter().filter(|k| k.contains("technology:*:so:SPACE_COMBAT_ENDED:produce")).count();
            paid_questions += nested.iter().filter(|k| k.contains("ask:pay ")).count();
            built += usize::from(state.system_state(&SystemId::new(crate::fixtures::plain_hub().centre)).units.iter().filter(|u| u.owner == attacker()).count() > 4);
        }
        assert!(offered > 0, "Salvage Operations was offered and followed");
        assert!(built > 0, "a ship was produced");
        assert!(paid_questions > 0, "a payment question was put inside the ability");
    }

    /// Salvage Operations producing a cruiser (package W3): the form a Mentak seat places for a cruiser is the core's
    /// `placed_unit` with the override of the hook table (Mentak's Corsair for a Cruiser II under The Table's Grace). Without the
    /// breakthrough it is the seat's plain cruiser; with Cruiser II and the breakthrough the Corsair. Both in lockstep.
    #[test]
    fn salvage_operations_produces_a_cruiser_in_the_core_window_in_lockstep() {
        let mut built = [0_usize; 2];
        for (grace, seeds) in [(false, 0..6_u64), (true, 0..6_u64)] {
            for seed in seeds {
                let (state, windows, differences, _) = hub_fight(
                    seed,
                    &["mentak", "hacan"],
                    |s, system| {
                        s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("so"));
                        if grace {
                            s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("cr2"));
                            s.player_mut(&attacker()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("mentakbt"));
                        }
                        put(s, system, "dreadnought", &attacker(), 3);
                        put(s, system, "carrier", &attacker(), 1);
                        put(s, system, "cruiser", &defender(), 2);
                        s.player_mut(&attacker()).unwrap().trade_goods = 5;
                    },
                    |_, _, _| 0,
                );
                if differences > 0 {
                    let all: Vec<String> = state.system_state(&SystemId::new(crate::fixtures::plain_hub().centre)).units.iter().map(|u| format!("{}:{}", u.owner, u.type_id)).collect();
                    eprintln!("string units after the fight: {all:?}");
                }
                assert_eq!(differences, 0, "grace {grace} seed {seed}: {windows:?}");
                assert!(windows.keys().all(|k| !k.contains("a fact the adapter did not compute")), "grace {grace} seed {seed}: {windows:?}");
                let units = &state.system_state(&SystemId::new(crate::fixtures::plain_hub().centre)).units;
                built[usize::from(grace)] += units.iter().filter(|u| u.owner == attacker() && u.type_id.as_str() == if grace { "mentak_cruiser3" } else { "cruiser" }).count();
            }
        }
        assert!(built[0] > 0, "a plain cruiser was salvaged");
        assert!(built[1] > 0, "a Corsair was salvaged");
    }

    /// Package W2: a Naaz side no longer gates the opening of a space round. In round 1 the ground-form mechs in the space area take their
    /// ship form (and join the fleet the end announcement measures the losses against), the Eidolon Maximum is repaired, and Supercharge
    /// is offered (both answers); the whole composed machine runs in the core and agrees with the string.
    #[test]
    fn a_naaz_side_opens_its_space_rounds_in_the_core_machine_in_lockstep() {
        let (mut compared_settle, mut used) = (0, 0);
        static ASKED_SC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        for (seed, composed) in (0..8_u64).flat_map(|s| [(s, false), (s, true)]) {
            let (state, windows, differences, _) = hub_fight_composed(
                composed,
                seed,
                &["naaz", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 2);
                    put(s, system, "naaz_mech", &attacker(), 2);
                    put(s, system, "cruiser", &defender(), 2);
                    put(s, system, "destroyer", &defender(), 1);
                    s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("sc"));
                },
                |seed, _, c| {
                    if c.prompt.starts_with("Supercharge") {
                        ASKED_SC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        usize::from(seed % 2 == 1)
                    } else {
                        0
                    }
                },
            );
            assert_eq!(differences, 0, "seed {seed} composed {composed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("a Naaz side")), "seed {seed}: {windows:?}");
            compared_settle += windows.get(if composed { "combat::settle" } else { "combat::begin_round" }).map_or(0, |w| w.compared);
            let units = &state.system_state(&SystemId::new(crate::fixtures::plain_hub().centre)).units;
            assert!(!units.iter().any(|u| u.type_id.as_str() == "naaz_mech_space"), "seed {seed}: no ship form survives the battle");
            used += usize::from(state.player(&attacker()).unwrap().exhausted_technologies.contains(ti4_model::id::text(&"sc")));
        }
        assert!(compared_settle > 0, "the composed machine ran: {compared_settle}");
        assert!(ASKED_SC.load(std::sync::atomic::Ordering::Relaxed) > 0, "Supercharge was offered");
        assert!(used > 0, "and used");
    }

    /// Package W2: a Letnev side opens its space rounds in the core (round opening and composed machine): Arc Secundus repairs itself at
    /// the start of each round, after the faction hooks of its owner. Fails on the pre-W2 gate ("a Letnev side": the window bailed and the
    /// composed settle ran bare).
    #[test]
    fn a_letnev_side_repairs_its_flagship_at_each_round_opening_in_lockstep() {
        let (mut compared, mut repaired_at_start) = (0, 0);
        for (seed, composed) in (0..8_u64).flat_map(|s| [(s, false), (s, true)]) {
            let (_, windows, differences, _) = hub_fight_composed(
                composed,
                seed,
                &["letnev", "hacan"],
                |s, system| {
                    put(s, system, "letnev_flagship", &attacker(), 1);
                    put(s, system, "cruiser", &attacker(), 1);
                    put(s, system, "cruiser", &defender(), 3);
                    for unit in &mut s.system_mut(system).units {
                        if unit.type_id.as_str() == "letnev_flagship" {
                            unit.sustained_damage = true;
                        }
                    }
                },
                |_, _, _| 0,
            );
            assert_eq!(differences, 0, "seed {seed} composed {composed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("a Letnev side")), "seed {seed}: {windows:?}");
            let counts = windows.get(if composed { "combat::settle" } else { "combat::begin_round" }).copied().unwrap_or_default();
            assert_eq!(counts.bailed, 0, "seed {seed} composed {composed}: {windows:?}");
            compared += counts.compared;
            repaired_at_start += 1;
        }
        assert!(compared > 0 && repaired_at_start == 16, "the opening ran in the core: {compared}");
    }

    /// Package W2: the hits the modules produce at the start of a combat and after a round run in the core window `ProduceHits` in
    /// lockstep: Mentak's Ambush (which ships roll; each against its own combat value), Yin's Impulse Core and Devotion (a ship is
    /// destroyed for a hit), Ghost's Dimensional Splicer (a wormhole in the system). Answers: the first offer, the decline, the last offer.
    #[test]
    fn the_modules_hits_at_the_start_and_after_a_round_run_in_the_core_window_in_lockstep() {
        static PUT: [std::sync::atomic::AtomicUsize; 4] = [const { std::sync::atomic::AtomicUsize::new(0) }; 4];
        fn answer(seed: u64, nth: usize, c: &Choice) -> usize {
            let Some(which) = ["Ambush", "Impulse Core", "Devotion", "Dimensional Splicer"].iter().position(|p| c.prompt.starts_with(p)) else {
                return 0;
            };
            PUT[which].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            match (seed + nth as u64) % 3 {
                0 => 0,
                1 => c.options.len() - 1,
                _ => c.options.len().saturating_sub(2),
            }
        }
        let cases: [(&str, &[&str], &str); 4] = [
            ("ambush", &["mentak", "hacan"], "Ambush"),
            ("impulse core", &["yin", "hacan"], "Impulse Core"),
            ("devotion", &["yin", "hacan"], "Devotion"),
            ("dimensional splicer", &["ghost", "hacan"], "Dimensional Splicer"),
        ];
        for (which, (name, factions, _prompt)) in cases.into_iter().enumerate() {
            let mut compared = 0;
            for seed in 0..12_u64 {
                let (_, windows, differences, _) = hub_fight_composed(
                    false,
                    seed,
                    factions,
                    |s, system| {
                        put(s, system, "cruiser", &attacker(), 2);
                        put(s, system, "destroyer", &attacker(), 2);
                        put(s, system, "cruiser", &defender(), 3);
                        put(s, system, "dreadnought", &defender(), 1);
                        if name == "impulse core" {
                            s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ic"));
                        }
                        if name == "dimensional splicer" {
                            s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ds"));
                            s.wormhole_tokens.insert("ALPHA".to_owned(), system.clone());
                        }
                    },
                    answer,
                );
                assert_eq!(differences, 0, "{name} seed {seed}: {windows:?}");
                let w = windows.get("combat::produce_hits").copied().unwrap_or_default();
                assert_eq!(w.bailed, 0, "{name} seed {seed}: {windows:?}");
                compared += w.compared;
            }
            assert!(compared > 0, "{name}: the window ran: {compared}");
            assert!(PUT[which].load(std::sync::atomic::Ordering::Relaxed) > 0, "{name}: the offer was put");
        }
    }

    /// The core sees what the Mentak seat of the salvage tests holds: Cruiser II and The Table's Grace.
    #[test]
    fn the_core_sees_cruiser_two_and_the_tables_grace_of_a_mentak_seat() {
        let mut state = crate::fixtures::seated_game(&[("a", "mentak"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
        state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("cr2"));
        state.player_mut(&attacker()).unwrap().breakthrough = Some(ti4_model::id::BreakthroughId::new("mentakbt"));
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        let (mut cx, r) = crate::dual_window::tables_for_tests(&state).unwrap();
        let view = crate::core_sync::CoreView::of(&state, &mut cx, crate::core_sync::Sources { galaxy: None::<&ti4_content::galaxy::Galaxy>, rng: None }, crate::production_view::READS)
            .unwrap()
            .into_state();
        let seat = r.seat("a").unwrap();
        let p = view.players().by_seat(seat).unwrap();
        assert!(p.tech.owned.contains(ti4_engine_core::generated::Technology::CruiserII), "Cruiser II");
        assert_eq!(p.cards.breakthrough, Some(ti4_engine_core::generated::Breakthrough::TheTablesGrace), "The Table's Grace");
    }

    /// Package W3: the end announcement flips the survivors of a dual form back to their ground form in the core window
    /// (`fx_strategy::flip_space_area` before the report is announced), compared with the string. A Z-Grav Eidolon fights in
    /// the fleet and does not outlive the battle as a ship.
    #[test]
    fn the_end_of_a_fight_flips_a_z_grav_eidolon_back_in_the_core_window_in_lockstep() {
        let (mut compared, mut flipped) = (0, 0);
        for seed in 0..8_u64 {
            let (state, windows, differences, _) = hub_fight(
                seed,
                &["naaz", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "naaz_mech_space", &attacker(), 2);
                    put(s, system, "destroyer", &defender(), 2);
                },
                |_, _, _| 0,
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            // The end is announced by the composed machine (inside `combat::settle`) or by the window of its own.
            let end = windows.get("combat::announce_end").copied().unwrap_or_default();
            assert_eq!(end.bailed, 0, "seed {seed}: the end is announced by the core: {windows:?}");
            compared += end.compared + windows.get("combat::settle").map_or(0, |w| w.compared);
            let units = &state.system_state(&SystemId::new(crate::fixtures::plain_hub().centre)).units;
            assert!(!units.iter().any(|u| u.type_id.as_str() == "naaz_mech_space"), "seed {seed}: no ship form survives");
            flipped += units.iter().filter(|u| u.owner == attacker() && u.type_id.as_str() == "naaz_mech").count();
        }
        assert!(compared > 0, "the end announcement ran in the core");
        assert!(flipped > 0, "a survivor took its ground form");
    }

    /// Ssruu (the Yssaril agent) copies Brother Milor (Yin, a third seat) in a fight: when a ship is destroyed in combat the
    /// borrower exhausts Ssruu and the owner of the ship places 2 fighters. A pair rule of the table: owned by the borrower, the
    /// lender in the id.
    #[test]
    fn ssruu_copies_brother_milor_in_the_core_window_in_lockstep() {
        let mut offered = 0;
        for seed in 0..10_u64 {
            let (state, windows, differences, nested) = hub_fight(
                seed,
                &["yssaril", "hacan", "yin"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "destroyer", &defender(), 3);
                    put(s, system, "dreadnought", &defender(), 1);
                },
                |_, _, c| c.options.iter().position(|o| o.id.contains("yssarilagent:yinagent")).unwrap_or(0),
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            // (A bail of the composed machine itself was silent before package W0; it names a position of the machine, not the ability.)
            assert!(windows.keys().all(|k| !k.contains("[bail:") || k.contains("[bail: the machine")), "seed {seed}: {windows:?}");
            offered += nested.iter().filter(|k| k.contains("yssarilagent:yinagent:SHIP_DESTROYED:after")).count();
            if state.player(&attacker()).unwrap().leaders.get(ti4_model::id::text(&"yssarilagent")) == Some(&ti4_model::state::LeaderStatus::Readied) {
                continue;
            }
        }
        assert!(offered > 0, "the copy was offered and followed");
    }

    /// Ssruu copies Viscount Unlenn (Letnev, a third seat) at the start of a space combat round: the borrower chooses one ship
    /// for an extra die, the mark names borrower and lender.
    #[test]
    fn ssruu_copies_viscount_unlenn_in_the_core_window_in_lockstep() {
        let (mut offered, mut asked) = (0, 0);
        for seed in 0..8_u64 {
            let (state, windows, differences, nested) = hub_fight(
                seed,
                &["yssaril", "hacan", "letnev"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "destroyer", &defender(), 3);
                },
                |seed, _, c| {
                    if let Some(i) = c.options.iter().position(|o| o.id.contains("ssruu-copy")) {
                        return i;
                    }
                    if c.prompt.starts_with("Ssruu copying") {
                        return seed as usize;
                    }
                    0
                },
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("[bail:")), "seed {seed}: {windows:?}");
            offered += nested.iter().filter(|k| k.contains(":letnevagent:c:COMBAT_ROUND_STARTED:after")).count();
            asked += nested.iter().filter(|k| k.contains("ask:Ssruu copying")).count();
            assert!(state.faction_marks.keys().any(|k| k.starts_with("yssaril:ssruu-round-agent:a:letnevagent:")) || offered == 0);
        }
        assert!(offered > 0 && asked > 0, "the copy was offered ({offered}) and asked ({asked})");
    }

    /// The extra die of a round agent is read from the core state: in the composed settle the mark is written and read in one
    /// window, which the string-oracle snapshot of the position at its start could not see (the Ssruu copy of Viscount Unlenn
    /// found it: 3 dice against 5).
    #[test]
    fn the_letnev_agent_extra_die_is_read_in_the_window_that_writes_the_mark() {
        let mut marked = 0;
        for seed in 0..6_u64 {
            let (state, windows, differences, _) = hub_fight(
                seed,
                &["letnev", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "destroyer", &defender(), 3);
                },
                |seed, _, c| if c.prompt.starts_with("leader") || c.prompt.contains("choose one unit") { seed as usize } else { 0 },
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            marked += state.faction_marks.keys().filter(|k| k.starts_with("yssaril:ssruu-round-agent:a:letnevagent:")).count();
        }
        assert!(marked > 0, "the agent marked a ship");
    }

    /// The owner has nothing to spend: no planet ready, no trade goods or commodities.
    fn starve(s: &mut GameState, who: &PlayerId) {
        let planets: Vec<_> = s.controlled_planets(who).into_iter().map(|(_, p)| p.clone()).collect();
        s.exhausted_planets.extend(planets);
        let seat = s.player_mut(who).unwrap();
        seat.trade_goods = 0;
        seat.commodities = 0;
    }

    /// The conditions that keep Dynamo, the Watcher and Salvage Operations out of a window are the string's: a flagship owner who
    /// cannot pay 2 influence, a card the Empyrean played itself, a mech out of reach, a Mentak who lost, a type with no
    /// plastic left and a bill the Mentak cannot meet. The core derives each from the state, never from a snapshot.
    #[test]
    fn the_conditions_of_dynamo_the_watcher_and_salvage_equal_the_strings() {
        for seed in 0..6_u64 {
            // Dynamo without influence: not offered.
            let (_, windows, differences, nested) = hub_fight(
                seed,
                &["empyrean", "hacan"],
                |s, system| {
                    put(s, system, "empyrean_flagship", &attacker(), 1);
                    put(s, system, "cruiser", &attacker(), 2);
                    put(s, system, "dreadnought", &defender(), 3);
                    starve(s, &attacker());
                },
                |_, _, _| 0,
            );
            assert_eq!(differences, 0, "dynamo, seed {seed}: {windows:?}");
            assert!(nested.iter().all(|k| !k.contains("empyrean_flagship")), "an empyrean who cannot pay is offered nothing: {nested:?}");
            // The Empyrean plays the card itself and has a far mech: no Watcher for its own card, none out of reach.
            let (_, windows, differences, nested) = hub_fight(
                seed,
                &["empyrean", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 2);
                    put(s, system, "dreadnought", &defender(), 2);
                    s.player_mut(&attacker()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("mb1")];
                    let hub = crate::fixtures::plain_hub();
                    let far = crate::fixtures::plain_systems(9).into_iter().find(|id| id != &hub.centre && !hub.outer.contains(id)).unwrap();
                    put(s, &SystemId::new(far), "empyrean_mech", &attacker(), 1);
                },
                |_, _, c| c.options.iter().position(|o| o.id.starts_with("reaction:")).unwrap_or(0),
            );
            assert_eq!(differences, 0, "watcher, seed {seed}: {windows:?}");
            assert!(nested.iter().all(|k| !k.contains("empyrean_mech")), "no Watcher for its own card, none out of reach: {nested:?}");
            // A Mentak who loses is owed its trade good and no ship; one with no resources is offered none; a type with no
            // plastic left is not offered.
            for (case, outcome) in [("loses", 0), ("starved", 1), ("no dreadnoughts left", 2)] {
                let (_, windows, differences, nested) = hub_fight(
                    seed,
                    &["mentak", "hacan"],
                    |s, system| {
                        s.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("so"));
                        put(s, system, "carrier", &attacker(), 1);
                        match outcome {
                            0 => {
                                put(s, system, "cruiser", &attacker(), 1);
                                put(s, system, "dreadnought", &defender(), 4);
                            }
                            1 => {
                                // The trade good the mandatory half pays first buys nothing of 4 resources.
                                put(s, system, "dreadnought", &attacker(), 3);
                                put(s, system, "dreadnought", &defender(), 1);
                                starve(s, &attacker());
                            }
                            _ => {
                                put(s, system, "dreadnought", &attacker(), 5);
                                put(s, system, "dreadnought", &defender(), 1);
                                s.player_mut(&attacker()).unwrap().trade_goods = 5;
                            }
                        }
                    },
                    |_, _, _| 0,
                );
                assert_eq!(differences, 0, "salvage {case}, seed {seed}: {windows:?}");
                assert!(nested.iter().all(|k| !k.contains("so:SPACE_COMBAT_ENDED:produce")) || outcome == 2, "{case}: {nested:?}");
            }
        }
    }

    /// Courageous to the End stays the string's: its casualty question for the opponent has the holder as producer, which the
    /// core question does not carry. The window that meets the card bails, named (Reveal Prototype's bail is the unit test of
    /// `cost_is_payable`, `None` for it, and `bail_a_played_card_the_core_does_not_play_is_named`).
    #[test]
    fn bail_courageous_to_the_end_is_counted() {
        let mut named = 0;
        for seed in 1..=10_u64 {
            let (_, _, bails) = declared_cards_fight(seed, &["courageous"], true);
            named += bails.iter().filter(|k| k.contains("(card courageous)")).count();
        }
        assert!(named > 0, "a Courageous to the End was played and named");
    }

    /// Direct Hit: the attacker's hit is sustained by a dreadnought, the attacker destroys that ship from the slot of
    /// `SUSTAIN_DAMAGE_USED`; the core removes it and announces `SHIP_DESTROYED` after the discard. With a faction module
    /// seated (staging) the exact sustained ship goes; without, the first of the type.
    #[test]
    fn direct_hit_destroys_the_sustained_ship_in_the_core_window_in_lockstep() {
        for factions in [["sol", "hacan"], ["mentak", "hacan"]] {
            let mut hit = 0;
            for seed in 1..=8_u64 {
                let (_, _, bails, plays) = cards_fight(seed, factions, &["dh1"], &[], false);
                assert!(bails.iter().all(|k| !k.contains("(card dh")), "{factions:?}: {bails:?}");
                hit += usize::from(plays.iter().any(|c| c == "dh1"));
            }
            assert!(hit > 0, "{factions:?}: a Direct Hit was played");
        }
    }

    /// The first golden run of the card plays found 12 differences: Reflective Shielding stages hits that the sustain
    /// window of the string takes up afterwards, a state the core windows do not carry. The card stays the string's: the
    /// window bails naming it, nothing differs.
    #[test]
    fn reflective_shielding_stays_in_the_string_in_lockstep() {
        let mut bailed = 0;
        for seed in 1..=8_u64 {
            let (_, _, bails) = declared_cards_fight(seed, &["reflective"], false);
            bailed += bails.iter().filter(|k| k.contains("(card reflective)")).count();
        }
        assert!(bailed > 0, "a Reflective Shielding was played and named");
    }

    /// One space fight in which the defender holds `hand`, every question answered with its first option; the windows
    /// are compared. `(cards played, choices of a card asked, the window keys of the report)`.
    fn declared_cards_fight(seed: u64, hand: &[&str], damaged: bool) -> (usize, usize, Vec<String>) {
        let (played, asked, bails, _) = cards_fight(seed, ["sol", "hacan"], &[], hand, damaged);
        (played, asked, bails)
    }

    /// [`declared_cards_fight`] with the attacker's hand too; the last element is the card plays of the state, in order.
    fn cards_fight(seed: u64, factions: [&str; 2], attacker_hand: &[&str], hand: &[&str], damaged: bool) -> (usize, usize, Vec<String>, Vec<String>) {
        cards_fight_at(seed, factions, attacker_hand, hand, damaged, false)
    }

    /// [`cards_fight`] on the centre of a plain hub, with its map lent to the combat (`hub`): the cards that look at the map
    /// (Skilled Retreat) find their destinations.
    fn cards_fight_at(seed: u64, factions: [&str; 2], attacker_hand: &[&str], hand: &[&str], damaged: bool, hub: bool) -> (usize, usize, Vec<String>, Vec<String>) {
        cards_fight_with(seed, factions, attacker_hand, hand, damaged, hub, &|_, _| {})
    }

    /// [`cards_fight_at`] with `extra` to set the position further (units, technologies) before the fight.
    #[allow(clippy::too_many_arguments)]
    fn cards_fight_with(seed: u64, factions: [&str; 2], attacker_hand: &[&str], hand: &[&str], damaged: bool, hub: bool, extra: &dyn Fn(&mut GameState, &SystemId)) -> (usize, usize, Vec<String>, Vec<String>) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let hub_map = crate::fixtures::plain_hub();
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let (mut played, mut asked) = (0, 0);
        let mut plays = Vec::new();
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || crate::dual_window::with_composed(true, || {
            let mut state = crate::fixtures::seated_game(&[("a", factions[0]), ("b", factions[1])], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            state.player_mut(&defender()).unwrap().action_cards = hand.iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
            state.player_mut(&attacker()).unwrap().action_cards = attacker_hand.iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
            let system = if hub { SystemId::new(hub_map.centre.clone()) } else { SystemId::new("18") };
            put(&mut state, &system, "cruiser", &attacker(), 3);
            if damaged {
                put(&mut state, &system, "cruiser", &defender(), 3);
            }
            put(&mut state, &system, "dreadnought", &defender(), 2);
            state.active_system = Some(system.clone());
            state.active = Some(attacker());
            for unit in state.system_mut(&system).units.iter_mut().filter(|u| u.owner == defender() && u.type_id.as_str() == "dreadnought") {
                unit.sustained_damage = damaged;
            }
            extra(&mut state, &system);
            let seen = std::rc::Rc::new(std::cell::RefCell::new(0));
            struct Count(std::rc::Rc<std::cell::RefCell<usize>>);
            impl crate::choice::Decider for Count {
                fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                    if choice.prompt.starts_with("play an action card") {
                        *self.0.borrow_mut() += 1;
                    }
                    crate::choice::FirstOption.choose(choice)
                }
            }
            let mut table = Table::with_default(Box::new(Count(seen.clone())));
            let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
            let mut resolver = crate::fixtures::armed_resolver(&state);
            let mut sequence = crate::event::EventSequence::new();
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: hub.then_some(&hub_map.galaxy) }),
            };
            resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
            played = state.action_card_plays.len();
            plays = state.action_card_plays.iter().map(|(_, card)| card.to_string()).collect();
            asked = *seen.borrow();
        }));
        let r = report();
        assert_eq!(r.total_differences(), 0, "{attacker_hand:?} {hand:?}: {:?}", r.differences);
        let bails = r.windows.keys().cloned().collect();
        let _ = take_report();
        (played, asked, bails, plays)
    }

    #[test]
    fn the_end_report_of_a_fight_is_checked_against_the_core() {
        // `combat=dual`: the reason, losers, retreats and lost ships of `SPACE_COMBAT_ENDED` are computed by
        // `rules::combat::end` and compared; a fight with losses has a non-empty fleet diff.
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut lost = 0;
        for seed in 0..6_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "destroyer", &defender(), 2);
            with_mask(AreaMask::STRING.with(Area::Combat, Backend::Dual).with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
            });
            lost += 5 - ships_of(&state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &attacker(), &system).len()
                - ships_of(&state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &defender(), &system).len();
        }
        let r = report();
        assert!(lost > 0, "some ships were lost");
        assert!(r.areas[&Area::Driver].calls >= 6, "the announcements were compared: {:?}", r.areas[&Area::Driver]);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_barrage_step_runs_as_a_core_window_in_lockstep() {
        // Round 1 with a barrage on `driver=dual`: the core window draws the same dice from the same stream
        // positions, removes the same fighters, records the same feat and announces the same events.
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut killed = 0;
        for seed in 0..10_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, &a_barrage_unit(), &attacker(), 3);
            put(&mut state, &system, "fighter", &defender(), if seed % 2 == 0 { 1 } else { 4 });
            put(&mut state, &system, "cruiser", &defender(), 1);
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                killed += usize::from(dice.history().iter().any(|r| r.reason.starts_with("anti") && r.hits() > 0));
            });
        }
        let r = report();
        assert!(killed > 0, "some barrage hit");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas[&Area::Driver].calls >= 20, "the steps were compared: {:?}", r.areas[&Area::Driver]);
        let _ = take_report();
    }

    /// A Waylay played before a side's barrage roll sends its hits to every ship: the core barrage step does not carry it, so the
    /// window bails (counted) instead of finishing with the fighters' removal; found by the soak (a sustained dreadnought).
    #[test]
    fn a_waylay_in_the_barrage_is_a_counted_bail_not_a_difference() {
        let mut bailed = 0;
        for seed in 0..10_u64 {
            let (_state, windows, differences, _nested) = hub_fight(
                seed,
                &["sol", "hacan"],
                |s, system| {
                    put(s, system, &a_barrage_unit(), &attacker(), 3);
                    put(s, system, "fighter", &defender(), 1);
                    put(s, system, "dreadnought", &defender(), 2);
                    s.player_mut(&attacker()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("waylay")];
                },
                |_, _, c| c.options.iter().position(|o| o.id.starts_with("reaction:")).unwrap_or(0),
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            bailed += windows.iter().filter(|(k, v)| k.contains("[bail: Waylay") && v.bailed > 0).count();
        }
        assert!(bailed > 0, "a Waylay was played in the barrage and the window bailed");
    }

    /// A reaction window the loss of a ship opens inside the hit assignment (Crash Landing, when the last ship falls) is not a hit's
    /// question: its pick carries no hit detail, in the string and in the core window; found by the soak (space cannon hits).
    #[test]
    fn a_reaction_inside_the_hit_assignment_carries_no_hit_detail() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        struct Decline;
        impl crate::choice::Decider for Decline {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                let i = choice.options.iter().position(|o| o.id == "decline").unwrap_or(0);
                choice.options.get(i).cloned().ok_or_else(|| IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() })
            }
        }
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        for seed in 0..4_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "mentak")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let (system, _planet) = crate::fixtures::a_placed_planet();
            state.active_system = Some(system.clone());
            state.active = Some(attacker());
            put(&mut state, &system, "cruiser", &defender(), 1);
            put(&mut state, &system, "infantry", &defender(), 1);
            state.player_mut(&defender()).unwrap().action_cards = vec![ti4_model::id::ActionCardId::new("crashlanding")];
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(Decline));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                absorb_hits_seeing_with(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, None, &mut ctx, &defender(), &system, &attacker(), 1, false, HitOrigin::UnitAbility).unwrap();
            });
        }
        let r = report();
        let compared = r.windows.get("combat::assign_hits").map_or(0, |w| w.compared);
        let offered = r.nested.keys().filter(|k| k.contains("reaction:*:SHIP_DESTROYED:when")).count();
        assert!(offered > 0 && compared > 0, "Crash Landing was offered inside a compared assignment: {offered} {compared} {:?} {:?}", r.windows, r.nested);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
        let _ = take_report();
    }

    /// Tellurian in the anti-fighter barrage window: the agent's cancellation is spent on the barrage's hits before the
    /// fighters are taken, by the core barrage window as by the string `apply_barrage`.
    #[test]
    fn tellurian_cancels_a_barrage_hit_in_the_core_window_in_lockstep() {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut spared = 0;
        for seed in 0..12_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "titans")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, &a_barrage_unit(), &attacker(), 3);
            put(&mut state, &system, "fighter", &defender(), 4);
            put(&mut state, &system, "cruiser", &defender(), 1);
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                spared += usize::from(state.player(&defender()).unwrap().leaders.get(ti4_model::id::text(&"titansagent")) == Some(&ti4_model::state::LeaderStatus::Exhausted));
            });
        }
        let r = report();
        assert!(spared > 0, "the agent was used");
        assert!(r.windows.keys().all(|k| !k.contains("unported ability leader:*:titansagent")), "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn the_roll_step_runs_as_a_core_window_in_lockstep() {
        // Whole fights on `driver=dual`: every round's dice, hits, round records and assignment queue come out
        // of the core window as the string step leaves them (no reroll question in these fleets).
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut rolled = 0;
        for seed in 0..12_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 2);
            put(&mut state, &system, "dreadnought", &attacker(), 1);
            put(&mut state, &system, "fighter", &attacker(), 2);
            put(&mut state, &system, "destroyer", &defender(), 2);
            put(&mut state, &system, "carrier", &defender(), 1);
            put(&mut state, &system, "fighter", &defender(), 3);
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                rolled += dice.history().iter().filter(|r| r.reason == "space combat").count();
            });
        }
        let r = report();
        assert!(rolled > 20, "dice were rolled: {rolled}");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.areas[&Area::Driver].calls >= 24, "the steps were compared: {:?}", r.areas[&Area::Driver]);
        let _ = take_report();
    }

    /// Whole fights on `driver=dual` with `setup` applied to a fresh 18-system fight per seed (the attacker `a`
    /// and defender `b`, a third seat `c` for the Heart of Ixth), answered by a seeded random decider (and by
    /// the first option of every question): the roll step's dice, questions, events, state and queue must
    /// come out of the core window as the string step leaves them. Returns every question that was asked.
    fn roll_step_lockstep(setup: &dyn Fn(&mut GameState, u64), calls_at_least: u64) -> Vec<crate::choice::Choice> {
        roll_step_counts(setup, calls_at_least).0
    }

    /// [`roll_step_lockstep`], also the (compared, bailed) counts of the roll step window.
    fn roll_step_counts(setup: &dyn Fn(&mut GameState, u64), calls_at_least: u64) -> (Vec<crate::choice::Choice>, u64, u64) {
        let single = roll_step_fights(setup, calls_at_least, false);
        // The same fights with `settle` compared as one composed machine.
        let composed = roll_step_fights(setup, 0, true);
        SETTLE_COUNTS.with(|c| c.set((composed.1, composed.2)));
        single
    }

    fn roll_step_fights(setup: &dyn Fn(&mut GameState, u64), calls_at_least: u64, composed: bool) -> (Vec<crate::choice::Choice>, u64, u64) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        crate::dual_window::with_composed(composed, || roll_step_fights_inner(setup, calls_at_least, composed))
    }

    fn roll_step_fights_inner(setup: &dyn Fn(&mut GameState, u64), calls_at_least: u64, composed: bool) -> (Vec<crate::choice::Choice>, u64, u64) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut asked = Vec::new();
        for seed in 0..16_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "dreadnought", &attacker(), 1);
            put(&mut state, &system, "fighter", &attacker(), 2);
            put(&mut state, &system, "destroyer", &defender(), 2);
            put(&mut state, &system, "carrier", &defender(), 1);
            put(&mut state, &system, "fighter", &defender(), 3);
            state.player_mut(&attacker()).unwrap().trade_goods = 8;
            state.player_mut(&defender()).unwrap().trade_goods = 8;
            setup(&mut state, seed);
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let decider: Box<dyn crate::choice::Decider> =
                    if seed % 4 == 0 { Box::new(crate::choice::FirstOption) } else { Box::new(crate::choice::SeededRandom::new(seed)) };
                let (decider, seen) = crate::choice::Capturing::new(decider);
                let mut table = Table::with_default(Box::new(decider));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
                asked.extend(seen.borrow().iter().cloned());
            });
        }
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        if calls_at_least > 0 {
            assert!(r.areas[&Area::Driver].calls >= calls_at_least, "the steps were compared: {:?}", r.areas[&Area::Driver]);
        }
        let counts = r.windows.get("combat::roll_round").cloned().unwrap_or_default();
        let settle = r.windows.get("combat::settle").cloned().unwrap_or_default();
        let _ = take_report();
        if composed {
            return (asked, settle.compared, settle.bailed);
        }
        (asked, counts.compared, counts.bailed)
    }

    thread_local! {
        /// (compared, bailed) of the composed machine in the last `roll_step_counts`.
        static SETTLE_COUNTS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
    }

    /// The gates the composed machine starts from, for the arena after `setup`.
    fn settle_gates(setup: &dyn Fn(&mut GameState, &SystemId)) -> crate::dual_window::SettleArgs {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &attacker(), 1);
        put(&mut state, &system, "cruiser", &defender(), 1);
        setup(&mut state, &system);
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        let (mut dice, mut rng) = (Dice::new(), GameRng::new(state.rng_seed));
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let ctx = Resolving { content: ContentStore::embedded(), sources: POK, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
        window.settle_args(&state, &ctx).expect("a map-less fight has gates")
    }

    #[test]
    fn the_composed_combat_stays_string_where_the_core_does_not_carry_the_position() {
        // Each string-only case of the composed machine is a gate (or a machine bail) with a reason: the
        // string `settle` stands there and the windows inside are compared one by one instead.
        let plain = settle_gates(&|_, _| {});
        assert!(!plain.opening_unported && !plain.barrage_unported && !plain.roll_unported && !plain.retreat_unported);
        // Naaz and Letnev (Voltron repair, Supercharge, Munitions, the Eidolon flip), diplomacy.
        // (A Naaz side is carried by the core since package W2: the Eidolon flip, the repair of the Maximum, Supercharge.)
        // (A Letnev side is carried since package W2 except for Munitions Reserves paid at the opening, below rules version 8.)
        let g = settle_gates(&|state, _| state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("letnev"));
        assert!(!g.opening_unported, "letnev");
        let g = settle_gates(&|state, _| {
            state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("letnev");
            state.rules_version = 7;
        });
        assert!(g.opening_unported, "letnev below rules version 8");
        let g = settle_gates(&|state, _| state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("naaz"));
        assert!(!g.opening_unported, "naaz");
        assert!(settle_gates(&|state, _| state.diplomacy.enabled = true).opening_unported);
        // Hits a module makes at the start of a combat (Mentak's Ambush, Assault Cannon, Dimensional Splicer,
        // Impulse Core): the string asks the modules, the core ability table is empty.
        assert!(settle_gates(&|state, _| state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("mentak")).opening_unported);
        // The Thundarian is an ability of the core table since package W2 (`a_readied_thundarian_replays_the_roll_step_in_the_core_window_in_lockstep`).
        // A Last Bastion seat's retreat note is what the retreat gate (`retreat_unported`) still names.
        let retreat = |setup: &dyn Fn(&mut GameState, &SystemId)| {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &attacker(), 1);
            put(&mut state, &system, "cruiser", &defender(), 1);
            setup(&mut state, &system);
            crate::dual_window::retreat_unported(&state, ContentStore::embedded(), POK, &attacker(), &system, &SystemId::new("19"))
        };
        assert!(!retreat(&|_, _| {}));
        assert!(retreat(&|state, _| state.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("bastion")));
        // The dual-form units, a Voltron on a planet and a Floating Factory are carried by the core retreat window since package W3.
        assert!(!retreat(&|state, system| put(state, system, "naaz_mech_space", &attacker(), 1)));
        assert!(!retreat(&|state, system| put(state, system, "saar_spacedock", &attacker(), 1)));
        // Duranium Armor repairs and Devotion's hits after a round: flags the machine bails on when it reaches them.
        let g = settle_gates(&|state, _| {
            state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("da"));
            state.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("yin");
        });
        assert_eq!(g.duranium, [true, false]);
        assert!(g.devotion);
        // Neutral sides never reach the core machine.
        assert!(crate::neutral_units::is_neutral(&crate::neutral_units::owner()));
    }

    #[test]
    fn the_composed_combat_machine_runs_whole_fights_in_lockstep() {
        // Whole fights on `driver=dual`: every settle from the stage it starts in to the next decision is run by
        // the composed core machine and compared (state, events, dice, streams, the window's fields).
        let _ = roll_step_counts(&|_, _| {}, 10);
        let (compared, bailed) = SETTLE_COUNTS.with(std::cell::Cell::get);
        assert!(compared > 20, "settles were compared: {compared} compared, {bailed} bailed");
    }

    /// Whole fights on a hub map, where a retreat has somewhere to go, on `driver=dual` with `settle` composed: the
    /// attacker may retreat to the first two outer systems, the defender to the next two. Every pending decision
    /// (announce a retreat, retreat to which system, sustain, assign a hit) is answered by a seeded random decider
    /// and compared with the core machine's question and answer (`combat::answer`). Returns the questions asked and
    /// the (compared, bailed) counts of the answers.
    fn pending_fights(setup: &dyn Fn(&mut GameState, &SystemId, u64), seeds: u64) -> (Vec<crate::choice::Choice>, u64, u64) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        crate::dual_window::with_composed(true, || {
            let _guard = crate::dual::tests::serial();
            let _ = take_report();
            let content = ContentStore::embedded();
            let mut asked = Vec::new();
            for seed in 0..seeds {
                let hub = crate::fixtures::plain_hub();
                let system = SystemId::new(&hub.centre);
                let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "xxcha")], ti4_model::content_types::DEFAULT);
                state.promissory_notes.clear();
                state.promissory_faceup.clear();
                state.rng_seed = seed;
                for id in std::iter::once(&hub.centre).chain(hub.outer.iter().take(4)) {
                    state.board.entry(SystemId::new(id)).or_default();
                }
                put(&mut state, &system, "cruiser", &attacker(), 3);
                put(&mut state, &system, "dreadnought", &attacker(), 1);
                put(&mut state, &system, "fighter", &attacker(), 2);
                put(&mut state, &system, "destroyer", &defender(), 2);
                put(&mut state, &system, "carrier", &defender(), 1);
                put(&mut state, &system, "dreadnought", &defender(), 1);
                put(&mut state, &system, "fighter", &defender(), 3);
                for (i, id) in hub.outer.iter().take(4).enumerate() {
                    let owner = if i < 2 { attacker() } else { defender() };
                    put(&mut state, &SystemId::new(id), "cruiser", &owner, 1);
                }
                setup(&mut state, &system, seed);
                with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                    let decider: Box<dyn crate::choice::Decider> = Box::new(crate::choice::SeededRandom::new(seed));
                    let (decider, seen) = crate::choice::Capturing::new(decider);
                    let mut outer = Table::with_default(Box::new(decider));
                    let mut inner = Table::with_default(Box::new(crate::choice::SeededRandom::new(seed ^ 0x55)));
                    let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                    let mut resolver = crate::fixtures::armed_resolver(&state);
                    let mut sequence = crate::event::EventSequence::new();
                    let mut ctx = Resolving {
                        content,
                        sources: ti4_model::content_types::DEFAULT,
                        dice: &mut dice,
                        rng: &mut rng,
                        table: &mut inner,
                        timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                    };
                    let mut window = CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system).with_galaxy(hub.galaxy.clone());
                    window.settle(&mut state, &mut ctx).unwrap();
                    for _ in 0..400 {
                        if window.outcome().is_some() {
                            break;
                        }
                        if let Some(choice) = window.pending_choice(&state, content, ti4_model::content_types::DEFAULT) {
                            // Rout, played by the defender: the attacker's retreat is forced this round.
                            if matches!(&window.stage, Stage::Announcing { asking, .. } if *asking == attacker()) && seed % 3 == 0 {
                                state.player_mut(&defender()).unwrap().rout_round = Some(state.combat_round_seq);
                            }
                            let choice = window.pending_choice(&state, content, ti4_model::content_types::DEFAULT).unwrap_or(choice);
                            let answer = outer.ask(&choice).unwrap();
                            window.resolve(&mut state, &mut ctx, answer).unwrap();
                        } else {
                            let _ = window.take_scoring_occurrence();
                            window.settle_open(&mut state, &mut ctx).unwrap();
                        }
                    }
                    asked.extend(seen.borrow().iter().cloned());
                });
            }
            let r = report();
            assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
            let counts = r.windows.get("combat::answer").cloned().unwrap_or_default();
            let _ = take_report();
            (asked, counts.compared, counts.bailed)
        })
    }

    #[test]
    fn the_pending_decisions_of_a_combat_run_in_the_core_machine() {
        // Announce a retreat (also forced by Rout), retreat to which system, sustain (with the `sustainers` detail
        // and the previews), assign a hit: the core machine puts each question byte for byte and takes the answer.
        let (asked, compared, bailed) = pending_fights(&|_, _, _| {}, 24);
        let by = |subtype: &str| asked.iter().filter(|c| c.context.as_ref().is_some_and(|x| x.subtype == subtype)).count();
        for subtype in ["announce_retreat", "retreat_to", "sustain_damage", "assign_casualty"] {
            assert!(by(subtype) > 5, "{subtype} was asked: {}", by(subtype));
        }
        assert!(asked.iter().any(|c| c.options.iter().any(|o| o.label == "retreat (forced)")), "a forced retreat");
        assert!(compared > 40, "answers were compared: {compared} compared, {bailed} bailed");
    }

    #[test]
    fn a_sustain_in_a_game_below_the_faction_bugfixes_is_answered_in_the_string() {
        // The commander's offer and the two-hit cancel of Non-Euclidean Shielding differ below
        // `RULES_FACTION_BUGFIXES`: the core machine does not answer a sustain there (the other answers still run).
        let (asked, compared, bailed) = pending_fights(&|state, _, _| state.rules_version = ti4_model::state::RULES_FACTION_BUGFIXES - 1, 12);
        assert!(asked.iter().any(|c| c.context.as_ref().is_some_and(|x| x.subtype == "sustain_damage")));
        assert!(bailed > 0, "sustains were answered in the string: {bailed}");
        assert!(compared > 0, "the other answers ran in the core: {compared}");
    }

    #[test]
    fn a_retreat_of_a_fleet_with_a_floating_factory_is_answered_in_the_core() {
        // Since package W3 the Floating Factory moves like a ship in the core retreat window: its destination question
        // is answered there, in lockstep with the string.
        let (asked, compared, _bailed) = pending_fights(&|state, system, _| put(state, system, "saar_spacedock", &attacker(), 1), 24);
        assert!(asked.iter().any(|c| c.context.as_ref().is_some_and(|x| x.subtype == "retreat_to")));
        assert!(compared > 0, "the answers ran in the core: {compared}");
    }

    #[test]
    fn a_dark_energy_tap_holder_retreats_to_empty_systems_in_the_core_machine() {
        // Package W3/W1: the holder's destinations are the union of 78.7c and the technology's looser set ("adjacent systems with no other
        // player's units, even without units or planets of its own there"); the composed machine puts the same retreat question.
        let hub = crate::fixtures::plain_hub();
        let (asked, compared, bailed) = pending_fights(
            &|state, _, _| {
                state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("det"));
            },
            24,
        );
        let own: Vec<&String> = hub.outer.iter().take(2).collect();
        let retreats: Vec<&crate::choice::Choice> = asked.iter().filter(|c| c.context.as_ref().is_some_and(|x| x.subtype == "retreat_to") && c.player == attacker()).collect();
        assert!(!retreats.is_empty(), "the holder was asked where to retreat");
        assert!(
            retreats.iter().any(|c| c.options.iter().any(|o| o.id != "decline" && !own.iter().any(|id| id.as_str() == o.id))),
            "an empty system the holder has no units in was offered: {:?}",
            retreats.iter().map(|c| c.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>()).collect::<Vec<_>>()
        );
        assert!(compared > 0, "the answers ran in the core: {compared} compared, {bailed} bailed");
    }

    #[test]
    fn a_hit_the_producer_assigns_is_answered_in_the_string() {
        // Devotion: the producing player, not the owner, picks the victim. The core machine does not ask that.
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let content = ContentStore::embedded();
        crate::dual_window::with_composed(true, || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &defender(), 1);
            put(&mut state, &system, "destroyer", &defender(), 1);
            put(&mut state, &system, "cruiser", &attacker(), 1);
            let mut window = CombatWindow::new(&state, content, POK, &system);
            window.stage = Stage::Assigning {
                round: 1,
                queue: vec![Pending { player: defender(), hits: 1, producer: attacker(), non_fighters_only: false, producer_assigns: true }],
            };
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(state.rng_seed));
                let mut table = Table::with_default(Box::new(FirstOption));
                let mut ctx = Resolving { content, sources: POK, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
                let choice = window.pending_choice(&state, content, POK).expect("the producer is asked");
                assert_eq!(choice.player, attacker());
                let answer = choice.options[0].clone();
                window.resolve(&mut state, &mut ctx, answer).unwrap();
            });
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let counts = r.windows.get("combat::answer").cloned().unwrap_or_default();
        assert_eq!((counts.compared, counts.bailed), (0, 1), "{counts:?}");
        let _ = take_report();
    }

    /// `roll_unported` of the arena's window after `setup`.
    fn roll_unported_after(setup: &dyn Fn(&mut GameState, &SystemId)) -> bool {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &attacker(), 1);
        put(&mut state, &system, "cruiser", &defender(), 1);
        setup(&mut state, &system);
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        window.roll_unported(&state, ContentStore::embedded())
    }

    /// Package W lane 1, the documented tail: each of these positions is a gate with its own reason (the windows inside are compared on their
    /// own), pinned here so a port removes the line deliberately: the start-of-combat hits of Assault Cannon, Dimensional Splicer and Impulse
    /// Core (`asc`, `ds`, `ic`; Mentak's Ambush above), an Argent side's excess barrage hits (Raid Formation is a window of its own, the
    /// step is not), the Jol-Nar commander's reroll question and a Waylay played in the barrage.
    #[test]
    fn the_start_hits_and_the_barrage_tail_stay_string_with_their_reasons() {
        for tech in ["asc", "ds", "ic"] {
            let g = settle_gates(&|state, _| {
                state.player_mut(&attacker()).unwrap().technologies.insert(ti4_model::id::TechnologyId::new(tech));
            });
            assert!(g.opening_unported, "{tech}");
            assert_eq!(g.gate_why, Some("Assault Cannon, Dimensional Splicer or Impulse Core (start-of-combat hits)"), "{tech}");
        }
        let barrage = |setup: &dyn Fn(&mut GameState)| {
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
            setup(&mut state);
            let (a, d, system) = (attacker(), defender(), SystemId::new("18"));
            let args = crate::dual_window::BarrageArgs { attacker: &a, defender: &d, system: &system, round: 1, occurrence: 0, staging: true, announce: false, reproducible: true };
            crate::dual_window::barrage_why(&state, &args)
        };
        assert_eq!(barrage(&|_| {}), None);
        assert_eq!(
            barrage(&|s| s.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("argent")),
            Some("an Argent side (the excess hits of the barrage)")
        );
        assert_eq!(
            barrage(&|s| {
                s.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("jolnar");
                s.player_mut(&defender()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("jolnarcommander"), ti4_model::state::LeaderStatus::Unlocked);
            }),
            Some("the Jol-Nar commander (a reroll question in the barrage)")
        );
        assert_eq!(
            barrage(&|s| {
                s.combat_round_seq = 3;
                s.player_mut(&attacker()).unwrap().waylay_barrage_round = Some(3);
            }),
            Some("Waylay played in this barrage")
        );
    }

    #[test]
    fn the_round_opening_of_naaz_letnev_and_diplomacy_games_stays_string() {
        // Munitions, the self-repairing flagship, the Letnev round agent and the attack promises are not in the core window:
        // `begin_round` bails there and counts it (Voltron repair, Supercharge and the Eidolon flip are since package W2).
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        fn args<'a>(a: &'a PlayerId, d: &'a PlayerId, system: &'a SystemId) -> crate::dual_window::BeginRoundArgs<'a> {
            crate::dual_window::BeginRoundArgs { attacker: a, defender: d, system, round: 2, announce: false }
        }
        let content = ContentStore::embedded();
        let (a, d, system) = (attacker(), defender(), SystemId::new("18"));
        let plain = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
        assert!(!crate::dual_window::round_opening_unported(&plain, content, &args(&a, &d, &system)));
        for (faction, version, unported) in [("naaz", plain.rules_version, false), ("letnev", plain.rules_version, false), ("letnev", 7, true), ("sol", 7, false)] {
            let mut state = plain.clone();
            state.player_mut(&d).unwrap().faction = ti4_model::id::FactionId::new(faction);
            state.rules_version = version;
            assert_eq!(crate::dual_window::round_opening_unported(&state, content, &args(&a, &d, &system)), unported, "{faction} {version}");
        }
        let mut state = plain.clone();
        state.diplomacy.enabled = true;
        assert!(crate::dual_window::round_opening_unported(&state, content, &args(&a, &d, &system)), "diplomacy");
        // and through the call site: the window counts the bail-out and nothing is compared
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = plain.clone();
        state.rules_version = 7;
        state.player_mut(&d).unwrap().faction = ti4_model::id::FactionId::new("letnev");
        put(&mut state, &system, "cruiser", &a, 1);
        put(&mut state, &system, "cruiser", &d, 1);
        with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            let (mut dice, mut rng, mut table) = (Dice::new(), GameRng::new(1), Table::with_default(Box::new(crate::choice::FirstOption)));
            let mut ctx = Resolving { content, sources: ti4_model::content_types::DEFAULT, dice: &mut dice, rng: &mut rng, table: &mut table, timing: None };
            let _ = resolve_resolving(&mut state, &mut ctx, &system, None);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let counts = r.windows.get("combat::begin_round").cloned().unwrap_or_default();
        assert!(counts.bailed > 0 && counts.compared == 0, "{counts:?}");
        let _ = take_report();
    }

    /// Package W2: a readied The Thundarian is an ability of the core table: the roll step emits the step-ended event, offers the card to
    /// its holder and, when it is used, plays the step again (the dice thrown away, the card exhausted, one replay per card). The roll
    /// window, the composed settle and the replays agree with the string. Fails on the pre-W2 gate (`roll_why` named the card).
    #[test]
    fn a_readied_thundarian_replays_the_roll_step_in_the_core_window_in_lockstep() {
        assert!(!roll_unported_after(&|_, _| {}));
        assert!(!roll_unported_after(&|state, _| {
            state.player_mut(&attacker()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nomadagentthundarian"), ti4_model::state::LeaderStatus::Readied);
        }));
        fn answer(seed: u64, nth: usize, c: &Choice) -> usize {
            let at = c.options.iter().position(|o| o.id.contains("nomadagentthundarian"));
            match at {
                Some(at) if (seed + nth as u64) % 3 != 1 => at,
                Some(_) => c.options.len() - 1,
                None => 0,
            }
        }
        let (mut used, mut compared) = (0, 0);
        for (seed, composed) in (0..10_u64).flat_map(|s| [(s, false), (s, true)]) {
            let (state, windows, differences, _) = hub_fight_composed(
                composed,
                seed,
                &["nomad", "hacan"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "destroyer", &defender(), 2);
                    put(s, system, "cruiser", &defender(), 2);
                    s.player_mut(&attacker()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nomadagentthundarian"), ti4_model::state::LeaderStatus::Readied);
                },
                answer,
            );
            assert_eq!(differences, 0, "seed {seed} composed {composed}: {windows:?}");
            assert!(windows.keys().all(|k| !k.contains("Thundarian")), "seed {seed}: {windows:?}");
            let label = if composed { "combat::settle" } else { "combat::roll_round" };
            let w = windows.get(label).copied().unwrap_or_default();
            assert_eq!(w.bailed, 0, "seed {seed} composed {composed}: {windows:?}");
            compared += w.compared;
            used += usize::from(state.player(&attacker()).unwrap().leaders.get(ti4_model::id::text(&"nomadagentthundarian")) == Some(&ti4_model::state::LeaderStatus::Exhausted));
        }
        assert!(compared > 0, "the roll ran in the core: {compared}");
        assert!(used > 0, "the card was used and the step replayed");
    }

    /// A readied Ssruu next to a Nomad's card: the step-ended event is emitted (a Yssaril copy of the card could hear it) and the string
    /// registers no copy of a Nomad agent, so the core roll window agrees with it (no bail, no difference).
    #[test]
    fn a_yssaril_ssruu_next_to_a_thundarian_runs_the_roll_window_in_lockstep() {
        fn any_offer(_: u64, _: usize, c: &Choice) -> usize {
            c.options.iter().position(|o| o.id.contains("ssruu-copy") || o.id.contains("nomadagentthundarian")).unwrap_or(0)
        }
        let mut compared = 0;
        for seed in 0..6_u64 {
            let (_, windows, differences, _) = hub_fight_composed(
                false,
                seed,
                &["yssaril", "nomad"],
                |s, system| {
                    put(s, system, "cruiser", &attacker(), 3);
                    put(s, system, "cruiser", &defender(), 3);
                    s.player_mut(&attacker()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("yssarilagent"), ti4_model::state::LeaderStatus::Readied);
                    s.player_mut(&defender()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("nomadagentthundarian"), ti4_model::state::LeaderStatus::Readied);
                },
                any_offer,
            );
            assert_eq!(differences, 0, "seed {seed}: {windows:?}");
            let w = windows.get("combat::roll_round").copied().unwrap_or_default();
            assert_eq!(w.bailed, 0, "seed {seed}: {windows:?}");
            compared += w.compared;
        }
        assert!(compared > 0);
    }

    #[test]
    fn a_keleres_agent_that_could_pay_keeps_the_roll_step_in_the_string() {
        // A Hacan flagship (Wrath of Kenara) whose owner holds commodities a readied Keleres agent could turn
        // into trade goods: the payment window is the string's.
        let keleres = |state: &mut GameState, ready: bool| {
            let seat = state.player_mut(&attacker()).unwrap();
            seat.commodities = 2;
            seat.trade_goods = 0;
            state.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("keleresx");
            if ready {
                state.player_mut(&defender()).unwrap().leaders.insert(
                    ti4_model::id::LeaderId::new("keleresagent"),
                    ti4_model::state::LeaderStatus::Readied,
                );
            }
        };
        assert!(!roll_unported_after(&|state, system| {
            put(state, system, "hacan_flagship", &attacker(), 1);
            keleres(state, false);
        }));
        assert!(roll_unported_after(&|state, system| {
            put(state, system, "hacan_flagship", &attacker(), 1);
            keleres(state, true);
        }));
    }

    #[test]
    fn the_crowns_of_thalnos_reroll_questions_run_in_the_core_roll_window() {
        let asked = roll_step_lockstep(
            &|state, seed| {
                // The relic with the attacker, the law elected onto the defender (and both on a few seeds).
                if seed % 3 != 1 {
                    state.player_mut(&attacker()).unwrap().relics.push(ti4_model::id::RelicId::new("thalnos"));
                }
                if seed % 3 != 0 {
                    state.laws.insert("crown_of_thalnos".to_owned(), "b".to_owned());
                }
            },
            24,
        );
        let per_die = |source: &str| asked.iter().filter(|c| c.context.as_ref().is_some_and(|x| x.subtype == source)).count();
        assert!(per_die("crown_of_thalnos_relic_reroll") > 10, "the relic asked about dice");
        assert!(per_die("crown_of_thalnos_law_reroll") > 10, "the law asked about dice");
    }

    #[test]
    fn a_roll_that_ends_the_fight_stays_string() {
        // A reroll of the Crown of Thalnos destroys the lone ship of a fleet whose die missed again: the fight is
        // over inside the roll step, which the core window does not follow. The string result stands (counted as a
        // bail-out, nothing compared) and the other seeds still run in the window.
        let (_, compared, bailed) = roll_step_counts(
            &|state, _| {
                let system = SystemId::new("18");
                state.system_mut(&system).units.clear();
                put(state, &system, "cruiser", &attacker(), 1);
                put(state, &system, "cruiser", &defender(), 1);
                state.player_mut(&attacker()).unwrap().relics.push(ti4_model::id::RelicId::new("thalnos"));
                state.player_mut(&defender()).unwrap().relics.push(ti4_model::id::RelicId::new("thalnos"));
            },
            0,
        );
        assert!(bailed > 0, "some roll ended the fight: compared {compared}, bailed {bailed}");
        assert!(compared > 0, "the others ran in the window");
    }

    #[test]
    fn the_heart_of_ixth_runs_in_the_core_roll_window() {
        let asked = roll_step_lockstep(
            &|state, seed| {
                for (who, bit) in [("a", 1), ("b", 2), ("c", 4)] {
                    if (seed + 1) & bit != 0 || seed % 5 == 0 {
                        state.player_mut(&PlayerId::new(who)).unwrap().relics.push(ti4_model::id::RelicId::new("heartofixth"));
                    }
                }
            },
            24,
        );
        let heart = asked.iter().filter(|c| c.context.as_ref().is_some_and(|x| x.subtype == "heart_ixth_die_adjust")).count();
        assert!(heart > 10, "the holders were asked: {heart}");
    }

    #[test]
    fn war_funding_runs_in_the_core_roll_window() {
        let asked = roll_step_lockstep(
            &|state, seed| {
                state.player_mut(&defender()).unwrap().faction = ti4_model::id::FactionId::new("letnev");
                state.player_mut(&PlayerId::new("c")).unwrap().faction = ti4_model::id::FactionId::new("hacan");
                let (holder, owner) = if seed % 2 == 0 { (attacker(), "letnev") } else { (PlayerId::new("c"), "letnev") };
                state.promissory_notes.insert(crate::promissory::note_id("war_funding", owner), holder);
            },
            24,
        );
        let offers = asked.iter().filter(|c| c.prompt.starts_with("War Funding")).count();
        assert!(offers > 5, "War Funding was offered: {offers}");
    }

    #[test]
    fn wrath_of_kenara_runs_in_the_core_roll_window() {
        let asked = roll_step_lockstep(
            &|state, seed| {
                let system = SystemId::new("18");
                put(state, &system, "hacan_flagship", &attacker(), 1);
                put(state, &system, "hacan_flagship", &defender(), (seed % 2) as usize);
            },
            24,
        );
        let offers = asked.iter().filter(|c| c.prompt.starts_with("Wrath of Kenara")).count();
        assert!(offers > 5, "Wrath of Kenara was offered: {offers}");
    }

    #[test]
    fn munitions_after_the_roll_runs_in_the_core_roll_window() {
        let asked = roll_step_lockstep(
            &|state, _| {
                state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("letnev");
                state.rules_version = ti4_model::state::RULES_MUNITIONS_AFTER_ROLL.max(state.rules_version);
            },
            24,
        );
        let offers = asked.iter().filter(|c| c.options.iter().any(|o| o.id == "munitions")).count();
        assert!(offers > 5, "Munitions Reserves was offered after the roll: {offers}");
    }

    #[test]
    fn raid_formation_runs_as_a_core_window_in_lockstep() {
        // Argent's barrage hits beyond the fighters damage the target's ships: one candidate type without a
        // question, two with the producer's choice; the core window asks and answers the same.
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut damaged = 0;
        for seed in 0..12_u64 {
            let mut state = crate::fixtures::seated_game(&[("a", "argent"), ("b", "hacan")], ti4_model::content_types::DEFAULT);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed;
            let system = SystemId::new("18");
            put(&mut state, &system, &a_barrage_unit(), &attacker(), 6);
            put(&mut state, &system, "fighter", &defender(), 1);
            put(&mut state, &system, "dreadnought", &defender(), 1);
            // As `Game` runs a fight: the question of the ability is asked in the battle (`about_battle`).
            state.active_space_combat = Some((system.clone(), attacker(), defender()));
            if seed % 2 == 0 {
                put(&mut state, &system, "warsun", &defender(), 1);
            }
            with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
                let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
                let (mut dice, mut rng) = (Dice::new(), GameRng::new(seed));
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut ctx = Resolving {
                    content: ContentStore::embedded(),
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
                };
                resolve_resolving(&mut state, &mut ctx, &system, None).unwrap();
            });
            damaged += state.system_state(&system).units.iter().filter(|u| u.sustained_damage).count().min(1);
        }
        let r = report();
        assert!(damaged > 0, "Raid Formation damaged something");
        // Package W2: the barrage STEP carries an Argent side (Raid Formation is a child of the window), so the whole step is compared; the
        // pre-W2 code gated it ("an Argent side") and compared the ability's window on its own.
        let w = r.windows.get("combat::barrage_step").copied().unwrap_or_default();
        assert!(w.compared >= 12 && w.bailed == 0, "{:?}", r.windows);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    #[test]
    fn assault_cannon_choice_says_it_is_a_destroy_of_a_non_fighter_ship() {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &attacker(), 3);
        put(&mut state, &system, "destroyer", &defender(), 1);
        put(&mut state, &system, "dreadnought", &defender(), 1);
        put(&mut state, &system, "fighter", &defender(), 2);
        state
            .player_mut(&attacker())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("asc"));
        let (decider, seen) = crate::choice::Capturing::new(Box::new(crate::choice::FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(3);
        resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();
        let asked = seen.borrow();
        let cannon = asked
            .iter()
            .filter_map(|choice| choice.context.as_ref())
            .find(|context| context.subtype == "assault_cannon_destroy")
            .expect("the cannon asked the victim which ship goes");
        let hit = cannon.hit.as_ref().expect("the kind of hit is stated");
        assert_eq!(hit.cause, HitCause::AssaultCannon);
        assert!(
            hit.destroy,
            "a destroy, not a hit that sustain could cancel"
        );
        assert_eq!(hit.restriction, HitRestriction::NonFighter);
        assert_eq!(hit.producer, Some(attacker()));
        assert_eq!(cannon.actor, defender(), "the victim chooses the ship");
        // Display only: the replay fingerprint and recorded decisions do not carry it.
        assert!(cannon.without_display_fields().hit.is_none());
        assert!(!cannon.canonical().contains("AssaultCannon"));
    }

    #[test]
    fn wrath_of_kenara_buys_a_hit_for_a_trade_good() {
        let (mut state, system) = arena();
        put(&mut state, &system, "hacan_flagship", &attacker(), 1);
        state.player_mut(&attacker()).unwrap().trade_goods = 1;
        // The flagship rolls 2 dice hitting on 7: two 6s are two near misses, and one trade good
        // buys one of them.
        let mut table = Table::with_default(Box::new(crate::choice::Scripted::new([
            "kenara|1".to_owned()
        ])));
        let mut dice = Dice::from_faces([6, 6]);
        let mut rng = GameRng::new(7);
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        let hits = roll_fleet_and_open(&mut state, &mut ctx, &attacker(), &system);
        assert_eq!(hits, 1);
        assert_eq!(state.player(&attacker()).unwrap().trade_goods, 0);

        // Without the flagship nothing is offered, and the misses stay misses.
        let (mut plain, system) = arena();
        put(&mut plain, &system, "dreadnought", &attacker(), 2);
        plain.player_mut(&attacker()).unwrap().trade_goods = 3;
        let mut table =
            Table::with_default(Box::new(crate::choice::Scripted::new(Vec::<String>::new())));
        let mut dice = Dice::from_faces([4, 4]);
        let mut ctx = Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        assert_eq!(
            roll_fleet_and_open(&mut plain, &mut ctx, &attacker(), &system),
            0
        );
        assert_eq!(plain.player(&attacker()).unwrap().trade_goods, 3);
    }

    /// A Letnev attacker with three cruisers (hit on 7) rolling `[1, 1, 10]`, `answers` scripted,
    /// run through the fleet roll and its windows. Returns the state, the hits, the dice and every
    /// question the Letnev seat was asked.
    fn munitions_fleet_roll(
        rules_version: u32,
        trade_goods: i32,
        faces: &[u32],
        answers: &[&str],
    ) -> (GameState, usize, Dice, Vec<crate::choice::Choice>) {
        let (mut state, system) = arena();
        state.rules_version = rules_version;
        state.player_mut(&attacker()).unwrap().faction = ti4_model::id::FactionId::new("letnev");
        state.player_mut(&attacker()).unwrap().trade_goods = trade_goods;
        put(&mut state, &system, "cruiser", &attacker(), faces.len());
        let (decider, seen) =
            crate::choice::Capturing::new(Box::new(crate::choice::Scripted::with_fallback(
                answers.iter().map(|id| (*id).to_owned()),
                Box::new(crate::choice::AlwaysDecline),
            )));
        let mut table = Table::with_default(Box::new(decider));
        let mut dice = Dice::from_faces(faces.iter().copied());
        let mut rng = GameRng::new(7);
        let hits = {
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: None,
            };
            roll_fleet_and_open(&mut state, &mut ctx, &attacker(), &system)
        };
        let asked = seen.borrow().clone();
        (state, hits, dice, asked)
    }

    #[test]
    fn munitions_after_roll_is_asked_with_the_dice_in_view_and_pays_then() {
        let (state, _, dice, asked) = munitions_fleet_roll(
            ti4_model::state::RULES_MUNITIONS_AFTER_ROLL,
            4,
            &[1, 1, 10],
            &["munitions"],
        );
        let offer = asked
            .iter()
            .find(|choice| choice.option("munitions").is_some())
            .expect("the re-roll is offered once the fleet has rolled");
        // The three faces are already on the table when the question is asked.
        let faces: Vec<i64> = offer.details["dice"]
            .as_array()
            .expect("the roll is shown")
            .iter()
            .map(|die| die["face"].as_i64().unwrap())
            .collect();
        assert_eq!(faces, vec![1, 1, 10]);
        assert_eq!(
            state.player(&attacker()).unwrap().trade_goods,
            2,
            "2 trade goods paid"
        );
        assert_eq!(
            dice.history()
                .iter()
                .filter(|d| d.reason == "munitions:a")
                .count(),
            2,
            "both misses were drawn again, the hit was kept"
        );
    }

    #[test]
    fn munitions_after_roll_can_be_declined_for_free() {
        let (state, hits, dice, _) = munitions_fleet_roll(
            ti4_model::state::RULES_MUNITIONS_AFTER_ROLL,
            4,
            &[1, 1, 10],
            &["decline"],
        );
        assert_eq!(state.player(&attacker()).unwrap().trade_goods, 4);
        assert_eq!(hits, 1);
        assert!(dice.history().iter().all(|d| d.reason != "munitions:a"));
    }

    #[test]
    fn munitions_after_roll_can_re_roll_chosen_dice_only() {
        let (state, _, dice, asked) = munitions_fleet_roll(
            ti4_model::state::RULES_MUNITIONS_AFTER_ROLL,
            4,
            &[1, 1, 10],
            &["munitions_choose", "decline", "reroll|0:1"],
        );
        assert_eq!(state.player(&attacker()).unwrap().trade_goods, 2);
        assert_eq!(
            dice.history()
                .iter()
                .filter(|d| d.reason == "munitions:a")
                .count(),
            1
        );
        // Every die is asked about, in order, as with the Crown of Thalnos.
        let per_die = asked
            .iter()
            .filter(|choice| choice.options.iter().any(|o| o.kind == "reroll_die"))
            .count();
        assert_eq!(per_die, 3);
    }

    #[test]
    fn munitions_after_roll_is_not_asked_without_a_miss_or_the_goods() {
        let (_, _, _, asked) = munitions_fleet_roll(
            ti4_model::state::RULES_MUNITIONS_AFTER_ROLL,
            4,
            &[10, 9, 8],
            &[],
        );
        assert!(
            asked.iter().all(|c| c.option("munitions").is_none()),
            "all hit"
        );
        let (_, _, _, asked) = munitions_fleet_roll(
            ti4_model::state::RULES_MUNITIONS_AFTER_ROLL,
            1,
            &[1, 1],
            &[],
        );
        assert!(
            asked.iter().all(|c| c.option("munitions").is_none()),
            "cannot pay"
        );
    }

    #[test]
    fn a_legacy_game_never_asks_after_the_roll() {
        let (state, _, _, asked) = munitions_fleet_roll(
            ti4_model::state::RULES_INFILTRATE_FIX,
            4,
            &[1, 1, 10],
            &["munitions"],
        );
        assert!(
            asked.is_empty(),
            "legacy asks at the round start, not in the fleet window"
        );
        assert_eq!(state.player(&attacker()).unwrap().trade_goods, 4);
    }

    #[test]
    fn munitions_rerolls_only_misses_and_lapses_after_its_round() {
        let (mut state, _) = arena();
        state.combat_round_seq = 9;
        state.player_mut(&attacker()).unwrap().munitions_round = Some(9);
        let mut dice = Dice::new();
        let mut rng = GameRng::new(3);
        let original = crate::dice::Roll {
            reason: "space combat".to_owned(),
            faces: vec![1, 10],
            hits_on: Some(7),
            rerolled: std::collections::BTreeSet::new(),
            by: None,
            system: None,
            outcome: None,
            source: None,
        };
        let rerolled = reroll_munitions_misses(&state, &mut dice, &mut rng, &attacker(), &original);

        assert_eq!(rerolled.rerolled, std::collections::BTreeSet::from([0]));
        assert_eq!(rerolled.faces[1], 10, "a hit is not rerolled");
        assert_eq!(
            dice.count(),
            1,
            "the replacement is recorded; callers already recorded the original batch"
        );
        assert_eq!(dice.history()[0].reason, "munitions:a");

        state.combat_round_seq = 10;
        let before = dice.count();
        assert_eq!(
            reroll_munitions_misses(&state, &mut dice, &mut rng, &attacker(), &original),
            original,
            "a marker from an earlier round does nothing"
        );
        assert_eq!(dice.count(), before);
    }

    #[test]
    fn space_cannon_fires_from_a_planet_at_the_active_player() {
        // A PDS sits on a planet and shoots into space. The engine had no such step at all
        // before this, so a PDS never once fired on a ship moving into its system.
        let (mut state, system) = arena();
        let planet = ti4_model::id::PlanetId::new("mecatol_rex");
        state
            .system_mut(&system)
            .planet_units
            .entry(planet)
            .or_default()
            .push(Unit::new(UnitTypeId::new(a_cannon_unit()), defender()));
        put(&mut state, &system, "cruiser", &attacker(), 1);
        let (_, mut dice, mut rng) = kit();

        let fired = space_cannon_offense(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            None,
        );

        assert_eq!(dice.count(), 1, "the gun on the planet fired");
        assert!(
            fired.iter().all(|(owner, _, _)| owner == &defender()),
            "only the non-active player shoots"
        );
    }

    #[test]
    fn a_space_cannon_roll_says_which_unit_and_planet_fired() {
        let (mut state, system) = arena();
        let planet = ti4_model::id::PlanetId::new("mecatol_rex");
        state
            .system_mut(&system)
            .planet_units
            .entry(planet)
            .or_default()
            .push(Unit::new(UnitTypeId::new(a_cannon_unit()), defender()));
        put(&mut state, &system, "cruiser", &attacker(), 1);
        let (_, mut dice, mut rng) = kit();

        let _ = space_cannon_offense(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            None,
        );

        let roll = dice.history().last().expect("one roll").clone();
        let source = roll.source.expect("the roll names its gun");
        assert_eq!(source.unit, a_cannon_unit());
        assert_eq!(source.planet.as_deref(), Some("mecatol_rex"));
        assert_eq!(source.system, None, "the gun stands in the system itself");
    }

    #[test]
    fn a_module_bar_silences_space_cannon_against_the_named_owner_only() {
        use crate::factions::hooks_combat::{CombatHooks, with_test_hooks};
        let bar = CombatHooks {
            space_cannon_barred: Some(|_, _, _, shooter, target, system| {
                shooter.as_str() == "b" && target.as_str() == "a" && system.as_str() == "18"
            }),
            ..CombatHooks::NONE
        };
        let planet = ti4_model::id::PlanetId::new("mecatol_rex");
        let fire = |barred: bool| {
            let (mut state, system) = arena();
            state
                .system_mut(&system)
                .planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new(a_cannon_unit()), defender()));
            put(&mut state, &system, "cruiser", &attacker(), 1);
            let (_, mut dice, mut rng) = kit();
            let run = |state: &mut GameState, dice: &mut Dice, rng: &mut GameRng| {
                space_cannon_offense(
                    state,
                    ContentStore::embedded(),
                    POK,
                    dice,
                    rng,
                    &system,
                    &attacker(),
                    None,
                )
            };
            if barred {
                with_test_hooks(bar, || run(&mut state, &mut dice, &mut rng));
            } else {
                run(&mut state, &mut dice, &mut rng);
            }
            dice.count()
        };
        assert_eq!(fire(false), 1, "neutral without the hook");
        assert_eq!(fire(true), 0, "the bar stops the gun");
    }

    #[test]
    fn a_module_bar_also_stops_adjacent_reaching_guns() {
        use crate::factions::hooks_combat::{CombatHooks, with_test_hooks};
        let hub = crate::fixtures::plain_hub();
        let active = SystemId::new(&hub.centre);
        let next_door = SystemId::new(&hub.outer[0]);
        let planet = ti4_model::id::PlanetId::new("a_planet_next_door");
        let fire = |barred: bool| {
            let mut state = crate::fixtures::game(&["a", "b"]);
            for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
                state.board.entry(SystemId::new(id)).or_default();
            }
            put(&mut state, &active, "cruiser", &attacker(), 1);
            state
                .board
                .get_mut(&next_door)
                .expect("system")
                .planet_units
                .entry(planet.clone())
                .or_default()
                .push(Unit::new(UnitTypeId::new("pds2"), defender()));
            let (_, mut dice, mut rng) = kit();
            let bar = CombatHooks {
                space_cannon_barred: Some(|_, _, _, _, target, _| target.as_str() == "a"),
                ..CombatHooks::NONE
            };
            let run = |state: &mut GameState, dice: &mut Dice, rng: &mut GameRng| {
                space_cannon_offense(
                    state,
                    ContentStore::embedded(),
                    POK,
                    dice,
                    rng,
                    &active,
                    &attacker(),
                    Some(&hub.galaxy),
                )
            };
            if barred {
                with_test_hooks(bar, || run(&mut state, &mut dice, &mut rng));
            } else {
                run(&mut state, &mut dice, &mut rng);
            }
            dice.count()
        };
        assert_eq!(fire(false), 1, "the PDS II next door fires");
        assert_eq!(fire(true), 0, "and is barred by the hook");
    }

    #[test]
    fn the_active_players_own_guns_do_not_fire_at_them() {
        let (mut state, system) = arena();
        put(&mut state, &system, &a_cannon_unit(), &attacker(), 3);
        let (_, mut dice, mut rng) = kit();

        let fired = space_cannon_offense(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            None,
        );

        assert!(fired.is_empty());
        assert_eq!(dice.count(), 0, "nobody shoots at themselves");
    }

    #[test]
    fn the_active_players_guns_fire_at_the_ships_it_attacks() {
        // User ruling 2026-09-17: space cannon offense is not only for the defender. An Xxcha
        // flagship moving in fires at the defender's ships before combat.
        let (mut state, system) = arena();
        put(&mut state, &system, "xxcha_flagship", &attacker(), 1);
        put(&mut state, &system, "cruiser", &defender(), 2);
        let (_, mut dice, mut rng) = kit();

        let _ = space_cannon_offense(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &system,
            &attacker(),
            None,
        );

        assert_eq!(
            dice.count(),
            1,
            "the attacker's flagship rolled its space cannon"
        );
        assert!(
            state.reroll_staging.contains_key(&attacker()),
            "the attacker's roll is staged for its reroll window"
        );
        assert_eq!(
            opponent_with_ships(&state, ContentStore::embedded(), POK, &attacker(), &system),
            Some(defender()),
            "and its hits go to the defender"
        );
    }

    #[test]
    fn hylarim_turns_each_nine_or_ten_into_two_extra_hits() {
        let (mut state, system) = arena();
        put(&mut state, &system, "jolnar_flagship", &attacker(), 1);
        let set = RerollSet {
            kind: "fleet".into(),
            system: system.clone(),
            rolls: vec![RerollEntry {
                unit: "jolnar_flagship".into(),
                planet: None,
                hits_on: Some(7),
                faces: vec![9, 3],
                rerolled: std::collections::BTreeSet::new(),
                deltas: std::collections::BTreeMap::new(),
                unit_types: std::iter::once(("jolnar_flagship".to_owned(), 1)).collect(),
            }],
        };
        // One ordinary hit from the 9, and two more because it was a 9.
        assert_eq!(
            fleet_hits(
                &state,
                ContentStore::embedded(),
                POK,
                &attacker(),
                &system,
                &set
            ),
            (3, 0)
        );
    }

    #[test]
    fn zero_zero_one_forces_its_fleets_hits_onto_non_fighters() {
        let (mut state, system) = arena();
        put(&mut state, &system, "l1z1x_flagship", &attacker(), 1);
        put(&mut state, &system, "l1z1x_dreadnought", &attacker(), 1);
        put(&mut state, &system, "carrier", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 3);
        put(&mut state, &system, "cruiser", &defender(), 1);
        let entry = |unit: &str, hits_on: u32| RerollEntry {
            unit: unit.into(),
            planet: None,
            hits_on: Some(hits_on),
            faces: vec![10],
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: std::iter::once((unit.to_owned(), 1)).collect(),
        };
        let set = RerollSet {
            kind: "fleet".into(),
            system: system.clone(),
            rolls: vec![entry("l1z1x_dreadnought", 5), entry("carrier", 9)],
        };
        let content = ContentStore::embedded();
        assert_eq!(
            fleet_hits(&state, content, POK, &attacker(), &system, &set),
            (1, 1),
            "the dreadnought's hit is forced, the carrier's is not"
        );

        let window = CombatWindow::new(&state, content, POK, &system);
        let forced = Pending {
            player: defender(),
            hits: 1,
            producer: attacker(),
            non_fighters_only: true,
            producer_assigns: false,
        };
        let candidates = window.casualty_candidates(&state, content, POK, &forced);
        assert_eq!(candidates.len(), 1, "only the cruiser may take it");
        assert_eq!(candidates[0].1.type_id.as_str(), "cruiser");

        // "If able": with no non-fighter left, the hit falls on a fighter as usual.
        state
            .system_mut(&system)
            .units
            .retain(|unit| unit.type_id.as_str() != "cruiser");
        let candidates = window.casualty_candidates(&state, content, POK, &forced);
        assert_eq!(candidates.len(), 3);
    }

    #[test]
    fn arc_secundus_repairs_itself_and_nothing_else() {
        let (mut state, system) = arena();
        for kind in ["letnev_flagship", "dreadnought"] {
            state
                .system_mut(&system)
                .units
                .push(Unit::new(UnitTypeId::new(kind), attacker()).sustained());
        }
        repair_self_repairing(&mut state, &system, &attacker());
        let damage: Vec<(String, bool)> = state
            .system_state(&system)
            .units
            .iter()
            .filter(|unit| unit.owner == attacker())
            .map(|unit| (unit.type_id.to_string(), unit.sustained_damage))
            .collect();
        assert!(damage.contains(&("letnev_flagship".to_owned(), false)));
        assert!(damage.contains(&("dreadnought".to_owned(), true)));
    }

    #[test]
    fn a_retreat_cannot_enter_an_asteroid_field_without_antimass_deflectors() {
        let asteroid = crate::fixtures::a_system_where("asteroid field");
        let hub = crate::fixtures::hub_with_outer(&asteroid);
        let mut state = crate::fixtures::game(&["a", "b"]);
        let centre = SystemId::new(hub.centre.clone());
        let field = SystemId::new(asteroid.clone());
        let me = attacker();
        put(&mut state, &centre, "cruiser", &me, 1);
        put(&mut state, &field, "carrier", &me, 1);
        let options = |state: &GameState| {
            eligible_retreats(
                state,
                ContentStore::embedded(),
                POK,
                &hub.galaxy,
                &me,
                &centre,
            )
        };
        assert!(!options(&state).contains(&field), "no technology");

        state.rules_version = ti4_model::state::RULES_RETREAT_HAZARDS - 1;
        assert!(
            options(&state).contains(&field),
            "legacy games keep the old set"
        );

        state.rules_version = ti4_model::state::RULES_VERSION;
        state
            .player_mut(&me)
            .expect("seat")
            .technologies
            .insert(ti4_model::TechnologyId::new("amd"));
        assert!(options(&state).contains(&field), "with Antimass Deflectors");
    }

    #[test]
    fn a_retreat_needs_somewhere_that_is_yours_and_unthreatened() {
        // 78.7c: adjacent, holds your units or a planet you control, and no enemy ships.
        let hub = crate::fixtures::plain_hub();
        let mut state = crate::fixtures::game(&["a", "b"]);
        let centre = SystemId::new(hub.centre.clone());
        let refuge = SystemId::new(hub.outer[0].clone());

        put(&mut state, &centre, "cruiser", &attacker(), 1);
        assert!(
            eligible_retreats(
                &state,
                ContentStore::embedded(),
                POK,
                &hub.galaxy,
                &attacker(),
                &centre
            )
            .is_empty(),
            "an empty neighbour is not a refuge"
        );

        put(&mut state, &refuge, "carrier", &attacker(), 1);
        assert!(
            eligible_retreats(
                &state,
                ContentStore::embedded(),
                POK,
                &hub.galaxy,
                &attacker(),
                &centre
            )
            .contains(&refuge),
            "your own fleet makes it one"
        );

        put(&mut state, &refuge, "destroyer", &defender(), 1);
        assert!(
            !eligible_retreats(
                &state,
                ContentStore::embedded(),
                POK,
                &hub.galaxy,
                &attacker(),
                &centre
            )
            .contains(&refuge),
            "an enemy ship there closes it again"
        );
    }

    #[test]
    fn a_retreat_strands_what_it_cannot_carry() {
        // 78.7b: only ships with a move value leave under their own power, and capacity
        // decides how much of the rest goes with them. The remainder is lost, which is the
        // cost of retreating rather than an oversight.
        let hub = crate::fixtures::plain_hub();
        let mut state = crate::fixtures::game(&["a", "b"]);
        let centre = SystemId::new(hub.centre.clone());
        let refuge = SystemId::new(hub.outer[0].clone());

        put(&mut state, &centre, "destroyer", &attacker(), 1); // no capacity
        put(&mut state, &centre, "fighter", &attacker(), 3);

        let stranded = retreat_to(
            &mut state,
            ContentStore::embedded(),
            POK,
            &attacker(),
            &centre,
            &refuge,
        );

        assert_eq!(stranded, 3, "a destroyer carries nothing");
        assert!(state.system_state(&centre).units.is_empty());
        assert_eq!(state.system_state(&refuge).units.len(), 1, "only the hull");
        assert!(
            state
                .system_state(&refuge)
                .command_tokens
                .contains(&attacker()),
            "78.7d: a token goes to the destination"
        );
    }

    #[test]
    fn a_retreating_ship_form_mech_lands_in_its_ground_form() {
        // Z-Grav Eidolon is a ship only in the active system during its battle; carried away by a
        // retreat it must not stay a ship in the system it retreats to.
        let hub = crate::fixtures::plain_hub();
        let mut state = crate::fixtures::game(&["a", "b"]);
        let centre = SystemId::new(hub.centre.clone());
        let refuge = SystemId::new(hub.outer[0].clone());
        put(&mut state, &centre, "carrier", &attacker(), 1);
        put(&mut state, &centre, "naaz_mech_space", &attacker(), 1);

        let stranded = retreat_to(
            &mut state,
            ContentStore::embedded(),
            ti4_model::content_types::DEFAULT,
            &attacker(),
            &centre,
            &refuge,
        );

        assert_eq!(stranded, 0);
        let landed: Vec<String> = state
            .system_state(&refuge)
            .units
            .iter()
            .map(|unit| unit.type_id.to_string())
            .collect();
        assert!(
            landed.iter().any(|id| id == "naaz_mech"),
            "flipped back: {landed:?}"
        );
        assert!(
            landed.iter().all(|id| id != "naaz_mech_space"),
            "{landed:?}"
        );
    }

    /// `retreat_to` on `driver=dual` for the given fleets: the label's (compared, bailed) counts.
    fn retreat_lockstep(factions: &[(&str, &str)], fleet: &dyn Fn(&mut GameState, &SystemId), setup: &dyn Fn(&mut GameState)) -> (u64, u64, usize) {
        use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = crate::fixtures::seated_game(factions, ti4_model::content_types::DEFAULT);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        let (centre, refuge) = (SystemId::new("18"), SystemId::new("19"));
        fleet(&mut state, &centre);
        setup(&mut state);
        let lost = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
            retreat_to(&mut state, ContentStore::embedded(), ti4_model::content_types::DEFAULT, &attacker(), &centre, &refuge)
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let counts = r.windows.get("combat::retreat_to").cloned().unwrap_or_default();
        let _ = take_report();
        (counts.compared, counts.bailed, lost)
    }

    #[test]
    fn a_retreat_runs_in_the_core_window() {
        // Carrier with its fighters and a mech (capacity enough), a sustained dreadnought, a galvanized
        // cruiser, and a destroyer fleet that strands its fighters; with and without staging (Argent is a
        // module faction).
        for factions in [[("a", "sol"), ("b", "hacan")], [("a", "argent"), ("b", "hacan")]] {
            for stranding in [false, true] {
                let (compared, bailed, lost) = retreat_lockstep(
                    &factions,
                    &|state, centre| {
                        put(state, centre, "carrier", &attacker(), 1);
                        put(state, centre, "dreadnought", &attacker(), 1);
                        put(state, centre, "cruiser", &attacker(), 1);
                        put(state, centre, "fighter", &attacker(), if stranding { 6 } else { 3 });
                        put(state, centre, "mech", &attacker(), 1);
                        put(state, centre, "cruiser", &defender(), 1);
                        let units = &mut state.system_mut(centre).units;
                        let at = units.iter().position(|u| u.type_id.as_str() == "dreadnought").unwrap();
                        units[at].sustained_damage = true;
                        let at = units.iter().position(|u| u.type_id.as_str() == "cruiser" && u.owner == attacker()).unwrap();
                        units[at].galvanized = true;
                    },
                    &|_| {},
                );
                assert_eq!((compared, bailed), (1, 0), "{factions:?} stranding={stranding}");
                assert_eq!(lost > 0, stranding);
            }
        }
    }

    #[test]
    fn a_retreat_in_a_game_with_the_last_bastion_stays_string() {
        // `bastion_note_retreat` is not in the core: the note of the retreat is the string's.
        let (compared, bailed, _) = retreat_lockstep(
            &[("a", "sol"), ("b", "bastion")],
            &|state, centre| put(state, centre, "carrier", &attacker(), 1),
            &|_| {},
        );
        assert_eq!((compared, bailed), (0, 1));
    }

    #[test]
    fn a_retreat_of_a_dual_form_or_loose_unit_runs_in_the_core_window() {
        // Package W3: a ship-form Eidolon (flips back where it lands) and a Floating Factory (moves like a ship without
        // being one) retreat in the core window, compared with the string.
        for unit in ["naaz_mech_space", "saar_spacedock"] {
            let (compared, bailed, _) = retreat_lockstep(
                &[("a", "sol"), ("b", "hacan")],
                &|state, centre| {
                    put(state, centre, "carrier", &attacker(), 1);
                    put(state, centre, unit, &attacker(), 1);
                },
                &|_| {},
            );
            assert_eq!((compared, bailed), (1, 0), "{unit}");
        }
    }

    #[test]
    fn a_carrier_takes_its_fighters_with_it() {
        let hub = crate::fixtures::plain_hub();
        let mut state = crate::fixtures::game(&["a", "b"]);
        let centre = SystemId::new(hub.centre.clone());
        let refuge = SystemId::new(hub.outer[0].clone());

        put(&mut state, &centre, "carrier", &attacker(), 1);
        put(&mut state, &centre, "fighter", &attacker(), 2);

        let stranded = retreat_to(
            &mut state,
            ContentStore::embedded(),
            POK,
            &attacker(),
            &centre,
            &refuge,
        );

        assert_eq!(stranded, 0);
        assert_eq!(state.system_state(&refuge).units.len(), 3);
    }

    #[test]
    fn a_declared_draw_beats_counting_the_survivors() {
        // Skilled Retreat: the fleet leaves and the combat ends in a draw. Counting ships would
        // hand the win to whoever stayed, and would also score "win a space combat" for a fight
        // nobody won.
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 2);
        let (mut table, mut dice, mut rng) = kit();
        state.combat_draw_round = Some(state.combat_round_seq);

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();

        assert_eq!(
            outcome.winner, None,
            "the round was declared a draw, so the last fleet standing did not win it"
        );
    }

    #[test]
    fn a_draw_declared_in_another_round_does_not_carry() {
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 2);
        let (mut table, mut dice, mut rng) = kit();
        state.combat_round_seq = 4;
        state.combat_draw_round = Some(3);

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();

        assert_eq!(
            outcome.winner,
            Some(attacker()),
            "last round's draw is not this round's"
        );
    }

    #[test]
    fn a_system_with_one_fleet_is_not_a_combat() {
        // 78.1: fewer than two players with ships, so there is nothing to fight.
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 2);
        let (mut table, mut dice, mut rng) = kit();

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();

        assert_eq!(outcome.winner, Some(attacker()));
        assert_eq!(outcome.rounds, 0);
        assert_eq!(dice.count(), 0, "no dice were rolled");
    }

    #[test]
    fn ground_forces_do_not_make_a_combat() {
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 1);
        put(&mut state, &system, "infantry", &defender(), 3);

        assert_eq!(
            combatants(&state, ContentStore::embedded(), POK, &system),
            vec![attacker()],
            "78.1 counts ships"
        );
    }

    #[test]
    fn a_fight_ends_with_one_fleet_standing() {
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 4);
        put(&mut state, &system, "fighter", &defender(), 1);
        let (mut table, mut dice, mut rng) = kit();

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .unwrap();

        assert!(outcome.winner.is_some(), "somebody won");
        assert!(outcome.rounds >= 1);
        let survivors = combatants(&state, ContentStore::embedded(), POK, &system);
        assert!(survivors.len() <= 1, "only one side can remain");
    }

    #[test]
    fn a_unit_that_does_not_fight_rolls_nothing() {
        // A printed combat value of zero means "does not fight", not "hits on 0".
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 1);
        let (_, mut dice, mut rng) = kit();

        let before = dice.count();
        let hits = roll_fleet(
            &state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &defender(),
            &system,
        );
        assert_eq!(hits, 0);
        assert_eq!(dice.count(), before, "an empty fleet rolls no dice");
    }

    #[test]
    fn every_fighting_ship_rolls() {
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 3);
        let (_, mut dice, mut rng) = kit();

        roll_fleet(
            &state,
            ContentStore::embedded(),
            POK,
            &mut dice,
            &mut rng,
            &attacker(),
            &system,
        );

        // Three destroyers share one combat value, so they roll together as one batch.
        assert_eq!(dice.count(), 1);
        assert_eq!(dice.history()[0].faces.len(), 3);
    }

    #[test]
    fn hits_are_absorbed_by_destroying_ships() {
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &defender(), 3);
        let (mut table, _, _) = kit();

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            2,
        )
        .unwrap();

        assert_eq!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).len(),
            1
        );
    }

    #[test]
    fn excess_hits_have_no_effect() {
        // 15.2a: more hits than units is not an error, and must not underflow.
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &defender(), 2);
        let (mut table, _, _) = kit();

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            9,
        )
        .unwrap();

        assert!(ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).is_empty());
    }

    #[test]
    fn sustain_damage_cancels_a_hit_instead_of_losing_the_ship() {
        // 87.1. FirstOption takes the sustain option, which is offered before the casualty.
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &defender(), 1);
        let mut table = Table::with_default(Box::new(FirstOption));

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            1,
        )
        .unwrap();

        let survivors = ships_of(&state, ContentStore::embedded(), POK, &defender(), &system);
        assert_eq!(survivors.len(), 1, "the ship survived");
        assert!(survivors[0].sustained_damage, "by taking damage");
    }

    #[test]
    fn a_carried_mech_cannot_sustain_a_space_hit() {
        // Only ships are assigned hits in space combat; a mech in the space area is cargo. Before
        // the fix FirstOption sustained the mech and the carrier lived.
        let (mut state, system) = arena();
        put(&mut state, &system, "carrier", &defender(), 1);
        put(&mut state, &system, "letnev_mech", &defender(), 1);
        let mut table = Table::with_default(Box::new(FirstOption));

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            1,
        )
        .unwrap();

        assert!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).is_empty(),
            "the carrier took the hit"
        );
        assert!(
            state
                .system_state(&system)
                .units
                .iter()
                .all(|unit| !unit.sustained_damage),
            "no unit sustained"
        );
    }

    #[test]
    fn a_combat_window_offers_no_sustain_to_a_carried_mech() {
        let (mut state, system) = arena();
        put(&mut state, &system, "carrier", &defender(), 1);
        put(&mut state, &system, "letnev_mech", &defender(), 1);
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        assert!(
            window
                .sustainers(&state, ContentStore::embedded(), POK, &defender())
                .is_empty()
        );
    }

    #[test]
    fn sustain_is_optional_and_declining_loses_the_ship() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &defender(), 1);
        let mut table = Table::with_default(Box::new(Scripted::new(["decline".to_owned()])));

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            1,
        )
        .unwrap();

        assert!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).is_empty(),
            "87.1 is always optional"
        );
    }

    #[test]
    fn an_already_damaged_ship_cannot_sustain_again() {
        let (mut state, system) = arena();
        state
            .system_mut(&system)
            .units
            .push(Unit::new(UnitTypeId::new("dreadnought"), defender()).sustained());
        let mut table = Table::with_default(Box::new(FirstOption));

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            1,
        )
        .unwrap();

        assert!(
            ships_of(&state, ContentStore::embedded(), POK, &defender(), &system).is_empty(),
            "it was already damaged, so the hit lands"
        );
    }

    #[test]
    fn interchangeable_casualties_are_one_decision_not_five() {
        // With five fighters and one dreadnought, offering per hull destroyed a fighter five
        // times in six whatever the decider thought of the trade — the count decided.
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &defender(), 5);
        put(&mut state, &system, "cruiser", &defender(), 1);
        let units = ships_of(&state, ContentStore::embedded(), POK, &defender(), &system);
        let mut table = Table::new();

        let taken = choose_casualty(
            &state,
            ContentStore::embedded(),
            POK,
            None,
            &mut table,
            &defender(),
            &units,
            &DecisionSource::Rule("78.4".to_owned()),
            "assign_casualty",
            Some(&system),
        )
        .unwrap();
        let offered = table.log.records.last().expect("a choice was recorded");

        assert_eq!(offered.offered.len(), 2, "one fighter, one cruiser");
        assert!(!taken.type_id.as_str().is_empty());
    }

    #[test]
    fn a_damaged_ship_is_a_different_casualty_from_a_fresh_one() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &defender(), 1);
        state
            .system_mut(&system)
            .units
            .push(Unit::new(UnitTypeId::new("dreadnought"), defender()).sustained());
        let units = ships_of(&state, ContentStore::embedded(), POK, &defender(), &system);
        let mut table = Table::new();

        choose_casualty(
            &state,
            ContentStore::embedded(),
            POK,
            None,
            &mut table,
            &defender(),
            &units,
            &DecisionSource::Rule("78.4".to_owned()),
            "assign_casualty",
            Some(&system),
        )
        .unwrap();
        let offered = table.log.records.last().unwrap();

        assert_eq!(offered.offered.len(), 2, "fresh and damaged are distinct");
    }

    /// OBS-008b1: a casualty option names the unit type and damage state it would destroy, and a
    /// sustain option names the unit type it would damage instead -- structured facts under the
    /// existing `unit`/`damaged` payload keys, not only the label. Both the standalone functions
    /// and `CombatWindow`'s own duplicated option-building carry it.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture exercising all four casualty/sustain option-building sites stays together"
    )]
    fn obs008b1_casualty_and_sustain_options_carry_their_unit_identity() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &defender(), 1);
        state
            .system_mut(&system)
            .units
            .push(Unit::new(UnitTypeId::new("dreadnought"), defender()).sustained());
        let units = ships_of(&state, ContentStore::embedded(), POK, &defender(), &system);

        let (decider, seen) = crate::choice::Capturing::new(Box::new(FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        choose_casualty(
            &state,
            ContentStore::embedded(),
            POK,
            None,
            &mut table,
            &defender(),
            &units,
            &DecisionSource::Rule("78.4".to_owned()),
            "assign_casualty",
            Some(&system),
        )
        .unwrap();
        let asked = seen.borrow();
        let fresh = asked[0]
            .options
            .iter()
            .find(|option| option.payload.get("damaged") == Some(&false.into()))
            .expect("the fresh dreadnought's option");
        assert_eq!(
            fresh
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );
        let damaged = asked[0]
            .options
            .iter()
            .find(|option| option.payload.get("damaged") == Some(&true.into()))
            .expect("the damaged dreadnought's option");
        assert_eq!(
            damaged
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );

        // The window's own Assigning stage duplicates this option-building and must agree.
        let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        window.stage = Stage::Assigning {
            queue: vec![Pending {
                player: defender(),
                hits: 1,
                producer: attacker(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let assigning = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("a hit is queued");
        assert!(
            assigning
                .options
                .iter()
                .all(|option| option.payload.contains_key("unit")),
            "every windowed casualty option names its unit"
        );

        // Sustain: one dreadnought, undamaged, so "damaged" would be dead weight and is omitted.
        state.system_mut(&system).units.pop(); // drop the already-damaged copy
        let content = ContentStore::embedded();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let (decider, sustain_seen) = crate::choice::Capturing::new(Box::new(FirstOption));
        let mut inner = Table::with_default(Box::new(decider));
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut inner,
            timing: None,
        };
        offer_sustain(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &defender(),
            &system,
            &attacker(),
            1,
            HitOrigin::CombatRoll,
            false,
            false,
        )
        .unwrap();
        let sustain_asked = sustain_seen.borrow();
        let sustain_option = sustain_asked[0]
            .options
            .iter()
            .find(|option| !option.is_decline())
            .expect("a sustain option was offered");
        assert_eq!(
            sustain_option
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );
        assert!(
            !sustain_option.payload.contains_key("damaged"),
            "every sustain candidate is undamaged by construction; the key is omitted, not false"
        );

        // A fresh dreadnought: `offer_sustain` above already damaged the previous one (it accepted
        // the first offered option), and a damaged ship no longer offers to sustain.
        let (mut window_state, window_system) = arena();
        put(
            &mut window_state,
            &window_system,
            "dreadnought",
            &defender(),
            1,
        );
        let mut window =
            CombatWindow::new(&window_state, ContentStore::embedded(), POK, &window_system);
        window.stage = Stage::Sustaining {
            queue: vec![Pending {
                player: defender(),
                hits: 1,
                producer: attacker(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let sustaining = window
            .pending_choice(&window_state, ContentStore::embedded(), POK)
            .expect("a hit is queued");
        let sustain_option = sustaining
            .options
            .iter()
            .find(|option| !option.is_decline())
            .expect("a sustain option");
        assert_eq!(
            sustain_option
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );
    }

    /// Every reroll question carries the whole roll (display only), so a client can show all the
    /// dice and which one is being asked about; the options and their ids are as before.
    #[test]
    fn every_reroll_question_carries_all_the_dice_and_its_own_position() {
        let (mut state, system) = arena();
        let player = defender();
        let entry = |unit: &str, hits_on, faces: Vec<u32>| RerollEntry {
            unit: unit.to_owned(),
            planet: None,
            hits_on: Some(hits_on),
            faces,
            rerolled: std::collections::BTreeSet::new(),
            deltas: std::collections::BTreeMap::new(),
            unit_types: std::collections::BTreeMap::new(),
        };
        state.reroll_staging.insert(
            player.clone(),
            RerollSet {
                kind: "ground".to_owned(),
                system,
                rolls: vec![entry("infantry", 8, vec![3, 9]), entry("mech", 6, vec![6])],
            },
        );
        let (decider, seen) = crate::choice::Capturing::new(Box::new(FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        let _ = choose_reroll_dice(
            &state,
            ContentStore::embedded(),
            POK,
            None,
            &mut table,
            &player,
            &DecisionSource::Content("test".to_owned()),
            "test_reroll",
        );

        let asked = seen.borrow();
        assert_eq!(asked.len(), 3, "one question per die");
        for (index, choice) in asked.iter().enumerate() {
            assert_eq!(choice.details["current"], index);
            let dice = choice.details["dice"].as_array().expect("the dice");
            assert_eq!(dice.len(), 3);
            assert_eq!(dice[1]["face"], 9);
            assert_eq!(dice[1]["hits_on"], 8);
            assert_eq!(dice[2]["unit"], "mech");
        }
    }

    /// OBS-008b2: a reroll option names its unit, current face, and hit threshold, and previews
    /// the exact d10 hit distribution the redraw would produce -- the odds `hit_count_preview`
    /// (OBS-007c) already computed exactly, finally attached to a real producer. Declining
    /// previews a certain, unchanged fact instead of the reroll's chance.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture exercising the reroll and decline previews stays together"
    )]
    fn obs008b2_reroll_options_preview_the_dies_exact_hit_distribution() {
        let (mut state, system) = arena();
        let player = defender();
        state.reroll_staging.insert(
            player.clone(),
            RerollSet {
                kind: "ground".to_owned(),
                system: system.clone(),
                rolls: vec![RerollEntry {
                    unit: "dreadnought".to_owned(),
                    planet: None,
                    hits_on: Some(6),
                    faces: vec![5], // 5 < 6: a current miss
                    rerolled: std::collections::BTreeSet::new(),
                    deltas: std::collections::BTreeMap::new(),
                    unit_types: std::collections::BTreeMap::new(),
                }],
            },
        );

        let (decider, seen) = crate::choice::Capturing::new(Box::new(FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        let _ = choose_reroll_dice(
            &state,
            ContentStore::embedded(),
            POK,
            None,
            &mut table,
            &player,
            &DecisionSource::Content("test".to_owned()),
            "test_reroll",
        );

        let asked = seen.borrow();
        let choice = &asked[0];
        let reroll = choice
            .options
            .iter()
            .find(|option| !option.is_decline())
            .expect("a reroll option");
        assert_eq!(
            reroll
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );
        assert_eq!(
            reroll
                .payload
                .get("face")
                .and_then(serde_json::Value::as_i64),
            Some(5)
        );
        assert_eq!(
            reroll
                .payload
                .get("hits_on")
                .and_then(serde_json::Value::as_i64),
            Some(6)
        );
        match &reroll
            .preview
            .as_ref()
            .expect("a reroll is previewed")
            .outcome
        {
            crate::preview::Outcome::Chanced { cases, out_of } => {
                // Hitting on 6+: 5 of 10 faces hit.
                assert_eq!(*out_of, 10);
                let hit = cases
                    .iter()
                    .find(|case| case.label == "1-hits")
                    .expect("a hit case");
                assert_eq!(hit.weight, 5);
                assert_eq!(hit.deltas, vec![Delta::new(Quantity::Hits, 0, 1)]);
                let miss = cases
                    .iter()
                    .find(|case| case.label == "0-hits")
                    .expect("a miss case");
                assert_eq!(miss.weight, 5);
                assert_eq!(miss.deltas, vec![Delta::new(Quantity::Hits, 0, 0)]);
            }
            other => panic!("a reroll preview is chanced, got {other:?}"),
        }

        let decline = choice
            .options
            .iter()
            .find(|option| option.is_decline())
            .expect("a decline option");
        assert_eq!(
            decline
                .payload
                .get("unit")
                .and_then(serde_json::Value::as_str),
            Some("dreadnought")
        );
        match &decline
            .preview
            .as_ref()
            .expect("declining is previewed")
            .outcome
        {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(
                    deltas,
                    &[Delta::new(Quantity::Hits, 0, 0)],
                    "declining leaves this die's current miss unchanged, not a chance"
                );
            }
            other => panic!("a decline preview is certain, got {other:?}"),
        }
    }

    /// OBS-008b3: announcing a retreat previews the whole fleet leaving the combat system (78.7
    /// retreats it together, not selectively); staying previews no change; both now carry
    /// `system`, which they previously lacked entirely. Choosing a destination previews the
    /// fleet arriving there, added to whatever the seat already holds.
    #[test]
    fn obs008b3_retreat_options_preview_the_whole_fleets_arrival() {
        let hub = crate::fixtures::plain_hub();
        let system = SystemId::new(&hub.centre);
        let mut state = crate::fixtures::game(&["a", "b"]);
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter().take(2)) {
            state.board.entry(SystemId::new(id)).or_default();
        }
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 2);
        let refuge_a = SystemId::new(&hub.outer[0]);
        let refuge_b = SystemId::new(&hub.outer[1]);
        put(&mut state, &refuge_a, "fighter", &defender(), 1);
        put(&mut state, &refuge_b, "fighter", &defender(), 3);

        let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system)
            .with_galaxy(hub.galaxy);
        // As it stands once round 1 has opened (no barrage between fighters).
        window.stage = Stage::Announcing {
            round: 1,
            asking: defender(),
            announced: Vec::new(),
        };

        // Announcing: the defender's own two fighters here previews 2 -> 2 for "stay" and
        // 2 -> 0 for "retreat", both carrying `system`.
        let announcing = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("round 1 asks the defender to announce");
        for (id, before, after) in [("stay", 2, 2), ("retreat", 2, 0)] {
            let option = announcing
                .options
                .iter()
                .find(|option| option.id == id)
                .unwrap_or_else(|| panic!("a {id} option"));
            assert_eq!(
                option
                    .payload
                    .get("system")
                    .and_then(serde_json::Value::as_str),
                Some(system.as_str())
            );
            match &option.preview.as_ref().expect("previewed").outcome {
                crate::preview::Outcome::Certain { deltas } => {
                    assert_eq!(
                        deltas,
                        &[Delta::new(Quantity::ShipsInSystem, before, after)]
                    );
                }
                other => panic!("a retreat announcement preview is certain, got {other:?}"),
            }
        }

        // Retreating: two fighters leave, arriving atop whatever is already at each refuge.
        let mut window = window;
        window.stage = Stage::Retreating {
            round: 1,
            leaving: vec![defender()],
        };
        let retreating = window
            .pending_choice(&state, ContentStore::embedded(), POK)
            .expect("two eligible destinations");
        for (id, already_there) in [(&refuge_a, 1), (&refuge_b, 3)] {
            let option = retreating
                .options
                .iter()
                .find(|option| option.id == id.to_string())
                .unwrap_or_else(|| panic!("a retreat-to-{id} option"));
            assert_eq!(
                option
                    .payload
                    .get("system")
                    .and_then(serde_json::Value::as_str),
                Some(id.as_str())
            );
            match &option.preview.as_ref().expect("previewed").outcome {
                crate::preview::Outcome::Certain { deltas } => {
                    assert_eq!(
                        deltas,
                        &[Delta::new(
                            Quantity::ShipsInSystem,
                            already_there,
                            already_there + 2
                        )]
                    );
                }
                other => panic!("a retreat destination preview is certain, got {other:?}"),
            }
        }
    }

    /// OBS-008b4: sustaining previews the fleet unchanged (the ship survives, damaged);
    /// declining previews it falling by one (the ship is destroyed) -- through both the
    /// standalone `offer_sustain` and the windowed `Sustaining` stage that duplicates it.
    #[test]
    fn obs008b4_sustain_options_preview_the_fleet_surviving_or_falling_by_one() {
        let (mut state, system) = arena();
        let player = defender();
        put(&mut state, &system, "dreadnought", &player, 1);
        put(&mut state, &system, "cruiser", &player, 1); // no sustain, but still counts as a ship

        let content = ContentStore::embedded();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let (decider, seen) = crate::choice::Capturing::new(Box::new(FirstOption));
        let mut inner = Table::with_default(Box::new(decider));
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut inner,
            timing: None,
        };
        offer_sustain(
            &mut state,
            content,
            POK,
            None,
            &mut ctx,
            &player,
            &system,
            &attacker(),
            1,
            HitOrigin::CombatRoll,
            false,
            false,
        )
        .unwrap();
        let asked = seen.borrow();
        let choice = &asked[0];
        let sustain = choice
            .options
            .iter()
            .find(|option| !option.is_decline())
            .expect("a sustain option");
        match &sustain.preview.as_ref().expect("previewed").outcome {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(
                    deltas,
                    &[Delta::new(Quantity::ShipsInSystem, 2, 2)],
                    "the ship survives, damaged -- the fleet count does not change"
                );
            }
            other => panic!("a sustain preview is certain, got {other:?}"),
        }
        let decline = choice
            .options
            .iter()
            .find(|option| option.is_decline())
            .expect("a decline option");
        match &decline.preview.as_ref().expect("previewed").outcome {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(
                    deltas,
                    &[Delta::new(Quantity::ShipsInSystem, 2, 1)],
                    "declining loses the ship -- the fleet falls by exactly one"
                );
            }
            other => panic!("a decline preview is certain, got {other:?}"),
        }

        // A fresh position: `offer_sustain` above already damaged the dreadnought (it accepted
        // the first offered option), and a damaged ship no longer offers to sustain.
        let (mut window_state, window_system) = arena();
        put(&mut window_state, &window_system, "dreadnought", &player, 1);
        put(&mut window_state, &window_system, "cruiser", &player, 1);

        // The windowed Sustaining stage duplicates this option-building and must agree.
        let mut window =
            CombatWindow::new(&window_state, ContentStore::embedded(), POK, &window_system);
        window.stage = Stage::Sustaining {
            queue: vec![Pending {
                player: player.clone(),
                hits: 1,
                producer: attacker(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let sustaining = window
            .pending_choice(&window_state, ContentStore::embedded(), POK)
            .expect("a hit is queued");
        let windowed_sustain = sustaining
            .options
            .iter()
            .find(|option| !option.is_decline())
            .expect("a sustain option");
        match &windowed_sustain
            .preview
            .as_ref()
            .expect("previewed")
            .outcome
        {
            crate::preview::Outcome::Certain { deltas } => {
                assert_eq!(deltas, &[Delta::new(Quantity::ShipsInSystem, 2, 2)]);
            }
            other => panic!("a sustain preview is certain, got {other:?}"),
        }
    }

    #[test]
    fn the_same_seed_fights_the_same_battle() {
        let fight = |seed: u64| {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "cruiser", &defender(), 3);
            let mut table = Table::new();
            let mut dice = Dice::new();
            let mut rng = GameRng::new(seed);
            let outcome = resolve(
                &mut state,
                ContentStore::embedded(),
                POK,
                &mut table,
                &mut dice,
                &mut rng,
                &system,
            )
            .unwrap();
            (outcome, dice.count())
        };

        assert_eq!(fight(11), fight(11), "a seed reproduces its battle");
    }

    #[test]
    fn a_stepped_combat_keeps_hits_simultaneous() {
        // The thing that makes a resumable combat hard: both sides' hits must be computed
        // before either is absorbed, or a casualty reduces return fire it had already earned.
        // Resolving one side to completion and then rolling the other - the obvious way to
        // write this - would break 78.6 exactly there.
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);

        let mut window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        let mut table = Table::new();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(2);
        let mut inner = Table::new();
        let mut ctx = crate::choice::Resolving {
            content: ContentStore::embedded(),
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut inner,
            timing: None,
        };
        window.settle(&mut state, &mut ctx).unwrap();

        let mut steps = 0;
        while let Some(choice) = window.pending_choice(&state, ContentStore::embedded(), POK) {
            let answer = table.ask(&choice).unwrap();
            window.resolve(&mut state, &mut ctx, answer).unwrap();
            steps += 1;
            assert!(steps < 500, "a stepped combat must terminate");
        }

        assert!(window.outcome().is_some(), "the fight concluded");
        let left = combatants(&state, ContentStore::embedded(), POK, &system);
        assert!(left.len() <= 1);
    }

    #[test]
    fn the_active_player_is_the_attacker_whoever_is_seated_where() {
        // LRR 13: "During combat, the active player is the attacker." LRR 29.1: the
        // defender is the player who is not the active player. `combatants` lists the
        // two sides in seating order, so the combat must anchor the roles to the active
        // seat: with b (seated second) active, b is the attacker and a the defender --
        // not the reverse that the raw seating order gives.
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);

        state.active = Some(attacker().clone());
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        assert_eq!(
            window.attacker,
            attacker(),
            "the active seat is the attacker"
        );
        assert_eq!(window.defender, defender());

        state.active = Some(defender().clone());
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        assert_eq!(
            window.attacker,
            defender(),
            "active-second is still the attacker"
        );
        assert_eq!(window.defender, attacker());

        // A combat opened without an active player (a test state the rules never
        // produce) keeps the seating order the engine always had.
        state.active = None;
        let window = CombatWindow::new(&state, ContentStore::embedded(), POK, &system);
        assert_eq!(window.attacker, attacker());
        assert_eq!(window.defender, defender());
    }

    /// Drive a combat window by hand — `settle` / `pending_choice` / `resolve` — consuming scoring
    /// pauses exactly as the synchronous API does (M07-022). Completion bookkeeping goes through
    /// the same `complete_window` that `resolve()` calls, so this replica cannot drift from it.
    /// Returns the outcome and how many choices had been asked when the first scoring pause was
    /// consumed (`None` if no pause was ever consumed) — the ordering pin for M07-023's review Q1.
    fn stepped_fight(
        state: &mut GameState,
        system: &SystemId,
        table: &mut Table,
        dice: &mut Dice,
        rng: &mut GameRng,
    ) -> (CombatOutcome, Option<usize>) {
        let content = ContentStore::embedded();
        // The Game driver and the synchronous resolve() both snapshot before the first die
        // (M07-021).
        let before = crate::combat::before_combat(state, content, POK, system);
        let mut window = CombatWindow::new(state, content, POK, system);
        // The context keeps its own table (the original harness shape): one long-lived context
        // borrows it for the whole loop, so asking must go through a separate table. resolve()
        // uses one table because Window::drive asks through ctx.table internally; here the ask
        // table is what tests assert on via its log, and the final assertion below keeps that
        // comparison honest (M07-023 review Q2).
        let mut inner = Table::new();
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice,
            rng,
            table: &mut inner,
            timing: None,
        };
        // How many choices had been asked when the first scoring pause was consumed (M07-023
        // review Q1): tests use this to assert that a choice came after the pause.
        let mut asks_before_pause: Option<usize> = None;
        window.settle(state, &mut ctx).unwrap();
        while window.outcome().is_none() {
            if let Some(choice) = window.pending_choice(state, content, POK) {
                let answer = table.ask(&choice).unwrap();
                window.resolve(state, &mut ctx, answer).unwrap();
            } else {
                // Mirror the synchronous API: consume scoring pauses and drive automatic
                // transitions.
                let occurrence = window.take_scoring_occurrence();
                if asks_before_pause.is_none() && occurrence.is_some() {
                    asks_before_pause = Some(table.log.records.len());
                }
                window.settle_open(state, &mut ctx).unwrap();
            }
        }
        let outcome = crate::combat::complete_window(state, content, POK, system, &before, &window)
            .expect("the fight resolved");
        // The log-equality assertions in the tests compare against the ask table only; if a
        // future fixture ever routes an internal ask through the context's table (e.g. faction
        // combat-round offers), fail informatively instead of comparing split logs (Q2).
        assert!(
            ctx.table.log.records.is_empty(),
            "the harness's context table must stay unasked: log assertions compare against the \
             ask table only"
        );
        (outcome, asks_before_pause)
    }

    #[test]
    fn a_stepped_combat_matches_the_driven_one() {
        // Stepping and driving are the same fight: Window::drive is only a loop over the
        // same two methods, and a seed proves they do not diverge.
        let fight = |stepped: bool| {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &attacker(), 3);
            put(&mut state, &system, "carrier", &defender(), 2);
            let mut table = Table::new();
            let mut dice = Dice::new();
            let mut rng = GameRng::new(17);
            if stepped {
                let (outcome, _asks_before_pause) =
                    stepped_fight(&mut state, &system, &mut table, &mut dice, &mut rng);
                (outcome, state)
            } else {
                let outcome = resolve(
                    &mut state,
                    ContentStore::embedded(),
                    POK,
                    &mut table,
                    &mut dice,
                    &mut rng,
                    &system,
                )
                .unwrap();
                (outcome, state)
            }
        };

        let (stepped_outcome, stepped_state) = fight(true);
        let (driven_outcome, driven_state) = fight(false);
        assert_eq!(stepped_outcome, driven_outcome);
        assert!(stepped_state.identical(&driven_state));
    }

    #[test]
    fn a_stepped_combat_matches_the_driven_one_across_a_barrage_pause() {
        // The same fight stepped by hand and driven synchronously must end identically even when
        // the round-1 barrage fires a feat and pauses for scoring (M07-022): the stepped side must
        // consume the pause exactly as resolve() does, or it stalls with the fight unresolved.
        let fight = |stepped: bool| {
            let (mut state, system) = arena();
            put(&mut state, &system, "destroyer", &attacker(), 1);
            put(&mut state, &system, "fighter", &defender(), 1);
            put(&mut state, &system, "cruiser", &defender(), 1);
            let mut table = Table::new();
            let mut dice = Dice::from_faces([10, 10, 10, 1]);
            let mut rng = GameRng::new(1);
            if stepped {
                let (outcome, _asks_before_pause) =
                    stepped_fight(&mut state, &system, &mut table, &mut dice, &mut rng);
                (outcome, state)
            } else {
                let outcome = resolve(
                    &mut state,
                    ContentStore::embedded(),
                    POK,
                    &mut table,
                    &mut dice,
                    &mut rng,
                    &system,
                )
                .unwrap();
                (outcome, state)
            }
        };

        let (stepped_outcome, stepped_state) = fight(true);
        let (driven_outcome, driven_state) = fight(false);
        assert_eq!(stepped_outcome, driven_outcome);
        // The barrage feat must be recorded on both sides — identity alone would also pass if
        // neither side had fired it.
        for state in [&stepped_state, &driven_state] {
            assert!(
                state
                    .player(&attacker())
                    .unwrap()
                    .event_feats
                    .iter()
                    .any(|(feat, _)| *feat == Feat::BarrageTookTheLastFighters),
                "the round-1 barrage feat must be recorded on both sides"
            );
        }
        assert!(stepped_state.identical(&driven_state));
    }

    #[test]
    fn a_stepped_combat_matches_the_driven_one_across_a_pause_and_assignment() {
        // The composition M07-022's review P2 left open: the fight pauses for scoring, resumes,
        // and then reaches a choice at the retained frame — the stepped driver must answer it.
        // No choice can arise before the round-1 barrage pause in this fixture (the barrage
        // stage offers none), so any recorded ask is necessarily after the pause.
        let fight = |stepped: bool| {
            let (mut state, system) = arena();
            put(&mut state, &system, "destroyer", &attacker(), 1);
            put(&mut state, &system, "fighter", &defender(), 1);
            put(&mut state, &system, "cruiser", &defender(), 2);
            let mut table = Table::new();
            // Round-1 AFB [10, 10] kills the fighter and pauses; round 1 then leaves one hit to
            // absorb across two cruisers (the assignment choice); round 2 ends the fight.
            let mut dice = Dice::from_faces([10, 10, 10, 1, 1, 10, 1]);
            let mut rng = GameRng::new(1);
            if stepped {
                let (outcome, asks_before_pause) =
                    stepped_fight(&mut state, &system, &mut table, &mut dice, &mut rng);
                (outcome, state, table.log.clone(), asks_before_pause)
            } else {
                let outcome = resolve(
                    &mut state,
                    ContentStore::embedded(),
                    POK,
                    &mut table,
                    &mut dice,
                    &mut rng,
                    &system,
                )
                .unwrap();
                (outcome, state, table.log.clone(), None)
            }
        };

        let (stepped_outcome, stepped_state, stepped_log, stepped_asks_before_pause) = fight(true);
        let (driven_outcome, driven_state, driven_log, _driven_asks_before_pause) = fight(false);
        assert_eq!(stepped_outcome, driven_outcome);
        // The barrage feat must be recorded on both sides — identity alone would also pass if
        // neither side had fired it.
        for state in [&stepped_state, &driven_state] {
            assert!(
                state
                    .player(&attacker())
                    .unwrap()
                    .event_feats
                    .iter()
                    .any(|(feat, _)| *feat == Feat::BarrageTookTheLastFighters),
                "the round-1 barrage feat must be recorded on both sides"
            );
        }
        // M07-023 review Q1: the ordering this test is named for must be asserted, not argued —
        // the pause was consumed with zero choices asked before it, so every recorded ask (the
        // assignment included) came after the pause.
        assert_eq!(
            stepped_asks_before_pause,
            Some(0),
            "the fixture must pause, and no choice may be asked before the barrage pause"
        );
        // The composition P2 names: a choice at the retained frame after the pause. Both sides
        // must have been asked to assign the hit, and their decision sequences must match.
        for log in [&stepped_log, &driven_log] {
            assert!(
                log.records
                    .iter()
                    .any(|r| r.prompt == "assign a hit" && r.player == defender()),
                "the driver must resume into the casualty-assignment choice after the pause"
            );
        }
        assert_eq!(stepped_log, driven_log);
        assert!(stepped_state.identical(&driven_state));
    }

    #[test]
    fn a_driven_combat_continues_after_its_barrage_scoring_pause() {
        let (mut state, system) = arena();
        put(&mut state, &system, "destroyer", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);
        put(&mut state, &system, "cruiser", &defender(), 1);
        let mut table = Table::new();
        let mut dice = Dice::from_faces([10, 10, 10, 1]);
        let mut rng = GameRng::new(1);

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .expect("the synchronous wrapper consumes its internal scoring pause");

        assert!(outcome.rounds >= 1);
        assert!(combatants(&state, ContentStore::embedded(), POK, &system).len() <= 1);
        assert!(
            state
                .player(&attacker())
                .unwrap()
                .event_feats
                .iter()
                .any(|(feat, _)| *feat == Feat::BarrageTookTheLastFighters)
        );
    }

    #[test]
    fn hits_are_simultaneous() {
        // 78.6. Two single fighters both hitting must both die: resolving sequentially would
        // let the first casualty cancel return fire it had already earned.
        let (mut state, system) = arena();
        put(&mut state, &system, "fighter", &attacker(), 1);
        put(&mut state, &system, "fighter", &defender(), 1);
        let mut table = Table::new();

        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &defender(),
            &system,
            &attacker(),
            1,
        )
        .unwrap();
        absorb_hits(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &attacker(),
            &system,
            &defender(),
            1,
        )
        .unwrap();

        assert!(
            combatants(&state, ContentStore::embedded(), POK, &system).is_empty(),
            "both fleets were destroyed in the same round"
        );
    }

    #[test]
    fn note_holdings_resolves_production_note_keys_to_seated_issuers() {
        // The production key is note_id(alias, owner_faction): "terraform:titans" is the
        // Titans' copy of Terraform. The issuer is the seat playing that faction — a PlayerId
        // built from the faction name matches no seat and can never fire.
        let mut state = crate::fixtures::game(&["a", "b"]);
        let content = ContentStore::embedded();
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("titans");
        state.player_mut(&b).unwrap().faction = ti4_model::id::FactionId::new("hacan");

        // A hand-held note never counts: the card says "in your play area", and Ceasefire is not
        // a play-area card. With only a held note, b has no issuers at all.
        crate::promissory::take(&mut state, content, &b, "cf:titans");
        let notes = note_holdings(&state);
        assert!(
            notes
                .get(&b)
                .is_none_or(std::collections::BTreeSet::is_empty),
            "a held Ceasefire is not in the play area"
        );

        // b receives a's Terraform: the corpus marks it playArea, so receipt puts it faceup in
        // b's play area — and the issuer must resolve to a's seat.
        crate::promissory::take(&mut state, content, &b, "terraform:titans");
        let notes = note_holdings(&state);
        assert_eq!(
            notes.get(&b),
            Some(&std::collections::BTreeSet::from([a.clone()])),
            "the seated Titans player is the issuer of a's note"
        );

        // A play-area note whose owner faction is not seated resolves to no issuer rather than a
        // phantom id.
        crate::promissory::take(&mut state, content, &b, "blood_pact:empyrean");
        // Blood Pact's own ACTION places it faceup (not receipt), so play it as the holder would.
        assert!(crate::promissory::play_action_note(
            &mut state,
            &b,
            "blood_pact:empyrean"
        ));
        let notes = note_holdings(&state);
        assert_eq!(
            notes.get(&b),
            Some(&std::collections::BTreeSet::from([a.clone()]))
        );
    }

    #[test]
    fn war_funding_is_offered_after_both_sides_roll_and_goes_home_when_used() {
        // Priced for trading, the card was never offered in a combat.
        struct UsesWarFunding;
        impl crate::choice::Decider for UsesWarFunding {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                if choice.prompt.starts_with("War Funding") {
                    return Ok(choice
                        .options
                        .iter()
                        .find(|option| option.id == "use")
                        .cloned()
                        .expect("use is offered"));
                }
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
        let a = PlayerId::new("a");
        let b = PlayerId::new("b");
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        state.player_mut(&b).unwrap().faction = ti4_model::id::FactionId::new("letnev");
        state.active = Some(a.clone());
        let note = crate::promissory::note_id("war_funding", "letnev");
        state.promissory_notes.insert(note.clone(), a.clone());
        let system = SystemId::new(&crate::fixtures::plain_systems(1)[0]);
        crate::fixtures::put(&mut state, &system, "dreadnought", &a, 2);
        crate::fixtures::put(&mut state, &system, "dreadnought", &b, 2);
        let mut table = Table::with_default(Box::new(UsesWarFunding));
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(11);

        resolve(
            &mut state,
            ContentStore::embedded(),
            POK,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .expect("the combat resolves");

        assert_eq!(
            state.promissory_notes.get(&note),
            Some(&b),
            "War Funding was used and went home to Letnev"
        );
    }

    #[test]
    fn neutral_ships_are_a_side_in_space_combat() {
        // Thunder's Edge neutral rule 2. The neutral owner is never seated, so a combatant list
        // built only from the seating order left a player alone against the Fracture's garrison.
        let mut state = crate::fixtures::game(&["a", "b"]);
        let system = SystemId::new("fracture1");
        let a = PlayerId::new("a");
        let neutral = crate::neutral_units::owner();
        crate::fixtures::put(&mut state, &system, "cruiser", &a, 1);
        crate::fixtures::put(&mut state, &system, "neutral_cruiser", &neutral, 2);

        let sides = combatants(
            &state,
            ContentStore::embedded(),
            ti4_model::content_types::FULL,
            &system,
        );

        assert_eq!(sides, vec![a, neutral]);
    }

    #[test]
    fn a_space_combat_against_neutral_units_is_fought_without_asking_the_neutral_side() {
        // Rules 2, 5, 6, 7 together: the combat happens, and nobody is ever asked to decide for
        // the neutral side -- not whether to retreat, not whether to sustain, not which unit dies.
        // The default decider refuses every question, so any neutral ask fails the combat.
        struct NeverAsked;
        impl crate::choice::Decider for NeverAsked {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "the neutral side must never be asked".to_owned(),
                })
            }
        }
        let a = PlayerId::new("a");
        let neutral = crate::neutral_units::owner();
        let system = SystemId::new("fracture4");
        let mut state = crate::fixtures::game(&["a"]);
        state.active = Some(a.clone());
        crate::fixtures::put(&mut state, &system, "dreadnought", &a, 3);
        crate::fixtures::put(&mut state, &system, "neutral_destroyer", &neutral, 1);
        crate::fixtures::put(&mut state, &system, "neutral_dreadnought", &neutral, 2);
        let mut table = Table::with_default(Box::new(NeverAsked));
        table.seat(a.clone(), Box::new(crate::choice::FirstOption));
        let mut dice = crate::dice::Dice::new();
        let mut rng = crate::rng::GameRng::new(7);

        let outcome = resolve(
            &mut state,
            ContentStore::embedded(),
            ti4_model::content_types::FULL,
            &mut table,
            &mut dice,
            &mut rng,
            &system,
        )
        .expect("the combat resolves without ever asking the neutral side");

        assert!(outcome.rounds >= 1, "a combat was actually fought");
        let content = ContentStore::embedded();
        let mine = ships_of(&state, content, ti4_model::content_types::FULL, &a, &system);
        let theirs = ships_of(
            &state,
            content,
            ti4_model::content_types::FULL,
            &neutral,
            &system,
        );
        assert!(
            mine.is_empty() || theirs.is_empty() || outcome.rounds >= MAX_ROUNDS,
            "the combat ran until one side was gone: {} vs {} left after {} rounds",
            mine.len(),
            theirs.len(),
            outcome.rounds
        );
    }

    #[test]
    fn space_combat_against_a_seated_note_issuer_records_betray_a_friend() {
        // Betray a Friend: "Win a combat against a player whose promissory note you had in your
        // play area at the start of your tactical action." The winner holds the loser's note,
        // faceup in its play area.
        let hub = crate::fixtures::plain_hub();
        let system = SystemId::new(hub.centre.clone());
        let mut state = crate::fixtures::game(&["a", "b"]);
        let content = ContentStore::embedded();
        let a = PlayerId::new("a"); // titans: the issuer, and the loser
        let b = PlayerId::new("b"); // hacan: holds a's note, and wins
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("titans");
        state.player_mut(&b).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        crate::fixtures::put(&mut state, &system, "cruiser", &a, 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b, 1);
        crate::promissory::take(&mut state, content, &b, "terraform:titans");

        let before = before_combat_with_notes(
            &state,
            ContentStore::embedded(),
            POK,
            &system,
            note_holdings(&state),
        );
        let occurrence = state.begin_feat_occurrence();
        let outcome = CombatOutcome {
            winner: Some(b.clone()),
            rounds: 1,
        };

        assert!(note_combat_event_feats(
            &mut state,
            ContentStore::embedded(),
            POK,
            &system,
            &before,
            &outcome,
            occurrence
        ));
        assert!(state.did_at_occurrence(&b, Feat::WonAgainstANoteHolder, occurrence));
    }

    #[test]
    fn a_note_received_after_the_snapshot_does_not_count_for_betray_a_friend() {
        // The card names the tactical action's start: a note that reaches the play area after
        // the snapshot is not one "you had" then.
        let hub = crate::fixtures::plain_hub();
        let system = SystemId::new(hub.centre.clone());
        let mut state = crate::fixtures::game(&["a", "b"]);
        let content = ContentStore::embedded();
        let a = PlayerId::new("a"); // titans: the issuer, and the loser
        let b = PlayerId::new("b"); // hacan: wins
        state.player_mut(&a).unwrap().faction = ti4_model::id::FactionId::new("titans");
        state.player_mut(&b).unwrap().faction = ti4_model::id::FactionId::new("hacan");
        crate::fixtures::put(&mut state, &system, "cruiser", &a, 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b, 1);

        let before = before_combat_with_notes(&state, content, POK, &system, note_holdings(&state));
        // The note arrives only after the snapshot.
        crate::promissory::take(&mut state, content, &b, "terraform:titans");
        assert!(state.promissory_faceup.contains("terraform:titans"));

        let occurrence = state.begin_feat_occurrence();
        let outcome = CombatOutcome {
            winner: Some(b.clone()),
            rounds: 1,
        };
        note_combat_event_feats(
            &mut state, content, POK, &system, &before, &outcome, occurrence,
        );
        assert!(
            !state.did_at_occurrence(&b, Feat::WonAgainstANoteHolder, occurrence),
            "the snapshot predates the receipt"
        );
    }

    fn nekro_arena(lent: &[&str]) -> (GameState, SystemId) {
        let mut state = crate::fixtures::nekro_with_z(&[("a", "nekro"), ("b", "sol")], lent);
        let system = SystemId::new("18");
        put(&mut state, &system, "nekro_flagship", &attacker(), 1);
        (state, system)
    }

    #[test]
    fn a_nekro_flagship_with_the_hacan_z_token_may_buy_hits_with_trade_goods() {
        let roll = |lent: &[&str], answers: Vec<String>| {
            let (mut state, system) = nekro_arena(lent);
            state.player_mut(&attacker()).unwrap().trade_goods = 5;
            let mut table = Table::with_default(Box::new(crate::choice::Scripted::new(answers)));
            // Two dice just short of the Nekro flagship's 9: every 8 is a near miss.
            let mut dice = Dice::from_faces([8, 8, 8, 8]);
            let mut rng = GameRng::new(7);
            let mut ctx = Resolving {
                content: ContentStore::embedded(),
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: None,
            };
            let hits = roll_fleet_and_open(&mut state, &mut ctx, &attacker(), &system);
            (hits, state.player(&attacker()).unwrap().trade_goods)
        };
        assert_eq!(roll(&[], vec![]), (0, 5), "off by default: nothing offered");
        let (hits, goods) = roll(&["hacan"], vec!["kenara|1".to_owned()]);
        assert_eq!((hits, goods), (1, 4));
    }

    #[test]
    fn a_nekro_flagship_with_the_jolnar_z_token_makes_two_extra_hits_on_nines() {
        let set = |system: &SystemId| RerollSet {
            kind: "fleet".into(),
            system: system.clone(),
            rolls: vec![RerollEntry {
                unit: "nekro_flagship".into(),
                planet: None,
                hits_on: Some(9),
                faces: vec![9, 3],
                rerolled: std::collections::BTreeSet::new(),
                deltas: std::collections::BTreeMap::new(),
                unit_types: std::iter::once(("nekro_flagship".to_owned(), 1)).collect(),
            }],
        };
        let hits = |lent: &[&str]| {
            let (state, system) = nekro_arena(lent);
            fleet_hits(
                &state,
                ContentStore::embedded(),
                POK,
                &attacker(),
                &system,
                &set(&system),
            )
        };
        assert_eq!(hits(&[]), (1, 0));
        assert_eq!(hits(&["jolnar"]), (3, 0));
    }

    #[test]
    fn a_nekro_flagship_with_the_l1z1x_z_token_forces_its_hits_onto_non_fighters() {
        let hits = |lent: &[&str]| {
            let (state, system) = nekro_arena(lent);
            let set = RerollSet {
                kind: "fleet".into(),
                system: system.clone(),
                rolls: vec![RerollEntry {
                    unit: "nekro_flagship".into(),
                    planet: None,
                    hits_on: Some(9),
                    faces: vec![10],
                    rerolled: std::collections::BTreeSet::new(),
                    deltas: std::collections::BTreeMap::new(),
                    unit_types: std::iter::once(("nekro_flagship".to_owned(), 1)).collect(),
                }],
            };
            fleet_hits(
                &state,
                ContentStore::embedded(),
                POK,
                &attacker(),
                &system,
                &set,
            )
        };
        assert_eq!(hits(&[]), (1, 0));
        assert_eq!(hits(&["l1z1x"]), (0, 1));
    }

    #[test]
    fn a_nekro_flagship_with_the_letnev_z_token_repairs_itself_each_round() {
        let damaged_after = |lent: &[&str]| {
            let (mut state, system) = nekro_arena(lent);
            let unit = state.system_state(&system).units[0].clone();
            let mut hurt = unit.clone();
            hurt.sustained_damage = true;
            state.system_mut(&system).replace_unit(&unit, hurt);
            repair_self_repairing(&mut state, &system, &attacker());
            state.system_state(&system).units[0].sustained_damage
        };
        assert!(damaged_after(&[]));
        assert!(!damaged_after(&["letnev"]));
    }
}

/// Plasma Scoring's pick for each firing player that holds it: the best (lowest) hit value among
/// its units about to roll. [`take_plasma`] spends it on the first unit rolling at that value.
pub(crate) fn plasma_picks<'u>(
    state: &GameState,
    firing: impl Iterator<Item = (&'u Unit, Option<i64>)>,
) -> std::collections::BTreeMap<PlayerId, i64> {
    let mut picks: std::collections::BTreeMap<PlayerId, i64> = std::collections::BTreeMap::new();
    for (unit, value) in firing {
        let Some(value) = value else { continue };
        if !crate::technology::plasma_scoring(state, &unit.owner) {
            continue;
        }
        picks
            .entry(unit.owner.clone())
            .and_modify(|best| *best = (*best).min(value))
            .or_insert(value);
    }
    picks
}

/// One extra die for this unit if it is its owner's Plasma Scoring pick, spending the pick.
pub(crate) fn take_plasma(
    picks: &mut std::collections::BTreeMap<PlayerId, i64>,
    owner: &PlayerId,
    value: i64,
) -> usize {
    if picks.get(owner) == Some(&value) {
        picks.remove(owner);
        1
    } else {
        0
    }
}

/// BF-00c-space: the space-combat routes faction modules hang on (events, hit production, the
/// sustain and Direct Hit gates, the destroy-ships effect, the barrage excess).
#[cfg(test)]
mod space_routes_tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use serde_json::Value;
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::{ActionCardId, PlayerId, SystemId, UnitTypeId};
    use ti4_model::state::{GameState, LeaderStatus};
    use ti4_model::units::Unit;

    use super::*;
    use crate::choice::{Decider, Scripted, Table};
    use crate::factions::hooks_combat::{
        AfbExcess, CombatHooks, CombatMoment, HitSite, ProducedHits, with_test_hooks,
    };

    type Log = Vec<(String, BTreeMap<String, Value>)>;

    const PROBED: [&str; 7] = [
        "SPACE_COMBAT_ENDED",
        "SPACE_COMBAT_ROUND_ENDED",
        "HITS_TO_ASSIGN",
        "SUSTAIN_DAMAGE_USED",
        "SHIP_DESTROYED",
        "ANTI_FIGHTER_BARRAGE_STARTED",
        "COMBAT_ROUND_STARTED",
    ];

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    /// The sustain question lists one option per type; its `sustainers` detail tells a client how
    /// many sustains of the type it can plan. Two cruisers under The Cavalry are one sustain (the
    /// second plan step the client used to send was refused by the engine), two dreadnoughts two.
    #[test]
    fn the_sustainer_count_tells_two_cavalry_cruisers_from_two_dreadnoughts() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "nomad"), ("b", "sol")], POK);
        crate::promissory::take(&mut state, content, &b(), "cavalry:nomad");
        state.promissory_faceup.insert("cavalry:nomad".to_owned());
        state
            .faction_marks
            .insert("nomad:cavalry:b".to_owned(), "18|cruiser".to_owned());
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 2);
        crate::fixtures::put(&mut state, &system, "dreadnought", &b(), 2);
        let types = catalogue(content, POK);
        let available: Vec<(String, Unit)> = state
            .system_state(&system)
            .units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                sustains_in_space(
                    &state,
                    content,
                    POK,
                    &system,
                    &b(),
                    unit,
                    types.get(unit.type_id.as_str()).unwrap(),
                    true,
                )
            })
            .map(|(index, unit)| (index.to_string(), unit.clone()))
            .collect();
        assert_eq!(available.len(), 4, "both cruisers are offered at first");
        let counts = sustainer_counts(&state, content, POK, &system, &b(), true, &available);
        assert_eq!(
            counts,
            serde_json::json!({ "cruiser": 1, "dreadnought": 2 })
        );
    }

    #[test]
    fn ordinary_barrage_loss_reaches_milor_through_the_real_apply_route() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        crate::fixtures::put(&mut state, &system, "destroyer", &a(), 1);
        crate::fixtures::put(&mut state, &system, "fighter", &b(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let occurrence = state.begin_feat_occurrence();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(Scripted::new([
            "leader:yin:yinagent:SHIP_DESTROYED:after",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
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
            apply_barrage(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &system,
                &a(),
                &b(),
                &[(a(), 1)],
                occurrence,
            )
            .expect("the barrage loss and its reaction resolve");
        }

        assert_eq!(fighters_of(&state, content, POK, &b(), &system), 2);
        let loss = resolver
            .applied_events()
            .iter()
            .find(|event| event.event_type == "SHIP_DESTROYED")
            .expect("ordinary AFB announces the removed fighter");
        assert_eq!(
            loss.text("cause"),
            Some("unit_ability:anti_fighter_barrage")
        );
        assert_eq!(loss.boolean("during_space_combat"), Some(true));
    }

    #[test]
    fn ordinary_barrage_keeps_original_six_games_event_and_choice_silent() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
        crate::fixtures::put(&mut state, &system, "fighter", &b(), 1);
        let occurrence = state.begin_feat_occurrence();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::new();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
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
            apply_barrage(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &system,
                &a(),
                &b(),
                &[(a(), 1)],
                occurrence,
            )
            .expect("the compatibility control resolves");
        }

        assert!(
            resolver
                .applied_events()
                .iter()
                .all(|event| event.event_type != "SHIP_DESTROYED")
        );
        assert!(table.log.records.is_empty(), "no new compatibility choice");
    }

    #[test]
    fn waylay_barrage_loss_reaches_milor_with_afb_as_its_source() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        let round = state.combat_round_seq;
        state.player_mut(&a()).unwrap().waylay_barrage_round = Some(round);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let occurrence = state.begin_feat_occurrence();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(Scripted::new([
            "leader:yin:yinagent:SHIP_DESTROYED:after",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
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
            apply_barrage(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &system,
                &a(),
                &b(),
                &[(a(), 1)],
                occurrence,
            )
            .expect("Waylay assigns and announces its casualty");
        }

        assert_eq!(fighters_of(&state, content, POK, &b(), &system), 2);
        let loss = resolver
            .applied_events()
            .iter()
            .find(|event| event.event_type == "SHIP_DESTROYED")
            .expect("Waylay announces the casualty");
        assert_eq!(
            loss.text("cause"),
            Some("unit_ability:anti_fighter_barrage")
        );
        assert_eq!(loss.boolean("during_space_combat"), Some(true));
    }

    #[test]
    fn reflective_shielding_loss_reaches_milor_with_inherited_combat_context() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        state.pending_reflective_hits = Some((system.clone(), b(), 1));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(Scripted::new([
            "leader:yin:yinagent:SHIP_DESTROYED:after",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
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
            emit_sustain_used(
                &mut state,
                &mut ctx,
                None,
                &system,
                &a(),
                "dreadnought",
                &b(),
                true,
            )
            .expect("the reflective hit and destruction resolve");
        }

        assert_eq!(fighters_of(&state, content, POK, &b(), &system), 2);
        let loss = resolver
            .applied_events()
            .iter()
            .find(|event| event.event_type == "SHIP_DESTROYED")
            .expect("Reflective Shielding announces the casualty");
        assert_eq!(loss.text("cause"), Some("action_card:reflective_shielding"));
        assert_eq!(loss.boolean("during_space_combat"), Some(true));
    }

    #[test]
    fn precombat_space_cannon_loss_refuses_milor_and_courageous() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        state.player_mut(&b()).unwrap().action_cards = vec![ActionCardId::new("courageous")];
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::new();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
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
            absorb_hits_seeing_with(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &b(),
                &system,
                &a(),
                1,
                false,
                HitOrigin::UnitAbility,
            )
            .expect("the cannon casualty resolves");
        }

        let loss = resolver
            .applied_events()
            .iter()
            .find(|event| event.event_type == "SHIP_DESTROYED")
            .expect("the module-enabled route announces the casualty");
        assert_eq!(loss.text("cause"), Some("unit_ability:space_cannon"));
        assert_eq!(loss.boolean("during_space_combat"), Some(false));
        assert_eq!(
            state
                .player(&a())
                .unwrap()
                .leaders
                .get(ti4_model::id::text(&"yinagent")),
            Some(&LeaderStatus::Readied)
        );
        assert_eq!(
            state.player(&b()).unwrap().action_cards,
            [ActionCardId::new("courageous")]
        );
        assert!(
            table.log.records.is_empty(),
            "neither false combat trigger asks"
        );
    }

    #[test]
    fn actual_sustain_records_the_newly_damaged_copy_for_direct_hit() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        state.board.clear();
        let earlier = Unit::new(UnitTypeId::new("dreadnought"), b()).sustained();
        let fresh = Unit::new(UnitTypeId::new("dreadnought"), b()).galvanized();
        state.system_mut(&system).units = vec![earlier.clone(), fresh.clone()];
        let mut table = Table::with_default(Box::new(Scripted::new(["sustain|space:1"])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        assert_eq!(
            offer_sustain(
                &mut state,
                content,
                POK,
                None,
                &mut ctx,
                &b(),
                &system,
                &a(),
                1,
                HitOrigin::CombatRoll,
                true,
                false
            )
            .unwrap(),
            0
        );
        let mark: serde_json::Value =
            serde_json::from_str(&state.faction_marks["combat:sustain_target"]).unwrap();
        let stored: Unit = serde_json::from_value(mark["unit"].clone()).unwrap();
        assert_eq!(stored, fresh.sustained());
        assert!(mark["planet"].is_null());
        assert_eq!(state.system_state(&system).units[0], earlier);
    }

    #[test]
    fn planetary_sustain_skips_the_already_damaged_maximum() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let planet = ti4_model::id::PlanetId::new("mecatolrex");
        let mut state = crate::fixtures::seated_game(
            &[("a", "sol"), ("b", "naaz")],
            ti4_model::content_types::DEFAULT,
        );
        state.board.clear();
        let earlier = Unit::new(UnitTypeId::new("naaz_voltron"), b()).sustained();
        let fresh = Unit::new(UnitTypeId::new("naaz_voltron"), b()).galvanized();
        state.system_mut(&system).units = vec![Unit::new(UnitTypeId::new("cruiser"), b())];
        state
            .system_mut(&system)
            .planet_units
            .insert(planet.clone(), vec![earlier.clone(), fresh.clone()]);
        let mut table = Table::with_default(Box::new(Scripted::new(["sustain|planet:mecatolrex"])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        assert_eq!(
            offer_sustain(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                None,
                &mut ctx,
                &b(),
                &system,
                &a(),
                1,
                HitOrigin::CombatRoll,
                true,
                false
            )
            .unwrap(),
            0
        );
        let mark: serde_json::Value =
            serde_json::from_str(&state.faction_marks["combat:sustain_target"]).unwrap();
        let stored: Unit = serde_json::from_value(mark["unit"].clone()).unwrap();
        assert_eq!(stored, fresh.sustained());
        assert_eq!(mark["planet"], planet.as_str());
        assert_eq!(state.system_state(&system).on_planet(&planet)[0], earlier);
        assert_eq!(
            state.system_state(&system).on_planet(&planet)[1],
            fresh.sustained()
        );
    }

    #[test]
    fn planetary_sustain_offers_distinct_fresh_maximum_variants() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let planet = ti4_model::id::PlanetId::new("mecatolrex");
        let mut state = crate::fixtures::seated_game(
            &[("a", "sol"), ("b", "naaz")],
            ti4_model::content_types::DEFAULT,
        );
        state.board.clear();
        let damaged = Unit::new(UnitTypeId::new("naaz_voltron"), b()).sustained();
        let plain = Unit::new(UnitTypeId::new("naaz_voltron"), b());
        let duplicate_plain = plain.clone();
        let galvanized = plain.clone().galvanized();
        state.system_mut(&system).units = vec![Unit::new(UnitTypeId::new("cruiser"), b())];
        state.system_mut(&system).planet_units.insert(
            planet.clone(),
            vec![
                damaged.clone(),
                plain.clone(),
                duplicate_plain,
                galvanized.clone(),
            ],
        );
        let mut table = Table::with_default(Box::new(Scripted::new([
            "sustain|planet:mecatolrex:variant:3",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        assert_eq!(
            offer_sustain(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                None,
                &mut ctx,
                &b(),
                &system,
                &a(),
                1,
                HitOrigin::CombatRoll,
                true,
                false,
            )
            .unwrap(),
            0
        );
        let mark: serde_json::Value =
            serde_json::from_str(&state.faction_marks["combat:sustain_target"]).unwrap();
        let stored: Unit = serde_json::from_value(mark["unit"].clone()).unwrap();
        assert_eq!(stored, galvanized.sustained());
        assert_eq!(mark["planet"], planet.as_str());
        let board = state.system_state(&system);
        let units = board.on_planet(&planet);
        assert_eq!(units[0], damaged);
        assert_eq!(units[1], plain);
        assert_eq!(
            units[2], plain,
            "identical eligible copies remain equivalent"
        );
        assert_eq!(units[3], galvanized.sustained());
    }

    #[test]
    fn windowed_planetary_sustain_resolves_the_selected_maximum_variant() {
        use crate::choice::Window as _;

        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let planet = ti4_model::id::PlanetId::new("mecatolrex");
        let mut state = crate::fixtures::seated_game(
            &[("a", "sol"), ("b", "naaz")],
            ti4_model::content_types::DEFAULT,
        );
        state.board.clear();
        let plain = Unit::new(UnitTypeId::new("naaz_voltron"), b());
        let galvanized = plain.clone().galvanized();
        state.system_mut(&system).units = vec![Unit::new(UnitTypeId::new("cruiser"), b())];
        state
            .system_mut(&system)
            .planet_units
            .insert(planet.clone(), vec![plain.clone(), galvanized.clone()]);
        let mut window =
            CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system);
        window.stage = Stage::Sustaining {
            queue: vec![Pending {
                player: b(),
                hits: 1,
                producer: a(),
                non_fighters_only: false,
                producer_assigns: false,
            }],
            round: 1,
        };
        let choice = window
            .pending_choice(&state, content, ti4_model::content_types::DEFAULT)
            .expect("the queued hit offers both planetary variants");
        let ids: Vec<_> = choice
            .options
            .iter()
            .filter(|option| !option.is_decline())
            .map(|option| option.id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "sustain|planet:mecatolrex",
                "sustain|planet:mecatolrex:variant:1"
            ]
        );
        let selected = choice
            .options
            .iter()
            .find(|option| option.id == "sustain|planet:mecatolrex:variant:1")
            .unwrap()
            .clone();
        let mut table = Table::with_default(Box::new(crate::choice::FirstOption));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: None,
        };
        window.resolve(&mut state, &mut ctx, selected).unwrap();
        let mark: serde_json::Value =
            serde_json::from_str(&state.faction_marks["combat:sustain_target"]).unwrap();
        let stored: Unit = serde_json::from_value(mark["unit"].clone()).unwrap();
        assert_eq!(stored, galvanized.sustained());
        assert_eq!(mark["planet"], planet.as_str());
        let board = state.system_state(&system);
        let units = board.on_planet(&planet);
        assert_eq!(units[0], plain);
        assert_eq!(units[1], galvanized.sustained());
    }

    #[test]
    fn strict_staged_ship_loss_restores_batch_and_services_before_retry() {
        let content = ContentStore::embedded();
        let system = SystemId::new("18");
        let mut state = crate::fixtures::seated_game(&[("a", "yin"), ("b", "sol")], POK);
        state.pending_destructions.push((
            system.clone(),
            b(),
            UnitTypeId::new("cruiser"),
            "action_card:direct_hit".to_owned(),
            true,
        ));
        let before = state.clone();
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(Scripted::new([
            "not-an-offered-ability",
            "leader:yin:yinagent:SHIP_DESTROYED:after",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        for retry in [false, true] {
            let mut ctx = Resolving {
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
            let result = try_announce_staged_destructions(&mut state, &mut ctx);
            if retry {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
                assert!(state.identical(&before));
                assert!(ctx.table.log.records.is_empty());
                assert!(ctx.timing.as_ref().unwrap().resolver.log().is_empty());
                assert!(
                    ctx.timing
                        .as_ref()
                        .unwrap()
                        .resolver
                        .applied_events()
                        .is_empty()
                );
                assert_eq!(
                    *ctx.timing.as_ref().unwrap().sequence,
                    crate::event::EventSequence::new()
                );
            }
        }
        assert!(state.pending_destructions.is_empty());
        assert_eq!(fighters_of(&state, content, POK, &b(), &system), 2);
        assert_eq!(table.log.records.len(), 1);
    }

    #[test]
    fn retreat_capacity_loss_is_flushed_as_a_combat_destruction() {
        let content = ContentStore::embedded();
        let (system, destination) = (SystemId::new("18"), SystemId::new("19"));
        let mut state = crate::fixtures::seated_game(
            &[("a", "yin"), ("b", "sol")],
            ti4_model::content_types::DEFAULT,
        );
        crate::fixtures::put(&mut state, &system, "destroyer", &a(), 1);
        crate::fixtures::put(&mut state, &system, "fighter", &a(), 1);
        crate::fixtures::put(&mut state, &system, "cruiser", &b(), 1);
        let mut window =
            CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system);
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(Box::new(Scripted::new([
            "leader:yin:yinagent:SHIP_DESTROYED:after",
        ])));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        {
            let mut ctx = Resolving {
                content,
                sources: ti4_model::content_types::DEFAULT,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle {
                    resolver: &mut resolver,
                    sequence: &mut sequence,
                    galaxy: None,
                }),
            };
            window.retreat(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &mut ctx,
                &a(),
                &destination,
            );
        }

        assert!(
            state.pending_destructions.is_empty(),
            "the retreat flushes staging"
        );
        assert_eq!(
            fighters_of(
                &state,
                content,
                ti4_model::content_types::DEFAULT,
                &a(),
                &system
            ),
            2
        );
        let loss = resolver
            .applied_events()
            .iter()
            .find(|event| event.event_type == "SHIP_DESTROYED")
            .expect("the stranded fighter is announced");
        assert_eq!(loss.text("cause"), Some("retreat_capacity"));
        assert_eq!(loss.boolean("during_space_combat"), Some(true));
    }

    fn arena() -> (GameState, SystemId) {
        let state = crate::setup::start_game(
            ContentStore::embedded(),
            &[a(), b()],
            ti4_model::content_types::DEFAULT,
            None,
        )
        .unwrap();
        (state, SystemId::new("18"))
    }

    fn put(state: &mut GameState, system: &SystemId, kind: &str, owner: &PlayerId, n: usize) {
        for _ in 0..n {
            state
                .system_mut(system)
                .units
                .push(Unit::new(UnitTypeId::new(kind), owner.clone()));
        }
    }

    struct Fight {
        log: Log,
        outcome: CombatOutcome,
        state: GameState,
    }

    impl Fight {
        fn of(&self, event: &str) -> Vec<&BTreeMap<String, Value>> {
            self.log
                .iter()
                .filter(|(kind, _)| kind == event)
                .map(|(_, payload)| payload)
                .collect()
        }

        fn index(&self, event: &str) -> usize {
            self.log
                .iter()
                .position(|(kind, _)| kind == event)
                .unwrap_or_else(|| panic!("no {event}"))
        }
    }

    /// Logs every probed event's payload, plus `_b_ships`: how many ships `b` has in the
    /// event's system when the event is announced.
    fn probe(kind: &str, owner: PlayerId, log: &Arc<Mutex<Log>>) -> crate::timing::Ability {
        let log = Arc::clone(log);
        crate::timing::Ability::stateful(
            format!("probe:{kind}"),
            owner,
            kind,
            crate::timing::Relation::After,
            Arc::new(move |event, _, context| {
                let mut payload = event.payload.clone();
                let system = SystemId::new(payload.get("system").and_then(Value::as_str).unwrap());
                let ships = ships_of(
                    context.state,
                    context.content,
                    context.sources,
                    &b(),
                    &system,
                );
                payload.insert("_b_ships".to_owned(), ships.len().into());
                log.lock()
                    .unwrap()
                    .push((event.event_type.clone(), payload));
                Ok(())
            }),
        )
    }

    /// Answers the first option, noting who was asked what.
    struct Recorder(Arc<Mutex<Vec<(String, String)>>>);

    impl Decider for Recorder {
        fn choose(
            &mut self,
            choice: &crate::choice::Choice,
        ) -> Result<ChoiceOption, IllegalChoice> {
            self.0
                .lock()
                .unwrap()
                .push((choice.player.to_string(), choice.prompt.clone()));
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

    /// Fight the combat in `system` to its end with every probed event logged.
    fn try_fight(
        mut state: GameState,
        system: &SystemId,
        seed: u64,
        decider: Box<dyn Decider>,
        galaxy: Option<ti4_content::galaxy::Galaxy>,
    ) -> Result<Fight, CombatError> {
        let content = ContentStore::embedded();
        let log: Arc<Mutex<Log>> = Arc::new(Mutex::new(Vec::new()));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        resolver.register(PROBED.iter().map(|kind| probe(kind, a(), &log)));
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::with_default(decider);
        let mut dice = Dice::new();
        let mut rng = GameRng::new(seed);
        let mut window =
            CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, system);
        if let Some(galaxy) = galaxy {
            window = window.with_galaxy(galaxy);
        }
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        window.settle_open(&mut state, &mut ctx)?;
        while window.outcome().is_none() {
            window.drive(&mut state, &mut ctx)?;
            if window.outcome().is_some() {
                break;
            }
            let _ = window.take_scoring_occurrence();
            window.settle_open(&mut state, &mut ctx)?;
        }
        let outcome = window.outcome().unwrap();
        let log = log.lock().unwrap().clone();
        Ok(Fight {
            log,
            outcome,
            state,
        })
    }

    fn fight(
        state: GameState,
        system: &SystemId,
        seed: u64,
        script: &[&str],
        galaxy: Option<ti4_content::galaxy::Galaxy>,
    ) -> Fight {
        let decider = Box::new(Scripted::new(script.iter().copied()));
        try_fight(state, system, seed, decider, galaxy).unwrap()
    }

    fn text<'a>(payload: &'a BTreeMap<String, Value>, key: &str) -> &'a str {
        payload.get(key).and_then(Value::as_str).unwrap_or("")
    }

    fn int(payload: &BTreeMap<String, Value>, key: &str) -> i64 {
        payload.get(key).and_then(Value::as_i64).unwrap_or(-1)
    }

    fn names(payload: &BTreeMap<String, Value>, key: &str) -> Vec<String> {
        payload
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    type Hit = fn(
        &mut crate::timing::TimingContext<'_>,
        &HitSite<'_>,
    ) -> Result<ProducedHits, IllegalChoice>;

    /// One hit from `a` at `moment` (round 1), none otherwise.
    fn one_hit_at(site: &HitSite<'_>, moment: CombatMoment, kind: &str) -> ProducedHits {
        if site.player.as_str() == "a" && site.moment == moment && site.round == 1 {
            let mut hits = ProducedHits::NONE;
            match kind {
                "forced" => hits.non_fighter = 1,
                "chosen" => hits.producer_assigned = 1,
                _ => hits.any_ship = 1,
            }
            hits
        } else {
            ProducedHits::NONE
        }
    }

    fn start_hook() -> CombatHooks {
        let hook: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::CombatStart, "any"));
        CombatHooks {
            produced_hits: Some(hook),
            ..CombatHooks::NONE
        }
    }

    #[test]
    fn every_round_is_announced_as_over_and_the_end_follows_with_its_payload() {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &a(), 3);
        put(&mut state, &system, "destroyer", &b(), 1);
        let fight = fight(state, &system, 3, &[], None);

        let rounds = fight.of("SPACE_COMBAT_ROUND_ENDED");
        assert_eq!(
            i64::try_from(rounds.len()).unwrap(),
            i64::from(fight.outcome.rounds)
        );
        for (index, payload) in rounds.iter().enumerate() {
            assert_eq!(text(payload, "system"), "18");
            assert_eq!(text(payload, "attacker"), "a");
            assert_eq!(text(payload, "defender"), "b");
            assert_eq!(int(payload, "round"), i64::try_from(index).unwrap() + 1);
        }
        let ended = fight.of("SPACE_COMBAT_ENDED");
        assert_eq!(ended.len(), 1, "announced exactly once");
        let end = ended[0];
        assert_eq!(text(end, "system"), "18");
        assert_eq!(text(end, "attacker"), "a");
        assert_eq!(text(end, "defender"), "b");
        assert_eq!(int(end, "rounds"), i64::from(fight.outcome.rounds));
        assert_eq!(fight.outcome.winner, Some(a()));
        assert_eq!(text(end, "winner"), "a");
        assert_eq!(names(end, "losers"), ["b"]);
        assert_eq!(text(end, "reason"), "destruction");
        assert!(names(end, "retreated").is_empty());
        let destroyed = end["destroyed"].as_array().unwrap();
        assert!(
            destroyed
                .iter()
                .any(|ship| ship["player"] == "b" && ship["unit"] == "destroyer"),
            "the loser's ship is listed: {destroyed:?}"
        );
        // The round that finished the fight is announced before the end.
        let last_round = fight
            .log
            .iter()
            .rposition(|(kind, _)| kind == "SPACE_COMBAT_ROUND_ENDED")
            .unwrap();
        assert!(last_round < fight.index("SPACE_COMBAT_ENDED"));
    }

    #[test]
    fn a_retreat_ends_the_combat_and_the_round_is_announced_over_after_it() {
        for seed in 0..40_u64 {
            let hub = crate::fixtures::plain_hub();
            let system = SystemId::new(&hub.centre);
            let mut state = crate::fixtures::game(&["a", "b"]);
            for id in std::iter::once(&hub.centre).chain(hub.outer.iter().take(1)) {
                state.board.entry(SystemId::new(id)).or_default();
            }
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "cruiser", &b(), 1);
            put(
                &mut state,
                &SystemId::new(&hub.outer[0]),
                "fighter",
                &b(),
                1,
            );
            let fight = fight(state, &system, seed, &["retreat"], Some(hub.galaxy));
            let end = fight.of("SPACE_COMBAT_ENDED");
            assert_eq!(end.len(), 1);
            if text(end[0], "reason") != "retreat" {
                continue; // the round's hits ended it first
            }
            assert_eq!(names(end[0], "retreated"), ["b"]);
            assert_eq!(text(end[0], "winner"), "a");
            let lost_by_b = end[0]["destroyed"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|ship| ship["player"] == "b")
                .count();
            assert_eq!(lost_by_b, 0, "a retreating fleet is not a destroyed one");
            // Retreat is step 5 of the round: when the round is announced over, b has gone.
            let over = fight.of("SPACE_COMBAT_ROUND_ENDED");
            let last = over.last().expect("the round is announced over");
            assert_eq!(int(last, "_b_ships"), 0, "announced after the retreat");
            assert!(fight.index("SPACE_COMBAT_ROUND_ENDED") < fight.index("SPACE_COMBAT_ENDED"));
            return;
        }
        panic!("some seed lets the defender live to retreat");
    }

    /// Tellurian (Titans agent): "Before a hit would be assigned: You may exhaust this card to
    /// cancel that hit." Driven through the real combat window: a hit `b` produces at the start of
    /// the combat is about to land on `a`'s only ship; the card is offered in the HITS_TO_ASSIGN
    /// window and its cancellation is spent before the ship is lost.
    #[test]
    fn tellurian_cancels_the_hit_that_would_destroy_the_only_ship() {
        use crate::factions::hooks_combat::{CombatHooks, with_test_hooks};
        let run = |ready: bool, script: &[&str], seed: u64| {
            let hook: Hit = |_, site| {
                let mut hits = ProducedHits::NONE;
                if site.player.as_str() == "b"
                    && site.moment == CombatMoment::CombatStart
                    && site.round == 1
                {
                    hits.any_ship = 1;
                }
                Ok(hits)
            };
            let hooks = CombatHooks {
                produced_hits: Some(hook),
                ..CombatHooks::NONE
            };
            with_test_hooks(hooks, || {
                let content = ContentStore::embedded();
                let mut state = crate::fixtures::seated_game(
                    &[("a", "titans"), ("b", "sol")],
                    ti4_model::content_types::DEFAULT,
                );
                let system = SystemId::new("18");
                put(&mut state, &system, "cruiser", &a(), 1);
                put(&mut state, &system, "cruiser", &b(), 1);
                if !ready {
                    state.player_mut(&a()).unwrap().leaders.insert(
                        ti4_model::id::LeaderId::new("titansagent"),
                        ti4_model::state::LeaderStatus::Exhausted,
                    );
                }
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let mut table =
                    Table::with_default(Box::new(Scripted::new(script.iter().copied())));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let mut window =
                    CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system);
                let mut ctx = Resolving {
                    content,
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle {
                        resolver: &mut resolver,
                        sequence: &mut sequence,
                        galaxy: None,
                    }),
                };
                window.settle_open(&mut state, &mut ctx).unwrap();
                let ships = ships_of(
                    &state,
                    content,
                    ti4_model::content_types::DEFAULT,
                    &a(),
                    &system,
                )
                .len();
                let agent = state
                    .player(&a())
                    .unwrap()
                    .leaders
                    .get(ti4_model::id::text(&"titansagent"))
                    .copied();
                (ships, agent, crate::combat::cancellable_hits(&state, &a()))
            })
        };
        // The combat goes on to roll dice after the produced hit; the same seed rolls the same
        // dice whether or not the card was used, so any difference is the cancelled hit.
        let window = ["leader:titans:titansagent:HITS_TO_ASSIGN:when"];
        let mut saved = 0;
        for seed in 0..30 {
            let (with, agent, pool) = run(true, &window, seed);
            let (declined, ..) = run(true, &["decline"], seed);
            let (without, spent, _) = run(false, &[], seed);
            assert_eq!(agent, Some(ti4_model::state::LeaderStatus::Exhausted));
            assert_eq!(spent, Some(ti4_model::state::LeaderStatus::Exhausted));
            assert_eq!(pool, 0, "the cancellation was spent on that hit");
            assert_eq!(declined, without, "declining is the same as not having it");
            assert!(with >= without, "seed {seed}");
            if with > without {
                saved += 1;
            }
        }
        assert!(saved > 0, "the cancelled hit saved the ship on some seed");
    }

    /// Shields Holding (a round-scoped grant) still cancels a hit before a sustain-capable ship
    /// absorbs it, both when granted before the window and when a card grants it inside the
    /// HITS_TO_ASSIGN window (the spend that follows the emit): the start-of-combat hit is
    /// cancelled, so the dreadnought is never offered a sustain for it. Without a cancellation the
    /// sustain is offered.
    #[test]
    fn a_cancellation_spares_a_sustaining_ship_before_the_sustain_offer() {
        use crate::factions::hooks_combat::{CombatHooks, with_test_hooks};
        // mode 0: nothing; 1: Shields Holding granted earlier in the round; 2: Tellurian in window.
        let run = |mode: u8, seed: u64| {
            let hook: Hit = |_, site| {
                let mut hits = ProducedHits::NONE;
                if site.player.as_str() == "b"
                    && site.moment == CombatMoment::CombatStart
                    && site.round == 1
                {
                    hits.any_ship = 1;
                }
                Ok(hits)
            };
            let hooks = CombatHooks {
                produced_hits: Some(hook),
                ..CombatHooks::NONE
            };
            with_test_hooks(hooks, || {
                let content = ContentStore::embedded();
                let mut state = crate::fixtures::seated_game(
                    &[("a", "titans"), ("b", "sol")],
                    ti4_model::content_types::DEFAULT,
                );
                let system = SystemId::new("18");
                put(&mut state, &system, "dreadnought", &a(), 1);
                put(&mut state, &system, "cruiser", &b(), 1);
                if mode != 2 {
                    state.player_mut(&a()).unwrap().leaders.insert(
                        ti4_model::id::LeaderId::new("titansagent"),
                        ti4_model::state::LeaderStatus::Exhausted,
                    );
                }
                let mut resolver = crate::fixtures::armed_resolver(&state);
                let mut sequence = crate::event::EventSequence::new();
                let script: &[&str] = if mode == 2 {
                    &["leader:titans:titansagent:HITS_TO_ASSIGN:when"]
                } else {
                    &[]
                };
                let mut table =
                    Table::with_default(Box::new(Scripted::new(script.iter().copied())));
                let mut dice = Dice::new();
                let mut rng = GameRng::new(seed);
                let mut window =
                    CombatWindow::new(&state, content, ti4_model::content_types::DEFAULT, &system);
                if mode == 1 {
                    // The start-of-combat hits are assigned in round 1 (the window has begun it).
                    state.combat_round_seq = 1;
                    grant_hit_cancellation(&mut state, &a(), 1);
                    state.combat_round_seq = 0;
                }
                let mut ctx = Resolving {
                    content,
                    sources: ti4_model::content_types::DEFAULT,
                    dice: &mut dice,
                    rng: &mut rng,
                    table: &mut table,
                    timing: Some(crate::choice::TimingHandle {
                        resolver: &mut resolver,
                        sequence: &mut sequence,
                        galaxy: None,
                    }),
                };
                window.settle_open(&mut state, &mut ctx).unwrap();

                // The window stops at its first pending decision: a sustain offer for `a` when
                // the start-of-combat hit still stands, something later when it was cancelled.
                window
                    .pending_choice(&state, content, ti4_model::content_types::DEFAULT)
                    .is_some_and(|choice| {
                        choice.player == a()
                            && choice
                                .options
                                .iter()
                                .any(|option| option.kind == SUSTAIN_KIND)
                    })
            })
        };
        // A later hit in the same round can raise a fresh offer, so the cancellation is judged
        // over seeds: the start hit's offer is always raised without it, and spared with it.
        let (mut spared_early, mut spared_in_window) = (0, 0);
        for seed in 0..30 {
            assert!(
                run(0, seed),
                "without a cancellation the sustain is offered"
            );
            spared_early += usize::from(!run(1, seed));
            spared_in_window += usize::from(!run(2, seed));
        }
        assert!(spared_early > 0, "an earlier grant cancels the offer");
        assert!(spared_in_window > 0, "an in-window grant cancels the offer");
    }

    #[test]
    fn hits_produced_at_the_start_precede_the_barrage_and_the_round_resumes_there() {
        with_test_hooks(start_hook(), || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "dreadnought", &b(), 1);
            let fight = fight(state, &system, 5, &[], None);

            assert_eq!(fight.log[0].0, "COMBAT_ROUND_STARTED");
            let first = &fight.log[1];
            assert_eq!(
                first.0, "HITS_TO_ASSIGN",
                "only the round's opening precedes the produced hit"
            );
            assert_eq!(text(&first.1, "player"), "b");
            assert_eq!(int(&first.1, "hits"), 1);
            assert_eq!(int(&first.1, "round"), 1);
            // "At the start of a space combat" is before anti-fighter barrage...
            assert!(
                fight.index("HITS_TO_ASSIGN") < fight.index("ANTI_FIGHTER_BARRAGE_STARTED"),
                "the produced hit is assigned before the barrage"
            );
            // ...and the round resumes at the barrage rather than opening again.
            assert_eq!(
                fight.of("ANTI_FIGHTER_BARRAGE_STARTED").len(),
                2,
                "once per side"
            );
            let round_one_starts = fight
                .of("COMBAT_ROUND_STARTED")
                .iter()
                .filter(|payload| int(payload, "round") == 1)
                .count();
            assert_eq!(round_one_starts, 1);
            // Assigned like any hit: the dreadnought is offered SUSTAIN DAMAGE, and the producer
            // is the module's player (what Direct Hit keys on).
            let sustain = fight.of("SUSTAIN_DAMAGE_USED")[0];
            assert_eq!(text(sustain, "player"), "b");
            assert_eq!(text(sustain, "producer"), "a");
        });
    }

    #[test]
    fn a_forced_hit_takes_a_non_fighter_while_one_remains() {
        let hook: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::CombatStart, "forced"));
        let hooks = CombatHooks {
            produced_hits: Some(hook),
            ..CombatHooks::NONE
        };
        with_test_hooks(hooks, || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "fighter", &b(), 2);
            put(&mut state, &system, "destroyer", &b(), 1);
            let fight = fight(state, &system, 5, &[], None);
            let first = fight.of("SHIP_DESTROYED")[0];
            assert_eq!(text(first, "player"), "b");
            assert_eq!(text(first, "unit"), "destroyer", "the fighters are spared");
            assert_eq!(
                text(first, "cause"),
                SHIP_CAUSE_COMBAT,
                "a casualty the combat itself assigned names that combat"
            );
        });
    }

    #[test]
    fn a_producer_assigned_hit_is_aimed_by_the_producer() {
        let chosen: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::CombatStart, "chosen"));
        let hooks = CombatHooks {
            produced_hits: Some(chosen),
            ..CombatHooks::NONE
        };
        let setup = || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "carrier", &b(), 1);
            put(&mut state, &system, "destroyer", &b(), 1);
            (state, system)
        };
        let asked = |hooks: CombatHooks| {
            with_test_hooks(hooks, || {
                let (state, system) = setup();
                let seen = Arc::new(Mutex::new(Vec::new()));
                let _ = try_fight(
                    state,
                    &system,
                    5,
                    Box::new(Recorder(Arc::clone(&seen))),
                    None,
                )
                .unwrap();
                std::mem::take(&mut *seen.lock().unwrap())
            })
        };
        let seen = asked(hooks);
        let first = seen
            .iter()
            .find(|(_, prompt)| prompt == "assign a hit")
            .expect("someone is asked to assign the produced hit");
        assert_eq!(first.0, "a", "the producer, not the owner, picks the ship");

        // The same hit as an ordinary one is the owner's choice.
        let ordinary: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::CombatStart, "any"));
        let seen = asked(CombatHooks {
            produced_hits: Some(ordinary),
            ..CombatHooks::NONE
        });
        let first = seen
            .iter()
            .find(|(_, prompt)| prompt == "assign a hit")
            .expect("the owner is asked");
        assert_eq!(first.0, "b");
    }

    #[test]
    fn a_producer_assigned_hit_on_neutral_ships_never_asks_the_neutral_side() {
        // The producer picks the neutral ship; the neutral side's SUSTAIN DAMAGE answer is still
        // automatic, so no question is ever put to the neutral seat (rules 5, 7). The table's
        // default decider fails on any question, so a neutral ask fails the combat.
        struct NeverAsked;
        impl Decider for NeverAsked {
            fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Err(IllegalChoice::DeciderFailed {
                    player: choice.player.clone(),
                    prompt: choice.prompt.clone(),
                    reason: "the neutral side must never be asked".to_owned(),
                })
            }
        }
        let chosen: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::CombatStart, "chosen"));
        let hooks = CombatHooks {
            produced_hits: Some(chosen),
            ..CombatHooks::NONE
        };
        let neutral = crate::neutral_units::owner();
        let system = SystemId::new("fracture4");
        let fight = |defenders: &[&str]| {
            with_test_hooks(hooks, || {
                let mut state = crate::fixtures::game(&["a"]);
                state.active = Some(a());
                crate::fixtures::put(&mut state, &system, "dreadnought", &a(), 3);
                for kind in defenders {
                    crate::fixtures::put(&mut state, &system, kind, &neutral, 1);
                }
                let seen = Arc::new(Mutex::new(Vec::new()));
                let mut table = Table::with_default(Box::new(NeverAsked));
                table.seat(a(), Box::new(Recorder(Arc::clone(&seen))));
                let mut dice = crate::dice::Dice::new();
                let mut rng = crate::rng::GameRng::new(7);
                resolve(
                    &mut state,
                    ContentStore::embedded(),
                    ti4_model::content_types::FULL,
                    &mut table,
                    &mut dice,
                    &mut rng,
                    &system,
                )
                .expect("the combat resolves without asking the neutral side");
                std::mem::take(&mut *seen.lock().unwrap())
            })
        };
        // Ships that can SUSTAIN DAMAGE: the neutral side's sustain answer must stay automatic
        // (before the fix it was left pending on the neutral seat and the combat failed).
        fight(&[
            "neutral_dreadnought",
            "neutral_dreadnought",
            "neutral_destroyer",
        ]);
        // No SUSTAIN DAMAGE on this side, so the produced hit must be aimed, and aimed by "a".
        let seen = fight(&["neutral_destroyer", "neutral_cruiser"]);
        assert!(
            seen.iter()
                .any(|(who, prompt)| who == "a" && prompt == "assign a hit"),
            "the producer picks the ship: {seen:?}"
        );
    }

    #[test]
    fn hits_added_after_the_roll_join_the_rounds_hits_and_draw_no_dice() {
        let setup = || {
            let (mut state, system) = arena();
            put(&mut state, &system, "carrier", &a(), 2);
            put(&mut state, &system, "fighter", &b(), 6);
            (state, system)
        };
        let first_hits = |fight: &Fight| {
            fight
                .of("HITS_TO_ASSIGN")
                .iter()
                .find(|payload| text(payload, "player") == "b" && int(payload, "round") == 1)
                .map_or(0, |payload| int(payload, "hits"))
        };
        let (state, system) = setup();
        let plain = fight(state, &system, 11, &[], None);
        let hook: Hit = |_, site| Ok(one_hit_at(site, CombatMoment::AfterRoll, "any"));
        let hooks = CombatHooks {
            produced_hits: Some(hook),
            ..CombatHooks::NONE
        };
        let boosted = with_test_hooks(hooks, || {
            let (state, system) = setup();
            fight(state, &system, 11, &[], None)
        });
        assert_eq!(first_hits(&boosted), first_hits(&plain) + 1);
    }

    #[test]
    fn hits_produced_after_a_round_follow_the_retreat_step_and_precede_the_announcement() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let hook: Hit = |context, site| {
            if site.moment == CombatMoment::RoundEnded {
                CALLS.fetch_add(1, Ordering::SeqCst);
                // Never asked once the fight is decided: both sides still have ships.
                for side in [site.player, site.opponent] {
                    assert!(
                        !ships_of(
                            context.state,
                            context.content,
                            context.sources,
                            side,
                            site.system
                        )
                        .is_empty(),
                        "RoundEnded asked after the combat was over"
                    );
                }
            }
            Ok(one_hit_at(site, CombatMoment::RoundEnded, "any"))
        };
        let hooks = CombatHooks {
            produced_hits: Some(hook),
            ..CombatHooks::NONE
        };
        let fight = with_test_hooks(hooks, || {
            let (mut state, system) = arena();
            put(&mut state, &system, "dreadnought", &a(), 4);
            put(&mut state, &system, "fighter", &b(), 10);
            fight(state, &system, 2, &[], None)
        });
        assert!(
            CALLS.load(Ordering::SeqCst) > 0,
            "asked while the fight went on"
        );
        let round_one_hits: Vec<usize> = fight
            .log
            .iter()
            .enumerate()
            .filter(|(_, (kind, payload))| {
                kind == "HITS_TO_ASSIGN"
                    && text(payload, "player") == "b"
                    && int(payload, "round") == 1
            })
            .map(|(index, _)| index)
            .collect();
        let over_at = fight
            .log
            .iter()
            .position(|(kind, payload)| {
                kind == "SPACE_COMBAT_ROUND_ENDED" && int(payload, "round") == 1
            })
            .unwrap();
        let last = *round_one_hits.last().unwrap();
        assert!(
            last < over_at,
            "the extra hit lands before the round is over"
        );
        assert_eq!(int(&fight.log[last].1, "hits"), 1);
    }

    #[test]
    fn a_module_hook_error_fails_the_combat_instead_of_passing_for_success() {
        let hook: Hit = |_, site| {
            if site.moment == CombatMoment::CombatStart {
                Err(IllegalChoice::NoOptions {
                    player: site.player.clone(),
                    prompt: "a module refused".to_owned(),
                })
            } else {
                Ok(ProducedHits::NONE)
            }
        };
        let hooks = CombatHooks {
            produced_hits: Some(hook),
            ..CombatHooks::NONE
        };
        with_test_hooks(hooks, || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "cruiser", &b(), 1);
            let result = try_fight(
                state,
                &system,
                1,
                Box::new(Scripted::new(Vec::<String>::new())),
                None,
            );
            assert!(
                matches!(
                    result,
                    Err(CombatError::IllegalChoice(IllegalChoice::NoOptions { .. }))
                ),
                "{:?}",
                result.map(|fight| fight.outcome)
            );
        });
    }

    #[test]
    fn a_module_can_forbid_sustain_damage_in_space() {
        let setup = || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "dreadnought", &b(), 1);
            (state, system)
        };
        with_test_hooks(start_hook(), || {
            let (state, system) = setup();
            let allowed = fight(state, &system, 5, &[], None);
            assert!(
                !allowed.of("SUSTAIN_DAMAGE_USED").is_empty(),
                "without the prohibition the dreadnought sustains"
            );
        });
        let may_sustain: fn(
            &GameState,
            &ContentStore,
            SourceSet,
            &crate::factions::CombatUnit<'_>,
        ) -> bool = |_, _, _, unit| unit.player.as_str() != "b";
        let hooks = CombatHooks {
            may_sustain: Some(may_sustain),
            ..start_hook()
        };
        with_test_hooks(hooks, || {
            let (state, system) = setup();
            let barred = fight(state, &system, 5, &[], None);
            assert!(barred.of("SUSTAIN_DAMAGE_USED").is_empty());
            assert_eq!(text(barred.of("SHIP_DESTROYED")[0], "unit"), "dreadnought");
        });
    }

    #[test]
    fn a_module_can_make_a_ship_immune_to_direct_hit() {
        let setup = || {
            let (mut state, system) = arena();
            put(&mut state, &system, "cruiser", &a(), 1);
            put(&mut state, &system, "dreadnought", &b(), 1);
            (state, system)
        };
        with_test_hooks(start_hook(), || {
            let (state, system) = setup();
            let plain = fight(state, &system, 5, &[], None);
            assert_eq!(plain.of("SUSTAIN_DAMAGE_USED")[0]["direct_hittable"], true);
        });
        let immune_hook: fn(
            &GameState,
            &ContentStore,
            SourceSet,
            &crate::factions::CombatUnit<'_>,
        ) -> bool = |_, _, _, unit| unit.unit_type == "dreadnought";
        let hooks = CombatHooks {
            direct_hit_immune: Some(immune_hook),
            ..start_hook()
        };
        with_test_hooks(hooks, || {
            let (state, system) = setup();
            let immune = fight(state, &system, 5, &[], None);
            assert_eq!(
                immune.of("SUSTAIN_DAMAGE_USED")[0]["direct_hittable"],
                false
            );
            let hittable = |unit: &str| {
                direct_hittable_at(
                    &immune.state,
                    ContentStore::embedded(),
                    ti4_model::content_types::DEFAULT,
                    &b(),
                    &system,
                    unit,
                )
            };
            assert!(!hittable("dreadnought"));
            // Everything else keeps its printed answer.
            let printed = direct_hittable(
                ContentStore::embedded(),
                ti4_model::content_types::DEFAULT,
                "carrier",
            );
            assert_eq!(hittable("carrier"), printed);
        });
    }

    #[test]
    fn an_empty_hook_table_changes_nothing() {
        let setup = || {
            let (mut state, system) = arena();
            put(&mut state, &system, "dreadnought", &a(), 2);
            put(&mut state, &system, "cruiser", &b(), 3);
            put(&mut state, &system, "fighter", &b(), 2);
            (state, system)
        };
        let (state, system) = setup();
        let plain = fight(state, &system, 9, &[], None);
        let empty = with_test_hooks(CombatHooks::NONE, || {
            let (state, system) = setup();
            fight(state, &system, 9, &[], None)
        });
        assert_eq!(plain.outcome, empty.outcome);
        assert_eq!(plain.log, empty.log);
        assert_eq!(plain.state.board, empty.state.board);
    }

    #[test]
    fn destroying_named_ships_is_atomic_and_announced_like_a_casualty() {
        let content = ContentStore::embedded();
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &a(), 2);
        put(&mut state, &system, "destroyer", &a(), 1);
        let cruiser = Unit::new(UnitTypeId::new("cruiser"), a());
        let missing = Unit::new(UnitTypeId::new("dreadnought"), a());

        // One victim is not there: nothing happens.
        let before = state.board.clone();
        let two_and_a_ghost = [cruiser.clone(), missing];
        assert_eq!(
            destroy_units(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &a(),
                &system,
                &two_and_a_ghost,
                SHIP_CAUSE_COMBAT
            ),
            0
        );
        assert_eq!(state.board, before);
        assert!(state.pending_destructions.is_empty());
        // Asking for three cruisers when two stand is refused too.
        let three = [cruiser.clone(), cruiser.clone(), cruiser.clone()];
        assert_eq!(
            destroy_units(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &a(),
                &system,
                &three,
                SHIP_CAUSE_COMBAT
            ),
            0
        );
        assert_eq!(state.board, before);

        // Two cruisers go, the destroyer stays, two announcements are staged.
        let two = [cruiser.clone(), cruiser];
        assert_eq!(
            destroy_units(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &a(),
                &system,
                &two,
                SHIP_CAUSE_COMBAT
            ),
            2
        );
        assert_eq!(
            ships_of(
                &state,
                content,
                ti4_model::content_types::DEFAULT,
                &a(),
                &system
            )
            .len(),
            1
        );
        assert_eq!(state.pending_destructions.len(), 2);

        // Announced through a resolver: the same SHIP_DESTROYED the combat emits, `last` set
        // from the board as it is then.
        let log: Arc<Mutex<Log>> = Arc::new(Mutex::new(Vec::new()));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        resolver.register(std::iter::once(probe("SHIP_DESTROYED", b(), &log)));
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::new();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        announce_staged_destructions(&mut state, &mut ctx);
        assert!(state.pending_destructions.is_empty());
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        for (_, payload) in log.iter() {
            assert_eq!(text(payload, "system"), "18");
            assert_eq!(text(payload, "player"), "a");
            assert_eq!(text(payload, "unit"), "cruiser");
            assert_eq!(payload["last"], false, "the destroyer still stands");
            assert_eq!(
                text(payload, "cause"),
                SHIP_CAUSE_COMBAT,
                "the cause the staging carried reaches the announcement"
            );
            assert_eq!(payload["during_space_combat"], true);
        }
    }

    /// The provenance a listener reads is the one the producer stated, never an inference from the
    /// board: a staged Nova Seed loss announces itself as such, and an event with no `cause` at all
    /// is not a combat loss.
    #[test]
    fn a_staged_destruction_announces_the_cause_it_was_staged_with() {
        let (mut state, system) = arena();
        let content = ContentStore::embedded();
        put(&mut state, &system, "cruiser", &b(), 1);
        let unit = state.system_state(&system).units[0].clone();
        assert_eq!(
            destroy_units(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &b(),
                &system,
                &[unit],
                "breakthrough:nova_seed"
            ),
            1
        );
        let log: Arc<Mutex<Log>> = Arc::new(Mutex::new(Vec::new()));
        let mut resolver = crate::fixtures::armed_resolver(&state);
        resolver.register(std::iter::once(probe("SHIP_DESTROYED", b(), &log)));
        let mut sequence = crate::event::EventSequence::new();
        let mut table = Table::new();
        let mut dice = Dice::new();
        let mut rng = GameRng::new(1);
        let mut ctx = Resolving {
            content,
            sources: ti4_model::content_types::DEFAULT,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle {
                resolver: &mut resolver,
                sequence: &mut sequence,
                galaxy: None,
            }),
        };
        announce_staged_destructions(&mut state, &mut ctx);
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(text(&log[0].1, "cause"), "breakthrough:nova_seed");
        assert_eq!(log[0].1["during_space_combat"], false);
        assert!(
            !destroyed_during_combat(&crate::event::Event::new(
                1,
                "SHIP_DESTROYED",
                log[0].1.clone()
            )),
            "a Nova Seed loss is not a combat loss"
        );
        let mut without_cause = log[0].1.clone();
        without_cause.remove("cause");
        assert!(
            !destroyed_during_combat(&crate::event::Event::new(
                1,
                "SHIP_DESTROYED",
                without_cause
            )),
            "an event that states no cause is not evidence of a combat"
        );
    }

    fn barrage_unit() -> String {
        ti4_content::units::catalogue(ContentStore::embedded(), ti4_model::content_types::DEFAULT)
            .iter()
            .find(|(_, kind)| kind.has_anti_fighter_barrage() && kind.is_ship())
            .map(|(id, _)| (*id).to_owned())
            .unwrap()
    }

    #[test]
    fn barrage_hits_beyond_the_fighters_reach_the_module() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static EXCESS: AtomicUsize = AtomicUsize::new(0);
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let note: fn(
            &mut crate::timing::TimingContext<'_>,
            &AfbExcess<'_>,
        ) -> Result<(), IllegalChoice> = |_, site| {
            assert_eq!(site.producer.as_str(), "a");
            assert_eq!(site.target.as_str(), "b");
            EXCESS.fetch_add(site.excess, Ordering::SeqCst);
            CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        let hooks = CombatHooks {
            afb_excess: Some(note),
            ..CombatHooks::NONE
        };
        let content = ContentStore::embedded();
        with_test_hooks(hooks, || {
            let (mut state, system) = arena();
            put(&mut state, &system, &barrage_unit(), &a(), 6);
            put(&mut state, &system, "fighter", &b(), 1);
            put(&mut state, &system, "cruiser", &b(), 1);
            let mut dice = Dice::new();
            let mut rng = GameRng::new(4);
            let fired = anti_fighter_barrage(
                &mut state,
                content,
                ti4_model::content_types::DEFAULT,
                &mut dice,
                &mut rng,
                &system,
                &a(),
                &b(),
            )
            .unwrap();
            let hits = fired
                .iter()
                .find(|(player, _)| *player == a())
                .map_or(0, |(_, hits)| *hits);
            assert!(
                hits >= 2,
                "six barrage ships hit more than once at this seed"
            );
            assert_eq!(
                EXCESS.load(Ordering::SeqCst),
                hits - 1,
                "one fighter took one"
            );
            assert_eq!(CALLS.load(Ordering::SeqCst), 1);
            // The fighter is gone, the cruiser is not a barrage target.
            assert_eq!(
                fighters_of(
                    &state,
                    content,
                    ti4_model::content_types::DEFAULT,
                    &b(),
                    &system
                ),
                0
            );
            assert_eq!(
                non_fighter_ships_of(
                    &state,
                    content,
                    ti4_model::content_types::DEFAULT,
                    &b(),
                    &system
                ),
                1
            );
        });
    }

    #[test]
    fn an_afb_hook_error_fails_the_barrage() {
        let refuse: fn(
            &mut crate::timing::TimingContext<'_>,
            &AfbExcess<'_>,
        ) -> Result<(), IllegalChoice> = |_, site| {
            Err(IllegalChoice::NoOptions {
                player: site.producer.clone(),
                prompt: "a module refused".to_owned(),
            })
        };
        let hooks = CombatHooks {
            afb_excess: Some(refuse),
            ..CombatHooks::NONE
        };
        with_test_hooks(hooks, || {
            let (mut state, system) = arena();
            put(&mut state, &system, &barrage_unit(), &a(), 6);
            put(&mut state, &system, "fighter", &b(), 1);
            let mut dice = Dice::new();
            let mut rng = GameRng::new(4);
            let result = anti_fighter_barrage(
                &mut state,
                ContentStore::embedded(),
                ti4_model::content_types::DEFAULT,
                &mut dice,
                &mut rng,
                &system,
                &a(),
                &b(),
            );
            assert!(matches!(
                result,
                Err(CombatError::IllegalChoice(IllegalChoice::NoOptions { .. }))
            ));
        });
    }

    thread_local! {
        static SAW_SHIP_FORM: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    #[test]
    fn a_ground_form_mech_in_space_takes_its_ship_form_for_the_combat_and_flips_back() {
        let kinds = |state: &GameState, system: &SystemId| -> Vec<String> {
            state
                .system_state(system)
                .units
                .iter()
                .filter(|unit| unit.owner == a())
                .map(|unit| unit.type_id.to_string())
                .collect()
        };
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &a(), 1);
        put(&mut state, &system, "naaz_mech", &a(), 1);
        put(&mut state, &system, "destroyer", &b(), 1);
        SAW_SHIP_FORM.with(|cell| cell.set(None));
        let probe = CombatHooks {
            produced_hits: Some(|ctx, site| {
                if site.player.as_str() == "a" && site.moment == CombatMoment::CombatStart {
                    let seen = ctx
                        .state
                        .system_state(site.system)
                        .units
                        .iter()
                        .any(|unit| {
                            unit.owner.as_str() == "a" && unit.type_id.as_str() == "naaz_mech_space"
                        });
                    SAW_SHIP_FORM.with(|cell| cell.set(Some(seen)));
                }
                Ok(ProducedHits::NONE)
            }),
            ..CombatHooks::NONE
        };
        let fight = crate::factions::hooks_combat::with_test_hooks(probe, || {
            fight(state, &system, 3, &[], None)
        });
        assert_eq!(
            SAW_SHIP_FORM.with(std::cell::Cell::get),
            Some(true),
            "the mech is a ship when the combat starts"
        );
        let left = kinds(&fight.state, &system);
        assert!(
            !left.iter().any(|kind| kind == "naaz_mech_space"),
            "no ship form survives the battle: {left:?}"
        );
    }

    #[test]
    fn combats_without_a_dual_form_unit_flip_nothing() {
        let (mut state, system) = arena();
        put(&mut state, &system, "cruiser", &a(), 2);
        put(&mut state, &system, "infantry", &a(), 1);
        put(&mut state, &system, "destroyer", &b(), 1);
        let before: Vec<String> = state
            .system_state(&system)
            .units
            .iter()
            .map(|unit| unit.type_id.to_string())
            .collect();
        let fight = fight(state, &system, 5, &[], None);
        let after: Vec<String> = fight
            .state
            .system_state(&system)
            .units
            .iter()
            .filter(|unit| unit.owner == a())
            .map(|unit| unit.type_id.to_string())
            .collect();
        assert!(after.iter().all(|kind| before.contains(kind)));
        assert!(!after.iter().any(|kind| kind.starts_with("naaz")));
    }
}
