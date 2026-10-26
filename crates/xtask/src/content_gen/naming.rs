//! Readable variant names and the frozen id -> name mapping files.
//!
//! The rules (also in `docs/CONTENT_GENERATOR.md`):
//!
//! 1. A name is derived from the record's printed name (`name`, `factionName` for factions),
//!    falling back to the id when the record has no name or the cleaned name does not start with an
//!    ASCII letter.
//! 2. Cleanup: a few non-ASCII letters are transliterated (`Ω` -> `Omega`, accented Latin letters
//!    -> the bare letter), every other non-ASCII character is dropped; apostrophes are deleted
//!    without splitting the word (`Z'eu` -> `Zeu`); every other non-alphanumeric character
//!    separates words; each word gets an upper-case first letter and keeps the rest as printed
//!    (`Hel-Titan II` -> `HelTitanII`, `X-89 Bacterial Weapon Ω` -> `X89BacterialWeaponOmega`).
//! 3. Records of one category whose derived names are equal form a group, and every member gets a
//!    suffix: (a) the trailing digits of its id when every member has some and they are distinct
//!    (`sabo1`..`sabo4` -> `Sabotage1`..`Sabotage4`); else (b) when one member's id is a prefix of
//!    every other member's id, the rest of the id in CamelCase (`veto`, `veto3` -> `Veto`, `Veto3`;
//!    `xxchahero`, `xxchahero-te` -> `XxekirGrom`, `XxekirGromTe`); else (c) the whole id in
//!    CamelCase (`base2`, `pok2diplomacy` -> `DiplomacyBase2`, `DiplomacyPok2diplomacy`).
//! 4. If the result still does not start with a letter, it is prefixed with `N`.
//! 5. The mapping file is frozen: an id that has an entry keeps it. Only ids without an entry get
//!    a derived name, appended in id order. Editing an entry is a deliberate override.
//! 6. Two ids of one category with the same variant name (including entries of ids that left the
//!    corpus, which keep their name reserved) make the generator fail.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{Result, bail};

/// One record as the namer sees it.
#[derive(Debug, Clone)]
pub struct NameInput<'a> {
    pub id: &'a str,
    pub printed: Option<&'a str>,
}

fn transliterate(c: char, out: &mut String) {
    let mapped = match c {
        'Ω' | 'ω' => "Omega",
        'Α' | 'α' => "Alpha",
        'Β' | 'β' => "Beta",
        'Γ' | 'γ' => "Gamma",
        'Δ' | 'δ' => "Delta",
        'Ε' | 'ε' => "Epsilon",
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'È' | 'É' | 'Ê' | 'Ë' => "E",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' => "I",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => "O",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' => "U",
        'ç' => "c",
        'Ç' => "C",
        'ñ' => "n",
        'Ñ' => "N",
        _ => "",
    };
    // A transliterated Greek letter is its own word: `Weapon Ω` and `WeaponΩ` read the same.
    if mapped.len() > 1 {
        out.push(' ');
        out.push_str(mapped);
        out.push(' ');
    } else {
        out.push_str(mapped);
    }
}

/// CamelCase of a printed text (rule 2). May be empty or start with a digit.
#[must_use]
pub fn camel(text: &str) -> String {
    let mut ascii = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            ascii.push(c);
        } else if c == '\u{2019}' || c == '\u{2018}' {
            ascii.push('\'');
        } else {
            transliterate(c, &mut ascii);
        }
    }
    let mut out = String::with_capacity(ascii.len());
    let mut word_start = true;
    for c in ascii.chars() {
        if c == '\'' {
            continue;
        }
        if c.is_ascii_alphanumeric() {
            if word_start {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
            word_start = false;
        } else {
            word_start = true;
        }
    }
    out
}

fn starts_with_letter(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// The base name of one record before group disambiguation (rules 1, 2).
#[must_use]
pub fn base_name(input: &NameInput<'_>) -> String {
    if let Some(printed) = input.printed {
        let name = camel(printed);
        if starts_with_letter(&name) {
            return name;
        }
    }
    camel(input.id)
}

fn trailing_digits(id: &str) -> &str {
    let start = id
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map_or(id.len(), |(i, _)| i);
    &id[start..]
}

/// Derived names for every input (rules 1-4), keyed by id. Pure: does not look at the frozen file.
#[must_use]
pub fn derive_all(inputs: &[NameInput<'_>]) -> BTreeMap<String, String> {
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for input in inputs {
        groups.entry(base_name(input)).or_default().push(input.id);
    }
    let mut out = BTreeMap::new();
    for (base, mut ids) in groups {
        ids.sort_unstable();
        if ids.len() == 1 {
            out.insert(ids[0].to_owned(), finish(base));
            continue;
        }
        let suffixes = group_suffixes(&ids);
        for (id, suffix) in ids.iter().zip(suffixes) {
            out.insert((*id).to_owned(), finish(format!("{base}{suffix}")));
        }
    }
    out
}

/// Rule 3: the suffixes of a group of ids (sorted) that share a derived name.
fn group_suffixes(ids: &[&str]) -> Vec<String> {
    let distinct = |s: &[String]| s.iter().collect::<BTreeSet<_>>().len() == s.len();
    // (a) Trailing digits, when every member has some and they differ: `sabo1` -> `1`.
    let digits: Vec<String> = ids
        .iter()
        .map(|id| trailing_digits(id).to_owned())
        .collect();
    if digits.iter().all(|d| !d.is_empty()) && distinct(&digits) {
        return digits;
    }
    // (b) One id is a prefix of all others: the rest of each id (`veto3` -> `3`,
    // `xxchahero-te` -> `Te`, `bioticstat` -> `Stat`; the shortest gets no suffix).
    if let Some(stem) = ids
        .iter()
        .find(|stem| ids.iter().all(|id| id.starts_with(**stem)))
    {
        let rest: Vec<String> = ids.iter().map(|id| camel(&id[stem.len()..])).collect();
        if distinct(&rest) {
            return rest;
        }
    }
    // (c) The whole id.
    ids.iter().map(|id| camel(id)).collect()
}

fn finish(name: String) -> String {
    if starts_with_letter(&name) {
        name
    } else {
        format!("N{name}")
    }
}

/// Whether a name may be a variant: `[A-Z][A-Za-z0-9]*`, and not `Self`.
#[must_use]
pub fn is_valid_variant(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
        && name != "Self"
}

/// A parsed mapping file: entries plus the trailing comment each carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameFile {
    pub entries: BTreeMap<String, (String, Option<String>)>,
}

impl NameFile {
    /// Parse the TOML subset this generator writes: `"id" = "Variant"  # optional comment`, blank
    /// lines and `#` comment lines (comment lines are not kept; trailing comments are).
    pub fn parse(text: &str, path: &str) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, rest)) = parse_quoted(trimmed) else {
                bail!("{path}:{}: expected `\"id\" = \"Variant\"`", number + 1);
            };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                bail!("{path}:{}: expected `=` after the id", number + 1);
            };
            let Some((value, rest)) = parse_quoted(rest.trim_start()) else {
                bail!("{path}:{}: expected a quoted variant name", number + 1);
            };
            let rest = rest.trim();
            let comment = if rest.is_empty() {
                None
            } else if let Some(c) = rest.strip_prefix('#') {
                Some(c.trim().to_owned())
            } else {
                bail!("{path}:{}: unexpected text after the entry", number + 1);
            };
            if !is_valid_variant(&value) {
                bail!(
                    "{path}:{}: {value:?} is not a valid variant name ([A-Z][A-Za-z0-9]*)",
                    number + 1
                );
            }
            if entries.insert(key.clone(), (value, comment)).is_some() {
                bail!("{path}:{}: id {key:?} appears twice", number + 1);
            }
        }
        Ok(Self { entries })
    }

    /// The file text: a fixed header, then one entry per id in byte order of the id.
    #[must_use]
    pub fn render(&self, category: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# Frozen id -> variant name mapping for `{category}` (docs/CONTENT_GENERATOR.md)."
        );
        out.push_str(
            "# `cargo xtask gen-cards` appends entries for new ids and never changes an existing one.\n\
             # Editing a name here is a deliberate override (the enum variant is renamed).\n",
        );
        for (id, (name, comment)) in &self.entries {
            let _ = write!(out, "{} = {}", quote(id), quote(name));
            if let Some(comment) = comment {
                let _ = write!(out, "  # {comment}");
            }
            out.push('\n');
        }
        out
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A basic TOML string at the start of `s`, and the rest.
fn parse_quoted(s: &str) -> Option<(String, &str)> {
    let s = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 1..])),
            '\\' => {
                let (_, e) = chars.next()?;
                match e {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    _ => return None,
                }
            }
            c => out.push(c),
        }
    }
    None
}

/// The outcome of applying the frozen file to the current ids.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Variant name of every current id.
    pub names: BTreeMap<String, String>,
    /// The file after appending the new ids.
    pub file: NameFile,
    /// Ids that were appended in this run.
    pub added: Vec<String>,
    /// Ids that have an entry but are no longer in the corpus (their names stay reserved).
    pub retired: Vec<String>,
}

/// Apply rule 5 and check rule 6.
///
/// # Errors
/// A collision, naming the ids and asking for an override.
pub fn resolve(category: &str, inputs: &[NameInput<'_>], frozen: &NameFile) -> Result<Resolved> {
    let derived = derive_all(inputs);
    let mut file = frozen.clone();
    let mut added = Vec::new();
    for (id, name) in &derived {
        if !file.entries.contains_key(id) {
            file.entries.insert(id.clone(), (name.clone(), None));
            added.push(id.clone());
        }
    }
    let current: BTreeSet<&str> = inputs.iter().map(|i| i.id).collect();
    let retired: Vec<String> = file
        .entries
        .keys()
        .filter(|id| !current.contains(id.as_str()))
        .cloned()
        .collect();

    let mut by_name: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, (name, _)) in &file.entries {
        by_name.entry(name.as_str()).or_default().push(id.as_str());
    }
    let clashes: Vec<String> = by_name
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(name, ids)| format!("{name}: {}", ids.join(", ")))
        .collect();
    if !clashes.is_empty() {
        bail!(
            "variant name collision in `{category}`:\n  {}\nadd an override for one of the ids in \
             crates/ti4-engine-core/names/{category}.toml (e.g. `\"<id>\" = \"<OtherName>\"`) and \
             run the generator again",
            clashes.join("\n  ")
        );
    }
    let names = file
        .entries
        .iter()
        .filter(|(id, _)| current.contains(id.as_str()))
        .map(|(id, (name, _))| (id.clone(), name.clone()))
        .collect();
    Ok(Resolved {
        names,
        file,
        added,
        retired,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(id: &'a str, printed: Option<&'a str>) -> NameInput<'a> {
        NameInput { id, printed }
    }

    #[test]
    fn printed_names_become_camel_case() {
        assert_eq!(camel("Z'eu"), "Zeu");
        assert_eq!(camel("Coup d'Etat"), "CoupDEtat");
        assert_eq!(camel("Hel-Titan II"), "HelTitanII");
        assert_eq!(
            camel("X-89 Bacterial Weapon ΩΩ"),
            "X89BacterialWeaponOmegaOmega"
        );
        assert_eq!(camel("I.I.H.Q. Modernization"), "IIHQModernization");
        assert_eq!(camel("Rin, the Master’s Legacy"), "RinTheMastersLegacy");
        assert_eq!(camel("???\\_NULL\\_REFERENCE\\_???"), "NULLREFERENCE");
    }

    #[test]
    fn a_name_that_does_not_start_with_a_letter_falls_back_to_the_id() {
        assert_eq!(base_name(&input("nokarhero", Some("2RAM"))), "Nokarhero");
        assert_eq!(base_name(&input("helios1", None)), "Helios1");
        let all = derive_all(&[input("0.0.0", Some("[0.0.0]"))]);
        assert_eq!(all["0.0.0"], "N000");
    }

    #[test]
    fn copies_get_digit_suffixes_and_other_groups_the_id() {
        let all = derive_all(&[
            input("sabo1", Some("Sabotage")),
            input("sabo2", Some("Sabotage")),
            input("veto", Some("Veto")),
            input("veto3", Some("Veto")),
            input("biotic", Some("Biotic Research Facility")),
            input("bioticstat", Some("Biotic Research Facility")),
            input("base2", Some("Diplomacy")),
            input("pok2diplomacy", Some("Diplomacy")),
        ]);
        assert_eq!(all["sabo1"], "Sabotage1");
        assert_eq!(all["veto"], "Veto");
        assert_eq!(all["veto3"], "Veto3");
        assert_eq!(all["biotic"], "BioticResearchFacility");
        assert_eq!(all["bioticstat"], "BioticResearchFacilityStat");
        assert_eq!(all["base2"], "DiplomacyBase2");
        assert_eq!(all["pok2diplomacy"], "DiplomacyPok2diplomacy");
    }

    #[test]
    fn the_file_round_trips_with_comments() {
        let mut file = NameFile::default();
        file.entries.insert(
            "a\"b".into(),
            ("Ab".into(), Some("override: reason".into())),
        );
        file.entries.insert("c".into(), ("C".into(), None));
        let text = file.render("x");
        assert_eq!(NameFile::parse(&text, "x").unwrap(), file);
    }

    #[test]
    fn frozen_entries_win_and_new_ids_are_appended() {
        let frozen = NameFile::parse("\"sabo1\" = \"Sab\"\n", "t").unwrap();
        let inputs = [
            input("sabo1", Some("Sabotage")),
            input("sabo2", Some("Sabotage")),
        ];
        let resolved = resolve("t", &inputs, &frozen).unwrap();
        assert_eq!(resolved.names["sabo1"], "Sab");
        assert_eq!(resolved.names["sabo2"], "Sabotage2");
        assert_eq!(resolved.added, ["sabo2"]);
        let again = resolve("t", &inputs, &resolved.file).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(again.file, resolved.file);
    }

    #[test]
    fn a_collision_fails_and_asks_for_an_override() {
        let frozen = NameFile::parse("\"a\" = \"Same\"\n", "t").unwrap();
        let err = resolve("t", &[input("a", None), input("b", Some("Same"))], &frozen)
            .unwrap_err()
            .to_string();
        assert!(err.contains("collision"), "{err}");
        assert!(err.contains("Same: a, b"), "{err}");
        assert!(err.contains("override"), "{err}");
    }

    #[test]
    fn a_retired_id_keeps_its_name_reserved() {
        let frozen = NameFile::parse("\"old\" = \"Thing\"\n", "t").unwrap();
        let err = resolve("t", &[input("new", Some("Thing"))], &frozen).unwrap_err();
        assert!(err.to_string().contains("Thing: new, old"));
        let ok = resolve("t", &[input("new", Some("Other"))], &frozen).unwrap();
        assert_eq!(ok.retired, ["old"]);
        assert!(!ok.names.contains_key("old"));
    }
}
