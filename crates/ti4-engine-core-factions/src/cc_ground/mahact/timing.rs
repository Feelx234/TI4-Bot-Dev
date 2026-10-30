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

pub fn register_abilities(t: &mut AbilityTable) {
    for slot in [LEGIONNAIRE, LEGIONNAIRE_II] {
        t.add(
            Spec::new(slot, AbilityKind::Unit, EventKind::GroundForceDestroyed, Relation::After, legionnaire)
                                .when(condition),
        );
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
