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
