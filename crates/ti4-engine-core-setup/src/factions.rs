//! The faction facts the setup reads (the pure half of `ti4_engine::builders::factions`).

/// The Nekro Virus (`factions::nekro::FACTION`): also the faction name in its promissory note ids.
pub const NEKRO: &str = "nekro";

/// The Empyrean (`factions::empyrean::FACTION`).
pub const EMPYREAN: &str = "empyrean";

/// The three Keleres variant aliases, in Tribuni order (`factions::keleres::VARIANTS`).
pub const KELERES_VARIANTS: [&str; 3] = ["keleresm", "keleresx", "keleresa"];

/// Factions whose setup choice is refused by their own text (Nocturne;
/// `factions::obsidian::NOT_CHOOSABLE_DURING_SETUP`).
pub const NOT_CHOOSABLE_DURING_SETUP: &[&str] = &["obsidian"];

/// The alias of every legacy faction module in `factions::MODULES` order: the roster the wide
/// seating draws from. A parity test compares it with the legacy table.
pub const MODULE_ALIASES: [&str; 27] = [
    "arborec", "argent", "bastion", "cabal", "crimson", "deepwrought", "empyrean", "firmament",
    "ghost", "keleresm", "keleresx", "keleresa", "mahact", "mentak", "muaat", "naalu", "naaz",
    "nekro", "nomad", "obsidian", "ralnel", "saar", "sardakk", "titans", "winnu", "yin", "yssaril",
];

/// Whether `faction` is any Keleres variant.
#[must_use]
pub fn is_keleres_faction(faction: &str) -> bool {
    KELERES_VARIANTS.contains(&faction)
}

/// Whether a content record's `faction` tag belongs to `alias`: the alias itself, or, for a Keleres
/// variant, the family `keleres` (and, for units, the first variant `keleresm`).
#[must_use]
pub fn tag_belongs_to(tag: &str, alias: &str, unit: bool) -> bool {
    tag.eq_ignore_ascii_case(alias)
        || (is_keleres_faction(alias)
            && (tag.eq_ignore_ascii_case("keleres")
                || (unit && tag.eq_ignore_ascii_case("keleresm"))))
}

/// Mahact, Hubris: "During setup, purge your Alliance promissory note."
#[must_use]
pub fn purges_alliance(faction: &str) -> bool {
    faction == "mahact"
}

/// The faction whose home system, command tokens and control markers a Keleres variant takes (The Tribuni), or `None` for any other.
#[must_use]
pub fn tribuni_base(variant: &str) -> Option<&'static str> {
    match variant {
        "keleresm" => Some("mentak"),
        "keleresx" => Some("xxcha"),
        "keleresa" => Some("argent"),
        _ => None,
    }
}

/// The Tribuni (and the one-faction-per-seat rule): a Keleres variant's base faction is unplayed, and only one seat is the Council
/// Keleres. `factions` is every seat's faction; order-independent. The error text is `ti4_engine::seating::SeatingError`'s.
///
/// # Errors
/// The message of `SeatingError::TribuniFactionPlayed` or `KeleresSeatedTwice`.
pub fn validate_tribuni<'a>(factions: impl IntoIterator<Item = &'a str> + Clone) -> Result<(), String> {
    let mut keleres = factions.clone().into_iter().filter(|faction| is_keleres_faction(faction));
    let Some(variant) = keleres.next() else {
        return Ok(());
    };
    if keleres.next().is_some() {
        return Err("the Council Keleres is seated more than once".to_owned());
    }
    match tribuni_base(variant) {
        Some(base) if factions.into_iter().any(|played| played == base) => {
            Err(format!("Keleres variant {variant:?} needs {base:?}, which another seat plays"))
        }
        _ => Ok(()),
    }
}
