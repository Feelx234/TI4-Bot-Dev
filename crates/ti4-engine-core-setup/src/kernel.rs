//! The kernel of the live path: the hook table with EVERY faction module registered (`ti4_engine_core_factions::register_all`).
//! `core_adapter::kernel_for` is the movement table only (the string engine answers the other hooks through `factions::*`); the live
//! loop has no string engine, so its `Env`, its `KernelEnv` and its windows read this one. Built once per source scope and process.

use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_model::content_types::SourceSet;

use crate::roster::scope;

static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];

/// The live kernel `k` is one of (the renderer's position holds a `'static` kernel); a kernel built by hand (a test's) renders against the full scope's.
#[must_use]
pub fn static_of(k: &Kernel) -> &'static Kernel {
    KERNELS.iter().filter_map(std::sync::OnceLock::get).find(|c| std::ptr::eq(*c, k)).unwrap_or_else(|| kernel_for(ti4_model::content_types::FULL))
}

/// `k` itself as a `'static` live kernel, when it is one of them (the same object, not an equal one): `None` for a kernel built by hand.
#[must_use]
pub fn exact_static(k: &Kernel) -> Option<&'static Kernel> {
    KERNELS.iter().filter_map(std::sync::OnceLock::get).find(|c| std::ptr::eq(*c, k))
}

/// The live kernel of a source scope.
#[must_use]
pub fn kernel_for(sources: SourceSet) -> &'static Kernel {
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| {
        let mut hooks = ti4_engine_core::kernel::HookTable::new();
        // The seams as the combat kernel sets them up, without the string bridge: the laws, relics and technologies the core carries, the
        // legacy hook slots on their seams (before the modules), the modules, and the ground-roll term the string answered through its oracle.
        ti4_engine_core::kernel::seam::providers::register_core(&mut hooks);
        ti4_engine_core::kernel::seam::adapters::bridge_legacy(&mut hooks.seam);
        ti4_engine_core_factions::register_all(&mut hooks);
        ti4_engine_core::kernel::seam::providers::register_ground_abilities(&mut hooks);
        // The Nomad's Quantum Manipulator ("while this unit is in a space area during combat, you may use its SUSTAIN DAMAGE ability") and The Cavalry: the
        // question of a hit reads the legacy slot, which the combat kernel of the string bridge fills with `register_naaz_nomad` (the Naaz half is registered by the
        // faction module itself): without it a Nomad mech in the space area was never offered as a sustainer (nomad 3 players seed 5, step 832).
        hooks.combat.grants_sustain.push(ti4_engine_core::rules::combat::nomad_grants_sustain);
        // The die a round agent gave one unit (a mark of the round): the slot the bridge reads, no module registers it.
        hooks.direct.borrowed_round_agents_extra_die_for.push(ti4_engine_core_factions::cc_ground::borrowed::query::extra_die_for);
        Kernel { hooks, sources: CoreSources(u8::try_from(scope(sources).0).unwrap_or(u8::MAX)) }
    })
}

#[cfg(test)]
mod tests {
    use ti4_engine_core::hooks::{CombatCtx, CombatUnit};
    use ti4_engine_core::state::CoreState;

    /// The live kernel answers the Nomad's mech: it may sustain against a hit in a space combat (and not against a space cannon offense hit).
    #[test]
    fn the_live_kernel_lets_a_nomad_mech_in_the_space_area_sustain() {
        let kernel = super::kernel_for(ti4_model::content_types::FULL);
        let state = CoreState::default();
        let unit = |ctx| CombatUnit { owner: ti4_engine_core::Seat(0), system: None, planet: None, unit: ti4_engine_core::generated::Unit::QuantumManipulator, ctx };
        assert!(kernel.hooks.combat.grants_sustain(&state, kernel, unit(CombatCtx::Space)));
        assert!(!kernel.hooks.combat.grants_sustain(&state, kernel, CombatUnit { unit: ti4_engine_core::generated::Unit::CruiserI, ..unit(CombatCtx::Space) }));
    }
}
