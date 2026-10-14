//! A seat's "never offer me this card" preference. The wrapper is shared with the browser build,
//! so it lives in `ti4-host`; the engine itself stays decision-pure.

pub use ti4_host::view::reaction_modes::*;
