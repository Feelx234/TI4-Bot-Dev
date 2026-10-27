//! sardakk: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `unit_roll_modifier` | `sardakk::unit_roll_modifier` (the flagship's +1 to other ships, Tekklar Legion) | Q | seam `hit_shift` |
//! | `ground.ground_rolls_extra_hits` | `sardakk::ground_rolls_extra_hits` (Valkyrie Particle Weave) | Q | `ground.ground_rolls_extra_hits` |
//! | `ground.commit_candidates` | `sardakk::commit_candidates` (G'hom Sek'kus) | Q | `ground.commit_candidates` (the plan's P2 `commit_blocked` is the opposite polarity) |
//! | `commander_unlocked` | `sardakk::commander_unlocked` | Q | `general.commander_unlocked` (a `first`) |
//! | `skip_to_commit`, `timing_abilities`, leaders | | W | `offer.rs` |

use ti4_engine_core::generated::{Faction, Leader, Planet, System, Technology, Unit};
use ti4_engine_core::hooks::ground::CandidateList;
use ti4_engine_core::hooks::{CombatCtx, CombatUnit, CommitCandidate, CommitOrigin, HookTable};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::{board, fleet};
use ti4_engine_core::state::{CoreState, Piece};

use crate::cd_registry::has_commander_ability;
use crate::common::{flagship_has_text, has_tech_text, mark, rec};

const SOURCE: Source = Source::Faction(Faction::SardakkNorr);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, roll_shift);
    t.ground.ground_rolls_extra_hits.push(ground_rolls_extra_hits);
    t.ground.commit_candidates.push(commit_candidates);
    t.general.commander_unlocked.push(commander_unlocked);
}

/// The C'morran N'orr: other ships of its owner in its system roll +1. Tekklar Legion: the holder's units +1 and the
/// N'orr player's -1, in the one ground combat it was played into.
pub fn unit_roll_modifier(s: &CoreState, _: &Kernel, unit: CombatUnit) -> i64 {
    match unit.ctx {
        CombatCtx::Space => {
            let Some(system) = unit.system else { return 0 };
            if flagship_has_text(s, unit.owner, unit.unit, Unit::CmorranNorr) {
                return 0; // "your other ship's combat rolls"
            }
            let here = s
                .board()
                .sys(system)
                .units
                .as_slice()
                .iter()
                .any(|p| p.owner() == unit.owner && p.kind().data().ship && flagship_has_text(s, p.owner(), p.kind(), Unit::CmorranNorr));
            i64::from(here)
        }
        CombatCtx::Ground => {
            let (Some(system), Some(planet)) = (unit.system, unit.planet) else { return 0 };
            let Some(fields) = mark(s, MarkKey::SardakkTekklarCombat()).and_then(rec) else { return 0 };
            let [MarkVal::Atom(Atom::System(m_system)), MarkVal::Atom(Atom::Planet(m_planet)), MarkVal::Atom(Atom::Seat(holder)), norr] = fields else {
                return 0;
            };
            if *m_system != system || *m_planet != planet {
                return 0;
            }
            let mut shift = 0;
            if unit.owner == *holder {
                shift += 1;
            }
            if matches!(norr, MarkVal::Atom(Atom::Seat(n)) if *n == unit.owner) {
                shift -= 1;
            }
            shift
        }
    }
}

fn roll_shift(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    if !matches!(site.kind, RollKind::Combat | RollKind::Ground) {
        return Delta::None;
    }
    match unit_roll_modifier(q.s, q.k, site.unit) {
        0 => Delta::None,
        n => Delta::Add(n),
    }
}

/// `vpw`: one additional hit when the opponent produced any this round.
pub fn ground_rolls_extra_hits(s: &CoreState, _: &Kernel, roller: Seat, _: System, _: Planet, _own_hits: usize, opponent_hits: usize) -> usize {
    usize::from(has_tech_text(s, roller, Technology::ValkyrieParticleWeave) && opponent_hits >= 1)
}

/// G'hom Sek'kus, unlock: control 5 planets in non-home systems.
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    if leader != Leader::GhomSekkus {
        return None;
    }
    let outside_homes = board::controlled_planets(s, who).iter().filter(|(system, _)| !system.data().home_system).count();
    Some(outside_homes >= 5)
}

/// The commander's extra commit sources: one ground force from each planet in the active system and in the adjacent
/// systems without the invader's command token (a supernova cannot be moved out of).
pub fn commit_candidates(s: &CoreState, _: &Kernel, invader: Seat, system: System, already: &[CommitOrigin]) -> CandidateList {
    let mut found = CandidateList::default();
    if !has_commander_ability(s, invader, Leader::GhomSekkus) {
        return found;
    }
    let take_from = |from: System, found: &mut CandidateList| {
        for planet in board::planets_of(s, from) {
            if already.iter().any(|(_, origin)| *origin == planet) {
                continue; // "up to 1 ground force from each planet"
            }
            let mut seen: Vec<Piece> = Vec::new();
            for unit in board::planet_units(s, planet).iter().filter(|p| p.owner() == invader) {
                if fleet::is_ground_force(unit.kind()) && !seen.contains(unit) {
                    seen.push(*unit);
                    found.push(CommitCandidate { system: from, planet, unit: *unit });
                }
            }
        }
    };
    take_from(system, &mut found);
    if let Some(adjacent) = board::neighbours(s, system) {
        for index in adjacent.iter() {
            let Some(next) = System::from_index(index) else { continue };
            if !(board::has_token(s, next, invader) || next.data().anomalies.supernova) {
                take_from(next, &mut found);
            }
        }
    }
    found
}
