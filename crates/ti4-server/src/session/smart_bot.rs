//! The smart bot: the random bot ([`crate::session::random_bot`]) with an opinion on the board.
//!
//! # Plan
//!
//! A `BotSmart` seat is a [`crate::session::random_bot::RandomBotDecider`] in smart mode. For
//! every question it first draws the random bot's pick (so the RNG stream matches), then asks
//! [`pick`] here. `pick` returns `Some(index)` only for the questions it has a simple opinion on,
//! and `None` for everything else, which keeps the random pick. An index is always one of the
//! offered options, so the bot can never answer illegally or stall a game the random bot would
//! finish. It reads only the seat's [`SeatObservation`] (public board plus its own hand), and is
//! deterministic: the same position and seed give the same answer.
//!
//! The opinions, deliberately few:
//!
//! - **Score** an objective whenever the engine offers one.
//! - **Action phase**: take a tactical action when some system is worth activating (below),
//!   sometimes the strategic action instead so the card still gets played.
//! - **Activation**: value each system as the planets it can take (resources + influence, Mecatol
//!   Rex extra while the custodians point is there to win), counting only the planets its
//!   reachable carriers can bring enough ground forces to; a hostile fleet must be beaten clearly
//!   (fleet-strength matchup, [`ti4_policy::fleet_strength`]) or the system is avoided. A home
//!   system with a space dock is worth activating to build when the seat is short of units.
//! - **Movement**: into a fight, move the strongest ships until the matchup is clearly won; move a
//!   carrier while the target's planets still need ground forces; then stop. Never move into a
//!   fight it cannot win. **Loading**: ground forces while planets need them, fighters for a fight.
//! - **Landing**: commit ground forces to the most valuable planet still short of a winning
//!   force, never to a planet it cannot outnumber; stop when every planet is covered.
//! - **Retreat** from a space combat it is clearly losing, otherwise stay.
//! - **Production**: buy the most useful unit per resource it can pay for (per production slot
//!   when resources are plentiful): ground forces and a carrier when it lacks them, warships
//!   otherwise, without exceeding fleet supply or capacity. No more ground forces once more wait
//!   at its docks than its ships can carry; stop when nothing useful is left.
//! - **Victory points**: score whenever offered, including Imperial's extra objective, and always
//!   pay to remove the custodians token.
//! - **Command tokens**: a new token goes to the fleet pool until it holds three (or enough for
//!   its biggest ship stack), then to the smaller of the tactic and strategy pools; a
//!   redistribution aims at the same arrangement. Buy Leadership tokens with spare influence
//!   (keeping six for the custodians while they stand). Always follow Technology and Imperial.
//! - **Payment**: exhaust the planet that wastes least.
//! - **Trades**: accept an offer only when its engine-estimated value to the bot is slightly
//!   positive; propose only offers that gain for both sides; never counter.
//! - **Over fleet supply**: remove the cheapest unit.
//!
//! Strategy cards, technology choice, agendas, action cards, the other secondaries and every
//! other window stay random.

use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use serde_json::Value;
use ti4_content::units::UnitType;
use ti4_engine::choice::{Choice, ChoiceOption, Observed, SeatObservation};
use ti4_engine::production::Spend;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::units::Unit;
use ti4_policy::fleet_strength::{Descriptors, descriptors, matchup};

/// A fight is taken only when the strength matchup (`ln` of the Lanchester strength ratio) is at
/// least this: roughly a 1.6x edge, "clearly winning" rather than a coin flip.
pub const WIN_MARGIN: f64 = 0.5;
/// A space combat is retreated from when the matchup is this bad.
pub const RETREAT_MARGIN: f64 = -0.5;
/// Extra value of Mecatol Rex while its custodians token (a victory point) is still on it.
pub const MECATOL_BONUS: f64 = 6.0;
/// A system must be worth at least this to steer the action and activation choice.
pub const ACTIVATION_THRESHOLD: f64 = 1.0;
/// An accepted trade must be worth at least this much to the bot (engine `net` estimate).
pub const ACCEPT_NET: f64 = 0.25;
/// Ground forces kept waiting at its docks even when no ship could carry them yet.
pub const MIN_IDLE_GROUND: i64 = 4;
/// Spendable resources from which production is limited by slots rather than by resources.
pub const RICH_RESOURCES: f64 = 6.0;
/// Fleet pool size the bot keeps before putting new tokens elsewhere.
pub const FLEET_TOKENS_WANTED: i32 = 3;
/// Mecatol Rex's planet id.
const MECATOL: &str = "mr";

/// The smart answer to `choice`, or `None` to keep the random pick.
#[must_use]
pub fn pick(choice: &Choice, seen: &SeatObservation<'_>, rng: &mut ChaCha8Rng) -> Option<usize> {
    let me = seen.bound_seat().clone();
    let observed = seen.observed();
    let subtype = choice.context.as_ref().map(|c| c.subtype.as_str());
    if let Some(index) = choice.options.iter().position(|o| o.kind == "score") {
        return Some(index);
    }
    match subtype {
        // A victory point is never worth declining once the engine offers it.
        Some("imperial_score_objective" | "remove_custodians") => {
            return choice.options.iter().position(|o| !o.is_decline());
        }
        Some("activate_system") => return pick_activation(choice, observed, &me),
        Some("movement_step") => return pick_move(choice, observed, &me),
        Some("load_cargo") => return pick_load(choice, observed, &me),
        Some("commit_ground_forces") => return pick_commit(choice, observed, &me),
        Some("announce_retreat") => return pick_retreat(choice, observed, &me),
        Some("gain_command_token") => return pick_token_pool(choice, observed, &me),
        Some("status_redistribute_tokens" | "warfare_redistribute_tokens") => {
            return pick_redistribution(choice, observed, &me);
        }
        Some("answer_transaction") => return pick_answer(choice),
        Some("propose_transaction") => return pick_proposal(choice),
        _ => {}
    }
    if choice
        .options
        .iter()
        .any(|o| o.kind == "action" && o.id == "tactical")
    {
        return pick_action(choice, observed, &me, rng);
    }
    if let Some(index) = pick_secondary(choice, observed, &me) {
        return Some(index);
    }
    if choice.options.iter().any(|o| o.kind == "produce") {
        return pick_produce(choice, observed, &me);
    }
    if choice.options.iter().all(|o| o.kind == "pay") {
        return pick_pay(choice);
    }
    if choice.prompt.contains("over fleet supply")
        && choice.options.iter().all(|o| o.kind == "remove")
    {
        return pick_cheapest_removal(choice, observed);
    }
    None
}

// -- reading the board ---------------------------------------------------------------------------

fn faction(seen: &Observed<'_>, player: &PlayerId) -> String {
    seen.seat(player)
        .map(|seat| seat.faction.as_str().to_owned())
        .unwrap_or_default()
}

fn unit_type<'a>(seen: &Observed<'a>, id: &str) -> Option<UnitType<'a>> {
    ti4_content::units::unit_type(seen.content(), id, seen.sources())
}

fn is_ship(seen: &Observed<'_>, unit: &Unit) -> bool {
    unit_type(seen, unit.type_id.as_str()).is_some_and(|t| t.is_ship())
}

fn is_ground(seen: &Observed<'_>, unit: &Unit) -> bool {
    unit_type(seen, unit.type_id.as_str()).is_some_and(|t| t.is_ground_force())
}

/// `(unit id, count)` pairs, as [`descriptors`] takes them.
type Counts = Vec<(String, usize)>;

/// Tally units into the `(id, count)` / damaged lists [`descriptors`] takes.
fn tally<'u>(units: impl IntoIterator<Item = &'u Unit>) -> (Counts, Counts) {
    let mut all: BTreeMap<String, usize> = BTreeMap::new();
    let mut hurt: BTreeMap<String, usize> = BTreeMap::new();
    for unit in units {
        *all.entry(unit.type_id.to_string()).or_default() += 1;
        if unit.sustained_damage {
            *hurt.entry(unit.type_id.to_string()).or_default() += 1;
        }
    }
    (all.into_iter().collect(), hurt.into_iter().collect())
}

/// Combat descriptors of `units`, all owned by `owner`.
fn fleet<'u>(
    seen: &Observed<'_>,
    owner: &PlayerId,
    units: impl IntoIterator<Item = &'u Unit>,
) -> Descriptors {
    let (all, hurt) = tally(units);
    descriptors(
        seen.content(),
        seen.sources(),
        &faction(seen, owner),
        &all,
        &hurt,
    )
}

/// Sum descriptors of several owners' units (a multi-player defence is treated as one side).
fn hostile_fleet<'u>(
    seen: &Observed<'_>,
    me: &PlayerId,
    units: impl IntoIterator<Item = &'u Unit>,
) -> Descriptors {
    let mut by_owner: BTreeMap<&PlayerId, Vec<&Unit>> = BTreeMap::new();
    for unit in units {
        if &unit.owner != me {
            by_owner.entry(&unit.owner).or_default().push(unit);
        }
    }
    let mut total = Descriptors::default();
    for (owner, units) in by_owner {
        let part = fleet(seen, owner, units);
        total.firepower += part.firepower;
        total.durability += part.durability;
        total.fighters += part.fighters;
        total.barrage += part.barrage;
        total.cannon += part.cannon;
        total.capacity += part.capacity;
        total.cost += part.cost;
    }
    total
}

/// Hostile ships in a system's space area, plus the space cannon of hostile units on its planets.
fn hostile_space(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> Descriptors {
    let board = seen.system(system);
    let mut ships = hostile_fleet(seen, me, board.units.iter().filter(|u| is_ship(seen, u)));
    let ground = hostile_fleet(seen, me, board.planet_units.values().flatten());
    ships.cannon += ground.cannon;
    ships
}

fn has_hostile_ships(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> bool {
    seen.system(system)
        .units
        .iter()
        .any(|u| &u.owner != me && is_ship(seen, u))
}

/// What a planet is worth to take: resources plus influence, Mecatol Rex more while its custodians
/// point is still there.
fn planet_worth(seen: &Observed<'_>, planet: &str) -> f64 {
    let Some(record) = ti4_content::galaxy::planet(seen.content(), planet, seen.sources()) else {
        return 0.0;
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "printed values are single digits"
    )]
    let mut worth = (record.resources() + record.influence()) as f64 + 0.5;
    if planet == MECATOL && !seen.custodians_removed() {
        worth += MECATOL_BONUS;
    }
    worth
}

/// Ground forces needed to take `planet`: one more than the hit points defending it.
fn ground_needed(
    seen: &Observed<'_>,
    me: &PlayerId,
    system: &SystemId,
    planet: &PlanetId,
) -> usize {
    let board = seen.system(system);
    let defenders: usize = board
        .planet_units
        .get(planet)
        .into_iter()
        .flatten()
        .filter(|u| &u.owner != me && is_ground(seen, u))
        .map(|u| {
            let sustains = !u.sustained_damage
                && unit_type(seen, u.type_id.as_str()).is_some_and(|t| t.sustain_damage());
            1 + usize::from(sustains)
        })
        .sum();
    defenders + 1
}

/// Planets in `system` worth invading, best first: (planet, worth, ground needed). Planets the
/// seat already holds, and Mecatol Rex while it cannot pay to lift the custodians, are left out.
fn targets(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> Vec<(PlanetId, f64, usize)> {
    let board = seen.system(system);
    let mut out: Vec<(PlanetId, f64, usize)> =
        ti4_content::galaxy::planets_in(seen.content(), system.as_str(), seen.sources())
            .into_iter()
            .map(|record| PlanetId::new(record.id()))
            .filter(|planet| board.planet_control.get(planet) != Some(me))
            .filter(|planet| {
                planet.as_str() != MECATOL
                    || seen.custodians_removed()
                    || seen.available_spend(me, Spend::Influence) >= 6
            })
            .map(|planet| {
                let worth = planet_worth(seen, planet.as_str());
                let need = ground_needed(seen, me, system, &planet);
                (planet, worth, need)
            })
            .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// The value of taking as many of `targets` as `ground` ground forces can, greedily by worth.
fn takeable_value(targets: &[(PlanetId, f64, usize)], mut ground: usize) -> f64 {
    let mut value = 0.0;
    for (_, worth, need) in targets {
        if *need <= ground {
            ground -= need;
            value += worth;
        }
    }
    value
}

/// Count this seat's units.
fn own_counts(seen: &Observed<'_>, me: &PlayerId) -> (usize, usize) {
    let (mut ships, mut ground) = (0, 0);
    for system in seen.board().values() {
        for unit in system
            .units
            .iter()
            .chain(system.planet_units.values().flatten())
        {
            if &unit.owner != me {
                continue;
            }
            let Some(kind) = unit_type(seen, unit.type_id.as_str()) else {
                continue;
            };
            if kind.is_ship() && !kind.is_fighter() {
                ships += 1;
            } else if kind.is_ground_force() {
                ground += 1;
            }
        }
    }
    (ships, ground)
}

/// Whether the seat is short of units and should build.
fn wants_units(seen: &Observed<'_>, me: &PlayerId) -> bool {
    let (ships, ground) = own_counts(seen, me);
    ships < 6 || ground < 5
}

fn has_own_production(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> bool {
    let board = seen.system(system);
    board
        .units
        .iter()
        .chain(board.planet_units.values().flatten())
        .any(|u| {
            &u.owner == me
                && unit_type(seen, u.type_id.as_str()).is_some_and(|t| t.has_production())
        })
}

/// What activating `system` is worth to `me`, from the public board.
///
/// Negative when the fight there would be lost; zero when the bot has no reason to go.
#[must_use]
pub fn activation_value(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> f64 {
    let movable = seen.movable_into(me, system);
    let board = seen.system(system);
    let present: Vec<&Unit> = board
        .units
        .iter()
        .filter(|u| &u.owner == me && is_ship(seen, u))
        .collect();

    // Space: everything that could come, plus what is already there.
    let arriving: Vec<&Unit> = movable.iter().map(|m| &m.unit).collect();
    let own = fleet(
        seen,
        me,
        present.iter().copied().chain(arriving.iter().copied()),
    );
    let enemy = hostile_space(seen, me, system);
    let contested = has_hostile_ships(seen, me, system);
    let mut value = 0.0;
    if contested {
        if movable.is_empty() || matchup(&own, &enemy) < WIN_MARGIN {
            return -10.0;
        }
        value += 1.0 + 0.5 * enemy.cost;
    }

    // Ground: what carriers coming from the reachable origins could bring.
    let capacity: i64 = movable.iter().map(|m| m.capacity).sum();
    let origins: BTreeSet<&SystemId> = movable
        .iter()
        .filter(|m| m.capacity > 0)
        .map(|m| &m.origin)
        .collect();
    let mut loadable_ground = 0usize;
    for origin in origins {
        loadable_ground += seen
            .loadable(me, origin)
            .iter()
            .filter(|cargo| is_ground(seen, &cargo.unit))
            .count();
    }
    let already_there = board
        .units
        .iter()
        .filter(|u| &u.owner == me && is_ground(seen, u))
        .count();
    let ground = loadable_ground.min(usize::try_from(capacity).unwrap_or(0)) + already_there;
    let planets = targets(seen, me, system);
    if !movable.is_empty() || already_there > 0 {
        value += takeable_value(&planets, ground);
    }

    // Production at home when short of units and able to pay.
    if !contested
        && has_own_production(seen, me, system)
        && seen.available_spend(me, Spend::Resources) >= 3
        && wants_units(seen, me)
    {
        value = value.max(2.0);
    }
    value
}

// -- the questions -------------------------------------------------------------------------------

/// Best activation among the offered systems; avoid losing fights; random among the rest.
fn pick_activation(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    seen.galaxy()?;
    let scored: Vec<(usize, f64)> = choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind == "activate")
        .map(|(i, o)| (i, activation_value(seen, me, &SystemId::new(&o.id))))
        .collect();
    best_above(&scored, ACTIVATION_THRESHOLD)
}

/// Highest score at or above `threshold`, first on a tie.
fn best_above(scored: &[(usize, f64)], threshold: f64) -> Option<usize> {
    scored
        .iter()
        .filter(|(_, s)| *s >= threshold)
        .fold(None, |best: Option<(usize, f64)>, &(i, s)| match best {
            Some((_, b)) if b >= s => best,
            _ => Some((i, s)),
        })
        .map(|(i, _)| i)
}

/// Action phase: tactical when a system is worth it (now and then the strategic action instead,
/// so the card is played); otherwise no opinion.
fn pick_action(
    choice: &Choice,
    seen: &Observed<'_>,
    me: &PlayerId,
    rng: &mut ChaCha8Rng,
) -> Option<usize> {
    let galaxy = seen.galaxy()?;
    let tactical = choice.options.iter().position(|o| o.id == "tactical")?;
    if seen.seat(me).is_some_and(|seat| seat.tactic_tokens <= 0) {
        return None;
    }
    let tokens = seen.systems_with_token(me);
    let worth = galaxy
        .system_ids()
        .into_iter()
        .map(SystemId::new)
        .filter(|system| !tokens.contains(system))
        .map(|system| activation_value(seen, me, &system))
        .fold(f64::NEG_INFINITY, f64::max);
    if worth < ACTIVATION_THRESHOLD {
        return None;
    }
    let strategic = choice
        .options
        .iter()
        .position(|o| o.kind == "action" && o.id.starts_with("strategic"));
    match strategic {
        Some(index) if rng.random_bool(0.25) => Some(index),
        _ => Some(tactical),
    }
}

fn payload_str<'o>(option: &'o ChoiceOption, key: &str) -> Option<&'o str> {
    option.payload.get(key).and_then(Value::as_str)
}

fn payload_f64(option: &ChoiceOption, key: &str) -> Option<f64> {
    option.payload.get(key).and_then(Value::as_f64)
}

/// Ground forces the active system's planets still need beyond what is already in its space.
fn ground_shortfall(seen: &Observed<'_>, me: &PlayerId, system: &SystemId, extra: usize) -> usize {
    let carried = seen
        .system(system)
        .units
        .iter()
        .filter(|u| &u.owner == me && is_ground(seen, u))
        .count()
        + extra;
    // Only the planets worth having count; an unreachable Mecatol is already filtered out.
    let need: usize = targets(seen, me, system).iter().map(|(_, _, n)| n).sum();
    need.saturating_sub(carried)
}

/// Movement: reinforce a fight until it is clearly won, bring a carrier while planets need ground
/// forces, then finish. Never into a fight that cannot be won.
fn pick_move(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let active = seen.active_system()?.clone();
    let done = choice.options.iter().position(ChoiceOption::is_decline)?;
    let moves: Vec<(usize, &ChoiceOption, UnitType<'_>)> = choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind == "move")
        .filter_map(|(i, o)| Some((i, o, unit_type(seen, payload_str(o, "unit")?)?)))
        .collect();
    let board = seen.system(&active);
    let present: Vec<&Unit> = board
        .units
        .iter()
        .filter(|u| &u.owner == me && is_ship(seen, u))
        .collect();

    if has_hostile_ships(seen, me, &active) {
        let enemy = hostile_space(seen, me, &active);
        let now = fleet(seen, me, present.iter().copied());
        if matchup(&now, &enemy) >= WIN_MARGIN + 0.3 {
            // Already winning: only a carrier for the invasion is still useful.
            return carrier_for_ground(&moves, seen, me, &active).or(Some(done));
        }
        let everything = fleet(
            seen,
            me,
            present
                .iter()
                .copied()
                .chain(seen.movable_into(me, &active).iter().map(|m| &m.unit)),
        );
        if matchup(&everything, &enemy) < WIN_MARGIN {
            return Some(done); // a losing fight: stay out
        }
        // Strongest warship first.
        return moves
            .iter()
            .filter(|(_, _, t)| t.combat_dice() > 0)
            .max_by(|a, b| {
                strength(a.2)
                    .total_cmp(&strength(b.2))
                    .then_with(|| b.0.cmp(&a.0))
            })
            .map(|(i, _, _)| *i)
            .or(Some(done));
    }

    if has_own_production(seen, me, &active) && targets(seen, me, &active).is_empty() {
        return Some(done); // a build activation: keep the fleet where it is
    }
    if let Some(index) = carrier_for_ground(&moves, seen, me, &active) {
        return Some(index);
    }
    if !targets(seen, me, &active).is_empty() || !present.is_empty() {
        return Some(done);
    }
    None
}

fn strength(kind: UnitType<'_>) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "dice counts are small")]
    let dice = kind.combat_dice() as f64;
    let hits = kind
        .combat_hits_on()
        .map_or(0.0, |on| ti4_policy::fleet_strength::hit_chance(on, 0));
    dice * hits * (1.0 + f64::from(u8::from(kind.sustain_damage())))
}

/// A ship with capacity whose origin has ground forces, while the active system needs more.
fn carrier_for_ground(
    moves: &[(usize, &ChoiceOption, UnitType<'_>)],
    seen: &Observed<'_>,
    me: &PlayerId,
    active: &SystemId,
) -> Option<usize> {
    if ground_shortfall(seen, me, active, 0) == 0 {
        return None;
    }
    moves
        .iter()
        .filter(|(_, o, t)| {
            t.capacity() > 0
                && payload_str(o, "origin").is_some_and(|origin| {
                    seen.loadable(me, &SystemId::new(origin))
                        .iter()
                        .any(|cargo| is_ground(seen, &cargo.unit))
                })
        })
        .max_by_key(|(i, _, t)| (t.capacity(), std::cmp::Reverse(*i)))
        .map(|(i, _, _)| *i)
}

/// Loading: ground forces while the target needs them, fighters for a fight, else stop.
fn pick_load(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let active = seen.active_system()?.clone();
    let done = choice.options.iter().position(ChoiceOption::is_decline)?;
    let loaded = choice
        .options
        .iter()
        .find_map(|o| payload_f64(o, "loaded_ground"))
        .unwrap_or(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small count"
    )]
    let loaded = loaded.max(0.0) as usize;
    let load_of = |ground: bool| {
        choice.options.iter().position(|o| {
            o.kind == "load"
                && payload_str(o, "unit")
                    .and_then(|id| unit_type(seen, id))
                    .is_some_and(|t| {
                        if ground {
                            t.is_ground_force()
                        } else {
                            t.is_fighter()
                        }
                    })
        })
    };
    if ground_shortfall(seen, me, &active, loaded) > 0
        && let Some(index) = load_of(true)
    {
        return Some(index);
    }
    if has_hostile_ships(seen, me, &active)
        && let Some(index) = load_of(false)
    {
        return Some(index);
    }
    Some(done)
}

/// Landing: the most valuable planet still short of a winning force that can still be reached.
fn pick_commit(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let done = choice.options.iter().position(ChoiceOption::is_decline)?;
    let system = seen
        .active_system()
        .cloned()
        .or_else(|| seen.invasion().map(|invasion| invasion.system))?;
    let troops: BTreeSet<&str> = choice
        .options
        .iter()
        .filter(|o| o.kind == "commit")
        .filter_map(|o| o.id.split('|').nth(1))
        .collect();
    let available = troops.len();
    let board = seen.system(&system);
    let mut best: Option<(usize, f64)> = None;
    for (index, option) in choice.options.iter().enumerate() {
        if option.kind != "commit" {
            continue;
        }
        let Some(planet) = payload_str(option, "planet") else {
            continue;
        };
        let planet = PlanetId::new(planet);
        let mine = board
            .planet_units
            .get(&planet)
            .into_iter()
            .flatten()
            .filter(|u| &u.owner == me && is_ground(seen, u))
            .count();
        let need = ground_needed(seen, me, &system, &planet);
        let held = board.planet_control.get(&planet) == Some(me) && need == 1;
        if held || mine >= need || mine + available < need {
            continue;
        }
        let worth = planet_worth(seen, planet.as_str());
        if best.is_none_or(|(_, b)| worth > b) {
            best = Some((index, worth));
        }
    }
    Some(best.map_or(done, |(index, _)| index))
}

/// Retreat when clearly losing the space combat, else stay.
fn pick_retreat(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let system = choice
        .options
        .iter()
        .find_map(|o| payload_str(o, "system"))
        .map(SystemId::new)?;
    let stay = choice.options.iter().position(|o| o.id == "stay")?;
    let retreat = choice.options.iter().position(|o| o.id == "retreat")?;
    let board = seen.system(&system);
    let own = fleet(
        seen,
        me,
        board
            .units
            .iter()
            .filter(|u| &u.owner == me && is_ship(seen, u)),
    );
    let enemy = hostile_space(seen, me, &system);
    Some(if matchup(&own, &enemy) < RETREAT_MARGIN {
        retreat
    } else {
        stay
    })
}

/// Ground forces waiting in systems with own production (where new ones appear), and the
/// capacity of every own ship that could carry them away.
fn idle_ground_and_lift(seen: &Observed<'_>, me: &PlayerId) -> (usize, i64) {
    let (mut idle, mut lift) = (0, 0);
    for (id, system) in seen.board() {
        let at_dock = has_own_production(seen, me, id);
        for unit in system
            .units
            .iter()
            .chain(system.planet_units.values().flatten())
        {
            if &unit.owner != me {
                continue;
            }
            let Some(kind) = unit_type(seen, unit.type_id.as_str()) else {
                continue;
            };
            if kind.is_ship() {
                lift += kind.capacity();
            } else if at_dock && kind.is_ground_force() {
                idle += 1;
            }
        }
    }
    (idle, lift)
}

/// Production: the most useful affordable unit per resource (per production slot when resources
/// are plentiful), within fleet supply and capacity. Ground forces only while fewer are waiting
/// at its docks than its ships can carry; when nothing useful is left, stop.
fn pick_produce(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let (ships, ground) = own_counts(seen, me);
    let (idle, lift) = idle_ground_and_lift(seen, me);
    let enough_ground = idle >= usize::try_from(lift.max(MIN_IDLE_GROUND)).unwrap_or(0);
    let carriers = seen
        .board()
        .values()
        .flat_map(|s| s.units.iter())
        .filter(|u| &u.owner == me)
        .filter(|u| unit_type(seen, u.type_id.as_str()).is_some_and(|t| t.capacity() >= 4))
        .count();
    let mut scored = Vec::new();
    for (index, option) in choice.options.iter().enumerate() {
        if option.kind != "produce" {
            continue;
        }
        let Some(kind) = payload_str(option, "unit").and_then(|id| unit_type(seen, id)) else {
            continue;
        };
        let cost = payload_f64(option, "owed")
            .or_else(|| payload_f64(option, "cost"))
            .unwrap_or_else(|| kind.cost());
        if payload_f64(option, "available_resources").is_some_and(|available| cost > available) {
            continue;
        }
        if payload_f64(option, "fleet_excess_after").unwrap_or(0.0) > 0.0
            || payload_f64(option, "capacity_excess_after").unwrap_or(0.0) > 0.0
        {
            continue;
        }
        if kind.is_ground_force() && enough_ground {
            continue;
        }
        let count = payload_f64(option, "count").unwrap_or(1.0);
        let mut worth =
            count * ti4_policy::valuation::unit_value(seen.content(), seen.sources(), kind.id());
        if kind.is_ground_force() && ground < 6 {
            worth *= 1.6;
        }
        if kind.capacity() >= 4 && carriers < 2 {
            worth *= 2.0;
        }
        if kind.is_ship() && !kind.is_fighter() && ships < 6 {
            worth *= 1.3;
        }
        if kind.is_fighter() {
            worth *= 0.6;
        }
        // With resources to spare, production slots are what runs out: value per slot.
        let rich = payload_f64(option, "available_resources").unwrap_or(0.0) >= RICH_RESOURCES;
        let price = if rich {
            payload_f64(option, "production_spent")
                .unwrap_or(count)
                .max(1.0)
        } else {
            cost.max(0.5)
        };
        scored.push((index, worth / price));
    }
    if scored.is_empty() {
        return choice.options.iter().position(ChoiceOption::is_decline);
    }
    best_above(&scored, f64::NEG_INFINITY)
}

/// Payment: the source that wastes least of what it is worth.
fn pick_pay(choice: &Choice) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let worth = payload_f64(o, "worth")?;
            let owed = payload_f64(o, "owed")?;
            Some((i, (worth - owed).max(0.0), worth))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
        .map(|(i, _, _)| i)
}

/// Remove the cheapest unit when over fleet supply.
fn pick_cheapest_removal(choice: &Choice, seen: &Observed<'_>) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let unit = o.label.strip_prefix("remove ")?;
            Some((
                i,
                ti4_policy::valuation::unit_value(seen.content(), seen.sources(), unit),
            ))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// A new command token: the fleet pool until it holds a few, then whichever of the tactic and
/// strategy pools is smaller (tactic on a tie, as tactical actions are what the bot mostly takes).
fn pick_token_pool(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let seat = seen.seat(me)?;
    let pool = if seat.fleet_tokens < fleet_wanted(seen, me) {
        "fleet_tokens"
    } else if seat.tactic_tokens <= seat.strategic_tokens {
        "tactic_tokens"
    } else {
        "strategic_tokens"
    };
    choice.options.iter().position(|o| o.id == pool)
}

/// The fleet pool the bot wants: a few tokens, or enough for its biggest stack of non-fighter
/// ships in one system.
fn fleet_wanted(seen: &Observed<'_>, me: &PlayerId) -> i32 {
    let stack = seen
        .board()
        .values()
        .map(|system| {
            system
                .units
                .iter()
                .filter(|u| &u.owner == me)
                .filter(|u| {
                    unit_type(seen, u.type_id.as_str())
                        .is_some_and(|t| t.is_ship() && !t.is_fighter())
                })
                .count()
        })
        .max()
        .unwrap_or(0);
    FLEET_TOKENS_WANTED.max(i32::try_from(stack).unwrap_or(i32::MAX))
}

/// Redistribution ("tactic|fleet|strategy"): the fleet pool at [`fleet_wanted`], the rest split
/// evenly between tactic and strategy, an odd token to tactic.
fn pick_redistribution(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let fleet = fleet_wanted(seen, me);
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let mut parts = o.id.split('|').map(|p| p.parse::<i32>().ok());
            let (t, f, s) = (parts.next()??, parts.next()??, parts.next()??);
            Some((i, 10 * (f - fleet).abs() + (t - s - 1).abs()))
        })
        .min_by_key(|&(i, cost)| (cost, i))
        .map(|(i, _)| i)
}

/// Strategy-card follow-ups worth a strategy token whenever offered: research and a secret
/// objective (Leadership's token purchase is handled the same way). Others stay random.
fn pick_secondary(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let yes = choice.options.iter().position(|o| o.id == "yes")?;
    let no = choice.options.iter().position(|o| o.id == "no")?;
    if choice
        .prompt
        .starts_with("spend a strategy token and 4 resources to research")
        || choice
            .prompt
            .starts_with("spend a strategy token to draw a secret objective")
    {
        return Some(yes);
    }
    if choice.prompt.contains("influence for a command token") {
        // Keep the six influence for the custodians while they still stand.
        let spare = seen.available_spend(me, Spend::Influence)
            - ti4_engine::strategy_cards::INFLUENCE_PER_TOKEN;
        return Some(if seen.custodians_removed() || spare >= 6 {
            yes
        } else {
            no
        });
    }
    None
}

/// Accept only a trade that is slightly in the bot's favour; otherwise refuse, never counter.
fn pick_answer(choice: &Choice) -> Option<usize> {
    let accept = choice.options.iter().position(|o| o.id == "accept");
    let refuse = choice.options.iter().position(ChoiceOption::is_decline);
    let net = accept.and_then(|i| payload_f64(&choice.options[i], "net"));
    match (accept, net) {
        (Some(index), Some(net)) if net >= ACCEPT_NET => Some(index),
        _ => refuse,
    }
}

/// Propose the offer that gains most for the bot among those that also gain for the other side.
fn pick_proposal(choice: &Choice) -> Option<usize> {
    let scored: Vec<(usize, f64)> = choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind == "offer")
        .filter_map(|(i, o)| {
            let net = payload_f64(o, "net")?;
            let theirs = payload_f64(o, "their_net")?;
            (net > 0.0 && theirs >= ACCEPT_NET).then_some((i, net))
        })
        .collect();
    best_above(&scored, f64::NEG_INFINITY)
        .or_else(|| choice.options.iter().position(ChoiceOption::is_decline))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use ti4_content::ContentStore;
    use ti4_engine::choice::{Decider, IllegalChoice};
    use ti4_engine::fixtures::{Hub, game, hub_from, plain_systems, put, put_on_planet};
    use ti4_model::POK;
    use ti4_model::state::GameState;

    fn me() -> PlayerId {
        PlayerId::new("a")
    }

    fn foe() -> PlayerId {
        PlayerId::new("b")
    }

    fn planets(system: &str) -> Vec<String> {
        ti4_content::galaxy::planets_in(ContentStore::embedded(), system, POK)
            .iter()
            .map(|p| p.id().to_owned())
            .collect()
    }

    /// A one-ring map whose centre has one planet and whose first two outer systems are empty.
    fn board() -> (GameState, Hub, SystemId, PlanetId) {
        let plain = plain_systems(60);
        let with_planet = plain
            .iter()
            .find(|id| {
                let p = planets(id);
                p.len() == 1
                    && p[0] != MECATOL
                    && ti4_content::galaxy::planet(ContentStore::embedded(), &p[0], POK)
                        .is_some_and(|r| r.homeworld_of().is_none() && r.resources() > 0)
            })
            .expect("a one-planet system")
            .clone();
        let mut ids = vec![with_planet.clone()];
        ids.extend(
            plain
                .iter()
                .filter(|id| planets(id).is_empty())
                .take(6)
                .cloned(),
        );
        let hub = hub_from(&ids);
        let state = game(&["a", "b"]);
        let planet = PlanetId::new(planets(&with_planet)[0].clone());
        (state, hub, SystemId::new(with_planet), planet)
    }

    fn observe<'a>(state: &'a GameState, hub: &'a Hub) -> Observed<'a> {
        Observed::new(state, ContentStore::embedded(), POK, Some(&hub.galaxy))
    }

    fn outer(hub: &Hub, i: usize) -> SystemId {
        SystemId::new(hub.outer[i].clone())
    }

    fn activation_choice(systems: &[&SystemId]) -> Choice {
        Choice::new(
            me(),
            "activate a system",
            systems
                .iter()
                .map(|s| ChoiceOption::labelled(s.as_str(), "activate", format!("activate {s}")))
                .collect(),
        )
    }

    #[test]
    fn activation_prefers_the_system_with_a_planet_it_can_take() {
        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "carrier", &me(), 1);
        put(&mut state, &home, "infantry", &me(), 2);
        let seen = observe(&state, &hub);
        assert!(activation_value(&seen, &me(), &centre) >= ACTIVATION_THRESHOLD);
        assert!(activation_value(&seen, &me(), &outer(&hub, 1)) < ACTIVATION_THRESHOLD);
        let empty = outer(&hub, 1);
        let choice = activation_choice(&[&empty, &centre]);
        assert_eq!(pick_activation(&choice, &seen, &me()), Some(1));
    }

    #[test]
    fn a_carrier_without_ground_forces_takes_nothing() {
        let (mut state, hub, centre, _) = board();
        put(&mut state, &outer(&hub, 0), "carrier", &me(), 1);
        let seen = observe(&state, &hub);
        assert!(activation_value(&seen, &me(), &centre) < ACTIVATION_THRESHOLD);
    }

    #[test]
    fn a_fight_it_would_lose_is_avoided_and_one_it_wins_is_taken() {
        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "carrier", &me(), 1);
        put(&mut state, &home, "infantry", &me(), 2);
        put(&mut state, &home, "destroyer", &me(), 1);
        put(&mut state, &centre, "dreadnought", &foe(), 2);
        let seen = observe(&state, &hub);
        assert!(activation_value(&seen, &me(), &centre) < 0.0);
        assert_eq!(
            pick_activation(&activation_choice(&[&centre]), &seen, &me()),
            None
        );

        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "carrier", &me(), 1);
        put(&mut state, &home, "infantry", &me(), 2);
        put(&mut state, &home, "dreadnought", &me(), 3);
        put(&mut state, &centre, "destroyer", &foe(), 1);
        let seen = observe(&state, &hub);
        assert!(activation_value(&seen, &me(), &centre) >= ACTIVATION_THRESHOLD);
    }

    #[test]
    fn mecatol_rex_is_worth_more_than_its_printed_values_until_the_custodians_fall() {
        let mut state = game(&["a"]);
        let hub = hub_from(&["18".to_owned()]);
        let seen = observe(&state, &hub);
        assert!(planet_worth(&seen, MECATOL) > 7.0 + MECATOL_BONUS - 0.1);
        state.custodians_removed = true;
        let seen = observe(&state, &hub);
        assert!(planet_worth(&seen, MECATOL) < 8.0);
    }

    #[test]
    fn mecatol_rex_is_invaded_only_when_the_custodians_can_be_paid_for() {
        let mut state = game(&["a"]);
        let hub = hub_from(&["18".to_owned()]);
        let mr = SystemId::new("18");
        state.player_mut(&me()).expect("seat").trade_goods = 0;
        let seen = observe(&state, &hub);
        assert!(
            targets(&seen, &me(), &mr).is_empty(),
            "six influence needed"
        );
        state.player_mut(&me()).expect("seat").trade_goods = 6;
        let seen = observe(&state, &hub);
        let found = targets(&seen, &me(), &mr);
        assert_eq!(found.len(), 1);
        assert!(
            found[0].1 > 10.0,
            "the custodians point makes it the best target"
        );
    }

    fn move_choice(origin: &SystemId, units: &[(&str, i64)]) -> Choice {
        let mut options: Vec<ChoiceOption> = units
            .iter()
            .enumerate()
            .map(|(i, (unit, capacity))| {
                ChoiceOption::labelled(format!("move|{origin}|{i}"), "move", format!("move {unit}"))
                    .with("origin", origin.to_string())
                    .with("unit", *unit)
                    .with("capacity", *capacity)
            })
            .collect();
        options.push(ChoiceOption::labelled(
            "done_moving",
            "decline",
            "finish movement",
        ));
        Choice::new(me(), "movement", options)
    }

    #[test]
    fn movement_brings_a_carrier_for_the_planet_then_stops() {
        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "cruiser", &me(), 1);
        put(&mut state, &home, "carrier", &me(), 1);
        put(&mut state, &home, "infantry", &me(), 2);
        state.active_system = Some(centre.clone());
        let choice = move_choice(&home, &[("cruiser", 0), ("carrier", 4)]);
        let seen = observe(&state, &hub);
        assert_eq!(pick_move(&choice, &seen, &me()), Some(1), "the carrier");

        // Once the ground forces are in the active system, finish.
        put(&mut state, &centre, "carrier", &me(), 1);
        put(&mut state, &centre, "infantry", &me(), 1);
        let choice = move_choice(&home, &[("cruiser", 0)]);
        let seen = observe(&state, &hub);
        assert_eq!(pick_move(&choice, &seen, &me()), Some(1), "done");
    }

    #[test]
    fn movement_stays_out_of_a_losing_fight_and_reinforces_a_winnable_one() {
        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "destroyer", &me(), 1);
        put(&mut state, &centre, "dreadnought", &foe(), 2);
        state.active_system = Some(centre.clone());
        let seen = observe(&state, &hub);
        let choice = move_choice(&home, &[("destroyer", 0)]);
        assert_eq!(pick_move(&choice, &seen, &me()), Some(1), "finish, not die");

        let (mut state, hub, centre, _) = board();
        let home = outer(&hub, 0);
        put(&mut state, &home, "destroyer", &me(), 1);
        put(&mut state, &home, "dreadnought", &me(), 2);
        put(&mut state, &centre, "destroyer", &foe(), 1);
        state.active_system = Some(centre.clone());
        let seen = observe(&state, &hub);
        let choice = move_choice(&home, &[("destroyer", 0), ("dreadnought", 0)]);
        assert_eq!(
            pick_move(&choice, &seen, &me()),
            Some(1),
            "the dreadnought first"
        );
    }

    fn load_choice(loaded_ground: i64) -> Choice {
        let load = |id: &str, unit: &str| {
            ChoiceOption::labelled(id, "load", format!("load {unit}"))
                .with("unit", unit)
                .with("loaded_ground", loaded_ground)
        };
        Choice::new(
            me(),
            "load carrier",
            vec![
                load("load|0", "fighter"),
                load("load|1", "infantry"),
                ChoiceOption::labelled("done_loading", "decline", "carry nothing further"),
            ],
        )
    }

    #[test]
    fn loading_takes_ground_forces_the_target_needs_and_no_more() {
        let (mut state, hub, centre, planet) = board();
        state.active_system = Some(centre.clone());
        put_on_planet(&mut state, &centre, &planet, "infantry", &foe(), 1);
        let seen = observe(&state, &hub);
        assert_eq!(pick_load(&load_choice(0), &seen, &me()), Some(1));
        assert_eq!(
            pick_load(&load_choice(1), &seen, &me()),
            Some(1),
            "two needed"
        );
        assert_eq!(pick_load(&load_choice(2), &seen, &me()), Some(2), "enough");
    }

    fn commit_choice(planet: &PlanetId, troops: usize) -> Choice {
        let mut options: Vec<ChoiceOption> = (0..troops)
            .map(|i| {
                ChoiceOption::labelled(format!("commit|{i}|{planet}"), "commit", "land infantry")
                    .with("planet", planet.to_string())
                    .with("unit", "infantry")
            })
            .collect();
        options.push(ChoiceOption::labelled(
            "done_committing",
            "decline",
            "commit no more ground forces",
        ));
        Choice::new(me(), "commit ground forces", options)
    }

    #[test]
    fn landing_commits_until_the_planet_is_outnumbered_and_not_into_a_lost_cause() {
        let (mut state, hub, centre, planet) = board();
        state.active_system = Some(centre.clone());
        put_on_planet(&mut state, &centre, &planet, "infantry", &foe(), 2);
        let seen = observe(&state, &hub);
        assert_eq!(
            pick_commit(&commit_choice(&planet, 2), &seen, &me()),
            Some(2),
            "2 < 3"
        );
        assert_eq!(
            pick_commit(&commit_choice(&planet, 3), &seen, &me()),
            Some(0)
        );

        put_on_planet(&mut state, &centre, &planet, "infantry", &me(), 3);
        let seen = observe(&state, &hub);
        assert_eq!(
            pick_commit(&commit_choice(&planet, 2), &seen, &me()),
            Some(2),
            "covered"
        );
    }

    #[test]
    fn a_clearly_lost_space_combat_is_retreated_from() {
        let (mut state, hub, centre, _) = board();
        put(&mut state, &centre, "destroyer", &me(), 1);
        put(&mut state, &centre, "dreadnought", &foe(), 2);
        let choice = Choice::new(
            me(),
            "announce a retreat",
            vec![
                ChoiceOption::labelled("stay", "retreat", "stay")
                    .with("system", centre.to_string()),
                ChoiceOption::labelled("retreat", "retreat", "go")
                    .with("system", centre.to_string()),
            ],
        );
        let seen = observe(&state, &hub);
        assert_eq!(pick_retreat(&choice, &seen, &me()), Some(1));
        put(&mut state, &centre, "dreadnought", &me(), 4);
        let seen = observe(&state, &hub);
        assert_eq!(pick_retreat(&choice, &seen, &me()), Some(0));
    }

    fn build(unit: &str, count: i64, owed: i64, available: i64) -> ChoiceOption {
        ChoiceOption::labelled(format!("build|{unit}|{count}"), "produce", unit)
            .with("unit", unit)
            .with("count", count)
            .with("owed", owed)
            .with("available_resources", available)
            .with("fleet_excess_after", 0)
            .with("capacity_excess_after", 0)
    }

    #[test]
    fn production_buys_a_carrier_when_it_has_none_and_never_what_it_cannot_pay() {
        let (state, hub, _, _) = board();
        let seen = observe(&state, &hub);
        let choice = Choice::new(
            me(),
            "produce",
            vec![
                build("fighter", 2, 1, 3),
                build("carrier", 1, 3, 3),
                build("dreadnought", 1, 4, 3),
                ChoiceOption::labelled("done_producing", "decline", "done"),
            ],
        );
        assert_eq!(pick_produce(&choice, &seen, &me()), Some(1));
        let broke = Choice::new(
            me(),
            "produce",
            vec![
                build("dreadnought", 1, 4, 3),
                ChoiceOption::labelled("done_producing", "decline", "done"),
            ],
        );
        assert_eq!(pick_produce(&broke, &seen, &me()), Some(1), "decline");
    }

    #[test]
    fn production_stops_buying_ground_forces_its_ships_cannot_carry() {
        let (mut state, hub, centre, planet) = board();
        put_on_planet(&mut state, &centre, &planet, "spacedock", &me(), 1);
        put_on_planet(&mut state, &centre, &planet, "infantry", &me(), 2);
        let choice = || {
            Choice::new(
                me(),
                "produce",
                vec![
                    build("infantry", 2, 1, 3),
                    build("destroyer", 1, 1, 3),
                    ChoiceOption::labelled("done_producing", "decline", "done"),
                ],
            )
        };
        let seen = observe(&state, &hub);
        assert_eq!(
            pick_produce(&choice(), &seen, &me()),
            Some(0),
            "few waiting"
        );

        put_on_planet(&mut state, &centre, &planet, "infantry", &me(), 2);
        let seen = observe(&state, &hub);
        assert_eq!(
            pick_produce(&choice(), &seen, &me()),
            Some(1),
            "four waiting, nothing to carry them: a warship instead"
        );

        // A carrier's worth of lift makes room for more.
        put(&mut state, &centre, "carrier", &me(), 2);
        let seen = observe(&state, &hub);
        assert_eq!(pick_produce(&choice(), &seen, &me()), Some(0));

        // Nothing but ground forces on offer and enough waiting: stop.
        let (mut state, hub, centre, planet) = board();
        put_on_planet(&mut state, &centre, &planet, "spacedock", &me(), 1);
        put_on_planet(&mut state, &centre, &planet, "infantry", &me(), 6);
        let seen = observe(&state, &hub);
        let only_ground = Choice::new(
            me(),
            "produce",
            vec![
                build("infantry", 2, 1, 3),
                ChoiceOption::labelled("done_producing", "decline", "done"),
            ],
        );
        assert_eq!(pick_produce(&only_ground, &seen, &me()), Some(1));
    }

    #[test]
    fn production_with_resources_to_spare_buys_the_most_per_slot() {
        let (state, hub, _, _) = board();
        let seen = observe(&state, &hub);
        let slot = |o: ChoiceOption| o.with("production_spent", 1);
        let choice = |available: i64| {
            Choice::new(
                me(),
                "produce",
                vec![
                    slot(build("destroyer", 1, 1, available)),
                    slot(build("dreadnought", 1, 4, available)),
                    ChoiceOption::labelled("done_producing", "decline", "done"),
                ],
            )
        };
        assert_eq!(
            pick_produce(&choice(4), &seen, &me()),
            Some(0),
            "per resource"
        );
        assert_eq!(pick_produce(&choice(10), &seen, &me()), Some(1), "per slot");
    }

    fn contextual(choice: Choice, subtype: &str) -> Choice {
        choice.contextualized(ti4_engine::decision_context::DecisionContext::new(
            me(),
            ti4_engine::decision_context::DecisionSource::Rule("test".to_owned()),
            subtype,
            ti4_model::state::Phase::Action,
            1,
        ))
    }

    /// What [`pick`] answers, through the engine's private ask.
    fn picked(state: &GameState, hub: &Hub, choice: &Choice) -> Option<usize> {
        let mut probe = Probe(None);
        ti4_engine::choice::ask_private(
            choice,
            state,
            ContentStore::embedded(),
            POK,
            Some(&hub.galaxy),
            &mut probe,
        )
        .expect("legal");
        probe.0
    }

    #[test]
    fn a_victory_point_is_never_declined() {
        let (state, hub, _, _) = board();
        let imperial = contextual(
            Choice::new(
                me(),
                "score a public objective with Imperial",
                vec![
                    ChoiceOption::labelled("corner", "objective", "corner"),
                    ChoiceOption::decline(),
                ],
            ),
            "imperial_score_objective",
        );
        assert_eq!(picked(&state, &hub, &imperial), Some(0));
        let custodians = contextual(
            Choice::new(
                me(),
                "spend 6 influence to remove the custodians token",
                vec![
                    ChoiceOption::labelled("no", "decline", "leave it"),
                    ChoiceOption::labelled("yes", "custodians", "remove it"),
                ],
            ),
            "remove_custodians",
        );
        assert_eq!(picked(&state, &hub, &custodians), Some(1));
    }

    #[test]
    fn command_tokens_fill_the_fleet_pool_first_then_the_smaller_pool() {
        let (mut state, hub, _, _) = board();
        let choice = contextual(
            Choice::new(
                me(),
                "gain a command token into which pool",
                ["tactic_tokens", "fleet_tokens", "strategic_tokens"]
                    .into_iter()
                    .map(|id| ChoiceOption::labelled(id, "pool", id))
                    .collect(),
            ),
            "gain_command_token",
        );
        let pools = |state: &mut GameState, t: i32, f: i32, s: i32| {
            let seat = state.player_mut(&me()).expect("seat");
            (seat.tactic_tokens, seat.fleet_tokens, seat.strategic_tokens) = (t, f, s);
        };
        pools(&mut state, 3, 2, 2);
        assert_eq!(picked(&state, &hub, &choice), Some(1), "fleet below three");
        pools(&mut state, 2, 3, 2);
        assert_eq!(picked(&state, &hub, &choice), Some(0), "tactic on a tie");
        pools(&mut state, 3, 3, 2);
        assert_eq!(picked(&state, &hub, &choice), Some(2));
        // A stack of four ships needs four fleet tokens.
        put(&mut state, &outer(&hub, 0), "cruiser", &me(), 4);
        assert_eq!(picked(&state, &hub, &choice), Some(1));
    }

    #[test]
    fn redistribution_keeps_three_fleet_tokens_and_splits_the_rest() {
        let (state, hub, _, _) = board();
        let ids = ["0|8|0", "0|2|6", "4|2|2", "2|3|3", "3|3|2", "5|3|0"];
        let choice = contextual(
            Choice::new(
                me(),
                "redistribute your command tokens",
                ids.iter()
                    .map(|id| ChoiceOption::labelled(*id, "redistribute", *id))
                    .collect(),
            ),
            "status_redistribute_tokens",
        );
        assert_eq!(picked(&state, &hub, &choice), Some(4), "3|3|2");
    }

    #[test]
    fn research_and_secret_objectives_are_always_followed_and_leadership_spares_the_custodians() {
        let (mut state, hub, _, _) = board();
        let yes_no = |prompt: &str| {
            Choice::new(
                me(),
                prompt,
                vec![
                    ChoiceOption::labelled("no", "strategy", "no"),
                    ChoiceOption::labelled("yes", "strategy", "yes"),
                ],
            )
        };
        for prompt in [
            "spend a strategy token and 4 resources to research",
            "spend a strategy token to draw a secret objective",
        ] {
            assert_eq!(picked(&state, &hub, &yes_no(prompt)), Some(1), "{prompt}");
        }
        assert_eq!(
            picked(
                &state,
                &hub,
                &yes_no("spend a strategy token to draw two action cards")
            ),
            None,
            "no opinion"
        );
        let leadership = yes_no("spend 3 influence for a command token");
        state.player_mut(&me()).expect("seat").trade_goods = 8;
        assert_eq!(picked(&state, &hub, &leadership), Some(0), "six kept back");
        state.player_mut(&me()).expect("seat").trade_goods = 9;
        assert_eq!(picked(&state, &hub, &leadership), Some(1));
        state.player_mut(&me()).expect("seat").trade_goods = 3;
        state.custodians_removed = true;
        assert_eq!(picked(&state, &hub, &leadership), Some(1));
    }

    #[test]
    fn payment_wastes_least() {
        let pay = |id: &str, worth: i64| {
            ChoiceOption::labelled(id, "pay", id)
                .with("worth", worth)
                .with("owed", 1)
        };
        let choice = Choice::new(
            me(),
            "pay 1",
            vec![pay("big", 3), pay("small", 1), pay("mid", 2)],
        );
        assert_eq!(pick_pay(&choice), Some(1));
    }

    #[test]
    fn trades_are_taken_only_slightly_in_its_favour_and_proposed_only_when_both_gain() {
        let answer = |net: f64| {
            Choice::new(
                me(),
                "accept?",
                vec![
                    ChoiceOption::labelled("accept", "transaction", "accept").with("net", net),
                    ChoiceOption::labelled("refuse", "decline", "refuse"),
                    ChoiceOption::labelled("counter", "transaction", "counter-offer"),
                ],
            )
        };
        assert_eq!(pick_answer(&answer(1.0)), Some(0));
        assert_eq!(pick_answer(&answer(0.1)), Some(1));
        assert_eq!(pick_answer(&answer(-3.0)), Some(1));

        let offer = |id: &str, net: f64, theirs: f64| {
            ChoiceOption::labelled(id, "offer", id)
                .with("net", net)
                .with("their_net", theirs)
        };
        let propose = Choice::new(
            me(),
            "transaction",
            vec![
                offer("greedy", 3.0, -1.0),
                offer("fair", 1.0, 1.0),
                offer("fairer", 0.5, 2.0),
                ChoiceOption::decline(),
            ],
        );
        assert_eq!(pick_proposal(&propose), Some(1));
        let none = Choice::new(
            me(),
            "transaction",
            vec![offer("greedy", 3.0, -1.0), ChoiceOption::decline()],
        );
        assert_eq!(pick_proposal(&none), Some(1));
    }

    #[test]
    fn over_fleet_supply_the_cheapest_unit_goes() {
        let (state, hub, _, _) = board();
        let seen = observe(&state, &hub);
        let choice = Choice::new(
            me(),
            "remove a unit: over fleet supply in 01",
            vec![
                ChoiceOption::labelled("remove|0", "remove", "remove carrier"),
                ChoiceOption::labelled("remove|1", "remove", "remove destroyer"),
            ],
        );
        assert_eq!(pick_cheapest_removal(&choice, &seen), Some(1));
    }

    /// The seat-bound entry point, through the engine's own private ask.
    struct Smart;
    impl Decider for Smart {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            Ok(choice.options[0].clone())
        }
        fn choose_seeing(
            &mut self,
            choice: &Choice,
            seen: &SeatObservation<'_>,
        ) -> Result<ChoiceOption, IllegalChoice> {
            let mut rng = ChaCha8Rng::seed_from_u64(1);
            let index = pick(choice, seen, &mut rng).unwrap_or(0);
            Ok(choice.options[index].clone())
        }
    }

    struct Probe(Option<usize>);
    impl Decider for Probe {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            Ok(choice.options[0].clone())
        }
        fn choose_seeing(
            &mut self,
            choice: &Choice,
            seen: &SeatObservation<'_>,
        ) -> Result<ChoiceOption, IllegalChoice> {
            self.0 = pick(choice, seen, &mut ChaCha8Rng::seed_from_u64(1));
            Ok(choice.options[0].clone())
        }
    }

    #[test]
    fn pick_always_scores_and_leaves_questions_without_an_opinion_to_the_random_pick() {
        let (state, hub, _, _) = board();
        let ask = |choice: &Choice| {
            ti4_engine::choice::ask_private(
                choice,
                &state,
                ContentStore::embedded(),
                POK,
                Some(&hub.galaxy),
                &mut Smart,
            )
            .expect("legal")
            .id
        };
        let score = Choice::new(
            me(),
            "score",
            vec![
                ChoiceOption::decline(),
                ChoiceOption::labelled("x", "score", "score x"),
            ],
        );
        assert_eq!(ask(&score), "x");

        let research = Choice::new(
            me(),
            "research",
            vec![
                ChoiceOption::labelled("t1", "research", "t1"),
                ChoiceOption::labelled("t2", "research", "t2"),
            ],
        );
        let mut probe = Probe(Some(99));
        ti4_engine::choice::ask_private(
            &research,
            &state,
            ContentStore::embedded(),
            POK,
            Some(&hub.galaxy),
            &mut probe,
        )
        .expect("legal");
        assert_eq!(probe.0, None, "no opinion on research");
    }
}
