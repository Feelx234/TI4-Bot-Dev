//! The environment of the space combat machine built from the position and the adapter's texts alone (the adapter's `dual_window::settle_run`,
//! `combat::settle_args` and `core_combat::window_env`, which read the string `GameState`). L5: reads the board, the technologies, the
//! abilities and the movement rules.
//!
//! The gates the string computes once per settle (`round_opening_why`, `barrage_why`, `roll_why`, `retreat_why`) name positions of dropped
//! content (diplomacy, Keleres goods, the Last Bastion, the Heart of Ixth) or of lanes that have since ported them; the live environment
//! leaves them off, and the machine's own stops (`Machine::bailed`) are the bail.

use crate::generated::{Ability, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::ability::CombatTexts;
use crate::kernel::Kernel;
use crate::query::reach::{self, SeatReach};
use crate::rules::abilities;
use crate::rules::combat::end_announce::Codes as EndCodes;
use crate::rules::combat::machine::{Env, Fields};
use crate::rules::combat::{is_neutral, retreat_may_enter, Rd};
use crate::state::{CombatStep as Step, CoreState, HitPhase};

/// `ti4_model::state::RULES_MUNITIONS_AFTER_ROLL`: Munitions Reserves is decided after the roll from this version.
const RULES_MUNITIONS_AFTER_ROLL: u32 = 8;

/// The two sides of the fight in progress and its system (`state.active_space_combat`, written by the head of the aftermath).
#[must_use]
pub fn sides(core: &CoreState) -> Option<(System, Seat, Seat)> {
    core.transient().combat.active_space_combat
}

fn owns(core: &CoreState, seat: Seat, tech: Technology) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(tech))
}

/// Where a fleet of `seat` may retreat to from `system` as far as the movement rules go (hyperlanes left out), each with whether the rules let it
/// enter (`core_combat::retreat_neighbors`).
fn neighbors(core: &CoreState, k: &Kernel, seat: Seat, system: System) -> Vec<(System, bool)> {
    let Some(reach) = SeatReach::new(core, k, seat, false) else { return Vec::new() };
    let rules = reach.rules_for(core, k, system);
    let mut found = Vec::new();
    for n in rules.adjacency.of(system).iter().map(crate::ids::system_at) {
        if rules.adjacency.is_hyperlane(n) {
            continue;
        }
        found.push((n, retreat_may_enter(core, n, &|dest| reach.rules_for(core, k, dest))));
    }
    found
}

/// Whether the system holds a wormhole now (`hooks_movement::wormholes_at`): printed, token, and the modules'.
fn wormhole(core: &CoreState, k: &Kernel, system: System) -> bool {
    core.galaxy().is_some_and(|g| g.wormhole_kinds(system) != 0) || k.hooks.movement.extra_wormhole_set(core).iter().any(|(s, _)| *s == system)
}

/// The environment of the fight in `core`, with the adapter's `texts`. `None`: no fight is open, or a side is not a seat.
#[must_use]
pub fn env_of(core: &CoreState, k: &Kernel, t: &CombatTexts, fields: &Fields, occurrence: u64) -> Option<Env> {
    let (system, attacker, defender) = sides(core)?;
    let rd = Rd::new(core, k).with_blanked(reach::blanked(core));
    let both = [attacker, defender];
    let has = |seat: Seat, a: Ability| abilities::has(core, seat, a);
    let begin_occurrence = occurrence == 0
        && (matches!(fields.step, Step::Opening { round: 1 }) || (fields.phase == HitPhase::CombatStart && matches!(fields.step, Step::Queue { .. })))
        && !crate::rules::combat::end::finished(rd, system, attacker, defender);
    let barred = k.hooks.direct.mahact_units_ship_movement_barred(core);
    // The generated enums are in the text order of their ids: the alphabetical order of a roll's unit types is the integer order.
    let mut alpha: Vec<Unit> = t.unit_codes.iter().map(|(u, _)| *u).collect();
    alpha.sort_unstable();
    Some(Env {
        attacker,
        defender,
        system,
        announce: true,
        // `supply::staging_enabled`: every faction of the agreed tables has a module.
        staging: true,
        occurrence,
        begin_occurrence,
        phases: t.phases,
        opening_unported: false,
        round_seq_listens: true,
        barrage_unported: false,
        barrage_cause: t.barrage_cause,
        barrage_kind: t.barrage_kind,
        barrage_extra: t.barrage_extra,
        unit_codes: t.unit_codes.clone(),
        shown_codes: t.shown_codes.clone(),
        barrage_extra_shown: t.barrage_extra_shown,
        jolnar: both.map(|s| core.players().by_seat(s).is_some_and(|p| p.leaders.status.get(&crate::generated::Leader::AgnlanOln) == Some(&crate::state::LeaderStatus::Unlocked))),
        roll_unported: false,
        munitions_after: both.map(|s| core.meta().rules_at_least(RULES_MUNITIONS_AFTER_ROLL) && has(s, Ability::MunitionsReserves)),
        munitions_start: both.map(|s| !core.meta().rules_at_least(RULES_MUNITIONS_AFTER_ROLL) && has(s, Ability::MunitionsReserves)),
        roll_cause: t.roll_cause,
        hacan: t.hacan,
        groups: t.groups.clone(),
        alpha,
        retreat_cause: t.retreat_cause,
        retreat_event: t.retreat_event,
        retreat_pool: t.retreat_pool,
        retreat_unported: false,
        neighbors: both.map(|s| neighbors(core, k, s, system)),
        may_retreat: both.map(|s| !(is_neutral(core, s)) && !barred),
        det: both.map(|s| owns(core, s, Technology::DarkEnergyTap)),
        duranium: both.map(|s| owns(core, s, Technology::DuraniumArmor)),
        devotion: core.players().iter().any(|p| p.faction == crate::generated::Faction::TheYinBrotherhood),
        wormhole: wormhole(core, k, system),
        ambush: t.ambush,
        cause_core: t.cause_core,
        cause_devotion: t.cause_devotion,
        raid: t.raid,
        ac: both.iter().any(|s| owns(core, *s, Technology::AssaultCannon)),
        ac_subtype: t.ac_subtype,
        ac_cause: t.ac_cause,
        end_codes: EndCodes { losers: t.end[0], retreated: t.end[1], reasons: [t.end[2], t.end[3], t.end[4], t.end[5]] },
        casualty: t.casualty,
        commander: t.commander,
    })
}
