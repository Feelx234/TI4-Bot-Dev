//! The typed payload of an open invasion (`Window::Invasion`): STUB (package M0, `docs/D2_LF_RESUME.md`). Lane M3 replaces the
//! body of [`Invasion`] by the composer's stages; until then it carries what `Window::Untyped` carried (an actor and three
//! numbers), so the dual's observers (`d1_action::sub_of`) keep working. The variant, the kind and the type name are stable.

use crate::ids::Seat;

/// An open invasion of an aftermath (the string `InvasionWindow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Invasion {
    pub actor: Option<Seat>,
    /// Placeholder words until lane M3 types the machine.
    pub data: [u32; 3],
}
