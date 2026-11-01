//! The refusals of the codecs and their audit (package D2-1, docs/todo/D2-prereqs.md item 5).
//!
//! [`Unmapped`] is what the typed core cannot express. Every one is built by [`bad`], which counts it by
//! (slice, reason) in [`super::unmapped`]; the gate binaries print the census and run strict. The table classifies
//! **every refusal site** of `core_sync` by whether a game that production can create reaches it. "Production" is:
//! `ti4_host::setup::create` (server lobby start, wasm new game), which now refuses what the typed state cannot hold
//! ([`super::guard`]); the embedded content corpus; games played by the engine itself. After D2-8 a refusal is a hard
//! error, so a site marked "reachable" must be closed first.
//!
//! | Site | (slice, reason) | Reachable from production input | Test (`tests_totality.rs`) |
//! |---|---|---|---|
//! | `ctx.rs` `Owners::with_extra` | (Seats, "more than 16 owners") | no: at most 8 players, the other owners are the neutral owner and holders of a plot token / captured unit, all of them players | `more_than_sixteen_owners_are_refused_when_the_owner_table_is_built` |
//! | `ctx.rs` `Ctx::seat` / `Ctx::owner` | (any, "owner not in the table" / "seat not in the table") | no: the table is built from the same state it then reads | `an_owner_or_seat_the_table_does_not_know_is_refused` |
//! | `ctx.rs` `Texts::intern` / `Texts::text` | (any, "text table full" / "unknown text code") | no: measured high-water marks are 38 texts / 3 blobs (`core_sync_gate` over 3 golden games, 100 template openings) and 32 texts / 3 blobs (`alpha_roundtrip`, 1 030 positions); both far below 65 535 | `the_text_table_refuses_the_65536th_text_and_a_code_it_never_issued` |
//! | `keys::Owners::new` (not an `Unmapped`: `Ctx.keys.owners` is `None`) | marks and outcomes then refuse "mark not typed" / "outcome not typed" | no: `guard::ids_supported` refuses a table with an id that is a proper prefix of another when the game is created | `an_id_that_is_a_proper_prefix_...` |
//! | `codec/seats.rs` more than 8 players | (Seats, "more than 8 players") | no: the host table size is `PLAYER_COUNTS = 2..=8`, `guard::ids_supported` refuses a ninth id | `a_table_the_seats_codec_cannot_hold_is_refused`, host `creation_guard.rs` |
//! | `codec/seats.rs` seating order | (Seats, "seating_order is not one entry per player") | no: the engine keeps one entry per player | same |
//! | `conv::id` in `players.rs` faction | (Players, "faction") | no: `guard::factions_known` refuses a non-corpus faction at creation; `generic` exists only in `fixtures::game` | `a_faction_outside_the_corpus_is_refused` |
//! | `cards.rs` `note_key` ("promissory note id", "promissory note", "faction") | (Cards, ...) | no: notes are dealt as `alias:faction` from the corpus | `a_promissory_note_that_is_not_alias_and_faction_is_refused` |
//! | `cards.rs` `deck_reserve` | (Cards, "deck_reserve (turn-redo replay)") | **disabled feature** (XF-2): D2-2 turns it into a panic | `unsupported_tests.rs` |
//! | `trade.rs` diplomacy rows (empty relationships row, deal key) | (Trade, "diplomacy: ...") | **disabled feature** (XF-1): D2-2 | `unsupported_tests.rs` |
//! | every other `conv::id` (units, systems, planets, technologies, leaders, relics, agendas, exploration cards, strategy cards, attachments, wormholes, galactic events, secret objectives, action cards, breakthroughs) in `board`, `players`, `meta`, `decks`, `explore`, `agenda`, `objectives`, `strategy`, `ledger`, `transient`, `trade` | (that slice, the id kind) | no: every id a game holds was drawn from the embedded corpus, which the generated enums cover (`gen-cards --check`); a custom or hand-edited content id only | `a_content_id_outside_the_corpus_is_refused_by_the_slice_that_holds_it` (unit type, planet, technology, strategy card: same constructor for all) |
//! | `conv::narrow`, `conv::opt32` (all counters of `players`, `meta`) | (Players / Meta, the counter name) | no: counters are small (a tactic pool of 16, trade goods below 100, turn stamps below 2^31) | `a_counter_out_of_the_typed_range_is_refused_instead_of_wrapped` |
//! | `board.rs` planet entry stored under another system / without a system / in an untouched system | (Board, "planet entry ...") | no: `placed_planets` and `planet_attachments` are written together with the system | not constructible without a `Galaxy` fixture: the golden positions (`core_sync_gate`) and the alpha logs are the dynamic check |
//! | `marks.rs` | (Marks, "mark not typed" / "mark text is not canonical") | **the one open set**: every `faction_marks` key template must be one of the typed families; `tools/engine_core/marks_census.py` lists the templates in the engine and checks them against `keys/marks.rs` | `a_mark_nobody_typed_and_a_pending_tag_nobody_knows_are_refused`; the golden/alpha runs are the dynamic check |
//! | `pending.rs` | (Pending, "pending tag") | no: the engine writes only `"move"` | same |
//! | `agenda.rs` outcome / prediction not typed, agenda name differs from the corpus | (Agenda, ...) | no: outcomes are the printed ones or a player id of the table; names come from the corpus | not constructible without an agenda in play: covered by the golden agenda positions |
//! | `ledger.rs` ledger reason not typed | (Ledger, "ledger reason not typed") | no: the reasons are the engine's closed set (`keys`, X1) | covered by the golden positions |
//! | `explore.rs` expedition slice | (Explore, "expedition slice") | no: the six printed expedition keys | covered by the Thunder's Edge template openings |
//! | `transient.rs` rerolled die index, starting tech list too long | (Transient, ...) | no: at most 12 dice groups and 4 starting technologies | covered by the golden positions |
//! | `players.rs` fragment trait, plot, plot form, plot card | (Players, ...) | no: the engine writes the four fragment traits and the printed plots | covered by the golden positions |
//! | `map.rs` no galaxy / galaxy not expressible | (Map, ...) | no: a game has its galaxy; the typed map covers every template of `TemplateLoader` and the hyperlane tiles (**verified dynamically**: `core_sync_gate` loads the Map slice of all 100 buildable template openings, `alpha_roundtrip` that of the stored games including a `4pHyperlanes` one: 0 refusals; 30 template specs do not build at all, which is the spec's refusal, not the codec's) | `the_map_slice_without_the_string_galaxy_is_refused` |
//! | `rng.rs` rng domain | (Rng, "rng domain") | no: all 22 production domains are literals; `RngDomain::Dyn` is minted nowhere outside the codec (D2-3 adds the debug assertion) | none yet (D2-3) |
//!
//! Sites marked "covered by the golden positions" are not constructible by a small unit test without rebuilding half of
//! a game around them; they are exercised dynamically by the round trip over the golden games, the template openings
//! and the alpha logs, and the census must stay empty there. `core_adapter::Unmapped` (no slice) is counted under the
//! pseudo-slice `adapter` with the same machinery.

use ti4_engine_core::state::Slice;

/// Something the typed core cannot express; the caller falls back to the string engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    pub slice: Slice,
    pub what: &'static str,
    pub text: String,
}

impl std::fmt::Display for Unmapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {} {:?}", self.slice, self.what, self.text)
    }
}

impl std::error::Error for Unmapped {}

pub type R<T> = Result<T, Unmapped>;

#[must_use]
pub fn bad(slice: Slice, what: &'static str, text: impl ToString) -> Unmapped {
    let u = Unmapped { slice, what, text: text.to_string() };
    super::unmapped::note(&u);
    u
}
