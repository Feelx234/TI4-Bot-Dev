//! titans: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | point | string function | state |
//! |---|---|---|
//! | Terragenesis | `titans::terragenesis` | a rule of the table ([`TERRAGENESIS`], package M2) |
//! | Hecatoncheires instead of a PDS (offer and perform) | `hecatoncheires_offer`, `hecatoncheires_perform` | blocked on typed `action_cards::placed_unit_id` and `supply::allowed` (the perform is a no-ask hook and ports with the offer) |
//! | Awaken, Ouranos, Ul the Progenitor, Terraform | `timing_abilities`, `component_actions`, `perform_component`, `use_leader` | B7 |

use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::generated::{Faction, Planet, System};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, EventKind, Key};

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}

/// Terragenesis (`ability:*:terragenesis:PLANET_EXPLORED:after`): "After you explore a planet that does not have a sleeper token: You may place or
/// move 1 sleeper token onto that planet."
pub const TERRAGENESIS: Slot = Slot(17001);

/// The sleeper tokens a Titans seat has (`titans::SLEEPER_TOKENS`).
const SLEEPER_TOKENS: usize = 5;

pub(super) fn is_titans(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheTitansOfUl)
}

/// The planets carrying one of `owner`'s sleeper tokens, in the text order of the mark keys (the string's `sleeper_planets`).
fn sleeper_planets(s: &CoreState, owner: Seat) -> Vec<Planet> {
    s.marks()
        .tail
        .iter()
        .filter_map(|(key, value)| {
            let MarkKey::TitansSleeper(planet) = *key else { return None };
            match value {
                MarkVal::Rec(f) if matches!(f.first(), Some(MarkVal::Atom(Atom::Seat(seat))) if *seat == owner) => Some(planet),
                _ => None,
            }
        })
        .collect()
}

/// Every sleeper token on the board: the planet, its owner and the system it was put in, in the text order of the mark keys.
pub(super) fn sleeper_rows(s: &CoreState) -> Vec<(Planet, Seat, System)> {
    s.marks()
        .tail
        .iter()
        .filter_map(|(key, value)| {
            let MarkKey::TitansSleeper(planet) = *key else { return None };
            match value {
                MarkVal::Rec(f) => match f.as_slice() {
                    [MarkVal::Atom(Atom::Seat(seat)), MarkVal::Atom(Atom::System(system))] => Some((planet, *seat, *system)),
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

fn has_sleeper(s: &CoreState, planet: Planet) -> bool {
    s.marks().tail.contains_key(&MarkKey::TitansSleeper(planet))
}

/// What Terragenesis can do for `planet` (`titans::terragenesis_options`): place a token from the supply and/or move one from another planet.
/// Empty when the planet already has a token or the seat has none to spare.
fn options(s: &CoreState, owner: Seat, planet: Planet) -> Vec<Option<Planet>> {
    if has_sleeper(s, planet) {
        return Vec::new();
    }
    let on_board = sleeper_planets(s, owner);
    let mut out = Vec::new();
    if on_board.len() < SLEEPER_TOKENS {
        out.push(None);
    }
    out.extend(on_board.into_iter().map(Some));
    out
}

fn explored(c: &CondCx<'_>) -> Option<(Seat, Planet)> {
    let p = &c.event.payload;
    let planet = p.planet(Key::Planet)?;
    (p.seat(Key::Player) == Some(c.def.owner) && is_titans(c.state, c.def.owner)).then_some((c.def.owner, planet))
}

fn terragenesis_when(c: &CondCx<'_>) -> bool {
    explored(c).is_some_and(|(owner, planet)| planet.data().system.is_some() && !options(c.state, owner, planet).is_empty())
}

fn terragenesis(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let Some(planet) = eff.event.payload.planet(Key::Planet) else { return Ok(()) };
    if eff.event.payload.seat(Key::Player) != Some(owner) || !is_titans(eff.read().0, owner) { // mutants:skip (the condition holds the same guard)
        return Ok(());
    }
    let Some(system) = planet.data().system else { return Ok(()) };
    let found = options(eff.read().0, owner, planet);
    // Every question before the first mutation.
    let chosen = match found.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let opts: Opts = many.iter().map(|from| Opt::Reaction(ROpt::Sleeper { from: *from })).collect();
            let choice = Choice {
                actor: owner,
                ask: Ask::Reaction(RAsk::Terragenesis { planet }),
                ctx: Ctx {
                    origin: Origin::FactionAbility(ti4_engine_core::choice::generic::Content::Other(eff.codes().terragenesis)),
                    subtype: Subtype::Reaction(RSub::Terragenesis),
                    optional: false,
                    target: None,
                    outstanding: ti4_engine_core::containers::SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            };
            let Pick::Opt(Opt::Reaction(ROpt::Sleeper { from })) = eff.ask(&choice) else { return Ok(()) };
            if !found.contains(&from) {
                return Ok(());
            }
            from
        }
    };
    place(eff.state(), owner, system, planet, chosen);
    Ok(())
}

/// Move the token off `from` (when it comes from the board), then put one on `planet` (`remove_sleeper`, `place_sleeper`).
fn place(s: &mut CoreState, owner: Seat, system: System, planet: Planet, from: Option<Planet>) {
    if let Some(from) = from {
        s.marks_mut().tail.edit().remove(&MarkKey::TitansSleeper(from));
    }
    if has_sleeper(s, planet) || sleeper_planets(s, owner).len() >= SLEEPER_TOKENS { // mutants:skip (the options never offer a placement the guard refuses)
        return;
    }
    s.marks_mut().tail.edit().insert(MarkKey::TitansSleeper(planet), MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(owner)), MarkVal::Atom(Atom::System(system))]));
}

/// The abilities of this tier (the table's rows).
pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(TERRAGENESIS, AbilityKind::Ability, EventKind::PlanetExplored, Relation::After, terragenesis).optional().when(terragenesis_when));
}
