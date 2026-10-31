//! The opening of a space combat round (string `CombatWindow::begin_round`), as a [`Window`] that never asks.
//!
//! The round counter and the presentation are set, last round's hit and dice records are cleared, round 1
//! keeps the barrage's starting fleet, and `SPACE_COMBAT_STARTED` (round 1) and `COMBAT_ROUND_STARTED` are
//! announced. In round 1 a ground-form dual-form unit in the space area takes its ship form ("at the start of a space combat",
//! the Naaz Eidolon; [`BeginRound::flipped`] tells the machine, which adds the forms to the fleet of the start), and after the
//! announcement the faction effects of the start of a round run for each participant (the hook `space_combat_round_started`:
//! Naaz repairs the Eidolon Maximum and offers Supercharge; the question rides on the tape of the step).
//!
//! Not covered (the adapter returns `None`, the string result stands; see `dual_window::begin_round`):
//! diplomacy enabled (the attack-promise evaluation) and a Munitions Reserves holder below rules version 8 (the payment is asked at the opening).
//! Arc Secundus (and a Nekro flagship lent its text) repairs itself after the hooks of each participant (package W2).

use super::{ships_of, Rd};
use crate::choice::{Choice, Pick};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

/// `ti4_model::state::RULES_FACTION_BUGFIXES`: from this version the Letnev agent listens to its owner's rounds.
const RULES_FACTION_BUGFIXES: u32 = 17;

/// The string's `borrowed_round_agents::has_round_copy(.., "letnevagent") || has_owner_space_round_agent(..)` on the core state: a
/// Letnev seat holds Viscount Unlenn readied (rules version 17 and later), or a Yssaril seat holds Ssruu readied while another seat
/// holds Viscount Unlenn, readied or exhausted. The hook table is not asked: the combat kernel carries no such slot.
#[must_use]
pub fn letnev_round_agent_usable(s: &CoreState) -> bool {
    use crate::generated::{Faction, Leader};
    use crate::state::LeaderStatus;
    let status = |p: &crate::state::Player, card: Leader| p.leaders.status.get(&card).copied();
    let owner = s.meta().rules_at_least(RULES_FACTION_BUGFIXES)
        && s.players().iter().any(|p| p.faction == Faction::TheBaronyOfLetnev && status(p, Leader::ViscountUnlenn) == Some(LeaderStatus::Readied));
    owner
        || s.players().iter().any(|b| {
            b.faction == Faction::TheYssarilTribes
                && status(b, Leader::Ssruu) == Some(LeaderStatus::Readied)
                && s.players().iter().any(|l| l.seat != b.seat && matches!(status(l, Leader::ViscountUnlenn), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Done,
}

/// The facts of one round opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub round: u32,
    /// The interned presentation phase (`"pre_roll"`).
    pub phase: Code,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// A round agent (Letnev's own, or a Yssaril copy of it) listens: `COMBAT_ROUND_STARTED` carries `round_seq`.
    pub round_seq_listens: bool,
}

#[derive(Debug, Clone)]
pub struct BeginRound {
    pub f: Facts,
    /// The dual-form units that took their ship form at the start of the combat: `(owner, new form)`.
    pub flipped: Vec<(Seat, crate::generated::Unit)>,
    stage: Stage,
}

impl BeginRound {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, flipped: Vec::new(), stage: Stage::Start }
    }
}

/// `combat::repair_self_repairing`: Arc Secundus (or a Nekro flagship lent its text by the Z token) repairs itself at the start of each
/// space combat round, after the faction hooks of the participant.
fn repair_self_repairing(s: &mut CoreState, system: System, who: Seat) {
    let repair: Vec<usize> = s
        .board()
        .sys(system)
        .units
        .as_slice()
        .iter()
        .enumerate()
        .filter(|(_, u)| u.owner() == who && u.sustained() && super::flagship_has_text(s, who, u.kind(), super::FlagshipText::Letnev))
        .map(|(i, _)| i)
        .collect();
    let units = s.board_mut().sys_mut(system).units.as_mut_slice();
    for at in repair {
        units[at] = units[at].with_sustained(false);
    }
}

impl Window for BeginRound {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Done {
            return Next::Done;
        }
        let f = self.f;
        sx.state.meta_mut().combat_round_seq = sx.state.meta().combat_round_seq.saturating_add(1);
        let scope = Rd::new(sx.state, sx.kernel).scope();
        let rd = Rd::new(sx.state, sx.kernel).with_blanked(reach::blanked(sx.state));
        let mut round_start = ships_of(rd, f.attacker, f.system);
        round_start.extend(ships_of(rd, f.defender, f.system));
        let combat = &mut sx.state.transient_mut().combat;
        combat.combat_round_hits.clear();
        combat.combat_round_dice = Default::default();
        let p = combat.presentation.edit();
        p.phase = f.phase;
        p.round = f.round;
        p.remaining_hits.clear();
        if f.round == 1 {
            p.barrage_start = round_start.clone();
            p.barrage_hits.clear();
            p.barrage_dice.clear();
        }
        p.round_start = round_start;
        if f.round == 1 {
            // "At the start of a space combat" a ground-form dual-form unit in the space area takes its ship form, so it fights
            // (`fleet::flip_to_ship_forms`, before the announcement).
            self.flipped = crate::kernel::fx_strategy::flip_space_area(sx.state, scope, &[f.attacker, f.defender], f.system, true);
        }
        let base = Payload::new()
            .with(Key::System, Val::System(f.system))
            .with(Key::Attacker, Val::Seat(f.attacker))
            .with(Key::Defender, Val::Seat(f.defender))
            .with(Key::Round, Val::Int(i64::from(f.round)));
        if f.announce {
            if f.round == 1 {
                let _ = sx.emit(fx::event(EventKind::SpaceCombatStarted, base.clone().with(Key::Player, Val::Seat(f.attacker))));
            }
            let announced = if f.round_seq_listens {
                let seq = i64::from(sx.state.meta().combat_round_seq);
                base.with(Key::RoundSeq, Val::Int(seq))
            } else {
                base
            };
            let _ = sx.emit(fx::event(EventKind::CombatRoundStarted, announced));
        }
        // The faction offers made at the round's opening window, before any dice, for each participant.
        for who in [f.attacker, f.defender] {
            let (k, mut cx) = (sx.kernel, sx.cx());
            k.hooks.general.space_combat_round_started(&mut cx, who);
            repair_self_repairing(sx.state, f.system, who);
        }
        self.stage = Stage::Done;
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
