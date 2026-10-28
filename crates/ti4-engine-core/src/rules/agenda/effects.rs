//! The effect of a passed agenda as a [`Window`] (package B9; string `agenda_effects::resolve_with`).
//!
//! Ported effects (they ask nothing, so the window is one step):
//! * Political Censure, Shard of the Throne, Crown of Emphidia: the elected player gains a victory point,
//!   capped at the target (`agenda_effects::effect_censure`).
//! * Classified Document Leaks: the elected scored secret objective becomes a public one
//!   (`agenda_effects::effect_classified`).
//!
//! * A law leaving play (`laws::repeal`): the law is removed; Political Censure's holder loses its point.
//!
//! Not ported, so the adapter runs the string effect alone (documented bails of the dual): every other agenda
//! effect; here an outcome that is not what the effect elects (a card with `For`/`Against` on the table).

use crate::choice::{Choice, Pick};
use crate::generated::{Agenda, SecretObjective};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::{LedgerReason, Outcome};
use crate::state::{CoreState, ObjectiveRef, VpRec};

/// `objectives::VICTORY_TARGET`.
pub const VICTORY_TARGET: i32 = 10;

/// What the effect does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The elected player gains a point (`agenda` is the ledger reason).
    Point(Agenda, Seat),
    /// The elected secret objective becomes public.
    Reveal(SecretObjective),
    /// The law leaves play.
    Repeal(Agenda),
}

/// How the effect ended (`agenda_effects::Effect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Apply,
    Done,
}

/// One agenda effect.
#[derive(Debug, Clone)]
pub struct Resolve {
    kind: Kind,
    stage: Stage,
    /// A repeal found the law in play.
    repealed: bool,
}

impl Resolve {
    /// The effect of `agenda` electing `outcome`; `None` when the core has no such effect.
    #[must_use]
    pub const fn of(agenda: Agenda, outcome: Outcome) -> Option<Self> {
        let kind = match (agenda, outcome) {
            (Agenda::PoliticalCensure | Agenda::ShardOfTheThrone | Agenda::TheCrownOfEmphidia, Outcome::Seat(who)) => Kind::Point(agenda, who),
            (Agenda::ClassifiedDocumentLeaks, Outcome::Secret(s)) => Kind::Reveal(s),
            _ => return None,
        };
        Some(Self { kind, stage: Stage::Apply, repealed: false })
    }

    /// `laws::repeal` of `law`.
    #[must_use]
    pub const fn repeal(law: Agenda) -> Self {
        Self { kind: Kind::Repeal(law), stage: Stage::Apply, repealed: false }
    }

    /// Whether the repealed law was in play.
    #[must_use]
    pub const fn repealed(&self) -> bool {
        self.repealed
    }

    /// `agenda_effects::adjust_victory_points` with +1: capped at the target, the ledger row records the
    /// change that happened (zero included).
    fn point(core: &mut CoreState, agenda: Agenda, who: Seat, delta: i32) {
        let Some(p) = core.players_mut().by_seat_mut(who) else { return };
        let before = i32::from(p.econ.victory_points);
        let after = (before + delta).clamp(0, VICTORY_TARGET);
        p.econ.victory_points = i16::try_from(after).unwrap_or(0);
        core.ledger_mut().vp.push(VpRec { player: who, delta: after - before, reason: LedgerReason::Agenda(agenda) });
    }
}

impl Window for Resolve {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Apply {
            match self.kind {
                Kind::Point(agenda, who) => Self::point(sx.state, agenda, who, 1),
                Kind::Repeal(law) => {
                    let owner = sx.state.agenda_mut().laws.remove(&law);
                    self.repealed = owner.is_some();
                    if let (Agenda::PoliticalCensure, Some(Outcome::Seat(holder))) = (law, owner) {
                        Self::point(sx.state, law, holder, -1);
                    }
                }
                Kind::Reveal(secret) => {
                    let card = ObjectiveRef::Secret(secret);
                    let revealed = &mut sx.state.objectives_mut().revealed;
                    if !revealed.as_slice().contains(&card) {
                        revealed.push(card);
                    }
                }
            }
            self.stage = Stage::Done;
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_ported_effects_open_a_window() {
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::Seat(Seat(0))).is_some());
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::For).is_none());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::Secret(SecretObjective::ALL[0])).is_some());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::For).is_none());
        assert!(!Resolve::repeal(Agenda::PoliticalCensure).repealed());
    }
}
