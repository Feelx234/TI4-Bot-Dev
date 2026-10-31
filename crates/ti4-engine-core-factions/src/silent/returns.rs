//! The units that wait on a card come back (`infantry_ii::returns`, `arborec::warrior_returns`, `mahact_units::legionnaire_returns`): "At the start of your next
//! turn, place each unit that is on this card on a planet you control in your home system." All three are mandatory abilities of `TURN_BEGAN` (After).
//! The generic Infantry II lands every unit on the first such planet; the two faction versions (Letani Warrior II, Crimson Legionnaire II) let the owner choose
//! the planet, once per unit, when the home system has several. A unit with no planet to land on stays on the card.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Landing, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, System, Technology, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::{board, laws, objectives, reach, tech};
use ti4_engine_core::rules::reactions::infantry_ii;
use ti4_engine_core::state::{CoreState, EventKind, Key};

use crate::cd_registry::yin::timing::placed_form_overridden;

/// Infantry II (`technology:*:inf2_return:TURN_BEGAN:after`).
pub const INF2_RETURN: Slot = Slot(40_003);
/// Letani Warrior II (`unit:*:arborec_infantry2_return:TURN_BEGAN:after`).
pub const WARRIOR_RETURN: Slot = Slot(40_004);
/// Crimson Legionnaire II (`unit:*:mahact_infantry2_return:TURN_BEGAN:after`).
pub const LEGIONNAIRE_RETURN: Slot = Slot(40_005);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Card {
    Inf2,
    Warrior,
    Legionnaire,
}

fn card_of(arg: u32) -> Card {
    match u16::try_from(arg).unwrap_or(0) {
        x if x == WARRIOR_RETURN.0 => Card::Warrior,
        x if x == LEGIONNAIRE_RETURN.0 => Card::Legionnaire,
        _ => Card::Inf2,
    }
}

fn key(card: Card, who: Seat) -> Option<MarkKey> {
    match card {
        Card::Inf2 => None,
        Card::Warrior => Some(MarkKey::ArborecLw2(who)),
        Card::Legionnaire => Some(MarkKey::MahactCl2(who)),
    }
}

/// Units on the card.
fn waiting(s: &CoreState, who: Seat, card: Card) -> u32 {
    match key(card, who) {
        None => infantry_ii::units_on_card(s, who),
        Some(k) => match s.marks().tail.get(&k) {
            Some(MarkVal::Atom(Atom::Num(n))) => *n,
            _ => 0,
        },
    }
}

fn set_waiting(s: &mut CoreState, who: Seat, card: Card, count: u32) {
    match key(card, who) {
        None => infantry_ii::set_on_card(s, who, count),
        Some(k) if count == 0 => {
            s.marks_mut().tail.edit().remove(&k);
        }
        Some(k) => {
            s.marks_mut().tail.edit().insert(k, MarkVal::Atom(Atom::Num(count)));
        }
    }
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && waiting(c.state, me, card_of(c.def.arg)) > 0
}

/// The planets of the home system the seat controls and may place on, in the order of `placement_spots`.
fn spots(s: &CoreState, who: Seat, home: System) -> Vec<Planet> {
    let blanked = reach::blanked(s);
    board::controlled_planets(s, who).as_slice().iter().filter(|(system, planet)| *system == home && !laws::planet_is_demilitarized(s, blanked, *planet)).map(|(_, planet)| *planet).collect()
}

fn returns(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let card = card_of(eff.def.arg);
    let count = waiting(eff.state(), me, card);
    if card == Card::Inf2 && !tech::owns(eff.state(), me, Technology::InfantryII) {
        // The card is gone and so is what was on it.
        set_waiting(eff.state(), me, card, 0);
        return Ok(());
    }
    let Some(home) = objectives::home_system_of(eff.state(), me) else { return Ok(()) };
    let mut landings: Vec<Planet> = Vec::new();
    for _ in 0..count {
        let planets = spots(eff.state(), me, home);
        let planet = match (card, planets.as_slice()) {
            (_, []) => break,
            (Card::Inf2, [first, ..]) => *first,
            (_, [only]) => *only,
            (card, _) => {
                let landing = if card == Card::Warrior { Landing::Warrior } else { Landing::Legionnaire };
                let source = if card == Card::Warrior { eff.codes().silent.lw2 } else { eff.codes().mahact_infantry2 };
                let choice = Choice {
                    actor: me,
                    ask: Ask::Reaction(RAsk::G(GAsk::Landing(landing))),
                    ctx: Ctx {
                        origin: Origin::FactionAbility(Content::Other(source)),
                        subtype: Subtype::Reaction(RSub::G(GSub::Landing(landing))),
                        optional: false,
                        target: None,
                        outstanding: SmallList::new(),
                        space_battle: false,
                        invasion_seq: None,
                    },
                    opts: planets.iter().map(|p| Opt::Reaction(ROpt::G(GOpt::Planet(*p)))).collect::<Opts>(),
                };
                let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Planet(chosen)))) = eff.ask(&choice) else { break };
                if !planets.contains(&chosen) {
                    break;
                }
                chosen
            }
        };
        landings.push(planet);
    }
    let scope = ti4_engine_core::tables::Scope(u32::from(eff.kernel().sources.0));
    let mut placed = 0;
    for planet in landings {
        let (state, kernel) = eff.state_and_kernel();
        let form = placed_form_overridden(state, kernel, scope, me, UnitBaseType::Infantry);
        placed += u32::try_from(fx::place_counted(state, kernel, scope, me, home, Some(planet), form, 1)).unwrap_or(0);
    }
    set_waiting(eff.state(), me, card, count.saturating_sub(placed));
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(INF2_RETURN, AbilityKind::Technology, EventKind::TurnBegan, Relation::After, returns).when(condition));
    t.add(Spec::new(WARRIOR_RETURN, AbilityKind::Unit, EventKind::TurnBegan, Relation::After, returns).applies(Applies::Faction(Faction::TheArborec)).when(condition));
    t.add(Spec::new(LEGIONNAIRE_RETURN, AbilityKind::Unit, EventKind::TurnBegan, Relation::After, returns).when(condition));
}
