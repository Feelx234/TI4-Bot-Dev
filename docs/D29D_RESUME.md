# D2-9 lane 9d resume

Branch `core-d29d-aliases`, cut from `core-integration-20` (b8e43ef8d). Text and config only; no Rust sources, no Cargo.toml, no CLAUDE.md.

## Done
- `.cargo/config.toml`: `-p ti4-engine-legacy` removed from `c-web`, `l-web`, `t-web`, `b-web`, `wire-web`; `t-web-slow` opt-level keys now `ti4-engine-core`, `-core-factions`, `ti4-engine`; new `t-legacy`, `c-dev`, `t-dev`; comment block rewritten; `b-web` named test target changed from `content_ids_resolve` (legacy) to `refusals` (ti4-host).
- Docs: `docs/ENGINE_LAYOUT.md` (new), `crates/ti4-engine/README.md`, `README.md` pointer, fixer and summary prompts, `docs/ENGINE_KNOWN_ISSUES.md` E2 note. Historical RESUME/INTEGRATION docs deliberately left as written.
- Verified with `cargo metadata --no-deps`: every `-p` package and named `--test` in every alias exists (no builds).

## To verify at integration
- `cargo t-web`/`l-web` green without legacy in the set (needs 9b: until then legacy is still built transitively through the facade).
- `t-web-slow`: the `slow_` tests are in server/wasm and ran with legacy opt-level 1; check the speed with the new keys.
- `b-web --test refusals` resolves and shares features with `t-web`.
- `t-legacy` green; `t-dev` green (the bridge may need `--features` that the dev set does not enable; adjust there).
- `tools/engine_core/{verify_package,verify_stage,end_of_d1}.sh` package lists and `clippy_core.sh` were not touched here (plan item, may name aliases).

## Join notes
- Touches only `.cargo/config.toml` and text files; integration 21 owns Rust and manifests. Conflicts expected only if 21 edits the same README/prompts.
- Nightly cron restore at 22:30 Berlin: checklist in `docs/ENGINE_LAYOUT.md`; CLAUDE.md untouched.
