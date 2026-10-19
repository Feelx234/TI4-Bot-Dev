//! The replay export: a game's history together with everything its table was set up with, so that
//! the file alone replays the game ([`ReplayExport::replay`]).
//!
//! `GET /api/games/{id}/replay` of the server writes it. Format 1 (no `format_version`) held the
//! history, the seed, the seats' ids and the map alias; the factions, the names, the card set, the
//! speaker order and the Thunder's Edge switch were missing, so it could not be replayed without
//! guessing them. Format 2 adds them (see `docs/REPLAY_FORMAT.md`). Format 1 files still load
//! ([`ReplayExport::parse`]); they replay only when the caller supplies the opening state.
//!
//! Integers that a JavaScript `Number` cannot hold exactly (the seed, the ends of the deck
//! segments, the stream positions) are decimal strings in the file ([`crate::wire`]).
//!
//! Nothing here is game state: no checksum, no decision hash and no stored record changes.

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::{FactionId, PlayerId};
use ti4_model::state::GameState;

use crate::map::GalaxyLayout;
use crate::rng_force::RngMarks;
use crate::setup::{Drawn, TableSetup};

/// The `format` of every version of the export.
pub const FORMAT: &str = "ti4-replay";

/// The version this build writes. A file without `format_version` is version 1.
pub const FORMAT_VERSION: u32 = 2;

/// The whole export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExport {
    /// Always [`FORMAT`].
    pub format: String,
    /// The `version` field of format 1 (`1`); format 2 writes [`FORMAT_VERSION`] here as well.
    #[serde(default)]
    pub version: u32,
    /// [`FORMAT_VERSION`]; missing in a format 1 file (read as 1).
    #[serde(default = "one")]
    pub format_version: u32,
    pub game_id: String,
    /// The game's seed, as a decimal string. `None` for a game made without one.
    #[serde(default, with = "crate::wire::opt_u64_text")]
    pub seed: Option<u64>,
    /// The ids of the players in the order of their seats.
    pub player_ids: Vec<PlayerId>,
    /// The alias of the map template (`None`: the seeded random board).
    #[serde(default)]
    pub map_template: Option<String>,
    /// What the table was set up with; `None` when the game was not made from a table (a test or a
    /// development game), or in a format 1 file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<ExportSetup>,
    /// Who sat where, with the faction, the name and who plays the seat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<ExportSeat>,
    /// The tile layout of the map the game is played on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_layout: Option<GalaxyLayout>,
    /// The build that wrote the file, and the rules the game started with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<ExportBuild>,
    /// The journal's size and hash when the file was written: a replay is checked against it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<ExportCheck>,
    pub history: ExportHistory,
}

const fn one() -> u32 {
    1
}

/// The table as the game was started from it: [`TableSetup`] (map, card set, speaker order,
/// starting technology, Thunder's Edge, seed) and the faction that each seat drew.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSetup {
    /// The seed in it is a string here.
    #[serde(serialize_with = "table_as_text")]
    pub table: TableSetup,
    /// The faction of each seat in the order of `player_ids` (empty for a scenario, which seats
    /// its own).
    pub factions: Vec<FactionId>,
    /// [`state_hash`] of the opening state that this setup makes. A replay that builds another
    /// opening state says so instead of quietly replaying a different game.
    pub initial_state_hash: String,
}

fn table_as_text<S: serde::Serializer>(
    table: &TableSetup,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut value = serde_json::to_value(table).map_err(serde::ser::Error::custom)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("seed".to_owned(), table.seed.to_string().into());
    }
    value.serialize(serializer)
}

/// One seat of the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportSeat {
    /// 0-based, the order of `player_ids`.
    pub seat: usize,
    pub player_id: PlayerId,
    /// The name the player gave at the table (a bot has its own).
    #[serde(default)]
    pub nickname: Option<String>,
    /// `human`, `smart` (the rule bot), `random`, or the name of a test controller.
    pub kind: String,
    pub faction: FactionId,
    /// The rule bot version that plays a `smart` seat, as the build that wrote the file has it.
    /// The decisions are journaled, so a replay does not need the bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_bot_version: Option<u32>,
}

/// What wrote the file and what the game started with.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportBuild {
    /// The commit (build id) of the server or page that wrote the file.
    #[serde(default)]
    pub build_commit: Option<String>,
    /// The crate version of the engine build.
    #[serde(default)]
    pub engine_version: Option<String>,
    /// The digest of the content corpus of that build.
    #[serde(default)]
    pub content_digest: Option<String>,
    /// The hash of the wasm file of the page, for a game played in the browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_sha256: Option<String>,
    /// [`GameState::rules_version`] of the opening state: the rules the game replays under.
    pub rules_version: u32,
    /// The rule bot version that new games of the writing build play.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_bot_version: Option<u32>,
}

/// The count and hash of the journal that the file was written with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportCheck {
    pub decisions: usize,
    /// [`decisions_hash`] of `history.decisions`.
    pub decisions_hash: String,
}

/// The history of the game: what `history.json` of the server holds. Every decision of every
/// seat is in `decisions` (bots, reaction windows that were declined and the single-option steps
/// that the engine settles itself are all either journaled or deterministic), so no answer has to
/// be made up to replay it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportHistory {
    pub decisions: Vec<DecisionRecord>,
    #[serde(default)]
    pub redo: Vec<DecisionRecord>,
    /// The events as the server wrote them. Kept as JSON: a replay does not read them, and a file
    /// of another build may have fields that this build's `GameEvent` does not know.
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
    #[serde(default)]
    pub redo_events: Vec<serde_json::Value>,
    #[serde(default, deserialize_with = "crate::wire::lenient_u64")]
    pub event_counter: u64,
    #[serde(default, deserialize_with = "crate::wire::lenient_u64")]
    pub revision: u64,
    #[serde(default, deserialize_with = "crate::wire::lenient_u64")]
    pub generation: u64,
    #[serde(default)]
    pub batches: Vec<serde_json::Value>,
    /// Forced stream positions of a timeline made by a turn redo, integers as strings.
    #[serde(
        default,
        skip_serializing_if = "RngMarks::is_empty",
        serialize_with = "crate::rng_force::serialize_as_text"
    )]
    pub rng_marks: RngMarks,
}

/// What a replay of an export found.
#[derive(Debug, Clone)]
pub struct ReplayOutcome {
    pub decisions: usize,
    /// [`decisions_hash`] of the decisions the replay made.
    pub decisions_hash: String,
    /// Every replayed decision hashed as the journal's (`ReplayReport::hashes_match`).
    pub hashes_match: bool,
    /// [`state_hash`] of the state after the last decision.
    pub final_state_hash: String,
    pub final_state: GameState,
    /// Whether the opening state built from the setup is the one the file was made with. `None`
    /// when the file has no `initial_state_hash`.
    pub setup_matches: Option<bool>,
}

/// A short hash of the decisions of a game, to tell on replay that the engine still decides the
/// same (FNV-1a over the canonical V1 fingerprint of each record).
#[must_use]
pub fn decisions_hash(records: &[DecisionRecord]) -> String {
    use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for record in records {
        let digest = decision_hash(CanonicalHashVersion::V1, record);
        for byte in format!("{digest:?}").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// A short hash of a whole game state (FNV-1a over its JSON, whose maps are ordered).
#[must_use]
pub fn state_hash(state: &GameState) -> String {
    let text = serde_json::to_string(state).expect("a state is JSON");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl ExportSetup {
    /// The setup of a game that was started from `table` with `factions`: the opening state it
    /// makes must be `initial` (its hash is kept).
    #[must_use]
    pub fn new(table: TableSetup, factions: Vec<FactionId>, initial: &GameState) -> Self {
        Self {
            table,
            factions,
            initial_state_hash: state_hash(initial),
        }
    }

    /// The opening state and the map that this setup makes for the players, in seat order.
    ///
    /// # Errors
    /// A message when the setup does not build (an unknown template, card set or scenario...).
    pub fn build(&self, players: &[PlayerId]) -> Result<(GameState, Galaxy), String> {
        let drawn = Drawn {
            factions: self.factions.clone(),
        };
        crate::setup::create(ContentStore::embedded(), &self.table, players, &drawn)
    }
}

impl ReplayExport {
    /// Reads an export of either format from `text`.
    ///
    /// # Errors
    /// A message when the text is not JSON, is not a `ti4-replay` file, or is from a newer format
    /// than this build knows.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        Self::from_value(value)
    }

    /// [`Self::parse`] of a value already read.
    ///
    /// # Errors
    /// As [`Self::parse`].
    pub fn from_value(value: serde_json::Value) -> Result<Self, String> {
        if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
            return Err(format!("not a {FORMAT} file"));
        }
        let export: Self =
            serde_json::from_value(value).map_err(|e| format!("replay file: {e}"))?;
        if export.format_version > FORMAT_VERSION {
            return Err(format!(
                "replay file format {} is newer than this build reads ({FORMAT_VERSION})",
                export.format_version
            ));
        }
        Ok(export)
    }

    /// The opening state and map that the file's setup makes.
    ///
    /// # Errors
    /// A message when the file has no setup (a format 1 file), or it does not build.
    pub fn opening(&self) -> Result<(GameState, Galaxy), String> {
        let setup = self.setup.as_ref().ok_or_else(|| {
            format!(
                "this replay file has no setup (format {}); the factions, the card set and the \
                 speaker order are not in it, so the opening state cannot be rebuilt",
                self.format_version
            )
        })?;
        setup.build(&self.player_ids)
    }

    /// Plays the game again from its setup through every journaled decision.
    ///
    /// # Errors
    /// A message when the setup is missing or does not build, or the engine does not accept the
    /// journal.
    pub fn replay(&self) -> Result<ReplayOutcome, String> {
        let (state, galaxy) = self.opening()?;
        let setup_matches = self
            .setup
            .as_ref()
            .map(|setup| setup.initial_state_hash == state_hash(&state));
        self.replay_from(&state, Some(&galaxy), setup_matches)
    }

    /// Plays the journal again from an opening state the caller made.
    ///
    /// # Errors
    /// A message when the engine does not accept the journal.
    pub fn replay_from(
        &self,
        state: &GameState,
        galaxy: Option<&Galaxy>,
        setup_matches: Option<bool>,
    ) -> Result<ReplayOutcome, String> {
        let report = crate::replay::replay_session_to_next_question(
            state,
            galaxy,
            &self.history.decisions,
            &self.history.rng_marks,
        )
        .map_err(|e| e.to_string())?;
        Ok(ReplayOutcome {
            decisions: report.decision_count,
            decisions_hash: decisions_hash(&self.history.decisions),
            hashes_match: report.hashes_match,
            final_state_hash: state_hash(&report.final_state),
            final_state: report.final_state,
            setup_matches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::deck_reserve::{DeckSegment, OPEN};

    fn history_with_marks() -> ExportHistory {
        let mut marks = RngMarks::new();
        marks.insert(
            331,
            [("dice".to_owned(), 11_u128), ("big".to_owned(), 1 << 70)].into(),
        );
        marks.deck_plan.push(DeckSegment {
            from: 863,
            until: OPEN,
            reserved: Vec::new(),
        });
        ExportHistory {
            decisions: Vec::new(),
            redo: Vec::new(),
            events: Vec::new(),
            redo_events: Vec::new(),
            event_counter: 7,
            revision: 2,
            generation: 1,
            batches: Vec::new(),
            rng_marks: marks,
        }
    }

    #[test]
    fn big_integers_are_strings_and_read_back_exactly() {
        let history = history_with_marks();
        let text = serde_json::to_string(&history).unwrap();
        assert!(text.contains(r#""until":"18446744073709551615""#), "{text}");
        assert!(text.contains(r#""big":"1180591620717411303424""#), "{text}");
        // A hostile trip: parsed and printed again by something that has no integers past 2^53
        // would turn a number into a float, but a string is untouched.
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let again: ExportHistory = serde_json::from_str(&value.to_string()).unwrap();
        assert_eq!(again.rng_marks, history.rng_marks);
        assert_eq!(again.rng_marks.deck_plan[0].until, OPEN);
    }

    #[test]
    fn a_file_whose_numbers_a_browser_rounded_still_reads() {
        // The rounded end of an open segment (2^64 as a float) is the open end again.
        let text = r#"{"decisions":[],"rng_marks":{"331":{"dice":11},"decks":[{"from":324,"reserved":[],"until":335},{"from":863,"reserved":[],"until":18446744073709552000}]}}"#;
        let history: ExportHistory = serde_json::from_str(text).unwrap();
        assert_eq!(history.rng_marks[&331]["dice"], 11);
        assert_eq!(history.rng_marks.deck_plan[0].until, 335);
        assert_eq!(history.rng_marks.deck_plan[1].until, OPEN);
    }

    #[test]
    fn a_format_1_file_loads_without_a_setup_and_says_why_it_cannot_replay() {
        let text = r#"{"format":"ti4-replay","version":1,"game_id":"g","seed":18432220952314264000,"player_ids":["a","b"],"map_template":"x","history":{"decisions":[],"events":[],"revision":1,"generation":0,"event_counter":0}}"#;
        let export = ReplayExport::parse(text).unwrap();
        assert_eq!(export.format_version, 1);
        assert_eq!(export.seed, Some(18_432_220_952_314_264_000));
        assert!(export.setup.is_none() && export.seats.is_empty());
        let error = export.replay().unwrap_err();
        assert!(error.contains("no setup"), "{error}");
        assert!(ReplayExport::parse(r#"{"format":"other"}"#).is_err());
        let newer = text.replace(r#""version":1"#, r#""version":9,"format_version":9"#);
        assert!(ReplayExport::parse(&newer).unwrap_err().contains("newer"));
    }

    #[test]
    fn the_seed_is_a_string_in_the_setup_and_the_envelope() {
        let table = TableSetup::new(2, 18_432_220_952_314_264_515);
        let (state, _) = {
            let players = [PlayerId::new("a"), PlayerId::new("b")];
            let drawn = crate::setup::draw(&table, &[None, None]).unwrap();
            crate::setup::create(ContentStore::embedded(), &table, &players, &drawn).unwrap()
        };
        let factions = state.players.iter().map(|p| p.faction.clone()).collect();
        let setup = ExportSetup::new(table, factions, &state);
        let text = serde_json::to_string(&setup).unwrap();
        assert!(text.contains(r#""seed":"18432220952314264515""#), "{text}");
        let back: ExportSetup = serde_json::from_str(&text).unwrap();
        assert_eq!(back.table.seed, 18_432_220_952_314_264_515);
        // And it rebuilds the state it was made from.
        let (rebuilt, _) = back
            .build(&[PlayerId::new("a"), PlayerId::new("b")])
            .unwrap();
        assert_eq!(state_hash(&rebuilt), back.initial_state_hash);
    }
}
