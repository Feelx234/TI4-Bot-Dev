//! `Area::LeadersAbilities` (package B7), the mutation half: the status writes of `leaders.rs`
//! (`ready_all`, `ready`, `exhaust`, `purge`, the write of `check_unlocks`) as `ti4_engine_core::rules::leaders`
//! functions over the Players slice. Every function answers `None` when the position is outside what the core
//! holds (more than 16 owners, a leader id the tables do not know, a value the codecs refuse): the caller
//! keeps the string write. The side effects the core cannot do (Nomad's agent hooks, the staged event) stay
//! in the string callers, after the status write.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::Leader;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::leaders as rules;
    use ti4_engine_core::state::{CoreState, Slice};
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    use crate::core_adapter::typed;
    use crate::generated::IdStr;
    use crate::core_sync::{Footprint, Sources};

    const WRITES: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);

    fn lend<T>(gs: &mut GameState, player: &PlayerId, f: impl FnOnce(&mut CoreState, Seat) -> T) -> Option<T> {
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let who = r.seat(player.as_str())?;
        crate::core_sync::with_core(gs, &mut cx, Sources::default(), WRITES, |core, _| f(core, who)).ok()
    }

    fn ids(list: &[Leader]) -> Vec<LeaderId> {
        list.iter().map(|l| LeaderId::new(l.as_str())).collect()
    }

    /// `leaders::ready_all`.
    pub fn ready_all(gs: &mut GameState, player: &PlayerId) -> Option<Vec<LeaderId>> {
        lend(gs, player, |core, who| ids(rules::ready_all(core, who).as_slice()))
    }

    /// `leaders::ready`.
    pub fn ready(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::ready(core, who, l))
    }

    /// `leaders::exhaust`.
    pub fn exhaust(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::exhaust(core, who, l))
    }

    /// `leaders::purge`.
    pub fn purge(gs: &mut GameState, player: &PlayerId, leader: &LeaderId) -> Option<bool> {
        let l = typed::<Leader>(leader.as_str())?;
        lend(gs, player, |core, who| rules::purge(core, who, l))
    }

    /// The status write of `leaders::check_unlocks`.
    pub fn unlock(gs: &mut GameState, player: &PlayerId, leaders: &[LeaderId]) -> Option<()> {
        let list: Option<Vec<Leader>> = leaders.iter().map(|l| typed::<Leader>(l.as_str())).collect();
        let list = list?;
        lend(gs, player, |core, who| rules::unlock(core, who, &list))
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{LeaderId, PlayerId};
    use ti4_model::state::GameState;

    pub fn ready_all(_: &mut GameState, _: &PlayerId) -> Option<Vec<LeaderId>> {
        None
    }
    pub fn ready(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn exhaust(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn purge(_: &mut GameState, _: &PlayerId, _: &LeaderId) -> Option<bool> {
        None
    }
    pub fn unlock(_: &mut GameState, _: &PlayerId, _: &[LeaderId]) -> Option<()> {
        None
    }
}
