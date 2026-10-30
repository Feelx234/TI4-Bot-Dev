//! Package D2-1: a table the typed core cannot hold is refused when the game is created
//! (`ti4_engine::core_sync::guard`), so a refusal in the codecs can only come from a corrupted save.

use ti4_content::ContentStore;
use ti4_engine::core_sync::guard;
use ti4_host::setup::{self, PLAYER_COUNTS, TableSetup};
use ti4_model::id::PlayerId;

fn ids(list: &[&str]) -> Vec<PlayerId> {
    list.iter().map(|s| PlayerId::new(*s)).collect()
}

#[test]
fn the_largest_table_a_host_starts_is_one_the_typed_state_holds() {
    assert!(*PLAYER_COUNTS.end() <= guard::MAX_PLAYERS, "PLAYER_COUNTS {PLAYER_COUNTS:?}");
}

#[test]
fn create_refuses_player_ids_one_of_which_is_a_prefix_of_another() {
    let setup = TableSetup::new(2, 7);
    let drawn = setup::draw(&setup, &[None, None]).expect("the table is drawn");
    let refused = setup::create(ContentStore::embedded(), &setup, &ids(&["p1", "p10"]), &drawn)
        .expect_err("p1 is a prefix of p10");
    assert!(refused.contains("prefix"), "{refused}");
    let ok = setup::create(ContentStore::embedded(), &setup, &ids(&["p1", "p2"]), &drawn);
    assert!(ok.is_ok(), "{:?}", ok.err());
}

#[test]
fn create_refuses_a_faction_the_corpus_does_not_have() {
    let setup = TableSetup::new(2, 7);
    let mut drawn = setup::draw(&setup, &[None, None]).expect("the table is drawn");
    drawn.factions[1] = ti4_model::id::FactionId::new("generic");
    let refused = setup::create(ContentStore::embedded(), &setup, &ids(&["p1", "p2"]), &drawn)
        .expect_err("generic is the scaffolding of the test fixtures");
    assert!(refused.contains("generic"), "{refused}");
}
