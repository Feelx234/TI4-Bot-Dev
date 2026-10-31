//! The step loop's precedence (package D1b-7; `Game::step_flow` and `Game::legal_options`): which of the windows and phase
//! jobs the next step serves, as a pure function of the position's facts. The order is load-bearing (an open contact before
//! the aftermath it paused; scoring before tokens before voting; trade after the agenda talks) and is the one thing the
//! windows' machines cannot check: each is right about its own question, none knows it should be asked first.
//!
//! The arms are names, not code: a step that reaches an arm the core does not run is a lent step (the D1 step dual counts it
//! by the arm's name). L5: reads the `Pending` stack and the meta slice only.

use crate::state::{CoreState, Phase, WindowKind};

/// The position facts the precedence reads, taken before the arm is chosen (after the per-step reconcile hooks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub finished: bool,
    /// `Game.blocked`: an error report stops the loop.
    pub blocked: bool,
    /// The kinds of the open windows, one bit per [`WindowKind`] (see [`bit`]).
    pub open: u32,
    pub phase: Phase,
    /// The active seat has had its turn prepared (`prepared_turn_seq == turn_seq`).
    pub turn_prepared: bool,
    pub has_active: bool,
    pub status_resolved: bool,
    pub agenda_resolved: bool,
    pub strategy_announced: bool,
    pub round: u32,
    /// Setup: no starting technology is left to pick.
    pub starting_picks_done: bool,
    /// A seat is in the closing menu of its turn (`Game.turn_closing`).
    pub turn_closing: bool,
}

/// The bit of `kind` in [`Facts::open`].
#[must_use]
pub const fn bit(kind: WindowKind) -> u32 {
    1 << (kind as u32)
}

impl Facts {
    /// The facts of a core position: the windows of the `Pending` stack, the phase and the driver flags. The fields the
    /// core does not hold yet (`blocked`, the prepared turn, `turn_closing`) are the caller's.
    #[must_use]
    pub fn of(core: &CoreState, blocked: bool, turn_prepared: bool, turn_closing: bool, starting_picks_done: bool) -> Self {
        use crate::state::DriverFlags as F;
        let p = core.pending();
        let m = core.meta();
        Self {
            finished: m.finished,
            blocked,
            open: p.stack.iter().fold(0, |acc, w| acc | bit(w.kind())),
            phase: m.phase,
            turn_prepared,
            has_active: m.active.is_some(),
            status_resolved: p.driver.has(F::STATUS_RESOLVED),
            agenda_resolved: p.driver.has(F::AGENDA_RESOLVED),
            strategy_announced: p.driver.has(F::STRATEGY_PHASE_ANNOUNCED),
            round: m.round,
            starting_picks_done,
            turn_closing,
        }
    }

    /// The facts of a core position, all of them (package KF1): the prepared turn, the closing menu and the starting
    /// technology picks left are the position's own (`Pending::prepared_turn_seq`, `Pending::turn_closing`,
    /// `Transient::misc.starting_tech_picks`); only `blocked` (an error report of the wrapper) is the caller's. [`Self::of`] stays
    /// for the dual, which takes those three from the string engine.
    #[must_use]
    pub fn of_core(core: &CoreState, blocked: bool) -> Self {
        let p = core.pending();
        Self::of(core, blocked, p.prepared_turn_seq == Some(core.meta().turn_seq), p.turn_closing.is_some(), core.transient().misc.starting_tech_picks.is_empty())
    }

    fn is_open(&self, kind: WindowKind) -> bool {
        self.open & bit(kind) != 0
    }
}

/// What `Game::step_flow` does with the step, in the order it is tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// The game is over: close what is held (the scoring window, the leader strategy) and settle the game end.
    Finished,
    Blocked,
    LeaderFollowers,
    Secondary,
    EventScoring,
    Scoring,
    Tokens,
    Voting,
    Diplomacy,
    Aftermath,
    AgendaTalks,
    Trade,
    Tactical,
    /// The start of a turn: start-of-turn technologies, Military Support, Spec Ops, the `TURN_BEGAN` window.
    TurnPrelude,
    Status,
    Agenda,
    /// The first strategy phase announces itself.
    StrategyAnnounce,
    /// A question is put to the table (`legal_options` is `Some`): [`ask_arm`] says which.
    Ask,
    /// Nothing is asked: the phase moves on (`step_phase`).
    Phase,
}

/// The arm of the step: `Game::step_flow` after the reconcile hooks. `asks` is whether `legal_options` has a question (the
/// last two arms).
#[must_use]
pub fn step_arm(f: &Facts, asks: bool) -> Arm {
    use WindowKind as K;
    if f.finished {
        return Arm::Finished;
    }
    if f.blocked {
        return Arm::Blocked;
    }
    // One window kind per arm, in the order of the string's `if` chain. The follower window of the strategic action is
    // tried before the scoring windows; a contact before the aftermath it paused.
    const CHAIN: [(WindowKind, Arm); 10] = [
        (K::LeaderFollowers, Arm::LeaderFollowers),
        (K::StrategySecondary, Arm::Secondary),
        (K::EventScoring, Arm::EventScoring),
        (K::Scoring, Arm::Scoring),
        (K::Tokens, Arm::Tokens),
        (K::Voting, Arm::Voting),
        (K::Diplomacy, Arm::Diplomacy),
        (K::Aftermath, Arm::Aftermath),
        (K::AgendaTalks, Arm::AgendaTalks),
        (K::Trade, Arm::Trade),
    ];
    for (kind, arm) in CHAIN {
        if f.is_open(kind) {
            return arm;
        }
    }
    if f.is_open(K::Tactical) {
        return Arm::Tactical;
    }
    if f.phase == Phase::Action && !f.turn_prepared && f.has_active {
        return Arm::TurnPrelude;
    }
    if f.phase == Phase::Status && !f.status_resolved {
        return Arm::Status;
    }
    if f.phase == Phase::Agenda && !f.agenda_resolved {
        return Arm::Agenda;
    }
    if f.phase == Phase::Strategy && f.round == 1 && !f.strategy_announced && f.starting_picks_done {
        return Arm::StrategyAnnounce;
    }
    if asks { Arm::Ask } else { Arm::Phase }
}

/// What a question is about (`Game::legal_options`), in the order it is tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskArm {
    /// The game is over or blocked.
    None,
    LeaderFollowers,
    EventScoring,
    Scoring,
    Tokens,
    Voting,
    Diplomacy,
    Aftermath,
    AgendaTalks,
    Tactical,
    StartingTech,
    /// The strategy draft.
    Draft,
    /// The follower window of a strategic action, asked in the action phase.
    Secondary,
    /// The closing menu of the turn.
    Closing,
    /// An open transaction (only [`live_ask_arm`]: the dual's [`ask_arm`] has no Trade arm, the string asks it through its own step).
    Trade,
    /// The action menu.
    ActionMenu,
    /// The status and agenda phases ask nothing of their own.
    Nothing,
}

/// The question the table is asked: `Game::legal_options`. Trade has no arm here (its window asks through its own step).
#[must_use]
pub fn ask_arm(f: &Facts) -> AskArm {
    use WindowKind as K;
    if f.finished || f.blocked {
        return AskArm::None;
    }
    const CHAIN: [(WindowKind, AskArm); 8] = [
        (K::LeaderFollowers, AskArm::LeaderFollowers),
        (K::EventScoring, AskArm::EventScoring),
        (K::Scoring, AskArm::Scoring),
        (K::Tokens, AskArm::Tokens),
        (K::Voting, AskArm::Voting),
        (K::Diplomacy, AskArm::Diplomacy),
        (K::Aftermath, AskArm::Aftermath),
        (K::AgendaTalks, AskArm::AgendaTalks),
    ];
    for (kind, arm) in CHAIN {
        if f.is_open(kind) {
            return arm;
        }
    }
    if f.is_open(K::Tactical) {
        return AskArm::Tactical;
    }
    if !f.starting_picks_done {
        return AskArm::StartingTech;
    }
    match f.phase {
        Phase::Strategy => AskArm::Draft,
        Phase::Action if f.is_open(K::StrategySecondary) => AskArm::Secondary,
        Phase::Action if f.turn_closing => AskArm::Closing,
        Phase::Action => AskArm::ActionMenu,
        Phase::Status | Phase::Agenda => AskArm::Nothing,
    }
}

/// [`ask_arm`] for the live loop: an open transaction asks its own question. (The dual keeps [`ask_arm`], which compares the
/// string's source name; the two merge when the trade island is native.)
#[must_use]
pub fn live_ask_arm(f: &Facts) -> AskArm {
    if !f.finished && !f.blocked && f.is_open(WindowKind::Trade) && ask_arm(f) == AskArm::ActionMenu {
        return AskArm::Trade;
    }
    ask_arm(f)
}

impl Arm {
    /// Every arm, in declaration order.
    pub const ALL: [Self; 19] = [
        Self::Finished,
        Self::Blocked,
        Self::LeaderFollowers,
        Self::Secondary,
        Self::EventScoring,
        Self::Scoring,
        Self::Tokens,
        Self::Voting,
        Self::Diplomacy,
        Self::Aftermath,
        Self::AgendaTalks,
        Self::Trade,
        Self::Tactical,
        Self::TurnPrelude,
        Self::Status,
        Self::Agenda,
        Self::StrategyAnnounce,
        Self::Ask,
        Self::Phase,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Facts {
        Facts {
            finished: false,
            blocked: false,
            open: 0,
            phase: Phase::Action,
            turn_prepared: true,
            has_active: true,
            status_resolved: false,
            agenda_resolved: false,
            strategy_announced: true,
            round: 2,
            starting_picks_done: true,
            turn_closing: false,
        }
    }

    fn with(kinds: &[WindowKind]) -> Facts {
        Facts { open: kinds.iter().fold(0, |a, k| a | bit(*k)), ..quiet() }
    }

    #[test]
    fn the_arm_list_is_complete_and_in_declaration_order() {
        assert_eq!(Arm::ALL.len(), 19);
        for (i, a) in Arm::ALL.iter().enumerate() {
            assert_eq!(*a as usize, i, "the bit of an arm is its index");
        }
    }

    #[test]
    fn facts_of_core_take_the_prepared_turn_the_closing_menu_and_the_picks_from_the_position() {
        use crate::ids::Seat;
        let mut s = CoreState::all_loaded();
        s.meta.turn_seq = 3;
        let f = Facts::of_core(&s, false);
        assert_eq!((f.turn_prepared, f.turn_closing, f.starting_picks_done), (false, false, true));
        s.pending.prepared_turn_seq = Some(3);
        s.pending.turn_closing = Some(Seat(1));
        s.transient.misc.starting_tech_picks.push(crate::state::StartingTechPick { player: Seat(0), of: 1, remaining: 1, source: crate::state::StartingTechSource::OwnedByOthers });
        let f = Facts::of_core(&s, true);
        assert_eq!((f.turn_prepared, f.turn_closing, f.starting_picks_done, f.blocked), (true, true, false, true));
        assert_eq!(f, Facts::of(&s, true, true, true, false), "the same as the dual's facts with the position's own values");
        s.meta.turn_seq = 4;
        assert!(!Facts::of_core(&s, false).turn_prepared, "a later turn is not the prepared one");
    }

    #[test]
    fn the_facts_of_a_core_position_are_read_from_its_meta_and_pending() {
        use crate::ids::Seat;
        use crate::state::{DriverFlags, Secondary, Window};
        let mut s = CoreState::all_loaded();
        s.meta.round = 1;
        s.meta.phase = Phase::Strategy;
        s.meta.active = Some(Seat(1));
        s.pending.driver.set(DriverFlags::STATUS_RESOLVED, true);
        s.pending.driver.set(DriverFlags::STRATEGY_PHASE_ANNOUNCED, true);
        s.pending.stack.push(Window::LeaderStrategy { player: Seat(0), leader: crate::generated::Leader::ALL[0], card: crate::generated::StrategyCard::ALL[0] });
        s.pending.stack.push(Window::StrategySecondary(Secondary {
            primary: Seat(0),
            card: crate::generated::StrategyCard::ALL[0],
            followers: Default::default(),
            next: 0,
            resolved: Default::default(),
            exhausts_card: false,
        }));
        let f = Facts::of(&s, true, true, false, true);
        assert_eq!(f.open, bit(WindowKind::LeaderStrategy) | bit(WindowKind::StrategySecondary), "two windows, not their overlap");
        assert_eq!((f.phase, f.round, f.has_active), (Phase::Strategy, 1, true));
        assert_eq!((f.status_resolved, f.agenda_resolved, f.strategy_announced), (true, false, true));
        assert_eq!((f.blocked, f.turn_prepared, f.turn_closing, f.starting_picks_done, f.finished), (true, true, false, true, false));
        assert_eq!(Facts::of(&CoreState::all_loaded(), false, false, false, false).open, 0, "no window, no bit");
    }

    #[test]
    fn the_turn_prelude_belongs_to_the_action_phase_only() {
        let f = Facts { turn_prepared: false, ..quiet() };
        assert_eq!(step_arm(&Facts { phase: Phase::Status, ..f }, false), Arm::Status);
        assert_eq!(step_arm(&Facts { phase: Phase::Strategy, round: 2, ..f }, false), Arm::Phase);
        assert_eq!(step_arm(&Facts { phase: Phase::Action, turn_prepared: true, ..f }, false), Arm::Phase);
    }

    #[test]
    fn the_windows_are_tried_in_the_order_of_the_string_chain() {
        use WindowKind as K;
        let order = [
            (K::LeaderFollowers, Arm::LeaderFollowers),
            (K::StrategySecondary, Arm::Secondary),
            (K::EventScoring, Arm::EventScoring),
            (K::Scoring, Arm::Scoring),
            (K::Tokens, Arm::Tokens),
            (K::Voting, Arm::Voting),
            (K::Diplomacy, Arm::Diplomacy),
            (K::Aftermath, Arm::Aftermath),
            (K::AgendaTalks, Arm::AgendaTalks),
            (K::Trade, Arm::Trade),
            (K::Tactical, Arm::Tactical),
        ];
        // With every window open the first of the chain wins; closing it hands the step to the next.
        for i in 0..order.len() {
            let open: Vec<K> = order[i..].iter().map(|(k, _)| *k).collect();
            assert_eq!(step_arm(&with(&open), false), order[i].1, "{open:?}");
        }
    }

    #[test]
    fn an_open_contact_comes_before_the_aftermath_it_paused_and_trade_after_the_agenda_talks() {
        use WindowKind as K;
        assert_eq!(step_arm(&with(&[K::Aftermath, K::Diplomacy]), false), Arm::Diplomacy);
        assert_eq!(step_arm(&with(&[K::Trade, K::AgendaTalks]), false), Arm::AgendaTalks);
        assert_eq!(step_arm(&with(&[K::Tactical, K::Trade]), false), Arm::Trade);
        assert_eq!(step_arm(&with(&[K::Tokens, K::Scoring]), false), Arm::Scoring, "scoring before tokens before voting");
        assert_eq!(step_arm(&with(&[K::Voting, K::Tokens]), false), Arm::Tokens);
    }

    #[test]
    fn a_finished_or_blocked_game_stops_before_any_window() {
        use WindowKind as K;
        let all = with(&[K::Scoring, K::Tactical]);
        assert_eq!(step_arm(&Facts { finished: true, blocked: true, ..all }, true), Arm::Finished);
        assert_eq!(step_arm(&Facts { blocked: true, ..all }, true), Arm::Blocked);
        assert_eq!(ask_arm(&Facts { finished: true, ..all }), AskArm::None);
        assert_eq!(ask_arm(&Facts { blocked: true, ..all }), AskArm::None);
    }

    #[test]
    fn the_phase_jobs_follow_the_windows() {
        let f = quiet();
        assert_eq!(step_arm(&Facts { turn_prepared: false, ..f }, true), Arm::TurnPrelude);
        assert_eq!(step_arm(&Facts { turn_prepared: false, has_active: false, ..f }, true), Arm::Ask, "no active seat: no turn to prepare");
        assert_eq!(step_arm(&Facts { phase: Phase::Status, ..f }, false), Arm::Status);
        assert_eq!(step_arm(&Facts { phase: Phase::Status, status_resolved: true, ..f }, false), Arm::Phase);
        assert_eq!(step_arm(&Facts { phase: Phase::Agenda, ..f }, false), Arm::Agenda);
        assert_eq!(step_arm(&Facts { phase: Phase::Agenda, agenda_resolved: true, ..f }, false), Arm::Phase);
        let first = Facts { phase: Phase::Strategy, round: 1, strategy_announced: false, ..f };
        assert_eq!(step_arm(&first, true), Arm::StrategyAnnounce);
        assert_eq!(step_arm(&Facts { starting_picks_done: false, ..first }, true), Arm::Ask, "setup comes before the announcement");
        assert_eq!(step_arm(&Facts { round: 2, ..first }, true), Arm::Ask);
        assert_eq!(step_arm(&Facts { strategy_announced: true, ..first }, true), Arm::Ask);
    }

    #[test]
    fn a_turn_is_prepared_only_after_the_windows_of_the_last_action_are_closed() {
        use WindowKind as K;
        let f = Facts { turn_prepared: false, ..with(&[K::Tactical]) };
        assert_eq!(step_arm(&f, false), Arm::Tactical);
    }

    #[test]
    fn the_question_follows_the_same_chain_without_trade_and_the_follower_window_waits_for_the_action_phase() {
        use WindowKind as K;
        assert_eq!(ask_arm(&with(&[K::Trade])), AskArm::ActionMenu, "trade asks through its own step");
        assert_eq!(live_ask_arm(&with(&[K::Trade])), AskArm::Trade, "the live loop names it");
        assert_eq!(live_ask_arm(&with(&[K::Tactical, K::Trade])), AskArm::Tactical, "only where the menu would have asked");
        assert_eq!(ask_arm(&with(&[K::Tactical, K::Aftermath])), AskArm::Aftermath);
        assert_eq!(ask_arm(&with(&[K::StrategySecondary, K::Tactical])), AskArm::Tactical);
        assert_eq!(ask_arm(&with(&[K::StrategySecondary])), AskArm::Secondary);
        assert_eq!(ask_arm(&Facts { turn_closing: true, ..with(&[K::StrategySecondary]) }), AskArm::Secondary);
        assert_eq!(ask_arm(&Facts { turn_closing: true, ..quiet() }), AskArm::Closing);
        assert_eq!(ask_arm(&Facts { starting_picks_done: false, ..quiet() }), AskArm::StartingTech);
        assert_eq!(ask_arm(&Facts { phase: Phase::Strategy, ..quiet() }), AskArm::Draft);
        assert_eq!(ask_arm(&Facts { phase: Phase::Status, ..quiet() }), AskArm::Nothing);
        assert_eq!(ask_arm(&with(&[K::Scoring, K::StrategySecondary])), AskArm::Scoring);
    }
}
