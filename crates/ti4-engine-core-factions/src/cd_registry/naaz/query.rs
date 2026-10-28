//! naaz: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `unit_roll_modifier` | `naaz::unit_roll_modifier` (Supercharge) | Q | seam `hit_shift` |
//! | `unit_dice` | `naaz::unit_dice` (Visz El Vir with a mech: +1 die; stays a provider, not a row) | Q | seam `dice` (an add) |
//! | `economy.explore_extra_draw` | `naaz::explore_extra_draw` (Distant Suns) | Q | `economy.explore_extra_draw` |
//! | `economy.cannot_produce` | `naaz::cannot_produce` (Eidolon Maximum) | Q | `economy.cannot_produce` |
//! | `economy.effect_placement_forbidden` | `naaz::effect_placement_forbidden` | Q | `economy.effect_placement_forbidden` |
//! | `economy.captured_unit_return_form` | `naaz::captured_unit_return_form` | Q | `economy.captured_unit_return_form` (a fold) |
//! | `combat.ability_hit_immune` | `naaz::ability_hit_immune` | Q | `combat.ability_hit_immune` |
//! | `general.commander_unlocked` | `naaz::commander_unlocked` (Dart and Tai) | Q | `general.commander_unlocked` |
//! | `economy.explored` (Pre-Fab Arcologies), `ground_combat_round_started`, `space_combat_round_started` | | O | `observe.rs` |
//! | `component_actions`, `perform_component`, leaders, `timing_abilities`, `commander_unlocked` | | W | B7 |

use ti4_engine_core::generated::{Faction, Leader, Planet, Unit, UnitBaseType};
use ti4_engine_core::hooks::{CombatCtx, CombatUnit, HookTable};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{flagship_has_text, is_faction, mark};

const SOURCE: Source = Source::Faction(Faction::TheNaazRokhaAlliance);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, supercharge);
    t.seam.dice.push_add(SOURCE, Layer::Faction, visz_el_vir);
    t.economy.explore_extra_draw.push(explore_extra_draw);
    t.economy.cannot_produce.push(cannot_produce);
    t.economy.effect_placement_forbidden.push(effect_placement_forbidden);
    t.economy.captured_unit_return_form.push(captured_unit_return_form);
    t.combat.ability_hit_immune.push(ability_hit_immune);
    t.general.commander_unlocked.push(commander_unlocked);
}

pub(crate) fn is_naaz(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheNaazRokhaAlliance)
}

/// Whether the unit kind is one of the Naaz mech forms whose string id starts `naaz_` (not the Absol copy).
fn is_naaz_mech_form(kind: Unit) -> bool {
    matches!(kind, Unit::Eidolon | Unit::ZGravEidolonNaazMechSpace | Unit::EidolonMaximum)
}

/// `nekro::is_nekro`.
fn is_nekro(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheNekroVirus)
}

/// Supercharge: +1 to the unit's roll in the combat of the round it was used in. `unit_roll_modifier`.
pub fn unit_roll_modifier(s: &CoreState, _: &Kernel, unit: CombatUnit) -> i64 {
    let word = match unit.ctx {
        CombatCtx::Space => Word::Space,
        CombatCtx::Ground => Word::Ground,
    };
    let stamped = mark(s, MarkKey::NaazSupercharge(unit.owner)).and_then(crate::common::rec).is_some_and(|f| {
        matches!(f, [MarkVal::Atom(Atom::Word(w)), MarkVal::Atom(Atom::Num(n))] if *w == word && *n == s.meta().combat_round_seq)
    });
    i64::from(stamped)
}

fn supercharge(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    if !matches!(site.kind, RollKind::Combat | RollKind::Ground) {
        return Delta::None;
    }
    match unit_roll_modifier(q.s, q.k, site.unit) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Visz El Vir: a Naaz (or Nekro) mech rolls 1 additional die while its owner's Visz El Vir stands in the system.
/// The string `unit_dice` fold, as the amount it adds.
pub fn unit_dice(s: &CoreState, _: &Kernel, unit: CombatUnit, dice: i64) -> i64 {
    let Some(system) = unit.system else { return dice };
    let is_mech = unit.unit.data().base_type == UnitBaseType::Mech && (is_naaz_mech_form(unit.unit) || is_nekro(s, unit.owner));
    let flagship = s
        .board()
        .sys(system)
        .units
        .as_slice()
        .iter()
        .any(|p| p.owner() == unit.owner && flagship_has_text(s, unit.owner, p.kind(), Unit::ViszElVir));
    if is_mech && flagship { dice + 1 } else { dice }
}

fn visz_el_vir(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    match unit_dice(q.s, q.k, site.unit, 0) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// Distant Suns: explore a planet that holds one of the player's mechs: draw 1 additional card.
pub fn explore_extra_draw(s: &CoreState, _: &Kernel, who: Seat, planet: Planet) -> bool {
    is_naaz(s, who) && board::planet_units(s, planet).iter().any(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Mech)
}

/// Whether an Eidolon Maximum of `who` stands anywhere (Absolute Synergy is flipped).
pub fn voltron_on_board(s: &CoreState, who: Seat) -> bool {
    let is = |p: &ti4_engine_core::state::Piece| p.owner() == who && p.kind() == Unit::EidolonMaximum;
    s.board().systems().any(|(_, c)| c.units.as_slice().iter().any(is)) || s.board().planets().any(|(_, c)| c.units.as_slice().iter().any(is))
}

/// "Game effects cannot place or produce your mechs": production.
pub fn cannot_produce(s: &CoreState, _: &Kernel, who: Seat, unit: UnitBaseType, _producer: UnitBaseType) -> bool {
    unit == UnitBaseType::Mech && is_naaz(s, who) && voltron_on_board(s, who)
}

/// The shared placement helper passes the resolved faction unit, so upgrades and forms cannot evade this gate.
pub fn effect_placement_forbidden(s: &CoreState, _: &Kernel, who: Seat, unit: Unit) -> bool {
    is_naaz(s, who) && voltron_on_board(s, who) && unit.data().base_type == UnitBaseType::Mech
}

/// A Maximum flips back when it is removed; captured models therefore return as Eidolons.
pub fn captured_unit_return_form(_: &CoreState, _: &Kernel, _owner: Seat, unit: Unit) -> Unit {
    if unit == Unit::EidolonMaximum { Unit::Eidolon } else { unit }
}

/// The Maximum is immune to ability hits.
pub fn ability_hit_immune(_: &CoreState, _: &Kernel, unit: CombatUnit) -> bool {
    unit.unit == Unit::EidolonMaximum
}

/// Dart and Tai, unlock: mechs in 3 systems.
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::DartAndTai).then(|| {
        crate::common::systems_with_piece(s, |p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Mech).len() >= 3
    })
}
