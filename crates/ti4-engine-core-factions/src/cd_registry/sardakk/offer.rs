//! sardakk: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | point | string function | status |
//! |---|---|---|
//! | Sh'val: skip to the commit step | `skip_to_commit` (asks, then writes the hero mark) | ported (silent-fix lane 18): `general.skip_to_commit`, read by the tactical window once the movement is over; the question is `Ask::Leaders(SardakkSkip)` |
//! | Exotrireme II, Tekklar Legion, Mech hit, agent, Supremacy, hero finish | `timing_abilities` | `timing.rs` |
//! | the commander unlock and the leader actions | `commander_unlocked` is Q (`query.rs`); `leader_action`, `use_leader` | Q / the leader windows |

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt, Subtype as LSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Leader, System};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{fx_move, Cx};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::fleet;
use ti4_engine_core::state::LeaderStatus;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.skip_to_commit.push(skip_to_commit);
}

/// `sardakk::skip_to_commit`, Sh'val: "After you move ships into the active system: You may skip directly to the 'Commit Ground Forces' step."
/// Asked once movement is over, only for an unlocked hero, after ships moved into the active system, and when a landing is possible (a planet
/// in the system and a ground force in its space area or one the commit step can draw from); the answer writes the hero mark the finish reads.
pub fn skip_to_commit(cx: &mut Cx<'_>, who: Seat, system: System) -> Option<bool> {
    let unlocked = cx.state.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::ShvalHarbinger) == Some(&LeaderStatus::Unlocked));
    if !unlocked || fx_move::ships_moved(cx.state) == 0 {
        return None;
    }
    let scope = ti4_engine_core::tables::Scope(u32::from(cx.kernel.sources.0));
    let has_ground = cx.state.board().sys(system).units.iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some() && fleet::is_ground_force(p.kind()));
    let has_candidates = !cx.kernel.hooks.ground.commit_candidates(cx.state, cx.kernel, who, system, &[]).is_empty();
    let has_planet = ti4_engine_core::query::board::planets_of(cx.state, system).next().is_some();
    if !has_planet || !(has_ground || has_candidates) {
        return None;
    }
    let choice = Choice {
        actor: who,
        ask: Ask::Leaders(LAsk::SardakkSkip),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Leader(Leader::ShvalHarbinger)),
            subtype: Subtype::Leaders(LSub::SardakkSkip),
            optional: true,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: [Opt::Leaders(LOpt::SardakkSkip), Opt::Decline].into_iter().collect(),
    };
    if cx.ask(&choice) != Pick::Opt(Opt::Leaders(LOpt::SardakkSkip)) {
        return Some(false);
    }
    cx.state.marks_mut().tail.edit().insert(MarkKey::SardakkHeroSkip(), MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(who)), MarkVal::Atom(Atom::System(system))]));
    Some(true)
}
