//! The typed payload of an open production step (`Window::Production`): STUB (package M0, `docs/D2_LF_RESUME.md`). Lane M2
//! replaces the body of [`ProductionUse`] by the stages of one use of PRODUCTION; until then it carries what `Window::Untyped`
//! carried (an actor and three numbers). The variant, the kind and the type name are stable.

use crate::ids::Seat;

/// An open use of PRODUCTION (the string `ProductionWindow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProductionUse {
    pub actor: Option<Seat>,
    /// Placeholder words until lane M2 types the machine.
    pub data: [u32; 3],
}
