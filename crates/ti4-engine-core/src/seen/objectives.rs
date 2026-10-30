//! Objectives: `Observed::{scored_by, revealed_objectives, revealed_objective_progress_imagining,
//! objectives_implicating_system}` and the bound seat's `SeatObservation::held_secret_progress_imagining`.
//!
//! The progress rows are the answers of `query::objectives` (the core half the string engine already calls through
//! `objectives_core`): `counting_progress`, `remaining_position_progress`, `bought_progress`, `secret_progress`. The
//! hook terms of a seat ([`Terms`]) come from the hook table; Keleres' IIHQ is empty because no Keleres seat can be
//! seated in the lobby.
//!
//! Not here (bail `D3-bail-02`): `scoreable_secret` (the bot never calls it).

use std::rc::Rc;

use super::Seen;
use crate::bits::SysSet;
use crate::containers::SmallList;
use crate::generated::{FactionTag, PublicObjective, System};
use crate::ids::Seat;
use crate::query::objectives::{self as q, CostFamily, Family, Imagined, Pos, SeatFacts, Terms};
use crate::query::{board, economy};
use crate::state::{ObjectiveRef, ObjectiveSet, Slice};

/// What a progress row counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowFamily {
    Count(Family),
    Cost(CostFamily),
}

/// One objective card's exact progress (`objectives::CardProgress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressRow {
    pub objective: ObjectiveRef,
    pub family: RowFamily,
    pub have: i64,
    pub threshold: i64,
    pub satisfied: bool,
    /// The printed stage of a public objective.
    pub stage: Option<u8>,
}

/// The per-seat scans of a position, built on first use.
#[derive(Default)]
pub struct FactsMemo {
    slots: [Option<Rc<SeatFacts>>; 16],
}

impl Seen<'_> {
    /// Whether the position has the map (the string `Observed::galaxy().is_some()`).
    fn has_map(&self) -> bool {
        self.st().loaded.has(Slice::Map) && self.st().galaxy().is_some()
    }

    fn terms(&self, seat: Seat) -> Terms {
        let (core, k) = (self.st(), self.kernel());
        let mut t = Terms { blanked: self.blanked(), ..Terms::default() };
        t.faction_tag = core
            .players()
            .by_seat(seat)
            .and_then(|p| FactionTag::ALL.iter().copied().find(|tag| matches!(tag.factions(), [only] if *only == p.faction)));
        for p in k.hooks.direct.borrowed_commanders_b_firmament_planets(core, k, seat).as_slice() {
            t.firmament.insert(*p);
        }
        t.per_good = k.hooks.economy.trade_good_worth(core, seat, 1).max(1);
        t.commodities = k.hooks.direct.keleres_spendable_commodities(core, seat);
        t.oceans = !k.hooks.direct.deepwrought_spendable_oceans(core, seat).is_empty();
        for (_, planet) in board::controlled_planets(core, seat).as_slice() {
            let bonus = k.hooks.direct.bastion_units_resource_bonus(core, *planet);
            if bonus != 0 {
                t.helios.push((*planet, bonus));
            }
        }
        t
    }

    fn facts(&self, seat: Seat) -> Rc<SeatFacts> {
        let slot = usize::from(seat.0);
        if let Some(f) = &self.memo().borrow().facts.slots[slot] {
            return Rc::clone(f);
        }
        let f = Rc::new(SeatFacts::of(self.st(), self.scope(), seat, self.terms(seat)));
        self.memo().borrow_mut().facts.slots[slot] = Some(Rc::clone(&f));
        f
    }

    fn edge(&self) -> SysSet {
        if self.has_map() { q::edge_systems(self.st()) } else { SysSet::EMPTY }
    }

    /// The seat's scored objectives (`Observed::scored_by`).
    #[must_use]
    pub fn scored_by(&self, seat: Seat) -> ObjectiveSet {
        self.st().objectives().scored[usize::from(seat.0)]
    }

    /// The revealed objectives in reveal order (`Observed::revealed_objectives`).
    #[must_use]
    pub fn revealed_objectives(&self) -> &[ObjectiveRef] {
        self.st().objectives().revealed.as_slice()
    }

    /// Exact progress of every revealed public objective for `seat`, after `imagined` happened
    /// (`Observed::revealed_objective_progress_imagining`). `None`: a priced objective the core cannot answer (ocean
    /// cards); the string engine answers there.
    #[must_use]
    pub fn revealed_objective_progress(&self, seat: Seat, imagined: Imagined<'_>) -> Option<Vec<ProgressRow>> {
        let facts = self.facts(seat);
        let pos = Pos::new(self.st(), &facts, self.has_map(), imagined);
        let edge = self.edge();
        let mut out = Vec::new();
        for r in self.st().objectives().revealed.iter() {
            // A secret made public has no public requirement and no price: no card.
            let ObjectiveRef::Public(obj) = r else { continue };
            let obj = *obj;
            let stage = q::stage_of(obj);
            if let Some(p) = q::counting_progress(&pos, &edge, obj).or_else(|| q::remaining_position_progress(&pos, obj)) {
                out.push(ProgressRow {
                    objective: *r,
                    family: RowFamily::Count(p.family),
                    have: p.have as i64,
                    threshold: p.threshold as i64,
                    satisfied: p.satisfied(),
                    stage,
                });
                continue;
            }
            if q::cost_of(obj).is_none() {
                continue;
            }
            let c = q::bought_progress(&pos, obj)?;
            out.push(ProgressRow {
                objective: *r,
                family: RowFamily::Cost(c.family),
                have: c.have,
                threshold: c.target,
                satisfied: c.satisfied(),
                stage,
            });
        }
        Some(out)
    }

    /// Progress of the secrets in the bound seat's hand after `imagined` (`SeatObservation::
    /// held_secret_progress_imagining`); occurrence-based secrets are omitted. `None`: a secret the core cannot answer.
    #[must_use]
    pub fn my_secret_progress(&self, imagined: Imagined<'_>) -> Option<Vec<ProgressRow>> {
        let facts = self.facts(self.me());
        let pos = Pos::new(self.st(), &facts, self.has_map(), imagined);
        let mut out = Vec::new();
        for obj in self.my_secrets() {
            match q::secret_progress(&pos, *obj) {
                Ok(Some(p)) => out.push(ProgressRow {
                    objective: ObjectiveRef::Secret(*obj),
                    family: RowFamily::Count(p.family),
                    have: p.have as i64,
                    threshold: p.threshold as i64,
                    satisfied: p.satisfied(),
                    stage: None,
                }),
                Ok(None) => {}
                Err(q::Unsupported) => return None,
            }
        }
        Some(out)
    }

    /// Revealed public objectives whose fixed map-shaped requirement includes `system`
    /// (`Observed::objectives_implicating_system`), in reveal order. Empty without a map.
    #[must_use]
    pub fn objectives_implicating_system(&self, seat: Seat, system: System) -> SmallList<PublicObjective, 12> {
        let mut out = SmallList::new();
        if !self.has_map() {
            return out;
        }
        let core = self.st();
        for r in core.objectives().revealed.iter() {
            let ObjectiveRef::Public(obj) = r else { continue };
            let implicated = match obj {
                PublicObjective::IntimidateCouncil => {
                    board::neighbours(core, q::mecatol_in_galaxy(core)).is_some_and(|set| set.contains(system.index()))
                }
                PublicObjective::PopulateTheOuterRim | PublicObjective::ControlTheBorderlands => {
                    let home = q::home_system_of(core, seat);
                    q::edge_systems(core).contains(system.index()) && home != Some(system)
                }
                _ => false,
            };
            if implicated {
                out.push(*obj);
            }
        }
        out
    }

    /// How many revealed public objectives the seat could score right now (`Observed::scoreable_public`): none while a
    /// home planet is lost (61.16, unless the faction scores without its home), a bought objective when it can be
    /// afforded, a position objective when its requirement is met, a leaked secret by its position requirement.
    /// `None`: a secret the core cannot answer.
    #[must_use]
    pub fn scoreable_public(&self, seat: Seat) -> Option<usize> {
        let core = self.st();
        let Some(player) = core.players().by_seat(seat) else { return Some(0) };
        let scores_without_home = self.kernel().hooks.strategy.scores_without_home(core, seat);
        let home_held = player.home_planets.as_slice().iter().all(|p| board::controls(core, seat, *p));
        if !(scores_without_home || home_held) {
            return Some(0);
        }
        let scored = self.scored_by(seat);
        let facts = self.facts(seat);
        let pos = Pos::new(core, &facts, self.has_map(), Imagined::NONE);
        let edge = self.edge();
        let mut n = 0;
        for r in core.objectives().revealed.iter() {
            match r {
                ObjectiveRef::Public(obj) => {
                    if scored.public.contains(*obj) {
                        continue;
                    }
                    let met = if q::cost_of(*obj).is_none() {
                        q::counting_progress(&pos, &edge, *obj)
                            .or_else(|| q::remaining_position_progress(&pos, *obj))
                            .is_some_and(|p| p.satisfied())
                    } else {
                        q::bought_progress(&pos, *obj).is_some_and(|c| c.satisfied())
                    };
                    n += usize::from(met);
                }
                ObjectiveRef::Secret(obj) => {
                    if scored.secret.contains(*obj) {
                        continue;
                    }
                    match q::secret_progress(&pos, *obj) {
                        Ok(Some(p)) => n += usize::from(p.satisfied()),
                        Ok(None) => {}
                        Err(q::Unsupported) => return None,
                    }
                }
            }
        }
        Some(n)
    }

    /// The value of a planet to spend (re-exported helper of the economy for the progress tests).
    #[must_use]
    pub fn spendable_total(&self, seat: Seat, kind: economy::Spend) -> i64 {
        economy::spendable_total(self.st(), self.scope(), self.blanked(), seat, kind)
    }
}
