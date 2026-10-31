//! mahact: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: the Crimson Legionnaire and
//! its upgrade, "After this unit is destroyed, gain 1 commodity or convert 1 of your commodities to a trade good":
//! mandatory; the owner chooses between the two only when both are possible (a commodity can be gained below the
//! faction's value, converted when at least one is held), when neither is possible there is nothing to take. The II
//! then waits on the card (the owner's `MahactCl2` count; the return at the start of the next turn is the string's).
//!
//! Registered for every seat, as the string does (the condition names the unit and its owner).
//!
//! Not covered (the window bails): a Firmament or Obsidian seat, whose Siphon plot watches the commodities gained.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::rules::strategy::commodity_limit;
use ti4_engine_core::state::{EventKind, Key};

/// Crimson Legionnaire (`unit:mahact:mahact_infantry:GROUND_FORCE_DESTROYED:after`).
pub const LEGIONNAIRE: Slot = Slot(5001);
/// Crimson Legionnaire II (`unit:mahact:mahact_infantry2:GROUND_FORCE_DESTROYED:after`).
pub const LEGIONNAIRE_II: Slot = Slot(5002);

/// Edict, a win of a space combat (`ability:*:edict:SPACE_COMBAT_WON:when`) and the end of a ground combat (`...:GROUND_COMBAT_ENDED:when`).
pub const EDICT_SPACE: Slot = Slot(5003);
pub const EDICT_GROUND: Slot = Slot(5004);

const fn unit_of(slot: Slot) -> Unit {
    if slot.0 == LEGIONNAIRE_II.0 { Unit::CrimsonLegionnaireII } else { Unit::CrimsonLegionnaireI }
}

fn condition(c: &CondCx<'_>) -> bool {
    let slot = Slot(u16::try_from(c.def.arg).unwrap_or(0));
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && c.event.payload.unit(Key::Unit) == Some(unit_of(slot))
}

fn watchers_of_commodities(s: &ti4_engine_core::state::CoreState) -> bool {
    let watchers = s.players().iter().find(|p| matches!(p.faction, Faction::TheFirmament | Faction::TheObsidian));
    if let Some(p) = watchers {
        ti4_engine_core::unsupported::faction_in_play(p.faction);
    }
    watchers.is_some()
}

fn legionnaire(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let slot = Slot(u16::try_from(eff.def.arg).unwrap_or(0));
    let second = slot.0 == LEGIONNAIRE_II.0;
    if watchers_of_commodities(eff.state()) {
        eff.unknown(Unknown::MissingFact);
        return Ok(());
    }
    let (held, limit) = {
        let s = eff.state();
        let Some(p) = s.players().by_seat(me) else { return Ok(()) };
        (i32::from(p.econ.commodities), commodity_limit(p.faction))
    };
    let (can_gain, can_convert) = (held < limit, held > 0);
    let convert = if can_gain && can_convert {
        let source = if second { eff.codes().mahact_infantry2 } else { eff.codes().mahact_infantry };
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::Legionnaire { second }),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Other(source)),
                subtype: Subtype::Reaction(RSub::Legionnaire),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: [Opt::Reaction(ROpt::Gain), Opt::Reaction(ROpt::Convert)].into_iter().collect::<Opts>(),
        };
        matches!(eff.ask(&choice), Pick::Opt(Opt::Reaction(ROpt::Convert)))
    } else {
        can_convert
    };
    if convert {
        if let Some(p) = eff.state().players_mut().by_seat_mut(me) {
            p.econ.commodities -= 1;
        }
        let source = if second { eff.codes().mahact_infantry2 } else { eff.codes().mahact_infantry };
        fx::gain_trade_goods_staged(eff.state(), me, 1, source);
    } else if can_gain && let Some(p) = eff.state().players_mut().by_seat_mut(me) {
        p.econ.commodities += 1;
    }
    if second {
        let now = match eff.state().marks().tail.get(&MarkKey::MahactCl2(me)) {
            Some(MarkVal::Atom(Atom::Num(n))) => *n,
            _ => 0,
        } + 1;
        eff.state().marks_mut().tail.edit().insert(MarkKey::MahactCl2(me), MarkVal::Atom(Atom::Num(now)));
    }
    Ok(())
}

// -- Edict ----------------------------------------------------------------------------------------------

/// `combat_win`: the winner of the combat the event reports and its opponents. A space win names `player` and the list of `opponents`; a
/// ground combat names `winner` with its `attacker` and `defender`.
fn combat_win(c: &ti4_engine_core::state::Payload, opponents_key: ti4_engine_core::state::Code) -> Option<(Seat, Vec<Seat>)> {
    use ti4_engine_core::state::List;
    if let Some(winner) = c.seat(Key::Winner) {
        let opponents = [Key::Attacker, Key::Defender].into_iter().filter_map(|k| c.seat(k)).filter(|side| *side != winner).collect();
        return Some((winner, opponents));
    }
    let winner = c.seat(Key::Player)?;
    let opponents = match c.list(Key::Other(opponents_key)) {
        Some(List::Seats(list)) => list.as_slice().to_vec(),
        _ => Vec::new(),
    };
    Some((winner, opponents))
}

/// `edict_targets`: the opponents whose token Edict could take now for `mahact`: it won, the opponent's token is in their reinforcements, the
/// pool holds none of theirs yet. Sorted, once each.
fn edict_targets(s: &ti4_engine_core::state::CoreState, mahact: Seat, p: &ti4_engine_core::state::Payload, opponents_key: ti4_engine_core::state::Code) -> Vec<Seat> {
    let Some((winner, opponents)) = combat_win(p, opponents_key) else { return Vec::new() };
    if winner != mahact || !crate::cc_ground::mahact::query::is_mahact(s, mahact) {
        return Vec::new();
    }
    let held = crate::cc_ground::mahact::query::fleet_pool_owners(s, mahact);
    let mut targets: Vec<Seat> = opponents.into_iter().filter(|o| *o != mahact && s.players().by_seat(*o).is_some() && !held.contains(o) && fx::tokens_in_reinforcements(s, *o) > 0).collect();
    targets.sort_by_key(|seat| seat.0);
    targets.dedup();
    targets
}

fn edict_when(c: &CondCx<'_>) -> bool {
    !edict_targets(c.state, c.def.owner, &c.event.payload, c.facts.combat.opponents).is_empty()
}

/// "Place 1 command token from your opponent's reinforcements in your fleet pool if it does not already contain 1 of that player's tokens."
fn edict(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let p = eff.event.payload.clone();
    let key = eff.adapter_facts().combat.opponents;
    let targets = edict_targets(eff.state(), me, &p, key);
    let target = match targets.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        _ => {
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::Edict),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Other(eff.codes().activation.edict)),
                    subtype: Subtype::Reaction(RSub::Edict),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: targets.iter().map(|who| Opt::Reaction(ROpt::Opponent(*who))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Reaction(ROpt::Opponent(who))) = eff.ask(&choice) else { return Ok(()) };
            who
        }
    };
    if targets.contains(&target) {
        let mut owners: Vec<Seat> = crate::cc_ground::mahact::query::fleet_pool_owners(eff.state(), me).iter().copied().collect();
        owners.push(target);
        owners.sort_by_key(|seat| seat.0);
        eff.state().marks_mut().tail.edit().insert(MarkKey::MahactFleet(me), MarkVal::List(owners.into_iter().map(|o| MarkVal::Atom(Atom::Seat(o))).collect()));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(EDICT_SPACE, AbilityKind::Ability, EventKind::SpaceCombatWon, Relation::When, edict).when(edict_when));
    t.add(Spec::new(EDICT_GROUND, AbilityKind::Ability, EventKind::GroundCombatEnded, Relation::When, edict).when(edict_when));
    for slot in [LEGIONNAIRE, LEGIONNAIRE_II] {
        t.add(
            Spec::new(slot, AbilityKind::Unit, EventKind::GroundForceDestroyed, Relation::After, legionnaire)
                                .when(condition),
        );
    }
}

#[cfg(test)]
mod edict_tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::state::{Code, List, Payload, Val};

    const KEY: Code = Code(7);

    /// A space win of `winner` over `opponents` (the payload the aftermath announces).
    fn space_win(winner: u8, opponents: &[u8]) -> Payload {
        let list: SmallList<Seat, 4> = opponents.iter().map(|o| Seat(*o)).collect();
        Payload::new().with(Key::Player, Val::Seat(Seat(winner))).with_list(Key::Other(KEY), List::Seats(list))
    }

    #[test]
    fn edict_takes_the_token_of_each_opponent_that_has_one_and_is_not_in_the_pool_yet() {
        let (mut s, _) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan]);
        assert_eq!(edict_targets(&s, Seat(0), &space_win(0, &[2, 1, 1]), KEY), vec![Seat(1), Seat(2)], "sorted, once each");
        assert!(edict_targets(&s, Seat(0), &space_win(1, &[0]), KEY).is_empty(), "the opponent won");
        assert!(edict_targets(&s, Seat(1), &space_win(1, &[0]), KEY).is_empty(), "not the Mahact");
        // A token already in the pool, and an opponent with none in the reinforcements.
        s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), MarkVal::List(vec![MarkVal::Atom(Atom::Seat(Seat(1)))]));
        assert_eq!(edict_targets(&s, Seat(0), &space_win(0, &[1, 2]), KEY), vec![Seat(2)]);
        s.players_mut().by_seat_mut(Seat(2)).unwrap().econ.tactic_tokens = 16;
        assert!(edict_targets(&s, Seat(0), &space_win(0, &[1, 2]), KEY).is_empty(), "no token left in the reinforcements of seat 2");
    }

    #[test]
    fn a_ground_combat_names_its_winner_with_the_attacker_and_the_defender() {
        let (s, _) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheFederationOfSol]);
        let ground = |winner: Option<u8>| {
            let mut p = Payload::new().with(Key::Attacker, Val::Seat(Seat(1))).with(Key::Defender, Val::Seat(Seat(0)));
            if let Some(w) = winner {
                p.set(Key::Winner, Val::Seat(Seat(w)));
            }
            p
        };
        assert_eq!(edict_targets(&s, Seat(0), &ground(Some(0)), KEY), vec![Seat(1)]);
        assert!(edict_targets(&s, Seat(0), &ground(Some(1)), KEY).is_empty());
        assert!(edict_targets(&s, Seat(0), &ground(None), KEY).is_empty(), "no winner named, and no opponents list: a draw");
    }
}

#[cfg(test)]
mod unsupported_tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::unsupported::{Policy, with_policy};

    fn panics_with(name: &str, f: impl FnOnce()) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::panic::set_hook(prev);
        r.err().is_some_and(|e| {
            e.downcast_ref::<String>().is_some_and(|t| t.contains(name))
                || e.downcast_ref::<&str>().is_some_and(|t| t.contains(name))
        })
    }

    /// XF-3 family: the Firmament and the Obsidian are switched off.
    #[test]
    fn the_commodities_watchers_panic_for_a_firmament_or_obsidian_seat() {
        for (f, name) in [(Faction::TheFirmament, "Firmament"), (Faction::TheObsidian, "Obsidian")] {
            let (s, _) = table(&[Faction::TheArborec, f]);
            assert!(with_policy(Policy::Off, || watchers_of_commodities(&s)));
            assert!(panics_with(name, || with_policy(Policy::Panic, || { watchers_of_commodities(&s); })));
        }
        let (s, _) = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        assert!(!with_policy(Policy::Panic, || watchers_of_commodities(&s)));
    }
}
