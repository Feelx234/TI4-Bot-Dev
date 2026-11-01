//! Lane G5: `Seen::revealed_cards`, the typed twin of `SeatObservation::revealed_cards`: the hidden cards other seats showed the bound seat
//! (the `CardsReveal` rows and the standing permission of the `may_view_hand` hook), and nothing of anyone else's.

use ti4_engine_core::generated::{ActionCard as C, Faction, PromissoryNote, SecretObjective};
use ti4_engine_core::hooks::RevealKind;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::seen::{Revealed, Seen, Shown};
use ti4_engine_core::state::{CoreState, NoteKey};
use ti4_engine_core_tests::prelude::*;

fn cards(list: &[C]) -> MarkVal {
    MarkVal::List(list.iter().map(|c| MarkVal::Atom(Atom::Card(*c))).collect())
}

fn reveal(s: &mut CoreState, scope: Word, source: Word, viewer: u8, owner: u8, deck: Word, row: MarkVal) {
    s.marks_mut().tail.edit().insert(MarkKey::CardsReveal(scope, source, Seat(viewer), Seat(owner), deck), row);
}

fn table() -> (CoreState, Kernel) {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheNaaluCollective]).hand(1, &[C::Rally, C::Sabotage1, C::Spy]).hand(2, &[C::Rally]).build()
}

fn seen(s: &CoreState, k: &Kernel, me: u8) -> Vec<Revealed> {
    Seen::new(s, k, Seat(me)).revealed_cards()
}

#[test]
fn only_the_viewer_sees_what_was_shown_and_only_while_the_owner_still_holds_it() {
    covers!("seen::hand");
    let (mut s, k) = table();
    assert!(seen(&s, &k, 0).is_empty(), "nothing shown yet");
    reveal(&mut s, Word::Action, Word::Spynet, 0, 1, Word::ActionCards, cards(&[C::Sabotage1, C::Spy]));
    // A second row of another scope merges into the same owner and kind; a repeated card is listed once.
    reveal(&mut s, Word::Choice, Word::Mi, 0, 1, Word::ActionCards, cards(&[C::Spy, C::Rally]));
    assert_eq!(seen(&s, &k, 0), vec![Revealed { owner: Seat(1), shown: Shown::ActionCards(vec![C::Sabotage1, C::Spy, C::Rally]) }]);
    assert!(seen(&s, &k, 1).is_empty(), "not the owner");
    assert!(seen(&s, &k, 2).is_empty(), "not a third seat");
    // A card the owner no longer holds is not reported; a row of nothing held is gone.
    s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.remove(1);
    assert_eq!(seen(&s, &k, 0), vec![Revealed { owner: Seat(1), shown: Shown::ActionCards(vec![C::Spy, C::Rally]) }]);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.hand.clear();
    assert!(seen(&s, &k, 0).is_empty());
}

#[test]
fn secrets_and_notes_are_reported_by_kind_in_the_order_of_the_owner_and_the_kind() {
    covers!("seen::hand");
    let (mut s, k) = table();
    let secret = SecretObjective::ALL[0];
    s.players_mut().by_seat_mut(Seat(2)).unwrap().cards.secrets.push(secret);
    let note = NoteKey { note: PromissoryNote::ALL[0], faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(note, Seat(1));
    reveal(&mut s, Word::Standing, Word::Spynet, 0, 2, Word::SecretObjectives, MarkVal::List(vec![MarkVal::Atom(Atom::Secret(secret))]));
    reveal(&mut s, Word::Standing, Word::Spynet, 0, 1, Word::PromissoryNotes, MarkVal::List(vec![MarkVal::Rec(vec![MarkVal::Atom(Atom::Note(note.note)), MarkVal::Atom(Atom::Faction(note.faction))])]));
    reveal(&mut s, Word::Standing, Word::Spynet, 0, 2, Word::ActionCards, cards(&[C::Rally]));
    assert_eq!(
        seen(&s, &k, 0),
        vec![
            Revealed { owner: Seat(1), shown: Shown::PromissoryNotes(vec![note]) },
            Revealed { owner: Seat(2), shown: Shown::ActionCards(vec![C::Rally]) },
            Revealed { owner: Seat(2), shown: Shown::SecretObjectives(vec![secret]) },
        ]
    );
    // A note turned faceup is public: no longer hidden, no longer reported.
    s.cards_mut().promissory_faceup.edit().insert(note, ());
    assert_eq!(seen(&s, &k, 0).len(), 2);
}

fn may_view_all(_: &CoreState, _: &Kernel, _: Seat, _: Seat, kind: RevealKind) -> bool {
    kind == RevealKind::PromissoryNotes
}

#[test]
fn a_standing_permission_shows_the_notes_in_the_neighbours_hands_and_merges_with_a_recorded_row() {
    covers!("seen::hand");
    let (mut s, mut k) = table();
    k.hooks.cards.may_view_hand.push(may_view_all);
    let (mine, theirs, other) = (
        NoteKey { note: PromissoryNote::ALL[0], faction: Faction::TheArborec },
        NoteKey { note: PromissoryNote::ALL[1], faction: Faction::TheEmbersOfMuaat },
        NoteKey { note: PromissoryNote::ALL[2], faction: Faction::TheEmbersOfMuaat },
    );
    s.cards_mut().promissory.edit().insert(mine, Seat(0));
    s.cards_mut().promissory.edit().insert(theirs, Seat(1));
    s.cards_mut().promissory.edit().insert(other, Seat(1));
    // Seat 0's own notes are not "shown"; seat 1's are, in seating order after nothing recorded.
    let rows = seen(&s, &k, 0);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].owner, Seat(1));
    let Shown::PromissoryNotes(notes) = &rows[0].shown else { panic!("notes") };
    let mut notes = notes.clone();
    notes.sort();
    assert_eq!(notes, vec![theirs, other]);
    // A recorded row of the same owner and kind absorbs the permission (no second row; the recorded id leads).
    reveal(&mut s, Word::Action, Word::Spynet, 0, 1, Word::PromissoryNotes, MarkVal::List(vec![MarkVal::Rec(vec![MarkVal::Atom(Atom::Note(other.note)), MarkVal::Atom(Atom::Faction(other.faction))])]));
    let rows = seen(&s, &k, 0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].shown, Shown::PromissoryNotes(vec![other, theirs]));
    // An action-card row of the same owner does not take the notes' place: the permission still merges into the NOTES row.
    reveal(&mut s, Word::Action, Word::Spynet, 0, 1, Word::ActionCards, cards(&[C::Sabotage1]));
    let rows = seen(&s, &k, 0);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].shown.kind(), RevealKind::ActionCards);
    assert_eq!(rows[1].shown, Shown::PromissoryNotes(vec![other, theirs]));
    // Without the hook nothing is shown of the same hands; and the viewer's own hand is never "shown" to it.
    let (mut plain, k2) = table();
    plain.cards_mut().promissory.edit().insert(theirs, Seat(1));
    assert!(seen(&plain, &k2, 0).is_empty());
    assert!(seen(&s, &k, 1).iter().all(|r| r.owner != Seat(1)), "a seat is not its own audience");
}
