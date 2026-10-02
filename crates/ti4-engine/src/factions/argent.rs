//! The Argent Flight (`argent`). See `factions/mod.rs` for the contract and
//! `plans/BASE_FACTIONS_PLAN_2026-10-02.md` for scope; the per-item record is
//! `plans/evidence/BF-argent.md`.
//!
//! Card texts (latest printing, `crates/ti4-content/content/*.json`):
//!
//! * Zeal: "You always vote first during the agenda phase. When you cast at least 1 vote, cast 1
//!   additional vote for each player in the game including you." (PARTIAL: the extra votes only.)
//! * Raid Formation: "When 1 or more of your units uses ANTI-FIGHTER BARRAGE: for each hit
//!   produced in excess of your opponent's fighters, choose 1 of your opponent's ships that has
//!   SUSTAIN DAMAGE to become damaged."
//! * Strike Wing Alpha II (`swa2`): "When this unit uses ANTI-FIGHTER BARRAGE, each result of 9 or
//!   10 also destroys 1 of your opponent's infantry in the space area of the active system."
//! * Strike Wing Ambuscade (`ambuscade`): "When 1 or more of your units make a roll for a unit
//!   ability: Choose 1 of those units to roll 1 additional die. Then, return this card to the
//!   Argent player."
//! * Trrakan Aun Zulok (`argentcommander`): "When 1 or more of your units make a roll for a unit
//!   ability: You may choose 1 of those units to roll 1 additional die." Unlock: "Have 6 units
//!   that have ANTI-FIGHTER BARRAGE, SPACE CANNON, or BOMBARDMENT on the game board."
//!
//! Not implemented (no route; see the evidence file): Zeal's voting order, the flagship's space
//! cannon bar, the mech's "transported" clause, Aerie Hololattice, the agent, the hero and the
//! breakthrough.

use std::sync::Arc;

use ti4_content::ContentStore;
use ti4_content::units::catalogue;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{LeaderId, PlayerId, SystemId};
use ti4_model::state::{GameState, LeaderStatus};
use ti4_model::units::Unit;

use super::hooks_combat::{AfbExcess, CombatHooks};
use super::{FactionModule, Hooks};
use crate::choice::{Choice, ChoiceOption, IllegalChoice};
use crate::decision_context::{DecisionContext, DecisionSource};
use crate::event::Event;
use crate::timing::{Ability, Relation, TimingContext, TimingError};

const FACTION: &str = "argent";
/// The Ambuscade note's id: the printed alias and the Argent player's faction name.
const AMBUSCADE_NOTE: &str = "ambuscade:argent";
/// The only unit whose barrage Strike Wing Alpha II's text belongs to.
const SWA2_UNIT: &str = "argent_destroyer2";

/// What this faction implements.
pub const MODULE: FactionModule = FactionModule {
    alias: FACTION,
    // `zeal` is partial (no voting-order route) and so is not claimed.
    abilities: &["raid_formation"],
    technologies: &["swa2"],
    // `argent_destroyer` is statistics only (verified data-driven by a test); the flagship and
    // mech have clauses with no route yet.
    units: &["argent_destroyer", "argent_destroyer2"],
    promissory: &["ambuscade"],
    leaders: &["argentcommander"],
    breakthroughs: &[],
    hooks: Hooks {
        vote_bonus: Some(vote_bonus),
        commander_unlocked: Some(commander_unlocked),
        timing_abilities: Some(timing_abilities),
        combat: CombatHooks {
            afb_excess: Some(afb_excess),
            ..CombatHooks::NONE
        },
        ..Hooks::NONE
    },
};

// -- small readers -------------------------------------------------------------------------------

fn is_argent(state: &GameState, player: &PlayerId) -> bool {
    state
        .player(player)
        .is_some_and(|seat| seat.faction.as_str() == FACTION)
}

fn decision(state: &GameState, player: &PlayerId, source: &str, subtype: &str) -> DecisionContext {
    DecisionContext::new(
        player.clone(),
        DecisionSource::FactionAbility(source.to_owned()),
        subtype,
        state.phase,
        state.round,
    )
}

fn illegal(error: IllegalChoice) -> TimingError {
    TimingError::IllegalChoice(error)
}

// -- Zeal ----------------------------------------------------------------------------------------

/// Zeal: "When you cast at least 1 vote, cast 1 additional vote for each player in the game
/// including you." `vote::record` adds the bonus only to votes actually cast, so a seat that
/// casts nothing gains nothing.
fn vote_bonus(state: &GameState, player: &PlayerId) -> i64 {
    if is_argent(state, player) {
        i64::try_from(state.players.len()).unwrap_or(0)
    } else {
        0
    }
}

// -- Raid Formation ------------------------------------------------------------------------------

/// Undamaged ships of `owner` in the space area that have SUSTAIN DAMAGE, by type (a damaged ship
/// cannot "become damaged"; two undamaged ships of one type are interchangeable).
fn damageable_types(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    owner: &PlayerId,
    system: &SystemId,
) -> Vec<String> {
    let types = catalogue(content, sources);
    let mut found: Vec<String> = Vec::new();
    for unit in &state.system_state(system).units {
        if &unit.owner != owner || unit.sustained_damage {
            continue;
        }
        let Some(kind) = types.get(unit.type_id.as_str()) else {
            continue;
        };
        let has_sustain = kind.sustain_damage()
            || (crate::relics::grants_sustain(state, owner) && !kind.is_fighter());
        if kind.is_ship()
            && has_sustain
            && !crate::laws::sustain_suppressed(state, kind.base_type())
            && !found.iter().any(|seen| seen == unit.type_id.as_str())
        {
            found.push(unit.type_id.to_string());
        }
    }
    found
}

/// Raid Formation, called for every barrage whose hits outnumbered the target's fighters.
fn afb_excess(context: &mut TimingContext<'_>, site: &AfbExcess<'_>) -> Result<(), IllegalChoice> {
    if !is_argent(context.state, site.producer) {
        return Ok(());
    }
    for _ in 0..site.excess {
        let candidates = damageable_types(
            context.state,
            context.content,
            context.sources,
            site.target,
            site.system,
        );
        let chosen = match candidates.as_slice() {
            [] => return Ok(()),
            [only] => only.clone(),
            many => {
                let options = many
                    .iter()
                    .map(|kind| {
                        ChoiceOption::labelled(
                            kind.clone(),
                            "raid_formation_damage",
                            format!("damage 1 {kind} of {}", site.target),
                        )
                    })
                    .collect();
                let choice = Choice::new(
                    site.producer.clone(),
                    "Raid Formation: choose a ship with SUSTAIN DAMAGE to become damaged"
                        .to_owned(),
                    options,
                )
                .contextualized(decision(
                    context.state,
                    site.producer,
                    "raid_formation",
                    "raid_formation_damage",
                ));
                context.ask_seeing(&choice)?.id
            }
        };
        if let Some(unit) = context
            .state
            .system_mut(site.system)
            .units
            .iter_mut()
            .find(|unit| {
                &unit.owner == site.target
                    && !unit.sustained_damage
                    && unit.type_id.as_str() == chosen
            })
        {
            unit.sustained_damage = true;
        }
    }
    Ok(())
}

// -- the commander's unlock ----------------------------------------------------------------------

/// "Have 6 units that have ANTI-FIGHTER BARRAGE, SPACE CANNON, or BOMBARDMENT on the game board."
fn commander_unlocked(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    _galaxy: Option<&ti4_content::galaxy::Galaxy>,
    player: &PlayerId,
    leader: &LeaderId,
) -> Option<bool> {
    if leader.as_str() != "argentcommander" {
        return None;
    }
    let types = catalogue(content, sources);
    let armed = |unit: &&Unit| {
        &unit.owner == player
            && types.get(unit.type_id.as_str()).is_some_and(|kind| {
                kind.has_anti_fighter_barrage() || kind.has_space_cannon() || kind.has_bombardment()
            })
    };
    let count: usize = state
        .board
        .values()
        .map(|board| {
            board.units.iter().filter(armed).count()
                + board
                    .planet_units
                    .values()
                    .map(|units| units.iter().filter(armed).count())
                    .sum::<usize>()
        })
        .sum();
    Some(count >= 6)
}

// -- timing abilities ----------------------------------------------------------------------------

fn timing_abilities(_state: &GameState, owner_name: &str, seat: &PlayerId) -> Vec<Ability> {
    vec![
        strike_wing(owner_name, seat),
        extra_die(owner_name, seat, Source::Ambuscade),
        extra_die(owner_name, seat, Source::Commander),
    ]
}

fn is_unit_ability_roll(set: &ti4_model::state::RerollSet) -> bool {
    matches!(
        set.kind.as_str(),
        "anti_fighter_barrage" | "space_cannon" | "bombardment"
    )
}

/// Whether the event is `seat`'s own unit-ability roll.
fn own_roll(event: &Event, seat: &PlayerId) -> bool {
    event.text("player") == Some(seat.as_str())
}

/// Strike Wing Alpha II: each 9 or 10 on its barrage dice also destroys 1 enemy infantry in the
/// space area. Hangs on the roll's AFTER window, where the staged faces are final (rerolls done).
/// A mandatory effect: the dice decide, and the only choice is which infantry type when the
/// opponent has several.
fn strike_wing(owner_name: &str, seat: &PlayerId) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    Ability::stateful(
        format!("technology:{owner_name}:swa2:UNIT_ABILITY_ROLLED:after"),
        seat.clone(),
        "UNIT_ABILITY_ROLLED",
        Relation::After,
        Arc::new(move |_event, _resolver, context| strike_wing_effect(context, &owner)),
    )
    .with_stateful_condition(Arc::new(move |event, _, context| {
        own_roll(event, &condition_owner)
            && !strike_wing_victims(context, &condition_owner).is_empty()
    }))
}

/// `(system, opponent)` and the results of 9 or 10 on this seat's staged Alpha II barrage.
fn strike_wing_hits(
    context: &TimingContext<'_>,
    seat: &PlayerId,
) -> Option<(SystemId, PlayerId, usize)> {
    let owns_upgrade = context.state.player(seat).is_some_and(|player| {
        player
            .technologies
            .contains(&ti4_model::id::TechnologyId::new("swa2"))
    });
    let set = context.state.reroll_staging.get(seat)?;
    if !owns_upgrade || set.kind != "anti_fighter_barrage" {
        return None;
    }
    let hits: usize = set
        .rolls
        .iter()
        .filter(|entry| entry.unit == SWA2_UNIT)
        .map(|entry| entry.faces.iter().filter(|face| **face >= 9).count())
        .sum();
    if hits == 0 {
        return None;
    }
    let opponent = crate::combat::opponent_with_ships(
        context.state,
        context.content,
        context.sources,
        seat,
        &set.system,
    )?;
    Some((set.system.clone(), opponent, hits))
}

/// The opponent's infantry in the space area, when the seat's barrage earned a kill.
fn strike_wing_victims(context: &TimingContext<'_>, seat: &PlayerId) -> Vec<Unit> {
    let Some((system, opponent, _)) = strike_wing_hits(context, seat) else {
        return Vec::new();
    };
    let types = catalogue(context.content, context.sources);
    context
        .state
        .system_state(&system)
        .units
        .iter()
        .filter(|unit| {
            unit.owner == opponent
                && types
                    .get(unit.type_id.as_str())
                    .is_some_and(|kind| kind.base_type() == "infantry")
        })
        .cloned()
        .collect()
}

fn strike_wing_effect(context: &mut TimingContext<'_>, seat: &PlayerId) -> Result<(), TimingError> {
    let Some((system, _, hits)) = strike_wing_hits(context, seat) else {
        return Ok(());
    };
    for _ in 0..hits {
        let victims = strike_wing_victims(context, seat);
        let mut kinds: Vec<&Unit> = Vec::new();
        for unit in &victims {
            if !kinds.iter().any(|seen| seen.type_id == unit.type_id) {
                kinds.push(unit);
            }
        }
        let victim = match kinds.as_slice() {
            [] => return Ok(()),
            [only] => (*only).clone(),
            many => {
                let options = many
                    .iter()
                    .map(|unit| {
                        ChoiceOption::labelled(
                            unit.type_id.to_string(),
                            "strike_wing_destroy",
                            format!("destroy 1 {} of {}", unit.type_id, unit.owner),
                        )
                    })
                    .collect();
                let choice = Choice::new(
                    seat.clone(),
                    "Strike Wing Alpha II: choose an infantry to destroy".to_owned(),
                    options,
                )
                .contextualized(decision(
                    context.state,
                    seat,
                    "swa2",
                    "swa2_infantry",
                ));
                let answer = context.ask_seeing(&choice).map_err(illegal)?;
                match many.iter().find(|unit| unit.type_id.as_str() == answer.id) {
                    Some(unit) => (*unit).clone(),
                    None => return Ok(()),
                }
            }
        };
        context
            .state
            .system_mut(&system)
            .remove(std::slice::from_ref(&victim));
    }
    Ok(())
}

/// Which card grants the additional die.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Ambuscade,
    Commander,
}

impl Source {
    const fn card(self) -> &'static str {
        match self {
            Self::Ambuscade => "ambuscade",
            Self::Commander => "argentcommander",
        }
    }
}

/// Whether `seat` may use `source` right now.
fn holds(state: &GameState, seat: &PlayerId, source: Source) -> bool {
    match source {
        Source::Ambuscade => {
            state.promissory_notes.get(AMBUSCADE_NOTE) == Some(seat) && !is_argent(state, seat)
        }
        Source::Commander => {
            is_argent(state, seat)
                && crate::leaders::status(state, seat, &LeaderId::new("argentcommander"))
                    == Some(LeaderStatus::Unlocked)
        }
    }
}

/// The units of `seat`'s staged unit-ability roll that can take a die, one per (type, planet),
/// as indices into the staged rolls. The Metali relic's own dice name no unit and are skipped.
fn rolling_units(state: &GameState, seat: &PlayerId) -> Vec<usize> {
    let Some(set) = state.reroll_staging.get(seat) else {
        return Vec::new();
    };
    if !is_unit_ability_roll(set) {
        return Vec::new();
    }
    let mut seen: Vec<(&str, Option<&ti4_model::id::PlanetId>)> = Vec::new();
    let mut found = Vec::new();
    for (index, entry) in set.rolls.iter().enumerate() {
        if entry.unit_types.is_empty() || entry.hits_on.is_none() {
            continue;
        }
        let key = (entry.unit.as_str(), entry.planet.as_ref());
        if !seen.contains(&key) {
            seen.push(key);
            found.push(index);
        }
    }
    found
}

/// Strike Wing Ambuscade (a note its holder plays) and the Argent commander share one text:
/// "When 1 or more of your units make a roll for a unit ability: choose 1 of those units to roll 1
/// additional die." The die is appended to the staged roll before the hits are read back, so it
/// counts exactly as if the unit had rolled it with the rest.
fn extra_die(owner_name: &str, seat: &PlayerId, source: Source) -> Ability {
    let owner = seat.clone();
    let condition_owner = seat.clone();
    let (kind, id) = match source {
        Source::Ambuscade => ("promissory", "ambuscade"),
        Source::Commander => ("leader", "argentcommander"),
    };
    Ability::stateful(
        format!("{kind}:{owner_name}:{id}:UNIT_ABILITY_ROLLED:when"),
        seat.clone(),
        "UNIT_ABILITY_ROLLED",
        Relation::When,
        Arc::new(move |_event, _resolver, context| extra_die_effect(context, &owner, source)),
    )
    .with_optional(true)
    .with_stateful_condition(Arc::new(move |event, _, context| {
        own_roll(event, &condition_owner)
            && holds(context.state, &condition_owner, source)
            && !rolling_units(context.state, &condition_owner).is_empty()
    }))
}

fn extra_die_effect(
    context: &mut TimingContext<'_>,
    seat: &PlayerId,
    source: Source,
) -> Result<(), TimingError> {
    if !holds(context.state, seat, source) {
        return Ok(());
    }
    let candidates = rolling_units(context.state, seat);
    let index = match candidates.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let Some(set) = context.state.reroll_staging.get(seat) else {
                return Ok(());
            };
            let options = many
                .iter()
                .map(|index| {
                    ChoiceOption::labelled(
                        format!("unit|{index}"),
                        "extra_die",
                        format!("{} rolls 1 additional die", set.rolls[*index].unit),
                    )
                })
                .collect();
            let choice = Choice::new(
                seat.clone(),
                format!("{}: choose a unit to roll 1 additional die", source.card()),
                options,
            )
            .contextualized(decision(
                context.state,
                seat,
                source.card(),
                "extra_die_unit",
            ));
            let answer = context.ask_seeing(&choice).map_err(illegal)?;
            match answer
                .id
                .strip_prefix("unit|")
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| many.contains(n))
            {
                Some(index) => index,
                None => return Ok(()),
            }
        }
    };
    let hits_on = context
        .state
        .reroll_staging
        .get(seat)
        .and_then(|set| set.rolls.get(index))
        .and_then(|entry| entry.hits_on);
    let roll = context
        .dice
        .roll_by(context.rng, 1, "argent additional die", hits_on, seat);
    if let Some(entry) = context
        .state
        .reroll_staging
        .get_mut(seat)
        .and_then(|set| set.rolls.get_mut(index))
    {
        entry.faces.extend(roll.faces);
    }
    if source == Source::Ambuscade {
        crate::promissory::give_back(context.state, AMBUSCADE_NOTE);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::{Scripted, Table};
    use crate::fixtures::{armed_resolver, put, seated_game, with_context};
    use std::collections::{BTreeMap, BTreeSet};
    use ti4_model::content_types::DEFAULT;
    use ti4_model::state::{RerollEntry, RerollSet};

    fn a() -> PlayerId {
        PlayerId::new("a")
    }
    fn b() -> PlayerId {
        PlayerId::new("b")
    }
    fn arena() -> (GameState, SystemId) {
        (
            seated_game(&[("a", "argent"), ("b", "sol")], DEFAULT),
            SystemId::new("18"),
        )
    }
    fn scripted(answers: &[&str]) -> Table {
        Table::with_default(Box::new(Scripted::new(answers.iter().copied())))
    }
    fn damaged(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
        state
            .system_state(system)
            .units
            .iter()
            .filter(|u| &u.owner == owner && u.type_id.as_str() == kind && u.sustained_damage)
            .count()
    }
    fn count(state: &GameState, system: &SystemId, owner: &PlayerId, kind: &str) -> usize {
        state
            .system_state(system)
            .units
            .iter()
            .filter(|u| &u.owner == owner && u.type_id.as_str() == kind)
            .count()
    }
    fn excess(state: &mut GameState, answers: &[&str], who: &PlayerId, hits: usize) {
        let mut table = scripted(answers);
        let target = if who == &a() { b() } else { a() };
        let system = SystemId::new("18");
        with_context(state, DEFAULT, None, &mut table, |ctx| {
            afb_excess(
                ctx,
                &AfbExcess {
                    producer: who,
                    target: &target,
                    system: &system,
                    excess: hits,
                },
            )
        })
        .expect("legal");
    }
    fn stage(state: &mut GameState, player: &PlayerId, kind: &str, rolls: &[(&str, u32, &[u32])]) {
        let system = SystemId::new("18");
        state.reroll_staging.insert(
            player.clone(),
            RerollSet {
                kind: kind.to_owned(),
                system,
                rolls: rolls
                    .iter()
                    .map(|(unit, on, faces)| RerollEntry {
                        unit: (*unit).to_owned(),
                        planet: None,
                        hits_on: Some(*on),
                        faces: faces.to_vec(),
                        rerolled: BTreeSet::new(),
                        deltas: BTreeMap::new(),
                        unit_types: std::iter::once(((*unit).to_owned(), 1)).collect(),
                    })
                    .collect(),
            },
        );
    }
    fn rolled(state: &mut GameState, answers: &[&str], who: &str, kind: &str) {
        let mut resolver = armed_resolver(state);
        let mut table = scripted(answers);
        with_context(state, DEFAULT, None, &mut table, |ctx| {
            let payload = [("player", who), ("system", "18"), ("kind", kind)]
                .iter()
                .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
                .collect();
            let event = ctx
                .event_sequence
                .next("UNIT_ABILITY_ROLLED", payload)
                .expect("event id");
            resolver
                .emit_with_context(ctx, event, |_, _| {})
                .expect("window resolves");
        });
    }
    fn faces(state: &GameState, who: &PlayerId) -> usize {
        state
            .reroll_staging
            .get(who)
            .map_or(0, |set| set.rolls.iter().map(|r| r.faces.len()).sum())
    }

    // -- Zeal ------------------------------------------------------------------------------------

    #[test]
    fn zeal_casts_one_extra_vote_per_player_for_the_argent_seat_only() {
        let state = seated_game(&[("a", "argent"), ("b", "sol"), ("c", "hacan")], DEFAULT);
        assert_eq!(crate::leaders::vote_bonus(&state, &a()), 3);
        assert_eq!(crate::leaders::vote_bonus(&state, &b()), 0);
    }

    // -- Raid Formation --------------------------------------------------------------------------

    #[test]
    fn raid_formation_damages_one_sustain_ship_per_excess_hit() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &b(), 2);
        put(&mut state, &system, "cruiser", &b(), 1);
        excess(&mut state, &[], &a(), 1);
        assert_eq!(damaged(&state, &system, &b(), "dreadnought"), 1);
        // Three excess hits: only the two undamaged sustain ships are left to take them.
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &b(), 2);
        put(&mut state, &system, "cruiser", &b(), 1);
        excess(&mut state, &[], &a(), 3);
        assert_eq!(damaged(&state, &system, &b(), "dreadnought"), 2);
        assert_eq!(damaged(&state, &system, &b(), "cruiser"), 0);
    }

    #[test]
    fn raid_formation_lets_the_producer_choose_between_types() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &b(), 1);
        put(&mut state, &system, "carrier", &b(), 1);
        put(&mut state, &system, "warsun", &b(), 1);
        excess(&mut state, &["warsun"], &a(), 1);
        assert_eq!(damaged(&state, &system, &b(), "warsun"), 1);
        assert_eq!(damaged(&state, &system, &b(), "dreadnought"), 0);
    }

    #[test]
    fn raid_formation_needs_the_ability_and_an_undamaged_sustain_ship() {
        let (mut state, system) = arena();
        put(&mut state, &system, "dreadnought", &a(), 1);
        // Sol's barrage has no Raid Formation.
        excess(&mut state, &[], &b(), 1);
        assert_eq!(damaged(&state, &system, &a(), "dreadnought"), 0);
        // No sustain ships (fighters, cruisers) and already-damaged ships are not targets.
        put(&mut state, &system, "fighter", &b(), 2);
        put(&mut state, &system, "cruiser", &b(), 1);
        excess(&mut state, &[], &a(), 2);
        assert!(
            state
                .system_state(&system)
                .units
                .iter()
                .all(|u| !u.sustained_damage)
        );
        let mut hurt = Unit::new(ti4_model::id::UnitTypeId::new("dreadnought"), b());
        hurt.sustained_damage = true;
        state.system_mut(&system).units.push(hurt);
        excess(&mut state, &[], &a(), 2);
        assert_eq!(
            damaged(&state, &system, &b(), "dreadnought"),
            1,
            "still the one"
        );
    }

    // -- Strike Wing Alpha II --------------------------------------------------------------------

    fn swa2_arena() -> (GameState, SystemId) {
        let (mut state, system) = arena();
        state
            .player_mut(&a())
            .unwrap()
            .technologies
            .insert(ti4_model::id::TechnologyId::new("swa2"));
        put(&mut state, &system, "argent_destroyer2", &a(), 1);
        put(&mut state, &system, "cruiser", &b(), 1);
        put(&mut state, &system, "infantry", &b(), 3);
        (state, system)
    }

    #[test]
    fn each_nine_or_ten_destroys_an_infantry_in_the_space_area() {
        let (mut state, system) = swa2_arena();
        stage(
            &mut state,
            &a(),
            "anti_fighter_barrage",
            &[(SWA2_UNIT, 6, &[9, 10, 4])],
        );
        rolled(&mut state, &[], "a", "anti_fighter_barrage");
        assert_eq!(count(&state, &system, &b(), "infantry"), 1);
        assert_eq!(
            count(&state, &system, &b(), "cruiser"),
            1,
            "ships are untouched"
        );
    }

    #[test]
    fn alpha_ii_stops_at_the_last_infantry_and_ignores_low_dice_and_the_base_unit() {
        let (mut state, system) = swa2_arena();
        stage(
            &mut state,
            &a(),
            "anti_fighter_barrage",
            &[(SWA2_UNIT, 6, &[10, 10, 9, 9, 9])],
        );
        rolled(&mut state, &[], "a", "anti_fighter_barrage");
        assert_eq!(count(&state, &system, &b(), "infantry"), 0);

        let (mut state, system) = swa2_arena();
        stage(
            &mut state,
            &a(),
            "anti_fighter_barrage",
            &[(SWA2_UNIT, 6, &[8, 6, 1])],
        );
        rolled(&mut state, &[], "a", "anti_fighter_barrage");
        assert_eq!(count(&state, &system, &b(), "infantry"), 3, "no 9 or 10");

        stage(
            &mut state,
            &a(),
            "anti_fighter_barrage",
            &[("argent_destroyer", 9, &[10, 10])],
        );
        rolled(&mut state, &[], "a", "anti_fighter_barrage");
        assert_eq!(
            count(&state, &system, &b(), "infantry"),
            3,
            "the first printing has no clause"
        );

        stage(&mut state, &a(), "space_cannon", &[(SWA2_UNIT, 6, &[10])]);
        rolled(&mut state, &[], "a", "space_cannon");
        assert_eq!(count(&state, &system, &b(), "infantry"), 3, "barrage only");
    }

    #[test]
    fn alpha_ii_does_not_touch_infantry_on_planets_or_the_owners_own() {
        let (mut state, system) = swa2_arena();
        put(&mut state, &system, "infantry", &a(), 2);
        stage(
            &mut state,
            &a(),
            "anti_fighter_barrage",
            &[(SWA2_UNIT, 6, &[10])],
        );
        rolled(&mut state, &[], "a", "anti_fighter_barrage");
        assert_eq!(count(&state, &system, &a(), "infantry"), 2);
        assert_eq!(count(&state, &system, &b(), "infantry"), 2);
    }

    #[test]
    fn the_first_strike_wing_has_the_printed_statistics() {
        let content = ContentStore::embedded();
        let types = catalogue(content, DEFAULT);
        let one = types["argent_destroyer"];
        assert_eq!((one.afb_hits_on(), one.afb_dice()), (Some(9), 2));
        assert_eq!(one.combat_hits_on(), Some(8));
        let two = types["argent_destroyer2"];
        assert_eq!((two.afb_hits_on(), two.afb_dice()), (Some(6), 3));
        assert_eq!(two.combat_hits_on(), Some(7));
        assert_eq!(two.capacity(), 1);
    }

    // -- Ambuscade and the commander -------------------------------------------------------------

    fn holder_game() -> GameState {
        let mut state = seated_game(&[("a", "sol"), ("b", "argent")], DEFAULT);
        crate::promissory::take(&mut state, ContentStore::embedded(), &a(), AMBUSCADE_NOTE);
        state
    }

    #[test]
    fn ambuscade_adds_one_die_to_a_chosen_unit_and_returns_to_argent() {
        let mut state = holder_game();
        assert_eq!(state.promissory_notes.get(AMBUSCADE_NOTE), Some(&a()));
        stage(
            &mut state,
            &a(),
            "space_cannon",
            &[("pds", 6, &[7]), ("pds2", 5, &[3])],
        );
        rolled(
            &mut state,
            &[
                "promissory:sol:ambuscade:UNIT_ABILITY_ROLLED:when",
                "unit|1",
            ],
            "a",
            "space_cannon",
        );
        let set = state.reroll_staging.get(&a()).unwrap();
        assert_eq!(set.rolls[0].faces.len(), 1);
        assert_eq!(
            set.rolls[1].faces.len(),
            2,
            "the chosen unit rolled one more"
        );
        assert_ne!(
            state.promissory_notes.get(AMBUSCADE_NOTE),
            Some(&a()),
            "returned"
        );
    }

    #[test]
    fn ambuscade_may_be_declined_and_needs_a_unit_ability_roll() {
        let mut state = holder_game();
        stage(&mut state, &a(), "space_cannon", &[("pds", 6, &[7])]);
        rolled(&mut state, &["decline"], "a", "space_cannon");
        assert_eq!(faces(&state, &a()), 1);
        assert_eq!(
            state.promissory_notes.get(AMBUSCADE_NOTE),
            Some(&a()),
            "kept"
        );
        // A ground roll is not a unit ability roll; the other player's roll is not the holder's.
        stage(&mut state, &a(), "ground", &[("infantry", 8, &[7])]);
        rolled(&mut state, &[], "a", "ground");
        stage(&mut state, &b(), "space_cannon", &[("pds", 6, &[7])]);
        rolled(&mut state, &[], "b", "space_cannon");
        assert_eq!(faces(&state, &a()), 1);
        assert_eq!(faces(&state, &b()), 1);
        assert_eq!(state.promissory_notes.get(AMBUSCADE_NOTE), Some(&a()));
    }

    #[test]
    fn the_commander_adds_a_die_only_once_unlocked_and_keeps_no_card_to_return() {
        let (mut state, _) = arena();
        stage(&mut state, &a(), "bombardment", &[("dreadnought", 5, &[7])]);
        rolled(
            &mut state,
            &["leader:argent:argentcommander:UNIT_ABILITY_ROLLED:when"],
            "a",
            "bombardment",
        );
        assert_eq!(faces(&state, &a()), 1, "locked");
        state
            .player_mut(&a())
            .unwrap()
            .leaders
            .insert(LeaderId::new("argentcommander"), LeaderStatus::Unlocked);
        rolled(
            &mut state,
            &["leader:argent:argentcommander:UNIT_ABILITY_ROLLED:when"],
            "a",
            "bombardment",
        );
        assert_eq!(faces(&state, &a()), 2);
        rolled(&mut state, &["decline"], "a", "bombardment");
        assert_eq!(faces(&state, &a()), 2, "declined");
        assert!(
            state
                .player(&a())
                .unwrap()
                .leaders
                .get(&LeaderId::new("argentcommander"))
                == Some(&LeaderStatus::Unlocked)
        );
    }

    #[test]
    fn the_commander_unlocks_with_six_armed_units() {
        let (mut state, system) = arena();
        let content = ContentStore::embedded();
        let leader = LeaderId::new("argentcommander");
        let check =
            |state: &GameState| commander_unlocked(state, content, DEFAULT, None, &a(), &leader);
        let base = check(&state).map(|_| ()).and(Some(()));
        assert_eq!(base, Some(()));
        // Home units may already count; measure from what is there.
        let have = {
            let types = catalogue(content, DEFAULT);
            state
                .board
                .values()
                .flat_map(|board| {
                    board
                        .units
                        .iter()
                        .chain(board.planet_units.values().flatten())
                })
                .filter(|u| u.owner == a())
                .filter(|u| {
                    types.get(u.type_id.as_str()).is_some_and(|k| {
                        k.has_anti_fighter_barrage() || k.has_space_cannon() || k.has_bombardment()
                    })
                })
                .count()
        };
        assert!(have < 6);
        put(&mut state, &system, "argent_destroyer", &a(), 5 - have);
        assert_eq!(check(&state), Some(false), "five");
        put(&mut state, &system, "argent_destroyer", &a(), 1);
        assert_eq!(check(&state), Some(true), "six");
        // Another player's units do not count, and another leader is not this module's.
        assert_eq!(
            commander_unlocked(&state, content, DEFAULT, None, &b(), &leader),
            Some(false)
        );
        assert_eq!(
            commander_unlocked(
                &state,
                content,
                DEFAULT,
                None,
                &a(),
                &LeaderId::new("argentagent")
            ),
            None
        );
    }

    // -- no Argent, no Argent abilities ----------------------------------------------------------

    #[test]
    fn a_game_without_an_argent_seat_is_offered_no_argent_ability() {
        let (decider, seen) = crate::choice::Capturing::new(Box::new(crate::choice::FirstOption));
        let mut table = Table::with_default(Box::new(decider));
        let mut state = seated_game(&[("a", "hacan"), ("b", "sol")], DEFAULT);
        let system = SystemId::new("18");
        put(&mut state, &system, "argent_destroyer2", &a(), 1);
        put(&mut state, &system, "infantry", &b(), 2);
        put(&mut state, &system, "dreadnought", &b(), 1);
        for who in ["a", "b"] {
            for kind in ["anti_fighter_barrage", "space_cannon", "bombardment"] {
                stage(
                    &mut state,
                    &PlayerId::new(who),
                    kind,
                    &[(SWA2_UNIT, 6, &[10, 9])],
                );
                let mut resolver = armed_resolver(&state);
                with_context(&mut state, DEFAULT, None, &mut table, |ctx| {
                    let payload = [("player", who), ("system", "18"), ("kind", kind)]
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), serde_json::Value::from(*v)))
                        .collect();
                    let event = ctx
                        .event_sequence
                        .next("UNIT_ABILITY_ROLLED", payload)
                        .unwrap();
                    resolver.emit_with_context(ctx, event, |_, _| {}).unwrap();
                });
            }
        }
        assert_eq!(count(&state, &system, &b(), "infantry"), 2);
        assert_eq!(faces(&state, &a()), 2);
        assert_eq!(vote_bonus(&state, &a()), 0);
        let asked: Vec<String> = seen
            .borrow()
            .iter()
            .filter(|c| {
                c.ids()
                    .iter()
                    .any(|id| id.contains("argent") || id.contains("ambuscade"))
            })
            .map(|c| c.prompt.clone())
            .collect();
        assert!(asked.is_empty(), "{asked:?}");
        excess(&mut state, &[], &a(), 2);
        assert_eq!(damaged(&state, &system, &b(), "dreadnought"), 0);
    }
}
