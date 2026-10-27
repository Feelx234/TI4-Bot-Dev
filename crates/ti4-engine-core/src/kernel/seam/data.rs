//! "Has X, then +n" as data (docs/ENGINE_DISENTANGLE.md 2.8, decision 3): the combat effects of the
//! string engine that are a literal presence test and a number. A row is registered by the table
//! loader, not written as code; the engine reads it through [`Seams`](super::Seams).
//!
//! The rows are the twelve found in `combat.rs` / `invasion.rs` and the helpers they call (`faction_abilities`,
//! `laws`, `technology`, `relics`): see [`COMBAT_ROWS`]. Anything that needs more than "this seat has
//! X" (Mahact's token test, Naaz's flagship-and-mech, Bastion's galvanized flag, Nekro's X/Y token) stays
//! a provider function; those are listed in `docs/ENGINE_CORE_FACTIONS_PLAN.md`.

use super::sites::RollKind;
use super::{Q, RuleRef, Source};
use crate::containers::SeatSet;
use crate::generated::{Ability, ActionCard, Agenda, Faction, Relic, System, Technology};
use crate::ids::Seat;
use crate::state::RoundKind;

/// The roll kinds a row applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollMask(u8);

impl RollMask {
    pub const ALL: Self = Self(0b1_1111);
    pub const COMBAT: Self = Self::of(RollKind::Combat);
    pub const CANNON: Self = Self::of(RollKind::SpaceCannon);

    #[must_use]
    pub const fn of(k: RollKind) -> Self {
        Self(1 << k as u8)
    }

    #[must_use]
    pub const fn has(self, k: RollKind) -> bool {
        self.0 & (1 << k as u8) != 0
    }
}

/// The presence test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// The seat's faction has this ability.
    Ability(Ability),
    /// The seat has this technology's text.
    Tech(Technology),
    /// Any foe has this technology's text.
    FoeTech(Technology),
    /// The seat holds this law (a ministry or a directive it was elected for).
    Law(Agenda),
    /// The seat holds this relic.
    Relic(Relic),
    /// One time per round mark of this kind equal to the live combat round sequence.
    RoundMarks(RoundKind),
    /// The seat's `combat_bonus_round` is the live combat round sequence.
    CombatBonusRound,
    /// The seat is the defender of a space combat in a nebula (59.5).
    DefenderInNebula,
}

/// What the row does when the test holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Add to the result of the roll (the `hit_shift` stack): +n lowers the number a die needs.
    RollShift(i64),
    /// Roll this many additional dice (the `dice` stack).
    ExtraDice(i64),
    /// A use of SUSTAIN DAMAGE cancels this many hits.
    CancelOnSustain(i64),
    /// Non-fighter units gain SUSTAIN DAMAGE.
    GrantsSustain,
    /// The seat may resolve ANTI-FIGHTER BARRAGE `hits_on` (x`dice`).
    Barrage { hits_on: i8, dice: u8 },
    /// Space docks gain SPACE CANNON `hits_on` (x`dice`).
    DockCannon { hits_on: i8, dice: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierRow {
    pub source: Source,
    pub needs: Needs,
    pub kind: RowKind,
    /// Roll kinds a [`RowKind::RollShift`] or [`RowKind::ExtraDice`] applies to.
    pub rolls: RollMask,
    pub fighters_only: bool,
}

const fn row(source: Source, needs: Needs, kind: RowKind, rolls: RollMask, fighters_only: bool) -> ModifierRow {
    ModifierRow { source, needs, kind, rolls, fighters_only }
}

/// The twelve rows. String origin in the comment.
pub const COMBAT_ROWS: [ModifierRow; 12] = [
    // faction_abilities::combat_modifier: "unrelenting" => +1
    row(Source::Faction(Faction::SardakkNorr), Needs::Ability(Ability::Unrelenting), RowKind::RollShift(1), RollMask::COMBAT, false),
    // faction_abilities::combat_modifier: "fragile" => -1
    row(Source::Faction(Faction::TheUniversitiesOfJolNar), Needs::Ability(Ability::Fragile), RowKind::RollShift(-1), RollMask::COMBAT, false),
    // combat.rs fighter_bonus_now: Fighter Prototype, 2 per copy played this round
    row(Source::ActionCard(ActionCard::FighterPrototype), Needs::RoundMarks(RoundKind::FighterBonus), RowKind::RollShift(2), RollMask::COMBAT, true),
    // laws::fighter_combat_bonus: Prophecy of Ixth
    row(Source::Law(Agenda::ProphecyOfIxth), Needs::Law(Agenda::ProphecyOfIxth), RowKind::RollShift(1), RollMask::COMBAT, true),
    // combat.rs effective_from: Morale Boost (`combat_bonus_round`)
    row(Source::ActionCard(ActionCard::MoraleBoost1), Needs::CombatBonusRound, RowKind::RollShift(1), RollMask::COMBAT, false),
    // combat.rs effective_from: 59.5 nebula defender
    row(Source::Rule(RuleRef(595)), Needs::DefenderInNebula, RowKind::RollShift(1), RollMask::COMBAT, false),
    // combat.rs space cannon: Antimass Deflectors, -1 to other players' SPACE CANNON rolls against the owner
    row(Source::Technology(Technology::AntimassDeflectors), Needs::FoeTech(Technology::AntimassDeflectors), RowKind::RollShift(-1), RollMask::CANNON, false),
    // combat.rs non_euclidean_shielding: each SUSTAIN DAMAGE cancels two hits
    row(Source::Technology(Technology::NonEuclideanShielding), Needs::Tech(Technology::NonEuclideanShielding), RowKind::CancelOnSustain(2), RollMask::ALL, false),
    // invasion.rs plasma_scoring: one unit rolls one additional die (the "one unit" choice stays in the window)
    row(Source::Technology(Technology::PlasmaScoring), Needs::Tech(Technology::PlasmaScoring), RowKind::ExtraDice(1), RollMask::CANNON, false),
    // relics::grants_sustain
    row(Source::Relic(Relic::MetaliVoidShielding), Needs::Relic(Relic::MetaliVoidShielding), RowKind::GrantsSustain, RollMask::ALL, false),
    // relics::extra_barrage
    row(Source::Relic(Relic::MetaliVoidArmaments), Needs::Relic(Relic::MetaliVoidArmaments), RowKind::Barrage { hits_on: 6, dice: 3 }, RollMask::of(RollKind::Barrage), false),
    // relics::space_dock_cannon
    row(Source::Relic(Relic::LightrailOrdnance), Needs::Relic(Relic::LightrailOrdnance), RowKind::DockCannon { hits_on: 5, dice: 2 }, RollMask::CANNON, false),
];

impl ModifierRow {
    /// How many times the row applies to `who` (0: never; more than 1 only for round marks).
    #[must_use]
    pub fn times(&self, q: Q<'_>, who: Seat, foes: SeatSet, system: Option<System>, round: u32) -> i64 {
        let s = q.s;
        let Some(p) = s.players().by_seat(who) else { return 0 };
        let yes = |b: bool| i64::from(b);
        match self.needs {
            Needs::Ability(a) => yes(p.faction.data().abilities.contains(&a)),
            Needs::Tech(t) => yes(crate::query::reach::technology_text(s, who, t)),
            Needs::FoeTech(t) => yes(foes.iter().any(|f| crate::query::reach::technology_text(s, f, t))),
            Needs::Law(a) => yes(crate::query::laws::holds_ministry(s, crate::query::reach::blanked(s), who, a)),
            Needs::Relic(r) => yes(p.cards.relics.contains(&r)),
            Needs::RoundMarks(kind) => p.round_marks.of(kind).filter(|n| *n == s.meta().combat_round_seq).count() as i64,
            Needs::CombatBonusRound => yes(p.combat_timers.combat_bonus_round.get() == Some(s.meta().combat_round_seq)),
            Needs::DefenderInNebula => {
                let sys = system.or(s.meta().active_system);
                let nebula = sys.is_some_and(|y| crate::query::reach::scope_of(q.k).system(y).is_some_and(|d| d.anomalies.nebula));
                yes(nebula && s.meta().active.is_some_and(|a| a != who) && round > 0)
            }
        }
    }

    /// What a [`RowKind::RollShift`] or [`RowKind::ExtraDice`] adds at a roll site.
    #[must_use]
    pub fn amount(&self, times: i64) -> i64 {
        match self.kind {
            RowKind::RollShift(n) | RowKind::ExtraDice(n) | RowKind::CancelOnSustain(n) => n * times,
            _ => 0,
        }
    }
}

/// Of `rows`, the sum of the [`RowKind::CancelOnSustain`] amounts that hold for `who`.
#[must_use]
pub fn cancel_on_sustain(q: Q<'_>, rows: &[ModifierRow], who: Seat) -> i64 {
    rows.iter()
        .filter(|r| matches!(r.kind, RowKind::CancelOnSustain(_)))
        .map(|r| r.amount(r.times(q, who, SeatSet::EMPTY, None, 0)))
        .sum()
}

/// Whether some row of `rows` grants SUSTAIN DAMAGE to `who`'s non-fighter units.
#[must_use]
pub fn grants_sustain(q: Q<'_>, rows: &[ModifierRow], who: Seat) -> bool {
    rows.iter().any(|r| r.kind == RowKind::GrantsSustain && r.times(q, who, SeatSet::EMPTY, None, 0) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::Kernel;
    use crate::state::{CoreState, Player, Players};

    #[test]
    fn twelve_rows_each_with_a_distinct_source_and_kind_pairing() {
        assert_eq!(COMBAT_ROWS.len(), 12);
        let shifts = COMBAT_ROWS.iter().filter(|r| matches!(r.kind, RowKind::RollShift(_))).count();
        assert_eq!(shifts, 7);
    }

    #[test]
    fn a_row_applies_only_to_the_seat_that_has_the_thing() {
        let k = Kernel::plain();
        let mut s = CoreState::all_loaded();
        *s.players_mut() = Players::from_vec(vec![
            Player::new(Seat(0), Faction::SardakkNorr),
            Player::new(Seat(1), Faction::TheUniversitiesOfJolNar),
        ]);
        let q = Q::new(&s, &k);
        let unrelenting = &COMBAT_ROWS[0];
        assert_eq!(unrelenting.times(q, Seat(0), SeatSet::EMPTY, None, 1), 1);
        assert_eq!(unrelenting.times(q, Seat(1), SeatSet::EMPTY, None, 1), 0);
        assert_eq!(COMBAT_ROWS[1].times(q, Seat(1), SeatSet::EMPTY, None, 1), 1);
        assert_eq!(COMBAT_ROWS[1].amount(1), -1);
    }
}
