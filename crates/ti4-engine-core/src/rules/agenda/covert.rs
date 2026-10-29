//! Covert Legislation's face-down draw as a [`Window`] (package B9; string `Game::draw_covert_agenda`).
//!
//! "The speaker draws the next card in the agenda deck but does not reveal it ... reads the eligible outcomes
//! aloud." A drawn card with no eligible outcome is discarded and the next one drawn; an empty deck leaves
//! nothing to vote on. The window leaves the drawn agenda in `agenda().covert` and the outcomes it elects in
//! [`CovertDraw::choices`].

use super::window::KernelEnv;
use super::{outcomes, Cx};
use crate::choice::{Choice, Pick};
use crate::containers::SmallList;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{CoreState, Outcome};
use crate::tables::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Draw,
    Done,
}

/// The draw.
#[derive(Debug, Clone)]
pub struct CovertDraw {
    scope: Scope,
    blanked: bool,
    stage: Stage,
    choices: SmallList<Outcome, 16>,
}

impl CovertDraw {
    #[must_use]
    pub const fn new(scope: Scope, blanked: bool) -> Self {
        Self { scope, blanked, stage: Stage::Draw, choices: SmallList::new() }
    }

    /// The outcomes of the agenda drawn (empty when the deck ran out).
    #[must_use]
    pub fn choices(&self) -> &[Outcome] {
        self.choices.as_slice()
    }
}

impl Window for CovertDraw {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Draw {
            sx.state.agenda_mut().covert = None;
            while let Some(hidden) = sx.state.decks_mut().agenda.draw() {
                let env = KernelEnv { core: sx.state, kernel: sx.kernel };
                let found = outcomes(&Cx { core: sx.state, scope: self.scope, blanked: self.blanked, env: &env }, hidden);
                if !found.is_empty() {
                    sx.state.agenda_mut().covert = Some(hidden);
                    self.choices = found;
                    break;
                }
            }
            self.stage = Stage::Done;
        }
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}
