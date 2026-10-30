//! The file formats of a golden game (docs/todo/D4-core-tests.md 4.2) and of the set indexes.
//!
//! A game file is zstd-compressed text. Line 1 is the header `{"hdr": {...}}`.
//!
//! * **full tier** (`gNNN.jsonl.zst`): then one line per decision, the X2 corpus line byte for byte
//!   (`{"s","g","step","k","state","choice","chosen","v1","v2"}`): the rendered `Choice` and both decision hashes, and
//!   the state hash at the start of every step that asked.
//! * **slim tier** (`gNNN.tsv.zst`): then `#S<TAB>step<TAB>state16` at the start of every step `n` with
//!   `n % 8 == 0`, and `step<TAB>k<TAB>chosen<TAB>v2<TAB>subtype-index` per decision (subtypes are interned in the
//!   header, tabs/newlines/backslashes of an id are escaped). A divergence is localised to 8 steps; `golden diff --full`
//!   plays the game to that step to print the offered options.
//!
//! Full states are never stored (hidden information, size): the state hash is the only trace of one.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ti4_selfplay::spec::GameSpec;

use super::corpus::{DecMeta, Played, hex_of};

/// Steps between the state checkpoints of the slim tier.
pub const SLIM_STRIDE: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Full,
    Slim,
}

impl Tier {
    #[must_use]
    pub fn ext(self) -> &'static str {
        match self {
            Tier::Full => "jsonl.zst",
            Tier::Slim => "tsv.zst",
        }
    }

    /// # Errors
    /// A name that is neither `full` nor `slim`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "full" => Ok(Tier::Full),
            "slim" => Ok(Tier::Slim),
            other => Err(format!("tier `{other}`: want full or slim")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_rounds: u32,
    pub max_steps: usize,
}

/// The first line of a game file: everything needed to play the game again and to know what it should do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    /// The game at its opening; `None` for a static source (the Choice objects of the wire fixtures: nothing to play).
    /// `spec.rules_version` is always `Some` (T7: the tool writes the number even when the config did not pin it).
    pub spec: Option<GameSpec>,
    pub rules_version: Option<u32>,
    /// The rule bot's `key=value` overrides the game was recorded with (`bless` plays with exactly these).
    pub bot: BTreeMap<String, String>,
    pub limits: Limits,
    /// Steps played; a replay plays exactly this many.
    pub steps: usize,
    pub decisions: usize,
    /// `host_session::decisions_hash` of the decision log (the bench's per-game hash).
    pub decisions_hash: String,
    pub finished: bool,
    /// Hash of the state after the last step; absent in the games imported from the X2 corpus (it recorded none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_state: Option<String>,
    /// Share of the steps whose arm the core ran without a lend, in per mille (informational before D2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pure_milli: Option<u32>,
    /// Slim tier: the interned subtypes the decision lines index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtypes: Option<Vec<String>>,
    /// The X2 source and the game number in it, for an imported game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<usize>,
}

#[derive(Serialize, Deserialize)]
struct HdrLine {
    hdr: Header,
}

/// One recorded decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dec {
    pub step: usize,
    pub k: usize,
    pub chosen: String,
    pub v2: String,
    pub subtype: String,
    /// Full tier: the state hash at the start of the step. Slim tier: `None` (see `GameFile::checkpoints`).
    pub state: Option<String>,
    /// Full tier: the original line, written back byte for byte.
    pub raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameFile {
    pub tier: Tier,
    pub header: Header,
    pub decs: Vec<Dec>,
    /// Slim tier: the `#S` checkpoints.
    pub checkpoints: Vec<(usize, String)>,
}

impl GameFile {
    /// The state hashes to compare, `(step, hash at the start of that step)`, ascending by step.
    #[must_use]
    pub fn state_checks(&self) -> Vec<(usize, &str)> {
        match self.tier {
            Tier::Slim => self.checkpoints.iter().map(|(s, h)| (*s, h.as_str())).collect(),
            Tier::Full => {
                let mut out: Vec<(usize, &str)> = Vec::new();
                for d in &self.decs {
                    if let Some(h) = d.state.as_deref()
                        && out.last().is_none_or(|(s, _)| *s != d.step)
                    {
                        out.push((d.step, h));
                    }
                }
                out
            }
        }
    }

    /// A game recorded by `corpus::play`, in `tier`.
    #[must_use]
    pub fn from_played(tier: Tier, mut header: Header, played: Played) -> Self {
        header.steps = played.steps;
        header.decisions = played.meta.len();
        header.decisions_hash = played.game_hash;
        header.finished = played.finished;
        header.end_state = Some(played.end_state);
        header.rules_version = Some(played.rules_version);
        if let Some(spec) = header.spec.as_mut() {
            spec.rules_version = Some(played.rules_version);
        }
        match tier {
            Tier::Full => {
                header.subtypes = None;
                let decs = played
                    .meta
                    .into_iter()
                    .zip(played.lines)
                    .map(|(m, raw)| {
                        let state = state_of_line(&raw);
                        Dec { step: m.step, k: m.k, chosen: m.chosen, v2: m.v2, subtype: m.subtype, state, raw: Some(raw) }
                    })
                    .collect();
                GameFile { tier, header, decs, checkpoints: Vec::new() }
            }
            Tier::Slim => {
                let decs = slim_decs(played.meta);
                header.subtypes = Some(intern_list(&decs));
                GameFile { tier, header, decs, checkpoints: played.checkpoints }
            }
        }
    }

    /// The text of the file (before compression).
    ///
    /// # Errors
    /// A full-tier decision without its original line.
    pub fn to_text(&self) -> Result<String, String> {
        let mut out = serde_json::to_string(&HdrLine { hdr: self.header.clone() }).map_err(|e| e.to_string())?;
        out.push('\n');
        match self.tier {
            Tier::Full => {
                for d in &self.decs {
                    out.push_str(d.raw.as_deref().ok_or("full-tier decision without its line")?);
                    out.push('\n');
                }
            }
            Tier::Slim => {
                let subtypes = self.header.subtypes.clone().unwrap_or_default();
                let mut cps = self.checkpoints.iter().peekable();
                let mut last_step = None;
                for d in &self.decs {
                    if last_step != Some(d.step) {
                        while let Some((s, h)) = cps.next_if(|(s, _)| *s <= d.step) {
                            out.push_str(&format!("#S\t{s}\t{h}\n"));
                        }
                        last_step = Some(d.step);
                    }
                    let idx = subtypes.iter().position(|s| *s == d.subtype).ok_or("subtype not interned")?;
                    out.push_str(&format!("{}\t{}\t{}\t{}\t{idx}\n", d.step, d.k, escape(&d.chosen), d.v2));
                }
                for (s, h) in cps {
                    out.push_str(&format!("#S\t{s}\t{h}\n"));
                }
            }
        }
        Ok(out)
    }

    /// # Errors
    /// A malformed file.
    pub fn from_text(tier: Tier, text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let first = lines.next().ok_or("empty game file")?;
        let header = serde_json::from_str::<HdrLine>(first).map_err(|e| format!("header: {e}"))?.hdr;
        let mut decs = Vec::new();
        let mut checkpoints = Vec::new();
        match tier {
            Tier::Full => {
                for (i, l) in lines.enumerate() {
                    decs.push(dec_of_full_line(l).map_err(|e| format!("line {}: {e}", i + 2))?);
                }
            }
            Tier::Slim => {
                let subtypes = header.subtypes.clone().unwrap_or_default();
                for (i, l) in lines.enumerate() {
                    let at = |m: &str| format!("line {}: {m}", i + 2);
                    let cols: Vec<&str> = l.split('\t').collect();
                    if cols.first() == Some(&"#S") {
                        if cols.len() != 3 {
                            return Err(at("checkpoint needs 3 columns"));
                        }
                        checkpoints.push((cols[1].parse().map_err(|_| at("step"))?, cols[2].to_owned()));
                    } else {
                        if cols.len() != 5 {
                            return Err(at("decision needs 5 columns"));
                        }
                        let idx: usize = cols[4].parse().map_err(|_| at("subtype index"))?;
                        decs.push(Dec {
                            step: cols[0].parse().map_err(|_| at("step"))?,
                            k: cols[1].parse().map_err(|_| at("k"))?,
                            chosen: unescape(cols[2]),
                            v2: cols[3].to_owned(),
                            subtype: subtypes.get(idx).ok_or_else(|| at("subtype index out of range"))?.clone(),
                            state: None,
                            raw: None,
                        });
                    }
                }
            }
        }
        Ok(GameFile { tier, header, decs, checkpoints })
    }

    /// Write `dir/name` (`name` without extension) compressed; returns `(file name, sha256 of the text, compressed bytes)`.
    ///
    /// # Errors
    /// An unwritable path.
    pub fn write(&self, dir: &Path, name: &str) -> Result<(String, String, u64), String> {
        let text = self.to_text()?;
        let file = format!("{name}.{}", self.tier.ext());
        let path = dir.join(&file);
        let f = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut enc = zstd::Encoder::new(f, 19).map_err(|e| e.to_string())?;
        enc.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        enc.finish().map_err(|e| e.to_string())?;
        let bytes = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        Ok((file, hex_of(&Sha256::digest(text.as_bytes())), bytes))
    }

    /// Read a game file; returns the file and the sha256 of its text.
    ///
    /// # Errors
    /// An unreadable or malformed file.
    pub fn read(path: &Path) -> Result<(Self, String), String> {
        let tier = if path.to_string_lossy().ends_with(Tier::Slim.ext()) { Tier::Slim } else { Tier::Full };
        let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let dec = zstd::Decoder::new(f).map_err(|e| e.to_string())?;
        let mut text = String::new();
        for l in BufReader::new(dec).lines() {
            text.push_str(&l.map_err(|e| format!("{}: {e}", path.display()))?);
            text.push('\n');
        }
        let sha = hex_of(&Sha256::digest(text.as_bytes()));
        Ok((Self::from_text(tier, &text).map_err(|e| format!("{}: {e}", path.display()))?, sha))
    }
}

fn slim_decs(meta: Vec<DecMeta>) -> Vec<Dec> {
    meta.into_iter()
        .map(|m| Dec { step: m.step, k: m.k, chosen: m.chosen, v2: m.v2, subtype: m.subtype, state: None, raw: None })
        .collect()
}

fn intern_list(decs: &[Dec]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for d in decs {
        if !seen.contains(&d.subtype) {
            seen.push(d.subtype.clone());
        }
    }
    seen
}

#[derive(Deserialize)]
struct FullRead {
    step: usize,
    k: usize,
    state: String,
    chosen: String,
    v2: String,
    #[serde(default)]
    choice: Option<ChoiceRead>,
}
#[derive(Deserialize)]
struct ChoiceRead {
    #[serde(default)]
    context: Option<CtxRead>,
}
#[derive(Deserialize)]
struct CtxRead {
    #[serde(default)]
    subtype: String,
}

pub fn dec_of_full_line(l: &str) -> Result<Dec, String> {
    let r: FullRead = serde_json::from_str(l).map_err(|e| e.to_string())?;
    let subtype = r.choice.and_then(|c| c.context).map(|c| c.subtype).unwrap_or_default();
    Ok(Dec { step: r.step, k: r.k, chosen: r.chosen, v2: r.v2, subtype, state: Some(r.state), raw: Some(l.to_owned()) })
}

fn state_of_line(l: &str) -> Option<String> {
    dec_of_full_line(l).ok().and_then(|d| d.state)
}

/// Escape tab, newline and backslash of an option id (ids are text; a tab must not split a column).
#[must_use]
pub fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

#[must_use]
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Header with the fields a test or an importer does not care about.
#[must_use]
pub fn blank_header(spec: Option<GameSpec>) -> Header {
    let rules_version = spec.as_ref().and_then(|s| s.rules_version);
    Header {
        spec,
        rules_version,
        bot: BTreeMap::new(),
        limits: Limits { max_rounds: 50, max_steps: 400_000 },
        steps: 0,
        decisions: 0,
        decisions_hash: String::new(),
        finished: false,
        end_state: None,
        pure_milli: None,
        subtypes: None,
        source: None,
        game: None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn sample_spec() -> GameSpec {
        GameSpec {
            setup: "t".into(),
            seed: 3,
            players: 3,
            map: "3pLearnToPlayPoK".into(),
            factions: vec!["a".into(), "b".into(), "c".into()],
            speaker: "seat".into(),
            card_set: None,
            rotation: None,
            starting_tech: true,
            thunders_edge: false,
            rules_version: Some(23),
        }
    }

    fn slim_sample() -> GameFile {
        let mut header = blank_header(Some(sample_spec()));
        header.steps = 20;
        header.decisions = 3;
        header.decisions_hash = "abc".into();
        header.end_state = Some("0123456789abcdef".into());
        header.subtypes = Some(vec!["strategy.pick".into(), "tactical.move".into()]);
        let dec = |step, k, chosen: &str, sub: &str| Dec {
            step,
            k,
            chosen: chosen.into(),
            v2: "aaaabbbbccccdddd".into(),
            subtype: sub.into(),
            state: None,
            raw: None,
        };
        GameFile {
            tier: Tier::Slim,
            header,
            decs: vec![dec(0, 0, "pick:1", "strategy.pick"), dec(0, 1, "weird\tid\\x\n", "tactical.move"), dec(9, 0, "pick:2", "strategy.pick")],
            checkpoints: vec![(0, "1111111111111111".into()), (8, "2222222222222222".into()), (16, "3333333333333333".into())],
        }
    }

    #[test]
    fn slim_round_trips_through_text_and_zstd() {
        let g = slim_sample();
        let text = g.to_text().unwrap();
        assert_eq!(GameFile::from_text(Tier::Slim, &text).unwrap(), g);
        // the checkpoint of step 8 sits before the first decision at step 9, the one of 16 after the last decision
        let order: Vec<&str> = text.lines().skip(1).map(|l| l.split('\t').next().unwrap()).collect();
        assert_eq!(order, ["#S", "0", "0", "#S", "9", "#S"]);
        let dir = scratch("slim_zst");
        let (file, sha, bytes) = g.write(&dir, "g000").unwrap();
        assert_eq!(file, "g000.tsv.zst");
        assert!(bytes > 0);
        let (back, sha2) = GameFile::read(&dir.join(&file)).unwrap();
        assert_eq!((back, sha2), (g, sha));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn full_tier_keeps_the_lines_byte_identical() {
        let l0 = r#"{"s":"x","g":4,"step":3,"k":0,"state":"00aa00aa00aa00aa","choice":{"player":"a","prompt":"p","options":[],"context":{"subtype":"score_public_objective"}},"chosen":"c1","v1":"1111111111111111","v2":"2222222222222222"}"#;
        let l1 = l0.replace("\"k\":0", "\"k\":1").replace("c1", "c2");
        let mut header = blank_header(Some(sample_spec()));
        header.decisions = 2;
        let text = format!("{}\n{l0}\n{l1}\n", serde_json::to_string(&HdrLine { hdr: header }).unwrap());
        let g = GameFile::from_text(Tier::Full, &text).unwrap();
        assert_eq!(g.decs[0].subtype, "score_public_objective");
        assert_eq!(g.decs[1].chosen, "c2");
        assert_eq!(g.to_text().unwrap(), text);
        assert_eq!(g.state_checks(), vec![(3, "00aa00aa00aa00aa")]);
    }

    #[test]
    fn header_refuses_unknown_fields() {
        let text = "{\"hdr\":{\"spec\":null,\"rules_version\":null,\"bot\":{},\"limits\":{\"max_rounds\":1,\"max_steps\":1},\"steps\":0,\"decisions\":0,\"decisions_hash\":\"\",\"finished\":false,\"surprise\":1}}\n";
        assert!(GameFile::from_text(Tier::Slim, text).is_err());
    }

    #[test]
    fn escape_round_trips() {
        for s in ["", "a", "a\tb", "a\\tb", "x\ny\\", "\\\\"] {
            assert_eq!(unescape(&escape(s)), s);
        }
    }

    pub(crate) fn scratch(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("golden-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
