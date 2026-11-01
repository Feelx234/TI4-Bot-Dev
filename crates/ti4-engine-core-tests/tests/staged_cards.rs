//! Lane G5: the consumer of the card moves an ability staged (`CardsStaged` marks; `kernel::fx_cards`): the discard goes on the pile and is
//! announced, the take is announced without the card, oldest first, once, after the typed event of the game that opened the window.

use ti4_engine_core::generated::ActionCard as C;
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::fx_cards::{announce_staged_cards, drain_staged_cards, has_staged_cards, Staged};
use ti4_engine_core::kernel::stage::{Illegal, Sx, Window};
use ti4_engine_core::kernel::Next;
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{CoreState, Emit, EventKind, Key, Payload, Val};
use ti4_engine_core_tests::prelude::*;

/// A window that stages the moves, then emits a typed event of the game (`emit_mirrored`) or announces directly.
#[derive(Debug, Clone)]
struct Probe {
    direct: bool,
    done: bool,
}

impl Window for Probe {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.done
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.done {
            self.done = true;
            assert!(fx::discard_action_card_staged(sx.state, Seat(0), C::Rally));
            assert!(fx::take_action_card_staged(sx.state, Seat(1), Seat(0), C::Sabotage1));
            assert!(fx::discard_action_card_staged(sx.state, Seat(1), C::Spy));
            if self.direct {
                announce_staged_cards(sx).expect("announces");
            } else {
                sx.emit_mirrored(fx::event(EventKind::TurnBegan, Payload::new().with(Key::Player, Val::Seat(Seat(0))))).expect("emits");
            }
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &ti4_engine_core::kernel::Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}

fn rig(direct: bool) -> Rig<Probe> {
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).hand(0, &[C::Rally, C::Sabotage1]).hand(1, &[C::Spy]);
    Rig::new(sc, Probe { direct, done: false })
}

fn typed(rig: &Rig<Probe>) -> Vec<(EventKind, Payload)> {
    rig.state.ledger().events.iter().filter_map(|e| if let Emit::Typed(r) = e { Some((r.kind, r.payload.clone())) } else { None }).collect()
}

#[test]
fn the_moves_are_announced_oldest_first_after_the_event_of_the_game_and_the_discards_reach_the_pile() {
    covers!("kernel::fx_cards");
    let mut rig = rig(false);
    rig.run();
    assert!(rig.is_done());
    assert!(!has_staged_cards(&rig.state), "every row is consumed");
    let kinds: Vec<EventKind> = typed(&rig).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, [EventKind::TurnBegan, EventKind::ActionCardDiscarded, EventKind::ActionCardTaken, EventKind::ActionCardDiscarded], "the game's event, then the moves in staging order");
    let events = typed(&rig);
    // The first discard: the seat and the card.
    assert_eq!(events[1].1.get(Key::Player), Some(Val::Seat(Seat(0))));
    assert_eq!(events[1].1.get(Key::Card), Some(Val::Action(C::Rally)));
    // The take names the taker and the previous holder, deliberately no card.
    assert_eq!(events[2].1.get(Key::Player), Some(Val::Seat(Seat(1))));
    assert_eq!(events[2].1.get(Key::From), Some(Val::Seat(Seat(0))));
    assert_eq!(events[2].1.get(Key::Card), None, "the card is private to the two seats");
    assert_eq!(rig.state.cards().discarded_action.as_slice(), &[C::Rally, C::Spy], "the pile, in the order of the discards");
    assert_eq!(rig.state.transient().combat.last_action_discarded, Some((Seat(1), C::Spy)), "the last discard is named");
}

#[test]
fn an_announcement_in_the_middle_of_a_window_is_the_same_as_after_an_event_and_nothing_staged_does_nothing() {
    covers!("kernel::fx_cards");
    let mut direct = rig(true);
    direct.run();
    let kinds: Vec<EventKind> = typed(&direct).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, [EventKind::ActionCardDiscarded, EventKind::ActionCardTaken, EventKind::ActionCardDiscarded]);
    assert_eq!(direct.state.cards().discarded_action.as_slice(), &[C::Rally, C::Spy]);
    // A second look finds nothing, so it announces nothing twice.
    let mut empty = rig(false);
    empty.run();
    let before = typed(&empty).len();
    let mut probe = Probe { direct: true, done: true };
    let mut rig2 = Rig::from_parts(empty.state.clone(), ti4_engine_core::kernel::Kernel::plain(), Probe { direct: true, done: true });
    rig2.run();
    let _ = &mut probe;
    assert_eq!(typed(&rig2).len(), before, "nothing staged: nothing announced");
}

#[test]
fn the_rows_decode_in_staging_order_and_a_malformed_row_is_dropped() {
    covers!("kernel::fx_cards");
    let mut s = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).hand(0, &[C::Rally, C::Sabotage1]).hand(1, &[C::Spy]).state();
    assert!(!has_staged_cards(&s));
    assert!(fx::discard_action_card_staged(&mut s, Seat(0), C::Sabotage1));
    assert!(fx::take_action_card_staged(&mut s, Seat(1), Seat(0), C::Rally));
    // A row that is no discard or take: not announced, but not kept either.
    s.marks_mut().tail.edit().insert(MarkKey::CardsStaged(7), ti4_engine_core::keys::MarkVal::Atom(ti4_engine_core::keys::Atom::Num(0)));
    assert!(has_staged_cards(&s));
    assert_eq!(
        drain_staged_cards(&mut s),
        vec![Staged::Discarded { who: Seat(0), card: C::Sabotage1 }, Staged::Taken { taker: Seat(1), from: Seat(0), card: C::Rally }]
    );
    assert!(!has_staged_cards(&s), "the malformed row is gone too");
}

/// Bail: the Data Skimmer (a Thunder's Edge breakthrough) could take a discard of another seat during the action phase: the pile is the string's.
#[test]
fn bail_a_discard_the_data_skimmer_could_take_is_unknown_and_only_then() {
    covers!("kernel::fx_cards");
    use ti4_engine_core::generated::Breakthrough;
    use ti4_engine_core::kernel::ability::Unknown;
    use ti4_engine_core::state::Phase;
    let skim = |phase: Phase, passed: bool, holder: u8| {
        let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).hand(0, &[C::Rally, C::Sabotage1]).hand(1, &[C::Spy]).phase(phase).build();
        s.players_mut().by_seat_mut(Seat(holder)).unwrap().cards.breakthrough = Some(Breakthrough::DataSkimmer);
        s.players_mut().by_seat_mut(Seat(holder)).unwrap().econ.passed = passed;
        let mut rig = Rig::from_parts(s, k, Probe { direct: true, done: false });
        rig.run();
        rig.driver.unknown()
    };
    assert_eq!(skim(Phase::Action, false, 1), Some(Unknown::DataSkimmer), "another seat holds it, not passed, during the action phase");
    assert_eq!(skim(Phase::Action, false, 0), Some(Unknown::DataSkimmer), "the other seat discards too");
    assert_eq!(skim(Phase::Action, true, 1), None, "the holder passed (and the holder's own discard is never taken)");
    assert_eq!(skim(Phase::Status, false, 1), None, "only during the action phase");
}

/// Regression: a footprint that does not load the marks (an agenda or a tactical window) must not trip the check for staged rows after an event.
#[test]
fn a_position_without_the_marks_slice_has_nothing_staged() {
    covers!("kernel::fx_cards");
    let mut s = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).state();
    s.loaded = ti4_engine_core::state::SliceSet::of_without(ti4_engine_core::state::SliceSet::ALL, ti4_engine_core::state::Slice::Marks);
    assert!(!has_staged_cards(&s));
}
