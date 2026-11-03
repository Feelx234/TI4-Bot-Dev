//! From `production_view.rs` of the legacy crate: the production kernel of a scope and the facts of the production options in the
//! renderer's position (copies; `ProdView` and the string-side answers stay in the legacy crate).

use ti4_engine_core::generated::Unit;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::rules::production::{self as rules, Cx};
use ti4_model::content_types::SourceSet;

/// [`kernel`] for the sources of a game: the faction hook table with the scope of the cards in play, which the windows that place a form
/// read (Self-Assembly Routines' mech is the one the sources name). Built once per scope and process.
pub fn kernel_of(sources: SourceSet) -> &'static Kernel {
    static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| {
        let mut k = crate::ties::core_adapter::kernel_for(sources).clone();
        k.hooks = ti4_engine_core::hooks::HookTable::new();
        ti4_engine_core_factions::register_all(&mut k.hooks);
        ti4_engine_core::kernel::seam::providers::register_core(&mut k.hooks);
        ti4_engine_core::kernel::seam::adapters::bridge_legacy(&mut k.hooks.seam);
        k
    })
}

/// [`kernel_of`] for the scope as the core states it (`Kernel::sources`): the production kernel an ability that produces runs with.
pub fn kernel_for_core(sources: ti4_engine_core::kernel::Sources) -> &'static Kernel {
    static SETS: std::sync::OnceLock<Vec<(u8, ti4_model::content_types::SourceSet)>> = std::sync::OnceLock::new();
    let sets = SETS.get_or_init(|| (1_u32..128).map(ti4_model::content_types::SourceSet::from_u32_truncated).filter_map(|set| u8::try_from(crate::ties::core_adapter::scope(set).0).ok().map(|bits| (bits, set))).collect());
    let set = sets.iter().find(|(bits, _)| *bits == sources.0).map_or(ti4_model::content_types::FULL, |(_, set)| *set);
    kernel_of(set)
}

/// The kernel of the production reads: the seams of the laws and relics, and the bridge to the legacy
/// hook slots (empty here), then every faction hook table. Built once.
pub fn kernel() -> &'static Kernel {
    static KERNEL: std::sync::OnceLock<Kernel> = std::sync::OnceLock::new();
    KERNEL.get_or_init(|| {
        let mut k = crate::ties::core_adapter::kernel_for(ti4_model::content_types::FULL).clone();
        k.hooks = ti4_engine_core::hooks::HookTable::new();
        ti4_engine_core_factions::register_all(&mut k.hooks);
        ti4_engine_core::kernel::seam::providers::register_core(&mut k.hooks);
        ti4_engine_core::kernel::seam::adapters::bridge_legacy(&mut k.hooks.seam);
        k
    })
}

/// Oceans and commodities of a seat as the payment reads need them.
fn extras_of(core: &ti4_engine_core::state::CoreState, who: Seat) -> (i64, Vec<ti4_engine_core::generated::Planet>) {
    (
        ti4_engine_core_factions::cb_economy::keleres::query::spendable_commodities(core, who),
        ti4_engine_core_factions::cb_economy::deepwrought::query::spendable_oceans(core, who).as_slice().to_vec(),
    )
}

/// The facts of one Choosing option in the position the renderer was given (`Render::at`).
#[must_use]
pub fn choosing_info(r: &crate::render::Render, ask: &ti4_engine_core::choice::production::ChooseAsk, unit: Unit) -> Option<rules::BuildInfo> {
    let pos = r.position()?;
    let core: &ti4_engine_core::state::CoreState = &pos.state;
    let scope = ti4_engine_core::tables::Scope(u32::from(pos.kernel.sources.0));
    let cx = Cx::new(core, pos.kernel, scope, ti4_engine_core::query::reach::blanked(core));
    let (commodities, oceans) = extras_of(core, ask.who);
    let ex = rules::Extras { commodities, oceans: &oceans };
    let before = rules::standing_before(&cx, ask.who, ask);
    let spendable = rules::available(&cx, ask.who, ti4_engine_core::query::economy::Spend::Resources, &ex);
    rules::build_for(&cx, ask.who, ask, unit, &ex, &before, spendable)
}

/// The facts of one placement option in the renderer's position.
#[must_use]
pub fn placing_info(r: &crate::render::Render, ask: &ti4_engine_core::choice::production::PlaceAsk, spot: rules::Spot) -> Option<rules::PlaceInfo> {
    let pos = r.position()?;
    let core: &ti4_engine_core::state::CoreState = &pos.state;
    let scope = ti4_engine_core::tables::Scope(u32::from(pos.kernel.sources.0));
    let cx = Cx::new(core, pos.kernel, scope, ti4_engine_core::query::reach::blanked(core));
    Some(rules::place_info(&cx, ask, spot))
}

