//! Hits the modules produce at a moment of a space combat (package W2; the string `CombatWindow::produce_hits` and the
//! `produced_hits` hooks of Ghost, Mentak and Yin), as a [`Window`] that asks.
//!
//! For each side, attacker first, the offers of the modules in the string order (Ghost, Mentak, Yin):
//!
//! * Dimensional Splicer (`CombatStart`, the `ds` technology, a wormhole in the system, ships on both sides): produce 1 hit that the
//!   producer assigns;
//! * Ambush (`CombatStart`, a Mentak seat with cruisers or destroyers): which (up to 2) ships roll, each against its printed
//!   combat value; every hit is one the opponent assigns to any ship;
//! * Impulse Core (`CombatStart`, the `ic` technology) and Devotion (`RoundEnded`, a Yin seat): destroy a cruiser or destroyer of
//!   the producer for 1 hit (Impulse Core: the opponent assigns it to a non-fighter if able; Devotion: the producer assigns it).
//!
//! The window's value is what each side produced; the caller queues the hits (the sustain/assign steps are other windows).

use super::{ships_of, FleetRoll, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Ability, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::hooks::{CombatMoment, ProducedHits};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::keys::{Atom, MarkKey, MarkVal};
use crate::query::reach;
use crate::state::{Code, CoreState, Piece, RngDomain, RollPurpose};

const DIE_SIDES: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Running,
    Asking,
    Done,
}

/// The facts of one moment.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub attacker: Seat,
    pub defender: Seat,
    pub system: System,
    pub moment: CombatMoment,
    /// The resolver carries the timing machinery (the destroyed ship is announced).
    pub announce: bool,
    /// The system holds a wormhole now (the string's `wormholes_at`: printed, tokens, storm, Nexus, modules).
    pub wormhole: bool,
    /// The interned reason text of Ambush's dice (`"ambush"`).
    pub ambush: Code,
    /// The interned causes of the sacrifice (`"technology:impulse_core"`, `"faction_ability:devotion"`).
    pub cause_core: Code,
    pub cause_devotion: Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Offer {
    Splicer,
    Ambush(Vec<(Unit, Option<Unit>)>),
    Sacrifice { devotion: bool, victims: Vec<Unit> },
}

/// The window.
#[derive(Debug, Clone)]
pub struct ProduceHits {
    pub f: Facts,
    /// What each side produced, `[attacker, defender]`.
    pub produced: [ProducedHits; 2],
    /// The dice rolled in order: (hit number, faces).
    pub drawn: Vec<(u32, Vec<u32>)>,
    side: usize,
    module: u8,
    offer: Option<Offer>,
    stage: Stage,
}

impl ProduceHits {
    #[must_use]
    pub fn new(f: Facts) -> Self {
        Self { f, produced: [ProducedHits::default(); 2], drawn: Vec::new(), side: 0, module: 0, offer: None, stage: Stage::Running }
    }

    fn seat(&self) -> Seat {
        if self.side == 0 { self.f.attacker } else { self.f.defender }
    }

    fn opponent(&self) -> Seat {
        if self.side == 0 { self.f.defender } else { self.f.attacker }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    fn is_faction(s: &CoreState, who: Seat, f: crate::generated::Faction) -> bool {
        s.players().by_seat(who).is_some_and(|p| p.faction == f)
    }

    /// The offer of module `module` to the current side, or `None` when it has nothing to ask.
    fn offer_of(&self, s: &CoreState, k: &Kernel, module: u8) -> Option<Offer> {
        let (me, other, f) = (self.seat(), self.opponent(), &self.f);
        let rd = Self::rd(s, k);
        match module {
            0 => {
                (f.moment == CombatMoment::CombatStart
                    && reach::technology_text(s, me, Technology::DimensionalSplicer)
                    && f.wormhole
                    && !ships_of(rd, me, f.system).is_empty()
                    && !ships_of(rd, other, f.system).is_empty())
                .then_some(Offer::Splicer)
            }
            1 => {
                if f.moment != CombatMoment::CombatStart || !Self::is_faction(s, me, crate::generated::Faction::TheMentakCoalition) {
                    return None;
                }
                let mut kinds: Vec<(Unit, usize)> = Vec::new();
                for piece in ships_of(rd, me, f.system) {
                    let data = rd.data(piece.kind())?;
                    if matches!(data.base_type, UnitBaseType::Cruiser | UnitBaseType::Destroyer) && data.combat.is_some() {
                        match kinds.iter_mut().find(|(u, _)| *u == piece.kind()) {
                            Some((_, n)) => *n += 1,
                            None => kinds.push((piece.kind(), 1)),
                        }
                    }
                }
                kinds.sort_by_key(|(u, _)| u.name_rank());
                if kinds.is_empty() {
                    return None;
                }
                let mut groups = Vec::new();
                for (i, (first, count)) in kinds.iter().enumerate() {
                    groups.push((*first, None));
                    if *count >= 2 {
                        groups.push((*first, Some(*first)));
                    }
                    for (second, _) in &kinds[i + 1..] {
                        groups.push((*first, Some(*second)));
                    }
                }
                Some(Offer::Ambush(groups))
            }
            2 => {
                let devotion = match f.moment {
                    CombatMoment::RoundEnded if Self::is_faction(s, me, crate::generated::Faction::TheYinBrotherhood) => true,
                    CombatMoment::CombatStart if reach::technology_text(s, me, Technology::ImpulseCore) => false,
                    _ => return None,
                };
                if ships_of(rd, other, f.system).is_empty() {
                    return None;
                }
                let mut victims: Vec<Unit> = Vec::new();
                for piece in ships_of(rd, me, f.system) {
                    let base = rd.data(piece.kind()).map(|d| d.base_type);
                    if matches!(base, Some(UnitBaseType::Cruiser | UnitBaseType::Destroyer)) && !victims.contains(&piece.kind()) {
                        victims.push(piece.kind());
                    }
                }
                (!victims.is_empty()).then_some(Offer::Sacrifice { devotion, victims })
            }
            _ => None,
        }
    }

    fn ask_for(offer: &Offer) -> CAsk {
        match offer {
            Offer::Splicer => CAsk::Splicer,
            Offer::Ambush(_) => CAsk::Ambush,
            Offer::Sacrifice { devotion, .. } => CAsk::Sacrifice { devotion: *devotion },
        }
    }

    fn roll_for(&mut self, sx: &mut Sx<'_>, kind: Unit) -> usize {
        let me = self.seat();
        let on = u32::from(Self::rd(sx.state, sx.kernel).data(kind).and_then(|d| d.combat).map_or(u8::MAX, |r| r.hits_on));
        let face = sx.die_on(RngDomain::Dice, DIE_SIDES, on);
        let roll = FleetRoll { hits_on: on, faces: vec![face], rerolled: Vec::new() };
        super::record_rolls(sx.state.ledger_mut(), me, Some(self.f.system), std::slice::from_ref(&roll), RollPurpose::Other(self.f.ambush));
        self.drawn.push((on, vec![face]));
        usize::from(face >= on)
    }

    fn apply(&mut self, sx: &mut Sx<'_>, offer: &Offer, pick: Pick) -> Result<(), Illegal> {
        let me = self.seat();
        let slot = self.side;
        match (offer, pick) {
            (_, Pick::Opt(Opt::Decline)) => {}
            (Offer::Splicer, Pick::Opt(Opt::Combat(COpt::ProduceHit))) => self.produced[slot].producer_assigned += 1,
            (Offer::Ambush(groups), Pick::Opt(Opt::Combat(COpt::Ambush { a, b }))) => {
                if !groups.contains(&(a, b)) {
                    return Err(Illegal);
                }
                let mut hits = 0;
                for kind in std::iter::once(a).chain(b) {
                    hits += self.roll_for(sx, kind);
                }
                self.produced[slot].any_ship += hits;
            }
            (Offer::Sacrifice { devotion, victims }, Pick::Opt(Opt::Combat(COpt::Sacrifice { unit }))) => {
                if !victims.contains(&unit) {
                    return Err(Illegal);
                }
                let system = self.f.system;
                let piece: Piece = ships_of(Self::rd(sx.state, sx.kernel), me, system).into_iter().find(|p| p.kind() == unit).ok_or(Illegal)?;
                super::assign::remove_ship(sx, system, me, piece);
                let cause = if *devotion { self.f.cause_devotion } else { self.f.cause_core };
                super::assign::announce_loss(sx, system, me, unit, cause, true, self.f.announce);
                if *devotion {
                    // A faction ability was used (Brother Omar's unlock); Impulse Core is a technology.
                    sx.state.marks_mut().tail.edit().insert(MarkKey::YinAbilityUsed(me), MarkVal::Atom(Atom::Num(1)));
                    self.produced[slot].producer_assigned += 1;
                } else {
                    self.produced[slot].non_fighter += 1;
                }
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

impl Window for ProduceHits {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            if self.stage == Stage::Done {
                return Next::Done;
            }
            if let Some(offer) = &self.offer {
                self.stage = Stage::Asking;
                return Next::Continue(Ask::Combat(Self::ask_for(offer)));
            }
            if self.side >= 2 {
                self.stage = Stage::Done;
                continue;
            }
            if self.module > 2 {
                self.side += 1;
                self.module = 0;
                continue;
            }
            self.offer = self.offer_of(sx.state, sx.kernel, self.module);
            self.module += 1;
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        let offer = self.offer.as_ref()?;
        let me = self.seat();
        let mut opts: SmallList<Opt, 16> = SmallList::new();
        let (origin, subtype) = match offer {
            Offer::Splicer => {
                opts.push(Opt::Combat(COpt::ProduceHit));
                (Origin::FactionAbility(Content::Technology(Technology::DimensionalSplicer)), CSub::Splicer)
            }
            Offer::Ambush(groups) => {
                for (a, b) in groups {
                    opts.push(Opt::Combat(COpt::Ambush { a: *a, b: *b }));
                }
                (Origin::FactionAbility(Content::Ability(Ability::Ambush)), CSub::Ambush)
            }
            Offer::Sacrifice { devotion, victims } => {
                for unit in victims {
                    opts.push(Opt::Combat(COpt::Sacrifice { unit: *unit }));
                }
                let content = if *devotion { Content::Ability(Ability::Devotion) } else { Content::Technology(Technology::ImpulseCore) };
                (Origin::FactionAbility(content), CSub::Sacrifice { devotion: *devotion })
            }
        };
        opts.push(Opt::Decline);
        let _ = (s, k);
        Some(Choice {
            actor: me,
            ask: Ask::Combat(Self::ask_for(offer)),
            ctx: Ctx { origin, subtype: Subtype::Combat(subtype), optional: false, target: None, outstanding: SmallList::new(), space_battle: false, invasion_seq: None },
            opts: Opts(opts),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let offer = self.offer.clone().ok_or(Illegal)?;
        self.apply(sx, &offer, pick)?;
        self.offer = None;
        self.stage = Stage::Running;
        Ok(())
    }
}
