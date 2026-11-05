//! The kernel of the live path: the hook table with EVERY faction module registered (`ti4_engine_core_factions::register_all`). The
//! tables live in the setup crate (`ti4_engine_core_setup::kernel`), which needs the same kernel to settle the starting technology.

pub use ti4_engine_core_setup::kernel::{kernel_for, static_of};
