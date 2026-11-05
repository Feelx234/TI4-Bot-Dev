//! The Jol-Nar commander's walk over a staged roll (Agnlan Oln: "after you roll dice for a unit ability: you may reroll any of those
//! dice"; the string `combat::reroll_window`, `choose_reroll_dice` and `apply_reroll_dice`), for the hosts that stage their dice in
//! `transient.combat.reroll_staging` and let the window of the roll run on them: the bombardment ([`crate::rules::invasion::bombard_roll`])
//! and the space cannon offense ([`crate::rules::combat::space_cannon_offense`]), and for the native invasion window. The anti-fighter barrage keeps its own walk over its entries.
//!
//! One optional question per die, in the order of the staged entries (every die of every entry, the ones that hit too); the answers are
//! collected and only then are the picked dice drawn again, in order, from the dice stream. A redraw replaces the face and the adjustment
//! the die carried, marks it rerolled, and is kept in the ledger as the string's dice history keeps it (the faces of the whole entry after
//! the draw, the die drawn again marked: a `Reroll` row). The host reads the faces back from the staged set.

use crate::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource, Subtype as CSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use crate::containers::SmallList;
use crate::generated::{Leader, System};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx};
use crate::state::{Code, CoreState, RerollSet, RngDomain, RollPurpose, RollRec};

/// The ten-sided die (`rules::combat::DIE_SIDES`).
const DIE_SIDES: u32 = 10;

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
        // A question about the system of the invasion in progress names it (the string `DecisionContext::about_invasion`: a question about a
        // battle does not, and neither does one about another system).
        let invasion_seq = state.transient().invasion.active.as_ref().filter(|a| a.system == self.system).map(|a| a.seq);
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
                invasion_seq,
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

    /// `apply_reroll_dice`: draw each picked die again in order and keep each draw in the ledger. 
    pub fn redraw(&mut self, sx: &mut Sx<'_>) {
        let picks = std::mem::take(&mut self.picks);
        for (e, d) in picks {
            let Some(len) = Self::staged(sx.state, self.who).and_then(|set| set.rolls.get(usize::from(e))).map(|entry| entry.faces.len()) else { continue };
            if usize::from(d) >= len {
                continue; // mutants:skip: the picks are dice of the set, which nothing changes between the question and the draw
            }
            let face = sx.die(RngDomain::Dice, DIE_SIDES);
            let staging = sx.state.transient_mut().combat.reroll_staging.edit();
            let Some((_, set)) = staging.iter_mut().find(|(s, _)| *s == self.who) else { continue };
            let system = set.system;
            let Some(entry) = set.rolls.get_mut(usize::from(e)) else { continue };
            entry.faces[usize::from(d)] = u8::try_from(face).unwrap_or(u8::MAX);
            // (The mark is display only, for the details of a later question; a die has at most 64 positions.)
            if d < 64 { // mutants:skip
                entry.rerolled |= 1 << d; // mutants:skip
            }
            entry.deltas.remove(&u8::try_from(d).unwrap_or(u8::MAX));
            // The string's dice history keeps the faces of the whole entry after the draw, the die drawn again marked.
            let on = entry.hits_on.unwrap_or(0); // mutants:skip: a staged entry always has its number
            let mark = if d < 64 { 1_u64 << d } else { 0 }; // mutants:skip: a die has at most 64 positions
            let row = RollRec { purpose: RollPurpose::Reroll, owner: Some(self.who), unit: None, planet: None, scene: Some(system), hits_on: Some(on), faces: entry.faces.iter().copied().collect(), rerolled: mark };
            if sx.state.loaded.has(crate::state::Slice::Ledger) {
                sx.state.ledger_mut().dice.push(row);
            }
        }
    }
}
