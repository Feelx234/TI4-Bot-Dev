# crates/ti4-engine (package `ti4-engine-legacy`)

This directory holds the **string engine**, package `ti4-engine-legacy` (lib `ti4_engine_legacy`). It keeps the
old directory name on purpose, so no tracked path moved (history, blame, merges, path-bound tool files stay valid).
It is the test suite and the comparison oracle; it is frozen after the flip and never part of a production build.

The product-facing crate `ti4-engine` (lib `ti4_engine`, used by host, server, wasm, rule-bot-typed, ...) is the thin package in
`crates/ti4-engine-next`. Production crates do not build the legacy engine; it is built only by the dev/oracle aliases `t-legacy`, `c-dev`, `t-dev` and the bridge. Layout and where to run what: `docs/ENGINE_LAYOUT.md`.

New features go on `ti4-engine-legacy` AND are forwarded by `crates/ti4-engine-next/Cargo.toml`. Always write the full package name
(`crates/ti4-legacy` is an unrelated stub). Plan and status: `docs/todo/D2-7-legacy-split.md`, `docs/D27_RESUME.md`.

A fix to a rule goes to the typed core (`crates/ti4-engine-core*`) and the thin crate, not here; legacy is the comparison oracle until D4.
