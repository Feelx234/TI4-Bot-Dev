//! The action menu and the closing menu of a turn, assembled by the core (package KM1/KM3, lane F): every source of
//! `driver::menu::ORDER` listed from the position (`driver::{menu_sources, menu_cards, menu_faction}`), the failed component actions
//! filtered, the closing menu from the entries' free and pass flags. Where a source cannot be listed for the position the whole menu is
//! the counted gap [`MenuGap`] (never a menu with a source silently missing).
//!
//! The string functions this mirrors: `Game::{turn_options, closing_options, is_free_option}`.

use crate::choice::turn::Opt;
use crate::choice::Choice;
use crate::driver::menu::{self, Entry, Source};
use crate::driver::{menu_cards, menu_faction, menu_sources};
use crate::ids::Seat;
use crate::kernel::turn_codes::MenuGap;
use crate::kernel::Kernel;
use crate::query::tech::UnitRoster;
use crate::query::{reach, transit};
use crate::state::CoreState;

/// `Game::can_take_tactical`: a tactic token and a system to activate.
fn can_take_tactical(core: &CoreState, k: &Kernel, who: Seat) -> bool {
    core.loaded.has(crate::state::Slice::Map) && core.players().by_seat(who).is_some_and(|p| p.econ.tactic_tokens > 0) && transit::activatable(core, k, who, false).is_some_and(|list| !list.is_empty())
}

/// The options of every source in menu order, before the failed-action filter.
///
/// # Errors
/// [`MenuGap`] when a source cannot be listed for the position.
pub fn sources(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> Result<Vec<(Source, Vec<Opt>)>, MenuGap> {
    let mut parts = vec![(Source::Strategic, menu_sources::strategic(core, who))];
    if can_take_tactical(core, k, who) {
        parts.push((Source::Tactical, vec![Opt::Tactical]));
    }
    parts.push((Source::Transactions, menu_cards::transactions(core, k, who)));
    parts.push((Source::Relics, menu_cards::relics(core, k, who)));
    parts.push((Source::Exploration, menu_cards::exploration(core, k, who)));
    parts.push((Source::Technology, menu_sources::technology(core, k, roster, who)));
    parts.push((Source::Faction, menu_faction::faction(core, k, roster, who)?));
    parts.push((Source::Leaders, menu_sources::leaders(core, k, who)));
    parts.push((Source::ActionCards, menu_cards::action_cards(core, k, who)?));
    Ok(parts)
}

/// `Game::turn_options`: the sources put together, without the component actions that already failed this turn.
///
/// # Errors
/// As [`sources`].
pub fn options(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> Result<Vec<Opt>, MenuGap> {
    let parts = sources(core, k, roster, who)?;
    let failed: Vec<Opt> = core.pending().failed_keys.as_slice().iter().filter_map(|key| Opt::from_key(*key)).collect();
    Ok(menu::turn_menu(&parts, false, &failed))
}

/// The action menu as the question (`Game::turn_options` with its context).
///
/// # Errors
/// As [`sources`].
pub fn question(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> Result<Choice, MenuGap> {
    let parts = sources(core, k, roster, who)?;
    let failed: Vec<Opt> = core.pending().failed_keys.as_slice().iter().filter_map(|key| Opt::from_key(*key)).collect();
    Ok(menu::action_menu(who, &parts, false, &failed))
}

/// `Game::is_free_option`: an option that leaves the turn's action untouched (a transaction, a leader printed "During the action phase:").
#[must_use]
pub fn is_free(opt: Opt) -> bool {
    matches!(opt, Opt::Open { .. }) || menu_sources::leader_is_free(opt) == Some(true)
}

/// `Game::closing_options`: "end your turn" first, then what takes no action, and with a second action everything but "pass".
///
/// # Errors
/// As [`sources`].
pub fn closing(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat) -> Result<Vec<Opt>, MenuGap> {
    let entries: Vec<Entry<Opt>> = options(core, k, roster, who)?.into_iter().map(|option| Entry { option, free: is_free(option), pass: option == Opt::Pass }).collect();
    let _ = reach::blanked(core);
    let second = menu::second_action(core, who, core.pending().actions_this_turn);
    Ok(menu::closing_menu(Opt::EndTurn, &entries, second, &[]))
}
