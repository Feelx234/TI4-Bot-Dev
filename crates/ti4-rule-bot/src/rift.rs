//! Gravity-rift risk model (bot version 6, [`crate::tuning::rift_risk`]; docs/BOT_GRAVITY_RIFT.md).
//!
//! A ship whose route exits a gravity rift rolls a die for each one (41.2): `1`-`3` of ten
//! destroys it and everything aboard, so a single exit costs the ship 30 %, two exits 51 %. The
//! engine chooses the route; the bot's choice is which ships sail and what they carry. Every ship
//! that could move into a system is *safe* (no exit) or *risky*. The risky ones are admitted one
//! by one, cheapest expected loss first, while the worth they add to the activation
//! ([`crate::activation_value_with`] with and without them, times
//! [`crate::tuning::rift_gain_scale`]) beats their expected loss times
//! [`crate::tuning::rift_margin`]; when no single ship pays, the cheapest prefix of the rest is
//! tried as a group (a fight that needs two rift ships to win). A ship that adds nothing (a fight
//! the safe ships win, a planet a safe carrier takes) is never admitted. The same set serves
//! activation (what can come), the movement step (which move options are offered) and the fight
//! check, so the three cannot disagree.

use ti4_engine::choice::{Choice, Observed};
use ti4_engine::tactical::Movable;
use ti4_model::id::{PlayerId, SystemId};

use crate::{activation_value_with, has_hostile_ships, is_ground, payload_str, tuning, unit_type};

/// Chance that one rift exit destroys the ship (the engine's own numbers).
#[must_use]
pub fn exit_p() -> f64 {
    f64::from(ti4_engine::transit::RIFT_DESTROYS_ON) / f64::from(ti4_engine::dice::SIDES)
}

/// Chance that a route with `exits` rift exits destroys the ship.
#[must_use]
pub fn p_destroy(exits: usize) -> f64 {
    1.0 - (1.0 - exit_p()).powi(i32::try_from(exits).unwrap_or(i32::MAX))
}

fn cost_of(seen: &Observed<'_>, type_id: &str) -> f64 {
    unit_type(seen, type_id).map_or(1.0, |t| t.cost())
}

/// Resource value of the ship, a flagship or war sun weighted up (there is one of each).
fn ship_value(seen: &Observed<'_>, type_id: &str) -> f64 {
    let base = cost_of(seen, type_id);
    if type_id.contains("flagship") || type_id.contains("warsun") {
        base * tuning::rift_flagship_weight()
    } else {
        base
    }
}

/// Resources aboard when the ship sails: ground forces (and fighters for a fight) it could take
/// from its origin, as many as it has room for.
fn cargo_value(seen: &Observed<'_>, me: &PlayerId, m: &Movable, contested: bool) -> f64 {
    let room = usize::try_from(m.capacity).unwrap_or(0);
    if room == 0 {
        return 0.0;
    }
    let mut cargo: Vec<f64> = seen
        .loadable(me, &m.origin)
        .iter()
        .filter(|c| is_ground(seen, &c.unit) || (contested && fighter(seen, &c.unit)))
        .map(|c| cost_of(seen, c.unit.type_id.as_str()))
        .collect();
    cargo.sort_by(|a, b| b.total_cmp(a));
    cargo.into_iter().take(room).sum()
}

fn fighter(seen: &Observed<'_>, unit: &ti4_model::units::Unit) -> bool {
    unit_type(seen, unit.type_id.as_str()).is_some_and(|t| t.is_fighter())
}

/// Rift exits on the way of `m` into `dest`.
fn exits(seen: &Observed<'_>, me: &PlayerId, m: &Movable, dest: &SystemId) -> usize {
    seen.rift_exits_into(
        me,
        &m.origin,
        m.unit.type_id.as_str(),
        (m.gravity_drive, m.ionian),
        dest,
    )
}

/// The expected loss of sending `m` with `exits` rift exits: chance of destruction times the
/// ship and its cargo.
fn expected_loss(
    seen: &Observed<'_>,
    me: &PlayerId,
    m: &Movable,
    exits: usize,
    contested: bool,
) -> f64 {
    p_destroy(exits)
        * (ship_value(seen, m.unit.type_id.as_str()) + cargo_value(seen, me, m, contested))
}

/// `movable` less the ships whose rift risk the worth they add does not cover (see the module).
/// Unchanged without the knob or when no ship exits a rift.
pub(crate) fn prune(
    seen: &Observed<'_>,
    me: &PlayerId,
    dest: &SystemId,
    movable: Vec<Movable>,
) -> Vec<Movable> {
    if !tuning::rift_risk() || movable.is_empty() {
        return movable;
    }
    let counts: Vec<usize> = movable.iter().map(|m| exits(seen, me, m, dest)).collect();
    if counts.iter().all(|n| *n == 0) {
        return movable;
    }
    let contested = has_hostile_ships(seen, me, dest);
    let pick =
        |ids: &[usize]| -> Vec<Movable> { ids.iter().map(|i| movable[*i].clone()).collect() };
    // Negative values (a lost fight) count as nothing: the gain is what the ships add.
    let value = |ids: &[usize]| activation_value_with(seen, me, dest, pick(ids)).max(0.0);
    let loss: Vec<f64> = movable
        .iter()
        .zip(&counts)
        .map(|(m, n)| expected_loss(seen, me, m, *n, contested))
        .collect();

    let mut chosen: Vec<usize> = (0..movable.len()).filter(|i| counts[*i] == 0).collect();
    let mut rest: Vec<usize> = (0..movable.len()).filter(|i| counts[*i] > 0).collect();
    rest.sort_by(|a, b| loss[*a].total_cmp(&loss[*b]).then(a.cmp(b)));
    let scale = tuning::rift_gain_scale();
    let margin = tuning::rift_margin();
    let mut base = value(&chosen);
    // One ship at a time: the best net gain first.
    loop {
        let mut best: Option<(usize, f64, f64)> = None;
        for (slot, &r) in rest.iter().enumerate() {
            let mut with = chosen.clone();
            with.push(r);
            let after = value(&with);
            let net = scale * (after - base) - margin * loss[r];
            if net > 0.0 && best.is_none_or(|(_, n, _)| net > n) {
                best = Some((slot, net, after));
            }
        }
        let Some((slot, _, after)) = best else { break };
        chosen.push(rest.remove(slot));
        base = after;
    }
    // A group: some fights need two rift ships to be won at all.
    let mut best_group: Option<(usize, f64)> = None;
    let mut with = chosen.clone();
    let mut group_loss = 0.0;
    for (k, &r) in rest.iter().enumerate() {
        with.push(r);
        group_loss += loss[r];
        let net = scale * (value(&with) - base) - margin * group_loss;
        if net > 0.0 && best_group.is_none_or(|(_, n)| net > n) {
            best_group = Some((k + 1, net));
        }
    }
    if let Some((k, _)) = best_group {
        chosen.extend(rest.drain(..k));
    }
    chosen.sort_unstable();
    chosen.into_iter().map(|i| movable[i].clone()).collect()
}

/// What the ships in `movable` (already pruned) are expected to lose to rifts on their way into
/// `dest`, in activation-worth points (loss divided by [`tuning::rift_gain_scale`]): taken off the
/// system's worth so that a target behind no rift beats an equal one behind a rift.
pub(crate) fn penalty(
    seen: &Observed<'_>,
    me: &PlayerId,
    dest: &SystemId,
    movable: &[Movable],
) -> f64 {
    if !tuning::rift_risk() {
        return 0.0;
    }
    let contested = has_hostile_ships(seen, me, dest);
    let total: f64 = movable
        .iter()
        .map(|m| (m, exits(seen, me, m, dest)))
        .filter(|(_, n)| *n > 0)
        .map(|(m, n)| expected_loss(seen, me, m, n, contested))
        .sum();
    total / tuning::rift_gain_scale().max(f64::EPSILON)
}

/// The movement-step options to keep: every move that exits no rift, and the risky ones the
/// pruned set still includes. `None` when the knob is off or nothing is dropped.
pub(crate) fn keep(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<Vec<usize>> {
    if !tuning::rift_risk() {
        return None;
    }
    let active = seen.active_system()?;
    let risky = choice.details.get("rift_exits")?.as_object()?;
    if risky.is_empty() {
        return None;
    }
    let allowed = prune(seen, me, active, seen.movable_into(me, active));
    let keep: Vec<usize> = choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            if o.kind != "move" || !risky.contains_key(&o.id) {
                return true;
            }
            let (Some(origin), Some(unit)) = (payload_str(o, "origin"), payload_str(o, "unit"))
            else {
                return true;
            };
            let flag = |key: &str| {
                o.payload
                    .get(key)
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            };
            allowed.iter().any(|m| {
                m.origin.as_str() == origin
                    && m.unit.type_id.as_str() == unit
                    && m.gravity_drive == flag("gravity_drive")
                    && m.ionian == flag("ionian")
            })
        })
        .map(|(i, _)| i)
        .collect();
    (keep.len() < choice.options.len() && !keep.is_empty()).then_some(keep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{self, Tuning};
    use ti4_content::ContentStore;
    use ti4_engine::fixtures::{Hub, a_system_where, game, hub_from, plain_systems, put};
    use ti4_model::POK;
    use ti4_model::state::GameState;

    fn me() -> PlayerId {
        PlayerId::new("a")
    }

    fn on() -> Tuning {
        Tuning {
            rift_risk: true,
            ..tuning::current()
        }
    }

    /// The richest free planet alone in a plain system, with its worth (resources + influence + 0.5).
    fn rich_system(poor: bool) -> (String, f64) {
        let mut best: Vec<(String, f64)> = plain_systems(80)
            .into_iter()
            .filter_map(|id| {
                let planets = ti4_content::galaxy::planets_in(ContentStore::embedded(), &id, POK);
                let [planet] = planets.as_slice() else {
                    return None;
                };
                (planet.homeworld_of().is_none() && planet.id() != crate::MECATOL).then(|| {
                    (
                        id.clone(),
                        (planet.resources() + planet.influence()) as f64 + 0.5,
                    )
                })
            })
            .collect();
        best.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        if poor {
            best.into_iter().find(|b| b.1 >= 1.5).expect("a planet")
        } else {
            best.pop().expect("a planet")
        }
    }

    /// `centre` (the system sailed into) with a ring whose first system is a gravity rift.
    fn rift_ring(centre: &str) -> (GameState, Hub, SystemId) {
        let rift = a_system_where("gravity rift");
        let mut ids = vec![centre.to_owned(), rift.clone()];
        ids.extend(
            plain_systems(80)
                .into_iter()
                .filter(|id| {
                    id != centre
                        && id != &rift
                        && ti4_content::galaxy::planets_in(ContentStore::embedded(), id, POK)
                            .is_empty()
                })
                .take(5),
        );
        let hub = hub_from(&ids);
        let mut state = game(&["a", "b"]);
        let centre = SystemId::new(centre);
        state.active_system = Some(centre.clone());
        (state, hub, centre)
    }

    fn observe<'a>(state: &'a GameState, hub: &'a Hub) -> Observed<'a> {
        Observed::new(state, ContentStore::embedded(), POK, Some(&hub.galaxy))
    }

    fn rift_of(hub: &Hub) -> SystemId {
        SystemId::new(hub.outer[0].clone())
    }

    fn plain_of(hub: &Hub) -> SystemId {
        SystemId::new(hub.outer[1].clone())
    }

    /// The movement step the engine would offer, with its rift facts.
    fn step(state: &GameState, hub: &Hub, centre: &SystemId) -> Choice {
        let seen = observe(state, hub);
        let choice =
            ti4_engine::tactical::movement_options(&me(), &seen.movable_into(&me(), centre));
        ti4_engine::tactical::preview_moves(
            state,
            ContentStore::embedded(),
            POK,
            &hub.galaxy,
            &me(),
            centre,
            choice,
        )
    }

    fn origins(choice: &Choice, keep: &Option<Vec<usize>>) -> Vec<String> {
        choice
            .options
            .iter()
            .enumerate()
            .filter(|(i, _)| keep.as_ref().is_none_or(|k| k.contains(i)))
            .filter_map(|(_, o)| payload_str(o, "origin").map(str::to_owned))
            .collect()
    }

    #[test]
    fn the_engine_counts_the_exits_of_a_route() {
        let (centre, _) = rich_system(false);
        let (mut state, hub, centre) = rift_ring(&centre);
        put(&mut state, &rift_of(&hub), "carrier", &me(), 1);
        put(&mut state, &plain_of(&hub), "carrier", &me(), 1);
        let seen = observe(&state, &hub);
        assert_eq!(
            seen.rift_exits_into(&me(), &rift_of(&hub), "carrier", (false, false), &centre),
            1
        );
        assert_eq!(
            seen.rift_exits_into(&me(), &plain_of(&hub), "carrier", (false, false), &centre),
            0
        );
        let choice = step(&state, &hub, &centre);
        assert_eq!(
            choice.details["rift_exits"].as_object().map(|m| m.len()),
            Some(1)
        );
        assert!((p_destroy(1) - 0.3).abs() < 1e-9 && (p_destroy(2) - 0.51).abs() < 1e-9);
    }

    #[test]
    fn an_equal_safe_route_beats_the_rift() {
        let (centre, _) = rich_system(false);
        let (mut state, hub, centre) = rift_ring(&centre);
        for system in [rift_of(&hub), plain_of(&hub)] {
            put(&mut state, &system, "carrier", &me(), 1);
            put(&mut state, &system, "infantry", &me(), 2);
        }
        let choice = step(&state, &hub, &centre);
        tuning::with(on(), || {
            let seen = observe(&state, &hub);
            let keep = keep(&choice, &seen, &me());
            assert_eq!(origins(&choice, &keep), vec![plain_of(&hub).to_string()]);
        });
        // Without the knob (version 5) nothing is dropped.
        tuning::with(tuning::for_version(5), || {
            let seen = observe(&state, &hub);
            assert!(keep(&choice, &seen, &me()).is_none());
        });
    }

    #[test]
    fn a_low_value_move_through_a_rift_is_refused() {
        let (centre, _) = rich_system(true);
        let (mut state, hub, centre) = rift_ring(&centre);
        // A warship with nothing to win, and a carrier for a planet worth less than the loss.
        put(&mut state, &rift_of(&hub), "dreadnought", &me(), 1);
        let choice = step(&state, &hub, &centre);
        tuning::with(on(), || {
            let seen = observe(&state, &hub);
            assert!(prune(&seen, &me(), &centre, seen.movable_into(&me(), &centre)).is_empty());
            let keep = keep(&choice, &seen, &me());
            assert_eq!(
                keep.as_ref().map(Vec::len),
                Some(1),
                "only 'finish movement' is left"
            );
        });
    }

    #[test]
    fn a_large_gain_still_crosses() {
        let (centre, worth) = rich_system(false);
        let (mut state, hub, centre) = rift_ring(&centre);
        put(&mut state, &rift_of(&hub), "carrier", &me(), 1);
        put(&mut state, &rift_of(&hub), "infantry", &me(), 2);
        let mut t = on();
        // The gain of this planet beats three times the carrier's and troops' expected loss.
        t.rift_margin = (worth / (p_destroy(1) * 4.0)) * 0.9;
        tuning::with(t, || {
            let seen = observe(&state, &hub);
            let kept = prune(&seen, &me(), &centre, seen.movable_into(&me(), &centre));
            assert_eq!(kept.len(), 1, "worth {worth}");
        });
    }

    #[test]
    fn a_loaded_carrier_is_protected_more_than_an_empty_one() {
        let (centre, worth) = rich_system(false);
        let (mut state, hub, centre) = rift_ring(&centre);
        put(&mut state, &rift_of(&hub), "carrier", &me(), 1);
        put(&mut state, &rift_of(&hub), "infantry", &me(), 4);
        let seen = observe(&state, &hub);
        let m = seen.movable_into(&me(), &centre).remove(0);
        let loaded = tuning::with(on(), || expected_loss(&seen, &me(), &m, 1, false));
        let mut bare = state.clone();
        bare.system_mut(&rift_of(&hub))
            .units
            .retain(|u| u.type_id.as_str() != "infantry");
        let seen_bare = observe(&bare, &hub);
        let empty = tuning::with(on(), || expected_loss(&seen_bare, &me(), &m, 1, false));
        assert!(loaded > empty + 0.5, "{loaded} vs {empty}");
        // A margin between the two losses: the carrier with two troops goes, with four it stays.
        let mut few = state.clone();
        few.system_mut(&rift_of(&hub))
            .units
            .retain(|u| u.type_id.as_str() != "infantry");
        put(&mut few, &rift_of(&hub), "infantry", &me(), 2);
        let seen_few = observe(&few, &hub);
        let loss2 = tuning::with(on(), || expected_loss(&seen_few, &me(), &m, 1, false));
        let mut t = on();
        t.rift_margin = worth / ((loss2 + loaded) / 2.0);
        tuning::with(t, || {
            assert_eq!(
                prune(
                    &seen_few,
                    &me(),
                    &centre,
                    seen_few.movable_into(&me(), &centre)
                )
                .len(),
                1
            );
            assert!(prune(&seen, &me(), &centre, seen.movable_into(&me(), &centre)).is_empty());
        });
    }

    #[test]
    fn the_unaffected_system_keeps_every_move() {
        let (centre, _) = rich_system(false);
        let (mut state, hub, centre) = rift_ring(&centre);
        put(&mut state, &plain_of(&hub), "carrier", &me(), 1);
        put(&mut state, &plain_of(&hub), "infantry", &me(), 2);
        let choice = step(&state, &hub, &centre);
        tuning::with(on(), || {
            let seen = observe(&state, &hub);
            assert!(keep(&choice, &seen, &me()).is_none());
        });
    }
}
