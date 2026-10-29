//! borrowed: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. The two combat-round agents (`borrowed_round_agents.rs`, `owner_abilities` of the string engine): Evelyn De Louis
//! (Sol, "at the start of a ground combat round: exhaust to choose 1 ground force in the active system: it rolls
//! 1 additional die during this combat round", rules version 16) and Viscount Unlenn (Letnev, the same for
//! 1 ship in a space combat round, rules version 17).
//!
//! The owner chooses among every eligible roll-roster entry of the system: for Sol the ground forces of every
//! seat on every planet of the system, for Letnev the ships of every seat. The choice is a mark
//! (`MarkKey::YssarilSsruuRoundAgent(owner, agent, round_seq)`, the owner's earlier marks are dropped) that the
//! dice read in the round; the card is exhausted. A Yssaril copy (Ssruu) is another registered ability and stays
//! string.
//!
//! Not covered (the adapter bails): a Nomad that listens to agent exhaustion (the exhaustion is announced), and
//! a game with diplomacy promises on (the use of a leader is a promise event).

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt, RoundTarget, Subtype as LSub};
use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, Planet, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::query::reach;
use ti4_engine_core::state::{CoreState, EventKind, EventRec, Key, LeaderStatus};

/// Evelyn De Louis (`leader:sol:solagent:GROUND_COMBAT_ROUND_STARTED:after`).
pub const SOL_AGENT: Slot = Slot(9001);
/// Viscount Unlenn (`leader:letnev:letnevagent:COMBAT_ROUND_STARTED:after`).
pub const LETNEV_AGENT: Slot = Slot(9002);

/// `RULES_FACTION_EFFECTS` and `RULES_FACTION_BUGFIXES`.
const RULES_SOL: u32 = 16;
const RULES_LETNEV: u32 = 17;

fn sol_holds(s: &CoreState, who: Seat) -> bool {
    s.meta().rules_at_least(RULES_SOL) && s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheFederationOfSol)
}

fn letnev_holds(s: &CoreState, who: Seat) -> bool {
    s.meta().rules_at_least(RULES_LETNEV) && s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev)
}

const fn agent_of(ground: bool) -> Leader {
    if ground { Leader::EvelynDeLouis } else { Leader::ViscountUnlenn }
}

/// The units the owner may choose (`copy_plan`), in the string order, with the round the event names.
struct Plan {
    system: System,
    combat_planet: Option<Planet>,
    round_seq: u32,
    candidates: Vec<RoundTarget>,
}

fn readied(s: &CoreState, who: Seat, agent: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&agent) == Some(&LeaderStatus::Readied))
}

fn target(unit: ti4_engine_core::state::Piece, planet: Option<Planet>, index: usize) -> RoundTarget {
    RoundTarget {
        owner: unit.owner(),
        planet,
        unit: unit.kind(),
        index: u8::try_from(index).unwrap_or(u8::MAX),
        sustained: unit.sustained(),
        galvanized: unit.galvanized(),
    }
}

/// `copy_plan` for the owner's own agent: `None` when the ability is not eligible for this event.
fn plan(s: &CoreState, k: &Kernel, event: &EventRec, owner: Seat, ground: bool) -> Option<Plan> {
    let agent = agent_of(ground);
    if !(if ground { sol_holds(s, owner) } else { letnev_holds(s, owner) }) || !readied(s, owner, agent) {
        return None;
    }
    let p = &event.payload;
    let system = p.system(Key::System)?;
    let round_seq = u32::try_from(p.int(Key::RoundSeq)?).ok()?;
    if round_seq != s.meta().combat_round_seq {
        return None;
    }
    let (attacker, defender) = (p.seat(Key::Attacker)?, p.seat(Key::Defender)?);
    if attacker == defender || s.players().by_seat(attacker).is_none() || s.players().by_seat(defender).is_none() {
        return None;
    }
    let combat_planet = if ground {
        let planet = p.planet(Key::Planet)?;
        if s.board().planet(planet).units.is_empty() {
            return None;
        }
        Some(planet)
    } else {
        None
    };
    let rd = Rd::new(s, k).with_blanked(reach::blanked(s));
    let mut candidates = Vec::new();
    for seat in s.players().iter().map(|pl| pl.seat) {
        if ground {
            for planet in ti4_engine_core::rules::combat::planets_of_sorted(s, system).iter() {
                let roster = s
                    .board()
                    .planet(*planet)
                    .units
                    .iter()
                    .filter(|u| u.owner() == seat)
                    .filter(|u| ti4_engine_core::rules::invasion::is_ground_force_here(rd, seat, system, *planet, **u));
                candidates.extend(roster.enumerate().map(|(i, u)| target(*u, Some(*planet), i)));
            }
        } else {
            candidates.extend(ti4_engine_core::rules::combat::ships_of(rd, seat, system).into_iter().enumerate().map(|(i, u)| target(u, None, i)));
        }
    }
    (!candidates.is_empty()).then_some(Plan { system, combat_planet, round_seq, candidates })
}

fn condition(c: &CondCx<'_>, ground: bool) -> bool {
    if plan(c.state, c.kernel, c.event, c.def.owner, ground).is_none() {
        return false;
    }
    // `leaders::exhaust` announces the exhaustion to a watching Nomad; the core does not.
    if c.kernel.hooks.direct.nomad_watches_agent_exhaustion(c.state) {
        return c.unknown(Unknown::NomadAgentExhaustion);
    }
    true
}

/// The value of the mark (`RoundBonus`): borrower, source owner, agent, system, combat planet, unit planet, unit
/// owner, unit type, sustained, galvanized, ordinal, round. A planet that is absent is `Atom::Absent`.
fn bonus(owner: Seat, agent: Leader, plan: &Plan, t: &RoundTarget) -> MarkVal {
    let planet = |p: Option<Planet>| MarkVal::Atom(p.map_or(Atom::Absent, Atom::Planet));
    MarkVal::Rec(vec![
        MarkVal::Atom(Atom::Seat(owner)),
        MarkVal::Atom(Atom::Seat(owner)),
        MarkVal::Atom(Atom::Leader(agent)),
        MarkVal::Atom(Atom::System(plan.system)),
        planet(plan.combat_planet),
        planet(t.planet),
        MarkVal::Atom(Atom::Seat(t.owner)),
        MarkVal::Atom(Atom::Unit(t.unit)),
        MarkVal::Atom(Atom::Num(u32::from(t.sustained))),
        MarkVal::Atom(Atom::Num(u32::from(t.galvanized))),
        MarkVal::Atom(Atom::Num(u32::from(t.index))),
        MarkVal::Atom(Atom::Num(plan.round_seq)),
    ])
}

fn question(owner: Seat, agent: Leader, plan: &Plan) -> Choice {
    Choice {
        actor: owner,
        ask: Ask::Leaders(LAsk::RoundAgentUnit { agent }),
        ctx: Ctx {
            origin: Origin::Content(Content::Leader(agent)),
            subtype: Subtype::Leaders(LSub::RoundAgentUnit),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: plan.candidates.iter().map(|t| Opt::Leaders(LOpt::RoundAgentUnit(*t))).collect::<Opts>(),
    }
}

fn effect(eff: &mut Eff<'_, '_>, ground: bool) -> Result<(), TimingError> {
    let owner = eff.owner();
    let agent = agent_of(ground);
    let Some(plan) = ({
        let (s, k) = eff.read();
        plan(s, k, eff.event, owner, ground)
    }) else {
        return Ok(());
    };
    let Pick::Opt(Opt::Leaders(LOpt::RoundAgentUnit(chosen))) = eff.ask(&question(owner, agent, &plan)) else {
        return Ok(());
    };
    if !plan.candidates.contains(&chosen) {
        return Ok(());
    }
    // The owner's earlier marks go, the new one is keyed by the round.
    let value = bonus(owner, agent, &plan, &chosen);
    let marks = eff.state().marks_mut().tail.edit();
    let old: Vec<MarkKey> = marks
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| matches!(key, MarkKey::YssarilSsruuRoundAgent(b, ..) if *b == owner))
        .collect();
    for key in old {
        marks.remove(&key);
    }
    marks.insert(MarkKey::YssarilSsruuRoundAgent(owner, agent, DecimalTextOrd(plan.round_seq)), value);
    if let Some(p) = eff.state().players_mut().by_seat_mut(owner) {
        p.leaders.status.insert(agent, LeaderStatus::Exhausted);
    }
    Ok(())
}

/// Register both agents (before the reaction-card slots: the string registers them ahead of the per-seat slots).
pub fn register_abilities(t: &mut AbilityTable) {
    t.add(
        Spec::new(SOL_AGENT, AbilityKind::Leader, EventKind::GroundCombatRoundStarted, Relation::After, |e| effect(e, true))
            .applies(Applies::Held(sol_holds))
            .optional()
            .when(|c| condition(c, true)),
    );
    t.add(
        Spec::new(LETNEV_AGENT, AbilityKind::Leader, EventKind::CombatRoundStarted, Relation::After, |e| effect(e, false))
            .applies(Applies::Held(letnev_holds))
            .optional()
            .when(|c| condition(c, false)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agents_are_registered_for_their_factions_only() {
        let mut t = AbilityTable::default();
        register_abilities(&mut t);
        assert_eq!(t.specs().len(), 2);
        assert!(t.specs().iter().all(|s| s.optional && s.condition.is_some()));
    }
}
