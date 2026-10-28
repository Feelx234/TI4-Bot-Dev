//! The effect of a passed agenda as a [`Window`] (package B9; string `agenda_effects::resolve_with`).
//!
//! Ported effects (they ask nothing, so the window is one step):
//! * Political Censure, Shard of the Throne, Crown of Emphidia: the elected player gains a victory point,
//!   capped at the target (`agenda_effects::effect_censure`).
//! * Classified Document Leaks: the elected scored secret objective becomes a public one
//!   (`agenda_effects::effect_classified`).
//!
//! * Standing and availability-only arms (the Ministers, the Research Teams, ..., Representative Government,
//!   Revolution on For): the card does nothing when it passes.
//! * Anti-Intellectual Revolution and Representative Government (Against), Miscount Disclosed (a law
//!   repealed): `Done::Deferred`, as the string effect defers what needs a question.
//! * Judicial Abolishment (the elected law leaves play), New Constitution (every law, on For), Mutiny (a point
//!   for or against each For voter), Holy Planet of Ixth (the controller of the elected planet gains a point).
//! * Nexus Sovereignty: Against puts a gamma wormhole token into Mecatol Rex (For is a standing rule).
//! * A law leaving play (`laws::repeal`): the law is removed; Political Censure's holder loses its point.
//!
//! Not ported, so the adapter runs the string effect alone (documented bails of the dual): every other agenda
//! effect; here an outcome that is not what the effect elects (a card with `For`/`Against` on the table).

use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, SecretObjective, System, Wormhole};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::{LedgerReason, Outcome};
use crate::state::{CoreState, ObjectiveRef, SysFlags, VpRec};

/// `objectives::VICTORY_TARGET`.
pub const VICTORY_TARGET: i32 = 10;

/// What the effect does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The elected player gains a point (`agenda` is the ledger reason).
    Point(Agenda, Seat),
    /// The elected secret objective becomes public.
    Reveal(SecretObjective),
    /// The law leaves play.
    Repeal(Agenda),
    /// New Constitution: every law leaves play.
    RepealAll,
    /// Mutiny: each voter (in the string ballot's order) gains `delta` points.
    Voters(Agenda, i32, SmallList<Seat, 8>),
    /// Holy Planet of Ixth: whoever controls the planet gains a point.
    Controller(Agenda, Planet),
    /// Nexus Sovereignty, Against: a gamma wormhole token in the Mecatol Rex system of the map.
    Gamma,
    /// The card does nothing when it passes.
    Nothing,
}

/// How the effect ended (`agenda_effects::Effect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    Resolved,
    /// The effect needs a question this path cannot ask (`Effect::Deferred`).
    Deferred,
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
    done: Done,
}

impl Resolve {
    /// The effect of `agenda` electing `outcome`; `None` when the core has no such effect.
    #[must_use]
    pub fn of(agenda: Agenda, outcome: Outcome, for_voters: &[Seat]) -> Option<Self> {
        use Agenda as A;
        let mut done = Done::Resolved;
        let kind = match (agenda, outcome) {
            (A::PoliticalCensure | A::ShardOfTheThrone | A::TheCrownOfEmphidia, Outcome::Seat(who)) => Kind::Point(agenda, who),
            (A::ClassifiedDocumentLeaks, Outcome::Secret(s)) => Kind::Reveal(s),
            (A::JudicialAbolishment, Outcome::Law(law)) => Kind::Repeal(law),
            (A::MiscountDisclosed, Outcome::Law(law)) => {
                done = Done::Deferred;
                Kind::Repeal(law)
            }
            (A::NewConstitution, Outcome::For) => Kind::RepealAll,
            (A::NewConstitution, _) => Kind::Nothing,
            (A::Mutiny, Outcome::For | Outcome::Against) => {
                Kind::Voters(agenda, if outcome == Outcome::For { 1 } else { -1 }, SmallList::from_slice(for_voters))
            }
            (A::NexusSovereignty, Outcome::Against) => Kind::Gamma,
            (A::NexusSovereignty, Outcome::For) => Kind::Nothing,
            (A::HolyPlanetOfIxth, Outcome::Planet(p)) => Kind::Controller(agenda, p),
            (A::AntiIntellectualRevolution | A::RepresentativeGovernmentBaseGame, Outcome::For) => Kind::Nothing,
            (A::AntiIntellectualRevolution | A::RepresentativeGovernmentBaseGame, Outcome::Against) => {
                done = Done::Deferred;
                Kind::Nothing
            }
            (
                A::RepresentativeGovernment
                | A::ResearchTeamBiotic
                | A::ResearchTeamCybernetic
                | A::ResearchTeamPropulsion
                | A::ResearchTeamWarfare
                | A::SenateSanctuary
                | A::TerraformingInitiative
                | A::MinisterOfExploration
                | A::MinisterOfPolicy
                | A::CommitteeFormation
                | A::ImperialArbiter
                | A::MinisterOfCommerce
                | A::MinisterOfIndustry
                | A::MinisterOfPeace
                | A::MinisterOfSciences
                | A::MinisterOfWar
                | A::ProphecyOfIxth
                | A::TheCrownOfThalnos
                | A::ChecksAndBalances
                | A::CovertLegislation,
                _,
            ) => Kind::Nothing,
            _ => return None,
        };
        Some(Self { kind, stage: Stage::Apply, repealed: false, done })
    }

    /// `laws::repeal` of `law`.
    #[must_use]
    pub const fn repeal(law: Agenda) -> Self {
        Self { kind: Kind::Repeal(law), stage: Stage::Apply, repealed: false, done: Done::Resolved }
    }

    /// Whether the repealed law was in play.
    #[must_use]
    pub const fn repealed(&self) -> bool {
        self.repealed
    }

    /// How the effect ended.
    #[must_use]
    pub const fn done(&self) -> Done {
        self.done
    }

    /// `laws::repeal`: the law leaves play; Political Censure's holder loses its point.
    fn leave(core: &mut CoreState, law: Agenda) -> bool {
        let owner = core.agenda_mut().laws.remove(&law);
        if let (Agenda::PoliticalCensure, Some(Outcome::Seat(holder))) = (law, owner) {
            Self::point(core, law, holder, -1);
        }
        owner.is_some()
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
            match &self.kind {
                Kind::Point(agenda, who) => Self::point(sx.state, *agenda, *who, 1),
                Kind::Repeal(law) => {
                    self.repealed = Self::leave(sx.state, *law);
                    // Miscount Disclosed with the law not in play: nothing was deferred.
                    if !self.repealed {
                        self.done = Done::Resolved;
                    }
                }
                Kind::RepealAll => {
                    let laws: Vec<Agenda> = sx.state.agenda().laws.iter().map(|(a, _)| *a).collect();
                    for law in laws {
                        Self::leave(sx.state, law);
                    }
                }
                Kind::Voters(agenda, delta, who) => {
                    for seat in who.as_slice() {
                        Self::point(sx.state, *agenda, *seat, *delta);
                    }
                }
                Kind::Controller(agenda, planet) => {
                    if let Some(owner) = crate::query::board::control(sx.state, *planet) {
                        Self::point(sx.state, *agenda, owner, 1);
                    }
                }
                Kind::Gamma => {
                    // `seating::mecatol_on`: the Thunder's Edge tile when the map has it, else the base one.
                    let placed = |tile: System| sx.state.board().systems().any(|(s, c)| s == tile && c.flags.has(SysFlags::TOUCHED));
                    let mecatol = if placed(System::MecatolRex112) { System::MecatolRex112 } else { System::MecatolRex18 };
                    sx.state.explore_mut().wormhole_tokens.insert(Wormhole::Gamma, mecatol);
                }
                Kind::Nothing => {}
                Kind::Reveal(secret) => {
                    let card = ObjectiveRef::Secret(*secret);
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
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::Seat(Seat(0)), &[]).is_some());
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::For, &[]).is_none());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::Secret(SecretObjective::ALL[0]), &[]).is_some());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::For, &[]).is_none());
        assert!(Resolve::of(Agenda::Mutiny, Outcome::Empty, &[]).is_none());
        assert!(!Resolve::repeal(Agenda::PoliticalCensure).repealed());
    }
}
