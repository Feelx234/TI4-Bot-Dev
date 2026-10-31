//! nomad: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `nomad_agents::agent_readied` (`leaders::ready`, `ready_all`) | O | `rules::leaders::agent_readied`, called by the sites that ready a leader (status phase 81.6, Temporal Command Suite) |
//!
//! `agent_readied` removes Artuno's pile mark and gains its trade goods with a staged `TRADE_GOODS_GAINED` whose source is the text `"artuno"`
//! (the fifth of `Codes::not_from_supply`). It is a plain function of the core (`rules::leaders::agent_readied`), called by the two sites that ready
//! a leader: the status phase's 81.6 (`driver::windows::status_finish`) and this module's Temporal Command Suite. The hook slot
//! `direct.nomad_agents_agent_readied` stays unused (a hook has no way to name the code).

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
