//! The committed printed-value -> variant tables of the five card text fields.
//!
//! `crates/ti4-content/card_fields.json` (docs/CARD_TEXT_FIELDS.md) lists, for the action-card
//! `window`, the agenda `target`, the leader `abilityWindow` and the strategy-card `id`, every
//! printed value the corpus carries with the variant it maps to, and, for the technology `text`,
//! the ids that cannot be researched. The generator reads the file and
//!
//! * writes the core enums (`Timing`, `AgendaElects`, `LeaderWindow`, `StrategyCardKind`) and the
//!   columns that use them, the adapter's printed-value tables, and the table data of
//!   `ti4-content/src/card_fields.rs` (`card_fields_tables.rs`), so no table is spelled twice;
//! * fails (`gen-cards` and `--check`) on a printed value the corpus has and the file does not, on a
//!   row the corpus no longer prints, and on a technology text that mentions "cannot be researched"
//!   in a form the rule does not know.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// One section: the corpus field it reads and the `(printed, variant)` rows.
#[derive(Debug, Clone)]
pub struct Table {
    pub field: String,
    pub rows: Vec<(String, String)>,
}

impl Table {
    /// The distinct variants, sorted by name.
    #[must_use]
    pub fn variants_by_name(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|(_, v)| v.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Whether every variant has exactly one printed value.
    #[must_use]
    pub fn is_one_to_one(&self) -> bool {
        self.variants_by_name().len() == self.rows.len()
    }

    /// The variants in integer order, with the key each is known by in the adapter: one printed
    /// value per variant -> the rows in file order (which must be in byte order) keyed by the
    /// printed value; else the variants sorted by name, keyed by the name.
    #[must_use]
    pub fn enum_layout(&self) -> Vec<(String, String)> {
        if self.is_one_to_one() {
            self.rows
                .iter()
                .map(|(p, v)| (v.clone(), p.clone()))
                .collect()
        } else {
            self.variants_by_name()
                .into_iter()
                .map(|v| (v.clone(), v))
                .collect()
        }
    }
}

/// The parsed data file.
#[derive(Debug, Clone)]
pub struct CardFields {
    pub action_card_window: Table,
    pub agenda_target: Table,
    pub leader_ability_window: Table,
    pub strategy_card_id: Table,
    /// The sentence a card prints, on a line of its own, when it cannot be researched.
    pub marker: String,
    pub not_researchable: BTreeSet<String>,
}

/// Which value enum a section feeds, and the category it reads.
pub const SECTIONS: &[(&str, &str, &str)] = &[
    // (value enum, category key, section name)
    ("Timing", "action_cards", "action_card_window"),
    ("AgendaElects", "agendas", "agenda_target"),
    ("LeaderWindow", "leaders", "leader_ability_window"),
    ("StrategyCardKind", "strategy_cards", "strategy_card_id"),
];

fn is_variant_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

fn parse_table(root: &Value, name: &str) -> Result<Table> {
    let section = root
        .get(name)
        .with_context(|| format!("card_fields.json has no section `{name}`"))?;
    let field = section
        .get("field")
        .and_then(Value::as_str)
        .with_context(|| format!("`{name}` has no `field`"))?
        .to_owned();
    let rows = section
        .get("rows")
        .and_then(Value::as_array)
        .with_context(|| format!("`{name}` has no `rows`"))?;
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, row) in rows.iter().enumerate() {
        let pair = row
            .as_array()
            .filter(|p| p.len() == 2)
            .with_context(|| format!("`{name}` row {i} is not [printed, variant]"))?;
        let (Some(printed), Some(variant)) = (pair[0].as_str(), pair[1].as_str()) else {
            bail!("`{name}` row {i} is not two strings");
        };
        if printed != printed.trim() {
            bail!(
                "`{name}` row {i}: {printed:?} has surrounding whitespace (values compare trimmed)"
            );
        }
        if !is_variant_name(variant) {
            bail!("`{name}` row {i}: {variant:?} is not a CamelCase variant name");
        }
        if !seen.insert(printed.to_owned()) {
            bail!("`{name}`: the value {printed:?} is listed twice");
        }
        out.push((printed.to_owned(), variant.to_owned()));
    }
    let table = Table { field, rows: out };
    if table.is_one_to_one() {
        // The variants are numbered in file order, which must be the byte order of the printed
        // value so that `Ord` of the enum is the order of the strings it replaces.
        let printed: Vec<&str> = table.rows.iter().map(|(p, _)| p.as_str()).collect();
        let mut sorted = printed.clone();
        sorted.sort_unstable();
        if printed != sorted {
            bail!(
                "`{name}`: one variant per value, so the rows must be in byte order of the printed value"
            );
        }
    }
    Ok(table)
}

impl CardFields {
    /// Parse the data file.
    ///
    /// # Errors
    /// Malformed JSON or a malformed row.
    pub fn parse(text: &str) -> Result<Self> {
        let root: Value = serde_json::from_str(text).context("parsing card_fields.json")?;
        let tech = root
            .get("technology_text")
            .context("card_fields.json has no section `technology_text`")?;
        let marker = tech
            .get("marker")
            .and_then(Value::as_str)
            .context("`technology_text` has no `marker`")?
            .to_owned();
        let not_researchable = tech
            .get("not_researchable")
            .and_then(Value::as_array)
            .context("`technology_text` has no `not_researchable`")?
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<BTreeSet<_>>>()
            .context("`not_researchable` holds a non-string")?;
        Ok(Self {
            action_card_window: parse_table(&root, "action_card_window")?,
            agenda_target: parse_table(&root, "agenda_target")?,
            leader_ability_window: parse_table(&root, "leader_ability_window")?,
            strategy_card_id: parse_table(&root, "strategy_card_id")?,
            marker,
            not_researchable,
        })
    }

    /// The table that feeds a value enum.
    #[must_use]
    pub fn table(&self, enm: &str) -> Option<&Table> {
        Some(match enm {
            "Timing" => &self.action_card_window,
            "AgendaElects" => &self.agenda_target,
            "LeaderWindow" => &self.leader_ability_window,
            "StrategyCardKind" => &self.strategy_card_id,
            _ => return None,
        })
    }

    /// Whether a technology may be researched, by the rule of `ResearchRestriction::parse`: a text
    /// that does not mention "cannot be researched" (ignoring case) is researchable; a text whose
    /// last line is the marker and that mentions it once is not; any other mention is an error.
    ///
    /// # Errors
    /// A mention in another form (it must be looked at, not guessed).
    pub fn researchable(&self, id: &str, text: &str) -> Result<bool> {
        let lower = text.to_ascii_lowercase();
        if !lower.contains("cannot be researched") {
            return Ok(true);
        }
        if text.lines().last().map(str::trim) == Some(self.marker.as_str())
            && lower.matches("cannot be researched").count() == 1
        {
            return Ok(false);
        }
        bail!(
            "technology {id}: the text mentions \"cannot be researched\" in a form the rule does not know \
             (last line must be exactly {:?}, mentioned once); decide it in card_fields.json / the rule",
            self.marker
        )
    }

    /// The generator's own checks against the corpus: the technology list equals what the rule
    /// finds. (The printed-value tables are checked as the columns are converted.)
    ///
    /// # Errors
    /// A mismatch.
    pub fn check_technologies(&self, texts: &[(String, String)]) -> Result<()> {
        let mut found = BTreeSet::new();
        for (id, text) in texts {
            if !self.researchable(id, text)? {
                found.insert(id.clone());
            }
        }
        if found != self.not_researchable {
            bail!(
                "card_fields.json `technology_text.not_researchable` is {:?} but the corpus texts make {:?} unresearchable",
                self.not_researchable,
                found
            );
        }
        Ok(())
    }
}

/// Group `(printed value)` occurrences by value, for the unmapped / dead row checks.
///
/// # Errors
/// Lists every printed value of the corpus that the table lacks, and every row nobody prints.
pub fn check_table(enm: &str, section: &str, table: &Table, seen: &BTreeSet<String>) -> Result<()> {
    let rows: BTreeMap<&str, &str> = table
        .rows
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    let unmapped: Vec<&String> = seen
        .iter()
        .filter(|s| !rows.contains_key(s.as_str()))
        .collect();
    if !unmapped.is_empty() {
        bail!(
            "{enm}: the corpus prints {} `{}` value(s) that crates/ti4-content/card_fields.json \
             (section `{section}`) does not map: {unmapped:#?}\nadd a row [printed, Variant] for each",
            unmapped.len(),
            table.field
        );
    }
    let dead: Vec<&str> = rows
        .keys()
        .copied()
        .filter(|p| !seen.contains(*p))
        .collect();
    if !dead.is_empty() {
        bail!(
            "{enm}: card_fields.json section `{section}` lists value(s) the corpus does not print: {dead:#?}\n\
             remove the row(s)"
        );
    }
    Ok(())
}

fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

/// `crates/ti4-content/src/card_fields_tables.rs`: the table data `card_fields.rs` includes.
#[must_use]
pub fn content_tables(fields: &CardFields) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    out.push_str(
        "// @generated by `cargo xtask gen-cards` from crates/ti4-content/card_fields.json. DO NOT EDIT.\n\
         // The table data of card_fields.rs: the printed windows of the action cards, and the rows of\n\
         // the value tables of the other fields. To change it: edit the data file and run\n\
         // `cargo xtask gen-cards`; `--check` fails while this file is stale.\n\n",
    );
    out.push_str("action_card_windows! {\n");
    for (printed, variant) in &fields.action_card_window.rows {
        let _ = writeln!(out, "    {variant} = {},", rust_str(printed));
    }
    out.push_str("}\n");
    for (name, ty, table) in [
        ("ELECT_TYPE_ROWS", "ElectType", &fields.agenda_target),
        (
            "LEADER_ABILITY_KIND_ROWS",
            "LeaderAbilityKind",
            &fields.leader_ability_window,
        ),
        (
            "STRATEGY_CARD_KIND_ROWS",
            "StrategyCardKind",
            &fields.strategy_card_id,
        ),
    ] {
        let _ = writeln!(
            out,
            "\n/// Rows of `{ty}::TABLE`: `{}` -> variant.\nconst {name}: &[(&str, {ty})] = &[",
            table.field
        );
        for (printed, variant) in &table.rows {
            let _ = writeln!(out, "    ({}, {ty}::{variant}),", rust_str(printed));
        }
        out.push_str("];\n");
    }
    out
}
