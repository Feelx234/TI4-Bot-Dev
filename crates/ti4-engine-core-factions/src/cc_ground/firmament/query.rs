//! firmament: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `firmament_puppeted` | `firmament::puppeted` | Q | direct (concat) |
//! | `firmament_plots_assail_space` | `firmament_plots::assail_space` | Q | direct (the `hit_shift` bridge reads it) |
//! | `firmament_plots_assail_against` | `firmament_plots::assail_against` | Q | direct |
//! | `firmament_plots_assail_ground` | `firmament_plots::assail_ground` | Q | direct, and a native `hit_shift` provider for ground rolls (no bridge covers them) |
//! | `firmament_space_cannon_silenced` | `firmament::space_cannon_silenced` | Q | direct |
//! | `firmament_viper_open`, `firmament_viper_defender_open` | `firmament::viper_open`, `viper_defender_open` | Q | direct |
//! | `firmament_remaining_cards` | `firmament::remaining_cards` | Q | direct (a count) |
//! | `firmament_plot_options` | `firmament::plot_options` | W | needs `secrets::fulfils` (B5): see `offer.rs` |
//! | `firmament_place_card`, `firmament_score_as_plot` | `place_card`, `score_as_plot` | W | `offer.rs` |
//! | movement slots | `firmament::{move_bonus, unladen_pass}` | Q | `movement.rs` (A2) |
//! | `timing_abilities`, `commander_unlocked`, `leader_action`, `use_leader`, component actions | | W | B7 |
//!
//! `firmament_plots::{secondary_waivers, secondary_waived}` are the Obsidian's (package CE).

use ti4_engine_core::containers::SeatSet;
use ti4_engine_core::generated::{Faction, Planet, System, Unit};
use ti4_engine_core::hooks::{CombatCtx, HookTable};
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::hooks::direct::Seats;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, PlotCard};

use super::movement::puppeted as puppeted_set;
use crate::common::{is_faction, mark, num, player, rec, scope, stamped_for};

const SOURCE: Source = Source::Faction(Faction::TheObsidian);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, assail_ground_shift);
    let d = &mut t.direct;
    d.firmament_puppeted.push(puppeted);
    d.firmament_plots_assail_space.push(assail_space);
    d.firmament_plots_assail_against.push(assail_against);
    d.firmament_plots_assail_ground.push(assail_ground);
    d.firmament_space_cannon_silenced.push(space_cannon_silenced);
    d.firmament_viper_open.push(viper_open);
    d.firmament_viper_defender_open.push(viper_defender_open);
    d.firmament_remaining_cards.push(remaining_cards);
}

/// `firmament::puppeted`: everyone holding a token on one of `owner`'s plots.
fn puppeted(s: &CoreState, owner: Seat) -> Seats {
    puppeted_set(s, owner).iter().collect()
}

/// `firmament_plots::puppets`: the tokens on `owner`'s faceup Assail, if `owner` plays the Obsidian.
fn assail_puppets(s: &CoreState, owner: Seat) -> SeatSet {
    let mut out = SeatSet::EMPTY;
    if !is_faction(s, owner, Faction::TheObsidian) {
        return out;
    }
    if let Some(p) = player(s, owner) {
        for plot in p.cards.plots.as_slice() {
            if plot.faceup && plot.card == Some(PlotCard::Assail) {
                for t in plot.tokens.iter() {
                    out.insert(t);
                }
            }
        }
    }
    out
}

/// Assail on a ground roll: the space rolls ride the legacy bridge (`firmament_plots_assail_space`), the ground ones
/// have no bridge, so the shift is a native provider.
fn assail_ground_shift(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    let u = &site.unit;
    if site.kind != RollKind::Ground || u.ctx != CombatCtx::Ground {
        return Delta::None;
    }
    let (Some(system), Some(planet)) = (u.system, u.planet) else { return Delta::None };
    match assail_ground(q.s, u.owner, system, planet) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// +1 to `owner`'s rolls against any of `targets`, if `owner` has Assail faceup with a token of one of them.
fn assail_against(s: &CoreState, owner: Seat, targets: &[Seat]) -> i64 {
    let puppeted = assail_puppets(s, owner);
    i64::from(targets.iter().any(|t| puppeted.contains(*t)))
}

/// Assail for `who`'s ship rolls in the combat in `system`: against the other side. The sides are the seats with a
/// ship in the system (or a ground force the Alastor sends into the fight).
fn assail_space(s: &CoreState, k: &Kernel, who: Seat, system: Option<System>) -> i64 {
    let Some(system) = system else { return 0 };
    if assail_puppets(s, who).is_empty() {
        return 0;
    }
    let sides = super::super::sides::combatants(s, k, system);
    let others: Seats = sides.iter().filter(|side| *side != who).collect();
    assail_against(s, who, others.as_slice())
}

/// Assail for `who`'s ground rolls on `planet`: against the other players' units there.
fn assail_ground(s: &CoreState, who: Seat, _: System, planet: Planet) -> i64 {
    if assail_puppets(s, who).is_empty() {
        return 0;
    }
    let mut others = SeatSet::EMPTY;
    for u in board::planet_units(s, planet) {
        if u.owner() != who {
            others.insert(u.owner());
        }
    }
    let others: Seats = others.iter().collect();
    assail_against(s, who, others.as_slice())
}

/// The ships that have moved this activation (`firmament:agent:moved`), by unit kind: the last entry of a kind wins.
fn moved_of(s: &CoreState, kind: Unit) -> Option<u32> {
    let f = mark(s, MarkKey::FirmamentAgentMoved()).and_then(rec)?;
    let [seq, MarkVal::List(list)] = f else { return None };
    if num(seq) != Some(s.meta().activation_seq) {
        return None;
    }
    let mut found = None;
    for entry in list {
        if let Some([MarkVal::Atom(Atom::Unit(u)), count]) = rec(entry)
            && *u == kind
        {
            found = num(count);
        }
    }
    found
}

/// SPACE CANNON cannot be used against the ships that moved during an activation Myru Vos covers.
fn space_cannon_silenced(s: &CoreState, k: &Kernel, active: Seat, system: System) -> bool {
    if !stamped_for(s, MarkKey::FirmamentAgent(), active) {
        return false;
    }
    let scope = scope(k);
    let mut here: Vec<(Unit, u32)> = Vec::new();
    for u in board::space_units(s, system) {
        if u.owner() == active && scope.unit(u.kind()).is_some_and(|d| d.ship) {
            match here.iter_mut().find(|(kind, _)| *kind == u.kind()) {
                Some((_, n)) => *n += 1,
                None => here.push((u.kind(), 1)),
            }
        }
    }
    here.iter().all(|(kind, count)| moved_of(s, *kind).is_some_and(|moved| moved >= *count))
}

/// Whether `who` has a Viper EX-23 on `planet`.
fn has_viper_on(s: &CoreState, who: Seat, planet: Planet) -> bool {
    board::planet_units(s, planet).iter().any(|u| u.owner() == who && u.kind() == Unit::ViperEX23)
}

/// Whether the Viper's choice is open for `invader`, whose ground forces (a Viper among them) have been committed to
/// `planet`: another player controls it and the invader is not already coexisting there.
fn viper_open(s: &CoreState, invader: Seat, system: System, planet: Planet) -> bool {
    has_viper_on(s, invader, planet)
        && !board::is_coexisting(s, system, planet, invader)
        && board::control(s, planet).is_some_and(|holder| holder != invader)
}

/// Whether the Viper's choice is open for `defender`, whose Viper stands on `planet` when `invader` commits there.
fn viper_defender_open(s: &CoreState, defender: Seat, invader: Seat, system: System, planet: Planet) -> bool {
    defender != invader && has_viper_on(s, defender, planet) && !board::is_coexisting(s, system, planet, defender)
}

/// The plot cards no player has in play, by card (`firmament::remaining_cards`): five cards exist; a card of unknown
/// identity still takes one place, so with five cards in play nothing remains.
pub(super) fn remaining(s: &CoreState) -> [bool; 5] {
    let mut known = [false; 5];
    let mut in_play = 0usize;
    for p in s.players().iter() {
        for plot in p.cards.plots.as_slice() {
            in_play += 1;
            if let Some(c) = plot.card {
                known[c as usize] = true;
            }
        }
    }
    if in_play >= 5 {
        return [false; 5];
    }
    known.map(|k| !k)
}

/// `firmament::remaining_cards().len()`.
fn remaining_cards(s: &CoreState) -> u32 {
    remaining(s).iter().filter(|r| **r).count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cc_ground::testkit::table;
    use ti4_engine_core::state::{Piece, Plot};

    const OBS: Faction = Faction::TheObsidian;
    const FIRM: Faction = Faction::TheFirmament;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn plot(card: PlotCard, token: u8, faceup: bool) -> Plot {
        let mut tokens = SeatSet::EMPTY;
        tokens.insert(Seat(token));
        Plot { card: Some(card), tokens, faceup }
    }

    #[test]
    fn assail_needs_a_faceup_card_with_the_target_token() {
        let (mut s, k) = table(&[OBS, SOL, FIRM]);
        let (system, planet) = (ti4_engine_core::ids::system_at(30), Planet::Abaddon);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        assert_eq!(assail_space(&s, &k, Seat(0), Some(system)), 0, "no card");
        s.players_mut().edit(0).cards.plots.push(plot(PlotCard::Assail, 1, false));
        assert_eq!(assail_space(&s, &k, Seat(0), Some(system)), 0, "facedown");
        s.players_mut().edit(0).cards.plots.as_mut_slice()[0].faceup = true;
        assert_eq!(assail_space(&s, &k, Seat(0), Some(system)), 1);
        assert_eq!(assail_space(&s, &k, Seat(0), None), 0);
        assert_eq!(assail_against(&s, Seat(0), &[Seat(2)]), 0, "no token of theirs");
        assert_eq!(assail_against(&s, Seat(1), &[Seat(0)]), 0, "not the owner");
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::InfantryI, Seat(1)));
        assert_eq!(assail_ground(&s, Seat(0), system, planet), 1);
        assert_eq!(assail_ground(&s, Seat(1), system, planet), 0);
        assert_eq!(puppeted(&s, Seat(0)).as_slice(), &[Seat(1)]);
    }

    #[test]
    fn five_cards_exist_and_remain_until_placed() {
        let (mut s, _) = table(&[FIRM, SOL]);
        assert_eq!(remaining_cards(&s), 5);
        s.players_mut().edit(0).cards.plots.push(plot(PlotCard::Seethe, 1, false));
        s.players_mut().edit(1).cards.plots.push(plot(PlotCard::Assail, 0, false));
        assert_eq!(remaining_cards(&s), 3);
    }

    #[test]
    fn the_viper_is_open_on_a_planet_someone_else_holds() {
        let (mut s, _) = table(&[FIRM, SOL]);
        let (system, planet) = (ti4_engine_core::ids::system_at(30), Planet::Abaddon);
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::ViperEX23, Seat(0)));
        assert!(!viper_open(&s, Seat(0), system, planet), "nobody holds it");
        s.board_mut().planet_mut(planet).control = Some(Seat(1));
        assert!(viper_open(&s, Seat(0), system, planet));
        assert!(!viper_open(&s, Seat(1), system, planet), "no Viper of theirs");
        assert!(viper_defender_open(&s, Seat(0), Seat(1), system, planet));
        assert!(!viper_defender_open(&s, Seat(0), Seat(0), system, planet), "the invader is not the defender");
    }
}
