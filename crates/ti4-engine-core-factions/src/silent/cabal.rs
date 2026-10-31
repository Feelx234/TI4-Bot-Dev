//! cabal: The Stillness of Stars, the agent of the Vuil'raith Cabal (`factions::cabal::stillness_of_stars`). "After another player replenishes commodities:
//! you may exhaust this card to convert their commodities to trade goods and capture 1 unit from their reinforcements that has a cost equal to or lower than
//! their commodity value." Optional; registered for the Cabal seat only. The Cabal player chooses the type when several qualify, before anything changes.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, fx_strategy, Kernel};
use ti4_engine_core::query::{fleet, leaders};
use ti4_engine_core::state::{CoreState, EventKind, EventRec, Key, LeaderStatus};

use crate::cd_registry::yin::timing::placed_form;
use crate::common::scope;

/// `leader:*:cabalagent:COMMODITIES_REPLENISHED:after`.
pub const STILLNESS: Slot = Slot(42_050);

/// The base types the agent looks at, in the string's order.
const BASES: [UnitBaseType; 11] = [
    UnitBaseType::Carrier,
    UnitBaseType::Cruiser,
    UnitBaseType::Destroyer,
    UnitBaseType::Dreadnought,
    UnitBaseType::Fighter,
    UnitBaseType::Flagship,
    UnitBaseType::Infantry,
    UnitBaseType::Mech,
    UnitBaseType::Pds,
    UnitBaseType::Spacedock,
    UnitBaseType::Warsun,
];

fn agent_ready(s: &CoreState, owner: Seat) -> bool {
    leaders::status(s, owner, Leader::TheStillnessOfStars) == Some(LeaderStatus::Readied)
}

/// `stillness_targets`: the forms `victim` places today that have a cost of at most `value` and a model left in the box.
fn targets(s: &CoreState, k: &Kernel, victim: Seat, value: i64) -> Vec<Unit> {
    BASES
        .iter()
        .filter_map(|base| placed_form(s, scope(k), victim, *base))
        .filter(|form| {
            // A structure has no printed cost, so it is not "a unit that has a cost".
            scope(k).unit(*form).and_then(|d| d.cost_halves).is_some_and(|halves| halves > 0 && i64::from(halves) <= value * 2) // mutants:skip (no unit prints a cost of nothing)
        })
        .filter(|form| fleet::remaining_all(s, scope(k), victim, *form) != 0)
        .collect()
}

fn victim_of(event: &EventRec) -> Option<(Seat, i64)> {
    Some((event.payload.seat(Key::Player)?, event.payload.int(Key::Commodities)?))
}

fn stillness_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    victim_of(c.event).is_some_and(|(victim, _)| victim != me) && agent_ready(c.state, me)
}

fn stillness(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((victim, value)) = victim_of(eff.event) else { return Ok(()) };
    if victim == me || !agent_ready(eff.read().0, me) { // mutants:skip (the condition holds the same guard)
        return Ok(());
    }
    let found = {
        let (s, k) = eff.read();
        targets(s, k, victim, value)
    };
    let taken = match found.as_slice() {
        [] => None,
        [only] => Some(*only),
        many => {
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::Stillness)),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Leader(Leader::TheStillnessOfStars)),
                    subtype: Subtype::Reaction(RSub::G(GSub::Stillness)),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: many.iter().map(|unit| Opt::Reaction(ROpt::G(GOpt::Capture { unit: *unit, victim }))).collect::<Opts>(),
            };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Capture { unit, .. }))) => found.iter().find(|u| **u == unit).copied(),
                _ => None,
            }
        }
    };
    let source = eff.codes().g2.cabal_agent;
    let (s, k) = eff.state_and_kernel();
    fx_strategy::exhaust_agent(s, k, me, Leader::TheStillnessOfStars);
    let goods = s.players().by_seat(victim).map_or(0, |p| i32::from(p.econ.commodities));
    if goods > 0 { // mutants:skip (a conversion of nothing stages nothing)
        if let Some(p) = s.players_mut().by_seat_mut(victim) {
            p.econ.commodities = 0;
        }
        fx::gain_trade_goods_staged(s, victim, goods, source);
    }
    if let Some(unit) = taken {
        // `capture_from_reinforcements`: a model must be left in the box.
        if fleet::remaining_all(s, scope(k), victim, unit) != 0 // mutants:skip (`targets` took only forms with a model left)
            && let Some(p) = s.players_mut().by_seat_mut(me)
        {
            p.misc.captured_units.push((victim, unit));
        }
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(STILLNESS, AbilityKind::Leader, EventKind::CommoditiesReplenished, Relation::After, stillness).applies(Applies::Faction(Faction::TheVuilraithCabal)).optional().when(stillness_when));
}
