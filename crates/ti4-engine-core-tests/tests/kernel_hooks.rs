//! D4-2c: the kernel hook table filled by the faction modules (`HookTable`, `Kernel`, `Sources`).

use ti4_engine_core::kernel::hooks::{CombatCtx, HookTable};
use ti4_engine_core::kernel::seam::Permute;
use ti4_engine_core::kernel::{Kernel, Sources};
use ti4_engine_core_tests::kernel::assert_hooks_wired;
use ti4_engine_core_tests::prelude::*;

#[test]
fn the_real_table_fills_every_group_of_slots_but_the_plain_table_none() {
    let all = kernel_all();
    let plain = kernel_plain();
    assert_eq!(plain.hooks.registered(), 0);
    assert_hooks_wired(&all);
    for (name, n) in [
        ("general", all.hooks.general.registered()),
        ("combat", all.hooks.combat.registered()),
        ("ground", all.hooks.ground.registered()),
        ("economy", all.hooks.economy.registered()),
        ("movement", all.hooks.movement.registered()),
        ("cards", all.hooks.cards.registered()),
        ("strategy", all.hooks.strategy.registered()),
        ("direct", all.hooks.direct.registered()),
    ] {
        assert!(n > 0, "the {name} group has no registered function: the factions fill it");
    }
    assert!(all.hooks.registered() > 0, "the factions fill the table");
}

#[test]
fn registering_twice_doubles_and_registering_nothing_adds_nothing() {
    let mut once = HookTable::new();
    ti4_engine_core_factions::register_all(&mut once);
    let mut twice = HookTable::new();
    ti4_engine_core_factions::register_all(&mut twice);
    ti4_engine_core_factions::register_all(&mut twice);
    assert_eq!(twice.registered(), 2 * once.registered());
    assert_eq!(HookTable::of(&[]).registered(), 0);
    let mut t = HookTable::new();
    t.register(ti4_engine_core_factions::register_all);
    assert_eq!(t.registered(), once.registered());
}

#[test]
fn a_permuted_table_keeps_every_registration() {
    let k = kernel_all();
    for p in Permute::AUDIT {
        assert_eq!(k.hooks.permuted(p).registered(), k.hooks.registered(), "{p:?}");
    }
}

#[test]
fn the_order_audit_passes_on_the_real_table_for_the_combat_modifiers_of_every_seat() {
    let (s, k) = Scenario::lobby(6).map_ring(7).build();
    let probe = |t: &HookTable| {
        let kernel = Kernel { hooks: t.clone(), sources: Sources::all() };
        (0..6u8).flat_map(|seat| [CombatCtx::Space, CombatCtx::Ground].map(|c| t.general.combat_modifier(&s, &kernel, Seat(seat), c))).collect::<Vec<_>>()
    };
    assert_eq!(k.hooks.audit_commutative(probe), Vec::<Permute>::new(), "a sum of faction modifiers does not depend on the registration order");
}

#[test]
fn the_sources_scope_is_a_bitset_over_the_expansions() {
    use ti4_engine_core::generated::Source;
    assert!(!Sources::NONE.has(Source::Base));
    let s = Sources::NONE.with(Source::Base).with(Source::Pok);
    assert!(s.has(Source::Base) && s.has(Source::Pok) && !s.has(Source::ThundersEdge));
    for src in Source::ALL {
        assert!(Sources::all().has(src));
    }
    assert!(Sources::default() == Sources::NONE);
    assert!(kernel_plain().sources == Sources::all(), "the plain kernel keeps every source in scope");
}
