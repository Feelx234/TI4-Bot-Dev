//! The sources of the action menu the core lists itself (package F2, KM1): the strategic action (or the lone "pass") and the leaders'
//! component actions, read from the position and the hook table alone. [`menu`](super::menu) puts the sources together; until now every
//! source was handed in as the ids the string engine listed.
//!
//! The string functions this mirrors: `strategy::strategic_action_options` (and the "pass" `Game::turn_options` falls back to) and
//! `leaders::component_actions` with `borrowed_component_actions` (Ssruu's copies). `ti4-engine`'s `menu_dual` builds the menu from
//! these lists instead of the string's, and the comparison with the string's own question is the oracle; the lockstep over positions is
//! `ti4-engine-core-bridge::check_menu_sources`.

use crate::choice::turn::Opt;
use crate::generated::{Leader, LeaderType, StrategyCard};
use crate::ids::Seat;
use crate::kernel::Kernel;
use crate::query::{leaders as qleaders, reach};
use crate::rules::leaders::{self, Cx};
use crate::state::{CoreState, LeaderStatus};

use super::kernel_env::KernelEnv;

/// `Player::unused_strategy_cards`: the cards the seat holds that are not exhausted, in the order held.
#[must_use]
pub fn unused_cards(core: &CoreState, who: Seat) -> Vec<StrategyCard> {
    core.players().by_seat(who).map_or_else(Vec::new, |p| p.strat.as_slice().iter().copied().filter(|c| !p.strat_exhausted.contains(*c)).collect())
}

/// The strategic action options, or the lone "pass" when the seat holds no unused card (`strategy::strategic_action_options`, and the
/// fallback of `Game::turn_options`). A lone card keeps the bare id; several name theirs.
#[must_use]
pub fn strategic(core: &CoreState, who: Seat) -> Vec<Opt> {
    let unused = unused_cards(core, who);
    match unused.as_slice() {
        [] => vec![Opt::Pass],
        [card] => vec![Opt::Strategic { card: *card, bare: true }],
        cards => cards.iter().map(|card| Opt::Strategic { card: *card, bare: false }).collect(),
    }
}

/// `leaders::action_leader_delivered`: the action-phase leaders whose effects the engine delivers itself (`DELIVERED_ACTION_LEADERS`).
#[must_use]
pub const fn delivered(leader: Leader) -> bool {
    matches!(
        leader,
        Leader::GgrocutoRinn
            | Leader::CarthOfGoldenSands
            | Leader::Zeu
            | Leader::JaceX4thAirLegion
            | Leader::DarktalonTreilla
            | Leader::RinTheMastersLegacy
            | Leader::TheHelmsman
            | Leader::XxekirGromTe
    )
}

/// The leaders' component actions of the action phase (`leaders::component_actions`): the seat's own readied agents and unlocked heroes
/// printed for the action phase that can resolve, in leader order, then Ssruu's copies of the other seats' agents.
#[must_use]
pub fn leaders(core: &CoreState, kernel: &Kernel, who: Seat) -> Vec<Opt> {
    if core.players().by_seat(who).is_none() {
        return Vec::new();
    }
    let env = KernelEnv::new(core, kernel);
    let cx = Cx { core, scope: reach::scope_of(kernel), blanked: reach::blanked(core), env: &env };
    let mut out: Vec<Opt> = leaders::action_candidates(&cx, who, delivered).as_slice().iter().map(|l| Opt::Leader(*l)).collect();
    out.extend(borrowed(&cx, who).into_iter().map(Opt::Borrowed));
    out
}

/// `leaders::borrowed_component_actions`: each other seat's agent (readied or exhausted) whose printed window is the action phase, for a seat that
/// holds a readied Ssruu, when its effect can resolve for the holder. Z'eu resolves for its owner only.
fn borrowed(cx: &Cx<'_>, who: Seat) -> Vec<Leader> {
    if qleaders::status(cx.core, who, Leader::Ssruu) != Some(LeaderStatus::Readied) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for other in cx.core.players().iter().filter(|p| p.seat != who) {
        for (agent, status) in other.leaders.status.iter() {
            let agent = *agent;
            if matches!(status, LeaderStatus::Readied | LeaderStatus::Exhausted)
                && leaders::kind(agent) == LeaderType::Agent
                && agent != Leader::Zeu
                && leaders::is_action_window(agent)
                && (delivered(agent) || cx.env.leader_action(who, agent).is_some())
                && leaders::can_resolve_action(cx, who, agent)
            {
                out.push(agent);
            }
        }
    }
    out
}

/// `Game::is_free_option` for a leader option: one printed "During the action phase:" leaves the turn's action untouched. A copy through Ssruu
/// follows the agent it copies. `None`: not a leader option.
#[must_use]
pub fn leader_is_free(opt: Opt) -> Option<bool> {
    match opt {
        Opt::Leader(leader) | Opt::Borrowed(leader) => Some(!leaders::uses_the_action(leader)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    fn table(factions: &[Faction]) -> CoreState {
        let mut s = CoreState::all_loaded();
        *s.players_mut() = Players::from_vec(factions.iter().enumerate().map(|(i, f)| Player::new(Seat(u8::try_from(i).unwrap()), *f)).collect());
        s
    }

    #[test]
    fn a_seat_without_an_unused_card_is_offered_the_lone_pass_and_one_card_keeps_the_bare_id() {
        let mut s = table(&[Faction::TheFederationOfSol]);
        assert_eq!(strategic(&s, Seat(0)), vec![Opt::Pass], "no card at all");
        let (a, b) = (StrategyCard::ALL[0], StrategyCard::ALL[1]);
        s.players_mut().edit(0).strat.push(a);
        assert_eq!(strategic(&s, Seat(0)), vec![Opt::Strategic { card: a, bare: true }]);
        s.players_mut().edit(0).strat.push(b);
        assert_eq!(strategic(&s, Seat(0)), vec![Opt::Strategic { card: a, bare: false }, Opt::Strategic { card: b, bare: false }]);
        s.players_mut().edit(0).strat_exhausted.insert(a);
        assert_eq!(strategic(&s, Seat(0)), vec![Opt::Strategic { card: b, bare: true }], "a spent card is not offered; the lone one is bare again");
        s.players_mut().edit(0).strat_exhausted.insert(b);
        assert_eq!(strategic(&s, Seat(0)), vec![Opt::Pass], "every card spent");
        assert_eq!(strategic(&s, Seat(5)), vec![Opt::Pass], "no such seat");
    }

    #[test]
    fn the_delivered_leaders_are_the_strings_eight() {
        let delivered_now: Vec<Leader> = Leader::ALL.iter().copied().filter(|l| delivered(*l)).collect();
        assert_eq!(delivered_now.len(), 8);
        assert!(!delivered(Leader::LetaniOspha));
    }

    #[test]
    fn only_a_leader_printed_during_the_action_phase_is_free() {
        let free = Leader::ALL.iter().copied().find(|l| leaders::is_action_window(*l) && !leaders::uses_the_action(*l)).expect("the corpus has one (Carth of Golden Sands)");
        let acts = Leader::ALL.iter().copied().find(|l| leaders::uses_the_action(*l)).expect("an ACTION: leader");
        assert_eq!(leader_is_free(Opt::Leader(free)), Some(true));
        assert_eq!(leader_is_free(Opt::Borrowed(free)), Some(true), "a copy follows its agent");
        assert_eq!(leader_is_free(Opt::Leader(acts)), Some(false));
        assert_eq!(leader_is_free(Opt::Pass), None);
    }
}
