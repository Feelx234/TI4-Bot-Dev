use crate::containers::SmallList;

/// Which `Vec<u32>` of rounds / sequence numbers a mark belongs to. The eight lists of the string
/// `Player` (`fighter_bonus_round`, `bunker_invasion`, `war_machine_use`, `blitz_invasion`,
/// `disable_invasion`, `solar_flare`, `lost_star`, `dominus_orb`) are almost always empty, so they
/// share one tagged list (24 bytes instead of 8 x 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RoundKind {
    FighterBonus,
    BunkerInvasion,
    WarMachine,
    BlitzInvasion,
    DisableInvasion,
    SolarFlare,
    LostStar,
    DominusOrb,
}

/// The tagged list. Per kind the entries keep their insertion order, exactly as the separate `Vec`s did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoundMarks {
    marks: SmallList<(RoundKind, u32), 2>,
}

impl RoundMarks {
    /// The values of one kind, in insertion order.
    pub fn of(&self, kind: RoundKind) -> impl Iterator<Item = u32> + '_ {
        self.marks.iter().filter(move |(k, _)| *k == kind).map(|(_, n)| *n)
    }

    #[must_use]
    pub fn contains(&self, kind: RoundKind, value: u32) -> bool {
        self.marks.iter().any(|(k, n)| *k == kind && *n == value)
    }

    pub fn push(&mut self, kind: RoundKind, value: u32) {
        self.marks.push((kind, value));
    }

    /// Drop every value of `kind`.
    pub fn clear(&mut self, kind: RoundKind) {
        self.marks.retain(|(k, _)| *k != kind);
    }

    /// Keep the values of `kind` the predicate accepts.
    pub fn retain(&mut self, kind: RoundKind, mut keep: impl FnMut(u32) -> bool) {
        self.marks.retain(|(k, n)| *k != kind || keep(*n));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// All entries in the order they were pushed (codec use).
    #[must_use]
    pub fn entries(&self) -> &[(RoundKind, u32)] {
        &self.marks
    }
}
