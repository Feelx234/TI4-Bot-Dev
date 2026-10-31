//! Assault Cannon (string `CombatWindow::assault_cannon`), as a [`Window`]: "At the start of a space combat
//! in a system that contains 3 or more of your non-fighter ships, your opponent must destroy 1 of their
//! non-fighter ships."
//!
//! Both sides' eligibility is read before either loss (`Start`); then, per holder in attacker, defender
//! order, the victim picks the ship to lose (`Choosing`/`Asking`; a single candidate is lost without a
//! question) and the loss is announced as `SHIP_DESTROYED` with the cause `technology:assault_cannon`.
//!
//! Bail (adapter returns `None`, the string result stands): a neutral victim (its casualty order is the
//! reference card's, string-only) and reaction activity inside the loss announcement.

use super::assign::{announce_loss, remove_ship};
use super::{casualty_opts, ships_of, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{System, Technology};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, Piece};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Choosing,
    Asking,
    Done,
}

/// The facts of one opening of Assault Cannon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    /// The interned subtype text (`"assault_cannon_destroy"`).
    pub subtype: Code,
    /// The interned cause (`"technology:assault_cannon"`).
    pub cause: Code,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AssaultCannon {
    pub f: Facts,
    /// `(holder, victim)` in firing order, fixed at `Start`.
    firing: [Option<(Seat, Seat)>; 2],
    at: u8,
    stage: Stage,
}

impl AssaultCannon {
    #[must_use]
    pub const fn new(f: Facts) -> Self {
        Self { f, firing: [None; 2], at: 0, stage: Stage::Start }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn current(&self) -> Option<(Seat, Seat)> {
        self.firing.get(usize::from(self.at)).copied().flatten()
    }

    /// `non_fighter_ships(victim)`: the ships (planet-standing combatants included) that are not fighters.
    fn targets(&self, s: &CoreState, k: &Kernel, victim: Seat) -> Vec<Piece> {
        let rd = Self::rd(s, k);
        ships_of(rd, victim, self.f.system).into_iter().filter(|p| !rd.is_fighter(p.kind())).collect()
    }

    fn lose(&mut self, sx: &mut Sx<'_>, victim: Seat, piece: Piece) {
        remove_ship(sx, self.f.system, victim, piece);
        announce_loss(sx, self.f.system, victim, piece.kind(), self.f.cause, true, self.f.announce);
        self.at += 1;
    }
}

impl Window for AssaultCannon {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Start => {
                    let (a, d) = (self.f.attacker, self.f.defender);
                    let mut firing: SmallList<(Seat, Seat), 2> = SmallList::new();
                    for (holder, victim) in [(a, d), (d, a)] {
                        let owns = reach::technology_text(sx.state, holder, Technology::AssaultCannon);
                        if owns && self.targets(sx.state, sx.kernel, holder).len() >= 3 {
                            firing.push((holder, victim));
                        }
                    }
                    for (slot, pair) in self.firing.iter_mut().zip(firing.iter()) {
                        *slot = Some(*pair);
                    }
                    self.stage = Stage::Choosing;
                }
                Stage::Choosing => {
                    let Some((_, victim)) = self.current() else {
                        self.stage = Stage::Done;
                        continue;
                    };
                    let targets = self.targets(sx.state, sx.kernel, victim);
                    match targets.as_slice() {
                        [] => self.at += 1,
                        [only] => {
                            let only = *only;
                            self.lose(sx, victim, only);
                        }
                        _ if super::is_neutral(sx.state, victim) => {
                            let doomed = super::neutral_pick(&targets).unwrap_or(targets[0]);
                            self.lose(sx, victim, doomed);
                        }
                        _ => self.stage = Stage::Asking,
                    }
                }
                Stage::Asking => return Next::Continue(Ask::Combat(CAsk::AssignHit)),
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let (_, victim) = self.current()?;
        Some(Choice {
            actor: victim,
            ask: Ask::Combat(CAsk::AssignHit),
            ctx: Ctx {
                origin: Origin::Content(Content::Technology(Technology::AssaultCannon)),
                subtype: Subtype::Combat(CSub::Casualty(self.f.subtype)),
                optional: false,
                target: Some(Target::System(self.f.system)),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: casualty_opts(&self.targets(s, k, victim)),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let (_, victim) = self.current().ok_or(Illegal)?;
        let Pick::Opt(Opt::Combat(COpt::Destroy { index, .. })) = pick else { return Err(Illegal) };
        let piece = self.targets(sx.state, sx.kernel, victim).get(usize::from(index)).copied().ok_or(Illegal)?;
        self.lose(sx, victim, piece);
        self.stage = Stage::Choosing;
        Ok(())
    }
}
