//! L4: rule areas (docs/ENGINE_CORE_STATE.md 4.3). One file per area, owned by its package; an area
//! never imports another area (it returns [`crate::kernel::Next`] instead).

pub mod combat;
pub mod invasion;
