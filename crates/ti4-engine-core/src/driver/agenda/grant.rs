//! Research Grant Reallocation (KV3; string `agenda_effects::effect_grant_reallocation`): "The elected player gains any 1 technology of
//! their choice. Then, for each prerequisite on that technology, they remove 1 token from their fleet pool and return it to their
//! reinforcements." A driver-level window: it composes the technology question of the relics (`rules::relic_windows::GainTechnology`),
//! which an area window may not import.

use crate::choice::{Choice, Pick};
use crate::generated::Technology;
use crate::ids::Seat;
use crate::kernel::agenda_codes::AgendaCodes;
use crate::kernel::fx_strategy::apply_unit_upgrades;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::query::tech;
use crate::query::tech::UnitRoster;
use crate::rules::agenda::effects::placements::spend_fleet;
use crate::rules::agenda::reveal::RULES_AGENDA_GRANT_REVEAL;
use crate::rules::relic_windows::GainTechnology;
use crate::state::CoreState;
use crate::tables::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GStage {
    Start,
    Asking,
    Done,
}

/// The grant to `who`.
#[derive(Debug, Clone)]
pub struct Grant {
    who: Seat,
    scope: Scope,
    roster: Option<&'static UnitRoster>,
    codes: AgendaCodes,
    stage: GStage,
    gain: Option<GainTechnology>,
    /// The roster was needed and missing (the unit upgrades of the technology could not be applied).
    pub unsupported: bool,
}

impl Grant {
    #[must_use]
    pub fn new(who: Seat, scope: Scope, roster: Option<&'static UnitRoster>, codes: AgendaCodes) -> Self {
        Self { who, scope, roster, codes, stage: GStage::Start, gain: None, unsupported: false }
    }

    /// The technologies a seat may be granted: those it does not hold, of the game's sources, its faction's own among the faction ones
    /// (90.11), in the order of the corpus.
    fn grantable(&self, core: &CoreState) -> Vec<Technology> {
        let Some(p) = core.players().by_seat(self.who) else { return Vec::new() };
        let mut out: Vec<Technology> = Technology::ALL
            .iter()
            .copied()
            .filter(|t| self.scope.0 & crate::tables::source_bit(t.data().source) != 0)
            .filter(|t| t.data().faction.is_none_or(|tag| tech::tag_is(tag, p.faction)))
            .filter(|t| !p.tech.owned.contains(*t))
            .collect();
        out.sort_by(|a, b| (self.codes.tech_corpus)(*a, *b));
        out
    }
}

impl Window for Grant {
    type Stage = GStage;

    fn stage(&self) -> GStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == GStage::Start {
            if sx.state.players().by_seat(self.who).is_none() {
                self.stage = GStage::Done;
                return Next::Done;
            }
            if !sx.state.meta().rules_at_least(RULES_AGENDA_GRANT_REVEAL) {
                // Before rules version 14 no technology was granted and a single fleet token was removed whatever it would have cost.
                spend_fleet(sx.state, self.who, 1);
                self.stage = GStage::Done;
                return Next::Done;
            }
            let options = self.grantable(sx.state);
            if options.is_empty() {
                self.stage = GStage::Done;
                return Next::Done;
            }
            self.gain = Some(GainTechnology::from_source(self.who, self.codes.grant, &options));
            self.stage = GStage::Asking;
        }
        let Some(gain) = self.gain.as_mut() else { return Next::Done };
        if let Next::Continue(ask) = gain.step(sx) {
            return Next::Continue(ask);
        }
        // Gained, not researched: no prerequisites needed, nothing that fires on research.
        if let Some(chosen) = gain.chosen
            && fx::grant_technology(sx.state, self.who, chosen)
        {
            match self.roster {
                Some(roster) => apply_unit_upgrades(sx.state, sx.kernel, self.scope, roster, self.who),
                None => self.unsupported = true,
            }
            let cost: usize = tech::prerequisites(chosen).iter().map(|n| usize::from(*n)).sum();
            spend_fleet(sx.state, self.who, cost);
        }
        self.gain = None;
        self.stage = GStage::Done;
        Next::Done
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.gain.as_ref().and_then(|g| g.ask_of(s, k))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.gain.as_mut().ok_or(Illegal)?.answer(sx, pick)
    }
}
