//! The kernel of the live path: the hook table with EVERY faction module registered (`ti4_engine_core_factions::register_all`).
//! `core_adapter::kernel_for` is the movement table only (the string engine answers the other hooks through `factions::*`); the live
//! loop has no string engine, so its `Env`, its `KernelEnv` and its windows read this one. Built once per source scope and process.

use ti4_engine_core::kernel::Kernel;
use ti4_model::content_types::SourceSet;

static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];

/// The live kernel `k` is one of (the renderer's position holds a `'static` kernel); a kernel built by hand (a test's) renders against the full scope's.
#[must_use]
pub fn static_of(k: &Kernel) -> &'static Kernel {
    KERNELS.iter().filter_map(std::sync::OnceLock::get).find(|c| std::ptr::eq(*c, k)).unwrap_or_else(|| kernel_for(ti4_model::content_types::FULL))
}

/// The live kernel of a source scope.
#[must_use]
pub fn kernel_for(sources: SourceSet) -> &'static Kernel {
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| {
        let mut hooks = ti4_engine_core::kernel::HookTable::new();
        ti4_engine_core_factions::register_all(&mut hooks);
        let base = crate::core_adapter::kernel_for(sources);
        Kernel { hooks, sources: base.sources }
    })
}
