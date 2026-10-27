//! Assigning the hits that sustain damage did not cancel (LRR 78.6; the `while remaining > 0` loop of the
//! string `absorb_hits_seeing_with_context`), as a [`Window`].
//!
//! Stages: `Choosing` computes who may take the next hit ([`hit_targets`]); nobody: the rest of the hits
//! have no effect (15.2a); one unit: it is lost without a question; otherwise the owner is asked "assign a
//! hit". `Removing` is the loss itself: Bastion's ship-lost note, the Maximum or an Alastor ground force
//! leaving its planet, the unit leaving the space area, and (with staging) the `SHIP_DESTROYED` event.
//!
//! Not covered (the adapter returns `None`): neutral players (the roster order of the reference card is
//! string-only) and producer-assigned hits.

use super::{hit_targets, planets_of_sorted, ships_of, casualty_opts, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, OwedKind, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::seam::{Obs, Phase};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Piece, Val};

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Choosing,
    Asking,
    Done,
}

/// The facts of one assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assign {
    pub player: Seat,
    pub system: System,
    /// The hit is a unit ability's.
    pub ability: bool,
    pub non_fighters_first: bool,
    /// The interned subtype text of the question (`"assign_casualty"`).
    pub subtype: Code,
    /// The interned cause of the loss (`"space_combat"`, ...), carried by `SHIP_DESTROYED`.
    pub cause: Code,
    pub during_space_combat: bool,
    /// `supply::staging_enabled`: the loss is announced.
    pub staging: bool,
    /// The resolver carries the timing machinery (`ctx.emit` announces); the windowless path does not.
    pub announce: bool,
}

/// The window: the facts and the hits still owed.
#[derive(Debug, Clone, Copy)]
pub struct AssignHits {
    pub a: Assign,
    pub remaining: u16,
    stage: Stage,
}

impl AssignHits {
    #[must_use]
    pub const fn new(a: Assign, remaining: u16) -> Self {
        Self { a, remaining, stage: Stage::Choosing }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn targets(&self, s: &CoreState, k: &Kernel) -> Vec<Piece> {
        hit_targets(Self::rd(s, k), self.a.player, self.a.system, self.a.ability, self.a.non_fighters_first)
    }

    /// `combat::remove_combat_ship` and the announcement of the loss.
    fn lose(&mut self, sx: &mut Sx<'_>, piece: Piece) {
        remove_ship(sx, self.a.system, self.a.player, piece);
        if self.a.staging {
            announce_loss(sx, self.a.system, self.a.player, piece.kind(), self.a.cause, self.a.during_space_combat, self.a.announce);
        }
        self.remaining = self.remaining.saturating_sub(1);
    }
}

/// `combat::remove_combat_ship`: Bastion's note, then the Maximum or an Alastor ground force leaving its
/// planet, or the unit leaving the space area.
pub fn remove_ship(sx: &mut Sx<'_>, system: System, owner: Seat, piece: Piece) {
    sx.observe(&Obs::ShipLost { system, piece }, Phase::After);
    let voltron = piece.kind() == Unit::EidolonMaximum;
    let alastor = !voltron && sx.kernel.hooks.direct.nekro_units_alastor_covers(sx.state, system, owner, piece.kind());
    let mut done = false;
    if voltron || alastor {
        let planets = planets_of_sorted(sx.state, system);
        for planet in planets.iter() {
            let units = &mut sx.state.board_mut().planet_mut(*planet).units;
            if let Some(at) = units.iter().position(|u| *u == piece) {
                units.remove(at);
                if alastor {
                    let (k, mut cx) = (sx.kernel, sx.cx());
                    k.hooks.direct.nekro_units_alastor_decrement(&mut cx, system, owner, *planet, piece.kind());
                }
                done = true;
                break;
            }
        }
    }
    if !done {
        let units = &mut sx.state.board_mut().sys_mut(system).units;
        if let Some(at) = units.iter().position(|u| *u == piece) {
            units.remove(at);
        }
    }
}

/// `try_announce_ship_destroyed_type`: record the loss and (with a timing resolver) emit `SHIP_DESTROYED`.
pub fn announce_loss(sx: &mut Sx<'_>, system: System, owner: Seat, unit: Unit, cause: Code, during_space_combat: bool, emit: bool) {
    sx.state.transient_mut().combat.last_ship_destroyed = Some((system, owner, unit));
    let remaining = ships_of(AssignHits::rd(sx.state, sx.kernel), owner, system).len();
    let (k, mut cx) = (sx.kernel, sx.cx());
    let galvanized = k.hooks.direct.bastion_take_ship_lost(&mut cx, system, owner, unit);
    let mut payload = Payload::new();
    if galvanized {
        payload.set(Key::Galvanized, Val::Flag(true));
    }
    payload.set(Key::System, Val::System(system));
    payload.set(Key::Player, Val::Seat(owner));
    payload.set(Key::Unit, Val::Unit(unit));
    payload.set(Key::Last, Val::Flag(remaining == 0));
    payload.set(Key::Cause, Val::Code(cause));
    payload.set(Key::DuringSpaceCombat, Val::Flag(during_space_combat));
    if emit {
        let _ = sx.emit(fx::event(EventKind::ShipDestroyed, payload));
    }
}

impl Window for AssignHits {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Choosing => {
                    if self.remaining == 0 {
                        self.stage = Stage::Done;
                        continue;
                    }
                    let alive = self.targets(sx.state, sx.kernel);
                    match alive.as_slice() {
                        [] => self.stage = Stage::Done, // 15.2a
                        [only] => {
                            let only = *only;
                            self.lose(sx, only);
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
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.a.player,
            ask: Ask::Combat(CAsk::AssignHit),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R78_4),
                subtype: Subtype::Combat(CSub::Casualty(self.a.subtype)),
                optional: false,
                target: Some(Target::System(self.a.system)),
                outstanding: [Owed { kind: OwedKind::UnitsToRemove, amount: i64::from(self.remaining), paid: 0 }]
                    .into_iter()
                    .collect::<SmallList<Owed, 4>>(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: casualty_opts(&self.targets(s, k)),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(Opt::Combat(COpt::Destroy { index, .. })) = pick else { return Err(Illegal) };
        let alive = self.targets(sx.state, sx.kernel);
        let piece = alive.get(usize::from(index)).copied().ok_or(Illegal)?;
        self.lose(sx, piece);
        self.stage = Stage::Choosing;
        Ok(())
    }
}
