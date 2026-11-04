//! Thin deciders on a legacy game or table (oracle, shadow and the tests that still drive a string window; never production): the legacy `Table` asks a legacy `Decider`; this one puts the same question,
//! through JSON (one serde shape, `tests/wire_parity.rs`), to a decider of the thin crate (the rule bot, the shadow) over a private thin table,
//! with the thin view of the same position, and answers with the option it picked.

use crate::choice as thin;
use ti4_engine_legacy::choice as legacy;

/// A wire value of one crate read as the same value of the other.
///
/// # Panics
/// Never for the wire types (one serde shape, `tests/wire_parity.rs`).
pub fn convert<A: serde::Serialize, B: serde::de::DeserializeOwned>(a: &A) -> B {
    serde_json::from_value(serde_json::to_value(a).expect("a wire value serialises")).expect("the other crate reads the same wire value")
}

/// The decision records of a legacy table as thin ones.
#[must_use]
pub fn thin_records(records: &[legacy::DecisionRecord]) -> Vec<thin::DecisionRecord> {
    records.iter().map(convert).collect()
}

/// A thin decider seated at a legacy table.
pub struct ThinDecider {
    table: thin::Table,
}

impl ThinDecider {
    #[must_use]
    pub fn new(decider: Box<dyn thin::Decider>) -> Self {
        Self { table: thin::Table::with_default(decider) }
    }
}

fn failed(c: &legacy::Choice, e: &thin::IllegalChoice) -> legacy::IllegalChoice {
    legacy::IllegalChoice::DeciderFailed { player: c.player.clone(), prompt: c.prompt.clone(), reason: e.to_string() }
}

impl legacy::Decider for ThinDecider {
    fn choose(&mut self, choice: &legacy::Choice) -> Result<legacy::ChoiceOption, legacy::IllegalChoice> {
        self.table.begin_step();
        let thin_choice: thin::Choice = convert(choice);
        self.table.ask(&thin_choice).map(|o| convert(&o)).map_err(|e| failed(choice, &e))
    }

    fn choose_seeing(&mut self, choice: &legacy::Choice, seen: &legacy::SeatObservation<'_>) -> Result<legacy::ChoiceOption, legacy::IllegalChoice> {
        self.table.begin_step();
        let thin_choice: thin::Choice = convert(choice);
        let (state, content, sources, galaxy) = seen.observed().parts();
        let observed = thin::Observed::new(state, content, sources, galaxy);
        self.table.ask_seeing(&thin_choice, &observed).map(|o| convert(&o)).map_err(|e| failed(choice, &e))
    }
}
