//! The offer to cancel hits with SUSTAIN DAMAGE (LRR 87; string `offer_sustain`, `combat.rs`), as a
//! [`Window`].
//!
//! Stages (one per suspension point of the string function): `Offering` computes what can sustain and asks
//! `CancelHit` ("cancel a hit at {system}"); `Commander` hands the Rear Admiral Farran offer to the nested
//! [`SustainCommander`] window (its question is this window's question while it runs); then the
//! `SUSTAIN_DAMAGE_USED` event is emitted and the loop starts again, `cancel_per_sustain` hits fewer.
//!
//! Not covered (the adapter returns `None`, the string result stands): staged planet sustainers
//! (`supply::staging_enabled`), games below `RULES_FACTION_EFFECTS` (the old commander payment), and the
//! hits a Reflective Shielding reaction stages during the emission.

use super::sustain_commander::SustainCommander;
use super::{cancel_per_sustain, direct_hittable_at, apply_sustain, sustain_opts, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Owed, OwedKind, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, EventKind, Key, Payload, Val};

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// What can sustain? Nothing: the window is over. Something: ask.
    Offering,
    /// Waiting on the owner's choice.
    Asking,
    /// The commander's offer runs (its question is ours).
    Commander,
    Done,
}

/// The facts of one offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer {
    pub player: Seat,
    pub producer: Seat,
    pub system: System,
    /// The hit is a unit ability's (a Maximum cannot be assigned it).
    pub ability: bool,
    /// The hits are part of a space combat.
    pub in_combat: bool,
    /// The loss is bound to non-fighters and one exists (display flag of the decline).
    pub non_fighters_first: bool,
    /// The interned `"letnevcommander"`.
    pub source: Code,
    /// `supply::staging_enabled`: every planet-standing sustainer is offered and named by its planet.
    pub staging: bool,
    /// The event is announced (the combat window); the windowless absorb path announces nothing.
    pub announce: bool,
}

/// The window: the offer, the hits still owed and where it has got to.
#[derive(Debug, Clone, Copy)]
pub struct SustainOffer {
    pub offer: Offer,
    pub hits: u16,
    stage: Stage,
    commander: SustainCommander,
    /// The unit that sustained, until its event is emitted.
    used: Option<Unit>,
    /// The last unit that sustained: where it stood and how it stands now (the `combat:sustain_target`
    /// handoff the adapter writes).
    pub last: Option<(crate::choice::combat::SustainSite, crate::state::Piece)>,
}

impl SustainOffer {
    #[must_use]
    pub fn new(offer: Offer, hits: u16) -> Self {
        Self { offer, hits, stage: Stage::Offering, commander: SustainCommander::new(offer.player, offer.source), used: None, last: None }
    }

    pub(super) fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn ask(&self) -> CAsk {
        CAsk::CancelHit {
            system: self.offer.system,
            hits: self.hits,
            non_fighters_first: self.offer.non_fighters_first,
            staging: self.offer.staging,
            ability: self.offer.ability,
            in_combat: self.offer.in_combat,
            window: None,
        }
    }

    fn emit_used(&self, sx: &mut Sx<'_>, unit: Unit) -> Result<(), crate::kernel::TimingError> {
        emit_used(sx, &self.offer, unit)
    }
}

/// `combat::remember_sustain_target`: with staging, the exact unit that sustained is written as the `combat:sustain_target`
/// mark (system, planet when the unit stands on one, owner, type, damage, galvanize token) that a Direct Hit reads and clears.
pub fn remember_target(state: &mut CoreState, system: System, staging: bool, site: crate::choice::combat::SustainSite, piece: crate::state::Piece) {
    use crate::keys::{Atom, MarkKey, MarkVal};
    if !staging {
        return;
    }
    let planet = match site {
        crate::choice::combat::SustainSite::Planet { planet, .. } => Atom::Planet(planet),
        crate::choice::combat::SustainSite::Space(_) => Atom::Absent,
    };
    let num = |b: bool| MarkVal::Atom(Atom::Num(u32::from(b)));
    let value = MarkVal::Rec(vec![
        MarkVal::Atom(Atom::System(system)),
        MarkVal::Atom(planet),
        MarkVal::Atom(Atom::Seat(piece.owner())),
        MarkVal::Atom(Atom::Unit(piece.kind())),
        num(piece.sustained()),
        num(piece.galvanized()),
    ]);
    state.marks_mut().tail.edit().insert(MarkKey::CombatSustainTarget(), value);
}

/// The handoff of the unit that sustained and, when announced, `SUSTAIN_DAMAGE_USED`.
pub fn emit_used(sx: &mut Sx<'_>, o: &Offer, unit: Unit) -> Result<(), crate::kernel::TimingError> {
    sx.state.transient_mut().combat.last_sustain = Some((o.system, o.player, unit, o.producer, o.in_combat));
    let direct = direct_hittable_at(SustainOffer::rd(sx.state, sx.kernel), o.player, o.system, unit);
    let payload = Payload::new()
        .with(Key::System, Val::System(o.system))
        .with(Key::Player, Val::Seat(o.player))
        .with(Key::Unit, Val::Unit(unit))
        .with(Key::Producer, Val::Seat(o.producer))
        .with(Key::DuringSpaceCombat, Val::Flag(o.in_combat))
        .with(Key::DirectHittable, Val::Flag(direct));
    if !o.announce {
        return Ok(());
    }
    sx.emit(fx::event(EventKind::SustainDamageUsed, payload)).map(|_| ())
}

impl Window for SustainOffer {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Offering => {
                    if self.hits == 0 {
                        self.stage = Stage::Done;
                        continue;
                    }
                    let opts = sustain_opts(Self::rd(sx.state, sx.kernel), self.offer.player, self.ask());
                    let Some(Opt::Combat(COpt::Sustain { .. })) = opts.as_slice().first().copied() else {
                        self.stage = Stage::Done;
                        continue;
                    };
                    // Neutral units use every ability they can: they sustain and are never asked.
                    if super::is_neutral(sx.state, self.offer.player) {
                        let first = opts.as_slice()[0];
                        let _ = self.take(sx, Pick::Opt(first));
                        continue;
                    }
                    self.stage = Stage::Asking;
                }
                Stage::Asking => return Next::Continue(Ask::Combat(self.ask())),
                Stage::Commander => match self.commander.step(sx) {
                    Next::Continue(a) => return Next::Continue(a),
                    _ => {
                        let unit = self.used.take().expect("a sustain is under way");
                        let _ = self.emit_used(sx, unit);
                        let cancels = cancel_per_sustain(Self::rd(sx.state, sx.kernel), self.offer.player);
                        self.hits = self.hits.saturating_sub(u16::try_from(cancels).unwrap_or(u16::MAX));
                        self.stage = Stage::Offering;
                    }
                },
                Stage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Asking => {
                let ask = self.ask();
                Some(Choice {
                    actor: self.offer.player,
                    ask: Ask::Combat(ask),
                    ctx: Ctx {
                        origin: Origin::Rule(Rule::R82),
                        subtype: Subtype::Combat(CSub::SustainDamage),
                        optional: false,
                        target: Some(Target::System(self.offer.system)),
                        outstanding: [Owed { kind: OwedKind::UnitsToRemove, amount: i64::from(self.hits), paid: 0 }]
                            .into_iter()
                            .collect::<SmallList<Owed, 4>>(),
                        space_battle: false,
                        invasion_seq: None,
                    },
                    opts: sustain_opts(Self::rd(s, k), self.offer.player, ask),
                })
            }
            Stage::Commander => self.commander.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            Stage::Asking => self.take(sx, pick),
            Stage::Commander => self.commander.answer(sx, pick),
            _ => Err(Illegal),
        }
    }
}

impl SustainOffer {
    /// The owner's answer: decline ends the offer; a sustain marks the unit and moves to the commander.
    fn take(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match pick {
            Pick::Opt(Opt::Decline) => {
                self.stage = Stage::Done;
                Ok(())
            }
            Pick::Opt(Opt::Combat(COpt::Sustain { site, unit, .. })) => {
                let rd = Self::rd(sx.state, sx.kernel);
                let Some((after, piece)) = apply_sustain(rd, self.offer.player, self.offer.system, site) else {
                    // No such unit sustains: nothing happens, but the use still cancels its hits.
                    let cancels = cancel_per_sustain(rd, self.offer.player);
                    self.hits = self.hits.saturating_sub(u16::try_from(cancels).unwrap_or(u16::MAX));
                    self.stage = Stage::Offering;
                    return Ok(());
                };
                *sx.state = after;
                remember_target(sx.state, self.offer.system, self.offer.staging, site, piece);
                self.last = Some((site, piece));
                self.used = Some(unit);
                self.commander = SustainCommander::new(self.offer.player, self.offer.source);
                self.stage = Stage::Commander;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
