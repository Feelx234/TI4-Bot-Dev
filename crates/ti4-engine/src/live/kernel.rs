//! The kernel of the live path: the hook table with EVERY faction module registered (`ti4_engine_core_factions::register_all`).
//! `core_adapter::kernel_for` is the movement table only (the string engine answers the other hooks through `factions::*`); the live
//! loop has no string engine, so its `Env`, its `KernelEnv` and its windows read this one. Built once per source scope and process.

use ti4_engine_core::kernel::Kernel;
use ti4_model::content_types::SourceSet;

/// The live kernel of a source scope.
#[must_use]
pub fn kernel_for(sources: SourceSet) -> &'static Kernel {
    static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| {
        let mut hooks = ti4_engine_core::kernel::HookTable::new();
        ti4_engine_core_factions::register_all(&mut hooks);
        let base = crate::core_adapter::kernel_for(sources);
        Kernel { hooks, sources: base.sources }
    })
}
