//! What a planet is worth now, which planets can pay, and command token reinforcements
//! (`production::{planet_value, planet_value_now, attachment_bonus, spendable_planets}`,
//! `GameState::tokens_in_reinforcements`).

use crate::generated::Planet;
use crate::ids::Seat;
use crate::state::CoreState;
use crate::tables::{Scope, source_bit};

use super::board::{self, Controlled};
use super::laws;

/// The two things a planet card can be exhausted for (`production::Spend`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spend {
    Resources,
    Influence,
}

/// Command tokens per faction (`ti4_model::state::TOKENS_PER_FACTION`).
pub const TOKENS_PER_FACTION: i32 = 16;

/// `production::planet_value`: the printed resources or influence, 0 for a planet out of scope.
#[must_use]
pub fn printed_value(scope: Scope, planet: Planet, spend: Spend) -> i64 {
    let data = planet.data();
    if scope.0 & source_bit(data.source) == 0 {
        return 0;
    }
    i64::from(match spend {
        Spend::Resources => data.resources,
        Spend::Influence => data.influence,
    })
}

/// `production::attachment_bonus`: the modifiers of the attachments on the planet.
#[must_use]
pub fn attachment_bonus(core: &CoreState, planet: Planet, spend: Spend) -> i64 {
    core.board()
        .planet(planet)
        .attach
        .as_slice()
        .iter()
        .map(|a| {
            let d = a.data();
            i64::from(match spend {
                Spend::Resources => d.resources,
                Spend::Influence => d.influence,
            }
            .unwrap_or(0))
        })
        .sum()
}

/// `production::planet_value_now` without faction hooks: printed value, law bonuses, attachment
/// modifiers, plus `extra` (the hook terms: the Last Bastion's Helios dock).
#[must_use]
pub fn value_now(core: &CoreState, scope: Scope, blanked: bool, planet: Planet, spend: Spend, extra: i64) -> i64 {
    printed_value(scope, planet, spend)
        + laws::planet_value_bonus(core, blanked, planet, spend)
        + attachment_bonus(core, planet, spend)
        + extra
}

/// `production::spendable_planets` without the Deepwrought oceans: controlled and readied, in the
/// string order.
#[must_use]
pub fn spendable_planets(core: &CoreState, seat: Seat) -> Controlled {
    let mut out = board::controlled_planets(core, seat);
    out.retain(|(_, p)| !board::exhausted(core, *p));
    out
}

/// The sum of [`value_now`] over [`spendable_planets`] (no hook terms, no trade goods).
#[must_use]
pub fn spendable_total(core: &CoreState, scope: Scope, blanked: bool, seat: Seat, spend: Spend) -> i64 {
    spendable_planets(core, seat).as_slice().iter().map(|(_, p)| value_now(core, scope, blanked, *p, spend, 0)).sum()
}

/// `GameState::tokens_in_reinforcements`; `mahact_held` is `tokens_held_by_mahact` (a faction mark).
#[must_use]
pub fn tokens_in_reinforcements(core: &CoreState, seat: Seat, mahact_held: i32) -> i32 {
    let on_sheet = core.players().by_seat(seat).map_or(0, |p| {
        i32::from(p.econ.tactic_tokens) + i32::from(p.econ.fleet_tokens) + i32::from(p.econ.strategic_tokens)
    });
    let on_board = core.board().systems().filter(|(_, c)| c.cmd.contains(seat)).count() as i32;
    (TOKENS_PER_FACTION - on_sheet - on_board - mahact_held).max(0)
}

/// `tokens::can_place_command_token`.
#[must_use]
pub fn can_place_command_token(core: &CoreState, seat: Seat, system: crate::generated::System, mahact_held: i32) -> bool {
    core.players().by_seat(seat).is_some()
        && tokens_in_reinforcements(core, seat, mahact_held) > 0
        && !board::has_token(core, system, seat)
}
