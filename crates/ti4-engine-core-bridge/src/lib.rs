//! Equivalence checks and the benchmark of the typed movement slice (docs/ENGINE_CORE.md).
//! The adapter itself is `ti4_engine::core_adapter` (feature `typed-core`).

pub use ti4_engine::core_adapter as adapter;
pub mod allow_list;
pub mod check;
pub mod dual_shard;
pub mod alpha;
pub mod roundtrip;
pub mod check_hooks;
pub mod check_kernel_env;
pub mod check_menu_sources;
pub mod check_leaders;
pub mod check_play_facts;
pub mod workload;
pub mod shadow;
pub mod seen_parity;
pub mod check_tech;
pub mod golden;
pub mod oracle;
pub mod islands;
pub mod native_subject;
pub mod native_play;
