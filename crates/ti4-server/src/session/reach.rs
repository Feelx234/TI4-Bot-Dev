//! The read-only "who can go here" query (see `protocol::reach`).
//!
//! Like the secondary preview it holds the session lock only to copy the newest published
//! position, then asks the engine (`ti4_engine::reach_query`) on the copy: the worker is never
//! waited on and the game, its RNG, logs and storage are never touched. Only units that stand on
//! the board (public) are read.

use std::sync::{Arc, Mutex};

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::SystemId;

use crate::protocol::PROTOCOL_VERSION;
use crate::protocol::reach::{ReachIntoMsg, ReachRefusal};
use crate::session::worker::SessionShared;

/// Answer one reach query for `system_id`.
#[must_use]
pub fn answer(
    shared: &Mutex<SessionShared>,
    game_id: &str,
    request_id: u64,
    system_id: &str,
) -> ReachIntoMsg {
    let (state, galaxy, version) = {
        let lock = shared.lock().expect("shared lock");
        let live = !(lock.stopped
            || lock.finished
            || lock.error.is_some()
            || (!lock.replay_complete && lock.pending_decision.is_none()));
        (
            live.then(|| lock.latest_state.clone()),
            lock.preview_galaxy.clone(),
            lock.game_version,
        )
    };
    let reply = |origins, refused| ReachIntoMsg {
        protocol_version: PROTOCOL_VERSION,
        game_id: game_id.to_owned(),
        request_id,
        system_id: system_id.to_owned(),
        as_of_version: version,
        origins,
        refused,
    };
    let (Some(state), Some(galaxy)) = (state, galaxy) else {
        return reply(Vec::new(), Some(ReachRefusal::Busy));
    };
    if !galaxy_has(&galaxy, system_id) {
        return reply(Vec::new(), Some(ReachRefusal::UnknownSystem));
    }
    let origins = ti4_engine::reach_query::who_can_reach(
        &state,
        ContentStore::embedded(),
        POK,
        &galaxy,
        &SystemId::new(system_id),
    );
    reply(origins, None)
}

fn galaxy_has(galaxy: &Arc<ti4_content::galaxy::Galaxy>, system_id: &str) -> bool {
    galaxy.system_ids().contains(&system_id)
}
