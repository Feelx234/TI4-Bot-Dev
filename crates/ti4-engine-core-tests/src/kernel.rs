//! The two kernels a test can run on, named so that the choice is visible at the call site.
//!
//! `Kernel::plain()` has an EMPTY hook table: a test that seats a real faction and runs it on the plain kernel proves
//! nothing about the faction (trap T6 of docs/todo/D4-core-tests.md). `kernel_all()` is the default.

use ti4_engine_core::kernel::Kernel;

/// The kernel of a real game: every faction module registered by `ti4_engine_core_factions::register_all`, every source in scope.
#[must_use]
pub fn kernel_all() -> Kernel {
    let mut k = Kernel::plain();
    ti4_engine_core_factions::register_all(&mut k.hooks);
    k
}

/// A kernel with an empty hook table. Only for tests of rules that must NOT depend on a faction (and for the
/// negative control of [`assert_hooks_wired`]).
#[must_use]
pub fn kernel_plain() -> Kernel {
    Kernel::plain()
}

/// Fail unless the kernel's hook table has registrations (the plain kernel has none).
///
/// # Panics
/// When the table is empty.
pub fn assert_hooks_wired(k: &Kernel) {
    assert!(k.hooks.registered() > 0, "the hook table is empty: this is the plain kernel, a faction test on it proves nothing");
}
