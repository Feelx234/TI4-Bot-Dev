//! The trade queries on a live position: the [`Env`](crate::query::trade::Env) the hook table of the kernel and the faction abilities answer
//! ([`KernelEnv`]), the queries' `Cx` of a position ([`with_cx`]) and the two reads the live loop makes of it: the seats the action menu may
//! offer a transaction with ([`open_partners_live`]) and Lie in Wait's neighbours who traded ([`lie_in_wait_traders`]). Package M1.
//!
//! L3: it reads the hook table; the rules above it (`rules::trade`, `action_cards`, the live trade window) share it.

use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::query::promissory as qn;
use crate::query::trade::{neighbours_who_transacted, open_partners, Cx, Env, Limits, SeatList, MAX_NEGOTIATIONS_PER_ACTION, MAX_NEGOTIATIONS_PER_ROUND};
use crate::state::{CoreState, TransientFlags};

/// `faction_abilities::has` for the two abilities the trade reads.
fn has_ability(core: &CoreState, who: Seat, ability: crate::generated::Ability) -> bool {
    core.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&ability))
}

/// The hooks of the A5 table and the abilities of the factions answer what the rules cannot read in the typed state
/// (`transactions::may_transact`'s `Env` on the live loop's side).
pub struct KernelEnv<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
}

impl Env for KernelEnv<'_> {
    /// I.I.H.Q. Modernization is a Keleres card: the faction is dropped.
    // mutants:skip (no seat plays the dropped faction: the answer is false by decision)
    fn has_iihq(&self, _: Seat) -> bool {
        false
    }
    /// `faction_abilities::ignores_neighbours || promissory::reaches_anyone`: Guild Ships, the module hook, a faceup Trade Convoys.
    fn wide(&self, who: Seat) -> bool {
        has_ability(self.core, who, crate::generated::Ability::GuildShips)
            || self.kernel.hooks.general.ignores_neighbours(self.core, self.kernel, who)
            || qn::reaches_anyone(self.core, who)
    }
    /// `hooks_cards::transaction_reach`: Temporal Command Suite's transaction, Deepgloom Executable.
    fn transaction_reach(&self, a: Seat, b: Seat) -> bool {
        self.kernel.hooks.cards.transaction_reach(self.core, self.kernel, a, b)
    }
    /// `hooks_cards::transaction_exempt_from_limit`.
    fn exempt_from_limit(&self, a: Seat, b: Seat) -> bool {
        self.kernel.hooks.cards.transaction_limit_exempt(self.core, self.kernel, a, b)
    }
    /// `faction_abilities::trades_action_cards`: Arbiters, or a module hook.
    fn trades_action_cards(&self, who: Seat) -> bool {
        has_ability(self.core, who, crate::generated::Ability::Arbiters) || self.kernel.hooks.general.trades_action_cards(self.core, self.kernel, who)
    }
    fn black_market(&self) -> bool {
        self.core.transient().misc.flags.has(TransientFlags::BLACK_MARKET)
    }
}

/// Run `f` with the trade queries' [`Cx`] of this position: the map's adjacency, the Mecatol Rex system, the negotiation limits and the hooks
/// of the kernel. `None`: the position has no map (the string answers with no galaxy then).
pub fn with_cx<T>(core: &CoreState, kernel: &Kernel, f: impl FnOnce(&Cx<'_>) -> T) -> Option<T> {
    let adj = core.map().adjacency()?;
    let env = KernelEnv { core, kernel };
    let cx = Cx {
        core,
        adj,
        mecatol: crate::query::objectives::mecatol_in_galaxy(core),
        limits: Limits { per_action: MAX_NEGOTIATIONS_PER_ACTION, per_round: MAX_NEGOTIATIONS_PER_ROUND },
        env: &env,
    };
    Some(f(&cx))
}

/// The seats `seat` may open a transaction with now (`transactions::open_partners`, the menu source of the action menu), in seating order;
/// none without a map.
#[must_use]
pub fn open_partners_live(core: &CoreState, kernel: &Kernel, seat: Seat) -> SeatList {
    with_cx(core, kernel, |cx| open_partners(cx, seat)).unwrap_or_default()
}

/// Lie in Wait: the first two neighbours of `seat` (by the map) who resolved a transaction this round (`transactions::
/// neighbours_who_transacted`); none when no transaction was resolved or the position has no map.
#[must_use]
pub fn lie_in_wait_traders(core: &CoreState, kernel: &Kernel, seat: Seat) -> [Option<Seat>; 2] {
    let mut out = [None; 2];
    if core.trade().this_round.is_empty() {
        return out;
    }
    let seats = with_cx(core, kernel, |cx| neighbours_who_transacted(cx, seat)).unwrap_or_default();
    for (slot, who) in out.iter_mut().zip(seats.as_slice()) {
        *slot = Some(*who);
    }
    out
}
