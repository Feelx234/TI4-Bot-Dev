//! nomad: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `nomad_agents::agent_readied` (`leaders::ready`, `ready_all`) | O | `direct.nomad_agents_agent_readied`: NOT ported, see below |
//!
//! `agent_readied` removes Artuno's pile mark and gains its trade goods with a staged `TRADE_GOODS_GAINED` whose source
//! is the text `"artuno"`. In the core the source of that event is a `Code`, an index into a per-game text table the
//! adapter owns (`docs/ENGINE_CORE_STATE_FIELDS.md`), and a core observer has no way to name it: porting the mark and the
//! gain without the event would change what the sweep announces. Blocked on X1 closing the gain sources (or a named
//! `Code` for them); then the port is: take `MarkKey::NomadArtuno(who)`, `fx::gain_trade_goods_staged(.., Code(artuno))`.

use ti4_engine_core::hooks::HookTable;

/// Fill the slots of this tier.
pub fn register(_: &mut HookTable) {}
