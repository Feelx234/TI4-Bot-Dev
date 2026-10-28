//! The fixtures the string-side faction tests share (`a`/`b`/`c` seated against Sol and Hacan, the corpus,
//! a scripted decider); each faction's own `testkit` re-exports these and adds what is specific to it.

use ti4_content::ContentStore;
use ti4_model::content_types::DEFAULT;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::{Scripted, Table};

pub(crate) fn a() -> PlayerId {
    PlayerId::new("a")
}
pub(crate) fn b() -> PlayerId {
    PlayerId::new("b")
}
pub(crate) fn c() -> PlayerId {
    PlayerId::new("c")
}
pub(crate) fn content() -> &'static ContentStore {
    ContentStore::embedded()
}
/// `a` plays `faction`, `b` Sol, `c` Hacan.
pub(crate) fn trio(faction: &str) -> GameState {
    crate::fixtures::seated_game(&[("a", faction), ("b", "sol"), ("c", "hacan")], DEFAULT)
}
pub(crate) fn scripted(answers: &[&str]) -> Table {
    Table::with_default(Box::new(Scripted::new(answers.iter().copied())))
}
