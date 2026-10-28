//! Raid Formation (Argent; string `argent::afb_excess`), as a [`Window`]: for each barrage hit beyond the
//! target's fighters, the producer chooses which of the target's undamaged ships with SUSTAIN DAMAGE
//! becomes damaged (by type; two undamaged ships of one type are interchangeable). One candidate type is
//! taken without a question; none ends the ability.

use super::{sustain_suppressed, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Ability, System, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Choosing,
    Asking,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub producer: Seat,
    pub target: Seat,
    pub system: System,
    pub excess: u16,
    /// The interned subtype text (`"raid_formation_damage"`).
    pub subtype: Code,
}

#[derive(Debug, Clone, Copy)]
pub struct RaidFormation {
    pub f: Facts,
    left: u16,
    stage: Stage,
}

impl RaidFormation {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, left: f.excess, stage: Stage::Choosing }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// `damageable_types`: undamaged ships of the target in the space area that have SUSTAIN DAMAGE, by type.
    fn candidates(&self, s: &CoreState, k: &Kernel) -> Vec<Unit> {
        let rd = Self::rd(s, k);
        let grants = crate::kernel::seam::data::grants_sustain(rd.q(), &rd.k.hooks.seam.rows, self.f.target);
        let mut found: Vec<Unit> = Vec::new();
        for p in s.board().sys(self.f.system).units.iter() {
            if p.owner() != self.f.target || p.sustained() || !rd.is_ship(p.kind()) {
                continue;
            }
            let Some(data) = rd.data(p.kind()) else { continue };
            let has_sustain = data.sustain_damage || (grants && !rd.is_fighter(p.kind()));
            if has_sustain && !sustain_suppressed(s, rd.blanked, data.base_type) && !found.contains(&p.kind()) {
                found.push(p.kind());
            }
        }
        found
    }

    fn damage(&mut self, sx: &mut Sx<'_>, unit: Unit) {
        let target = self.f.target;
        let units = &mut sx.state.board_mut().sys_mut(self.f.system).units;
        if let Some(at) = units.iter().position(|p| p.owner() == target && !p.sustained() && p.kind() == unit) {
            let marked = units.as_slice()[at].with_sustained(true);
            units.as_mut_slice()[at] = marked;
        }
        self.left -= 1;
    }
}

impl Window for RaidFormation {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Choosing => {
                    if self.left == 0 {
                        self.stage = Stage::Done;
                        continue;
                    }
                    let found = self.candidates(sx.state, sx.kernel);
                    match found.as_slice() {
                        [] => self.stage = Stage::Done,
                        [only] => {
                            let only = *only;
                            self.damage(sx, only);
                        }
                        _ => self.stage = Stage::Asking,
                    }
                }
                Stage::Asking => return Next::Continue(Ask::Combat(CAsk::RaidDamage { target: self.f.target })),
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.f.producer,
            ask: Ask::Combat(CAsk::RaidDamage { target: self.f.target }),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Ability(Ability::RaidFormation)),
                subtype: Subtype::Combat(CSub::Casualty(self.f.subtype)),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: Opts(self.candidates(s, k).into_iter().map(|unit| Opt::Combat(COpt::RaidDamage { unit })).collect()),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(Opt::Combat(COpt::RaidDamage { unit })) = pick else { return Err(Illegal) };
        if !self.candidates(sx.state, sx.kernel).contains(&unit) {
            return Err(Illegal);
        }
        self.damage(sx, unit);
        self.stage = Stage::Choosing;
        Ok(())
    }
}
