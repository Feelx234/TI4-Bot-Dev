//! Rules for questions the rule bot used to answer at random (bot version 2).
//!
//! Each family sits behind its own switch in [`crate::tuning`] (`vote_rule`, `casualty_rule`,
//! `misc_rule`); with a switch off the question keeps the random pick, as in bot version 1.
//!
//! - **Agenda phase** (`vote_rule`): the outcome that helps the seat (an election of itself for
//!   a card or a point, its own planet for an attachment, a rival leader's planet or the rival
//!   leader for a harmful one, the points agendas by the standings), cast with every planet
//!   (planets are readied at the end of the agenda phase, so a vote costs nothing); no opinion:
//!   abstain. Trade goods are never spent on votes. Politics: the seat makes itself speaker, and
//!   puts an agenda it would like on top of the deck, one it would not at the bottom.
//! - **Combat** (`casualty_rule`): a hit is sustained whenever a unit can (the cheapest such
//!   unit first, keeping a flagship or war sun out of Direct Hit's way); a casualty is the least
//!   valuable unit, a damaged one before a fresh one of the same kind.
//! - **Small windows** (`misc_rule`): readying the most valuable planet, retreating to the
//!   system with most of its own, and Trade's free replenish for nobody else.

use ti4_engine::choice::{Choice, ChoiceOption, Observed};
use ti4_model::id::{PlanetId, PlayerId, SystemId};

use crate::{is_ship, payload_str, planet_worth, tuning};

/// The answer for one of the questions above, or `None` to keep the random pick.
pub fn pick(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let subtype = choice.context.as_ref().map(|c| c.subtype.as_str())?;
    match subtype {
        "cast_vote" | "vote_tiebreak" if tuning::vote_rule() => cast_vote(choice, seen, me),
        "vote_exhaust_planet" if tuning::vote_rule() => exhaust_for_vote(choice, seen, me),
        "vote_spend_trade_goods" if tuning::vote_rule() => {
            choice.options.iter().position(ChoiceOption::is_decline)
        }
        "politics_choose_speaker" if tuning::vote_rule() => {
            own_seat_option(choice, me).or_else(|| weakest_seat_option(choice, seen))
        }
        "politics_place_agenda" if tuning::vote_rule() => place_agenda(choice),
        "sustain_damage" if tuning::casualty_rule() => sustain(choice, seen),
        "assign_casualty" if tuning::casualty_rule() => casualty(choice, seen),
        "ready_planet" if tuning::misc_rule() => ready_best_planet(choice, seen),
        "retreat_to" if tuning::misc_rule() => retreat_to(choice, seen, me),
        "trade_choose_replenish" if tuning::misc_rule() => {
            choice.options.iter().position(|o| o.id == "done")
        }
        _ => None,
    }
}

// -- agenda phase ---------------------------------------------------------------------------------

/// The agenda a vote or a Politics placement is about (`details.agenda.id`).
fn agenda_of(choice: &Choice) -> Option<&str> {
    choice.details.get("agenda")?.get("id")?.as_str()
}

/// Elections whose winner gains something.
const GOOD_FOR_ELECTED: &[&str] = &[
    "secret",
    "committee",
    "arbiter",
    "minister_commerce",
    "minister_exploration",
    "minister_industry",
    "minister_peace",
    "minister_policy",
    "minister_sciences",
    "minister_war",
    "minister_antiquities",
    "prophecy",
    "shard_of_the_throne",
    "crown_of_emphidia",
    "crown_of_thalnos",
    "standardization",
    "censure",
    "grant_reallocation",
    "warrant",
];
/// Elections whose winner loses something.
const BAD_FOR_ELECTED: &[&str] = &["execution"];
/// Planet elections that make the planet better for its owner.
const GOOD_PLANET: &[&str] = &[
    "core_mining",
    "holy_planet_of_ixth",
    "rt_biotic",
    "rt_cybernetic",
    "rt_propulsion",
    "rt_warfare",
    "senate_sanctuary",
    "terraforming_initiative",
];
/// Planet elections that hurt the planet's owner.
const BAD_PLANET: &[&str] = &["redistribution", "disarmament", "demilitarized_zone"];

fn points(seen: &Observed<'_>, player: &PlayerId) -> i32 {
    seen.seat(player).map_or(0, |seat| seat.victory_points)
}

/// The rival with most points (ties: seating order).
fn rival_leader(seen: &Observed<'_>, me: &PlayerId) -> Option<PlayerId> {
    seen.players()
        .into_iter()
        .filter(|p| *p != me)
        .max_by_key(|p| (points(seen, p), std::cmp::Reverse((*p).clone())))
        .cloned()
}

fn planet_owner(seen: &Observed<'_>, planet: &str) -> Option<PlayerId> {
    let id = PlanetId::new(planet);
    seen.board()
        .values()
        .find_map(|system| system.planet_control.get(&id).cloned())
}

/// The outcome the seat wants on `agenda`, among `outcomes`; `None`: it does not care.
fn preferred_outcome(
    agenda: &str,
    outcomes: &[&str],
    seen: &Observed<'_>,
    me: &PlayerId,
) -> Option<String> {
    let offered = |id: &str| outcomes.contains(&id).then(|| id.to_owned());
    if GOOD_FOR_ELECTED.contains(&agenda) {
        return offered(me.as_str());
    }
    if BAD_FOR_ELECTED.contains(&agenda) {
        return offered(rival_leader(seen, me)?.as_str());
    }
    if GOOD_PLANET.contains(&agenda) || BAD_PLANET.contains(&agenda) {
        let good = GOOD_PLANET.contains(&agenda);
        let leader = rival_leader(seen, me);
        return outcomes
            .iter()
            .filter(|o| **o != "for" && **o != "against")
            .filter(|o| {
                let owner = planet_owner(seen, o);
                if good {
                    owner.as_ref() == Some(me)
                } else {
                    owner.is_some() && owner.as_ref() == leader.as_ref()
                }
            })
            .max_by(|a, b| {
                planet_worth(seen, a)
                    .total_cmp(&planet_worth(seen, b))
                    .then_with(|| b.cmp(a))
            })
            .map(|o| (*o).to_owned());
    }
    let mine = points(seen, me);
    let all: Vec<i32> = seen.players().iter().map(|p| points(seen, p)).collect();
    let top = all.iter().copied().max().unwrap_or(0);
    let side = match agenda {
        // A point for each "For" voter if it passes.
        "mutiny" | "unconventional" | "articles_war" | "incentive" => "for",
        // The leader or the last gains a point: never hand a rival the lead or the win.
        "seed_empire" => {
            if mine >= top {
                "for"
            } else {
                "against"
            }
        }
        "economic_equality" => {
            let goods = seen.seat(me).map_or(0, |seat| seat.trade_goods);
            if goods < 5 { "for" } else { "against" }
        }
        _ => return None,
    };
    offered(side)
}

fn outcome_ids(choice: &Choice) -> Vec<&str> {
    choice
        .options
        .iter()
        .filter(|o| !o.is_decline())
        .map(|o| o.id.as_str())
        .collect()
}

/// Vote for the preferred outcome, else abstain (a tie-break, which cannot abstain, keeps the
/// random pick when the seat does not care).
fn cast_vote(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let agenda = agenda_of(choice)?;
    let outcomes = outcome_ids(choice);
    if let Some(want) = preferred_outcome(agenda, &outcomes, seen, me) {
        return choice.options.iter().position(|o| o.id == want);
    }
    choice.options.iter().position(ChoiceOption::is_decline)
}

/// Votes a planet option casts (its label: "exhaust X for N votes").
fn votes_of(option: &ChoiceOption) -> i64 {
    option
        .label
        .split(" for ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Every planet for the outcome it wants (they are readied after the agenda phase), the biggest
/// first; none for any other outcome.
fn exhaust_for_vote(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    let outcome = choice
        .prompt
        .strip_prefix("exhaust a planet to vote ")?
        .trim()
        .to_owned();
    let agenda = agenda_of(choice)?;
    // The outcomes are not listed here; the preference only needs the one being voted.
    let outcomes = [outcome.as_str()];
    let wanted = preferred_outcome(agenda, &outcomes, seen, me).is_some_and(|w| w == outcome);
    if !wanted {
        return choice.options.iter().position(ChoiceOption::is_decline);
    }
    choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.is_decline())
        .max_by_key(|(i, o)| (votes_of(o), std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
}

/// The option that names the seat itself (`details.seats`: name -> seat).
fn own_seat_option(choice: &Choice, me: &PlayerId) -> Option<usize> {
    let seats = choice.details.get("seats")?.as_object()?;
    choice.options.iter().position(|o| {
        seats
            .get(&o.id)
            .and_then(|seat| seat.as_str())
            .is_some_and(|seat| seat == me.as_str())
    })
}

/// Already the speaker: the seat with fewest points gets the token (the least dangerous one).
fn weakest_seat_option(choice: &Choice, seen: &Observed<'_>) -> Option<usize> {
    let seats = choice.details.get("seats")?.as_object()?;
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let seat = PlayerId::new(seats.get(&o.id)?.as_str()?);
            Some((i, points(seen, &seat)))
        })
        .min_by_key(|&(i, p)| (p, i))
        .map(|(i, _)| i)
}

/// Politics' look at the agenda deck: an agenda with an outcome the seat wants goes on top, one
/// that harms it (an election of a rival for a card, a point for the leader) at the bottom.
fn place_agenda(choice: &Choice) -> Option<usize> {
    let agenda = agenda_of(choice)?;
    let good = GOOD_FOR_ELECTED.contains(&agenda)
        || GOOD_PLANET.contains(&agenda)
        || matches!(agenda, "mutiny" | "unconventional" | "incentive");
    let id = if good { "top" } else { "bottom" };
    choice.options.iter().position(|o| o.id == id)
}

// -- combat ---------------------------------------------------------------------------------------

fn value_of(seen: &Observed<'_>, unit: &str) -> f64 {
    ti4_engine::valuation::unit_value(seen.content(), seen.sources(), unit)
}

fn is_flagship_or_war_sun(unit: &str) -> bool {
    unit.contains("flagship") || unit.contains("warsun") || unit.contains("war_sun")
}

/// Sustain whenever a unit can, on the cheapest unit that can (a flagship or war sun last).
fn sustain(choice: &Choice, seen: &Observed<'_>) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.is_decline())
        .filter_map(|(i, o)| {
            let unit = payload_str(o, "unit")?;
            let rank = value_of(seen, unit) + if is_flagship_or_war_sun(unit) { 100.0 } else { 0.0 };
            Some((i, rank))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(i, _)| i)
}

/// The least valuable casualty; a damaged unit counts as worth less than a fresh one.
fn casualty(choice: &Choice, seen: &Observed<'_>) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let unit = payload_str(o, "unit")?;
            let damaged = o.payload.get("damaged").and_then(serde_json::Value::as_bool) == Some(true);
            let value = value_of(seen, unit) * if damaged { 0.7 } else { 1.0 };
            Some((i, value))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(i, _)| i)
}

// -- small windows --------------------------------------------------------------------------------

fn ready_best_planet(choice: &Choice, seen: &Observed<'_>) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter_map(|(i, o)| {
            let record = ti4_content::galaxy::planet(seen.content(), &o.id, seen.sources())?;
            let (r, f) = (record.resources(), record.influence());
            Some((i, 2 * r.max(f) + r.min(f)))
        })
        .max_by_key(|&(i, v)| (v, std::cmp::Reverse(i)))
        .map(|(i, _)| i)
}

/// Retreat where most of its own is: planets it controls, its ships, a dock.
fn retreat_to(choice: &Choice, seen: &Observed<'_>, me: &PlayerId) -> Option<usize> {
    choice
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| !o.is_decline())
        .map(|(i, o)| {
            let board = seen.system(&SystemId::new(&o.id));
            let planets = board.planet_control.values().filter(|p| *p == me).count();
            let ships = board
                .units
                .iter()
                .filter(|u| &u.owner == me && is_ship(seen, u))
                .count();
            (i, 3 * planets + ships)
        })
        .max_by_key(|&(i, v)| (v, std::cmp::Reverse(i)))
        .map(|(i, _)| i)
}
