//! Typed IDs for all domain entities.
//!
//! Every ID is a newtype around a shared, immutable string (`Arc<str>`) to prevent accidental
//! mixing of entity types. A clone is a reference-count increment instead of a heap copy.
//!
//! Everything observable is the string's: `Eq`, `Ord` and `Hash` compare and hash the text (so a
//! `BTreeMap` keyed by an id iterates in exactly the order it did with `String`), `Debug` prints
//! `PlayerId("a")` as before, `Display` the text, and serde reads and writes a plain string
//! (docs/ENGINE_OPTIMIZATION.md, "String ids").

use std::fmt;
use std::sync::Arc;

/// Serialize and deserialize an id as its plain string, as `#[serde(transparent)]` over
/// `String` did.
/// `Eq` and `Ord` of the text, with a shortcut for two handles of the same string (a clone):
/// exactly the derived `String` comparisons, without the `memcmp` when the pointers agree.
macro_rules! string_order {
    ($($t:ident),* $(,)?) => {$(
        impl PartialEq for $t {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
            }
        }
        impl Eq for $t {}
        /// Probe a `BTreeMap`/`BTreeSet`/`HashMap` of ids with a plain `&str`, without building
        /// an id: `Eq`, `Ord` and `Hash` are the text's, so they agree with `str`'s.
        impl std::borrow::Borrow<str> for $t {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
        impl std::hash::Hash for $t {
            #[inline]
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        impl PartialOrd for $t {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for $t {
            #[inline]
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                if Arc::ptr_eq(&self.0, &other.0) {
                    std::cmp::Ordering::Equal
                } else {
                    self.0.cmp(&other.0)
                }
            }
        }
    )*};
}

macro_rules! string_serde {
    ($($t:ident),* $(,)?) => {$(
        impl serde::Serialize for $t {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                <std::borrow::Cow<'de, str> as serde::Deserialize>::deserialize(deserializer)
                    .map(|text| Self(Arc::from(text.as_ref())))
            }
        }
    )*};
}

// ─── Core entity IDs ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlayerId(Arc<str>);

impl PlayerId {
    pub fn new(s: impl AsRef<str>) -> Self {
        crate::count::bump("PlayerId::new");
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FactionId(Arc<str>);

impl FactionId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanetId(Arc<str>);

impl PlanetId {
    pub fn new(s: impl AsRef<str>) -> Self {
        crate::count::bump("PlanetId::new");
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}


impl fmt::Display for PlanetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SystemId(Arc<str>);

impl SystemId {
    pub fn new(s: impl AsRef<str>) -> Self {
        crate::count::bump("SystemId::new");
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UnitTypeId(Arc<str>);

impl UnitTypeId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnitTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TechnologyId(Arc<str>);

impl TechnologyId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TechnologyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActionCardId(Arc<str>);

impl ActionCardId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActionCardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ObjectiveId(Arc<str>);

impl ObjectiveId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SecretObjectiveId(Arc<str>);

impl SecretObjectiveId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretObjectiveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LeaderId(Arc<str>);

impl LeaderId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LeaderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct PromissoryNoteId(Arc<str>);

impl PromissoryNoteId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PromissoryNoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RelicId(Arc<str>);

impl RelicId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct ExplorationCardId(Arc<str>);

impl ExplorationCardId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExplorationCardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct FragmentId(Arc<str>);

impl FragmentId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FragmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Tactical IDs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TokenId(Arc<str>);

impl TokenId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct AgendaId(Arc<str>);

impl AgendaId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgendaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct LawId(Arc<str>);

impl LawId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LawId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StrategyCardId(Arc<str>);

impl StrategyCardId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StrategyCardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct CommandId(Arc<str>);

impl CommandId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Thunder's Edge IDs ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ExpeditionTileId(Arc<str>);

impl ExpeditionTileId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExpeditionTileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct BreakthroughId(Arc<str>);

impl BreakthroughId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BreakthroughId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Event/Choice IDs ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EventId(Arc<str>);

impl EventId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct ChoiceId(Arc<str>);

impl ChoiceId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChoiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct OptionId(Arc<str>);

impl OptionId {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(Arc::from(s.as_ref()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Utility ───────────────────────────────────────────────────────────────────


impl PlayerId {
    #[must_use]
    pub fn is_unseated(&self) -> bool {
        self.0.is_empty()
    }
}

string_serde!(PlayerId, FactionId, PlanetId, SystemId, UnitTypeId, TechnologyId, ActionCardId, ObjectiveId, SecretObjectiveId, LeaderId, PromissoryNoteId, RelicId, ExplorationCardId, FragmentId, TokenId, AgendaId, LawId, StrategyCardId, CommandId, ExpeditionTileId, BreakthroughId, EventId, ChoiceId, OptionId);
string_order!(PlayerId, FactionId, PlanetId, SystemId, UnitTypeId, TechnologyId, ActionCardId, ObjectiveId, SecretObjectiveId, LeaderId, PromissoryNoteId, RelicId, ExplorationCardId, FragmentId, TokenId, AgendaId, LawId, StrategyCardId, CommandId, ExpeditionTileId, BreakthroughId, EventId, ChoiceId, OptionId);

/// The text of anything string-like, for probing a map or set of ids by `&str` (every id is
/// `Borrow<str>`): `seat.technologies.contains(text(&alias))` instead of building an id.
#[inline]
#[must_use]
pub fn text<T: AsRef<str> + ?Sized>(value: &T) -> &str {
    value.as_ref()
}
