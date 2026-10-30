# W lane 4 resume note (W16, W17, W4 rest), branch `core-w-l4`

Cut from `core-w-l2` @ `d5a42dc0`; `core-w-l1` merged at W4 (the Nomad slots: Future Sight is `Slot(10_005)` here, The Thundarian keeps
10_003 / 10_004; lane 3 has Future Sight at 10_003 and Artuno at 10_004, so the final merge must renumber one side). Pure refactor:
`RULES_VERSION`, serde output and fixtures are unchanged. Issue ids: `docs/ENGINE_KNOWN_ISSUES.md` rows `W16-*`, `W17-*`, `W4-*` (lane 4).

## Gate deferred to the end of D1

By the user's directive no heavy work runs before the end of D1. NOT RUN: `verify_package.sh`, the gate profile, the golden dual
(`profile-20`, `golden-lent.json`, `all=dual`, wide mask), `board_equivalence`, benches, clippy, soak, wasm size, release builds, the mutation
ratchet, any census. Gate deferred to end of D1. The commands are in `docs/W_RESUME.md`. What the gate must look at for this lane:
`legendary::end_turn`, `legendary::pass`, `game::close_vote` (Technology Rider), `combat::begin_round` and the other windows with a reaction slot
(Reveal Prototype: facts taken when the window starts), and `TI4_CORE_STRICT=1` on the golden games (no assertion may fire).

## Done

* W16: the legendary abilities window, Styx's point, the salvage yard purge (see W16-design-01, W16-bail-01).
* W17: the product assertions (W17-design-01) and the host test over every template and alpha init.
* W4: Technology Rider, Reveal Prototype from the slot (W4-design-01/02).

## Not done, with the reason

Courageous To The End, Scramble Frequency, Reflective Shielding from a window, and the Construction / Diplomacy / Warfare Riders: pinned bails,
the tail by the census (W4-bail-02). Jupiter Brain's `control_gained`, the Nexus flip, Maxis Central Control's control chain: W16-bail-01.
XF-1/2/3 (diplomacy, deck reserve, Last Bastion / Crimson) core panics are left to their own task; the codec refusal assertion already covers the
deck reserve and the Last Bastion / Crimson seats are caught by the L21 faction assertion.

## Cheap checks that were run

`cargo test -p ti4-engine-core --lib` (touched modules), `cargo test -p ti4-engine --lib` (whole lib once at the first W16 commit: 4625 passed, one size
test failed and was fixed; then the touched modules after each commit), `cargo test -p ti4-host --test product_assertions`, `check_layers.py core`
(0 violations), `lint_core.sh` (0 hits), single-file mutation checks by a Haiku runner on `63e996fd` (results in W16-mut-01).
