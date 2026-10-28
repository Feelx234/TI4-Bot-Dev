//! Renderer of the `leaders` area: `impl Family` for `ti4_engine_core::choice::leaders` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Rear Admiral Farran offer (prompt
//! "Rear Admiral Farran: gain 1 trade good for the sustained damage", subtype `letnev_commander_trade_good`,
//! option `gain` of kind `letnev_commander_trade_good`, then the generic decline).

use ti4_engine_core::choice::leaders::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::choice::ChoiceOption;

pub const GAIN_ID: &str = "gain";
pub const KIND: &str = "letnev_commander_trade_good";

/// The renderer of the leader questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::SustainCommander => "Rear Admiral Farran: gain 1 trade good for the sustained damage".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::SustainCommander => KIND.to_owned(),
        }
    }

    fn option(_: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::GainTradeGood => ChoiceOption::labelled(GAIN_ID, KIND, "Gain 1 trade good"),
        }
    }

    fn parse(_: &Render, _: &Ask, id: &str) -> Option<Opt> {
        (id == GAIN_ID).then_some(Opt::GainTradeGood)
    }
}
