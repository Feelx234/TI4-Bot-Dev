//! `cargo xtask gen-cards`: the content generator (docs/CONTENT_GENERATOR.md).
//!
//! One run reads `crates/ti4-content/content/*.json` and the frozen names files and produces
//!
//! * the core output (`crates/ti4-engine-core/src/generated/`): per-category id enums, value
//!   enums and typed gameplay tables, with no strings at all;
//! * the adapter output (`crates/ti4-engine/src/generated/`): id strings, value strings and the
//!   display text, plus `text_parsed_fields.md`, the report of the rules decided by parsing text;
//! * the names files (`crates/ti4-engine-core/names/<category>.toml`), with entries appended for
//!   new ids.
//!
//! Nothing is written in `--check` mode: every file is produced in memory and compared.

pub mod emit;
pub mod naming;
pub mod report;
pub mod spec;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};

use naming::{NameFile, NameInput};
use spec::{Cat, Conv, Norm, Shape, VALUE_ENUMS};

/// Where the generator reads and writes. Every path is a parameter; [`Config::for_repo`] gives
/// the defaults.
#[derive(Debug, Clone)]
pub struct Config {
    pub content_dir: PathBuf,
    pub core_out: PathBuf,
    pub adapter_out: PathBuf,
    pub names_dir: PathBuf,
    /// The Rust path the adapter output uses to reach the core output.
    pub core_path: String,
    /// The categories to generate (`None` = all of [`spec::categories`]).
    pub only: Option<Vec<String>>,
}

impl Config {
    /// The defaults, relative to the workspace root.
    #[must_use]
    pub fn for_repo(root: &Path) -> Self {
        Self {
            content_dir: root.join("crates/ti4-content/content"),
            core_out: root.join("crates/ti4-engine-core/src/generated"),
            adapter_out: root.join("crates/ti4-engine/src/generated"),
            names_dir: root.join("crates/ti4-engine-core/names"),
            core_path: "ti4_engine_core::generated".to_owned(),
            // Stage 1 (cards); widened to every category once the extension landed.
            only: Some(spec::CARD_KEYS.iter().map(|k| (*k).to_owned()).collect()),
        }
    }

    /// The workspace root of this checkout (two levels above the xtask crate).
    #[must_use]
    pub fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crates/xtask sits two levels below the workspace root")
            .to_path_buf()
    }
}

/// One content record: its id and its fields.
#[derive(Debug, Clone)]
pub struct Rec {
    pub id: String,
    pub fields: Map<String, Value>,
}

/// Everything a column conversion may look at.
pub struct Ctx {
    records: BTreeMap<&'static str, Vec<Rec>>,
    /// id -> variant per category key.
    names: BTreeMap<&'static str, BTreeMap<String, String>>,
    types: BTreeMap<&'static str, &'static str>,
    /// key -> variant per value enum, after collection.
    values: BTreeMap<&'static str, BTreeMap<String, String>>,
    /// Pass 1: keys seen per value enum.
    collecting: RefCell<Option<BTreeMap<&'static str, BTreeSet<String>>>>,
}

impl Ctx {
    /// The records of a generated category, in id order.
    #[must_use]
    pub fn records(&self, key: &str) -> &[Rec] {
        self.records.get(key).map_or(&[], Vec::as_slice)
    }

    /// Whether a generated category has this id.
    #[must_use]
    pub fn has(&self, cat: &str, id: &str) -> bool {
        self.names.get(cat).is_some_and(|n| n.contains_key(id))
    }

    /// `Type::Variant` of an id of a generated category.
    ///
    /// # Errors
    /// The category is not generated or has no such id.
    pub fn variant(&self, cat: &str, id: &str) -> Result<String> {
        let names = self
            .names
            .get(cat)
            .ok_or_else(|| anyhow!("references to `{cat}` need that category generated"))?;
        let name = names
            .get(id)
            .ok_or_else(|| anyhow!("no {cat} record has the id {id:?}"))?;
        Ok(format!("{}::{name}", self.types[cat]))
    }

    /// `Enum::Variant` of a value-enum key.
    ///
    /// # Errors
    /// Unknown value enum or key.
    pub fn value(&self, enm: &'static str, key: &str) -> Result<String> {
        if let Some(seen) = self.collecting.borrow_mut().as_mut() {
            seen.entry(enm).or_default().insert(key.to_owned());
            return Ok(format!("{enm}::Collecting"));
        }
        let variants = self
            .values
            .get(enm)
            .ok_or_else(|| anyhow!("unknown value enum {enm}"))?;
        let name = variants
            .get(key)
            .ok_or_else(|| anyhow!("{enm} has no key {key:?}"))?;
        Ok(format!("{enm}::{name}"))
    }

    /// `&[Enum::A, ...]` of a list of raw strings through `norm`.
    ///
    /// # Errors
    /// As [`Self::value`], or a failing `norm`.
    pub fn value_list(&self, enm: &'static str, raw: &[String], norm: Norm) -> Result<String> {
        let mut items = Vec::new();
        for r in raw {
            if let Some(key) = norm(r)? {
                items.push(self.value(enm, &key)?);
            }
        }
        Ok(format!("&[{}]", items.join(", ")))
    }
}

/// The result of one run, in memory.
#[derive(Debug, Clone, Default)]
pub struct Output {
    /// Every file the run owns, by absolute path.
    pub files: BTreeMap<PathBuf, String>,
    /// The directories whose `*.rs` / `*.md` files are all owned by the run (stray ones are stale).
    pub owned_dirs: Vec<PathBuf>,
    /// Human-readable notes (appended ids, retired ids).
    pub notes: Vec<String>,
}

fn read_category(dir: &Path, cat: &Cat) -> Result<Vec<Rec>> {
    let path = dir.join(format!("{}.json", cat.key));
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let Value::Array(items) = value else {
        bail!("{} is not a JSON array", path.display());
    };
    let mut out = Vec::with_capacity(items.len());
    let mut seen = BTreeSet::new();
    for (index, item) in items.into_iter().enumerate() {
        let Value::Object(fields) = item else {
            bail!("{} record {index} is not an object", path.display());
        };
        let Some(Value::String(id)) = fields.get(cat.id_field) else {
            bail!(
                "{} record {index} has no string `{}`",
                path.display(),
                cat.id_field
            );
        };
        let id = id.clone();
        if !seen.insert(id.clone()) {
            bail!("{}: the id {id:?} appears twice", path.display());
        }
        out.push(Rec { id, fields });
    }
    // Byte order of the id: `str`'s `Ord`, which is what BTreeMap iteration and `deck::ids`
    // sort by today. The enum's integer order is this order.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn read_names(cfg: &Config, key: &str) -> Result<NameFile> {
    let path = cfg.names_dir.join(format!("{key}.toml"));
    match std::fs::read_to_string(&path) {
        Ok(text) => NameFile::parse(&text, &path.display().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NameFile::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// The variant name of a value-enum key: CamelCase, all-caps keys title-cased first.
fn value_variant(key: &str) -> String {
    let all_caps = key.chars().any(|c| c.is_ascii_alphabetic())
        && !key.chars().any(|c| c.is_ascii_lowercase());
    let source = if all_caps {
        key.to_ascii_lowercase()
    } else {
        key.to_owned()
    };
    let name = naming::camel(&source);
    if name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        name
    } else {
        format!("N{name}")
    }
}

/// The selected categories.
#[must_use]
pub fn selected(cfg: &Config) -> Vec<Cat> {
    spec::categories()
        .into_iter()
        .filter(|c| {
            cfg.only
                .as_ref()
                .is_none_or(|only| only.iter().any(|k| k == c.key))
        })
        .collect()
}

/// Run the generator in memory.
///
/// # Errors
/// Unreadable input, a malformed record, a dangling reference, a name collision.
pub fn generate(cfg: &Config) -> Result<Output> {
    let cats = selected(cfg);
    let mut out = Output::default();
    let mut records = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut types = BTreeMap::new();
    for cat in &cats {
        let recs = read_category(&cfg.content_dir, cat)?;
        let inputs: Vec<NameInput<'_>> = recs
            .iter()
            .map(|r| NameInput {
                id: &r.id,
                printed: r.fields.get(cat.name_field).and_then(Value::as_str),
            })
            .collect();
        let frozen = read_names(cfg, cat.key)?;
        let resolved = naming::resolve(cat.key, &inputs, &frozen)?;
        if !resolved.added.is_empty() {
            out.notes.push(format!(
                "{}: {} new id(s) named: {}",
                cat.key,
                resolved.added.len(),
                resolved
                    .added
                    .iter()
                    .map(|id| format!("{id} -> {}", resolved.names[id]))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !resolved.retired.is_empty() {
            out.notes.push(format!(
                "{}: ids no longer in the corpus keep their names reserved: {}",
                cat.key,
                resolved.retired.join(", ")
            ));
        }
        out.files.insert(
            cfg.names_dir.join(format!("{}.toml", cat.key)),
            resolved.file.render(cat.key),
        );
        records.insert(cat.key, recs);
        names.insert(cat.key, resolved.names);
        types.insert(cat.key, cat.ty);
    }

    let mut ctx = Ctx {
        records,
        names,
        types,
        values: BTreeMap::new(),
        collecting: RefCell::new(Some(BTreeMap::new())),
    };

    // Pass 1: collect the keys of every value enum by running every conversion.
    rows(&ctx, &cats)?;
    let mut seen = ctx.collecting.borrow_mut().take().unwrap_or_default();
    for enm in VALUE_ENUMS {
        let keys = seen.entry(enm.name).or_default();
        keys.extend(enm.seed.iter().map(|s| (*s).to_owned()));
        let mut variants = BTreeMap::new();
        let mut by_variant: BTreeMap<String, &str> = BTreeMap::new();
        for key in keys.iter() {
            let variant = value_variant(key);
            if let Some(other) = by_variant.insert(variant.clone(), key) {
                bail!(
                    "value enum {}: keys {other:?} and {key:?} both name {variant}",
                    enm.name
                );
            }
            variants.insert(key.clone(), variant);
        }
        ctx.values.insert(enm.name, variants);
    }

    // Pass 2: the real expressions.
    let rows = rows(&ctx, &cats)?;
    let faction_tags = cats.iter().any(|c| c.key == "factions");

    let core = &cfg.core_out;
    let adapter = &cfg.adapter_out;
    out.files.insert(core.join("mod.rs"), emit::core_mod(&cats));
    out.files.insert(
        core.join("values.rs"),
        emit::core_values(&ctx, faction_tags),
    );
    out.files.insert(
        adapter.join("mod.rs"),
        emit::adapter_mod(&cats, &cfg.core_path),
    );
    out.files.insert(
        adapter.join("values.rs"),
        emit::adapter_values(&ctx, &cfg.core_path),
    );
    for (cat, cat_rows) in cats.iter().zip(&rows) {
        out.files.insert(
            core.join(format!("{}.rs", cat.key)),
            emit::core_category(cat, &ctx, cat_rows),
        );
        out.files.insert(
            adapter.join(format!("{}.rs", cat.key)),
            emit::adapter_category(cat, &ctx, &cfg.core_path)?,
        );
    }
    out.files.insert(
        adapter.join("text_parsed_fields.md"),
        report::text_parsed(&ctx, &cats),
    );
    out.owned_dirs = vec![core.clone(), adapter.clone()];
    Ok(out)
}

/// The expressions of every column of every record, per category.
fn rows(ctx: &Ctx, cats: &[Cat]) -> Result<Vec<Vec<Vec<String>>>> {
    cats.iter()
        .map(|cat| {
            ctx.records(cat.key)
                .iter()
                .map(|rec| {
                    cat.columns
                        .iter()
                        .map(|col| {
                            convert(rec, &col.conv, ctx).with_context(|| {
                                format!("{} {:?}, column `{}`", cat.key, rec.id, col.name)
                            })
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn raw_strings(rec: &Rec, field: &str, shape: Shape) -> Result<Vec<String>> {
    match (rec.fields.get(field), shape) {
        (None | Some(Value::Null), Shape::One) => bail!("`{field}` is missing"),
        (None | Some(Value::Null), _) => Ok(Vec::new()),
        (Some(Value::String(s)), Shape::Opt) if s.is_empty() => Ok(Vec::new()),
        (Some(Value::String(s)), Shape::One | Shape::Opt) => Ok(vec![s.clone()]),
        (Some(Value::Array(items)), Shape::List) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow!("`{field}` holds {v}"))
            })
            .collect(),
        (Some(other), _) => bail!("`{field}` = {other} does not have the expected shape"),
    }
}

fn convert(rec: &Rec, conv: &Conv, ctx: &Ctx) -> Result<String> {
    Ok(match conv {
        Conv::Flag(field) => match rec.fields.get(*field) {
            None | Some(Value::Null) => "false".to_owned(),
            Some(Value::Bool(b)) => b.to_string(),
            Some(other) => bail!("`{field}` = {other} is not a boolean"),
        },
        Conv::Int {
            field,
            ty,
            required,
        } => {
            let value = match rec.fields.get(*field) {
                None | Some(Value::Null) => None,
                Some(Value::Number(n)) => Some(
                    n.as_i64()
                        .ok_or_else(|| anyhow!("`{field}` = {n} is not an integer"))?,
                ),
                Some(Value::String(s)) if !s.trim().starts_with('+') => Some(
                    s.trim()
                        .parse::<i64>()
                        .map_err(|_| anyhow!("`{field}` = {s:?} is not an integer"))?,
                ),
                Some(other) => bail!("`{field}` = {other} is not an integer"),
            };
            let (lo, hi) = match *ty {
                "u8" => (0, 255),
                "i8" => (-128, 127),
                other => bail!("unsupported integer type {other}"),
            };
            if let Some(v) = value
                && !(lo..=hi).contains(&v)
            {
                bail!("`{field}` = {v} does not fit {ty}");
            }
            match (value, required) {
                (Some(v), true) => v.to_string(),
                (None, true) => bail!("`{field}` is missing"),
                (Some(v), false) => format!("Some({v})"),
                (None, false) => "None".to_owned(),
            }
        }
        Conv::Value {
            field,
            enm,
            norm,
            shape,
        } => {
            let raw = raw_strings(rec, field, *shape)?;
            match shape {
                Shape::List => ctx.value_list(enm, &raw, *norm)?,
                Shape::One | Shape::Opt => {
                    let key = match raw.first() {
                        Some(r) => norm(r)?,
                        None => None,
                    };
                    match (key, shape) {
                        (Some(k), Shape::One) => ctx.value(enm, &k)?,
                        (Some(k), _) => format!("Some({})", ctx.value(enm, &k)?),
                        (None, Shape::One) => bail!("`{field}` normalizes to nothing"),
                        (None, _) => "None".to_owned(),
                    }
                }
            }
        }
        Conv::Ref { field, cat, shape } => {
            let raw = raw_strings(rec, field, *shape)?;
            let items: Vec<String> = raw
                .iter()
                .map(|id| ctx.variant(cat, id))
                .collect::<Result<_>>()?;
            match shape {
                Shape::List => format!("&[{}]", items.join(", ")),
                Shape::One => items
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("`{field}` is missing"))?,
                Shape::Opt => items
                    .into_iter()
                    .next()
                    .map_or_else(|| "None".to_owned(), |v| format!("Some({v})")),
            }
        }
        Conv::Custom { f, .. } => f(rec, ctx)?,
    })
}

/// Write every file of a run, and remove stray generated files in the owned directories.
///
/// # Errors
/// I/O.
pub fn write(out: &Output) -> Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    for (path, text) in &out.files {
        if std::fs::read_to_string(path).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))?;
        changed.push(path.clone());
    }
    for stray in stray_files(out) {
        std::fs::remove_file(&stray).with_context(|| format!("removing {}", stray.display()))?;
        changed.push(stray);
    }
    Ok(changed)
}

fn stray_files(out: &Output) -> Vec<PathBuf> {
    let mut stray = Vec::new();
    for dir in &out.owned_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let generated_kind = path.extension().is_some_and(|e| e == "rs" || e == "md");
            if generated_kind && !out.files.contains_key(&path) {
                stray.push(path);
            }
        }
    }
    stray.sort();
    stray
}

/// The files that differ from (or are missing on) disk, and stray files.
#[must_use]
pub fn stale(out: &Output) -> Vec<PathBuf> {
    let mut stale: Vec<PathBuf> = out
        .files
        .iter()
        .filter(|(path, text)| std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str()))
        .map(|(path, _)| path.clone())
        .collect();
    stale.extend(stray_files(out));
    stale
}

/// `--check`: regenerate in memory and fail if anything on disk is stale.
///
/// # Errors
/// A generator error, or the list of stale files.
pub fn check(cfg: &Config) -> Result<()> {
    let out = generate(cfg)?;
    let stale = stale(&out);
    if stale.is_empty() {
        return Ok(());
    }
    bail!(
        "generated content files are stale (run `cargo xtask gen-cards`):\n  {}",
        stale
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    )
}
