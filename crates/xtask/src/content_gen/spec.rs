//! What is generated: the categories, their typed gameplay columns, their display fields.
//!
//! The gameplay columns are the fields `docs/CARD_USAGE_AUDIT.md` classes as `gameplay` (and the
//! `scope` tag), plus typed forms of the `gameplay-text` fields where the engine's rule is a
//! mechanical function of the text. Everything else a category carries that a reader uses is a
//! display field (adapter only). Fields nothing reads are not generated.

use anyhow::{Result, bail};
use serde_json::Value;

use super::Ctx;
use super::Rec;

/// One generated category.
pub struct Cat {
    /// File stem of `content/<key>.json`, module name and names-file stem.
    pub key: &'static str,
    /// The id enum (`ActionCard`).
    pub ty: &'static str,
    /// SCREAMING_SNAKE prefix of the tables (`ACTION_CARD`).
    pub upper: &'static str,
    /// The identity field.
    pub id_field: &'static str,
    /// The printed name the variant names are derived from.
    pub name_field: &'static str,
    /// One line for the enum's doc comment.
    pub doc: &'static str,
    /// Typed gameplay data (core).
    pub columns: Vec<Col>,
    /// Display fields (adapter), in output order.
    pub display: &'static [&'static str],
}

/// A transform of a JSON string into the key of a value enum. `Ok(None)` drops a list element.
pub type Norm = fn(&str) -> Result<Option<String>>;

/// How a gameplay column is read.
pub enum Conv {
    /// A boolean flag, absent = false.
    Flag(&'static str),
    /// An integer field (number or numeric string); `required` = no `Option`.
    Int {
        field: &'static str,
        ty: &'static str,
        required: bool,
    },
    /// A string (or list of strings) that is the key of a value enum.
    Value {
        field: &'static str,
        enm: &'static str,
        norm: Norm,
        shape: Shape,
    },
    /// An id (or list of ids) of another generated category.
    Ref {
        field: &'static str,
        cat: &'static str,
        shape: Shape,
    },
    /// Anything else: the Rust type and a function producing the expression.
    Custom {
        ty: &'static str,
        f: fn(&Rec, &Ctx) -> Result<String>,
    },
}

/// Required, optional (absent / null / "" = `None`), or list (absent = empty).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    One,
    Opt,
    List,
}

pub struct Col {
    pub name: &'static str,
    pub doc: &'static str,
    pub conv: Conv,
}

const fn col(name: &'static str, doc: &'static str, conv: Conv) -> Col {
    Col { name, doc, conv }
}

/// A value enum: a closed set of keys collected from the corpus.
pub struct ValueEnum {
    pub name: &'static str,
    pub doc: &'static str,
    /// Keys that exist even if no record uses them today.
    pub seed: &'static [&'static str],
}

pub const VALUE_ENUMS: &[ValueEnum] = &[
    ValueEnum {
        name: "Source",
        doc: "The expansion a record belongs to (the `source` tag).",
        seed: &[],
    },
    ValueEnum {
        name: "FactionTag",
        doc: "A faction as card, unit and planet records name it: a faction id, or a family tag such as `keleres` that stands for several factions (see `FactionTag::factions`).",
        seed: &[],
    },
    ValueEnum {
        name: "TechType",
        doc: "A technology colour or `UNITUPGRADE` (technology `types`, breakthrough `synergy`, planet and attachment specialties; upper-cased, `NONE` dropped).",
        seed: &[],
    },
    ValueEnum {
        name: "AgendaType",
        doc: "Agenda `type`.",
        seed: &[],
    },
    ValueEnum {
        name: "AgendaElects",
        doc: "What an agenda elects: its `target` up to the first `(`, without the leading `Elect`; `For/Against` when the text does not start with `Elect` (the rule of `vote::outcomes`).",
        seed: &["For/Against"],
    },
    ValueEnum {
        name: "LeaderType",
        doc: "Leader `type`.",
        seed: &[],
    },
    ValueEnum {
        name: "LeaderWindow",
        doc: "When a leader is used, from its `abilityWindow`: `action` (starts with `ACTION`: a component action that ends the turn), `action_phase` (`During the action phase:`), `other` (the rules of `leaders::is_action_window` / `uses_the_action`).",
        seed: &["action", "action_phase", "other"],
    },
    ValueEnum {
        name: "ObjectivePhase",
        doc: "Secret objective `phase`: when it may be scored.",
        seed: &[],
    },
    ValueEnum {
        name: "ExploreType",
        doc: "Exploration deck (`type` of an exploration card).",
        seed: &[],
    },
    ValueEnum {
        name: "ExploreResolution",
        doc: "Exploration card `resolution`.",
        seed: &[],
    },
    ValueEnum {
        name: "StrategyCardKind",
        doc: "The printed strategy card a record is a printing of (its `name`, on which the engine dispatches).",
        seed: &[],
    },
    ValueEnum {
        name: "PlanetTrait",
        doc: "A planet type as the corpus writes it (`planetType` / `planetTypes`, attachment `planetTypes`; upper-cased). Includes `FACTION`, `FAKE`, `SPACESTATION` and the like, which are not traits in the rules.",
        seed: &[],
    },
    ValueEnum {
        name: "Wormhole",
        doc: "A wormhole kind printed on a tile (`wormholes`).",
        seed: &[],
    },
    ValueEnum {
        name: "TileBack",
        doc: "Tile back colour (`tileBack`).",
        seed: &[],
    },
    ValueEnum {
        name: "UnitBaseType",
        doc: "Unit `baseType`.",
        seed: &[],
    },
];

fn keep(s: &str) -> Result<Option<String>> {
    Ok(Some(s.to_owned()))
}

fn upper(s: &str) -> Result<Option<String>> {
    Ok(Some(s.to_ascii_uppercase()))
}

fn tech_type(s: &str) -> Result<Option<String>> {
    let u = s.to_ascii_uppercase();
    Ok((u != "NONE").then_some(u))
}

/// `vote::outcomes`: the text up to the first `(`, trimmed; `Elect X` elects `X`, anything else is
/// For/Against.
pub fn agenda_elects(target: &str) -> Result<Option<String>> {
    let head = target.split('(').next().unwrap_or(target).trim();
    Ok(Some(match head.strip_prefix("Elect") {
        Some(rest) if !rest.trim().is_empty() => rest.trim().to_owned(),
        _ => "For/Against".to_owned(),
    }))
}

/// `leaders::is_action_window` and `leaders::uses_the_action`.
pub fn leader_window(window: &str) -> Result<Option<String>> {
    let printed = window.trim().trim_end_matches(':').trim_end();
    Ok(Some(
        if window.trim().starts_with("ACTION") {
            "action"
        } else if printed.eq_ignore_ascii_case("during the action phase") {
            "action_phase"
        } else {
            "other"
        }
        .to_owned(),
    ))
}

fn source() -> Col {
    col(
        "source",
        "The expansion of the record (`source`).",
        Conv::Value {
            field: "source",
            enm: "Source",
            norm: keep,
            shape: Shape::One,
        },
    )
}

fn faction_tag(field: &'static str, doc: &'static str, shape: Shape) -> Col {
    col(
        "faction",
        doc,
        Conv::Value {
            field,
            enm: "FactionTag",
            norm: keep,
            shape,
        },
    )
}

fn str_field<'a>(rec: &'a Rec, field: &str) -> Option<&'a str> {
    rec.fields.get(field).and_then(Value::as_str)
}

fn component_action(rec: &Rec, _: &Ctx) -> Result<String> {
    let Some(window) = str_field(rec, "window") else {
        bail!("action card {} has no `window`", rec.id);
    };
    Ok((window.trim() == "Action").to_string())
}

fn researchable(rec: &Rec, _: &Ctx) -> Result<String> {
    let text = str_field(rec, "text").unwrap_or_default();
    Ok((!text.to_ascii_lowercase().contains("cannot be researched")).to_string())
}

fn prerequisites(rec: &Rec, _: &Ctx) -> Result<String> {
    let raw = match rec.fields.get("requirements") {
        None | Some(Value::Null) => "",
        Some(Value::String(s)) => s.as_str(),
        Some(other) => bail!("technology {}: `requirements` is {other}", rec.id),
    };
    let raw = if matches!(raw, "null" | "None") {
        ""
    } else {
        raw
    };
    let (mut b, mut g, mut r, mut y) = (0u8, 0u8, 0u8, 0u8);
    for c in raw.chars() {
        match c {
            'B' => b += 1,
            'G' => g += 1,
            'R' => r += 1,
            'Y' => y += 1,
            other => bail!(
                "technology {}: requirement letter {other:?} is not B, G, R or Y",
                rec.id
            ),
        }
    }
    Ok(format!(
        "Prerequisites {{ blue: {b}, green: {g}, red: {r}, yellow: {y} }}"
    ))
}

fn int_of(rec: &Rec, field: &str) -> Result<Option<i64>> {
    Ok(match rec.fields.get(field) {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => Some(
            n.as_i64()
                .ok_or_else(|| anyhow::anyhow!("{}: `{field}` = {n} is not an integer", rec.id))?,
        ),
        Some(Value::String(s)) => Some(
            s.trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("{}: `{field}` = {s:?} is not an integer", rec.id))?,
        ),
        Some(other) => bail!("{}: `{field}` = {other} is not an integer", rec.id),
    })
}

fn u8_of(rec: &Rec, field: &str) -> Result<Option<u8>> {
    int_of(rec, field)?
        .map(|v| {
            u8::try_from(v).map_err(|_| anyhow::anyhow!("{}: `{field}` = {v} out of u8", rec.id))
        })
        .transpose()
}

/// `spaceCannonHitsOn` + `spaceCannonDieCount` (dice default 1, as `UnitType::space_cannon_dice`).
fn space_cannon(rec: &Rec, _: &Ctx) -> Result<String> {
    let hits = u8_of(rec, "spaceCannonHitsOn")?.filter(|&v| v != 0);
    let dice = u8_of(rec, "spaceCannonDieCount")?;
    Ok(match hits {
        Some(hits_on) => format!(
            "Some(Roll {{ hits_on: {hits_on}, dice: {} }})",
            dice.unwrap_or(1)
        ),
        None => {
            if dice.is_some() {
                bail!("{}: spaceCannonDieCount without spaceCannonHitsOn", rec.id);
            }
            "None".to_owned()
        }
    })
}

/// A unit roll: `<prefix>HitsOn` (0 = none, as `UnitType`) and `<prefix>DieCount` (default given).
fn roll(rec: &Rec, prefix: &str, default_dice: u8) -> Result<String> {
    let hits = u8_of(rec, &format!("{prefix}HitsOn"))?.filter(|&v| v != 0);
    let dice = u8_of(rec, &format!("{prefix}DieCount"))?;
    Ok(match hits {
        Some(hits_on) => format!(
            "Some(Roll {{ hits_on: {hits_on}, dice: {} }})",
            dice.unwrap_or(default_dice)
        ),
        None => "None".to_owned(),
    })
}

fn combat(rec: &Rec, _: &Ctx) -> Result<String> {
    roll(rec, "combat", 0)
}
fn afb(rec: &Rec, _: &Ctx) -> Result<String> {
    roll(rec, "afb", 0)
}
fn bombardment(rec: &Rec, _: &Ctx) -> Result<String> {
    roll(rec, "bombard", 1)
}

fn cost(rec: &Rec, _: &Ctx) -> Result<String> {
    Ok(match rec.fields.get("cost") {
        None | Some(Value::Null) => "None".to_owned(),
        Some(Value::Number(n)) => {
            let v = n
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("{}: cost {n}", rec.id))?;
            #[allow(clippy::cast_possible_truncation)]
            let halves = (v * 2.0).round() as i64;
            #[allow(clippy::cast_precision_loss)]
            if (halves as f64 - v * 2.0).abs() > 1e-9 || !(0..=255).contains(&halves) {
                bail!("{}: cost {v} is not a multiple of 0.5 in 0..=127.5", rec.id);
            }
            format!("Some({halves})")
        }
        Some(other) => bail!("{}: cost {other}", rec.id),
    })
}

/// `productionValue` (`5`, `"5"` or `"+2"`) and `basicProduction: "res"`, as `UnitType::production`.
fn production(rec: &Rec, _: &Ctx) -> Result<String> {
    let Some(raw) = rec.fields.get("productionValue") else {
        return Ok("None".to_owned());
    };
    let text = raw.as_str().map_or_else(|| raw.to_string(), str::to_owned);
    let relative = text.starts_with('+') || str_field(rec, "basicProduction") == Some("res");
    let value: u8 = text
        .trim_start_matches('+')
        .parse()
        .map_err(|_| anyhow::anyhow!("{}: productionValue {text:?}", rec.id))?;
    Ok(format!(
        "Some(Production {{ value: {value}, plus_planet_resources: {relative} }})"
    ))
}

fn legendary(rec: &Rec, _: &Ctx) -> Result<String> {
    Ok(str_field(rec, "legendaryAbilityName").is_some().to_string())
}

/// Planet traits: the single `planetType`, else the `planetTypes` list (`Planet::planet_types`).
fn planet_traits(rec: &Rec, ctx: &Ctx) -> Result<String> {
    let values: Vec<String> = match str_field(rec, "planetType") {
        Some(single) => vec![single.to_owned()],
        None => strings(rec, "planetTypes")?,
    };
    ctx.value_list("PlanetTrait", &values, upper)
}

fn strings(rec: &Rec, field: &str) -> Result<Vec<String>> {
    match rec.fields.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("{}: `{field}` holds {v}", rec.id))
            })
            .collect(),
        Some(other) => bail!("{}: `{field}` is {other}, not a list", rec.id),
    }
}

/// Whether a planet printed on this tile is some faction's homeworld (`galaxy::is_home_system`
/// over every source).
fn home_system(rec: &Rec, ctx: &Ctx) -> Result<String> {
    let home = ctx.records("planets").iter().any(|p| {
        str_field(p, "tileId") == Some(rec.id.as_str())
            && str_field(p, "factionHomeworld").is_some()
    });
    Ok(home.to_string())
}

fn anomaly(rec: &Rec, _: &Ctx) -> Result<String> {
    let flag = |f: &str| rec.fields.get(f).and_then(Value::as_bool).unwrap_or(false);
    Ok(format!(
        "Anomalies {{ asteroid_field: {}, gravity_rift: {}, nebula: {}, supernova: {}, scar: {} }}",
        flag("isAsteroidField"),
        flag("isGravityRift"),
        flag("isNebula"),
        flag("isSupernova"),
        flag("isScar"),
    ))
}

/// `attachmentId` names an attachment (Attach cards) or a token (Token cards); either must exist.
fn explore_target(rec: &Rec, ctx: &Ctx) -> Result<(Option<String>, Option<String>)> {
    let Some(id) = str_field(rec, "attachmentId") else {
        return Ok((None, None));
    };
    if ctx.has("attachments", id) {
        Ok((Some(ctx.variant("attachments", id)?), None))
    } else if ctx.has("tokens", id) {
        Ok((None, Some(ctx.variant("tokens", id)?)))
    } else {
        bail!("`attachmentId` {id:?} is neither an attachment nor a token")
    }
}

/// References the corpus makes to records it does not carry, accepted by name: (category, record
/// id, field, referenced id, why). Any other unresolved reference fails the generator.
pub const DANGLING: &[(&str, &str, &str, &str, &str)] = &[(
    "units",
    "ghemina_carrier2",
    "requiredTechId",
    "dsghemcv",
    "a Discordant Stars (homebrew) technology; the Ghemina units are leftovers of that extraction",
)];

fn required_technology(rec: &Rec, ctx: &Ctx) -> Result<String> {
    let Some(id) = str_field(rec, "requiredTechId") else {
        return Ok("Requirement::None".to_owned());
    };
    if ctx.has("technologies", id) {
        return Ok(format!(
            "Requirement::Known({})",
            ctx.variant("technologies", id)?
        ));
    }
    if DANGLING
        .iter()
        .any(|d| d.0 == "units" && d.1 == rec.id && d.2 == "requiredTechId" && d.3 == id)
    {
        return Ok("Requirement::NotInCorpus".to_owned());
    }
    bail!("no technologies record has the id {id:?} (add it to spec::DANGLING if that is intended)")
}

fn token_space(rec: &Rec, _: &Ctx) -> Result<String> {
    match str_field(rec, "spaceOrPlanet") {
        Some("space") => Ok("true".to_owned()),
        Some("planet") => Ok("false".to_owned()),
        other => bail!("`spaceOrPlanet` = {other:?}"),
    }
}

fn some_or_none(v: Option<String>) -> String {
    v.map_or_else(|| "None".to_owned(), |v| format!("Some({v})"))
}

fn explore_attachment(rec: &Rec, ctx: &Ctx) -> Result<String> {
    Ok(some_or_none(explore_target(rec, ctx)?.0))
}

fn explore_token(rec: &Rec, ctx: &Ctx) -> Result<String> {
    Ok(some_or_none(explore_target(rec, ctx)?.1))
}

fn value(field: &'static str, enm: &'static str, shape: Shape) -> Conv {
    Conv::Value {
        field,
        enm,
        norm: keep,
        shape,
    }
}

fn reference(field: &'static str, cat: &'static str, shape: Shape) -> Conv {
    Conv::Ref { field, cat, shape }
}

/// Every generated category, cards first.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn categories() -> Vec<Cat> {
    use Shape::{List, One, Opt};
    vec![
        Cat {
            key: "abilities",
            ty: "Ability",
            upper: "ABILITY",
            id_field: "id",
            name_field: "name",
            doc: "A faction ability (`abilities.json`).",
            columns: vec![source()],
            display: &[
                "name",
                "faction",
                "window",
                "windowEffect",
                "permanentEffect",
            ],
        },
        Cat {
            key: "action_cards",
            ty: "ActionCard",
            upper: "ACTION_CARD",
            id_field: "alias",
            name_field: "name",
            doc: "One physical action card (`action_cards.json`); copies are separate variants.",
            columns: vec![
                source(),
                col(
                    "component_action",
                    "The printed `window` is exactly `Action`. The reaction timing of every other window is engine knowledge (`reactions::window_table`), see `text_parsed_fields.md`.",
                    Conv::Custom {
                        ty: "bool",
                        f: component_action,
                    },
                ),
            ],
            display: &["name", "window", "phase", "text", "automationID"],
        },
        Cat {
            key: "agendas",
            ty: "Agenda",
            upper: "AGENDA",
            id_field: "alias",
            name_field: "name",
            doc: "An agenda card (`agendas.json`).",
            columns: vec![
                source(),
                col(
                    "kind",
                    "Law or Directive (`type`).",
                    value("type", "AgendaType", One),
                ),
                col(
                    "elects",
                    "What is elected (`target` up to `(`).",
                    Conv::Value {
                        field: "target",
                        enm: "AgendaElects",
                        norm: agenda_elects,
                        shape: One,
                    },
                ),
            ],
            display: &["name", "target", "text1", "text2", "mapText"],
        },
        Cat {
            key: "attachments",
            ty: "Attachment",
            upper: "ATTACHMENT",
            id_field: "id",
            name_field: "name",
            doc: "A planet attachment (`attachments.json`).",
            columns: vec![
                source(),
                col(
                    "resources",
                    "`resourcesModifier` (absent = 0).",
                    Conv::Int {
                        field: "resourcesModifier",
                        ty: "i8",
                        required: false,
                    },
                ),
                col(
                    "influence",
                    "`influenceModifier` (absent = 0).",
                    Conv::Int {
                        field: "influenceModifier",
                        ty: "i8",
                        required: false,
                    },
                ),
                col(
                    "specialties",
                    "Technology specialties given (`techSpeciality`).",
                    Conv::Value {
                        field: "techSpeciality",
                        enm: "TechType",
                        norm: tech_type,
                        shape: List,
                    },
                ),
                col(
                    "traits",
                    "Planet traits added (`planetTypes`).",
                    Conv::Value {
                        field: "planetTypes",
                        enm: "PlanetTrait",
                        norm: upper,
                        shape: List,
                    },
                ),
                col(
                    "space_cannon",
                    "SPACE CANNON granted (`spaceCannonHitsOn`, `spaceCannonDieCount`).",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: space_cannon,
                    },
                ),
            ],
            display: &["name", "isFakeAttachment"],
        },
        Cat {
            key: "breakthroughs",
            ty: "Breakthrough",
            upper: "BREAKTHROUGH",
            id_field: "alias",
            name_field: "name",
            doc: "A Thunder's Edge breakthrough (`breakthroughs.json`).",
            columns: vec![
                source(),
                faction_tag("faction", "The owning faction (`faction`).", One),
                col(
                    "synergy",
                    "The colours that count as one another (`synergy`; `NONE` = empty).",
                    Conv::Value {
                        field: "synergy",
                        enm: "TechType",
                        norm: tech_type,
                        shape: List,
                    },
                ),
            ],
            display: &["name", "text"],
        },
        Cat {
            key: "explores",
            ty: "Explore",
            upper: "EXPLORE",
            id_field: "id",
            name_field: "name",
            doc: "An exploration card (`explores.json`).",
            columns: vec![
                source(),
                col(
                    "deck",
                    "Which exploration deck (`type`).",
                    value("type", "ExploreType", One),
                ),
                col(
                    "resolution",
                    "How it resolves (`resolution`).",
                    value("resolution", "ExploreResolution", One),
                ),
                col(
                    "attachment",
                    "The attachment an Attach card places (`attachmentId` when it names an attachment).",
                    Conv::Custom {
                        ty: "Option<Attachment>",
                        f: explore_attachment,
                    },
                ),
                col(
                    "token",
                    "The token a Token card places (`attachmentId` when it names a token, not an attachment).",
                    Conv::Custom {
                        ty: "Option<Token>",
                        f: explore_token,
                    },
                ),
            ],
            display: &["name", "text", "flavorText"],
        },
        Cat {
            key: "galactic_events",
            ty: "GalacticEvent",
            upper: "GALACTIC_EVENT",
            id_field: "alias",
            name_field: "name",
            doc: "A Thunder's Edge galactic event (`galactic_events.json`); not implemented by the engine.",
            columns: vec![source()],
            display: &["name", "text"],
        },
        Cat {
            key: "genericcards",
            ty: "GenericCard",
            upper: "GENERIC_CARD",
            id_field: "alias",
            name_field: "name",
            doc: "A generic card (`genericcards.json`: Firmament plots and their twins).",
            columns: vec![source()],
            display: &["name", "text", "cardType"],
        },
        Cat {
            key: "leaders",
            ty: "Leader",
            upper: "LEADER",
            id_field: "id",
            name_field: "name",
            doc: "A leader: agent, commander or hero (`leaders.json`).",
            columns: vec![
                source(),
                faction_tag("faction", "The faction dealt the leader (`faction`).", One),
                col(
                    "kind",
                    "Agent, commander or hero (`type`).",
                    value("type", "LeaderType", One),
                ),
                col(
                    "window",
                    "Action, action phase or other (`abilityWindow`).",
                    Conv::Value {
                        field: "abilityWindow",
                        enm: "LeaderWindow",
                        norm: leader_window,
                        shape: One,
                    },
                ),
                col(
                    "replaces",
                    "The record this reprint removes from the deal (`homebrewReplacesID`).",
                    reference("homebrewReplacesID", "leaders", Opt),
                ),
            ],
            display: &[
                "name",
                "title",
                "abilityName",
                "abilityWindow",
                "abilityText",
                "unlockCondition",
            ],
        },
        Cat {
            key: "promissory_notes",
            ty: "PromissoryNote",
            upper: "PROMISSORY_NOTE",
            id_field: "alias",
            name_field: "name",
            doc: "A promissory note (`promissory_notes.json`); the generic notes keep their `<color>_x` corpus ids.",
            columns: vec![
                source(),
                faction_tag(
                    "faction",
                    "The owning faction of a faction note (`faction`).",
                    Opt,
                ),
                col(
                    "play_area",
                    "Goes face up in the play area when received (`playArea`).",
                    Conv::Flag("playArea"),
                ),
            ],
            display: &["name", "text"],
        },
        Cat {
            key: "public_objectives",
            ty: "PublicObjective",
            upper: "PUBLIC_OBJECTIVE",
            id_field: "alias",
            name_field: "name",
            doc: "A public objective (`public_objectives.json`).",
            columns: vec![
                source(),
                col(
                    "points",
                    "Victory points; 1 = stage I, 2 = stage II (`points`).",
                    Conv::Int {
                        field: "points",
                        ty: "u8",
                        required: true,
                    },
                ),
            ],
            display: &["name", "text", "phase"],
        },
        Cat {
            key: "relics",
            ty: "Relic",
            upper: "RELIC",
            id_field: "alias",
            name_field: "name",
            doc: "A relic (`relics.json`).",
            columns: vec![
                source(),
                col(
                    "fake",
                    "A pseudo-relic kept out of the relic deck (`isFakeRelic`).",
                    Conv::Flag("isFakeRelic"),
                ),
            ],
            display: &["name", "text"],
        },
        Cat {
            key: "secret_objectives",
            ty: "SecretObjective",
            upper: "SECRET_OBJECTIVE",
            id_field: "alias",
            name_field: "name",
            doc: "A secret objective (`secret_objectives.json`).",
            columns: vec![
                source(),
                col(
                    "phase",
                    "When it may be scored (`phase`).",
                    value("phase", "ObjectivePhase", One),
                ),
                col(
                    "points",
                    "Victory points (`points`).",
                    Conv::Int {
                        field: "points",
                        ty: "u8",
                        required: true,
                    },
                ),
            ],
            display: &["name", "text"],
        },
        Cat {
            key: "strategy_cards",
            ty: "StrategyCard",
            upper: "STRATEGY_CARD",
            id_field: "id",
            name_field: "name",
            doc: "One printing of a strategy card (`strategy_cards.json`).",
            columns: vec![
                source(),
                col(
                    "kind",
                    "The strategy card this is a printing of (`name`).",
                    value("name", "StrategyCardKind", One),
                ),
                col(
                    "initiative",
                    "Initiative number (`initiative`).",
                    Conv::Int {
                        field: "initiative",
                        ty: "u8",
                        required: true,
                    },
                ),
            ],
            display: &["name", "primaryTexts", "secondaryTexts"],
        },
        Cat {
            key: "technologies",
            ty: "Technology",
            upper: "TECHNOLOGY",
            id_field: "alias",
            name_field: "name",
            doc: "A technology (`technologies.json`).",
            columns: vec![
                source(),
                col(
                    "types",
                    "Colour(s) or unit upgrade (`types`; `NONE` = empty).",
                    Conv::Value {
                        field: "types",
                        enm: "TechType",
                        norm: tech_type,
                        shape: List,
                    },
                ),
                col(
                    "prerequisites",
                    "Prerequisites by letter (`requirements`; absent, `null`, `None` = none).",
                    Conv::Custom {
                        ty: "Prerequisites",
                        f: prerequisites,
                    },
                ),
                faction_tag(
                    "faction",
                    "The faction a faction technology belongs to (`faction`).",
                    Opt,
                ),
                col(
                    "base_upgrade",
                    "The generic unit upgrade this faction upgrade replaces (`baseUpgrade`).",
                    reference("baseUpgrade", "technologies", Opt),
                ),
                col(
                    "researchable",
                    "False when the printed `text` contains `cannot be researched` (case-insensitive), the rule of `technology.rs` and `deepwrought.rs`.",
                    Conv::Custom {
                        ty: "bool",
                        f: researchable,
                    },
                ),
            ],
            display: &["name", "text"],
        },
        // ----- non-card content -----
        Cat {
            key: "factions",
            ty: "Faction",
            upper: "FACTION",
            id_field: "alias",
            name_field: "factionName",
            doc: "A faction (`factions.json`), including the `neutral` pseudo-faction.",
            columns: vec![
                source(),
                col(
                    "home_system",
                    "Home system tile (`homeSystem`; empty = none).",
                    reference("homeSystem", "systems", Opt),
                ),
                col(
                    "home_planets",
                    "Home planets (`homePlanets`).",
                    reference("homePlanets", "planets", List),
                ),
                col(
                    "commodities",
                    "Commodity value (`commodities`).",
                    Conv::Int {
                        field: "commodities",
                        ty: "u8",
                        required: true,
                    },
                ),
                col(
                    "starting_tech",
                    "Fixed starting technologies (`startingTech`).",
                    reference("startingTech", "technologies", List),
                ),
                col(
                    "starting_tech_options",
                    "Technologies the starting choice is made from (`startingTechOptions`).",
                    reference("startingTechOptions", "technologies", List),
                ),
                col(
                    "starting_tech_amount",
                    "How many starting technologies are chosen (`startingTechAmount`).",
                    Conv::Int {
                        field: "startingTechAmount",
                        ty: "u8",
                        required: false,
                    },
                ),
                col(
                    "faction_tech",
                    "Faction technologies (`factionTech`).",
                    reference("factionTech", "technologies", List),
                ),
                col(
                    "abilities",
                    "Faction abilities (`abilities`).",
                    reference("abilities", "abilities", List),
                ),
                col(
                    "leaders",
                    "Leaders (`leaders`).",
                    reference("leaders", "leaders", List),
                ),
                col(
                    "promissory_notes",
                    "Faction promissory notes (`promissoryNotes`).",
                    reference("promissoryNotes", "promissory_notes", List),
                ),
                col(
                    "units",
                    "Unit roster (`units`).",
                    reference("units", "units", List),
                ),
            ],
            display: &[
                "factionName",
                "shortName",
                "shortTag",
                "complexity",
                "startingFleet",
            ],
        },
        Cat {
            key: "planets",
            ty: "Planet",
            upper: "PLANET",
            id_field: "id",
            name_field: "name",
            doc: "A planet or space station (`planets.json`).",
            columns: vec![
                source(),
                col(
                    "resources",
                    "Resource value (`resources`).",
                    Conv::Int {
                        field: "resources",
                        ty: "u8",
                        required: true,
                    },
                ),
                col(
                    "influence",
                    "Influence value (`influence`).",
                    Conv::Int {
                        field: "influence",
                        ty: "u8",
                        required: true,
                    },
                ),
                col(
                    "traits",
                    "Planet types as printed (`planetType`, else `planetTypes`).",
                    Conv::Custom {
                        ty: "&'static [PlanetTrait]",
                        f: planet_traits,
                    },
                ),
                col(
                    "specialties",
                    "Technology specialties (`techSpecialties`).",
                    Conv::Value {
                        field: "techSpecialties",
                        enm: "TechType",
                        norm: tech_type,
                        shape: List,
                    },
                ),
                col(
                    "legendary",
                    "Has a legendary ability (`legendaryAbilityName` is set).",
                    Conv::Custom {
                        ty: "bool",
                        f: legendary,
                    },
                ),
                col(
                    "system",
                    "The tile it is printed on (`tileId`); `None` for planets placed during play.",
                    reference("tileId", "systems", Opt),
                ),
                col(
                    "homeworld_of",
                    "The faction whose homeworld this is (`factionHomeworld`).",
                    Conv::Value {
                        field: "factionHomeworld",
                        enm: "FactionTag",
                        norm: keep,
                        shape: Opt,
                    },
                ),
                col(
                    "space_cannon",
                    "SPACE CANNON of the planet (`spaceCannonHitsOn`, `spaceCannonDieCount`).",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: space_cannon,
                    },
                ),
            ],
            display: &[
                "name",
                "shortName",
                "legendaryAbilityName",
                "legendaryAbilityText",
                "flavourText",
                "aliases",
            ],
        },
        Cat {
            key: "systems",
            ty: "System",
            upper: "SYSTEM",
            id_field: "id",
            name_field: "name",
            doc: "A system tile (`systems.json`), hyperlane tiles and their rotations included. Adjacency is not static content (it depends on the layout) and is not generated.",
            columns: vec![
                source(),
                col(
                    "planets",
                    "Planets on the tile, in corpus order (`planets`).",
                    reference("planets", "planets", List),
                ),
                col(
                    "wormholes",
                    "Wormholes printed on the tile (`wormholes`).",
                    value("wormholes", "Wormhole", List),
                ),
                col(
                    "anomalies",
                    "Anomaly flags (`isAsteroidField`, `isGravityRift`, `isNebula`, `isSupernova`, `isScar`).",
                    Conv::Custom {
                        ty: "Anomalies",
                        f: anomaly,
                    },
                ),
                col(
                    "hyperlane",
                    "A hyperlane tile (`isHyperlane`).",
                    Conv::Flag("isHyperlane"),
                ),
                col(
                    "tile_back",
                    "Tile back colour (`tileBack`).",
                    value("tileBack", "TileBack", Opt),
                ),
                col(
                    "home_system",
                    "Some planet printed on this tile is a faction homeworld (derived from `planets.json`).",
                    Conv::Custom {
                        ty: "bool",
                        f: home_system,
                    },
                ),
            ],
            display: &["name", "aliases"],
        },
        Cat {
            key: "tokens",
            ty: "Token",
            upper: "TOKEN",
            id_field: "id",
            name_field: "name",
            doc: "A token (`tokens.json`): wormhole, anomaly and planet tokens.",
            columns: vec![
                source(),
                col(
                    "wormholes",
                    "Wormholes the token carries (`wormholes`).",
                    value("wormholes", "Wormhole", List),
                ),
                col(
                    "space",
                    "Placed in the space area, not on a planet (`spaceOrPlanet` = `space`).",
                    Conv::Custom {
                        ty: "bool",
                        f: token_space,
                    },
                ),
            ],
            display: &["aliasList", "tokenPlanetName"],
        },
        Cat {
            key: "units",
            ty: "Unit",
            upper: "UNIT",
            id_field: "id",
            name_field: "name",
            doc: "A unit type: generic, upgraded, faction or neutral (`units.json`).",
            columns: vec![
                source(),
                col(
                    "base_type",
                    "The unit kind (`baseType`).",
                    value("baseType", "UnitBaseType", One),
                ),
                faction_tag("faction", "The faction of a faction unit (`faction`).", Opt),
                col(
                    "cost_halves",
                    "Cost in halves (`cost` x 2: a fighter is 1); `None` = no cost printed.",
                    Conv::Custom {
                        ty: "Option<u8>",
                        f: cost,
                    },
                ),
                col(
                    "movement",
                    "Move value (`moveValue`).",
                    Conv::Int {
                        field: "moveValue",
                        ty: "u8",
                        required: false,
                    },
                ),
                col(
                    "capacity",
                    "Capacity (`capacityValue`).",
                    Conv::Int {
                        field: "capacityValue",
                        ty: "u8",
                        required: false,
                    },
                ),
                col(
                    "capacity_used",
                    "Capacity this unit takes when transported (`capacityUsed`).",
                    Conv::Int {
                        field: "capacityUsed",
                        ty: "u8",
                        required: false,
                    },
                ),
                col(
                    "combat",
                    "Combat roll (`combatHitsOn`, `combatDieCount`); 0 = none.",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: combat,
                    },
                ),
                col(
                    "anti_fighter_barrage",
                    "ANTI-FIGHTER BARRAGE (`afbHitsOn`, `afbDieCount`).",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: afb,
                    },
                ),
                col(
                    "bombardment",
                    "BOMBARDMENT (`bombardHitsOn`, `bombardDieCount`, dice default 1).",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: bombardment,
                    },
                ),
                col(
                    "space_cannon",
                    "SPACE CANNON (`spaceCannonHitsOn`, `spaceCannonDieCount`, dice default 1).",
                    Conv::Custom {
                        ty: "Option<Roll>",
                        f: space_cannon,
                    },
                ),
                col(
                    "production",
                    "PRODUCTION (`productionValue`, `basicProduction`).",
                    Conv::Custom {
                        ty: "Option<Production>",
                        f: production,
                    },
                ),
                col("ship", "`isShip`.", Conv::Flag("isShip")),
                col(
                    "ground_force",
                    "`isGroundForce` as printed (the engine's `UnitType::is_ground_force` decides by base type instead).",
                    Conv::Flag("isGroundForce"),
                ),
                col("structure", "`isStructure`.", Conv::Flag("isStructure")),
                col("space_only", "`isSpaceOnly`.", Conv::Flag("isSpaceOnly")),
                col(
                    "sustain_damage",
                    "SUSTAIN DAMAGE (`sustainDamage`).",
                    Conv::Flag("sustainDamage"),
                ),
                col(
                    "planetary_shield",
                    "PLANETARY SHIELD (`planetaryShield`).",
                    Conv::Flag("planetaryShield"),
                ),
                col(
                    "deep_space_cannon",
                    "`deepSpaceCannon`.",
                    Conv::Flag("deepSpaceCannon"),
                ),
                col(
                    "disables_planetary_shield",
                    "`disablesPlanetaryShield`.",
                    Conv::Flag("disablesPlanetaryShield"),
                ),
                col(
                    "can_be_direct_hit",
                    "`canBeDirectHit`.",
                    Conv::Flag("canBeDirectHit"),
                ),
                col(
                    "upgrades_to",
                    "`upgradesToUnitId`.",
                    reference("upgradesToUnitId", "units", Opt),
                ),
                col(
                    "upgrades_from",
                    "`upgradesFromUnitId`.",
                    reference("upgradesFromUnitId", "units", Opt),
                ),
                col(
                    "required_technology",
                    "The technology that unlocks this unit (`requiredTechId`); `NotInCorpus` for a reference listed in the generator's `DANGLING`.",
                    Conv::Custom {
                        ty: "Requirement<Technology>",
                        f: required_technology,
                    },
                ),
                col(
                    "replaces",
                    "The unit this reprint replaces (`homebrewReplacesID`).",
                    reference("homebrewReplacesID", "units", Opt),
                ),
            ],
            display: &["name", "subtitle", "ability", "asyncId"],
        },
    ]
}

/// The extra types the data tables use, emitted once into the core's `values.rs`.
pub const SHARED_TYPES: &str = r"
/// A die roll: hits on `hits_on` or higher, `dice` dice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Roll {
    pub hits_on: u8,
    pub dice: u8,
}

/// PRODUCTION: `value`, plus the planet's resources when `plus_planet_resources`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Production {
    pub value: u8,
    pub plus_planet_resources: bool,
}

/// Technology prerequisites by printed letter (B, G, R, Y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Prerequisites {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub yellow: u8,
}

/// A reference that may be absent, or name a record the corpus does not carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Requirement<T> {
    /// Nothing required.
    None,
    Known(T),
    /// The corpus names something it does not have (see the generator's `DANGLING` list): the
    /// requirement can never be met.
    NotInCorpus,
}

/// The anomaly flags of a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Anomalies {
    pub asteroid_field: bool,
    pub gravity_rift: bool,
    pub nebula: bool,
    pub supernova: bool,
    pub scar: bool,
}

impl Anomalies {
    /// Any anomaly at all (`System::is_anomaly`).
    #[must_use]
    pub const fn any(self) -> bool {
        self.asteroid_field || self.gravity_rift || self.nebula || self.supernova || self.scar
    }
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agenda_elects_follows_vote_outcomes() {
        let e = |s| agenda_elects(s).unwrap().unwrap();
        assert_eq!(e("For/Against"), "For/Against");
        assert_eq!(e("Elect Player"), "Player");
        assert_eq!(e("Elect Law (When this agenda ...)"), "Law");
        assert_eq!(
            e("When this agenda is revealed, the speaker ... Elect Player"),
            "For/Against"
        );
    }

    #[test]
    fn leader_window_follows_the_engine() {
        let w = |s| leader_window(s).unwrap().unwrap();
        assert_eq!(w("ACTION:"), "action");
        assert_eq!(w("During the action phase:"), "action_phase");
        assert_eq!(w("At the start of a space combat:"), "other");
    }
}
