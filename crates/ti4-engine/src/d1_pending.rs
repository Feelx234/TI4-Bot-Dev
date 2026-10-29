//! The D1 dual of the driver's `Pending` (package D1a; docs/ENGINE_AREA_STATUS.md 4.1): after every `Game::step` the typed
//! windows the core holds in `Pending.stack` are compared with the window fields of the string `Game`.
//!
//! The windows of the finished areas that have a typed payload in the core (`StrategySecondary`,
//! `StrategySecondaryAfterTactical`, `LeaderStrategy`, `ExecutiveOrder`, `AgendaQueueAfterEventScoring`) are copied from the
//! string fields when the string opens them (the open is phase flow, which is D1a's next piece). From then on the window is
//! **held in the core**: the follower window is moved on by the core's own outcome of each step
//! (`rules::strategy_secondary::advance`, fed by the `strategy::secondary` window dual) and closed when the core says it
//! is complete; the other kinds have no core transition yet and follow the string. Every step then
//!
//! 1. compares the held window with the string field it stands for (`compare_value`, area `driver`, label
//!    `d1::pending.<kind>`), and
//! 2. checks that the typed copy of every open window turns back into the string field it was made from
//!    (`d1::pending.roundtrip`): the codec is total on the positions the games reach.
//!
//! The kinds with no typed payload (`Scoring`, `Tokens`, `Voting`, the tactical and trade windows, ...) are counted under
//! `d1::pending.untyped [bail: <field>]`. A step whose result is an error, a preview window (a planning fork), an id the typed
//! tables do not know and a window the core step does not cover (a card nobody holds) are counted as bails and re-seed the
//! held copy from the string. Runs only with `driver=dual`; a pure observer (the string result stands).

use ti4_engine_core::containers::{SeatSet, SmallList};
use ti4_engine_core::generated::{Agenda, Leader, StrategyCard};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::rules::strategy_secondary::{self as core_secondary, Outcome, Resolution};
use ti4_engine_core::state::{Pending, Scoring, ScoringTiming, Secondary, SecondaryHow, Window, WindowKind};
use ti4_model::id::{LeaderId, PlayerId, StrategyCardId};

use super::{Game, StepResult};
use crate::dual::{Area, Backend, compare_value, note_bail, note_window};
use crate::generated::IdStr;
use crate::objectives::Book;
use crate::secrets::Timing;
use crate::render::Render;
use crate::strategy::{SecondaryParts, SecondaryResolution, StepOutcome, StrategySecondaryWindow};

const LABEL: &str = "d1::pending";

/// What the driver holds of the windows between steps (a field of `Game`; the fork rule copies it).
#[derive(Debug, Clone, Default)]
pub(super) struct Shadow {
    /// The core's `Pending` as the last checked step left it.
    pending: Pending,
    /// The outcome the core half of this step's follower window gave (set by `Game::secondary_step`, taken by [`check`]).
    pub(super) outcome: Option<StepOutcome>,
    /// The book of the scoring window after the core half of this step's scoring answer (`event`: the event window),
    /// set by `Game::score_step`, taken by [`check`].
    pub(super) scoring: Option<(bool, Book)>,
}

/// Whether the D1 dual is on for this thread.
pub(super) fn active() -> bool {
    cfg!(feature = "typed-core") && crate::dual::backend(Area::Driver) == Backend::Dual
}

fn any_open(g: &Game<'_>) -> bool {
    g.secondary.is_some()
        || g.secondary_after_tactical.is_some()
        || g.leader_strategy.is_some()
        || g.executive_order.is_some()
        || g.agenda_queue_after_event_scoring.is_some()
        || g.scoring.is_some()
        || g.event_scoring.is_some()
}

/// The string fields that hold a window no typed payload exists for, by name.
fn untyped_open(g: &Game<'_>) -> Vec<&'static str> {
    [
        ("leader_followers", g.leader_followers.is_some()),
        ("tokens", g.tokens.is_some()),
        ("voting", g.voting.is_some()),
        ("tactical", g.tactical.is_some()),
        ("aftermath", g.aftermath.is_some()),
        ("trade", g.trade.is_some()),
        ("diplomacy", g.diplomacy.is_some()),
        ("agenda_talks", g.agenda_talks.is_some()),
    ]
    .into_iter()
    .filter_map(|(name, open)| open.then_some(name))
    .collect()
}

// --- string -> typed -------------------------------------------------------------------------------------------------

fn how_of(r: SecondaryResolution) -> SecondaryHow {
    match r {
        SecondaryResolution::Ineligible => SecondaryHow::Ineligible,
        SecondaryResolution::Declined => SecondaryHow::Declined,
        SecondaryResolution::Followed => SecondaryHow::Followed,
    }
}

fn resolution_of(h: SecondaryHow) -> SecondaryResolution {
    match h {
        SecondaryHow::Ineligible => SecondaryResolution::Ineligible,
        SecondaryHow::Declined => SecondaryResolution::Declined,
        SecondaryHow::Followed => SecondaryResolution::Followed,
    }
}

/// The typed copy of a follower window; `Err` for a preview window and an id the tables do not know.
fn secondary_of(w: &StrategySecondaryWindow, r: &Render) -> Result<Secondary, &'static str> {
    let parts = w.parts();
    if parts.preview {
        return Err("a preview window");
    }
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let mut followers = SmallList::new();
    for f in parts.followers {
        followers.push(seat(f)?);
    }
    let mut resolved = SmallList::new();
    for (p, how) in parts.resolutions {
        resolved.push((seat(p)?, how_of(*how)));
    }
    Ok(Secondary {
        primary: seat(parts.primary)?,
        card: StrategyCard::from_id(parts.card.as_str()).ok_or("a strategy card the tables do not know")?,
        followers,
        next: u8::try_from(parts.next).map_err(|_| "more than 255 followers")?,
        resolved,
        exhausts_card: parts.exhausts_card,
    })
}

/// The string window a typed copy stands for (the inverse of [`secondary_of`]).
fn secondary_back(s: &Secondary, r: &Render) -> StrategySecondaryWindow {
    StrategySecondaryWindow::from_parts(
        r.player(s.primary),
        StrategyCardId::new(s.card.as_str()),
        s.followers.as_slice().iter().map(|f| r.player(*f)).collect(),
        usize::from(s.next),
        s.resolved.as_slice().iter().map(|(p, how)| (r.player(*p), resolution_of(*how))).collect(),
        s.exhausts_card,
    )
}

fn timing_of(t: Timing) -> ScoringTiming {
    match t {
        Timing::Status => ScoringTiming::Status,
        Timing::Action => ScoringTiming::Action,
        Timing::Agenda => ScoringTiming::Agenda,
    }
}

fn timing_back(t: ScoringTiming) -> Timing {
    match t {
        ScoringTiming::Status => Timing::Status,
        ScoringTiming::Action => Timing::Action,
        ScoringTiming::Agenda => Timing::Agenda,
    }
}

/// The typed copy of a scoring window's book; `Err` for a seat the table does not know.
fn scoring_of(book: &Book, r: &Render) -> Result<Scoring, &'static str> {
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let set = |ps: &[PlayerId]| -> Result<SeatSet, &'static str> {
        let mut set = SeatSet::EMPTY;
        for p in ps {
            set.insert(seat(p)?);
        }
        Ok(set)
    };
    let mut pending = SmallList::new();
    for p in &book.pending {
        pending.push(seat(p)?);
    }
    Ok(Scoring {
        pending,
        public_used: set(&book.public_used)?,
        secret_used: set(&book.secret_used)?,
        plot_card: match &book.plot_card {
            Some((a, b)) => Some((seat(a)?, seat(b)?)),
            None => None,
        },
        timing: timing_of(book.timing),
        occurrence: book.occurrence.map(|o| o.0),
        one_per_player: book.one_per_player,
    })
}

/// The string book a typed copy stands for (the inverse of [`scoring_of`]).
fn scoring_back(s: &Scoring, r: &Render) -> Book {
    let players = |set: SeatSet| -> Vec<PlayerId> {
        let mut ids: Vec<PlayerId> = (0..16u8).map(Seat).filter(|seat| set.contains(*seat)).map(|seat| r.player(seat)).collect();
        // The string keeps these in the order of its `BTreeSet<PlayerId>`.
        ids.sort();
        ids
    };
    Book {
        pending: s.pending.as_slice().iter().map(|seat| r.player(*seat)).collect(),
        public_used: players(s.public_used),
        secret_used: players(s.secret_used),
        plot_card: s.plot_card.map(|(a, b)| (r.player(a), r.player(b))),
        timing: timing_back(s.timing),
        occurrence: s.occurrence.map(ti4_model::state::FeatOccurrence),
        one_per_player: s.one_per_player,
    }
}

/// The typed windows the string fields hold now, in the order of the `Game` fields.
fn observe(g: &Game<'_>, r: &Render) -> Result<Vec<Window>, &'static str> {
    let seat = |p: &PlayerId| r.seat(p.as_str()).ok_or("a seat the table does not know");
    let mut out = Vec::new();
    if let Some(w) = &g.secondary {
        out.push(Window::StrategySecondary(secondary_of(w, r)?));
    }
    if let Some(w) = &g.secondary_after_tactical {
        out.push(Window::StrategySecondaryAfterTactical(secondary_of(w, r)?));
    }
    if let Some((player, leader, card)) = &g.leader_strategy {
        out.push(Window::LeaderStrategy {
            player: seat(player)?,
            leader: Leader::from_id(leader.as_str()).ok_or("a leader the tables do not know")?,
            card: StrategyCard::from_id(card.as_str()).ok_or("a strategy card the tables do not know")?,
        });
    }
    if let Some((owner, speaker)) = &g.executive_order {
        out.push(Window::ExecutiveOrder { owner: seat(owner)?, speaker: seat(speaker)? });
    }
    if let Some(w) = &g.scoring {
        out.push(Window::Scoring(scoring_of(&w.book(), r)?));
    }
    if let Some(w) = &g.event_scoring {
        out.push(Window::EventScoring(scoring_of(&w.book(), r)?));
    }
    if let Some(queue) = &g.agenda_queue_after_event_scoring {
        let mut agendas = SmallList::new();
        for alias in queue {
            agendas.push(Agenda::from_id(alias).ok_or("an agenda the tables do not know")?);
        }
        out.push(Window::AgendaQueueAfterEventScoring(agendas));
    }
    Ok(out)
}

/// Whether every typed window turns back into the string field it was made from.
fn roundtrip(g: &Game<'_>, windows: &[Window], r: &Render) -> bool {
    let mut ok = true;
    for w in windows {
        ok &= match w {
            Window::StrategySecondary(s) => compare_value(Area::Driver, "d1::pending.roundtrip", &g.secondary.clone(), &Some(secondary_back(s, r))),
            Window::StrategySecondaryAfterTactical(s) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.secondary_after_tactical.clone(), &Some(secondary_back(s, r)))
            }
            Window::LeaderStrategy { player, leader, card } => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.leader_strategy.clone(),
                &Some((r.player(*player), LeaderId::new(leader.as_str()), StrategyCardId::new(card.as_str()))),
            ),
            Window::ExecutiveOrder { owner, speaker } => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.executive_order.clone(),
                &Some((r.player(*owner), r.player(*speaker))),
            ),
            Window::AgendaQueueAfterEventScoring(q) => compare_value(
                Area::Driver,
                "d1::pending.roundtrip",
                &g.agenda_queue_after_event_scoring.clone(),
                &Some(q.as_slice().iter().map(|a| a.as_str().to_owned()).collect::<Vec<_>>()),
            ),
            Window::Scoring(sc) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.scoring.as_ref().map(|w| w.book()), &Some(scoring_back(sc, r)))
            }
            Window::EventScoring(sc) => {
                compare_value(Area::Driver, "d1::pending.roundtrip", &g.event_scoring.as_ref().map(|w| w.book()), &Some(scoring_back(sc, r)))
            }
            Window::Untyped { .. } => true,
        };
    }
    ok
}

// --- the held follower window ---------------------------------------------------------------------------------------

/// The core's outcome of the step, from the string-typed value the follower-window dual compared.
fn core_outcome(out: &StepOutcome, r: &Render) -> Option<Outcome> {
    let mut skipped = SmallList::new();
    for p in &out.skipped {
        skipped.push(r.seat(p.as_str())?);
    }
    let answered = match &out.answered {
        Some((p, how)) => Some((
            r.seat(p.as_str())?,
            match how {
                SecondaryResolution::Declined => Resolution::Declined,
                SecondaryResolution::Followed => Resolution::Followed,
                SecondaryResolution::Ineligible => return None,
            },
        )),
        None => None,
    };
    Some(outcome_of(skipped, answered, out.complete))
}

fn outcome_of(skipped: SmallList<Seat, 16>, answered: Option<(Seat, Resolution)>, complete: bool) -> Outcome {
    Outcome { skipped, answered, complete }
}

/// Move the held follower window on by the core's outcome of this step; `true` when it was moved (and closed when complete).
fn advance_held(pending: &mut Pending, out: &Outcome) -> bool {
    let Some(Window::StrategySecondary(w)) = pending.window_mut(WindowKind::StrategySecondary) else { return false };
    core_secondary::advance(w, out);
    if core_secondary::is_complete(w) {
        pending.close(WindowKind::StrategySecondary);
    }
    true
}

// --- the check ------------------------------------------------------------------------------------------------------

/// Compare the held `Pending` with the string fields after `Game::step`, then re-seed it from them.
pub(super) fn check(g: &mut Game<'_>, result: &StepResult) {
    let outcome = g.d1.outcome.take();
    for field in untyped_open(g) {
        note_bail("d1::pending.untyped", field);
    }
    if !any_open(g) && g.d1.pending.stack.is_empty() {
        return;
    }
    let Ok(cx) = crate::core_sync::Ctx::of(&g.state) else {
        note_bail(LABEL, "more than 16 owners");
        g.d1.pending = Pending::default();
        return;
    };
    let r = Render::new(cx.owners.ids().to_vec());
    let observed = match observe(g, &r) {
        Ok(w) => w,
        Err(why) => {
            note_bail(LABEL, why);
            g.d1.pending = Pending::default();
            return;
        }
    };
    let mut compared = false;
    let mut ok = true;
    // The held follower window, moved on by the core's own outcome of this step.
    let mut held = g.d1.pending.clone();
    let moved = result.error.is_none() && outcome.as_ref().and_then(|o| core_outcome(o, &r)).is_some_and(|o| advance_held(&mut held, &o));
    if moved {
        compared = true;
        let expected = held.window(WindowKind::StrategySecondary);
        let actual = observed.iter().find(|w| w.kind() == WindowKind::StrategySecondary);
        ok &= compare_value(Area::Driver, "d1::pending.secondary", &actual, &expected);
    } else if outcome.is_some() {
        note_bail(LABEL, "the held follower window did not take the step (not open, an error, or an id the table does not know)");
    }
    // The scoring window's book after the core half of this step's answer.
    if let Some((event, book)) = g.d1.scoring.take() {
        let kind = if event { WindowKind::EventScoring } else { WindowKind::Scoring };
        match scoring_of(&book, &r) {
            Ok(typed) if result.error.is_none() => {
                compared = true;
                let expected = Some(if event { Window::EventScoring(typed) } else { Window::Scoring(typed) });
                let actual = observed.iter().find(|w| w.kind() == kind);
                ok &= compare_value(Area::Driver, "d1::pending.scoring", &actual, &expected.as_ref());
            }
            _ => note_bail(LABEL, "the scoring book of the core half could not be compared (an error, or a seat the table does not know)"),
        }
    }
    // The kinds without a core transition follow the string: what is compared is that the typed copy is total.
    if !observed.is_empty() {
        compared = true;
        ok &= roundtrip(g, &observed, &r);
    }
    note_window(LABEL, compared);
    let _ = ok;
    g.d1.pending.stack = observed;
}

#[cfg(test)]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::id::PlayerId;

    use super::*;
    use crate::choice::Table;
    use crate::dual::{AreaMask, report, take_report, with_mask};
    use crate::strategy_secondary_dual_tests::{Followers, dealt, pid};

    const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

    fn both() -> AreaMask {
        AreaMask::STRING.with(Area::Strategy, Backend::Dual).with(Area::Driver, Backend::Dual)
    }

    fn window_of(g: &Game<'_>) -> Option<Secondary> {
        match g.d1.pending.window(WindowKind::StrategySecondary) {
            Some(Window::StrategySecondary(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Play a strategic action of `a` to its end with the driver and the strategy area on Dual; returns what the held window
    /// looked like after each step (`(next, resolved)`, `None` once closed) and the report.
    fn play(name: &str, tokens: &[(&str, i32)], answer: &'static str) -> (Vec<Option<(u8, usize)>>, crate::dual::DualReport, Game<'static>) {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = dealt(&F3, name);
        for (who, n) in tokens {
            state.player_mut(&pid(who)).unwrap().strategic_tokens = *n;
        }
        let mut seen = Vec::new();
        let mut game = None;
        with_mask(both(), || {
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: answer })));
            for _ in 0..40 {
                let step = g.step();
                assert!(step.error.is_none(), "{:?}", step.error);
                seen.push(window_of(&g).map(|s| (s.next, s.resolved.len())));
                if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") {
                    break;
                }
            }
            game = Some(g);
        });
        let r = report();
        let _ = take_report();
        (seen, r, game.expect("played"))
    }

    #[test]
    fn the_follower_window_is_held_in_the_core_across_steps_and_agrees_with_the_string() {
        let (seen, r, g) = play("Politics", &[], "yes");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        // Opened by the strategic action, then one follower at a time (b asked, c asked), closed when none is left.
        assert!(seen.iter().flatten().count() >= 2, "the window stayed open over several steps: {seen:?}");
        assert!(seen.iter().flatten().any(|(next, _)| *next >= 1), "{seen:?}");
        assert_eq!(seen.last(), Some(&None), "closed with the action: {seen:?}");
        assert!(g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE"));
        let w = r.windows.get(LABEL).copied().unwrap_or_default();
        assert!(w.compared >= 2 && w.bailed == 0, "{:?}", r.windows);
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
    }

    #[test]
    fn followers_without_a_token_are_skipped_by_the_core_outcome_and_the_window_closes_in_one_step() {
        let (seen, r, _) = play("Warfare", &[("b", 0), ("c", 0)], "yes");
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        // Two ineligible followers are recorded and the window closes in the very step that skips them.
        assert_eq!(seen.last(), Some(&None), "{seen:?}");
        assert!(r.windows.get(LABEL).is_some_and(|w| w.compared >= 1), "{:?}", r.windows);
    }

    /// The mutation check of the harness half: a held window that does not take the core's skipped followers is a difference.
    #[test]
    fn a_held_window_that_forgets_the_skipped_followers_is_a_difference() {
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let state = dealt(&F3, "Warfare");
        with_mask(both(), || {
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
            for _ in 0..3 {
                if g.secondary.is_some() {
                    break;
                }
                let _ = g.step();
            }
            assert!(g.secondary.is_some(), "the action opened the follower window");
            // Hand the held copy a window that is one follower behind the string's: the next step must find it.
            let mut stale = window_of(&g).expect("held");
            stale.followers = SmallList::new();
            g.d1.pending.open(Window::StrategySecondary(stale));
            let _ = g.step();
        });
        let r = report();
        let _ = take_report();
        assert!(r.differences.iter().any(|d| d.label == "d1::pending.secondary"), "{:?} {:?}", r.differences, r.windows);
    }

    /// The status phase of a real position: the scoring window opens over both seats, seat a scores a public objective, the book
    /// moves on by the core's outcome of the answer and the window closes.
    #[test]
    fn the_scoring_window_is_held_and_its_book_agrees_with_the_core_half_after_every_answer() {
        use ti4_model::id::{ObjectiveId, PlanetId, SystemId};
        let _serial = crate::dual::tests::serial();
        let _ = take_report();
        let mut state = crate::fixtures::seated_game(&[("a", "letnev"), ("b", "sol")], ti4_model::content_types::POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.revealed_objectives = vec![ObjectiveId::new("expand_borders")];
        let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), ti4_model::content_types::POK);
        let planets: Vec<String> = catalogue
            .iter()
            .filter(|(_, planet)| planet.homeworld_of().is_none() && !planet.is_placed_during_play())
            .map(|(id, _)| (*id).to_owned())
            .take(6)
            .collect();
        for planet in planets {
            let system = catalogue.get(planet.as_str()).and_then(ti4_content::galaxy::Planet::system_id).unwrap_or("18");
            state.system_mut(&SystemId::new(system)).set_control(PlanetId::new(planet), pid("a"));
        }
        state.phase = ti4_model::state::Phase::Status;
        let mut seen = Vec::new();
        let mut finished = None;
        with_mask(both().with(Area::Objectives, Backend::Dual), || {
            let (decider, _) = crate::choice::Capturing::new(Box::new(crate::choice::Scripted::new(["expand_borders".to_owned()])));
            let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(decider)));
            for _ in 0..12 {
                let step = g.step();
                assert!(step.error.is_none(), "{:?}", step.error);
                seen.push(match g.d1.pending.window(WindowKind::Scoring) {
                    Some(Window::Scoring(sc)) => Some((sc.pending.len(), sc.public_used.len())),
                    _ => None,
                });
                if g.state.phase != ti4_model::state::Phase::Status {
                    break;
                }
            }
            finished = Some(g.state.player(&pid("a")).map(|p| p.victory_points));
        });
        let r = report();
        let _ = take_report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert_eq!(finished, Some(Some(1)), "a scored the public objective: {seen:?}");
        let held: Vec<(usize, usize)> = seen.iter().flatten().copied().collect();
        assert_eq!(held.first(), Some(&(2, 0)), "opened over both seats: {seen:?}");
        assert!(held.iter().any(|(_, used)| *used == 1), "the book recorded the public score: {seen:?}");
        assert!(held.windows(2).any(|w| w[0].0 > w[1].0), "the pending seats shrink as seats answer: {seen:?}");
        assert!(!r.windows.keys().any(|k| k.starts_with("d1::pending [bail")), "{:?}", r.windows);
    }

    fn checked(g: &mut Game<'static>) -> (crate::dual::DualReport, Pending) {
        let _ = take_report();
        let result = g.result(false, None);
        with_mask(both(), || check(g, &result));
        let r = report();
        let _ = take_report();
        (r, g.d1.pending.clone())
    }

    #[test]
    fn the_other_typed_windows_turn_back_into_their_string_fields() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let card = state.player(&pid("a")).unwrap().strategy_cards[0].clone();
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.executive_order = Some((pid("b"), pid("c")));
        g.leader_strategy = Some((pid("a"), ti4_model::id::LeaderId::new(crate::factions::mahact::AGENT), card.clone()));
        g.agenda_queue_after_event_scoring = Some(vec!["checks".to_owned(), "committee".to_owned()]);
        g.secondary_after_tactical = Some(StrategySecondaryWindow::foreign(pid("a"), card, vec![pid("b"), pid("c")]));
        g.scoring = Some(crate::objectives::ScoringWindow::new(&[pid("c"), pid("a"), pid("b")]));
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let kinds: Vec<WindowKind> = held.stack.iter().map(Window::kind).collect();
        assert_eq!(
            kinds,
            [
                WindowKind::StrategySecondaryAfterTactical,
                WindowKind::LeaderStrategy,
                WindowKind::ExecutiveOrder,
                WindowKind::Scoring,
                WindowKind::AgendaQueueAfterEventScoring
            ],
            "{:?} {:?}",
            held.stack,
            r.windows
        );
        assert_eq!(held.window(WindowKind::ExecutiveOrder), Some(&Window::ExecutiveOrder { owner: Seat(1), speaker: Seat(2) }));
        match held.window(WindowKind::AgendaQueueAfterEventScoring) {
            Some(Window::AgendaQueueAfterEventScoring(q)) => assert_eq!(q.len(), 2),
            other => panic!("{other:?}"),
        }
        // Closing the string fields closes the held copies.
        g.executive_order = None;
        g.agenda_queue_after_event_scoring = None;
        g.leader_strategy = None;
        g.secondary_after_tactical = None;
        g.scoring = None;
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(held.stack.is_empty());
    }

    #[test]
    fn an_agenda_or_seat_the_tables_do_not_know_is_a_counted_bail_not_a_difference() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.agenda_queue_after_event_scoring = Some(vec!["a_homebrew_agenda".to_owned()]);
        let (r, held) = checked(&mut g);
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        assert!(r.windows.keys().any(|k| k == "d1::pending [bail: an agenda the tables do not know]"), "{:?}", r.windows);
        assert!(held.stack.is_empty());
        g.agenda_queue_after_event_scoring = None;
        g.executive_order = Some((PlayerId::new("nobody"), pid("a")));
        let (r, _) = checked(&mut g);
        assert!(r.windows.keys().any(|k| k.contains("a seat the table does not know")), "{:?}", r.windows);
    }

    #[test]
    fn windows_without_a_typed_payload_are_counted_by_field() {
        let _serial = crate::dual::tests::serial();
        let state = dealt(&F3, "Politics");
        let mut g = Game::with_table(state, ContentStore::embedded(), Table::with_default(Box::new(Followers { follow: "yes" })));
        g.agenda_talks = Some(super::super::AgendaTalks { alias: "checks".to_owned(), choices: vec![], queue: vec![], seats: vec![] });
        let (r, _) = checked(&mut g);
        assert!(r.windows.keys().any(|k| k == "d1::pending.untyped [bail: agenda_talks]"), "{:?}", r.windows);
    }
}
