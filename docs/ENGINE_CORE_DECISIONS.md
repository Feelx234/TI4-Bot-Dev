# Typed decisions: the model as built by F4, and how the rule bot (D3) uses it

Branch `core-f4-decision-model`. Extends `ENGINE_CORE_STATE.md` section 2. Code: core
`crates/ti4-engine-core/src/choice/{mod,generic,<area>}.rs`, adapter `crates/ti4-engine/src/render/{mod,generic,<area>}.rs`,
harness `crates/ti4-engine/tests/decision_replay.rs`.

## What exists

| Type (core, `choice::`) | Notes |
|---|---|
| `Choice { actor: Seat, ask: Ask, ctx: Ctx, opts: Opts }` | one decision point; `pick(i)`, `can_decline()`, `decline_index()` |
| `Ask`, `Opt`, `Subtype` | one variant per area, generated from the single `areas!` list in `choice/mod.rs` (18 areas + `Generic`); `.area()` on each |
| `Opt::{Decline, Done, Number(i32)}` | the generic families; everything else is `Opt::<Area>(area::Opt)` |
| `Ctx` | `origin: Origin`, `subtype`, `optional`, `target: Option<Target>`, `outstanding: SmallList<Owed,4>`, `space_battle`, `invasion_seq` (phase/round are `Meta`, the actor is `Choice.actor`) |
| `Pick` | `Opt(Opt)` or `Number(i32)`; the free-form transaction deal gets its variant when B4 defines the deal type |
| `Opts` | `SmallList<Opt,16>`, today's order, `index_of` |

All `Copy` except `Ctx`/`Opts`/`Choice` (inline lists). The per-area `Ask/Opt/Subtype` are uninhabited placeholders
until the owner fills them; the adapter's `Unported<..>` family makes every unported area type-check with no code.

Adapter (`render::`): `trait Family { prompt, subtype, option, parse, wording, numbers, details, free_form }`, the
`registry!` macro (one line per area: replace `unported!(x)` by `x::Fam`), `Render` (owner table + code interner),
`render_choice`, `parse`, `parse_pick`, and `Typed { choice, rendered }`.

Deviations from the section 2.3 sketch (all forced by the corpus):
* The text of a generic option depends on the ask ("Decline" vs "take the hit", id `done` with three labels), so the
  family supplies a `Wording` (kind/label/payload overrides); the id is fixed by the generic family.
* `Subtype` text can be parametric (`reaction_after_<EVENT>`, `<card>_choose_reward`), so `Family::subtype` returns an
  owned text, not a `&'static str` table. 325 distinct subtype texts exist in the corpus.
* `Origin::Content/FactionAbility` hold a `generic::Content` (record kind found by looking the alias up in the
  generated tables; unknown aliases are `Other(Code)`). `Origin::Reaction` holds `EventKind`; until A5 numbers events the
  adapter interns the event name (`Render::code`). `generic::Rule` lists the 29 LRR references seen in the corpus
  (append-only, through the steward).
* Numbers are an `Opt` only where the ask says so (`Family::numbers`): system ids like `29` are numeric text too.

## What the corpus shows (114 654 decisions, 654 829 options, 10 sources)

* `decline`-kind options come with 30+ ids (`decline`, `done`, `done_moving`, `refuse`, ...). Only `decline`/`decline`
  and `done`/`decline` are generic (22 380 and 871 occurrences); the rest are area options that happen to be declines.
* count options (`kind=count`, canonical decimal id): 48, all Bribery.
* Contexts: all 108 892 present contexts decode into the typed `Ctx` and encode back to the recorded JSON.

## For the rule bot: how D3 uses the typed model

Today `ti4-rule-bot` receives `&Choice` (strings) and spends its time on `id.split('|')`, label contains-checks and
prompt matching (BOT_STRING_AUDIT items 7-8, ~120 parse sites). The typed model removes the need:

1. **Input.** The driver hands the bot a `render::Typed`: the typed `core::Choice` and the rendered string `Choice`,
   index-aligned (`rendered.options[i]` is `choice.opts[i]`). The bot reads `ask` (what is being asked, with parameters),
   `ctx.subtype`/`origin`/`target`/`outstanding` instead of `context.subtype` text, and `opts[i]` instead of parsing
   `option.id`. `Opt` is `Copy + Eq + Ord + Hash`, so a bot can use it as a map key (feature caches, policy tables).
2. **Dispatch.** `match (choice.ask, opt)` per area replaces the `kind`/prompt classification; area modules in the bot
   map one-to-one onto `choice/<area>.rs`. The bot's router (`other` catch-all) becomes `choice.ask.area()`.
3. **Facts without text.** Opt variants carry typed ids (`Exhaust { planet }`, `Move { origin, index, route }`), so
   evaluation reads the board with `CoreView` queries directly (no `PlanetId::new(&str)`, no `BTreeMap` lookups).
4. **Output.** The bot returns an index (or `Pick::Opt(opts[i])`); the adapter turns it into the option id with
   `Typed::option(i)`, so the engine, the decision log and fingerprints see exactly today's strings.
5. **Migration.** The bot ports area by area with the engine: an area that is ported produces `Typed` with a real
   `Ask`; unported asks reach the bot as `Typed` with no core side, and the bot keeps its string path for them
   (`Typed::choice` is `Option` at the seam, added by D3 when the first area lands, because today `Unported` areas
   have no typed value). Rule-bot A/B (`bot_version`, bench-v1..v5) must stay 200/200 identical per area, since the
   picks are the same options in the same order.
6. **What F4 gives D3 now:** the types, `Typed`, `Opts::index_of`, `Choice::can_decline`; nothing in the bot changes
   in F4.

## Harness

`cargo test -p ti4-engine --test decision_replay` (set `TI4_DECISION_CORPUS`; `TI4_REQUIRE_CORPUS=1` makes a missing
corpus an error). Per decision: manifest sha/count, recorded V1/V2 recomputed, generic option round trip and
rendering, context round trip, and the area lifters (`LIFTERS`, empty): a ported area adds
`fn(&Render,&Choice)->Option<core::Choice>` and gets `render_choice(lift(c)) == c` checked on every decision it claims.
