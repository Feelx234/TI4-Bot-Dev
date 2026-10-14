//! Battle odds from rollouts of [`arena`]: the request and response shapes, the validation of a
//! request, and the rollout loops. `ti4-advisor` serves these over HTTP.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::{POK, SourceSet};
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::arena::{self, GroundSide, Side};

pub const MAX_PLACEMENTS: usize = 256;
pub const DEFAULT_SIMULATIONS: usize = 2000;
pub const MAX_SIMULATIONS: usize = 50_000;

/// Why a request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcError(pub String);

impl CalcError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CalcError {}

/// Units count input, accepting either `{"unit_id": count}` or `[["unit_id", count]]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UnitCounts {
    Map(BTreeMap<String, usize>),
    List(Vec<(String, usize)>),
}

impl UnitCounts {
    #[must_use]
    pub fn into_entries(self) -> Vec<(String, usize)> {
        match self {
            Self::Map(map) => map.into_iter().collect(),
            Self::List(list) => list,
        }
    }
}

/// A participant in space combat, specified either by a player ID or custom side parameters.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ParticipantInput {
    Player(PlayerId),
    Side(SideInput),
}

/// Fleet configuration for one participant in space combat.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideInput {
    /// Player ID whose fleet/guns at the combat system should be included.
    #[serde(default)]
    pub player: Option<PlayerId>,
    /// Faction identifier (e.g. "sol", "letnev", "jolnar"). If omitted and `player` is given, read from state.
    #[serde(default)]
    pub faction: Option<String>,
    /// Ships to field as `{"unit_id": count}` or `[["unit_id", count]]`. Added to units from state.
    #[serde(default)]
    pub units: Option<UnitCounts>,
    /// Ships that start damaged as `{"unit_id": count}` or `[["unit_id", count]]`.
    #[serde(default)]
    pub damaged: Option<UnitCounts>,
    /// Units firing space cannon before combat as `{"unit_id": count}` or `[["unit_id", count]]`.
    #[serde(default)]
    pub guns: Option<UnitCounts>,
}

/// Outcome statistics from [`arena`] rollouts.
#[derive(Debug, Clone, Serialize)]
pub struct BattleResponse {
    pub simulations: usize,
    pub attacker_win_rate: f64,
    pub defender_win_rate: f64,
    pub mutual_destruction_rate: f64,
    pub unresolved_rate: f64,
    pub average_rounds: f64,
    pub attacker_expected_survivors: BTreeMap<String, f64>,
    pub defender_expected_survivors: BTreeMap<String, f64>,
    pub attacker_fielded: BTreeMap<String, usize>,
    pub defender_fielded: BTreeMap<String, usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundOddsSide {
    pub faction: String,
    pub units: BTreeMap<String, usize>,
    #[serde(default)]
    pub damaged: BTreeMap<String, usize>,
    #[serde(default)]
    pub guns: BTreeMap<String, usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundOddsRequest {
    pub attacker: GroundOddsSide,
    pub defender: GroundOddsSide,
    /// Legal bombarding ships that fire Harrow after rounds (never before landing).
    #[serde(default)]
    pub harrow: BTreeMap<String, usize>,
    #[serde(default)]
    pub simulations: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct GroundOddsResponse {
    pub simulations: usize,
    pub attacker_win_rate: f64,
}

#[allow(clippy::too_many_lines)]
/// One side of a space combat: the ships and guns of `participant`, read from `state` at
/// `system` when a player is named, plus the units the request adds. Returns the side and the
/// ships it fields by unit id.
///
/// # Errors
///
/// Returns a [`CalcError`] if the player, the state or a unit id is missing or unknown.
pub fn resolve_side(
    participant: &ParticipantInput,
    state: Option<&GameState>,
    system: Option<&SystemId>,
    galaxy: Option<&Galaxy>,
    content: &ContentStore,
    sources: SourceSet,
) -> Result<(Side, BTreeMap<String, usize>), CalcError> {
    let (player_id, explicit_faction, extra_units, extra_damaged, extra_guns) = match participant {
        ParticipantInput::Player(pid) => (Some(pid), None, Vec::new(), Vec::new(), Vec::new()),
        ParticipantInput::Side(side) => (
            side.player.as_ref(),
            side.faction.as_deref(),
            side.units
                .clone()
                .map(UnitCounts::into_entries)
                .unwrap_or_default(),
            side.damaged
                .clone()
                .map(UnitCounts::into_entries)
                .unwrap_or_default(),
            side.guns
                .clone()
                .map(UnitCounts::into_entries)
                .unwrap_or_default(),
        ),
    };

    let mut fleet: BTreeMap<String, usize> = BTreeMap::new();
    let mut damaged: BTreeMap<String, usize> = BTreeMap::new();
    let mut guns: BTreeMap<String, usize> = BTreeMap::new();

    let faction: String = if let Some(pid) = player_id {
        let game_state =
            state.ok_or_else(|| CalcError::new("state is required when player is specified"))?;
        let seat = game_state
            .player(pid)
            .ok_or_else(|| CalcError::new(format!("player {pid} not found in state")))?;
        let player_faction = explicit_faction.unwrap_or(seat.faction.as_str());

        if let Some(sys) = system {
            let board = game_state.system_state(sys);
            for unit in &board.units {
                if &unit.owner == pid
                    && let Some(kind) =
                        ti4_content::units::unit_type(content, unit.type_id.as_str(), sources)
                    && kind.is_ship()
                {
                    *fleet.entry(unit.type_id.to_string()).or_default() += 1;
                    if unit.sustained_damage {
                        *damaged.entry(unit.type_id.to_string()).or_default() += 1;
                    }
                }
            }
            for unit in board.planet_units.values().flatten() {
                if &unit.owner == pid
                    && let Some(kind) =
                        ti4_content::units::unit_type(content, unit.type_id.as_str(), sources)
                    && kind.has_space_cannon()
                {
                    *guns.entry(unit.type_id.to_string()).or_default() += 1;
                }
            }
            if let Some(g) = galaxy {
                for adjacent_id in g.adjacent(sys.as_str()) {
                    let adj_sys = SystemId::new(adjacent_id);
                    let adj_board = game_state.system_state(&adj_sys);
                    let adj_units = adj_board
                        .planet_units
                        .values()
                        .flatten()
                        .chain(&adj_board.units);
                    for unit in adj_units {
                        if &unit.owner == pid {
                            let reaches = matches!(
                                unit.type_id.as_str(),
                                "pds2" | "xxcha_mech" | "xxcha_flagship"
                            ) || ti4_content::units::unit_type(
                                content,
                                unit.type_id.as_str(),
                                sources,
                            )
                            .and_then(|k| k.record().text("ability"))
                            .is_some_and(|ability| {
                                let a = ability.to_ascii_lowercase();
                                a.contains("space cannon against ships that are")
                                    && a.contains("adjacent")
                            });
                            if reaches {
                                *guns.entry(unit.type_id.to_string()).or_default() += 1;
                            }
                        }
                    }
                }
            }
        }
        player_faction.to_owned()
    } else {
        explicit_faction.unwrap_or("generic").to_owned()
    };

    for (id, count) in extra_units {
        *fleet.entry(id).or_default() += count;
    }
    for (id, count) in extra_damaged {
        *damaged.entry(id).or_default() += count;
    }
    for (id, count) in extra_guns {
        *guns.entry(id).or_default() += count;
    }

    // Validate unit IDs
    for (id, &count) in &fleet {
        if count > 0 && ti4_content::units::unit_type(content, id, sources).is_none() {
            return Err(CalcError::new(format!("unknown unit id in fleet: {id}")));
        }
    }
    for (id, &count) in &damaged {
        if count > 0 && ti4_content::units::unit_type(content, id, sources).is_none() {
            return Err(CalcError::new(format!("unknown unit id in damaged: {id}")));
        }
    }
    for (id, &count) in &guns {
        if count > 0 && ti4_content::units::unit_type(content, id, sources).is_none() {
            return Err(CalcError::new(format!("unknown unit id in guns: {id}")));
        }
    }

    let fleet_vec: Vec<(String, usize)> =
        fleet.clone().into_iter().filter(|(_, n)| *n > 0).collect();
    let damaged_vec: Vec<(String, usize)> = damaged.into_iter().filter(|(_, n)| *n > 0).collect();
    let guns_vec: Vec<(String, usize)> = guns.into_iter().filter(|(_, n)| *n > 0).collect();

    let side =
        Side::of(content, &fleet_vec, &damaged_vec, &faction, true).with_guns(content, &guns_vec);

    Ok((side, fleet))
}

/// How the rollouts of a space combat run. Each field is as in the request of the advisor.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpaceOptions {
    /// Number of rollout simulations (defaults to 2000, capped at 50000).
    pub simulations: Option<usize>,
    /// Whether attacker's space cannon fires before combat (defaults to true).
    pub attacker_cannon: Option<bool>,
    /// Whether combat is already in progress (bypasses space cannon and round-1 AFB).
    pub in_progress: Option<bool>,
    /// Optional seed for deterministic simulations.
    pub seed: Option<u64>,
}

/// Space combat odds from [`arena`] rollouts. Each side is what [`resolve_side`] returns.
///
/// # Errors
///
/// Returns a [`CalcError`] if the number of simulations is out of range.
#[allow(clippy::cast_precision_loss)]
pub fn space_odds(
    attacker: (Side, BTreeMap<String, usize>),
    defender: (Side, BTreeMap<String, usize>),
    options: SpaceOptions,
) -> Result<BattleResponse, CalcError> {
    let (attacker_side, attacker_fielded) = attacker;
    let (defender_side, defender_fielded) = defender;

    let simulations = match options.simulations {
        Some(0) => return Err(CalcError::new("simulations must be at least 1")),
        Some(n) if n > MAX_SIMULATIONS => {
            return Err(CalcError::new(format!(
                "simulations cannot exceed {MAX_SIMULATIONS}"
            )));
        }
        Some(n) => n,
        None => DEFAULT_SIMULATIONS,
    };

    let attacker_cannon = options.attacker_cannon.unwrap_or(true);
    let in_progress = options.in_progress.unwrap_or(false);
    let base_seed = options.seed.unwrap_or(0);

    let mut attacker_wins = 0usize;
    let mut defender_wins = 0usize;
    let mut mutual_destruction = 0usize;
    let mut unresolved = 0usize;
    let mut total_rounds = 0u64;

    let mut attacker_survivors = vec![0u64; attacker_side.names().len()];
    let mut defender_survivors = vec![0u64; defender_side.names().len()];

    for i in 0..simulations {
        let seed = base_seed.wrapping_add(i as u64);
        let outcome = if in_progress {
            arena::fight_in_progress(&attacker_side, &defender_side, seed)
        } else {
            arena::fight_outcome(&attacker_side, &defender_side, seed, attacker_cannon)
        };

        if outcome.unresolved {
            unresolved += 1;
        }
        match outcome.winner {
            Some("a") => attacker_wins += 1,
            Some("b") => defender_wins += 1,
            _ => mutual_destruction += 1,
        }
        total_rounds += u64::from(outcome.rounds);

        for (idx, &count) in outcome.attacker_left.iter().enumerate() {
            attacker_survivors[idx] += count as u64;
        }
        for (idx, &count) in outcome.defender_left.iter().enumerate() {
            defender_survivors[idx] += count as u64;
        }
    }

    let sim_f64 = simulations as f64;
    let mut attacker_expected_survivors = BTreeMap::new();
    for (idx, name) in attacker_side.names().iter().enumerate() {
        attacker_expected_survivors.insert(name.clone(), attacker_survivors[idx] as f64 / sim_f64);
    }
    let mut defender_expected_survivors = BTreeMap::new();
    for (idx, name) in defender_side.names().iter().enumerate() {
        defender_expected_survivors.insert(name.clone(), defender_survivors[idx] as f64 / sim_f64);
    }

    Ok(BattleResponse {
        simulations,
        attacker_win_rate: attacker_wins as f64 / sim_f64,
        defender_win_rate: defender_wins as f64 / sim_f64,
        mutual_destruction_rate: mutual_destruction as f64 / sim_f64,
        unresolved_rate: unresolved as f64 / sim_f64,
        average_rounds: total_rounds as f64 / sim_f64,
        attacker_expected_survivors,
        defender_expected_survivors,
        attacker_fielded,
        defender_fielded,
    })
}

/// Ground combat odds from [`arena`] rollouts.
///
/// # Errors
///
/// Returns a [`CalcError`] if a count, a unit id or the number of simulations is invalid.
#[allow(clippy::cast_precision_loss)]
pub fn ground_odds(
    content: &ContentStore,
    request: &GroundOddsRequest,
) -> Result<GroundOddsResponse, CalcError> {
    let n = request.simulations.unwrap_or(DEFAULT_SIMULATIONS);
    if n == 0 || n > MAX_SIMULATIONS {
        return Err(CalcError::new("simulations must be between 1 and 50000"));
    }
    if !request.attacker.guns.is_empty() {
        return Err(CalcError::new(
            "only the defender has standing defense guns",
        ));
    }
    if request.harrow.len() > MAX_PLACEMENTS
        || request.harrow.values().any(|count| *count > MAX_PLACEMENTS)
        || request.harrow.values().sum::<usize>() > MAX_PLACEMENTS
    {
        return Err(CalcError::new("too many Harrow units"));
    }
    for id in request.harrow.keys() {
        if !ti4_content::units::unit_type(content, id, POK)
            .is_some_and(|unit| unit.has_bombardment() && unit.bombard_dice() > 0)
        {
            return Err(CalcError::new(format!("invalid Harrow unit: {id}")));
        }
    }
    let resolve = |side: &GroundOddsSide| -> Result<GroundSide, CalcError> {
        if side.units.len() > MAX_PLACEMENTS
            || side.guns.len() > MAX_PLACEMENTS
            || side.units.values().any(|count| *count > MAX_PLACEMENTS)
            || side.guns.values().any(|count| *count > MAX_PLACEMENTS)
            || side.units.values().sum::<usize>() > MAX_PLACEMENTS
            || side.guns.values().sum::<usize>() > MAX_PLACEMENTS
        {
            return Err(CalcError::new("too many ground units"));
        }
        for (id, count) in &side.units {
            if *count > 0
                && !ti4_content::units::unit_type(content, id, POK)
                    .is_some_and(|unit| unit.is_ground_force())
            {
                return Err(CalcError::new(format!("invalid ground force: {id}")));
            }
            if side.damaged.get(id).copied().unwrap_or(0) > *count {
                return Err(CalcError::new("damaged count exceeds force count"));
            }
        }
        if side.damaged.keys().any(|id| !side.units.contains_key(id)) {
            return Err(CalcError::new("damaged unit is not fielded"));
        }
        for id in side.guns.keys() {
            if !ti4_content::units::unit_type(content, id, POK)
                .is_some_and(|unit| unit.space_cannon_hits_on().is_some())
            {
                return Err(CalcError::new(format!("invalid defense gun: {id}")));
            }
        }
        let forces: Vec<_> = side.units.iter().map(|(id, n)| (id.clone(), *n)).collect();
        let damaged: Vec<_> = side
            .damaged
            .iter()
            .map(|(id, n)| (id.clone(), *n))
            .collect();
        let guns: Vec<_> = side.guns.iter().map(|(id, n)| (id.clone(), *n)).collect();
        Ok(GroundSide::of(content, &forces, &damaged, &side.faction)
            .with_defense_guns(content, &guns))
    };
    let harrow: Vec<_> = request
        .harrow
        .iter()
        .map(|(id, count)| (id.clone(), *count))
        .collect();
    let attacker = resolve(&request.attacker)?.with_bombardment(content, &harrow, false, true);
    let defender = resolve(&request.defender)?;
    let wins = (0..n)
        .filter(|seed| arena::ground_fight(&attacker, &defender, *seed as u64).winner == Some("a"))
        .count();
    Ok(GroundOddsResponse {
        simulations: n,
        attacker_win_rate: wins as f64 / n as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn odds(
        state: Option<&GameState>,
        system: Option<&SystemId>,
        attacker: &ParticipantInput,
        defender: &ParticipantInput,
        options: SpaceOptions,
    ) -> Result<BattleResponse, CalcError> {
        let content = ContentStore::embedded();
        let attacker = resolve_side(attacker, state, system, None, content, POK)?;
        let defender = resolve_side(defender, state, system, None, content, POK)?;
        space_odds(attacker, defender, options)
    }

    fn side(faction: &str, units: UnitCounts) -> ParticipantInput {
        ParticipantInput::Side(SideInput {
            faction: Some(faction.to_owned()),
            units: Some(units),
            ..Default::default()
        })
    }

    #[test]
    fn direct_fleet_battle_simulation_computes_odds() {
        let attacker = side(
            "sol",
            UnitCounts::List(vec![("carrier".to_owned(), 1), ("fighter".to_owned(), 4)]),
        );
        let defender = side(
            "letnev",
            UnitCounts::Map([("dreadnought".to_owned(), 1)].into_iter().collect()),
        );
        let options = SpaceOptions {
            simulations: Some(100),
            seed: Some(42),
            ..Default::default()
        };

        let response = odds(None, None, &attacker, &defender, options).expect("battle succeeds");
        assert_eq!(response.simulations, 100);
        let total_rate = response.attacker_win_rate
            + response.defender_win_rate
            + response.mutual_destruction_rate;
        assert!((total_rate - 1.0).abs() < 1e-6);
        assert_eq!(response.attacker_fielded.get("carrier"), Some(&1));
        assert_eq!(response.attacker_fielded.get("fighter"), Some(&4));
        assert_eq!(response.defender_fielded.get("dreadnought"), Some(&1));
        assert!(response.average_rounds > 0.0);
    }

    #[test]
    fn battle_from_game_state_snapshot_resolves_fleets() {
        let mut state = ti4_engine::fixtures::game(&["a", "b"]);
        let sys = SystemId::new("18");
        let a_player = PlayerId::new("a");
        let b_player = PlayerId::new("b");

        for (unit, owner) in [
            ("cruiser", &a_player),
            ("cruiser", &a_player),
            ("destroyer", &b_player),
        ] {
            state
                .system_mut(&sys)
                .units
                .push(ti4_model::units::Unit::new(
                    ti4_model::id::UnitTypeId::new(unit),
                    owner.clone(),
                ));
        }

        let options = SpaceOptions {
            simulations: Some(50),
            seed: Some(123),
            ..Default::default()
        };
        let response = odds(
            Some(&state),
            Some(&sys),
            &ParticipantInput::Player(a_player),
            &ParticipantInput::Player(b_player),
            options,
        )
        .expect("state battle succeeds");
        assert_eq!(response.simulations, 50);
        assert_eq!(response.attacker_fielded.get("cruiser"), Some(&2));
        assert_eq!(response.defender_fielded.get("destroyer"), Some(&1));
        assert!(response.attacker_win_rate > response.defender_win_rate);
    }

    #[test]
    fn battle_validation_rejects_invalid_inputs() {
        let fighter = || UnitCounts::List(vec![("fighter".to_owned(), 1)]);
        let attacker = side("sol", fighter());
        let defender = side("letnev", fighter());
        let simulations = |n| SpaceOptions {
            simulations: Some(n),
            ..Default::default()
        };

        // 0 simulations rejected
        assert!(odds(None, None, &attacker, &defender, simulations(0)).is_err());

        // Simulations exceeding MAX_SIMULATIONS rejected
        assert!(
            odds(
                None,
                None,
                &attacker,
                &defender,
                simulations(MAX_SIMULATIONS + 1)
            )
            .is_err()
        );

        // Unknown unit ID rejected
        let bad_unit = side(
            "sol",
            UnitCounts::List(vec![("nonexistent_ship".to_owned(), 1)]),
        );
        assert!(odds(None, None, &bad_unit, &defender, simulations(10)).is_err());

        // Player specified without state rejected
        let no_state = ParticipantInput::Player(PlayerId::new("missing_player"));
        assert!(odds(None, None, &no_state, &defender, simulations(10)).is_err());
    }

    #[test]
    fn ground_odds_counts_the_wins_of_the_invader() {
        let force = |faction: &str, count| GroundOddsSide {
            faction: faction.to_owned(),
            units: [("infantry".to_owned(), count)].into_iter().collect(),
            damaged: BTreeMap::new(),
            guns: BTreeMap::new(),
        };
        let mut request = GroundOddsRequest {
            attacker: force("sol", 4),
            defender: force("letnev", 1),
            harrow: BTreeMap::new(),
            simulations: Some(200),
        };
        let content = ContentStore::embedded();
        let response = ground_odds(content, &request).expect("ground odds");
        assert_eq!(response.simulations, 200);
        assert!(response.attacker_win_rate > 0.5);

        request.simulations = Some(0);
        assert!(ground_odds(content, &request).is_err());
    }
}
