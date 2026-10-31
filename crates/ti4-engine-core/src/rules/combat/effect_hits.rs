//! The hit flows an action card runs as child windows of its own play (package B6d, docs/B6_CARDS.md): the card windows
//! reach this area through the kernel's links (`kernel::children::Spawn`, registered in `driver::children`), never by
//! importing it.
//!
//! * [`Destroy`] (Courageous To The End): "your opponent must choose and destroy 1 of their ships". The owner picks one of
//!   their ships in the system (the string `choose_casualty_as` with `destroy: true`: a single ship is taken without a
//!   question), it leaves the board, and its destruction is *staged* in `combat.pending_destructions`: the announcement of the
//!   play (`AbilityTable::after_play`) gives it its `SHIP_DESTROYED` windows after the card has been discarded.
//! * [`Absorb`] (Experimental Battlestation): the hits of an effect land on the ships of the seat the effect names
//!   (the string `absorb_hits_seeing` on a stub resolver): the cancellations a card granted are spent, SUSTAIN DAMAGE is
//!   offered ([`SustainOffer`]), the rest is assigned ([`AssignHits`]). Nothing is announced: an effect has no resolver
//!   of its own.
//!
//! Not covered (the adapter declines the play, the string result stands): neutral owners, and what the two windows it composes
//! do not cover (`sustain_offer`: games below `RULES_FACTION_EFFECTS`, the commander payment of a holder that is not unlocked).

use super::assign::{remove_ship, Assign, AssignHits};
use super::sustain_offer::{Offer, SustainOffer};
use super::{casualty_opts, ships_of, Rd};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{ActionCard, System};
use crate::ids::Seat;
use crate::kernel::children::HitTexts;
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::kernel::{hits, Kernel, Next};
use crate::query::reach;
use crate::state::{Code, CoreState, PendingDestruction, Piece};

/// The suspension points of [`Destroy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestroyStage {
    /// Which ships can be lost? None: the card does nothing; one: it goes; several: ask.
    Choosing,
    /// Waiting on the owner's choice.
    Asking,
    Done,
}

/// The owner of a ship chooses one of their own ships in a system and it is destroyed (the destruction is staged).
#[derive(Debug, Clone, Copy)]
pub struct Destroy {
    /// The seat that chooses (and loses the ship).
    pub who: Seat,
    pub system: System,
    /// The card whose question it is (the source of the question).
    pub card: ActionCard,
    /// The interned subtype of the question (`courageous_to_the_end_assign_casualty`).
    pub subtype: Code,
    /// The interned cause of the staged destruction (`action_card:courageous`).
    pub cause: Code,
    /// A ship was destroyed.
    pub destroyed: bool,
    stage: DestroyStage,
}

impl Destroy {
    #[must_use]
    pub const fn new(who: Seat, system: System, card: ActionCard, subtype: Code, cause: Code) -> Self {
        Self { who, system, card, subtype, cause, destroyed: false, stage: DestroyStage::Choosing }
    }

    fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
        Rd::new(s, k).with_blanked(reach::blanked(s))
    }

    /// Every ship of the owner in the system (the string `ships_of`, in board order): a destroying effect is not a hit, so no
    /// unit is immune to it.
    fn targets(&self, s: &CoreState, k: &Kernel) -> Vec<Piece> {
        ships_of(Self::rd(s, k), self.who, self.system)
    }

    /// `combat::remove_combat_ship`, then the destruction is staged for the announcement of the play.
    fn lose(&mut self, sx: &mut Sx<'_>, piece: Piece) {
        remove_ship(sx, self.system, self.who, piece);
        sx.state.transient_mut().combat.pending_destructions.push(PendingDestruction {
            system: self.system,
            owner: self.who,
            unit: piece.kind(),
            reason: self.cause,
            during_space_combat: true,
        });
        self.destroyed = true;
    }
}

impl Window for Destroy {
    type Stage = DestroyStage;

    fn stage(&self) -> DestroyStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                DestroyStage::Choosing => {
                    let alive = self.targets(sx.state, sx.kernel);
                    match alive.as_slice() {
                        [] => self.stage = DestroyStage::Done,
                        [only] => {
                            let only = *only;
                            self.lose(sx, only);
                            self.stage = DestroyStage::Done;
                        }
                        _ => self.stage = DestroyStage::Asking,
                    }
                }
                DestroyStage::Asking => return Next::Continue(Ask::Combat(CAsk::AssignHit)),
                DestroyStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        (self.stage == DestroyStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Combat(CAsk::AssignHit),
            ctx: Ctx {
                origin: Origin::ActionCard(self.card),
                subtype: Subtype::Combat(CSub::Casualty(self.subtype)),
                optional: false,
                target: Some(Target::System(self.system)),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: casualty_opts(&self.targets(s, k)),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != DestroyStage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(Opt::Combat(COpt::Destroy { index, .. })) = pick else { return Err(Illegal) };
        let alive = self.targets(sx.state, sx.kernel);
        let piece = alive.get(usize::from(index)).copied().ok_or(Illegal)?;
        self.lose(sx, piece);
        self.stage = DestroyStage::Done;
        Ok(())
    }
}

/// The suspension points of [`Absorb`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsorbStage {
    /// The cancellations a card granted are spent.
    Spending,
    /// SUSTAIN DAMAGE is offered (the child [`SustainOffer`]).
    Offering,
    /// The rest is assigned (the child [`AssignHits`]).
    Assigning,
    Done,
}

/// The hits of an effect land on a seat's ships in a system and are absorbed: cancellations, SUSTAIN DAMAGE, casualties.
#[derive(Debug, Clone)]
pub struct Absorb {
    offer: Offer,
    assign: Assign,
    hits: u16,
    stage: AbsorbStage,
    sustain: Child<SustainOffer>,
    assigning: Child<AssignHits>,
}

impl Absorb {
    /// `hits` hits produced by `producer` land on the ships of `who` in `system`. They are not part of a combat and nothing is
    /// announced (the string stub resolver).
    #[must_use]
    pub fn new(who: Seat, producer: Seat, system: System, hits: u16, texts: HitTexts) -> Self {
        let offer = Offer {
            player: who,
            producer,
            system,
            ability: false,
            in_combat: false,
            non_fighters_first: false,
            source: texts.commander,
            staging: texts.staging,
            announce: false,
        };
        let assign = Assign {
            player: who,
            system,
            ability: false,
            non_fighters_first: false,
            subtype: texts.casualty,
            cause: texts.effect_hit,
            during_space_combat: false,
            staging: texts.staging,
            announce: false,
        };
        Self { offer, assign, hits, stage: AbsorbStage::Spending, sustain: Child::idle(), assigning: Child::idle() }
    }

    /// The hits are part of a space combat (a Waylay's barrage): the sustain offer and the loss say so.
    #[must_use]
    pub fn in_space_combat(mut self) -> Self {
        self.offer.in_combat = true;
        self.assign.during_space_combat = true;
        self
    }

    /// The hits of a unit ability's roll (the space cannon offense: `absorb_hits_seeing_with` with `HitOrigin::UnitAbility`):
    /// like [`Self::new`], but a Maximum cannot be assigned them, they are not part of a combat, `bound` binds the loss to
    /// non-fighter ships while one is left (Graviton Laser System; `non_fighter_left` is the display flag of the decline), the
    /// loss carries `cause`, and `announce` says whether the resolver carries the timing machinery.
    #[must_use]
    pub fn unit_ability(who: Seat, producer: Seat, system: System, hits: u16, texts: HitTexts, cause: Code, bound: bool, non_fighter_left: bool, announce: bool) -> Self {
        let mut w = Self::new(who, producer, system, hits, texts);
        w.offer.ability = true;
        w.offer.non_fighters_first = bound && non_fighter_left;
        w.offer.announce = announce;
        w.assign.ability = true;
        w.assign.non_fighters_first = bound;
        w.assign.cause = cause;
        w.assign.announce = announce;
        w
    }
}

impl Window for Absorb {
    type Stage = AbsorbStage;

    fn stage(&self) -> AbsorbStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                AbsorbStage::Spending => {
                    // Shields Holding and Maneuvering Jets cancel hits before any is assigned (`spend_cancellations`).
                    let spent = hits::spend(sx.state, self.offer.player, u32::from(self.hits));
                    self.hits = self.hits.saturating_sub(u16::try_from(spent).unwrap_or(u16::MAX));
                    self.sustain.start(SustainOffer::new(self.offer, self.hits));
                    self.stage = AbsorbStage::Offering;
                }
                AbsorbStage::Offering => match self.sustain.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(offered)) => {
                        self.hits = offered.hits;
                        self.assigning.start(AssignHits::new(self.assign, self.hits));
                        self.stage = AbsorbStage::Assigning;
                    }
                    None => self.stage = AbsorbStage::Done,
                },
                AbsorbStage::Assigning => match self.assigning.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(_)) | None => self.stage = AbsorbStage::Done,
                },
                AbsorbStage::Done => return Next::Done,
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            AbsorbStage::Offering => self.sustain.ask_of(s, k),
            AbsorbStage::Assigning => self.assigning.ask_of(s, k),
            AbsorbStage::Spending | AbsorbStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.stage {
            AbsorbStage::Offering => self.sustain.answer(sx, pick),
            AbsorbStage::Assigning => self.assigning.answer(sx, pick),
            AbsorbStage::Spending | AbsorbStage::Done => Err(Illegal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::Pick;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{Faction, Unit};
    use crate::kernel::children::Report;
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players, Seats};

    const CAUSE: Code = Code(12);
    const SUBTYPE: Code = Code(11);

    /// Two seats; seat 0 is the producer, seat 1 owns `units` in the first system.
    fn fleet(units: &[Unit]) -> (CoreState, System) {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
        s.seats = Seats::new(&[Seat(0), Seat(1)], &[Seat(0), Seat(1)], 2);
        let system = System::ALL[0];
        for unit in units {
            s.board_mut().sys_mut(system).units.push(Piece::new(*unit, Seat(1)));
        }
        (s, system)
    }

    fn destroy(system: System) -> Driver<Destroy> {
        Driver::new(Destroy::new(Seat(1), system, ActionCard::CourageousToTheEnd, SUBTYPE, CAUSE), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    /// A single ship is lost without a question; its destruction is staged (not announced, nothing emitted) with the cause of the card,
    /// and the handoff of the last destruction is left to the announcement.
    #[test]
    fn destroy_takes_a_single_ship_without_a_question_and_stages_its_loss() {
        let (mut s, system) = fleet(&[Unit::CruiserI]);
        let k = Kernel::plain();
        let mut d = destroy(system);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.destroyed);
        assert!(s.board().sys(system).units.is_empty(), "the ship left the board");
        let staged = s.transient().combat.pending_destructions.as_slice().to_vec();
        assert_eq!(staged, vec![PendingDestruction { system, owner: Seat(1), unit: Unit::CruiserI, reason: CAUSE, during_space_combat: true }]);
        assert!(s.transient().combat.last_ship_destroyed.is_none(), "the announcement of the play sets the handoff");
        assert!(d.emitted.is_empty(), "nothing is announced by the window itself");
    }

    /// Several ships: the owner is asked with one option per kind (damage told apart), as the source of the card, no owed hits; the
    /// pick destroys that ship and not another.
    #[test]
    fn destroy_asks_the_owner_which_ship_and_destroys_the_one_picked() {
        let (mut s, system) = fleet(&[Unit::CruiserI, Unit::CruiserI, Unit::DreadnoughtI]);
        let k = Kernel::plain();
        let mut d = destroy(system);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("a question") };
        assert_eq!(c.actor, Seat(1));
        assert_eq!(c.ask, crate::choice::Ask::Combat(CAsk::AssignHit));
        assert_eq!(c.ctx.origin, Origin::ActionCard(ActionCard::CourageousToTheEnd));
        assert_eq!(c.ctx.subtype, Subtype::Combat(CSub::Casualty(SUBTYPE)));
        assert_eq!(c.ctx.target, Some(Target::System(system)));
        assert!(c.ctx.outstanding.is_empty(), "a destruction is not a hit that is owed");
        assert_eq!(c.opts.len(), 2, "two cruisers are one choice");
        assert_eq!(s.board().sys(system).units.len(), 3, "nothing is lost before the answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Number(0)), Err(DriverError::Illegal));
        // The second option is the dreadnought.
        assert_eq!(d.resume(&mut s, &k, c.pick(1).unwrap()).unwrap(), Status::Done);
        let left: Vec<Unit> = s.board().sys(system).units.iter().map(|p| p.kind()).collect();
        assert_eq!(left, vec![Unit::CruiserI, Unit::CruiserI]);
        assert_eq!(s.transient().combat.pending_destructions.as_slice()[0].unit, Unit::DreadnoughtI);
        assert!(d.window.destroyed);
    }

    /// A seat with no ship loses nothing and the card is told so (it stops asking).
    #[test]
    fn destroy_with_no_ship_to_lose_destroys_nothing() {
        let (mut s, system) = fleet(&[]);
        let k = Kernel::plain();
        let mut d = destroy(system);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.destroyed);
        assert!(s.transient().combat.pending_destructions.is_empty());
    }

    fn absorb(system: System, hits: u16, staging: bool) -> Driver<Absorb> {
        let texts = HitTexts { staging, commander: Code(1), casualty: Code(2), effect_hit: Code(3) };
        Driver::new(Absorb::new(Seat(1), Seat(0), system, hits, texts), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    /// The hits of an effect: the cancellations a card granted are spent first, then SUSTAIN DAMAGE is offered (the unit that sustains
    /// is damaged and cancels one hit), the rest is assigned; nothing is announced (the window has no resolver of its own).
    #[test]
    fn absorb_spends_cancellations_offers_sustain_damage_and_assigns_the_rest() {
        let (mut s, system) = fleet(&[Unit::DreadnoughtI, Unit::CruiserI]);
        hits::grant(&mut s, Seat(1), 1);
        let k = Kernel::plain();
        let mut d = absorb(system, 3, true);
        // Three hits, one cancelled by the card: two remain; the offer to sustain names them.
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("the sustain offer") };
        let crate::choice::Ask::Combat(CAsk::CancelHit { hits: owed, in_combat, ability, .. }) = c.ask else { panic!("a sustain offer: {:?}", c.ask) };
        assert_eq!((owed, in_combat, ability), (2, false, false), "the cancellation was spent before the offer; the hits are not part of a combat");
        assert_eq!(c.actor, Seat(1));
        assert_eq!(hits::cancellable(&s, Seat(1)), 0);
        // Sustain (the first option): one more hit is cancelled, the last one is assigned to a ship of the owner's choice.
        let Status::Asking(c) = d.resume(&mut s, &k, c.pick(0).unwrap()).unwrap() else { panic!("the casualty question") };
        let crate::choice::Ask::Combat(CAsk::AssignHit) = c.ask else { panic!("a casualty question") };
        assert_eq!(c.ctx.outstanding.as_slice()[0].amount, 1);
        assert_eq!(c.ctx.subtype, Subtype::Combat(CSub::Casualty(Code(2))));
        // Two options: the damaged dreadnought and the cruiser.
        assert_eq!(c.opts.len(), 2);
        assert_eq!(d.resume(&mut s, &k, c.pick(1).unwrap()).unwrap(), Status::Done);
        let left: Vec<(Unit, bool)> = s.board().sys(system).units.iter().map(|p| (p.kind(), p.sustained())).collect();
        assert_eq!(left, vec![(Unit::DreadnoughtI, true)], "the cruiser was lost, the dreadnought sustained");
        assert!(d.emitted.is_empty(), "no event is announced");
        assert_eq!(s.transient().combat.last_ship_destroyed, Some((system, Seat(1), Unit::CruiserI)), "with staging the loss is recorded");
        assert!(s.transient().combat.pending_destructions.is_empty(), "nothing is staged for an announcement");
    }

    /// Hits that the cancellations cover, a seat with no ship and no sustainer: nothing is asked.
    #[test]
    fn absorb_asks_nothing_when_nothing_can_be_hit_or_everything_is_cancelled() {
        let k = Kernel::plain();
        let (mut s, system) = fleet(&[]);
        assert_eq!(absorb(system, 3, false).run(&mut s, &k).unwrap(), Status::Done);
        let (mut s, system) = fleet(&[Unit::DreadnoughtI, Unit::CruiserI]);
        hits::grant(&mut s, Seat(1), 2);
        let before = s.clone();
        let mut d = absorb(system, 2, false);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s.board().sys(system).units.len(), 2, "both hits were cancelled");
        assert_eq!(hits::cancellable(&s, Seat(1)), 0);
        assert_ne!(s, before, "the cancellations were spent");
    }

    /// The hits of an effect are not a combat's: nothing is announced, the loss is not "during a space combat" and carries the cause of
    /// an effect's hit; the facts of the two windows it composes are the texts and the flag it was given. (The cause and the flag are
    /// read only by the announcement of a loss, which the window never makes, so no run can show them.)
    #[test]
    fn absorb_builds_the_offer_and_the_assignment_of_an_effects_hits_not_a_combats() {
        let texts = HitTexts { staging: true, commander: Code(1), casualty: Code(2), effect_hit: Code(3) };
        let window = Absorb::new(Seat(1), Seat(0), System::ALL[0], 2, texts);
        let (o, a) = (window.offer, window.assign);
        assert_eq!((o.player, o.producer, o.source, o.staging), (Seat(1), Seat(0), Code(1), true));
        assert!(!o.ability && !o.in_combat && !o.non_fighters_first && !o.announce, "an effect's hit: not an ability's, not in a combat, nothing announced");
        assert_eq!((a.player, a.subtype, a.cause, a.staging), (Seat(1), Code(2), Code(3), true));
        assert!(!a.ability && !a.non_fighters_first && !a.during_space_combat && !a.announce, "and the loss is recorded, not announced, outside a combat");
        assert_eq!(window.hits, 2);
    }

    /// A lone ship takes the hit without a question; without staging the loss is not recorded as a handoff.
    #[test]
    fn absorb_takes_a_lone_ship_without_a_question() {
        let (mut s, system) = fleet(&[Unit::CruiserI]);
        let k = Kernel::plain();
        assert_eq!(absorb(system, 1, false).run(&mut s, &k).unwrap(), Status::Done);
        assert!(s.board().sys(system).units.is_empty());
        assert!(s.transient().combat.last_ship_destroyed.is_none());
    }

    /// The reports the registered links read: whether a ship was destroyed.
    #[test]
    fn the_destroy_window_reports_whether_a_ship_was_destroyed() {
        let table = {
            let mut t = AbilityTable::default();
            crate::driver::children::register(&mut t);
            t
        };
        for (units, want) in [(vec![Unit::CruiserI], true), (vec![], false)] {
            let (mut s, system) = fleet(&units);
            let request = crate::kernel::children::Spawn::Destroy { who: Seat(1), system, card: ActionCard::CourageousToTheEnd, subtype: SUBTYPE, cause: CAUSE };
            let mut window = table.links.start(&request).expect("a window");
            let k = Kernel::plain();
            let mut tape = crate::kernel::ability::Tape::default();
            let mut resolver = Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0)));
            let mut next_id = 1_u64;
            let mut emitted = Vec::new();
            let mut sx = Sx { state: &mut s, kernel: &k, resolver: &mut resolver, table: &table, tape: &mut tape, next_id: &mut next_id, emitted: &mut emitted, seeds: &crate::driver::NoSeeds };
            assert!(matches!(window.step(&mut sx), Next::Done));
            assert_eq!(window.report(), Report::Destroyed(want));
        }
    }
}
