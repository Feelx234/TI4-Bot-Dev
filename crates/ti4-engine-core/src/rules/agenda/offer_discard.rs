//! `laws::offer_discard` as a [`Window`] (package B9): the owner of a ministry law may discard it ("discard this
//! card to ..."). The question is the same for all four callers; the text of the prompt and the display facts
//! under it are the callers' (the adapter interns the prompt and completes the details from the recorded question).

use crate::choice::agenda::{Ask as AAsk, Opt as AOpt};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Opt, Opts, Origin, Pick};
use crate::generated::Agenda;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{Code, CoreState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Asking,
    Done,
}

/// The offer of one law to its owner.
#[derive(Debug, Clone)]
pub struct OfferDiscard {
    law: Agenda,
    owner: Seat,
    /// The interned prompt of the caller, and the interned alias of the law (the question's source).
    prompt: Code,
    source: Code,
    stage: Stage,
    taken: bool,
}

impl OfferDiscard {
    #[must_use]
    pub const fn new(law: Agenda, owner: Seat, prompt: Code, source: Code) -> Self {
        Self { law, owner, prompt, source, stage: Stage::Asking, taken: false }
    }

    /// The owner discarded the law.
    #[must_use]
    pub const fn taken(&self) -> bool {
        self.taken
    }
}

impl Window for OfferDiscard {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, _: &mut Sx<'_>) -> Next {
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Agenda(AAsk::DiscardLaw { law: self.law, prompt: self.prompt })),
            Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let mut opts = Opts::default();
        opts.push(Opt::Agenda(AOpt::DiscardLaw));
        opts.push(Opt::Decline);
        let _ = s;
        Some(super::question(self.owner, AAsk::DiscardLaw { law: self.law, prompt: self.prompt }, Origin::Content(Content::Other(self.source)), opts))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Agenda(AOpt::DiscardLaw)) => {
                // `laws::discard_ministry`: the card leaves play for good, with no printed consequence.
                self.taken = sx.state.agenda_mut().laws.remove(&self.law).is_some();
            }
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}
