//! Small windows of the kit.

use ti4_engine_core::choice::{Choice, Pick};
use ti4_engine_core::kernel::stage::{Illegal, Sx, Window};
use ti4_engine_core::kernel::{Kernel, Next};
use ti4_engine_core::state::{CoreState, EventRec};

/// A window that emits one event through the resolver and ends: the way to run the reaction windows of the registered abilities around it
/// (`Rig::with_abilities`).
#[derive(Clone)]
pub struct EmitEvent {
    pub rec: EventRec,
    done: bool,
}

impl EmitEvent {
    #[must_use]
    pub const fn new(rec: EventRec) -> Self {
        Self { rec, done: false }
    }
}

impl Window for EmitEvent {
    type Stage = bool;

    fn stage(&self) -> bool {
        self.done
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.done {
            self.done = true;
            let _ = sx.emit(self.rec.clone());
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
