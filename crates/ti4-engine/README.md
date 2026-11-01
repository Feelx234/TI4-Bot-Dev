# crates/ti4-engine (package `ti4-engine-legacy`)

This directory holds the **string engine**, package `ti4-engine-legacy` (lib `ti4_engine_legacy`). It keeps the
old directory name on purpose, so no tracked path moved (history, blame, merges, path-bound tool files stay valid).
It is the test suite and the comparison oracle; it is frozen after the flip and never part of a production build.

The product-facing crate `ti4-engine` (lib `ti4_engine`, used by host, server, wasm, rule-bot-typed, ...) is the thin package in
`crates/ti4-engine-next`. At the facade stage it is `pub use ti4_engine_legacy::*;`; it is filled module by module (D2-8/D2-9).

New features go on `ti4-engine-legacy` AND are forwarded by `crates/ti4-engine-next/Cargo.toml`. Always write the full package name
(`crates/ti4-legacy` is an unrelated stub). Plan and status: `docs/todo/D2-7-legacy-split.md`, `docs/D27_RESUME.md`.
