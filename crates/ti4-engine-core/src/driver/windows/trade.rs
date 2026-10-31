//! The trade island as a native arm (package M1; `Game::step_trade`, the opening of `Game::apply_choice`).
//!
//! `Arm::Trade` ([`advance_trade`], [`ask_trade`]) is the transaction window of `rules::trade_window::TradeWindow`, HELD on the stack as
//! `Window::Trade`: every call rebuilds the machine from the payload and the position (the reach by the map, the Arbiters, the unlocked
//! commanders and the free-form deal are read from the CORE, `trade_window::live_facts`), runs it to the next question and writes it back.
//! One call applies one answer, as one string `step_trade` does: the typed `TRANSACTION_RESOLVED` and the plain event follow the answer inside
//! the same call, the staged `TRADE_GOODS_GAINED` events of the deal wait for the next reconcile.
//!
//! The OPENER ([`begin`]) is what the action menu's "open a transaction" option calls (the dispatch of lane F): the string's order of
//! `apply_choice` is note the negotiation, spend the pair's one transaction for the turn unless a module exempts it (94.1, spent on
//! OPENING), put the window on the stack and announce `TRANSACTION_OPENED` (the reaction windows of the table open there). The caller does
//! `settle_extreme_duress` first. Both writes happen before the window exists and before the first answer, so an illegal answer later never
//! leaves the pair unrecorded.

use crate::choice::Pick;
use crate::driver::live::{advance, ask, Asking, Env, Live, LiveError, Out};
use crate::ids::Seat;
use crate::rules::trade::{note_negotiation, record_transaction};
use crate::rules::trade_window::TradeWindow;
use crate::state::{CoreState, Trade, Window as OpenWindow};

/// Open the transaction `proposer` starts with `partner` and run it to its first question (the opening is announced there).
///
/// # Errors
/// As [`advance`].
pub fn begin(core: &mut CoreState, env: &Env<'_>, proposer: Seat, partner: Seat) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    note_negotiation(core, proposer);
    if !env.kernel.hooks.cards.transaction_limit_exempt(core, env.kernel, proposer, partner) {
        record_transaction(core, proposer, partner);
    }
    core.pending_mut().open(OpenWindow::Trade(Trade::opened(proposer, partner)));
    let result = advance::<TradeWindow>(core, env, None);
    if result.is_err() {
        *core = before;
    }
    result
}

/// `Arm::Trade`: the answer to the window's question (`None`: run a window that asks nothing yet).
///
/// # Errors
/// As [`advance`].
pub fn advance_trade(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance::<TradeWindow>(core, env, answer)
}

/// The question of `Arm::Trade`.
///
/// # Errors
/// As [`ask`].
pub fn ask_trade(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<TradeWindow>(core, env)
}
