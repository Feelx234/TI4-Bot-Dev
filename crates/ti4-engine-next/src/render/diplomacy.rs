//! The `diplomacy` area is a dropped feature (`core_sync::unsupported`: a table with diplomacy on is refused at creation and at load, the
//! live loop registers no arm for it), so this family renders what it must to be total over the core's questions and nothing more: a
//! question that cannot be asked, with options that cannot be picked. The legacy renderer (`ti4-engine-legacy`, `render/diplomacy.rs`)
//! is the string engine's own and stays there.

use serde_json::Value;
use ti4_engine_core::choice::diplomacy::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::wire::choice::ChoiceOption;

/// The kind of the placeholder option.
pub const UNSUPPORTED_KIND: &str = "diplomacy_unsupported";

/// The placeholder family.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, _: &Ask) -> String {
        "diplomacy is not supported".to_owned()
    }

    fn subtype(_: &Render, _: &Subtype) -> String {
        "diplomacy_unsupported".to_owned()
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        ChoiceOption::labelled(format!("{o:?}"), UNSUPPORTED_KIND, "diplomacy is not supported")
    }

    fn parse(_: &Render, _: &Ask, _: &str) -> Option<Opt> {
        None
    }

    fn details(_: &Render, _: &Ask) -> serde_json::Map<String, Value> {
        serde_json::Map::new()
    }
}

/// The id of a note in a diplomacy option: `{alias}:{faction}`, and a Support for the Throne is `support:{faction}`
/// (`promissory::support`), which sorts among the others by that text. (Sula's give question names notes the same way.)
#[must_use]
pub fn dip_note_id(key: ti4_engine_core::state::NoteKey) -> String {
    if key.note == ti4_engine_core::generated::PromissoryNote::SupportForTheThrone {
        format!("support:{}", crate::generated::IdStr::as_str(key.faction))
    } else {
        super::trade::note_id(key)
    }
}
