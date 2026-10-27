//! P5, the observer: synchronous bookkeeping at a program point of a window. Not a reaction (no window, no
//! decision, no frequency), so it never goes through the `Resolver`.

use crate::containers::SeatSet;
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::Cx;
use crate::state::Piece;

/// Before or after the shared code at the program point ("run BEFORE shared code" of the string hooks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    Before,
    After,
}

/// One program point, with the payload the string hook receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Obs {
    ShipLost { system: System, piece: Piece },
    ShipRetreated { system: System, to: System, owner: Seat },
    RetreatsCleared { system: System },
    CombatRoundStarted { system: System, round: u32, who: SeatSet },
    CombatEnded { system: System },
    GroundRoundStarted { system: System, planet: Planet, round: u32 },
    GroundRoundEnded { system: System, planet: Planet, round: u32 },
    ShipPlaced { who: Seat, producing: System, target: System, unit: Unit },
    /// Replaces `argent_clear_agent_production_destinations` (7 call sites) and `keleres_close_agent_window` (4).
    ProductionWindowClosed { who: Seat, system: System },
    ControlGained { who: Seat, system: System, planet: Planet },
    Replenished { who: Seat },
}

/// The kind of [`Obs`], for the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObsKind {
    ShipLost,
    ShipRetreated,
    RetreatsCleared,
    CombatRoundStarted,
    CombatEnded,
    GroundRoundStarted,
    GroundRoundEnded,
    ShipPlaced,
    ProductionWindowClosed,
    ControlGained,
    Replenished,
}

impl ObsKind {
    pub const COUNT: usize = 11;
}

impl Obs {
    #[must_use]
    pub const fn kind(&self) -> ObsKind {
        match self {
            Self::ShipLost { .. } => ObsKind::ShipLost,
            Self::ShipRetreated { .. } => ObsKind::ShipRetreated,
            Self::RetreatsCleared { .. } => ObsKind::RetreatsCleared,
            Self::CombatRoundStarted { .. } => ObsKind::CombatRoundStarted,
            Self::CombatEnded { .. } => ObsKind::CombatEnded,
            Self::GroundRoundStarted { .. } => ObsKind::GroundRoundStarted,
            Self::GroundRoundEnded { .. } => ObsKind::GroundRoundEnded,
            Self::ShipPlaced { .. } => ObsKind::ShipPlaced,
            Self::ProductionWindowClosed { .. } => ObsKind::ProductionWindowClosed,
            Self::ControlGained { .. } => ObsKind::ControlGained,
            Self::Replenished { .. } => ObsKind::Replenished,
        }
    }
}

/// An observer may change the state; it never emits an event (an emit stays in the window).
pub type Observer = fn(&mut Cx<'_>, &Obs);

#[derive(Clone)]
pub struct Observers {
    before: [Vec<Observer>; ObsKind::COUNT],
    after: [Vec<Observer>; ObsKind::COUNT],
}

impl Default for Observers {
    fn default() -> Self {
        Self { before: std::array::from_fn(|_| Vec::new()), after: std::array::from_fn(|_| Vec::new()) }
    }
}

impl Observers {
    #[must_use]
    pub fn len(&self) -> usize {
        self.before.iter().chain(&self.after).map(Vec::len).sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, kind: ObsKind, phase: Phase, f: Observer) {
        match phase {
            Phase::Before => self.before[kind as usize].push(f),
            Phase::After => self.after[kind as usize].push(f),
        }
    }

    #[must_use]
    pub fn of(&self, kind: ObsKind, phase: Phase) -> &[Observer] {
        match phase {
            Phase::Before => &self.before[kind as usize],
            Phase::After => &self.after[kind as usize],
        }
    }
}

impl Cx<'_> {
    /// Fire the observers of `obs` at `phase`, in registration order.
    pub fn observe(&mut self, obs: &Obs, phase: Phase) {
        let kernel = self.kernel;
        for f in kernel.hooks.seam.observers.of(obs.kind(), phase) {
            f(self, obs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{FirstOption, Kernel};
    use crate::state::CoreState;

    #[test]
    fn observers_run_in_registration_order_at_their_phase() {
        let mut k = Kernel::plain();
        let o = &mut k.hooks.seam.observers;
        o.push(ObsKind::Replenished, Phase::After, |cx, _| cx.state.meta_mut().combat_round_seq += 1);
        o.push(ObsKind::Replenished, Phase::After, |cx, _| cx.state.meta_mut().combat_round_seq *= 10);
        o.push(ObsKind::Replenished, Phase::Before, |cx, _| cx.state.meta_mut().combat_round_seq = 5);
        let mut s = CoreState::all_loaded();
        let mut asker = FirstOption;
        let mut cx = Cx::new(&mut s, &k, &mut asker);
        let ev = Obs::Replenished { who: Seat(0) };
        cx.observe(&ev, Phase::Before);
        cx.observe(&ev, Phase::After);
        assert_eq!(cx.state.meta().combat_round_seq, 60);
        cx.observe(&Obs::CombatEnded { system: System::MecatolRex112 }, Phase::After);
        assert_eq!(cx.state.meta().combat_round_seq, 60);
    }
}
