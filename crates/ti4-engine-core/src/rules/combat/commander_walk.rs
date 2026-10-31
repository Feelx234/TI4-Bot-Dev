//! The Jol-Nar commander's walk over a staged roll (Agnlan Oln: "after you roll dice for a unit ability: you may reroll any of those
//! dice"; the string `combat::reroll_window`, `choose_reroll_dice` and `apply_reroll_dice`), for the hosts that stage their dice in
//! `transient.combat.reroll_staging` and let the window of the roll run on them: the bombardment ([`crate::rules::invasion::bombard_roll`])
//! and the space cannon offense ([`super::space_cannon_offense`]). The anti-fighter barrage keeps its own walk over its entries.
//!
//! One optional question per die, in the order of the staged entries (every die of every entry, the ones that hit too); the answers are
//! collected and only then are the picked dice drawn again, in order, from the dice stream. A redraw replaces the face and the adjustment
//! the die carried, marks it rerolled, and is kept in the ledger as the string's dice history keeps it (the faces of the whole entry after
//! the draw, the die drawn again marked: a `Reroll` row). The host reads the faces back from the staged set.

use super::{record_rolls, FleetRoll, DIE_SIDES};
use crate::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Leader, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx};
use crate::state::{Code, CoreState, RerollSet, RngDomain};

/// The walk of one roller over the set it staged.
#[derive(Debug, Clone)]
pub struct Walk {
    who: Seat,
    system: System,
    /// The unit text of each staged entry in the question renderer's table (the label of the question); an entry without one is shown
    /// with the staged entry's own code.
    shown: Vec<Code>,
    /// Every die of the set, (entry, die), in the order the string asks them.
    dice: Vec<(u8, u16)>,
    at: usize,
    picks: Vec<(u8, u16)>,
}

impl Walk {
    /// The walk over the set `who` has staged in `state` (`None`: nothing staged, or no die in it, which the string never asks about).
    #[must_use]
    pub fn start(state: &CoreState, who: Seat, shown: &[Code]) -> Option<Self> {
        let set = Self::staged(state, who)?;
        let dice: Vec<(u8, u16)> = set
            .rolls
            .iter()
            .enumerate()
            .flat_map(|(e, entry)| (0..entry.faces.len()).map(move |d| (u8::try_from(e).unwrap_or(u8::MAX), u16::try_from(d).unwrap_or(u16::MAX))))
            .collect();
        if dice.is_empty() {
            return None;
        }
        Some(Self { who, system: set.system, shown: shown.to_vec(), dice, at: 0, picks: Vec::new() })
    }

    /// The walk of `who` over the set staged in `state`, picked up where it stood: `at` dice have been answered and `picks` are the ones
    /// chosen so far (a window that keeps its position in the state, not in itself, rebuilds the walk at every call). `shown` empty: a
    /// question is labelled by the staged entry's own code.
    #[must_use]
    pub fn resume(state: &CoreState, who: Seat, shown: &[Code], at: usize, picks: &[(u8, u16)]) -> Option<Self> {
        let mut walk = Self::start(state, who, shown)?;
        walk.at = at;
        walk.picks = picks.to_vec();
        Some(walk)
    }

    /// How many dice have been answered.
    #[must_use]
    pub fn at(&self) -> usize {
        self.at
    }

    /// The dice picked so far, (entry, die).
    #[must_use]
    pub fn picks(&self) -> &[(u8, u16)] {
        &self.picks
    }

    fn staged(state: &CoreState, who: Seat) -> Option<&RerollSet> {
        state.transient().combat.reroll_staging.iter().find(|(s, _)| *s == who).map(|(_, set)| set)
    }

    /// Every die has been asked about.
    #[must_use]
    pub fn done(&self) -> bool {
        self.at >= self.dice.len()
    }

    /// The question about the die the walk stands at.
    fn question(&self, state: &CoreState) -> Option<(CAsk, COpt)> {
        let (e, d) = *self.dice.get(self.at)?;
        let set = Self::staged(state, self.who)?;
        let entry = set.rolls.get(usize::from(e))?;
        let face = *entry.faces.get(usize::from(d))?;
        let unit = self.shown.get(usize::from(e)).copied().unwrap_or(entry.unit);
        let die = u8::try_from(d).unwrap_or(u8::MAX);
        Some((CAsk::RerollDie { source: RerollSource::Commander, unit, die }, COpt::RerollDie { unit, entry: e, die, face, hits_on: entry.hits_on }))
    }

    /// The question of the open walk.
    #[must_use]
    pub fn ask(&self, state: &CoreState) -> Option<Choice> {
        let (ask, opt) = self.question(state)?;
        Some(Choice {
            actor: self.who,
            ask: Ask::Combat(ask),
            ctx: Ctx {
                origin: Origin::Content(Content::Leader(Leader::AgnlanOln)),
                subtype: Subtype::Combat(CSub::Reroll(RerollSource::Commander)),
                optional: false,
                target: Some(Target::System(self.system)),
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: Opts([Opt::Combat(opt), Opt::Decline].into_iter().collect()),
        })
    }

    /// The answer to the open question: the die is drawn again (picked) or kept.
    ///
    /// # Errors
    /// [`Illegal`] when nothing is asked or the pick is not one of the two options.
    pub fn answer(&mut self, state: &CoreState, pick: Pick) -> Result<(), Illegal> {
        let (_, asked) = self.question(state).ok_or(Illegal)?;
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        match opt {
            Opt::Decline => {}
            Opt::Combat(chosen) if chosen == asked => self.picks.push(self.dice[self.at]),
            _ => return Err(Illegal),
        }
        self.at += 1;
        Ok(())
    }

    /// `apply_reroll_dice`: draw each picked die again in order and keep each draw in the ledger. Returns how many dice were drawn.
    pub fn redraw(&mut self, sx: &mut Sx<'_>) -> usize {
        let picks = std::mem::take(&mut self.picks);
        let mut drawn = 0;
        for (e, d) in picks {
            let Some(len) = Self::staged(sx.state, self.who).and_then(|set| set.rolls.get(usize::from(e))).map(|entry| entry.faces.len()) else { continue };
            if usize::from(d) >= len {
                continue;
            }
            let face = sx.die(RngDomain::Dice, DIE_SIDES);
            let staging = sx.state.transient_mut().combat.reroll_staging.edit();
            let Some((_, set)) = staging.iter_mut().find(|(s, _)| *s == self.who) else { continue };
            let system = set.system;
            let Some(entry) = set.rolls.get_mut(usize::from(e)) else { continue };
            entry.faces[usize::from(d)] = u8::try_from(face).unwrap_or(u8::MAX);
            if d < 64 {
                entry.rerolled |= 1 << d;
            }
            entry.deltas.remove(&u8::try_from(d).unwrap_or(u8::MAX));
            let group = FleetRoll { hits_on: entry.hits_on.map_or(0, u32::from), faces: entry.faces.iter().map(|f| u32::from(*f)).collect(), rerolled: vec![u32::from(d)] };
            if sx.state.loaded.has(crate::state::Slice::Ledger) {
                record_rolls(sx.state.ledger_mut(), self.who, Some(system), &[group], crate::state::RollPurpose::Reroll);
            }
            drawn += 1;
        }
        drawn
    }
}
