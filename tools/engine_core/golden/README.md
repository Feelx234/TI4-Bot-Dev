# Golden games (package D4-4)

Frozen, per-`RULES_VERSION` recordings of whole games. `golden check` replays a game **by its recorded answers** (it never
runs the bot, so a bot change cannot move a snapshot) and compares the state hash and the V2 hash of every question.
Design: `docs/todo/D4-core-tests.md` 4. Tool: `crates/ti4-engine-core-bridge/src/bin/golden.rs` (library `ti4_engine_core_bridge::golden`).
Run everything from the repo root:

```text
cargo build --profile lockstep -p ti4-engine-core-bridge --bin golden       # CARGO_TARGET_DIR private, see CLAUDE.md
G=$CARGO_TARGET_DIR/lockstep/golden
$G list                                  # sets.json as a table
$G check                                 # every set, process-default mask (TI4_AREAS)
$G check --mask all=core                 # the typed core everywhere, bails fall to the string
$G check --mask all=string               # pure string engine; both must reproduce every hash
$G check v35-std --games g003,g007       # one set, some games   (--smoke: the 4 smallest games of each set; --static: no engine)
$G diff v35-std [--full]                 # the games that differ: first divergent step, subtype (area), decisions affected
```

## Layout

```text
sets.json            index (written by `golden index`); CHANGES.md   append-only bless log
<set>/set.json       tier, rules_version, frozen, games (file, steps, decisions, decisions_hash, bytes, sha256), smoke
<set>/gNNN.jsonl.zst full tier: header line + the X2 corpus line per decision (rendered Choice, chosen, V1, V2, state hash)
<set>/gNNN.tsv.zst   slim tier: header + `#S step state16` every 8th step + `step k chosen v2 subtype-index` per decision
legacy-sources/<source>/   the X2 sources (bench*, te, templates, fixtures) split per game; v23-golden20 is the X2 `golden20`
ladder/              one short slim game per rules version 0..=RULES_VERSION (each header pins its own version)
```

Full states are never stored (hidden information, size): hashes and rendered per-actor Choices only. Every header pins the
rules version the game was played at (T7); a set has exactly one (the ladder is the one exception).

## When a rule fix moves a snapshot

Behaviour fixes are gated by `RULES_VERSION`, so sets recorded at an older version never move. Sets are append-only per version:
the set recorded at N is frozen the moment `RULES_VERSION` becomes N+1 (`golden index` writes the flag).

```text
golden diff SET                          replay and list the games that no longer reproduce
golden bless SET --reason "..." --expect SUBTYPE[,SUBTYPE..] [--issue ID] [--bugfix] [--games g003,..]
golden bless-new --set v36-std --config tools/engine_profile/soak/std-100.json --tier slim [--require-dual-clean all]
```

`bless` re-records ONLY the games that differ (bot-driven, with the bot and limits pinned in the header: a changed rule changes what
is offered, so the old answers may be illegal; the whole file is replaced). It **refuses**

* a frozen set (below the current `RULES_VERSION`, or flagged) unless `--bugfix --issue ENGINE_KNOWN_ISSUES-ID`;
* a change in subtypes outside `--expect` (collateral); `--expect` is mandatory, `'*'` accepts any explicitly;
* a missing `--reason`;
* uncommitted changes under `tools/engine_core/golden/` (the rule fix itself is expected in the tree, outside this directory).

and appends one block to `CHANGES.md` (Berlin date, commit, set, games, histogram of changed subtypes, reason, issue). The commit
that blesses contains only `tools/engine_core/golden/**` and the rule fix; every changed subtype in `CHANGES.md` must be named in
the fix's commit message. A `RULES_VERSION` bump: run `bless-new` for the new version (the old sets freeze), and the test
`every_version_has_a_set` fails until a set and a ladder game exist for the new number.

## Recording

```text
golden record --set v35-std --config tools/engine_profile/soak/std-100.json --games 0..60 --tier slim \
              --require-dual-clean all [--shard I/N] [--workers N] [--rules-version N] [--bot-version V]
golden finalize v35-std                  # after sharded records: builds set.json, reindexes
golden ladder [--require-dual-clean all] # records the missing versions only (--force: all)
golden import-corpus [--from tools/engine_core/decision_corpus] [SOURCE...]   # idempotent; lines byte-identical, proven against the manifest sha256
```

`--require-dual-clean MASK` (`all` = `all=dual`) plays each game under that mask first and records it **only if the dual run saw
no difference** (a recorded difference would freeze a bug, T13; the excluded games are listed in `<set>/excluded.txt`), then plays it
under `all=core` and demands identical lines (a mismatch is a porting bug and fails the record). The dual report is process-wide, so such
a record plays one game at a time per process: parallelise with `--shard I/N` processes. Heavy: take `flock /root/.ti4-heavy.lock`.

## Tests

`cargo test --profile lockstep -p ti4-engine-core-bridge --lib golden::` : format round trips, bless guards on a fake engine, the importer, the
ladder, `every_version_has_a_set`, `every_golden_file_matches_its_set_json` (sha256, counts, versions, Choice round trip and V1/V2 of
the full tier; no engine) and `the_smoke_subset_reproduces` (the smallest games of every set on the string engine).
`decision_corpus check tools/engine_core/decision_corpus` keeps working on the old files until D4-8 removes them.
