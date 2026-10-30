//! The corpus test and the order test of the key codecs.
//!
//! `corpus/marks.tsv` and `corpus/other.tsv` hold every key and value that the debug dump
//! (`corpus/keydump.patch`) saw in the 200-game golden set (which contains the 20 profile games),
//! in every test of the workspace that builds or loads a state (the fixtures included). See
//! `corpus/README.md`.

use std::collections::{BTreeMap, BTreeSet};

use ti4_engine_core::containers::Interner;
use ti4_engine_core::generated::{
    ActionCard, Agenda, Faction, Leader, Planet, PromissoryNote, Relic, SecretObjective,
    StrategyCard, System, Technology, Unit, Wormhole,
};
use ti4_engine_core::keys::{
    Atom, DecimalTextOrd, Expedition, ExploreDeck, Family, LedgerReason, MarkKey, Word, tables,
};

use super::marks::PATTERNS;
use super::other::{
    parse_destroy, parse_expedition, parse_explore_deck, parse_ledger, render_destroy,
    render_expedition, render_explore_deck, render_ledger,
};
use super::text::{Codec, Owners};
use super::word_text::WORD_TEXT;
use crate::generated::IdStr;

const MARKS: &str = include_str!("corpus/marks.tsv");
const OTHER: &str = include_str!("corpus/other.tsv");
const UNMAPPED: &str = include_str!("corpus/unmapped.txt");

fn json(text: &str) -> String {
    serde_json::from_str(text).expect("a json string")
}

fn marks() -> Vec<(String, String)> {
    MARKS
        .lines()
        .map(|line| {
            let (key, value) = line.split_once('\t').expect("key tab value");
            (json(key), json(value))
        })
        .collect()
}

fn other(kind: &str) -> Vec<String> {
    OTHER
        .lines()
        .filter_map(|line| {
            let (k, text) = line.split_once('\t')?;
            (k == kind).then(|| json(text))
        })
        .collect()
}

fn codec() -> Codec {
    let owners = Owners::new(["a", "b", "c", "d", "e", "f", "g", "h"]).expect("a table");
    Codec::new(owners)
}

// ---- tables ------------------------------------------------------------------------------

#[test]
fn pattern_table_is_in_family_order() {
    for (i, (family, _)) in PATTERNS.iter().enumerate() {
        assert_eq!(family.index(), i, "{family:?}");
        assert_eq!(Family::ALL[i], *family);
    }
}

#[test]
fn word_texts_are_in_text_order() {
    for pair in WORD_TEXT.windows(2) {
        assert!(pair[0].as_bytes() < pair[1].as_bytes(), "{pair:?}");
    }
}

#[test]
fn seats_follow_text_order_and_refuse_prefix_ids() {
    let owners = Owners::new(["bob", "alice", "carol"]).unwrap();
    assert_eq!(owners.seat("alice").unwrap().0, 0);
    assert_eq!(owners.seat("carol").unwrap().0, 2);
    assert_eq!(owners.id(owners.seat("bob").unwrap()), Some("bob"));
    assert!(
        Owners::new(["p1", "p10"]).is_none(),
        "p1 is a prefix of p10"
    );
    assert!(
        Owners::new((0..17).map(|i| [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "q", "r"
        ][i]))
        .is_none()
    );
}

type Pairs = Vec<(u16, u16, u8)>;
type Table<'a> = (&'a str, Pairs, &'a [(u16, u16, u8)]);

/// Every pair of ids of one type where one is a proper prefix of the other, with the byte of the
/// longer after the shorter.
fn prefix_pairs<T: IdStr>() -> Vec<(u16, u16, u8)> {
    let ids = T::IDS;
    let mut out = Vec::new();
    for (a, short) in ids.iter().enumerate() {
        for (b, long) in ids.iter().enumerate().skip(a + 1) {
            if long.len() > short.len() && long.starts_with(short) {
                out.push((a as u16, b as u16, long.as_bytes()[short.len()]));
            }
        }
    }
    out
}

fn table_text(name: &str, pairs: &[(u16, u16, u8)]) -> String {
    let rows: Vec<String> = pairs
        .iter()
        .map(|(a, b, c)| format!("({a}, {b}, {c})"))
        .collect();
    format!(
        "#[rustfmt::skip]\npub const {name}: PrefixPairs = &[{}];\n",
        rows.join(", ")
    )
}

#[test]
fn prefix_tables_are_current() {
    let (rank, note_pairs) = note_ranks();
    assert_eq!(
        rank.as_slice(),
        tables::NOTE_RANK,
        "stale note ranks: regenerate the tables"
    );
    let current: [Table; 10] = [
        ("SYSTEM", prefix_pairs::<System>(), tables::SYSTEM),
        ("PLANET", prefix_pairs::<Planet>(), tables::PLANET),
        ("UNIT", prefix_pairs::<Unit>(), tables::UNIT),
        (
            "TECHNOLOGY",
            prefix_pairs::<Technology>(),
            tables::TECHNOLOGY,
        ),
        (
            "ACTION_CARD",
            prefix_pairs::<ActionCard>(),
            tables::ACTION_CARD,
        ),
        ("PROMISSORY_NOTE", note_pairs, tables::PROMISSORY_NOTE),
        ("AGENDA", prefix_pairs::<Agenda>(), tables::AGENDA),
        ("FACTION", prefix_pairs::<Faction>(), tables::FACTION),
        ("LEADER", prefix_pairs::<Leader>(), tables::LEADER),
        ("WORD", word_pairs(), tables::WORD),
    ];
    for (name, expected, actual) in current {
        assert_eq!(
            expected.as_slice(),
            actual,
            "stale: run `cargo test -p ti4-engine print_prefix_tables -- --ignored --nocapture` and paste\n{}",
            table_text(name, &expected)
        );
    }
}

fn word_pairs() -> Vec<(u16, u16, u8)> {
    text_pairs(&WORD_TEXT)
}

/// The prefix pairs of texts that are already in text order.
fn text_pairs(texts: &[&str]) -> Vec<(u16, u16, u8)> {
    let mut out = Vec::new();
    for (a, short) in texts.iter().enumerate() {
        for (b, long) in texts.iter().enumerate().skip(a + 1) {
            if long.len() > short.len() && long.starts_with(short) {
                out.push((a as u16, b as u16, long.as_bytes()[short.len()]));
            }
        }
    }
    out
}

/// The promissory notes by the text the state writes (`an` for `<color>_an`): the rank of each id
/// in that order, and the prefix pairs of the ranks.
fn note_ranks() -> (Vec<u16>, Vec<(u16, u16, u8)>) {
    let texts: Vec<&str> = PromissoryNote::IDS
        .iter()
        .map(|id| id.strip_prefix("<color>_").unwrap_or(id))
        .collect();
    let mut order: Vec<usize> = (0..texts.len()).collect();
    order.sort_by_key(|i| texts[*i].as_bytes());
    let mut rank = vec![0u16; texts.len()];
    for (r, i) in order.iter().enumerate() {
        rank[*i] = r as u16;
    }
    let sorted: Vec<&str> = order.iter().map(|i| texts[*i]).collect();
    (rank, text_pairs(&sorted))
}

/// The generator of `ti4-engine-core/src/keys/tables.rs`.
#[test]
#[ignore = "prints the prefix tables"]
fn print_prefix_tables() {
    let mut out = String::from(
        "//! Prefix pairs of the id types (see `order`). Numbers only.\n//! Regenerate with `cargo test -p ti4-engine print_prefix_tables -- --ignored --nocapture`\n//! (the test `prefix_tables_are_current` fails while this file is stale).\n\nuse super::order::PrefixPairs;\n\npub const NONE: PrefixPairs = &[];\n",
    );
    out += &table_text("SYSTEM", &prefix_pairs::<System>());
    out += &table_text("PLANET", &prefix_pairs::<Planet>());
    out += &table_text("UNIT", &prefix_pairs::<Unit>());
    out += &table_text("TECHNOLOGY", &prefix_pairs::<Technology>());
    out += &table_text("ACTION_CARD", &prefix_pairs::<ActionCard>());
    let (rank, note_pairs) = note_ranks();
    out += &format!(
        "#[rustfmt::skip]\npub const NOTE_RANK: [u16; {}] = {rank:?};\n",
        rank.len()
    );
    out += &table_text("PROMISSORY_NOTE", &note_pairs);
    out += &table_text("AGENDA", &prefix_pairs::<Agenda>());
    out += &table_text("FACTION", &prefix_pairs::<Faction>());
    out += &table_text("LEADER", &prefix_pairs::<Leader>());
    out += &table_text("WORD", &word_pairs());
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ti4-engine-core/src/keys/tables.rs"
        ),
        out,
    )
    .unwrap();
}

#[test]
fn id_namespaces_that_share_a_text_position_are_disjoint() {
    fn overlap<A: IdStr, B: IdStr>() -> Vec<&'static str> {
        A::IDS
            .iter()
            .copied()
            .filter(|id| B::IDS.contains(id))
            .collect()
    }
    // Outcome kinds are tried in order without the agenda's hint: no text may be two kinds.
    assert!(overlap::<Planet, StrategyCard>().is_empty());
    assert!(overlap::<Planet, Agenda>().is_empty());
    assert!(overlap::<Planet, SecretObjective>().is_empty());
    assert!(overlap::<StrategyCard, Agenda>().is_empty());
    assert!(overlap::<StrategyCard, SecretObjective>().is_empty());
    assert!(overlap::<Agenda, SecretObjective>().is_empty());
    // A bare leader id is a destruction cause; the fixed words are not leader ids.
    for word in [
        "nova_seed",
        "bombardment",
        "sundered",
        "last_dispatch",
        "plot_seethe",
    ] {
        assert!(Leader::from_id(word).is_none(), "{word} is a leader id");
    }
    // Ledger words against relic and agenda ids.
    for word in [
        "custodians",
        "imperial_primary",
        "imperial_rider",
        "objective",
        "secret_objective",
        "styx",
        "support_for_the_throne",
    ] {
        assert!(Relic::from_id(word).is_none(), "{word} is a relic id");
    }
}

// ---- the corpus --------------------------------------------------------------------------

/// A mark from the corpus either round-trips or is listed, with its reason, in
/// `corpus/unmapped.txt`: the test keys and the ids that are not in the compiled-in content
/// (systems and planets a test invents). Nothing else may fail.
#[test]
fn every_observed_mark_round_trips_or_is_a_listed_exception() {
    let mut blobs = Interner::default();
    let mut codec = codec();
    let mut typed = 0usize;
    let mut unmapped = BTreeSet::new();
    for (key, value) in marks() {
        match codec.parse_mark(&mut blobs, &key, &value) {
            Some((k, v)) => {
                let (rk, rv) = codec.render_mark(&blobs, &k, &v).expect("a parsed mark renders");
                assert_eq!((rk.as_str(), rv.as_str()), (key.as_str(), value.as_str()));
                typed += 1;
            }
            None => {
                let reason = match codec.parse_mark_key(&mut blobs, &key) {
                    None => "key",
                    Some(_) => "value",
                };
                unmapped.insert(format!("{reason}\t{key}\t{value}"));
            }
        }
    }
    if std::env::var_os("X1_WRITE_UNMAPPED").is_some() {
        let mut text = String::from(
            "# reason<TAB>key<TAB>value: marks of the corpus the codec does not type, see corpus/README.md\n",
        );
        for line in &unmapped {
            text += line;
            text.push('\n');
        }
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/core_sync/keys/corpus/unmapped.txt"
            ),
            text,
        )
        .unwrap();
        return;
    }
    let listed: BTreeSet<String> = UNMAPPED
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(str::to_owned)
        .collect();
    let new: Vec<_> = unmapped.difference(&listed).collect();
    let gone: Vec<_> = listed.difference(&unmapped).collect();
    assert!(
        new.is_empty() && gone.is_empty(),
        "typed {typed}; unmapped but not listed: {new:#?}\nlisted but now typed: {gone:#?}"
    );
    assert!(typed > 7000, "typed {typed}");
}

#[test]
fn every_observed_key_text_is_typed_or_a_listed_exception() {
    let mut blobs = Interner::default();
    let mut codec = codec();
    let mut keys: BTreeSet<String> = marks().into_iter().map(|(k, _)| k).collect();
    keys.insert("allow_pass".to_owned());
    let mut untyped = Vec::new();
    for key in &keys {
        match codec.parse_mark_key(&mut blobs, key) {
            Some(typed) => assert_eq!(codec.render_mark_key(&blobs, &typed).as_deref(), Some(key.as_str())),
            None => untyped.push(key.clone()),
        }
    }
    for key in &untyped {
        let listed = UNMAPPED
            .lines()
            .any(|line| line.split('\t').nth(1) == Some(key.as_str()));
        assert!(listed, "{key} is neither typed nor listed");
    }
}

#[test]
fn every_family_is_in_the_corpus() {
    let mut blobs = Interner::default();
    let mut codec = codec();
    let mut seen = BTreeSet::new();
    for (key, _) in marks() {
        if let Some(k) = codec.parse_mark_key(&mut blobs, &key) {
            seen.insert(k.family());
        }
    }
    let missing: Vec<_> = Family::ALL.iter().filter(|f| !seen.contains(f)).collect();
    assert!(
        missing.is_empty(),
        "families the corpus never saw: {missing:?}"
    );
}

/// Covert Legislation (rules version >= 33) is voted on the outcomes of the face-down agenda
/// (`vote::voted_agenda`): its row must accept every elected kind, not only for / against.
#[test]
fn hidden_next_agenda_outcomes_are_any_kind() {
    use ti4_engine_core::generated::AgendaElects;
    let codec = codec();
    assert!(codec.parse_outcome("for", Some(AgendaElects::HiddenNextAgenda)).is_some());
    let law = Agenda::ALL[0].as_str();
    assert!(codec.parse_outcome(law, Some(AgendaElects::HiddenNextAgenda)).is_some());
    assert!(codec.parse_outcome(law, Some(AgendaElects::ForAgainst)).is_none());
}

/// `agenda_predictions` values (not in the dump's corpus; the gate over the golden games sees them).
#[test]
fn predictions_round_trip() {
    let codec = codec();
    for text in [
        "for", "against", "for|tech_rider", "against|trade_rider", "for|keleres_rider", "for|sanction",
        "none|assassin", "none|political_secret",
    ] {
        let p = codec.parse_prediction(text).unwrap_or_else(|| panic!("{text}"));
        assert_eq!(codec.render_prediction(p).as_deref(), Some(text));
    }
    for text in ["none", "none|x", "for|x", "FOR|lead_rider", "|"] {
        assert!(codec.parse_prediction(text).is_none(), "{text}");
    }
}

#[test]
fn observed_outcomes_round_trip() {
    let codec = codec();
    let mut n = 0;
    for kind in ["law_val", "vote", "choice", "veto"] {
        for text in other(kind) {
            // law_val is "key\u{1}value"
            let value = text.split('\u{1}').next_back().unwrap();
            match codec.parse_outcome(value, None) {
                Some(outcome) => {
                    assert_eq!(
                        codec.render_outcome(outcome).as_deref(),
                        Some(value),
                        "{kind}"
                    );
                    n += 1;
                }
                None => panic!("{kind}: outcome {value:?} is not typed"),
            }
        }
    }
    assert!(n > 100, "{n}");
}

#[test]
fn observed_law_keys_round_trip() {
    for text in other("law_key") {
        let agenda = Agenda::from_id(&text).unwrap_or_else(|| panic!("law key {text}"));
        assert_eq!(agenda.as_str(), text);
    }
}

#[test]
fn observed_ledger_reasons_round_trip() {
    for text in other("ledger") {
        match parse_ledger(&text) {
            Some(reason) => assert_eq!(render_ledger(reason), text),
            None => assert!(
                text == "a reason of tomorrow",
                "ledger reason {text:?} is not typed"
            ),
        }
    }
    assert_eq!(
        parse_ledger("agenda:censure"),
        Some(LedgerReason::Agenda(Agenda::from_id("censure").unwrap()))
    );
    assert!(parse_ledger("thesilverflame").is_some());
}

#[test]
fn observed_destruction_causes_round_trip() {
    let mut texts = other("destroy");
    // The causes of the staged ground destructions are part of the mark values.
    for (_, value) in marks() {
        if let Some(rest) = value.strip_prefix("destroyed|") {
            texts.push(rest.rsplit('|').next().unwrap().to_owned());
        }
    }
    let mut typed = 0;
    let mut untyped = BTreeSet::new();
    for text in texts {
        match parse_destroy(&text) {
            Some(reason) => {
                assert_eq!(render_destroy(reason), text);
                typed += 1;
            }
            None => {
                untyped.insert(text);
            }
        }
    }
    assert_eq!(
        untyped.iter().map(String::as_str).collect::<Vec<_>>(),
        ["action_card:test", "test"],
        "test-only causes are the only untyped ones"
    );
    assert!(typed > 20, "{typed}");
}

#[test]
fn observed_deck_and_slice_keys_round_trip() {
    for text in other("deck_key") {
        let deck = parse_explore_deck(&text).unwrap_or_else(|| panic!("deck {text}"));
        assert_eq!(render_explore_deck(deck), text);
    }
    for text in other("expedition") {
        let slice = parse_expedition(&text).unwrap_or_else(|| panic!("slice {text}"));
        assert_eq!(render_expedition(slice), text);
    }
    for text in other("wormhole_token") {
        let kind = Wormhole::from_id(&text).unwrap_or_else(|| panic!("wormhole {text}"));
        assert_eq!(kind.as_str(), text);
    }
    assert_eq!(
        ExploreDeck::ALL.map(render_explore_deck),
        ["CULTURAL", "FRONTIER", "HAZARDOUS", "INDUSTRIAL"]
    );
    let slices: Vec<_> = Expedition::ALL.map(render_expedition).to_vec();
    let mut sorted = slices.clone();
    sorted.sort_unstable();
    assert_eq!(slices, sorted);
}

// ---- iteration order ---------------------------------------------------------------------

/// Iterating a `BTreeMap<MarkKey, _>` visits the keys in the order the `BTreeMap<String, _>` does.
fn assert_same_order(codec: &Codec, texts: &BTreeSet<String>, label: &str) {
    let mut blobs = Interner::default();
    let mut codec = codec.clone();
    let strings: BTreeMap<String, ()> = texts.iter().map(|t| (t.clone(), ())).collect();
    let mut typed: BTreeMap<MarkKey, ()> = BTreeMap::new();
    for text in texts {
        typed.insert(
            codec
                .parse_mark_key(&mut blobs, text)
                .unwrap_or_else(|| panic!("{label}: {text}")),
            (),
        );
    }
    assert_eq!(typed.len(), strings.len(), "{label}: two texts, one key");
    let rendered: Vec<String> = typed
        .keys()
        .map(|k| codec.render_mark_key(&blobs, k).expect("renders"))
        .collect();
    let expected: Vec<&String> = strings.keys().collect();
    let got: Vec<&String> = rendered.iter().collect();
    if got != expected {
        let at = got.iter().zip(&expected).position(|(a, b)| a != b).unwrap();
        panic!(
            "{label}: order differs at {at}: typed {:?} / text {:?}",
            &got[at..(at + 3).min(got.len())],
            &expected[at..(at + 3).min(expected.len())]
        );
    }
}

#[test]
fn observed_keys_iterate_in_text_order() {
    let mut blobs = Interner::default();
    let codec = codec();
    let mut parse = codec.clone();
    let texts: BTreeSet<String> = marks()
        .into_iter()
        .map(|(k, _)| k)
        .filter(|k| parse.parse_mark_key(&mut blobs, k).is_some())
        .collect();
    assert!(texts.len() > 800, "{}", texts.len());
    assert_same_order(&codec, &texts, "corpus");
}

/// Ids that are prefixes of others and a few that are not.
fn pool<T: IdStr>(extra: usize) -> Vec<&'static str> {
    let mut ids: BTreeSet<&'static str> = BTreeSet::new();
    for (a, b, _) in prefix_pairs::<T>().into_iter().take(40) {
        ids.insert(T::IDS[a as usize]);
        ids.insert(T::IDS[b as usize]);
    }
    for id in T::IDS
        .iter()
        .step_by((T::IDS.len() / extra.max(1)).max(1))
        .take(extra)
    {
        ids.insert(id);
    }
    ids.into_iter().collect()
}

/// Every family with its fields drawn from small pools that contain the prefix-colliding ids.
#[test]
fn constructed_keys_iterate_in_text_order() {
    let mut blobs = Interner::default();
    let codec = codec();
    let mut parse = codec.clone();
    let systems: Vec<&str> = {
        let mut p = pool::<System>(3);
        p.truncate(7);
        for id in ["01", "10", "100", "101", "11", "82a", "9"] {
            if System::from_id(id).is_some() && !p.contains(&id) {
                p.push(id);
            }
        }
        p.truncate(8);
        p
    };
    let planets: Vec<&str> = pool::<Planet>(2).into_iter().take(6).collect();
    let units: Vec<&str> = pool::<Unit>(3).into_iter().take(6).collect();
    let techs: Vec<&str> = pool::<Technology>(3).into_iter().take(5).collect();
    let notes: Vec<&str> = pool::<PromissoryNote>(2).into_iter().take(4).collect();
    let factions: Vec<&str> = pool::<Faction>(2).into_iter().take(3).collect();
    let leaders: Vec<&str> = pool::<Leader>(3).into_iter().take(5).collect();
    let seats = ["a", "b", "c"];
    let decs = ["0", "1", "10", "100", "11", "2", "9", "99"];
    let words = |ws: &[Word]| -> Vec<String> {
        ws.iter().map(|w| WORD_TEXT[w.index()].to_owned()).collect()
    };
    let mut all = BTreeSet::new();
    for (family, segs) in &PATTERNS {
        let pools: Vec<Vec<String>> = field_kinds(segs)
            .iter()
            .map(|kind| match kind {
                Kind::System => systems.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Planet => planets.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Unit => units.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Tech => techs.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Note => notes.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Faction => factions.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Leader => leaders.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Seat => seats.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Dec => decs.iter().map(|s| (*s).to_owned()).collect(),
                Kind::Pad(w) => ["0", "1", "9", "10", "12"]
                    .iter()
                    .map(|n| format!("{n:0>w$}", w = usize::from(*w)))
                    .collect(),
                Kind::Word(ws) => words(ws),
                Kind::OptPlanet => {
                    let mut v: Vec<String> =
                        planets.iter().take(3).map(|s| (*s).to_owned()).collect();
                    v.push(String::new());
                    v
                }
            })
            .collect();
        let mut count = 0usize;
        let mut indexes = vec![0usize; pools.len()];
        'product: loop {
            let fields: Vec<&str> = indexes
                .iter()
                .zip(&pools)
                .map(|(i, p)| p[*i].as_str())
                .collect();
            let text = fill(segs, &fields);
            if parse.parse_mark_key(&mut blobs, &text).is_some() {
                all.insert(text);
                count += 1;
            }
            let mut d = pools.len();
            loop {
                if d == 0 {
                    break 'product;
                }
                d -= 1;
                indexes[d] += 1;
                if indexes[d] < pools[d].len() {
                    break;
                }
                indexes[d] = 0;
            }
        }
        assert!(
            count > 0 || pools.iter().any(Vec::is_empty),
            "{family:?} produced no key"
        );
    }
    assert!(all.len() > 5000, "{}", all.len());
    assert_same_order(&codec, &all, "constructed");
}

#[derive(Clone, Copy)]
enum Kind {
    Seat,
    System,
    Planet,
    Unit,
    Tech,
    Note,
    Faction,
    Leader,
    Dec,
    Pad(u8),
    Word(&'static [Word]),
    OptPlanet,
}

use super::marks::Seg;
use super::text::K;

fn field_kinds(segs: &[Seg]) -> Vec<Kind> {
    let mut out = Vec::new();
    for seg in segs {
        match seg {
            Seg::L(_) => {}
            Seg::F(k) => out.push(match k {
                K::Seat => Kind::Seat,
                K::System => Kind::System,
                K::Planet => Kind::Planet,
                K::Unit => Kind::Unit,
                K::Tech => Kind::Tech,
                K::Note => Kind::Note,
                K::Faction => Kind::Faction,
                K::Leader => Kind::Leader,
                K::Dec => Kind::Dec,
                K::Pad(w) => Kind::Pad(*w),
                K::Word(ws) => Kind::Word(ws),
                other => panic!("no pool for {other:?}"),
            }),
            Seg::O(group) => {
                assert_eq!(group.len(), 2);
                out.push(Kind::OptPlanet);
            }
        }
    }
    out
}

fn fill(segs: &[Seg], fields: &[&str]) -> String {
    let mut out = String::new();
    let mut i = 0;
    for seg in segs {
        match seg {
            Seg::L(l) => out.push_str(l),
            Seg::F(_) => {
                out.push_str(fields[i]);
                i += 1;
            }
            Seg::O(group) => {
                if !fields[i].is_empty() {
                    if let Seg::L(l) = group[0] {
                        out.push_str(l);
                    }
                    out.push_str(fields[i]);
                }
                i += 1;
            }
        }
    }
    out
}

#[test]
fn a_following_byte_decides_between_prefix_ids() {
    let mut blobs = Interner::default();
    // The known trap: system 10 is a prefix of system 100, and `:` sorts after every digit.
    let mut codec = codec();
    let low = "bastion:lost:100:a:cruiser";
    let high = "bastion:lost:10:a:cruiser";
    assert!(low < high, "text order");
    let (low_key, high_key) = (
        codec.parse_mark_key(&mut blobs, low).unwrap(),
        codec.parse_mark_key(&mut blobs, high).unwrap(),
    );
    assert!(low_key < high_key);
    // At the end of the key the shorter id comes first.
    let (a, b) = (
        codec.parse_mark_key(&mut blobs, "crimson:breach:active:10").unwrap(),
        codec.parse_mark_key(&mut blobs, "crimson:breach:active:100").unwrap(),
    );
    assert!(a < b);
}

#[test]
fn decimal_keys_sort_as_text() {
    let mut blobs = Interner::default();
    let mut codec = codec();
    let texts = [
        "private:#moved:1",
        "private:#moved:10",
        "private:#moved:2",
        "private:#moved:9",
    ];
    let mut keys: Vec<MarkKey> = texts
        .iter()
        .map(|t| codec.parse_mark_key(&mut blobs, t).unwrap())
        .collect();
    keys.reverse();
    keys.sort();
    let sorted: Vec<String> = keys
        .iter()
        .map(|k| codec.render_mark_key(&blobs, k).unwrap())
        .collect();
    assert_eq!(sorted, texts);
    assert_eq!(
        codec.parse_mark_key(&mut blobs, "private:#moved:2"),
        Some(MarkKey::PrivateMoved(DecimalTextOrd(2)))
    );
    // Not canonical numbers do not parse.
    assert!(codec.parse_mark_key(&mut blobs, "private:#moved:02").is_none());
    assert!(codec.parse_mark_key(&mut blobs, "private:#moved:").is_none());
}

#[test]
fn atoms_of_a_key_are_what_its_fields_say() {
    let mut blobs = Interner::default();
    let mut codec = codec();
    let key = codec.parse_mark_key(&mut blobs, "cabal:devour:06|0.0.0").unwrap();
    let (atoms, n) = key.atoms();
    assert_eq!(n, 2);
    assert!(matches!(atoms[0], Atom::System(_)) && matches!(atoms[1], Atom::Planet(_)));
    let bare = codec.parse_mark_key(&mut blobs, "cabal:devour:06").unwrap();
    assert!(matches!(bare.atoms().0[1], Atom::Absent));
    assert!(bare < key);
    assert_eq!(MarkKey::from_atoms(key.family(), &atoms[..n]), Some(key));
}

#[test]
fn closed_key_sets_iterate_in_text_order() {
    // Law keys: the typed key is the agenda, numbered in the text order of the aliases.
    let mut laws: Vec<&str> = Agenda::IDS.to_vec();
    laws.sort_unstable();
    assert_eq!(laws, Agenda::IDS);
    let mut systems: Vec<&str> = System::IDS.to_vec();
    systems.sort_unstable();
    assert_eq!(systems, System::IDS);
    let texts: Vec<_> = ExploreDeck::ALL.map(render_explore_deck).to_vec();
    let mut sorted = texts.clone();
    sorted.sort_unstable();
    assert_eq!(texts, sorted);
    assert!(ExploreDeck::ALL.windows(2).all(|w| w[0] < w[1]));
    assert!(Expedition::ALL.windows(2).all(|w| w[0] < w[1]));
    let mut wormholes: Vec<&str> = Wormhole::IDS.to_vec();
    wormholes.sort_unstable();
    assert_eq!(wormholes, Wormhole::IDS);
}
