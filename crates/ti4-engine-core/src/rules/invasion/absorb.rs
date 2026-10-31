//! Assigning the hits of a ground combat to one side's ground forces (LRR 42, 87; the `while remaining > 0` loop of
//! the string `absorb_ground_with_origin`), as a [`Window`].
//!
//! Stages: `Choosing` takes the next hit: a ground force with SUSTAIN DAMAGE absorbs it (and the Non-Euclidean
//! Shielding extra) before anyone chooses; with no candidate the rest is wasted (15.2a); one candidate is lost
//! without a question; otherwise the owner is asked "assign a hit on {planet}" (`Asking`).
//!
//! A neutral owner's hit goes to the unit lowest on the reference card (rule 7): nobody is asked.

use super::{can_sustain_here, ground_hit_eligible, is_ground_force_here, Rd};
use crate::choice::generic::Rule;
use crate::choice::invasion::{Ask as IAsk, Opt as IOpt, Subtype as ISub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::seam::data as seam_data;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{CoreState, Piece};

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Choosing,
    Asking,
    Done,
}

/// `hits` hits on `player`'s ground forces on `planet`.
#[derive(Debug, Clone, Copy)]
pub struct AbsorbGround {
    pub player: Seat,
    pub system: System,
    pub planet: Planet,
    pub remaining: u16,
    pub unit_ability: bool,
    /// `supply::staging_enabled`: the options say whether a unit is damaged.
    pub staging: bool,
    stage: Stage,
}

impl AbsorbGround {
    #[must_use]
    pub const fn new(player: Seat, system: System, planet: Planet, hits: u16, unit_ability: bool, staging: bool) -> Self {
        Self { player, system, planet, remaining: hits, unit_ability, staging, stage: Stage::Choosing }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// The ground forces that may take the hit, in planet order.
    fn present(&self, s: &CoreState, k: &Kernel) -> Vec<Piece> {
        let rd = Self::rd(s, k);
        s.board()
            .planet(self.planet)
            .units
            .iter()
            .filter(|u| u.owner() == self.player)
            .filter(|u| is_ground_force_here(rd, self.player, self.system, self.planet, **u))
            .filter(|u| ground_hit_eligible(rd, self.system, self.planet, **u, self.unit_ability))
            .copied()
            .collect()
    }

    fn destroy(&self, s: &mut CoreState, unit: Piece) {
        let units = &mut s.board_mut().planet_mut(self.planet).units;
        if let Some(at) = units.iter().position(|u| *u == unit) {
            units.remove(at);
        }
    }

    fn opts(&self, present: &[Piece]) -> Opts {
        let mut opts = Opts::default();
        let mut seen: Vec<(crate::generated::Unit, bool)> = Vec::new();
        for (index, unit) in present.iter().enumerate() {
            let key = (unit.kind(), unit.sustained());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            opts.push(Opt::Invasion(IOpt::DestroyGround { index: index as u16, unit: unit.kind(), damaged: unit.sustained(), staging: self.staging }));
        }
        opts
    }

    fn ask(&self) -> IAsk {
        IAsk::AssignGroundHit { system: self.system, planet: self.planet, staging: self.staging }
    }
}

impl Window for AbsorbGround {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        while self.stage == Stage::Choosing {
            if self.remaining == 0 {
                self.stage = Stage::Done;
                break;
            }
            self.remaining -= 1;
            let rd = Self::rd(sx.state, sx.kernel);
            let sturdy = sx
                .state
                .board()
                .planet(self.planet)
                .units
                .iter()
                .filter(|u| u.owner() == self.player)
                .filter(|u| ground_hit_eligible(rd, self.system, self.planet, **u, self.unit_ability))
                .find(|u| can_sustain_here(rd, **u, self.system, self.planet))
                .copied();
            if let Some(unit) = sturdy {
                super::apply_ground_hit(sx.state, self.planet, super::GroundHit::Sustained(unit));
                let extra = match seam_data::cancel_on_sustain(Self::rd(sx.state, sx.kernel).q(), &sx.kernel.hooks.seam.rows, self.player) {
                    n if n > 0 => n as usize - 1,
                    _ => 0,
                };
                self.remaining = self.remaining.saturating_sub(u16::try_from(extra).unwrap_or(u16::MAX));
                continue;
            }
            let present = self.present(sx.state, sx.kernel);
            match present.as_slice() {
                [] => self.stage = Stage::Done, // 15.2a
                [only] => {
                    let only = *only;
                    self.destroy(sx.state, only);
                }
                // Nobody decides for a neutral side: the hit goes to the unit lowest on the reference card (rule 7).
                _ if !crate::query::seats::is_player(sx.state, self.player) => {
                    let doomed = crate::query::neutral::neutral_pick(&present).unwrap_or(present[0]);
                    self.destroy(sx.state, doomed);
                }
                _ => self.stage = Stage::Asking,
            }
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Invasion(self.ask())),
            Stage::Choosing | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.player,
            ask: Ask::Invasion(self.ask()),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R42),
                subtype: Subtype::Invasion(ISub::AssignGroundCasualty),
                optional: false,
                target: Some(Target::Planet { system: self.system, planet: self.planet }),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: self.opts(&self.present(s, k)),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(Opt::Invasion(IOpt::DestroyGround { index, .. })) = pick else { return Err(Illegal) };
        let present = self.present(sx.state, sx.kernel);
        let doomed = *present.get(usize::from(index)).or_else(|| present.first()).ok_or(Illegal)?;
        self.destroy(sx.state, doomed);
        self.stage = Stage::Choosing;
        Ok(())
    }
}
