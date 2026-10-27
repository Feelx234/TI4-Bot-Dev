# Decision families: the grammar of every option the string engine offers

Package X2 (`core-x2-decision-corpus`) of `docs/ENGINE_CORE_STATE.md`. This is the inventory of the
option-building sites of `ti4-engine` (the "736 sites" of `docs/ENGINE_CORE.md`), the grammar they
produce, and the recorded corpus that proves a typed port renders the same bytes.

The tables from "Corpus totals" down are generated from the corpus and the sources
(`python3 tools/engine_core/decision_corpus/analyze.py --write-doc`); everything above is hand-written.

## 1. What this is for

`Choice { player, prompt, options[ {id, kind, label, payload} ], context, details, free_form }` is the
wire contract and the input of both fingerprints. A typed `Ask`/`Opt` model (section 2 of the state
design) must render, for every decision, exactly the same options in the same order. This document
fixes **what "the same" means per family** and the corpus makes it a test: for every recorded decision,
`render(opts(state)) == recorded Choice JSON`, `parse(option(o).id) == o`, and the V1/V2 hashes follow.

## 2. The recorded corpus

Location: `tools/engine_core/decision_corpus/`. Recorder and checker: the bin
`crates/ti4-engine-core-bridge/src/bin/decision_corpus.rs` (`decision_corpus record|verify|check DIR`).

| File | What | Content |
|---|---|---|
| `golden20.jsonl.zst` | the 20 golden games of `tools/engine_profile/profile-20.json` (4p/6p, rules version 23), every decision | all 20 hashes equal `tools/rulebot/baseline.json` |
| `bench.jsonl.zst`, `bench-v1..v5.jsonl.zst` | the six bench configurations of `tools/rulebot/selfplay.sh` (bot versions 4/5/4/1/2/3 as in `verify_benches.sh`), four games each (indices 20, 21, 120, 121: seeds 21 and 22 of the 4p and 6p setups, not in `golden20`) | every game hash equals the matching `baseline*.json` |
| `templates.jsonl.zst` | the first 60 steps of the opening of every map template the host seats, with and without Thunder's Edge, factions rotated through the lobby pool | |
| `te.jsonl.zst` | four 6-player Thunder's Edge tables with starting technology, seating all 24 lobby factions (R11 of the design), 5000 steps each | |
| `fixtures.jsonl.zst` | the `Choice` objects inside the checked-in wire fixtures (`fixtures/protocol`, server, host, wasm tests) | no state: recorded as the engine's types read and write them |
| `manifest.json` | per source: games, decisions, raw and compressed bytes, SHA-256 of the uncompressed stream, per game steps and the decision hash of the game | |

One JSON line per decision: `{"s", "g", "step", "k", "state", "choice", "chosen", "v1", "v2"}`.

* `choice` is the rendered `Choice` exactly as `serde_json` writes it (option order, payload keys sorted,
  int vs float number types, `details`, `context`, `free_form`).
* `state` = first 16 hex digits of SHA-256 over the serialised `GameState` at the **start of the step**
  that asked. A decider cannot see the state, and the driver reconciles derived facts (stations, Nexus,
  Keleres) before asking, so the hash is of the state the step starts from; the decisions of one step
  (`k` = 0, 1, ...) share it. To rebuild the state, replay the game (`g`) to `step`: the games are seed
  deterministic, which `verify` proves.
* `chosen` is the option the rule bot picked (the next step of the game); `v1`/`v2` are the first 16 hex
  digits of `fingerprint::decision_hash` V1 and V2 of the `DecisionRecord` the engine logs for it.
* Size: about 4.7 MB compressed (zstd level 19, long window; 114 654 decisions, about 150 MB raw). It is
  stored compressed because the lines repeat heavily; the manifest holds the SHA of the raw stream.

Commands (the cargo target dir must be private and outside the live checkout):

```text
cargo build --release -p ti4-engine-core-bridge --bin decision_corpus
D=tools/engine_core/decision_corpus
decision_corpus check  $D                   # files vs manifest, Choice round trip, V1/V2 recomputed (seconds, no game play)
decision_corpus verify $D --workers 6       # replays every source with the current string engine, compares line by line
decision_corpus record $D [SOURCE...]       # re-record (only on a deliberate behaviour change, through the baseline process)
```

`verify` is the gate of this package and the dual-run input of every later one: a typed family that is
on `Core` must make `verify` pass with `render` in place of the string builder.

## 3. How the grammar is laid out (conventions found in the corpus)

Distilled from the generated tables below; each is a property the typed port keeps.

1. **Ids are `|`-, `:`- or `>`-separated segments** and the separator set is part of the kind: `move|02|0`,
   `exhaust|lisisii`, `build|fighter|2`, `leader:saar:saaragent:SYSTEM_ACTIVATED:after`, `ingress_move 18>54`.
   A kind may have several shapes (see "Kind index"): `parse` is per shape, and the shape is chosen by
   segment count and literal head (`move`/`move_gd`, `exhaust`/`trade_good`, `build`/`deploy`).
2. **Integers inside ids are zero-padded or plain exactly as the engine formats them** (`02` system tiles,
   `0` indices). Round trip is on the text: `format!` of the parsed value must reproduce the id.
3. **Kinds are strings the engine builds**, some from a format: `leader_<faction><role>_<what>`,
   and two kinds contain a space (`strategy card`, `promissory note`), older siblings of `strategy_card` and
   `promissory_note`. Kinds are part of the wire bytes; the typed model maps `Opt` variant -> kind text.
4. **`decline` is a kind with several ids** (`decline`, `done`, `done_committing`, ...): the kind is fixed,
   the id is not. It is last in the vast majority of decisions, first in a few, in the middle in some
   (counts in "Corpus totals"): its position is part of the order rule of the family, not a global rule.
5. **Order is per family and of two sorts**: ids ascending as text/number (the builder iterates a
   `BTreeMap`/sorted list) or insertion order of the rule (the builder pushes in rule order). The tables
   report, per kind, what the corpus shows; "ids ascending" only proves the order is consistent with a
   sort, it cannot tell a sort from a sorted insertion, so the typed `opts()` must be written to the
   builder's loop, not to the sort, and the corpus decides. Multi-kind decisions (`activate` + `decline`,
   `action` mixtures) concatenate groups; "interleaved with other kinds" marks families where the groups
   are not contiguous.
6. **Payload values are typed**: `int`, `str`, `bool`, `array`, and **`float`** in a few keys (`net`,
   `their_net`, `opportunity_cost`). `Value::from(1)` and `Value::from(1.0)` serialise differently
   (`1` vs `1.0`), so the adapter must build the same `Value` variant. See "Payload keys by type".
   Payload is excluded from `ChoiceOption` equality but is on the wire and in `Choice` JSON, so it is in
   the corpus comparison.
7. **Labels are display text assembled from names and counts**; they are never parsed back, but they are on
   the wire, so the corpus compares them. The template column shows ids folded to `<id>`, numbers to `#`.
8. **Prompts are in the V1 and V2 fingerprints**: 1.4k distinct templates after folding numbers, event names
   and ids; the long tail is card/planet names in prose. Prompt text is produced by `Family::prompt`.
9. **`context.subtype` / `source`** are fixed vocabularies (subtype counts are in every block); they come from
   `Subtype::as_str`, never from the prompt.
10. **`free_form`** choices (a transaction deal, a number entry) accept unlisted ids with a prefix: they are
    `Pick::Number`/`Pick::Amount` in the core and parsed with today's error text.
11. **Single-option decisions are auto-resolved** before they are asked; the corpus holds only decisions the
    engine asked (`decisions_hash` of a game counts them).

## 4. Gaps and traps found while recording

* `Bot::decide` in `workload.rs` and `engine_profile.rs` reset the thread tuning to the
  shipped version at every decision (`tuning::set(tuning::current())`): harmless with the default bot,
  wrong for `bot_version=N`. The corpus recorder installs the tuning once per worker thread, as
  `selfplay run` does; bench hashes then equal the baselines for all six configurations. Not fixed in the
  other two files (out of scope, tools only).
* The decisions of one step share a state hash (above); a decision that comes from `choose` rather than
  `choose_seeing` (windows without a position) is recorded the same way.
* The kinds in the corpus are the ones the rule bot's games reach. Kinds that need rare cards, leaders or
  Thunder's Edge faction text appear only through `te` and `templates`; kinds never offered are listed
  in the source inventory but have no recorded example (the dual run over engine unit tests covers
  them).

<!-- generated by tools/engine_core/decision_corpus/analyze.py: do not edit by hand -->

### Corpus totals

| Source | Decisions |
|---|---:|
| bench | 9663 |
| bench-v1 | 8039 |
| bench-v2 | 8145 |
| bench-v3 | 9855 |
| bench-v4 | 9363 |
| bench-v5 | 8940 |
| fixtures | 8 |
| golden20 | 42680 |
| te | 9944 |
| templates | 8017 |
| **all** | **114654** |

Offered options: 654829; distinct option kinds: 134; distinct prompt templates (digits folded): 1451.

Fields present on a rendered `Choice`: `context` (108892), `details` (71186), `free_form` (1847), `options` (114654), `player` (114654), `prompt` (114654).

Context fields present: `actor` (108892), `hit` (63), `invasion_seq` (9173), `optional` (108892), `outstanding` (108892), `phase` (108892), `previews` (1865), `round` (108892), `source` (108892), `space_battle` (1619), `subtype` (108892), `target` (108892), `trigger` (8811), `version` (108892).

`details` keys: `ability` (12), `action_card_effects` (17617), `actions_taken` (17617), `active` (7012), `agenda` (8281), `capacity` (4734), `capacity_consumed` (940), `capacity_transport` (940), `captions` (599), `card` (6175), `closing` (17617), `commodities` (1534), `controls_mecatol` (174), `cost` (12), `costs_token` (5570), `current` (53), `dice` (65), `distinct_owed` (70), `faction` (84), `facts` (599), `first` (18), `fleet_charged` (940), `fleet_limit` (940), `fleet_supply` (14128), `fleet_supply_bonus` (173), `free_form` (1847), `goods` (6), `held` (84), `holders` (1), `kind` (35135), `laws` (3), `misses` (12), `mode` (12002), `movable` (7677), `not_offered` (57), `of` (708), `outcome` (29), `partners` (17617), `pick` (84), `played_by` (5588), `pools` (12002), `purchase` (3279), `ready_hints` (1602), `reason` (940), `reinforcements` (12002), `rift_exits` (517), `routes` (1194), `rule` (84), `seats` (525), `secrets_held` (174), `secrets_max` (174), `step` (624), `strategy_cards` (17617), `sustainers` (606), `system` (940), `tally` (4430), `tokens` (17617), `tokens_left` (5570), `tokens_to_place` (9379), `total` (2623), `unit_limits` (4734), `units` (54), `votes` (29), `votes_added` (23), `votes_per_good` (6), `votes_so_far` (4430).

Position of `decline` within a decision: last 51847, middle 1421, first 47.

### Kind index

| Kind | Options | Decisions | Id grammar (separator-split segments) |
|---|---:|---:|---|
| `activate` | 166872 | 4792 | `89 distinct (int/name)` x166872 |
| `decline` | 53315 | 53315 | `8 distinct (name) e.g. decline, done, done_committing` x53315 |
| `redistribute` | 48128 | 2623 | `int 0..13`\|`int 0..15`\|`int 0..13` x45358; `{fleet,strategy,tactic}`\|`{fleet,strategy,tactic}` x2770 |
| `open_transaction` | 41224 | 15798 | `{component}`\|`{trade}`\|`24 distinct (name) e.g. arborec, argent, cabal` x41224 |
| `pay` | 40967 | 9103 | `{exhaust}`\|`99 distinct (name)` x35407; `{trade_good}` x5227; `{exhaust}`\|`63 distinct (name)`\|`{influence,resources}` x333 |
| `produce` | 30187 | 4842 | `{build,deploy}`\|`78 distinct (name)`\|`int 1..2` x29417; `{exchange}`\|`8 distinct (name) e.g. cabal_flagship, cabal_mech, carrier` x770 |
| `offer` | 29723 | 1847 | `114 distinct (name)`:`24 distinct (name) e.g. arborec, argent, cabal`:`int 0..4` x13944; `29 distinct (name) e.g. nnacq, nnambuscade, nnan`:`24 distinct (name) e.g. arborec, argent, cabal`>`29 distinct (name) e.g. acq, ambuscade, an`:`24 distinct (name) e.g. arborec, argent, cabal` x7183; `30 distinct (int/name) e.g. 0, 1, 2`:`int 0..3` x7095; `32 distinct (name) e.g. cnabs, cnassassin, cnbunker`>`50 distinct (name)`:`14 distinct (int/name) e.g. 0, 2, 3` x768; +1 more shapes |
| `vote_planet` | 20943 | 4430 | `86 distinct (name)` x20943 |
| `load` | 20748 | 8847 | `{load}`\|`int 0..35` x20748 |
| `pool` | 20157 | 6719 | `{fleet,fleet_tokens,strategic,strategic_tokens,tactic,tactic_tokens}` x20157 |
| `action` | 17909 | 9447 | `{pass,strategic,tactical}` x14593; `{strategic}`\|`8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x3316 |
| `strategy` | 16039 | 7859 | `{no,yes}` x15689; `{follow}`\|`{waived}`\|`int 0..0`\|`{agent}`\|`47 distinct (int/name)` x249; `{follow}`\|`{waived}`\|`int 1..1`\|`{ms}`\|`{an,cf,gift,pop,ps,ta}`:`{arborec,mentak,naalu,saar}` x77; `{follow}`\|`{waived}`\|`int 3..3`\|`{acq}` x24 |
| `move` | 14863 | 7682 | `{move,move_gd}`\|`80 distinct (int/name)`\|`int 0..18` x14854; `{move}`\|`{alioprima,bakal,meer,vegamajor,vegaminor,velnor}`\|`{alioprima,arinam,bakal,lisis,vegamajor,vegaminor}`\|`{infantry,l1z1x_mech,saar_mech}` x9 |
| `strategy_card` | 14564 | 2726 | `8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x14519; `{a,b,c,d,e,f}`\|`8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x45 |
| `commit` | 12757 | 5581 | `{commit}`\|`int 0..10`\|`82 distinct (name)` x12757 |
| `build` | 11753 | 1231 | `{pds,spacedock}`\|`63 distinct (int/name)`\|`99 distinct (name)` x11005; `{produce,titans}`\|`57 distinct (int/name)` x437; `{pds,structure}` x311 |
| `component` | 11470 | 5902 | `{action_card,faction,purge,relic}`\|`20 distinct (int/name) e.g. 0, 1, 2` x7041; `{component,faction}`\|`9 distinct (name) e.g. cabal, expedition, ghost`\|`42 distinct (name)` x3999; `{component,faction}`\|`{leader,mahact,titans,yssaril}`\|`{bt_stall,mi,terraform,vaults,yssarilagent}`\|`43 distinct (name)` x426; `{c,d,e}` x4 |
| `research` | 9446 | 645 | `76 distinct (name)` x9446 |
| `ability` | 9153 | 8860 | `7 distinct (name) e.g. ability, breakthrough, leader`:`24 distinct (name) e.g. arborec, argent, cabal`:`74 distinct (name)`:`39 distinct (name) e.g. ACTION_CARD_PLAYED, ACTION_COMPLETED, AGENDA_REVEALED`:`{after,produce,when}` x5189; `{reaction}`:`24 distinct (name) e.g. arborec, argent, cabal`:`32 distinct (name) e.g. ACTION_CARD_DISCARDED, ACTION_CARD_PLAYED, ACTION_COMPLETED`:`{after,when}` x3501; `{leader}`:`8 distinct (name) e.g. argent, ghost, mentak`:`7 distinct (name) e.g. a, b, c`:`{l1z1xagent,letnevagent,solagent,yssarilagent}`:`12 distinct (name) e.g. argentagent, b, c`:`10 distinct (name) e.g. COMBAT_ROUND_STARTED, GROUND_COMBAT_ROUND_STARTED, GROUND_FORCES_BEING_PRODUCED`:`{after,when}` x225; `19 distinct (name) e.g. ang, archonvail, arinam` x210; +1 more shapes |
| `vote` | 8253 | 1518 | `100 distinct (name)` x8253 |
| `end_turn` | 8184 | 8184 | `{end_turn}` x8184 |
| `ready` | 5139 | 1181 | `100 distinct (name)` x4950; `{ready}`\|`{planet}`\|`18 distinct (name) e.g. ang, archonvail, arnor` x189 |
| `planet` | 3452 | 296 | `82 distinct (name)` x1828; `58 distinct (int/name)`\|`88 distinct (name)` x1623; `{meer}`:`{WARFARE}` x1 |
| `legendary` | 3110 | 1278 | `60 distinct (int/name)`\|`92 distinct (name)` x1771; `8 distinct (name) e.g. card, convert, goods` x1339 |
| `transaction` | 2842 | 1421 | `{accept,counter}` x2842 |
| `retreat` | 2661 | 1333 | `{retreat,stay}` x2661 |
| `system` | 2292 | 366 | `84 distinct (int/name)` x2265; `{system}`\|`int 9..79` x27 |
| `creuss_token` | 1867 | 91 | `{ALPHA,BETA}`\|`int 1..80` x1867 |
| `discard` | 1862 | 243 | `int 0..11` x1862 |
| `explore` | 1604 | 902 | `39 distinct (name) e.g. aw1, aw2, aw3` x1604 |
| `return` | 1517 | 489 | `40 distinct (name)` x1517 |
| `score` | 1481 | 1039 | `66 distinct (name)` x1481 |
| `remove` | 1370 | 940 | `{remove}`\|`int 0..7` x1370 |
| `commit_infantry` | 1359 | 33 | `int 18..76`\|`60 distinct (name)` x1359 |
| `transit` | 1161 | 16 | `{transit}`\|`int 12..75`\|`17 distinct (name) e.g. accoen, ashtroth, cealdri`\|`{infantry,jolnar_mech}`\|`int 12..75`\|`17 distinct (name) e.g. accoen, ashtroth, cealdri` x1161 |
| `replenish` | 1088 | 311 | `24 distinct (name) e.g. arborec, argent, cabal` x1088 |
| `ship` | 994 | 121 | `41 distinct (int/name)`\|`37 distinct (int/name) e.g. 0, 1, 10` x977; `{stop}` x17 |
| `ground_casualty` | 953 | 953 | `{fight}` x953 |
| `agenda` | 856 | 428 | `{bottom,top}` x856 |
| `speaker` | 793 | 214 | `24 distinct (name) e.g. arborec, argent, cabal` x793 |
| `player` | 702 | 191 | `21 distinct (name) e.g. a, arborec, b` x702 |
| `casualty` | 687 | 357 | `{destroy}`\|`int 0..5` x687 |
| `technology` | 672 | 72 | `56 distinct (name)` x633; `{a,b,c,d,f}`\|`13 distinct (name) e.g. amd, bs, cr2` x39 |
| `sustain` | 671 | 606 | `{sustain}`\|`int 0..9` x512; `{sustain}`\|`{space}`:`int 0..8` x159 |
| `prediction` | 668 | 118 | `65 distinct (name)` x668 |
| `retreat_to` | 541 | 219 | `int 1..78` x541 |
| `own_hand_card` | 485 | 105 | `120 distinct (name)` x485 |
| `promissory_note` | 476 | 414 | `{use}` x400; `{note}`\|`14 distinct (name) e.g. an, bmf, cavalry`:`14 distinct (name) e.g. arborec, empyrean, ghost` x76 |
| `stillness` | 421 | 66 | `22 distinct (name) e.g. argent_destroyer, argent_destroyer2, argent_mech` x421 |
| `place_unit` | 407 | 121 | `50 distinct (int/name)`\|`80 distinct (name)` x407 |
| `starting_technology` | 364 | 84 | `8 distinct (name) e.g. aida, amd, det` x364 |
| `leader_ssruu_round_agent_unit` | 351 | 82 | `{a,b,c,d,e,f}`\|`39 distinct (name) e.g. abyz, accoen, arinam`\|`32 distinct (name) e.g. argent_destroyer, argent_flagship, argent_mech`\|`int 0..5`\|`int 0..2` x351 |
| `tiebreak` | 320 | 58 | `74 distinct (name)` x320 |
| `mercer_take` | 317 | 52 | `{take}`\|`int 18..76`\|`39 distinct (name) e.g. abyz, accoen, alioprima`\|`{infantry,nomad_mech}`\|`int 0..1` x317 |
| `sleeper` | 268 | 27 | `59 distinct (name)` x243; `{move}`\|`13 distinct (name) e.g. alioprima, ashtroth, bakal` x25 |
| `place` | 267 | 117 | `{place}`\|`55 distinct (name)` x244; `int 7..71` x23 |
| `infantry` | 248 | 18 | `int 1..76`\|`26 distinct (name) e.g. 0.0.0, accoen, archonvail`\|`int 0..20` x239; `{stop}` x9 |
| `objective` | 232 | 174 | `25 distinct (name) e.g. amass_wealth, ancient_monuments, build_defenses` x232 |
| `pillage` | 218 | 109 | `{commodity,trade_good}`\|`{a,b,c,d,e,f}` x218 |
| `action_card` | 213 | 60 | `57 distinct (name)` x213 |
| `recombine` | 207 | 49 | `{recombine}`\|`39 distinct (name) e.g. a, abyz, accoen` x207 |
| `skilled_retreat` | 196 | 52 | `58 distinct (int/name)` x196 |
| `leader_hacanagent_branch` | 194 | 35 | `9 distinct (name) e.g. a, b, c` x194 |
| `mitosis` | 139 | 13 | `{infantry,mech}`\|`int 5..74`\|`26 distinct (name) e.g. abyz, arinam, bereg` x139 |
| `reparations_exhaust` | 137 | 16 | `60 distinct (name)` x137 |
| `promissory note` | 134 | 26 | `13 distinct (name) e.g. an, ce, cf`:`9 distinct (name) e.g. cabal, ghost, l1z1x` x134 |
| `component_action` | 133 | 133 | `{faction}`\|`{empyrean,mentak}`\|`{blood_pact,dark_pact,pop}` x133 |
| `economy` | 100 | 50 | `{convert,gain}` x100 |
| `nekro_malleon` | 95 | 28 | `{token}`\|`{fleet,strategy,tactic}` x82; `{discard}` x13 |
| `indoctrination` | 93 | 60 | `{infantry,mech}` x93 |
| `silence` | 89 | 26 | `int 1..76` x89 |
| `leader` | 83 | 83 | `{skip,use,yes}` x83 |
| `ready_technology` | 82 | 66 | `{ready}`\|`{technology}`\|`{gr,pi}` x82 |
| `agent` | 75 | 75 | `{exhaust}` x75 |
| `leader_jolnarhero_swap` | 75 | 11 | `26 distinct (name) e.g. aida, amd, asc` x64; `{keep}`\|`{amd,gd,gls,nm,ps,st}` x11 |
| `ambush_roll` | 73 | 60 | `{cruiser,cruiser+cruiser,cruiser2,cruiser2+cruiser2}` x73 |
| `self_assembly` | 70 | 43 | `13 distinct (name) e.g. 0.0.0, acheron, alioprima` x70 |
| `tribute` | 68 | 34 | `{tribute}`\|`{token,vote}` x68 |
| `flock_migration_ship` | 68 | 14 | `{ship}`\|`int 0..7` x68 |
| `stymie` | 62 | 2 | `int 17..76` x62 |
| `recall` | 54 | 16 | `int 1..77` x54 |
| `law` | 53 | 53 | `{discard}` x53 |
| `reroll_die` | 53 | 53 | `{reroll}`\|`int 0..1`:`int 0..5` x53 |
| `annex` | 50 | 15 | `24 distinct (name) e.g. abaddon, archonvail, arinam` x50 |
| `count` | 48 | 14 | `int 0..4` x48 |
| `ingress_system` | 48 | 20 | `int 19..76` x48 |
| `custodians` | 47 | 47 | `{yes}` x47 |
| `unit` | 47 | 34 | `{pds,spacedock}`\|`int 18..75`\|`14 distinct (name) e.g. accoen, bereg, corneeq` x30; `{deploy,use}` x17 |
| `follower` | 46 | 23 | `{no,yes}`\|`{a,b,c,d,e,f}` x46 |
| `breakthrough` | 45 | 45 | `{allow}` x45 |
| `leader_xxchaagent_infantry` | 44 | 22 | `{keep,remove}` x44 |
| `elect` | 37 | 15 | `{a,b,c,d,e,f}` x37 |
| `reparations_ready` | 36 | 7 | `28 distinct (name) e.g. abyz, alioprima, archonren` x36 |
| `mercer_place` | 34 | 16 | `15 distinct (name) e.g. abyz, arinam, arnor` x34 |
| `tether` | 34 | 4 | `{move}`\|`int 18..54`\|`int 56..65`\|`int 18..60`\|`int 30..72` x22; `{place}`\|`int 18..60`\|`int 60..65` x12 |
| `scuttle` | 26 | 10 | `int 0..3` x26 |
| `vortex` | 24 | 8 | `{a,b,c,d,f}`\|`9 distinct (name) e.g. argent_destroyer, carrier, cruiser` x24 |
| `ingress_move` | 24 | 4 | `int 18..76`>`int 54..54` x24 |
| `vote_predictive` | 23 | 23 | `{predictive}`\|`{cast}` x23 |
| `flock_migration_to` | 22 | 11 | `{to}`\|`int 14..73` x22 |
| `fragment` | 21 | 9 | `{CULTURAL,HAZARDOUS,INDUSTRIAL}` x21 |
| `production_agent` | 21 | 21 | `{exhaust_agent}`\|`{winnuagent}` x21 |
| `hero` | 20 | 10 | `{discard,take}` x20 |
| `vote_trade_goods` | 19 | 6 | `{spend}`\|`int 1..5` x19 |
| `research_waiver_payment` | 18 | 4 | `{ground}`\|`int 3..76`\|`13 distinct (name) e.g. centauri, darien, hopesend` x17; `{space}`\|`int 76..76` x1 |
| `take_revealed_card` | 17 | 6 | `16 distinct (name) e.g. assassin, confusing, cripple` x17 |
| `flock_migration_cargo` | 16 | 6 | `{cargo}`\|`int 0..4` x16 |
| `nekro_assimilate` | 14 | 7 | `{gain,z}`\|`{c,d,e,f}`\|`{amd,gd,naalu,nm,sr,st}` x14 |
| `letnev_commander_trade_good` | 11 | 11 | `{gain}` x11 |
| `commander` | 11 | 4 | `{action_cards,promissory_notes,secret_objectives}` x11 |
| `research_waiver` | 10 | 10 | `{waiver}`\|`int 0..0` x10 |
| `scepter` | 9 | 2 | `int 19..76` x9 |
| `relic` | 9 | 9 | `{neuraloop,purge}` x9 |
| `repeal` | 8 | 3 | `{censure,conscription,minister_sciences,regulations,revolution,warrant}` x8 |
| `cavalry_ship` | 8 | 3 | `{carrier,carrier2,cruiser,dreadnought,ghost_flagship}` x8 |
| `produce_hit` | 7 | 7 | `{produce_hit}` x7 |
| `mahact_hero` | 7 | 3 | `int 21..74`\|`int 6..76` x7 |
| `watcher` | 7 | 3 | `{watcher}`\|`int 22..74`\|`{atlas,lodor,newalbion,tarmann,thedark,vegamajor}` x7 |
| `awaken` | 6 | 3 | `{mech,pds}` x6 |
| `salvage` | 4 | 4 | `{carrier,destroyer}` x4 |
| `extra_die` | 4 | 1 | `{unit}`\|`int 0..3` x4 |
| `star_forge` | 4 | 3 | `int 20..69`\|`{destroyer,fighters}` x4 |
| `production_discount` | 3 | 3 | `{exhaust}` x3 |
| `cruiser` | 3 | 1 | `int 0..2` x3 |
| `strategy card` | 3 | 1 | `{pok3politics,pok7technology,pok8imperial}` x3 |
| `rescue_from` | 3 | 1 | `int 37..69` x3 |
| `nomadbt` | 3 | 1 | `{pair}`\|`int 0..2` x3 |
| `produce_unit` | 2 | 1 | `{opt_carrier,opt_infantry}` x2 |
| `leader_hacanhero_free_production` | 2 | 2 | `{use}` x2 |
| `leader_l1z1xagent_copy_planet` | 2 | 1 | `{retillion,shalloq}` x2 |
| `answer` | 2 | 1 | `{no,yes}` x2 |
| `destroy_own_ship` | 1 | 1 | `{destroyer}` x1 |
| `production` | 1 | 1 | `int 11..11` x1 |
| `give_note` | 1 | 1 | `{ta}`:`{mentak}` x1 |

### Per-kind grammar

Each block: payload keys with JSON number types (`int` is serialised without a fraction, `float` with one: they are not interchangeable in the wire bytes), label templates (ids folded to `<id>`, numbers to `#`), prompt templates, order rule, subtypes, sources.

#### `activate`

- offered 166872 times in 4792 decisions; per decision 18..57 options in the whole choice
- ids: `89 distinct (int/name)` x166872
  examples: `03` / "activate 03"; `70` / "activate 70"; `60` / "activate 60"
- payload: none
- label: 6 template(s): "activate 03" -> `activate #` x150226; "free tactical action in 03" -> `free tactical action in #` x12000; "activate 82a" -> `activate <id>` x3588
- prompt: 3 template(s): "activate a system" x4412; "Warfare: a tactical action without a command token" x347; "Z'eu: perform a tactical action without placing a command token" x33
- order: insertion order x4792, contiguous x4792
- subtypes: `activate_system` x4412, `warfare_free_tactical` x347, `zeu_activate_system` x33
- sources: bench 400, bench-v1 329, bench-v2 334, bench-v3 397, bench-v4 380, bench-v5 370, fixtures 1, golden20 1679, te 383, templates 519
- built in: `strategy_cards.rs`, `tactical.rs`

#### `decline`

- offered 53315 times in 53315 decisions; per decision 1..133 options in the whole choice
- ids: `8 distinct (name) e.g. decline, done, done_committing` x53315
  examples: `done_moving` / "finish movement"; `done_loading` / "carry nothing further"; `done_committing` / "commit no more ground forces"
- payload: `ground_available`: int 8847, `loaded_fighters`: int 8847, `loaded_ground`: int 8847, `non_fighters_first`: bool 25, `system`: str 8847, `unit`: str 53; key sets: {-} x44390; {ground_available,loaded_fighters,loaded_ground,system} x8847; {unit} x53; {non_fighters_first} x25
- label: 13 template(s): "Decline" -> `Decline` x21758; "finish movement" -> `finish movement` x9394; "carry nothing further" -> `carry nothing further` x8847
- prompt: 1137 template(s): "movement" x9394; "after SHIP_DESTROYED" x7521; "commit ground forces in 70" x5601
- order: never more than one of this kind in a decision
- free_form prefixes: `x;`
- subtypes: `movement_step` x9394, `load_cargo` x8847, `commit_ground_forces` x5610, `produce_unit` x4734, `vote_exhaust_planet` x4409
- sources: bench 4385, bench-v1 3581, bench-v2 3550, bench-v3 4538, bench-v4 4207, bench-v5 4090, fixtures 7, golden20 19537, te 4821, templates 4599
- built in: `choice.rs`, `faction_effects_tests/d4.rs`, `faction_effects_tests/f3.rs`, `invasion.rs`, `production.rs`, `strategy.rs` (+1)

#### `redistribute`

- offered 48128 times in 2623 decisions; per decision 3..105 options in the whole choice
- ids: `int 0..13`|`int 0..15`|`int 0..13` x45358; `{fleet,strategy,tactic}`|`{fleet,strategy,tactic}` x2770
  examples: `0|2|6` / "tactic 0 / fleet 2 / strategy 6"; `0|3|5` / "tactic 0 / fleet 3 / strategy 5"; `0|4|4` / "tactic 0 / fleet 4 / strategy 4"
- payload: none
- label: 2 template(s): "tactic 0 / fleet 2 / strategy 6" -> `tactic # / fleet # / strategy #` x45358; "move 1 token from tactic to fleet" -> `move # token from <id> to <id>` x2770
- prompt: 4 template(s): "redistribute your command tokens" x1384; "Predictive Intelligence: redistribute command tokens" x556; "Warfare: redistribute your command tokens before the tactical action" x347
- order: contiguous x2623, ids ascending (string) x1860, insertion order x687, ids descending x76
- subtypes: `status_redistribute_tokens` x1384, `warfare_redistribute_tokens` x683, `predictive_intelligence_redistribute` x556
- sources: bench 232, bench-v1 226, bench-v2 250, bench-v3 272, bench-v4 253, bench-v5 204, golden20 840, te 184, templates 162
- built in: `technology.rs`, `tokens.rs`

#### `open_transaction`

- offered 41224 times in 15798 decisions; per decision 2..20 options in the whole choice
- ids: `{component}`|`{trade}`|`24 distinct (name) e.g. arborec, argent, cabal` x41224
  examples: `component|trade|ghost` / "open a transaction with ghost"; `component|trade|cabal` / "open a transaction with cabal"; `component|trade|yin` / "open a transaction with yin"
- payload: none
- label: 1 template(s): "open a transaction with ghost" -> `open a transaction with <id>` x41224
- prompt: 3 template(s): "action phase" x8182; "end your turn" x7599; "end your turn, or take your second action" x17
- order: contiguous x12770, insertion order x5541, ids ascending (string) x4284, ids descending x2945
- subtypes: `action_menu` x8182, `end_turn` x7616
- sources: bench 1414, bench-v1 1156, bench-v2 1203, bench-v3 1410, bench-v4 1376, bench-v5 1300, golden20 6247, te 1401, templates 291
- built in: not found as a literal (built from a variable or in host/bot code)

#### `pay`

- offered 40967 times in 9103 decisions; per decision 2..29 options in the whole choice
- ids: `{exhaust}`|`99 distinct (name)` x35407; `{trade_good}` x5227; `{exhaust}`|`63 distinct (name)`|`{influence,resources}` x333
  examples: `exhaust|newalbion` / "exhaust newalbion for 1 resources"; `exhaust|starpoint` / "exhaust starpoint for 3 resources"; `exhaust|acheron` / "exhaust acheron for 4 resources"
- payload: `kind`: str 40967, `owed`: int 40967, `source`: str 35740, `worth`: int 40967; key sets: {kind,owed,source,worth} x35740; {kind,owed,worth} x5227
- label: 5 template(s): "exhaust newalbion for 1 resources" -> `<id> <id> for # resources` x17937; "exhaust creuss for 2 influence" -> `<id> <id> for # influence` x17470; "spend a trade good" -> `spend a trade good` x5227
- prompt: 2 template(s): "pay 3 more resources" x4923; "pay 3 more influence" x4180
- order: contiguous x9103, insertion order x5883, ids ascending (string) x2540, ids descending x680
- subtypes: `pay_resources` x4923, `pay_influence` x4180
- sources: bench 814, bench-v1 684, bench-v2 682, bench-v3 762, bench-v4 790, bench-v5 723, golden20 3640, te 845, templates 163
- built in: `production.rs`

#### `produce`

- offered 30187 times in 4842 decisions; per decision 2..16 options in the whole choice
- ids: `{build,deploy}`|`78 distinct (name)`|`int 1..2` x29417; `{exchange}`|`8 distinct (name) e.g. cabal_flagship, cabal_mech, carrier` x770
  examples: `build|fighter|2` / "produce 2x fighter for 1"; `build|infantry|2` / "produce 2x infantry for 1"; `build|carrier|1` / "produce 1x carrier for 3"
- payload: `available_resources`: int 28946, `capacity_excess_after`: int 29393, `capacity_free_after`: int 29393, `capacity_needed`: int 29716, `capacity_provided`: int 29716, `capacity_role`: str 29716, `capacity_used`: int 29393, `cost`: int 30187, `count`: int 30187, `credit`: int 29716, `credit_used`: int 29716, `destination`: str 29393, `discount`: int 29716, `exchange`: bool 770, `fleet_excess_after`: int 29393, `fleet_headroom_after`: int 29393, `fleet_ships`: int 29716, `free_this_use`: bool 28946, `integrated_economy`: bool 38, `orbital_drop_deploy`: bool 2, `owed`: int 29716, `placed`: int 29716, `placement_pending`: int 323, `planet`: str 40, `printed_cost`: int 29716, `production_spent`: int 29716, `sling_relay`: bool 178, `space_placement`: str 29716, `system`: str 30187, `unit`: str 30187, `yield`: int 28946; key sets: {available_resources,capacity_excess_after,capacity_free_after,capacity_needed,capacity_provided,capacity_role,capacity_used,cost,count,credit,credit_used,destination,discount,fleet_excess_after,fleet_headroom_after,fleet_ships,free_this_use,owed,placed,printed_cost,production_spent,space_placement,system,unit,yield} x28623; {capacity_excess_after,capacity_free_after,capacity_needed,capacity_provided,capacity_role,capacity_used,cost,count,credit,credit_used,destination,discount,exchange,fleet_excess_after,fleet_headroom_after,fleet_ships,owed,placed,printed_cost,production_spent,space_placement,system,unit} x770; {available_resources,capacity_needed,capacity_provided,capacity_role,cost,count,credit,credit_used,discount,fleet_ships,free_this_use,owed,placed,placement_pending,printed_cost,production_spent,space_placement,system,unit,yield} x323; {cost,count,system,unit} x253
- label: 4 template(s): "produce 2x fighter for 1" -> `produce #x <id> for #` x29377; "produce 1x carrier by returning a captured carrier" -> `produce #x <id> by returning a captured <id>` x770; "Integrated Economy: produce 2x fighter for 1" -> `Integrated Economy: produce #x <id> for #` x38
- prompt: 13 template(s): "produce in 06 (7 left)" x4732; "Freelancers: produce one unit in 33" x48; "Sling Relay: produce one ship" x37
- order: contiguous x4823, insertion order x4784, ids ascending (string) x29, ids descending x10
- subtypes: `produce_unit` x4734, `(none)` x58, `produce_one_unit` x48, `orbital_drop_deploy_mech` x2
- sources: bench 435, bench-v1 360, bench-v2 340, bench-v3 339, bench-v4 413, bench-v5 393, fixtures 1, golden20 1728, te 364, templates 469
- built in: `production.rs`

#### `offer`

- offered 29723 times in 1847 decisions; per decision 2..101 options in the whole choice
- ids: `114 distinct (name)`:`24 distinct (name) e.g. arborec, argent, cabal`:`int 0..4` x13944; `29 distinct (name) e.g. nnacq, nnambuscade, nnan`:`24 distinct (name) e.g. arborec, argent, cabal`>`29 distinct (name) e.g. acq, ambuscade, an`:`24 distinct (name) e.g. arborec, argent, cabal` x7183; `30 distinct (int/name) e.g. 0, 1, 2`:`int 0..3` x7095; `32 distinct (name) e.g. cnabs, cnassassin, cnbunker`>`50 distinct (name)`:`14 distinct (int/name) e.g. 0, 2, 3` x768; +1 more shapes
  examples: `ss` / "exchange Support for the Throne notes"; `pncf:cabal:0` / "give cf:cabal"; `pncrucible:cabal:0` / "give crucible:cabal"
- payload: `action_card`: str 159, `alias`: str 10014, `gift`: bool 5225, `moves_to`: str 514, `net`: float 29723, `note`: str 7709, `planet`: str 514, `promissory`: str 9631, `received_planet`: str 111, `received_promissory`: str 11113, `their_net`: float 29723; key sets: {net,their_net} x7812; {net,promissory,received_promissory,their_net} x7183; {alias,gift,net,note,their_net} x5225; {net,received_promissory,their_net} x3930
- label: 228 template(s): "give 1 for 0" -> `give # for #` x6046; "give the note cf:cabal for the note ta:ghost" -> `give the note cf:<id> for the note <id>:<id>` x2898; "give the note ps:cabal for the note ta:ghost" -> `give the note ps:<id> for the note <id>:<id>` x2088
- prompt: 24 template(s): "transaction with sol" x218; "transaction with l1z1x" x185; "transaction with yin" x173
- order: contiguous x1845, insertion order x1836, ids ascending (string) x7, ids descending x2
- free_form prefixes: `x;`
- subtypes: `propose_transaction` x1847
- sources: bench 178, bench-v1 106, bench-v2 121, bench-v3 159, bench-v4 165, bench-v5 166, golden20 726, te 211, templates 15
- built in: `choice.rs`, `transactions.rs`

#### `vote_planet`

- offered 20943 times in 4430 decisions; per decision 1..18 options in the whole choice
- ids: `86 distinct (name)` x20943
  examples: `mr` / "exhaust mr for 6 votes"; `tarmann` / "exhaust tarmann for 1 votes"; `bereg` / "exhaust bereg for 1 votes"
- payload: `planet`: str 20943, `system`: str 20943; key sets: {planet,system} x20943
- label: 1 template(s): "exhaust mr for 6 votes" -> `exhaust <id> for # votes` x20943
- prompt: 22 template(s): "exhaust a planet to vote a" x2644; "exhaust a planet to vote for" x979; "exhaust a planet to vote against" x544
- order: contiguous x3883, insertion order x3035, ids ascending (string) x474, ids descending x374
- subtypes: `vote_exhaust_planet` x4430
- sources: bench 352, bench-v1 282, bench-v2 315, bench-v3 568, bench-v4 453, bench-v5 346, golden20 1701, te 413
- built in: `vote.rs`

#### `load`

- offered 20748 times in 8847 decisions; per decision 2..9 options in the whole choice
- ids: `{load}`|`int 0..35` x20748
  examples: `load|0` / "load fighter from space"; `load|4` / "load infantry from darien"; `load|5` / "load infantry from darien"
- payload: `capacity_remaining`: int 20748, `damaged`: bool 20748, `galvanized`: bool 20748, `loaded_fighters`: int 20748, `loaded_ground`: int 20748, `pickup_system`: str 20748, `source`: str 13259/null 7489, `system`: str 20748, `unit`: str 20748; key sets: {capacity_remaining,damaged,galvanized,loaded_fighters,loaded_ground,pickup_system,source,system,unit} x20748
- label: 649 template(s): "load fighter from space" -> `<id> fighter from space` x5343; "load infantry from space" -> `<id> infantry from space` x1067; "load sol_infantry from jord" -> `<id> sol_infantry from jord` x648
- prompt: 6 template(s): "load carrier (4 free)" x5498; "load l1z1x_dreadnought (2 free)" x2611; "load dreadnought (1 free)" x566
- order: contiguous x6950, ids ascending (string) x6355, insertion order x587, ids descending x8
- subtypes: `load_cargo` x8847
- sources: bench 734, bench-v1 562, bench-v2 555, bench-v3 665, bench-v4 628, bench-v5 666, fixtures 1, golden20 2947, te 735, templates 1354
- built in: `transit.rs`

#### `pool`

- offered 20157 times in 6719 decisions; per decision 3..3 options in the whole choice
- ids: `{fleet,fleet_tokens,strategic,strategic_tokens,tactic,tactic_tokens}` x20157
  examples: `tactic_tokens` / "tactic pool"; `fleet_tokens` / "fleet pool"; `strategic_tokens` / "strategy pool"
- payload: none
- label: 4 template(s): "tactic pool" -> `tactic pool` x6706; "fleet pool" -> `fleet pool` x6706; "strategy pool" -> `strategy pool` x6706
- prompt: 2 template(s): "gain a command token into which pool" x6706; "Fabrication: which pool gains the token" x13
- order: insertion order x6719, contiguous x6719
- subtypes: `gain_command_token` x6706, `token_pool` x13
- sources: bench 603, bench-v1 511, bench-v2 521, bench-v3 630, bench-v4 609, bench-v5 571, golden20 2595, te 593, templates 86
- built in: `factions/naaz.rs`, `strategy_cards.rs`, `tokens.rs`

#### `action`

- offered 17909 times in 9447 decisions; per decision 1..20 options in the whole choice
- ids: `{pass,strategic,tactical}` x14593; `{strategic}`|`8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x3316
  examples: `strategic|pok1leadership` / "take the strategic action of 1. Leadership"; `strategic|pok7technology` / "take the strategic action of 7. Technology"; `tactical` / "take a tactical action"
- payload: none
- label: 11 template(s): "take a tactical action" -> `take a <id> action` x6809; "take your strategic action" -> `take your <id> action` x4793; "pass" -> `<id>` x2991
- prompt: 2 template(s): "action phase" x9433; "end your turn, or take your second action" x14
- order: contiguous x7104, ids ascending (string) x6794, insertion order x252, ids descending x58
- subtypes: `action_menu` x9433, `end_turn` x14
- sources: bench 786, bench-v1 673, bench-v2 706, bench-v3 806, bench-v4 799, bench-v5 732, golden20 3516, te 786, templates 643
- built in: `secondary_preview_tests.rs`, `strategy.rs`

#### `strategy`

- offered 16039 times in 7859 decisions; per decision 2..7 options in the whole choice
- ids: `{no,yes}` x15689; `{follow}`|`{waived}`|`int 0..0`|`{agent}`|`47 distinct (int/name)` x249; `{follow}`|`{waived}`|`int 1..1`|`{ms}`|`{an,cf,gift,pop,ps,ta}`:`{arborec,mentak,naalu,saar}` x77; `{follow}`|`{waived}`|`int 3..3`|`{acq}` x24
  examples: `no` / "decline"; `yes` / "place"; `follow|waived|0|agent|36` / "exhaust Jae Mir Kan: use d's command token from Arnor and Lor (system 36) instead of a token"
- payload: none
- label: 120 template(s): "decline" -> `decline` x4725; "spend nothing further" -> `spend nothing further` x3134; "spend 3 influence" -> `spend # influence` x2673
- prompt: 9 template(s): "spend 3 influence for a command token" x2673; "spend a strategy token to produce at home" x1066; "spend a strategy token to place a structure" x793
- order: contiguous x7859, ids ascending (string) x7687, insertion order x150, ids descending x22
- subtypes: `(none)` x4725, `buy_token_with_influence` x3134
- sources: bench 688, bench-v1 588, bench-v2 653, bench-v3 686, bench-v4 659, bench-v5 639, golden20 2914, te 643, templates 389
- built in: `strategy.rs`

#### `move`

- offered 14863 times in 7682 decisions; per decision 2..20 options in the whole choice
- ids: `{move,move_gd}`|`80 distinct (int/name)`|`int 0..18` x14854; `{move}`|`{alioprima,bakal,meer,vegamajor,vegaminor,velnor}`|`{alioprima,arinam,bakal,lisis,vegamajor,vegaminor}`|`{infantry,l1z1x_mech,saar_mech}` x9
  examples: `move|03|0` / "move carrier from 03"; `move|03|2` / "move destroyer from 03"; `move|03|1` / "move destroyer from 03"
- payload: `capacity`: int 14854, `count`: int 9, `damaged`: bool 14854, `from`: str 9, `gravity_drive`: bool 14854, `origin`: str 14854, `to`: str 9, `unit`: str 14863; key sets: {capacity,damaged,gravity_drive,origin,unit} x14854; {count,from,to,unit} x9
- label: 104 template(s): "move carrier from 03" -> `<id> carrier from #` x4276; "move destroyer from 03" -> `<id> destroyer from #` x2171; "move cruiser from 54" -> `<id> cruiser from #` x2158
- prompt: 2 template(s): "movement" x7677; "Ghost Squad: move ground forces in 74" x5
- order: contiguous x4695, ids ascending (string) x4207, insertion order x344, ids descending x144
- subtypes: `movement_step` x7677, `ghost_squad_move` x5
- sources: bench 606, bench-v1 507, bench-v2 493, bench-v3 588, bench-v4 569, bench-v5 572, fixtures 3, golden20 2667, te 610, templates 1067
- built in: `action_cards.rs`, `tactical.rs`

#### `strategy_card`

- offered 14564 times in 2726 decisions; per decision 1..8 options in the whole choice
- ids: `8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x14519; `{a,b,c,d,e,f}`|`8 distinct (name) e.g. pok1leadership, pok2diplomacy, pok3politics` x45
  examples: `pok1leadership` / "1. Leadership"; `pok2diplomacy` / "2. Diplomacy"; `pok3politics` / "3. Politics"
- payload: none
- label: 16 template(s): "3. Politics" -> `#. Politics` x2308; "2. Diplomacy" -> `#. Diplomacy` x2242; "5. Trade" -> `#. Trade` x1993
- prompt: 7 template(s): "choose a strategy card" x2611; "Manipulate Investments: place a trade good on which strategy card" x70; "choose a different strategy card" x18
- order: contiguous x2721, insertion order x1488, ids ascending (string) x1228, ids descending x5
- subtypes: `draft_strategy_card` x2611, `investments_pick_strategy_card` x70, `public_disgrace_choose_card` x18, `(none)` x16, `hero_card` x11
- sources: bench 192, bench-v1 175, bench-v2 178, bench-v3 201, bench-v4 186, bench-v5 181, golden20 854, te 149, templates 610
- built in: `draft.rs`, `factions/naaz.rs`, `factions/winnu.rs`

#### `commit`

- offered 12757 times in 5581 decisions; per decision 2..27 options in the whole choice
- ids: `{commit}`|`int 0..10`|`82 distinct (name)` x12757
  examples: `commit|0|kraag` / "land infantry on kraag"; `commit|0|siig` / "land infantry on siig"; `commit|0|bereg` / "land infantry on bereg"
- payload: `damaged`: bool 12757, `from_planet`: str 1466, `from_system`: str 1466, `planet`: str 12757, `unit`: str 12757; key sets: {damaged,planet,unit} x11291; {damaged,from_planet,from_system,planet,unit} x1466
- label: 53 template(s): "land infantry on kraag" -> `land infantry on <id>` x8468; "land sol_infantry on kraag" -> `land sol_infantry on <id>` x824; "land mahact_infantry on vegamajor" -> `land mahact_infantry on <id>` x404
- prompt: 2 template(s): "commit ground forces in 70" x5572; "commit ground forces in 82a" x9
- order: contiguous x4370, ids ascending (string) x3681, ids descending x430, insertion order x259
- subtypes: `commit_ground_forces` x5581
- sources: bench 434, bench-v1 319, bench-v2 323, bench-v3 379, bench-v4 371, bench-v5 381, fixtures 1, golden20 2066, te 515, templates 792
- built in: `invasion.rs`

#### `build`

- offered 11753 times in 1231 decisions; per decision 1..39 options in the whole choice
- ids: `{pds,spacedock}`|`63 distinct (int/name)`|`99 distinct (name)` x11005; `{produce,titans}`|`57 distinct (int/name)` x437; `{pds,structure}` x311
  examples: `structure` / "place a structure"; `produce|06` / "use PRODUCTION in 06"; `pds|06|0.0.0` / "place pds on 0.0.0"
- payload: `planet`: str 11005, `system`: str 11005, `unit`: str 11005; key sets: {planet,system,unit} x11005; {-} x748
- label: 20 template(s): "place pds on 0.0.0" -> `place <id> on <id>` x11005; "use PRODUCTION in 06" -> `use PRODUCTION in #` x426; "place a structure" -> `place a <id>` x302
- prompt: 3 template(s): "place a structure" x920; "Construction: a structure or a production" x302; "place a PDS or an alternative" x9
- order: contiguous x1210, insertion order x861, ids descending x209, ids ascending (string) x140
- subtypes: `place_structure` x920, `construction_choose_ability` x302, `place_structure_pds_alternative` x9
- sources: bench 113, bench-v1 100, bench-v2 107, bench-v3 109, bench-v4 102, bench-v5 105, golden20 466, te 91, templates 38
- built in: `factions/titans.rs`, `strategy_cards.rs`

#### `component`

- offered 11470 times in 5902 decisions; per decision 2..20 options in the whole choice
- ids: `{action_card,faction,purge,relic}`|`20 distinct (int/name) e.g. 0, 1, 2` x7041; `{component,faction}`|`9 distinct (name) e.g. cabal, expedition, ghost`|`42 distinct (name)` x3999; `{component,faction}`|`{leader,mahact,titans,yssaril}`|`{bt_stall,mi,terraform,vaults,yssarilagent}`|`43 distinct (name)` x426; `{c,d,e}` x4
  examples: `action_card|2` / "play Impersonation"; `action_card|1` / "play Impersonation"; `action_card|0` / "play Plague"
- payload: `grants_breakthrough`: bool 256, `opportunity_cost`: float 256, `slice`: str 256; key sets: {-} x11214; {grants_breakthrough,opportunity_cost,slice} x256
- label: 98 template(s): "Orbital Drop: spend a strategy token to land 2 infantry" -> `Orbital Drop: spend a strategy token to land # infantry` x638; "Ggrocuto Rinn" -> `Ggrocuto Rinn` x519; "use Sling Relay" -> `use Sling Relay` x407
- prompt: 4 template(s): "action phase" x5620; "end your turn" x268; "end your turn, or take your second action" x12
- order: contiguous x3057, ids descending x1483, insertion order x971, ids ascending (string) x616, interleaved with other kinds x13
- subtypes: `action_menu` x5620, `end_turn` x280, `expedition_placement_tiebreak` x2
- sources: bench 516, bench-v1 434, bench-v2 471, bench-v3 536, bench-v4 490, bench-v5 459, golden20 2086, te 531, templates 379
- built in: `action_cards.rs`, `faction_abilities.rs`, `factions/borrowed_commanders.rs`, `factions/nekro_units.rs`, `leaders.rs`, `relics.rs` (+3)

#### `research`

- offered 9446 times in 645 decisions; per decision 2..30 options in the whole choice
- ids: `76 distinct (name)` x9446
  examples: `aida` / "AI Development Algorithm"; `amd` / "Antimass Deflectors"; `det` / "Dark Energy Tap"
- payload: `cost`: int 9438, `cost_tokens`: int 9438; key sets: {cost,cost_tokens} x9438; {-} x8
- label: 76 template(s): "Dark Energy Tap" -> `Dark Energy Tap` x626; "AI Development Algorithm" -> `AI Development Algorithm` x618; "Psychoarchaeology" -> `Psychoarchaeology` x614
- prompt: 2 template(s): "research a technology" x637; "Deepwrought commander: reduce this research by 1 (pays c)" x8
- order: ids ascending (string) x637, contiguous x637
- subtypes: `research_technology` x637, `deepwrought_reduce_research` x8
- sources: bench 47, bench-v1 41, bench-v2 61, bench-v3 60, bench-v4 54, bench-v5 42, golden20 281, te 56, templates 3
- built in: `strategy_cards.rs`

#### `ability`

- offered 9153 times in 8860 decisions; per decision 2..5 options in the whole choice
- ids: `7 distinct (name) e.g. ability, breakthrough, leader`:`24 distinct (name) e.g. arborec, argent, cabal`:`74 distinct (name)`:`39 distinct (name) e.g. ACTION_CARD_PLAYED, ACTION_COMPLETED, AGENDA_REVEALED`:`{after,produce,when}` x5189; `{reaction}`:`24 distinct (name) e.g. arborec, argent, cabal`:`32 distinct (name) e.g. ACTION_CARD_DISCARDED, ACTION_CARD_PLAYED, ACTION_COMPLETED`:`{after,when}` x3501; `{leader}`:`8 distinct (name) e.g. argent, ghost, mentak`:`7 distinct (name) e.g. a, b, c`:`{l1z1xagent,letnevagent,solagent,yssarilagent}`:`12 distinct (name) e.g. argentagent, b, c`:`10 distinct (name) e.g. COMBAT_ROUND_STARTED, GROUND_COMBAT_ROUND_STARTED, GROUND_FORCES_BEING_PRODUCED`:`{after,when}` x225; `19 distinct (name) e.g. ang, archonvail, arinam` x210; +1 more shapes
  examples: `leader:yin:yinagent:SHIP_DESTROYED:after` / "Agent Brother Milor (after ship destroyed)"; `promissory:cabal:iff:TURN_BEGAN:after` / "promissory:cabal:iff:TURN_BEGAN:after"; `leader:cabal:cabalagent:COMMODITIES_REPLENISHED:after` / "Agent The Stillness of Stars (after commodities replenished)"
- payload: `card`: str 3225, `card_name`: str 3225, `card_names`: array 3501, `event`: str 8943, `planet`: str 191, `system`: str 191, `technology`: str 191; key sets: {event} x5442; {card,card_name,card_names,event} x3225; {card_names,event} x276; {planet,system,technology} x191
- label: 147 template(s): "Agent Captain Mendosa (after system activated)" -> `Agent Captain Mendosa (<id> system activated)` x802; "Aetherpassage (after system activated)" -> `Aetherpassage (<id> system activated)` x585; "Agent Brother Milor (after ship destroyed)" -> `Agent Brother Milor (<id> ship destroyed)` x474
- prompt: 4 template(s): "after SHIP_DESTROYED" x7538; "when AGENDA_REVEALED" x1184; "Psychoarchaeology: exhaust a specialty for 1 trade good" x126
- order: contiguous x275, ids ascending (string) x150, ids descending x110, insertion order x15
- subtypes: `reaction_after_SYSTEM_ACTIVATED` x2471, `reaction_after_AGENDA_REVEALED` x761, `reaction_after_TACTICAL_ACTION_ENDED` x579, `reaction_after_SHIP_DESTROYED` x514, `reaction_when_AGENDA_REVEALED` x486
- sources: bench 691, bench-v1 586, bench-v2 524, bench-v3 747, bench-v4 637, bench-v5 673, golden20 3446, te 900, templates 656
- built in: `combat.rs`, `faction_abilities.rs`, `production.rs`, `technology.rs`, `timing.rs`

#### `vote`

- offered 8253 times in 1518 decisions; per decision 1..60 options in the whole choice
- ids: `100 distinct (name)` x8253
  examples: `for` / "for"; `against` / "against"; `alioprima` / "alioprima"
- payload: `current_votes`: int 8253, `planet`: str 3048, `system`: str 3048; key sets: {current_votes} x5205; {current_votes,planet,system} x3048
- label: 7 template(s): "for" -> `<id>` x4418; "a" -> `a` x737; "b" -> `b` x733
- prompt: 1 template(s): "vote for which outcome" x1518
- order: contiguous x1477, ids ascending (string) x811, ids descending x666
- subtypes: `cast_vote` x1518
- sources: bench 117, bench-v1 110, bench-v2 89, bench-v3 153, bench-v4 133, bench-v5 145, golden20 629, te 142
- built in: `vote.rs`

#### `end_turn`

- offered 8184 times in 8184 decisions; per decision 1..11 options in the whole choice
- ids: `{end_turn}` x8184
  examples: `end_turn` / "end your turn"
- payload: none
- label: 1 template(s): "end your turn" -> `end your turn` x8184
- prompt: 2 template(s): "end your turn" x8167; "end your turn, or take your second action" x17
- order: never more than one of this kind in a decision
- subtypes: `end_turn` x8184
- sources: bench 725, bench-v1 565, bench-v2 585, bench-v3 700, bench-v4 685, bench-v5 663, golden20 3137, te 734, templates 390
- built in: not found as a literal (built from a variable or in host/bot code)

#### `ready`

- offered 5139 times in 1181 decisions; per decision 1..16 options in the whole choice
- ids: `100 distinct (name)` x4950; `{ready}`|`{planet}`|`18 distinct (name) e.g. ang, archonvail, arnor` x189
  examples: `bereg` / "ready bereg"; `lirtaiv` / "ready lirtaiv"; `creuss` / "ready creuss"
- payload: `planet`: str 5139, `system`: str 5139, `technology`: str 189; key sets: {planet,system} x4950; {planet,system,technology} x189
- label: 2 template(s): "ready bereg" -> `ready <id>` x4950; "Bio-Stims: ready vegaminor" -> `Bio-Stims: <id> <id>` x189
- prompt: 2 template(s): "ready which planet" x1031; "Bio-Stims" x150
- order: contiguous x963, insertion order x751, ids ascending (string) x151, ids descending x61
- subtypes: `ready_planet` x1031, `bio_stims_ready` x150
- sources: bench 102, bench-v1 114, bench-v2 117, bench-v3 109, bench-v4 108, bench-v5 105, golden20 413, te 83, templates 30
- built in: `strategy_cards.rs`, `technology.rs`

#### `planet`

- offered 3452 times in 296 decisions; per decision 1..48 options in the whole choice
- ids: `82 distinct (name)` x1828; `58 distinct (int/name)`|`88 distinct (name)` x1623; `{meer}`:`{WARFARE}` x1
  examples: `tarmann` / "tarmann"; `bereg` / "bereg"; `alioprima` / "alioprima"
- payload: `planet`: str 3070, `system`: str 3070; key sets: {planet,system} x3070; {-} x382
- label: 24 template(s): "tarmann" -> `<id>` x2966; "explore qucenn" -> `explore <id>` x354; "vefutii in 20" -> `<id> in #` x38
- prompt: 18 template(s): "Garv and Gunn: explore which of your planets" x89; "Leader: ready which planet" x45; "Archaeological Expedition: which planet's deck" x22
- order: contiguous x281, ids ascending (string) x185, insertion order x91, ids descending x5
- subtypes: `agent_planet` x77, `leader_xxchaagent_ready_planet` x45, `arch_expedition_pick_planet` x22, `plague_pick_planet` x21, `cripple_pick_planet` x21
- sources: bench 25, bench-v1 19, bench-v2 17, bench-v3 23, bench-v4 23, bench-v5 18, golden20 130, te 21, templates 20
- built in: `action_cards.rs`, `exploration.rs`, `faction_abilities.rs`, `factions/bastion_units.rs`, `factions/borrowed_commanders.rs`, `factions/borrowed_round_agents.rs` (+18)

#### `legendary`

- offered 3110 times in 1278 decisions; per decision 2..19 options in the whole choice
- ids: `60 distinct (int/name)`|`92 distinct (name)` x1771; `8 distinct (name) e.g. card, convert, goods` x1339
  examples: `hopesend` / "Imperial Arms Vault"; `mech` / "place 1 mech"; `card` / "draw 1 action card"
- payload: `planet`: str 1771, `system`: str 1771, `unit`: str 1771; key sets: {planet,system,unit} x1771; {-} x1339
- label: 9 template(s): "place on jord" -> `place on <id>` x1771; "Imperial Arms Vault" -> `Imperial Arms Vault` x459; "The Atrament" -> `The Atrament` x307
- prompt: 5 template(s): "use a legendary planet ability" x834; "Imperial Arms Vault" x218; "The Atrament: place infantry where" x145
- order: contiguous x459, ids descending x245, ids ascending (string) x214
- subtypes: `legendary_end_of_turn` x834, `legendary_arms_vault` x218, `legendary_place` x199, `legendary_exterrix` x27
- sources: bench 102, bench-v1 67, bench-v2 87, bench-v3 128, bench-v4 113, bench-v5 74, golden20 604, te 103
- built in: `legendary.rs`

#### `transaction`

- offered 2842 times in 1421 decisions; per decision 3..3 options in the whole choice
- ids: `{accept,counter}` x2842
  examples: `accept` / "accept"; `counter` / "counter-offer"
- payload: `net`: float 1421; key sets: {net} x1421; {-} x1421
- label: 2 template(s): "accept" -> `<id>` x1421; "counter-offer" -> `<id>-offer` x1421
- prompt: 880 template(s): "sol gives 4 commodities for 4 commodities -- accept?" x23; "xxcha gives 2 commodities for 2 commodities -- accept?" x16; "mahact gives 2 commodities for 2 commodities -- accept?" x15
- order: ids ascending (string) x1421, interleaved with other kinds x1421
- subtypes: `answer_transaction` x1421
- sources: bench 128, bench-v1 89, bench-v2 100, bench-v3 125, bench-v4 125, bench-v5 121, golden20 566, te 153, templates 14
- built in: `transactions.rs`

#### `retreat`

- offered 2661 times in 1333 decisions; per decision 1..2 options in the whole choice
- ids: `{retreat,stay}` x2661
  examples: `stay` / "stay and fight"; `retreat` / "announce a retreat"
- payload: `system`: str 2661; key sets: {system} x2661
- label: 3 template(s): "stay and fight" -> `<id> and fight` x1328; "announce a retreat" -> `announce a <id>` x1328; "retreat (forced)" -> `<id> (forced)` x5
- prompt: 2 template(s): "announce a retreat from 72" x1331; "announce a retreat from 82a" x2
- order: ids descending x1328, contiguous x1328
- subtypes: `announce_retreat` x1333
- sources: bench 99, bench-v1 102, bench-v2 98, bench-v3 158, bench-v4 97, bench-v5 120, golden20 529, te 112, templates 18
- built in: `factions/borrowed_commanders.rs`

#### `system`

- offered 2292 times in 366 decisions; per decision 1..30 options in the whole choice
- ids: `84 distinct (int/name)` x2265; `{system}`|`int 9..79` x27
  examples: `35` / "35"; `51` / "51"; `71` / "71"
- payload: none
- label: 12 template(s): "35" -> `#` x1786; "jam 32" -> `jam #` x141; "place Thunder's Edge in system 20" -> `place Thunder's Edge in system #` x120
- prompt: 18 template(s): "choose a system for Diplomacy" x247; "Leader: which system to gather your ships in" x21; "Signal Jamming: jam which system" x13
- order: contiguous x352, ids ascending (string) x308, insertion order x43, ids descending x1
- subtypes: `diplomacy_choose_system` x247, `leader_l1z1xhero_destination` x21, `jamming_pick_system` x13, `swap_first` x11, `(none)` x11
- sources: bench 30, bench-v1 29, bench-v2 27, bench-v3 33, bench-v4 36, bench-v5 31, golden20 142, te 32, templates 6
- built in: `action_cards.rs`, `combat.rs`, `faction_abilities.rs`, `factions/arborec.rs`, `factions/argent.rs`, `factions/bastion.rs` (+37)

#### `creuss_token`

- offered 1867 times in 91 decisions; per decision 1..54 options in the whole choice
- ids: `{ALPHA,BETA}`|`int 1..80` x1867
  examples: `ALPHA|74` / "Creuss IFF: the ALPHA token into 74"; `ALPHA|72` / "Creuss IFF: the ALPHA token into 72"; `ALPHA|34` / "Creuss IFF: the ALPHA token into 34"
- payload: `system`: str 1867; key sets: {system} x1867
- label: 3 template(s): "Creuss IFF: the ALPHA token into 74" -> `Creuss IFF: the <id> token into #` x1676; "Wormhole Generator: the ALPHA token into 35" -> `Wormhole Generator: the <id> token into #` x106; "Icarus Drive: the ALPHA token into 28" -> `Icarus Drive: the <id> token into #` x85
- prompt: 3 template(s): "Icarus Drive: remove the mech to place or move a Creuss wormhole token into its system" x47; "Creuss IFF: place or move a Creuss wormhole token" x42; "Wormhole Generator: place or move a Creuss wormhole token" x2
- order: contiguous x80, insertion order x45, ids ascending (string) x35
- subtypes: `icarus_drive` x47, `creuss_token` x44
- sources: bench 12, bench-v1 4, bench-v2 6, bench-v3 15, bench-v4 15, bench-v5 7, golden20 28, te 4
- built in: `factions/ghost.rs`

#### `discard`

- offered 1862 times in 243 decisions; per decision 2..12 options in the whole choice
- ids: `int 0..11` x1862
  examples: `1` / "Ancient Burial Sites"; `4` / "Cripple Defenses"; `7` / "Hack Election"
- payload: none
- label: 94 template(s): "Direct Hit" -> `Direct Hit` x90; "Diplomatic Pressure" -> `Diplomatic Pressure` x76; "Maneuvering Jets" -> `Maneuvering Jets` x75
- prompt: 2 template(s): "over the hand limit — discard one of 8" x237; "discard an action card for the expedition" x6
- order: contiguous x243, insertion order x236, ids ascending (string) x7
- subtypes: `discard_over_hand_limit` x237, `expedition_discard_action_card` x6
- sources: bench 40, bench-v1 7, bench-v2 19, bench-v3 12, bench-v4 29, bench-v5 29, golden20 95, te 10, templates 2
- built in: `action_cards.rs`, `factions/nekro_units.rs`, `factions/yssaril.rs`, `laws.rs`, `thunders_edge.rs`

#### `explore`

- offered 1604 times in 902 decisions; per decision 1..3 options in the whole choice
- ids: `39 distinct (name) e.g. aw1, aw2, aw3` x1604
  examples: `remove_infantry` / "remove 1 infantry and gain the reward"; `decline` / "do not remove an infantry"; `place` / "place 1 infantry"
- payload: none
- label: 12 template(s): "gain 1 commodity" -> `<id> # commodity` x395; "remove 1 infantry and gain the reward" -> `remove # infantry and gain the reward` x309; "do not remove an infantry" -> `do not remove an infantry` x309
- prompt: 6 template(s): "remove 1 infantry from this planet to resolve the exploration card" x309; "Local Fabricators" x232; "Functioning Base" x163
- order: contiguous x684, ids descending x500, ids ascending (string) x166, insertion order x18
- subtypes: `local_fabricators_choose_reward` x232, `functioning_base_choose_reward` x163, `vfs1_choose_reward` x102, `mercenary_outfit_choose_reward` x96, `abandoned_warehouses_choose_reward` x83
- sources: bench 48, bench-v1 52, bench-v2 53, bench-v3 52, bench-v4 54, bench-v5 54, golden20 279, te 75, templates 235
- built in: `exploration.rs`

#### `return`

- offered 1517 times in 489 decisions; per decision 2..4 options in the whole choice
- ids: `40 distinct (name)` x1517
  examples: `mp` / "return mp"; `gamf` / "return gamf"; `eh` / "return eh"
- payload: none
- label: 1 template(s): "return mp" -> `return <id>` x1517
- prompt: 1 template(s): "return a secret objective to the deck" x489
- order: contiguous x489, insertion order x291, ids ascending (string) x107, ids descending x91
- subtypes: `return_over_secret_hand_limit` x489
- sources: bench 50, bench-v1 39, bench-v2 47, bench-v3 46, bench-v4 39, bench-v5 44, golden20 186, te 38
- built in: `secrets.rs`, `thunders_edge.rs`

#### `score`

- offered 1481 times in 1039 decisions; per decision 2..7 options in the whole choice
- ids: `66 distinct (name)` x1481
  examples: `trade_routes` / "trade_routes"; `corner` / "corner"; `build_defenses` / "build_defenses"
- payload: none
- label: 1 template(s): "trade_routes" -> `<id>` x1481
- prompt: 1 template(s): "score an objective" x1039
- order: contiguous x325, ids ascending (string) x144, ids descending x115, insertion order x66
- subtypes: `score_objective` x933, `score_secret_objective` x106
- sources: bench 89, bench-v1 86, bench-v2 84, bench-v3 98, bench-v4 87, bench-v5 79, golden20 412, te 104
- built in: `objectives.rs`

#### `remove`

- offered 1370 times in 940 decisions; per decision 1..4 options in the whole choice
- ids: `{remove}`|`int 0..7` x1370
  examples: `remove|0` / "remove fighter"; `remove|1` / "remove l1z1x_mech"; `remove|3` / "remove ghost_flagship"
- payload: none
- label: 62 template(s): "remove fighter" -> `<id> fighter` x318; "remove infantry" -> `<id> infantry` x269; "remove carrier" -> `<id> carrier` x122
- prompt: 2 template(s): "remove a unit: over capacity in 03" x750; "remove a unit: over fleet supply in 51" x190
- order: ids ascending (string) x318, contiguous x318
- subtypes: `(none)` x940
- sources: bench 109, bench-v1 75, bench-v2 67, bench-v3 86, bench-v4 98, bench-v5 100, golden20 338, te 55, templates 12
- built in: `fleet.rs`, `leaders.rs`

#### `commit_infantry`

- offered 1359 times in 33 decisions; per decision 24..52 options in the whole choice
- ids: `int 18..76`|`60 distinct (name)` x1359
  examples: `75|abaddon` / "commit infantry to abaddon in 75"; `75|ashtroth` / "commit infantry to ashtroth in 75"; `75|loki` / "commit infantry to loki in 75"
- payload: none
- label: 1 template(s): "commit infantry to abaddon in 75" -> `commit infantry to <id> in #` x1359
- prompt: 1 template(s): "Quantum Dissemination: commit infantry 1 of 3" x33
- order: insertion order x33, contiguous x33
- subtypes: `yin_hero_commit` x33
- sources: bench 3, bench-v1 6, bench-v2 6, bench-v3 7, bench-v4 3, golden20 8
- built in: `factions/yin.rs`

#### `transit`

- offered 1161 times in 16 decisions; per decision 37..133 options in the whole choice
- ids: `{transit}`|`int 12..75`|`17 distinct (name) e.g. accoen, ashtroth, cealdri`|`{infantry,jolnar_mech}`|`int 12..75`|`17 distinct (name) e.g. accoen, ashtroth, cealdri` x1161
  examples: `transit|12|nar|infantry|12|jol` / "move infantry from nar to jol"; `transit|12|nar|infantry|24|meharxull` / "move infantry from nar to meharxull"; `transit|12|nar|infantry|29|qucenn` / "move infantry from nar to qucenn"
- payload: `destination_system`: str 1161, `planet`: str 1161, `source`: str 1161, `source_system`: str 1161, `unit`: str 1161; key sets: {destination_system,planet,source,source_system,unit} x1161
- label: 1 template(s): "move infantry from nar to jol" -> `move <id> from <id> to <id>` x1161
- prompt: 1 template(s): "Transit Diodes: redeploy ground forces (4 left)" x16
- order: contiguous x16, ids ascending (string) x14, insertion order x2
- subtypes: `transit_diodes_redeploy` x16
- sources: golden20 16
- built in: not found as a literal (built from a variable or in host/bot code)

#### `replenish`

- offered 1088 times in 311 decisions; per decision 2..8 options in the whole choice
- ids: `24 distinct (name) e.g. arborec, argent, cabal` x1088
  examples: `l1z1x` / "l1z1x replenishes commodities"; `yin` / "yin replenishes commodities"; `ghost` / "ghost replenishes commodities"
- payload: none
- label: 1 template(s): "l1z1x replenishes commodities" -> `<id> replenishes commodities` x1088
- prompt: 1 template(s): "let another player replenish commodities" x311
- order: contiguous x292, insertion order x188, ids ascending (string) x67, ids descending x37
- subtypes: `trade_choose_replenish` x311
- sources: bench 24, bench-v1 51, bench-v2 22, bench-v3 22, bench-v4 23, bench-v5 22, golden20 100, te 18, templates 29
- built in: `strategy_cards.rs`

#### `ship`

- offered 994 times in 121 decisions; per decision 2..33 options in the whole choice
- ids: `41 distinct (int/name)`|`37 distinct (int/name) e.g. 0, 1, 10` x977; `{stop}` x17
  examples: `ship|0` / "fighter in 11 (move 0 to 2)"; `ship|1` / "saar_flagship in 11 (move 1 to 2)"; `27|0` / "ghost_flagship in 27"
- payload: `system`: str 207; key sets: {-} x787; {system} x207
- label: 91 template(s): "replace it with a carrier from reinforcements" -> `<id> it with a <id> from reinforcements` x89; "carrier in 51" -> `carrier in #` x70; "dreadnought in 29" -> `dreadnought in #` x53
- prompt: 14 template(s): "Letani Ospha: choose a non-fighter ship" x34; "Captain Mendosa: raise which of the activating player's ships" x18; "Scuttle: which ship to scuttle" x17
- order: contiguous x116, ids ascending (string) x88, insertion order x28
- subtypes: `scuttle_pick_ship` x34, `ospha_ship` x34, `ospha_replace` x34, `agent_ship` x18, `lucky_pick_ship` x1
- sources: bench 6, bench-v1 5, bench-v2 6, bench-v3 5, bench-v4 7, bench-v5 7, golden20 23, te 15, templates 47
- built in: `action_cards.rs`, `factions/arborec.rs`, `factions/crimson_cards.rs`, `factions/obsidian.rs`, `factions/ralnel.rs`, `factions/saar.rs` (+2)

#### `ground_casualty`

- offered 953 times in 953 decisions; per decision 1..1 options in the whole choice
- ids: `{fight}` x953
  examples: `fight` / "fight a round on lirtaiv"
- payload: none
- label: 68 template(s): "fight a round on mr" -> `<id> a round on mr` x61; "fight a round on semlore" -> `<id> a round on semlore` x56; "fight a round on lirtaiv" -> `<id> a round on lirtaiv` x49
- prompt: 68 template(s): "fight a round on mr" x61; "fight a round on semlore" x56; "fight a round on lirtaiv" x49
- order: never more than one of this kind in a decision
- subtypes: `fight_ground_combat_round` x953
- sources: bench 105, bench-v1 74, bench-v2 70, bench-v3 96, bench-v4 90, bench-v5 73, golden20 358, te 83, templates 4
- built in: `invasion.rs`

#### `agenda`

- offered 856 times in 428 decisions; per decision 2..2 options in the whole choice
- ids: `{bottom,top}` x856
  examples: `top` / "on top of the deck"; `bottom` / "on the bottom"
- payload: none
- label: 2 template(s): "on top of the deck" -> `on <id> of the deck` x428; "on the bottom" -> `on the <id>` x428
- prompt: 32 template(s): "place wormhole_recon where" x161; "place checks where" x20; "place incentive where" x18
- order: ids descending x428, contiguous x428
- subtypes: `politics_place_agenda` x428
- sources: bench 38, bench-v1 32, bench-v2 38, bench-v3 36, bench-v4 38, bench-v5 34, golden20 182, te 20, templates 10
- built in: `reaction_preview.rs`, `strategy_cards.rs`, `trigger.rs`

#### `speaker`

- offered 793 times in 214 decisions; per decision 2..5 options in the whole choice
- ids: `24 distinct (name) e.g. arborec, argent, cabal` x793
  examples: `ghost` / "ghost becomes speaker"; `l1z1x` / "l1z1x becomes speaker"; `yin` / "yin becomes speaker"
- payload: none
- label: 1 template(s): "ghost becomes speaker" -> `<id> becomes speaker` x793
- prompt: 1 template(s): "who becomes speaker" x214
- order: contiguous x214, insertion order x147, ids ascending (string) x36, ids descending x31
- subtypes: `politics_choose_speaker` x214
- sources: bench 19, bench-v1 16, bench-v2 19, bench-v3 18, bench-v4 19, bench-v5 17, golden20 91, te 10, templates 5
- built in: `strategy_cards.rs`

#### `player`

- offered 702 times in 191 decisions; per decision 2..7 options in the whole choice
- ids: `21 distinct (name) e.g. a, arborec, b` x702
  examples: `ghost` / "ghost's command token"; `cabal` / "cabal's command token"; `yin` / "yin's command token"
- payload: none
- label: 57 template(s): "ghost's command token" -> `<id>'s command token` x51; "a" -> `a` x33; "c" -> `c` x33
- prompt: 13 template(s): "Umbat: choose a player who may produce up to 2 units costing 4 or less" x45; "Nekro Malleon: choose a player" x28; "Diplomatic Pressure: choose another player" x26
- order: contiguous x161, ids ascending (string) x148, insertion order x10, ids descending x3
- subtypes: `umbat_player` x45, `malleon_player` x28, `spy_pick_player` x17, `fsb_pick_player` x15, `abs_pick_player` x14
- sources: bench 7, bench-v1 12, bench-v2 16, bench-v3 23, bench-v4 22, bench-v5 9, golden20 49, te 18, templates 35
- built in: `action_cards.rs`, `combat.rs`, `exploration.rs`, `faction_abilities.rs`, `factions/argent.rs`, `factions/bastion_units.rs` (+40)

#### `casualty`

- offered 687 times in 357 decisions; per decision 1..4 options in the whole choice
- ids: `{destroy}`|`int 0..5` x687
  examples: `destroy|0` / "destroy carrier"; `destroy|1` / "destroy destroyer"; `destroy|2` / "destroy fighter"
- payload: `damaged`: bool 687, `unit`: str 687; key sets: {damaged,unit} x687
- label: 41 template(s): "destroy carrier" -> `<id> carrier` x228; "destroy fighter" -> `<id> fighter` x122; "destroy destroyer" -> `<id> destroyer` x89
- prompt: 1 template(s): "assign a hit" x357
- order: ids ascending (string) x269, contiguous x269
- subtypes: `assign_casualty` x352, `courageous_to_the_end_assign_casualty` x5
- sources: bench 32, bench-v1 39, bench-v2 34, bench-v3 43, bench-v4 26, bench-v5 23, golden20 126, te 29, templates 5
- built in: `combat.rs`

#### `technology`

- offered 672 times in 72 decisions; per decision 2..37 options in the whole choice
- ids: `56 distinct (name)` x633; `{a,b,c,d,f}`|`13 distinct (name) e.g. amd, bs, cr2` x39
  examples: `amd` / "Antimass Deflectors"; `gd` / "Gravity Drive"; `nm` / "Neural Motivator"
- payload: none
- label: 66 template(s): "gain amd" -> `gain <id>` x395; "research aida" -> `research <id>` x43; "exhaust Supercharge" -> `exhaust Supercharge` x26
- prompt: 9 template(s): "Supercharge: exhaust to apply +1 to each combat roll this round" x26; "gain which technology" x11; "Divert Funding: what to research with the funding" x10
- order: contiguous x39, ids ascending (string) x27, insertion order x11, ids descending x1
- subtypes: `supercharge` x26, `divert_funding_pick_technology` x19, `grant_reallocation_choose_technology` x8, `plagiarize_pick_technology` x7, `nullification_field_end_turn` x7
- sources: bench 8, bench-v1 2, bench-v2 5, bench-v3 5, bench-v4 2, bench-v5 2, golden20 32, te 16
- built in: `entropic_scars.rs`, `exploration.rs`, `factions/deepwrought.rs`, `factions/deepwrought_cards.rs`, `factions/firmament.rs`, `factions/firmament_plots.rs` (+5)

#### `sustain`

- offered 671 times in 606 decisions; per decision 2..4 options in the whole choice
- ids: `{sustain}`|`int 0..9` x512; `{sustain}`|`{space}`:`int 0..8` x159
  examples: `sustain|1` / "sustain damage on l1z1x_flagship"; `sustain|0` / "sustain damage on l1z1x_flagship"; `sustain|2` / "sustain damage on yin_flagship"
- payload: `unit`: str 671; key sets: {unit} x671
- label: 32 template(s): "sustain damage on dreadnought" -> `<id> damage on dreadnought` x252; "sustain damage on l1z1x_dreadnought" -> `<id> damage on l#z#x_dreadnought` x89; "sustain damage on sol_carrier2" -> `<id> damage on sol_carrier#` x41
- prompt: 1 template(s): "cancel a hit at 62" x606
- order: ids ascending (string) x62, contiguous x62
- subtypes: `sustain_damage` x606
- sources: bench 61, bench-v1 52, bench-v2 53, bench-v3 76, bench-v4 58, bench-v5 65, golden20 200, te 39, templates 2
- built in: `combat.rs`, `invasion.rs`

#### `prediction`

- offered 668 times in 118 decisions; per decision 2..44 options in the whole choice
- ids: `65 distinct (name)` x668
  examples: `alioprima` / "predict alioprima"; `archonvail` / "predict archonvail"; `arnor` / "predict arnor"
- payload: none
- label: 7 template(s): "predict alioprima" -> `predict <id>` x448; "predict a" -> `predict a` x43; "predict b" -> `predict b` x43
- prompt: 9 template(s): "Politics Rider: predict the agenda outcome" x20; "Construction Rider: predict the agenda outcome" x18; "Trade Rider: predict the agenda outcome" x15
- order: contiguous x118, ids descending x65, ids ascending (string) x53
- subtypes: `predict_agenda_outcome` x118
- sources: bench 4, bench-v1 20, bench-v2 11, bench-v3 23, bench-v4 23, bench-v5 9, golden20 24, te 4
- built in: `action_cards.rs`

#### `retreat_to`

- offered 541 times in 219 decisions; per decision 2..5 options in the whole choice
- ids: `int 1..78` x541
  examples: `32` / "retreat to 32"; `60` / "retreat to 60"; `06` / "retreat to 06"
- payload: `system`: str 541; key sets: {system} x541
- label: 1 template(s): "retreat to 32" -> `retreat to #` x541
- prompt: 1 template(s): "retreat to which system" x219
- order: contiguous x219, ids ascending (string) x211, insertion order x7, ids descending x1
- subtypes: `retreat_to` x219
- sources: bench 20, bench-v1 17, bench-v2 16, bench-v3 15, bench-v4 16, bench-v5 16, golden20 92, te 27
- built in: not found as a literal (built from a variable or in host/bot code)

#### `own_hand_card`

- offered 485 times in 105 decisions; per decision 2..10 options in the whole choice
- ids: `120 distinct (name)` x485
  examples: `coup` / "Coup d'Etat"; `dp1` / "Diplomatic Pressure"; `war_machine3` / "War Machine"
- payload: none
- label: 92 template(s): "Maneuvering Jets" -> `Maneuvering Jets` x34; "Veto" -> `Veto` x27; "Morale Boost" -> `Morale Boost` x20
- prompt: 6 template(s): "Scheming: choose and discard 1 action card from your hand" x76; "Stall Tactics: discard 1 action card from your hand" x14; "show f 1 action card from your hand" x5
- order: contiguous x105, insertion order x64, ids ascending (string) x35, ids descending x6
- subtypes: `scheming_discard` x76, `stall_tactics_discard` x14, `show_action_card` x9, `deepgloom_stall_tactics_discard` x4, `malleon_discard` x2
- sources: golden20 78, te 27
- built in: `action_cards.rs`

#### `promissory_note`

- offered 476 times in 414 decisions; per decision 2..8 options in the whole choice
- ids: `{use}` x400; `{note}`|`14 distinct (name) e.g. an, bmf, cavalry`:`14 distinct (name) e.g. arborec, empyrean, ghost` x76
  examples: `use` / "use ps"; `note|an:sol` / "give an:sol to the Naalu player"; `note|cf:sol` / "give cf:sol to the Naalu player"
- payload: none
- label: 5 template(s): "use ps" -> `<id> ps` x267; "use quash" -> `<id> quash` x114; "give an:sol to the Naalu player" -> `give <id>:<id> to the Naalu player` x76
- prompt: 10 template(s): "Quash: spend 1 strategy token to discard this agenda and reveal the next" x114; "Political Secret: b cannot vote on this agenda" x66; "Political Secret: f cannot vote on this agenda" x52
- order: ids ascending (string) x14, contiguous x14
- subtypes: `ps` x267, `quash` x114, `give_note` x14, `war_funding` x10, `favor` x9
- sources: bench 36, bench-v1 36, bench-v2 28, bench-v3 43, bench-v4 38, bench-v5 32, golden20 143, te 58
- built in: `factions/naalu.rs`

#### `stillness`

- offered 421 times in 66 decisions; per decision 4..7 options in the whole choice
- ids: `22 distinct (name) e.g. argent_destroyer, argent_destroyer2, argent_mech` x421
  examples: `carrier` / "capture a carrier from a's reinforcements"; `cruiser` / "capture a cruiser from a's reinforcements"; `destroyer` / "capture a destroyer from a's reinforcements"
- payload: none
- label: 6 template(s): "capture a carrier from a's reinforcements" -> `capture a <id> from a's reinforcements` x246; "capture a sol_carrier from e's reinforcements" -> `capture a <id> from e's reinforcements` x73; "capture a carrier from c's reinforcements" -> `capture a <id> from c's reinforcements` x33
- prompt: 1 template(s): "The Stillness of Stars: which unit do you capture" x66
- order: contiguous x66, insertion order x46, ids ascending (string) x20
- subtypes: `stillness_target` x66
- sources: bench 7, bench-v1 9, bench-v2 5, bench-v3 7, bench-v4 7, bench-v5 6, golden20 17, te 5, templates 3
- built in: `factions/cabal.rs`

#### `place_unit`

- offered 407 times in 121 decisions; per decision 2..15 options in the whole choice
- ids: `50 distinct (int/name)`|`80 distinct (name)` x407
  examples: `03|darien` / "place 2x infantry on darien in 03"; `03|space` / "place 2x infantry on space in 03"; `32|dalbootha` / "place 2x infantry on dalbootha in 32"
- payload: `count`: int 404, `system`: str 404; key sets: {count,system} x404; {-} x3
- label: 4 template(s): "place 2x infantry on darien in 03" -> `place #x infantry on <id> in #` x328; "place 1x mech on druaa in 09" -> `place #x mech on <id> in #` x75; "place 1 infantry with the mech on starpoint in 27" -> `place # infantry with the mech on <id> in #` x3
- prompt: 3 template(s): "place up to 2 infantry" x109; "place up to 1 mech" x9; "Ember Colossus: place 1 infantry with this mech" x3
- order: contiguous x84, ids ascending (string) x83, insertion order x1
- subtypes: `place_units_from_reinforcements` x118, `ember_colossus` x3
- sources: bench 4, bench-v1 6, bench-v2 3, bench-v3 5, bench-v4 1, bench-v5 3, golden20 52, te 14, templates 33
- built in: `action_cards.rs`, `factions/muaat.rs`, `production.rs`

#### `starting_technology`

- offered 364 times in 84 decisions; per decision 2..8 options in the whole choice
- ids: `8 distinct (name) e.g. aida, amd, det` x364
  examples: `nm` / "Neural Motivator"; `ps` / "Plasma Scoring"; `st` / "Sarween Tools"
- payload: `colour`: str 364, `owners`: array 133, `prerequisites`: int 364; key sets: {colour,prerequisites} x231; {colour,owners,prerequisites} x133
- label: 8 template(s): "Neural Motivator" -> `Neural Motivator` x84; "Sarween Tools" -> `Sarween Tools` x84; "Plasma Scoring" -> `Plasma Scoring` x56
- prompt: 2 template(s): "choose a starting technology (1 of 2)" x56; "choose a starting technology" x28
- order: ids ascending (string) x84, contiguous x84
- subtypes: `starting_technology` x84
- sources: golden20 15, te 3, templates 66
- built in: `starting_tech.rs`

#### `leader_ssruu_round_agent_unit`

- offered 351 times in 82 decisions; per decision 2..10 options in the whole choice
- ids: `{a,b,c,d,e,f}`|`39 distinct (name) e.g. abyz, accoen, arinam`|`32 distinct (name) e.g. argent_destroyer, argent_flagship, argent_mech`|`int 0..5`|`int 0..2` x351
  examples: `a|bereg|sol_infantry|0|0` / "a's sol_infantry on bereg (unit 1)"; `a|lirtaiv|sol_infantry|0|0` / "a's sol_infantry on lirtaiv (unit 1)"; `a|lirtaiv|sol_infantry|1|0` / "a's sol_infantry on lirtaiv (unit 2)"
- payload: none
- label: 12 template(s): "a's sol_infantry on bereg (unit 1)" -> `a's <id> on <id> (unit #)` x65; "f's infantry on zohbat (unit 1)" -> `f's <id> on <id> (unit #)` x54; "e's sol_infantry on mellon (unit 1)" -> `e's <id> on <id> (unit #)` x44
- prompt: 4 template(s): "solagent: choose one unit for +1 combat die" x47; "letnevagent: choose one unit for +1 combat die" x22; "Ssruu copying letnevagent: choose one unit for +1 combat die" x9
- order: contiguous x82, ids ascending (string) x53, insertion order x29
- subtypes: `leader_ssruu_round_agent_unit` x82
- sources: bench 9, bench-v5 8, golden20 58, te 7
- built in: `factions/borrowed_round_agents.rs`

#### `tiebreak`

- offered 320 times in 58 decisions; per decision 2..44 options in the whole choice
- ids: `74 distinct (name)` x320
  examples: `for` / "for"; `against` / "against"; `c` / "c"
- payload: `planet`: str 185, `system`: str 185; key sets: {planet,system} x185; {-} x135
- label: 7 template(s): "for" -> `<id>` x263; "a" -> `a` x12; "c" -> `c` x11
- prompt: 1 template(s): "speaker breaks the tie" x58
- order: contiguous x58, ids descending x38, ids ascending (string) x20
- subtypes: `vote_tiebreak` x58
- sources: bench 3, bench-v1 3, bench-v2 5, bench-v3 9, bench-v4 10, bench-v5 5, golden20 20, te 3
- built in: `vote.rs`

#### `mercer_take`

- offered 317 times in 52 decisions; per decision 4..10 options in the whole choice
- ids: `{take}`|`int 18..76`|`39 distinct (name) e.g. abyz, accoen, alioprima`|`{infantry,nomad_mech}`|`int 0..1` x317
  examples: `take|29|qucenn|infantry|0` / "remove infantry from qucenn"; `take|29|rarron|infantry|0` / "remove infantry from rarron"; `take|37|arinam|infantry|0` / "remove infantry from arinam"
- payload: `system`: str 317, `unit`: str 317; key sets: {system,unit} x317
- label: 2 template(s): "remove infantry from qucenn" -> `remove <id> from <id>` x311; "remove infantry from the space area of 65" -> `remove <id> from the <id> area of #` x6
- prompt: 1 template(s): "Field Marshal Mercer: remove a ground force (1 of 2)" x52
- order: contiguous x52, ids ascending (string) x35, insertion order x17
- subtypes: `mercer_take` x52
- sources: golden20 39, te 8, templates 5
- built in: `factions/nomad_agents.rs`

#### `sleeper`

- offered 268 times in 27 decisions; per decision 2..51 options in the whole choice
- ids: `59 distinct (name)` x243; `{move}`|`13 distinct (name) e.g. alioprima, ashtroth, bakal` x25
  examples: `place` / "place a sleeper token on alioprima"; `move|sakulag` / "move the sleeper token on sakulag to alioprima"; `move|alioprima` / "move the sleeper token on alioprima to dalbootha"
- payload: none
- label: 30 template(s): "ask to place a sleeper token on mr" -> `ask to place a sleeper token on <id>` x222; "allow" -> `<id>` x5; "move the sleeper token on ashtroth to bereg" -> `<id> the sleeper token on <id> to bereg` x3
- prompt: 19 template(s): "Slumberstate Computing: sleeper token on a planet another player controls" x6; "Terragenesis: sleeper token for zohbat" x2; "Terragenesis: sleeper token for starpoint" x2
- order: contiguous x22, insertion order x13, ids descending x9
- subtypes: `terragenesis_token` x16, `slumberstate_planet` x6, `slumberstate_allow` x5
- sources: golden20 14, te 11, templates 2
- built in: `factions/titans.rs`, `factions/titans_leaders.rs`

#### `place`

- offered 267 times in 117 decisions; per decision 2..8 options in the whole choice
- ids: `{place}`|`55 distinct (name)` x244; `int 7..71` x23
  examples: `place|kraag` / "place on kraag"; `place|siig` / "place on siig"; `place|archonren` / "place on archonren"
- payload: `capacity_excess_after`: int 242, `capacity_free_after`: int 242, `capacity_used`: int 242, `count`: int 242, `destination`: str 242, `fleet_excess_after`: int 242, `fleet_headroom_after`: int 242, `placed`: int 242, `sling_relay`: bool 23, `system`: str 267, `unit`: str 244; key sets: {capacity_excess_after,capacity_free_after,capacity_used,count,destination,fleet_excess_after,fleet_headroom_after,placed,system,unit} x242; {sling_relay,system} x23; {system,unit} x2
- label: 2 template(s): "place on kraag" -> `<id> on <id>` x244; "produce in 29" -> `produce in #` x23
- prompt: 3 template(s): "place the infantry" x77; "place the xxcha_mech" x29; "Sling Relay: produce in which dock system" x11
- order: contiguous x117, ids ascending (string) x102, insertion order x12, ids descending x3
- subtypes: `place_unit` x105, `(none)` x12
- sources: bench 12, bench-v2 2, bench-v4 9, bench-v5 8, golden20 66, te 20
- built in: `factions/firmament.rs`, `factions/titans.rs`, `production.rs`

#### `infantry`

- offered 248 times in 18 decisions; per decision 4..27 options in the whole choice
- ids: `int 1..76`|`26 distinct (name) e.g. 0.0.0, accoen, archonvail`|`int 0..20` x239; `{stop}` x9
  examples: `01|jord|1` / "infantry in 01"; `01|jord|2` / "infantry in 01"; `27|newalbion|0` / "infantry in 27"
- payload: `system`: str 239; key sets: {system} x239; {-} x9
- label: 2 template(s): "infantry in 01" -> `infantry in #` x239; "stop after one" -> `<id> after one` x9
- prompt: 2 template(s): "Refit Troops: which infantry to replace" x9; "Refit Troops: another infantry or stop" x9
- order: contiguous x18, insertion order x10, ids ascending (string) x8
- subtypes: `refit_pick_infantry` x18
- sources: bench 2, bench-v1 2, bench-v2 2, bench-v3 2, bench-v5 2, golden20 6, te 2
- built in: `action_cards.rs`, `agenda_effects.rs`, `breakthroughs.rs`, `exploration.rs`, `faction_effects_tests/d1.rs`, `faction_effects_tests/d2.rs` (+24)

#### `objective`

- offered 232 times in 174 decisions; per decision 2..5 options in the whole choice
- ids: `25 distinct (name) e.g. amass_wealth, ancient_monuments, build_defenses` x232
  examples: `amass_wealth` / "amass_wealth"; `corner` / "corner"; `trade_routes` / "trade_routes"
- payload: none
- label: 1 template(s): "amass_wealth" -> `<id>` x232
- prompt: 1 template(s): "score a public objective with Imperial" x174
- order: contiguous x44, ids ascending (string) x18, ids descending x14, insertion order x12
- subtypes: `imperial_score_objective` x174
- sources: bench 17, bench-v1 16, bench-v2 13, bench-v3 17, bench-v4 14, bench-v5 13, golden20 71, te 13
- built in: `factions/yin.rs`, `strategy_cards.rs`

#### `pillage`

- offered 218 times in 109 decisions; per decision 2..2 options in the whole choice
- ids: `{commodity,trade_good}`|`{a,b,c,d,e,f}` x218
  examples: `trade_good|a` / "take 1 trade good from a"; `commodity|a` / "take 1 commodity from a"; `trade_good|b` / "take 1 trade good from b"
- payload: none
- label: 12 template(s): "take 1 trade good from a" -> `take # trade good from a` x54; "take 1 commodity from a" -> `take # <id> from a` x53; "take 1 trade good from c" -> `take # trade good from c` x20
- prompt: 1 template(s): "Pillage: take 1 trade good or commodity" x109
- order: contiguous x109, ids descending x107, ids ascending (string) x2
- subtypes: `pillage_take` x109
- sources: bench 4, bench-v1 13, bench-v2 5, bench-v3 5, bench-v4 8, bench-v5 8, golden20 58, te 8
- built in: `factions/mentak.rs`

#### `action_card`

- offered 213 times in 60 decisions; per decision 3..35 options in the whole choice
- ids: `57 distinct (name)` x213
  examples: `rally` / "play Rally"; `fs4` / "play Flank Speed"; `const_rider` / "play Construction Rider"
- payload: `card`: str 114, `card_name`: str 114; key sets: {card,card_name} x114; {-} x99
- label: 23 template(s): "take mb1" -> `take <id>` x99; "play Flank Speed" -> `play Flank Speed` x23; "play Skilled Retreat" -> `play Skilled Retreat` x14
- prompt: 2 template(s): "play an action card (after SYSTEM_ACTIVATED)" x57; "The Codex: take which action card" x3
- order: contiguous x60, ids descending x36, ids ascending (string) x21, insertion order x3
- subtypes: `play_reaction_after_SYSTEM_ACTIVATED` x26, `play_reaction_after_COMBAT_ROUND_STARTED` x16, `play_reaction_after_AGENDA_REVEALED` x15, `codex_take_action_card` x3
- sources: bench 1, bench-v1 11, bench-v2 5, bench-v3 10, bench-v4 8, bench-v5 4, golden20 18, te 3
- built in: `reactions.rs`, `relics.rs`, `transactions.rs`

#### `recombine`

- offered 207 times in 49 decisions; per decision 3..32 options in the whole choice
- ids: `{recombine}`|`39 distinct (name) e.g. a, abyz, accoen` x207
  examples: `recombine|for` / "c must vote for or return a fleet token"; `recombine|against` / "c must vote against or return a fleet token"; `recombine|a` / "d must vote a or return a fleet token"
- payload: none
- label: 36 template(s): "c must vote for or return a fleet token" -> `c must vote <id> or return a fleet token` x43; "d must vote for or return a fleet token" -> `d must vote <id> or return a fleet token` x16; "d must vote a or return a fleet token" -> `d must vote a or return a fleet token` x8
- prompt: 6 template(s): "Genetic Recombination before d votes" x16; "Genetic Recombination before c votes" x14; "Genetic Recombination before a votes" x9
- order: contiguous x49, ids ascending (string) x29, ids descending x20
- subtypes: `recombination_outcome` x49
- sources: bench 7, bench-v1 8, bench-v2 7, bench-v3 9, bench-v4 2, bench-v5 7, golden20 9
- built in: not found as a literal (built from a variable or in host/bot code)

#### `skilled_retreat`

- offered 196 times in 52 decisions; per decision 2..7 options in the whole choice
- ids: `58 distinct (int/name)` x196
  examples: `17` / "withdraw to 17"; `28` / "withdraw to 28"; `51` / "withdraw to 51"
- payload: none
- label: 2 template(s): "withdraw to 17" -> `withdraw to #` x193; "withdraw to 82a" -> `withdraw to <id>` x3
- prompt: 1 template(s): "Skilled Retreat: withdraw to which system" x52
- order: ids ascending (string) x52, contiguous x52
- subtypes: `skilled_retreat_choose_system` x52
- sources: bench 3, bench-v1 4, bench-v2 5, bench-v3 7, bench-v4 3, bench-v5 5, golden20 22, te 3
- built in: `action_cards.rs`

#### `leader_hacanagent_branch`

- offered 194 times in 35 decisions; per decision 3..8 options in the whole choice
- ids: `9 distinct (name) e.g. a, b, c` x194
  examples: `self` / "gain 2 commodities"; `b` / "replenish b"; `c` / "replenish c"
- payload: none
- label: 9 template(s): "gain 2 commodities" -> `gain # commodities` x35; "replenish c" -> `replenish c` x33; "replenish b" -> `replenish b` x27
- prompt: 1 template(s): "Leader: gain 2 commodities or replenish another player" x35
- order: insertion order x35, contiguous x35
- subtypes: `leader_hacanagent_branch` x35
- sources: golden20 11, te 5, templates 19
- built in: `leaders.rs`

#### `mitosis`

- offered 139 times in 13 decisions; per decision 7..17 options in the whole choice
- ids: `{infantry,mech}`|`int 5..74`|`26 distinct (name) e.g. abyz, arinam, bereg` x139
  examples: `infantry|05|nestphar` / "Mitosis: place 1 infantry on nestphar in 05"; `infantry|34|centauri` / "Mitosis: place 1 infantry on centauri in 34"; `infantry|35|bereg` / "Mitosis: place 1 infantry on bereg in 35"
- payload: none
- label: 2 template(s): "Mitosis: place 1 infantry on nestphar in 05" -> `Mitosis: place # <id> on <id> in #` x116; "Mitosis: replace an infantry on nestphar in 05 with a mech" -> `Mitosis: replace an infantry on <id> in # with a <id>` x23
- prompt: 1 template(s): "Mitosis: place an infantry or replace one with a mech" x13
- order: ids ascending (string) x13, contiguous x13
- subtypes: `mitosis_place` x13
- sources: golden20 8, te 5
- built in: `factions/arborec.rs`

#### `reparations_exhaust`

- offered 137 times in 16 decisions; per decision 6..14 options in the whole choice
- ids: `60 distinct (name)` x137
  examples: `bereg` / "bereg"; `jord` / "jord"; `kraag` / "kraag"
- payload: `planet`: str 137, `system`: str 137; key sets: {planet,system} x137
- label: 1 template(s): "bereg" -> `<id>` x137
- prompt: 1 template(s): "exhaust a planet (Reparations)" x16
- order: ids ascending (string) x16, contiguous x16
- subtypes: `reparations_exhaust` x16
- sources: bench 1, bench-v1 2, bench-v2 2, bench-v3 2, bench-v4 2, bench-v5 1, golden20 6
- built in: `action_cards.rs`

#### `promissory note`

- offered 134 times in 26 decisions; per decision 3..8 options in the whole choice
- ids: `13 distinct (name) e.g. an, ce, cf`:`9 distinct (name) e.g. cabal, ghost, l1z1x` x134
  examples: `an:yin` / "an:yin"; `cf:yin` / "cf:yin"; `greyfire:yin` / "greyfire:yin"
- payload: none
- label: 1 template(s): "an:yin" -> `<id>:<id>` x134
- prompt: 1 template(s): "Diplomatic Pressure: which note do you give" x26
- order: ids ascending (string) x26, contiguous x26
- subtypes: `dp3_pick_promissory note` x11, `dp2_pick_promissory note` x7, `dp1_pick_promissory note` x5, `dp4_pick_promissory note` x3
- sources: bench-v1 6, bench-v2 3, bench-v3 9, bench-v4 8
- built in: not found as a literal (built from a variable or in host/bot code)

#### `component_action`

- offered 133 times in 133 decisions; per decision 2..13 options in the whole choice
- ids: `{faction}`|`{empyrean,mentak}`|`{blood_pact,dark_pact,pop}` x133
  examples: `faction|mentak|pop` / "Promise of Protection: place it faceup in your play area"; `faction|empyrean|blood_pact` / "Blood Pact: place it faceup in your play area"; `faction|empyrean|dark_pact` / "Dark Pact: place it faceup in your play area"
- payload: none
- label: 3 template(s): "Promise of Protection: place it faceup in your play area" -> `Promise of Protection: place it faceup in your play area` x75; "Blood Pact: place it faceup in your play area" -> `Blood Pact: place it faceup in your play area` x39; "Dark Pact: place it faceup in your play area" -> `Dark Pact: place it faceup in your play area` x19
- prompt: 1 template(s): "action phase" x133
- order: never more than one of this kind in a decision
- subtypes: `action_menu` x133
- sources: bench 20, bench-v5 14, golden20 75, te 24
- built in: `factions/empyrean_units.rs`, `factions/keleres.rs`, `factions/mentak.rs`

#### `economy`

- offered 100 times in 50 decisions; per decision 2..2 options in the whole choice
- ids: `{convert,gain}` x100
  examples: `gain` / "gain 1 commodity"; `convert` / "convert 1 commodity to a trade good"
- payload: none
- label: 2 template(s): "gain 1 commodity" -> `<id> # commodity` x50; "convert 1 commodity to a trade good" -> `<id> # commodity to a trade good` x50
- prompt: 3 template(s): "Crimson Legionnaire: gain 1 commodity or convert 1 to a trade good" x44; "Crimson commander: gain 1 commodity or convert 1 commodity to a trade good" x5; "Deepwrought commander: gain 1 commodity or convert 1 to a trade good" x1
- order: ids descending x50, contiguous x50
- subtypes: `legionnaire_payment` x44, `crimson_payment` x5, `deepwrought_payment` x1
- sources: bench 10, bench-v1 7, bench-v2 6, bench-v3 3, bench-v4 2, bench-v5 2, golden20 12, te 8
- built in: `factions/borrowed_commanders.rs`, `factions/mahact_units.rs`, `strategy_cards.rs`

#### `nekro_malleon`

- offered 95 times in 28 decisions; per decision 3..5 options in the whole choice
- ids: `{token}`|`{fleet,strategy,tactic}` x82; `{discard}` x13
  examples: `token|tactic` / "spend 1 tactic command token to gain 2 trade goods"; `token|fleet` / "spend 1 fleet command token to gain 2 trade goods"; `token|strategy` / "spend 1 strategy command token to gain 2 trade goods"
- payload: none
- label: 2 template(s): "spend 1 tactic command token to gain 2 trade goods" -> `spend # <id> command <id> to gain # trade goods` x82; "discard 1 action card to gain 2 trade goods" -> `<id> # action card to gain # trade goods` x13
- prompt: 1 template(s): "Nekro Malleon: discard 1 action card or spend 1 command token to gain 2 trade goods?" x28
- order: contiguous x28, insertion order x27, ids descending x1
- subtypes: `malleon_payment` x28
- sources: golden20 11, te 6, templates 11
- built in: `factions/nekro_units.rs`

#### `indoctrination`

- offered 93 times in 60 decisions; per decision 2..3 options in the whole choice
- ids: `{infantry,mech}` x93
  examples: `infantry` / "spend 2 influence to replace 1 infantry of c with a infantry"; `mech` / "spend 3 influence to replace 1 infantry of c with a mech"
- payload: `cost`: int 93; key sets: {cost} x93
- label: 12 template(s): "spend 2 influence to replace 1 infantry of c with a infantry" -> `spend # influence to replace # <id> of c with a <id>` x19; "spend 3 influence to replace 1 infantry of c with a mech" -> `spend # influence to replace # infantry of c with a <id>` x13; "spend 2 influence to replace 1 infantry of e with a infantry" -> `spend # influence to replace # <id> of e with a <id>` x12
- prompt: 32 template(s): "Indoctrination on semlore: replace an enemy infantry" x6; "Indoctrination on wellon: replace an enemy infantry" x5; "Indoctrination on lirtaiv: replace an enemy infantry" x5
- order: ids ascending (string) x33, contiguous x33
- subtypes: `indoctrination_replace` x60
- sources: bench 11, bench-v1 5, bench-v2 8, bench-v3 12, bench-v4 9, bench-v5 8, golden20 4, te 3
- built in: `factions/yin.rs`

#### `silence`

- offered 89 times in 26 decisions; per decision 2..7 options in the whole choice
- ids: `int 1..76` x89
  examples: `03` / "ships in 03 may pass blockades"; `60` / "ships in 60 may pass blockades"; `70` / "ships in 70 may pass blockades"
- payload: none
- label: 1 template(s): "ships in 03 may pass blockades" -> `ships in # may pass blockades` x89
- prompt: 1 template(s): "In The Silence Of Space: whose ships ignore blockades" x26
- order: ids ascending (string) x26, contiguous x26
- subtypes: `silence_choose_system` x26
- sources: bench 3, bench-v1 4, bench-v2 4, bench-v3 3, bench-v4 4, bench-v5 4, golden20 2, te 2
- built in: `action_cards.rs`

#### `leader`

- offered 83 times in 83 decisions; per decision 2..2 options in the whole choice
- ids: `{skip,use,yes}` x83
  examples: `use` / "exhaust the agent"; `skip` / "skip to commit"; `yes` / "exhaust the agent"
- payload: none
- label: 2 template(s): "exhaust the agent" -> `exhaust the agent` x75; "skip to commit" -> `<id> to commit` x8
- prompt: 9 template(s): "I48S: exhaust to let c replace 1 infantry in the active system with a mech" x31; "Doctor Sucaban: exhaust to let e trade infantry for research" x11; "Doctor Sucaban: exhaust to let f trade infantry for research" x11
- order: never more than one of this kind in a decision
- subtypes: `doctor_sucaban_exhaust` x43, `l1z1x_agent_swap` x32, `sardakk_hero_skip` x8
- sources: bench 5, bench-v1 9, bench-v2 5, bench-v3 3, bench-v4 5, bench-v5 3, golden20 40, te 13
- built in: `factions/argent.rs`, `factions/deepwrought_research.rs`, `factions/sardakk.rs`, `reactions.rs`, `strategy_cards.rs`, `timing.rs`

#### `ready_technology`

- offered 82 times in 66 decisions; per decision 2..7 options in the whole choice
- ids: `{ready}`|`{technology}`|`{gr,pi}` x82
  examples: `ready|technology|gr` / "Bio-Stims: ready Genetic Recombination"; `ready|technology|pi` / "Bio-Stims: ready Predictive Intelligence"
- payload: `bio_stims`: bool 82, `technology`: str 82; key sets: {bio_stims,technology} x82
- label: 2 template(s): "Bio-Stims: ready Genetic Recombination" -> `Bio-Stims: <id> Genetic Recombination` x62; "Bio-Stims: ready Predictive Intelligence" -> `Bio-Stims: <id> Predictive Intelligence` x20
- prompt: 1 template(s): "Bio-Stims" x66
- order: ids ascending (string) x16, contiguous x16
- subtypes: `bio_stims_ready` x66
- sources: bench 8, bench-v1 6, bench-v2 7, bench-v3 17, bench-v4 4, bench-v5 8, golden20 12, te 4
- built in: `technology.rs`

#### `agent`

- offered 75 times in 75 decisions; per decision 2..2 options in the whole choice
- ids: `{exhaust}` x75
  examples: `exhaust` / "exhaust Suffi An: you and the pillaged player each draw 1 action card"
- payload: none
- label: 1 template(s): "exhaust Suffi An: you and the pillaged player each draw 1 action card" -> `<id> Suffi An: you and the pillaged player each draw # action card` x75
- prompt: 6 template(s): "Suffi An: exhaust to draw 1 action card each with a" x25; "Suffi An: exhaust to draw 1 action card each with b" x17; "Suffi An: exhaust to draw 1 action card each with c" x15
- order: never more than one of this kind in a decision
- subtypes: `suffi_an` x75
- sources: bench 5, bench-v1 6, bench-v2 4, bench-v3 5, bench-v4 6, bench-v5 6, golden20 41, te 2
- built in: `factions/mentak.rs`, `leaders.rs`

#### `leader_jolnarhero_swap`

- offered 75 times in 11 decisions; per decision 6..8 options in the whole choice
- ids: `26 distinct (name) e.g. aida, amd, asc` x64; `{keep}`|`{amd,gd,gls,nm,ps,st}` x11
  examples: `fl` / "trade for fl"; `lwd` / "trade for lwd"; `det` / "trade for det"
- payload: none
- label: 2 template(s): "trade for fl" -> `trade for <id>` x64; "keep amd" -> `<id> <id>` x11
- prompt: 1 template(s): "Leader: replace which technology with what" x11
- order: insertion order x11, contiguous x11
- subtypes: `leader_jolnarhero_swap` x11
- sources: golden20 11
- built in: `leaders.rs`

#### `ambush_roll`

- offered 73 times in 60 decisions; per decision 2..3 options in the whole choice
- ids: `{cruiser,cruiser+cruiser,cruiser2,cruiser2+cruiser2}` x73
  examples: `cruiser` / "Ambush: roll for cruiser"; `cruiser+cruiser` / "Ambush: roll for cruiser and cruiser"; `cruiser2` / "Ambush: roll for cruiser2"
- payload: none
- label: 3 template(s): "Ambush: roll for cruiser" -> `Ambush: roll for <id>` x60; "Ambush: roll for cruiser and cruiser" -> `Ambush: roll for cruiser and cruiser` x12; "Ambush: roll for cruiser2 and cruiser2" -> `Ambush: roll for cruiser# and cruiser#` x1
- prompt: 1 template(s): "Ambush: choose up to 2 cruisers or destroyers to roll" x60
- order: ids ascending (string) x13, contiguous x13
- subtypes: `ambush_roll` x60
- sources: bench 3, bench-v1 2, bench-v2 2, bench-v3 5, bench-v4 4, bench-v5 6, golden20 36, te 2
- built in: `factions/mentak.rs`

#### `self_assembly`

- offered 70 times in 43 decisions; per decision 2..4 options in the whole choice
- ids: `13 distinct (name) e.g. 0.0.0, acheron, alioprima` x70
  examples: `acheron` / "exhaust Self-Assembly Routines to place a mech on acheron"; `alioprima` / "exhaust Self-Assembly Routines to place a mech on alioprima"; `bakal` / "exhaust Self-Assembly Routines to place a mech on bakal"
- payload: none
- label: 1 template(s): "exhaust Self-Assembly Routines to place a mech on acheron" -> `exhaust Self-Assembly Routines to place a mech on <id>` x70
- prompt: 1 template(s): "Self-Assembly Routines: exhaust to place 1 mech from your reinforcements on a planet you control in this syste" x43
- order: ids ascending (string) x18, contiguous x18
- subtypes: `self_assembly_mech` x43
- sources: bench 5, bench-v5 4, golden20 26, te 2, templates 6
- built in: `technology.rs`

#### `tribute`

- offered 68 times in 34 decisions; per decision 2..2 options in the whole choice
- ids: `{tribute}`|`{token,vote}` x68
  examples: `tribute|vote` / "cast at least 1 vote for for"; `tribute|token` / "remove 1 token from your fleet pool and return it to reinforcements"
- payload: none
- label: 9 template(s): "remove 1 token from your fleet pool and return it to reinforcements" -> `remove # <id> from your fleet pool and return it to reinforcements` x34; "cast at least 1 vote for for" -> `cast at least # <id> for for` x7; "cast at least 1 vote for a" -> `cast at least # <id> for a` x7
- prompt: 8 template(s): "Genetic Recombination: vote for or return a fleet token" x7; "Genetic Recombination: vote a or return a fleet token" x7; "Genetic Recombination: vote d or return a fleet token" x6
- order: ids descending x34, contiguous x34
- subtypes: `recombination_tribute` x34
- sources: bench 6, bench-v1 2, bench-v2 3, bench-v3 8, bench-v4 2, bench-v5 6, golden20 7
- built in: not found as a literal (built from a variable or in host/bot code)

#### `flock_migration_ship`

- offered 68 times in 14 decisions; per decision 3..9 options in the whole choice
- ids: `{ship}`|`int 0..7` x68
  examples: `ship|0` / "move argent_destroyer from 27"; `ship|1` / "move carrier from 29"; `ship|2` / "move argent_destroyer from 29"
- payload: none
- label: 5 template(s): "move fighter from 58" -> `move fighter from #` x22; "move argent_destroyer from 27" -> `move argent_destroyer from #` x15; "move dreadnought from 58" -> `move dreadnought from #` x13
- prompt: 1 template(s): "Flock Migration: choose a ship to move" x14
- order: ids ascending (string) x14, contiguous x14
- subtypes: `flock_migration_ship` x14
- sources: golden20 12, te 2
- built in: `factions/argent.rs`

#### `stymie`

- offered 62 times in 2 decisions; per decision 31..31 options in the whole choice
- ids: `int 17..76` x62
  examples: `17` / "place d's command token in 17"; `18` / "place d's command token in 18"; `20` / "place d's command token in 20"
- payload: none
- label: 2 template(s): "place d's command token in 17" -> `place d's command token in #` x31; "place c's command token in 17" -> `place c's command token in #` x31
- prompt: 2 template(s): "Stymie: where to put d's command token" x1; "Stymie: where to put c's command token" x1
- order: ids ascending (string) x2, contiguous x2
- subtypes: `stymie_system` x2
- sources: golden20 2
- built in: `factions/arborec.rs`

#### `recall`

- offered 54 times in 16 decisions; per decision 2..6 options in the whole choice
- ids: `int 1..77` x54
  examples: `18` / "recall your token from 18"; `30` / "recall your token from 30"; `35` / "recall your token from 35"
- payload: none
- label: 1 template(s): "recall your token from 18" -> `recall your token from #` x54
- prompt: 1 template(s): "Unexpected Action: recall your token from where" x16
- order: ids ascending (string) x16, contiguous x16
- subtypes: `unexpected_pick_recall` x16
- sources: bench 2, bench-v2 1, bench-v3 2, bench-v4 2, golden20 8, te 1
- built in: `strategy_cards.rs`

#### `law`

- offered 53 times in 53 decisions; per decision 2..2 options in the whole choice
- ids: `{discard}` x53
  examples: `discard` / "discard arbiter"
- payload: none
- label: 3 template(s): "discard minister_peace" -> `<id> minister_peace` x27; "discard minister_war" -> `<id> minister_war` x14; "discard arbiter" -> `<id> arbiter` x12
- prompt: 3 template(s): "Minister of Peace: discard to end the active player's turn" x27; "Minister of War: discard to retrieve a command token and act again" x14; "Imperial Arbiter: discard to swap a strategy card" x12
- order: never more than one of this kind in a decision
- subtypes: `offer_discard_law` x53
- sources: bench 4, bench-v1 5, bench-v2 6, bench-v3 5, bench-v4 5, bench-v5 3, golden20 20, te 5
- built in: `laws.rs`, `vote.rs`

#### `reroll_die`

- offered 53 times in 53 decisions; per decision 2..2 options in the whole choice
- ids: `{reroll}`|`int 0..1`:`int 0..5` x53
  examples: `reroll|0:0` / "reroll die 1 of combat value 7 (shows 3)"; `reroll|0:1` / "reroll die 2 of combat value 7 (shows 6)"; `reroll|0:2` / "reroll die 3 of combat value 7 (shows 9)"
- payload: `face`: int 53, `hits_on`: int 53, `unit`: str 53; key sets: {face,hits_on,unit} x53
- label: 6 template(s): "reroll die 1 of combat value 7 (shows 3)" -> `<id> die # of combat value # (shows #)` x40; "reroll die 1 of muaat_warsun (shows 3)" -> `<id> die # of muaat_warsun (shows #)` x6; "reroll die 1 of dreadnought2 (shows 8)" -> `<id> die # of dreadnought# (shows #)` x2
- prompt: 6 template(s): "reroll die 1 of combat value 7" x40; "reroll die 1 of muaat_warsun" x6; "reroll die 1 of dreadnought2" x2
- order: never more than one of this kind in a decision
- subtypes: `fire_team_reroll` x40, `crown_of_thalnos_relic_reroll` x11, `jolnar_commander_reroll` x2
- sources: bench 5, bench-v1 5, bench-v2 3, bench-v3 5, bench-v4 3, bench-v5 5, golden20 16, te 11
- built in: `combat.rs`

#### `annex`

- offered 50 times in 15 decisions; per decision 2..9 options in the whole choice
- ids: `24 distinct (name) e.g. abaddon, archonvail, arinam` x50
  examples: `wellon` / "gain control of wellon"; `arinam` / "gain control of arinam"; `meer` / "gain control of meer"
- payload: `planet`: str 50, `system`: str 50; key sets: {planet,system} x50
- label: 1 template(s): "gain control of wellon" -> `gain control of <id>` x50
- prompt: 1 template(s): "Peace Accords: annex a planet" x15
- order: contiguous x12, insertion order x7, ids ascending (string) x3, ids descending x2
- subtypes: `peace_accords_annex` x15
- sources: bench 1, bench-v1 3, bench-v2 5, bench-v3 1, bench-v4 1, golden20 1, te 1, templates 2
- built in: `faction_abilities.rs`

#### `count`

- offered 48 times in 14 decisions; per decision 2..5 options in the whole choice
- ids: `int 0..4` x48
  examples: `0` / "spend 0 trade goods"; `1` / "spend 1 trade goods"; `2` / "spend 2 trade goods"
- payload: none
- label: 1 template(s): "spend 0 trade goods" -> `spend # trade goods` x48
- prompt: 1 template(s): "Bribery: how many trade goods to spend?" x14
- order: ids ascending (string) x14, contiguous x14
- subtypes: `bribery_pick_count` x14
- sources: bench 2, bench-v1 3, bench-v3 1, bench-v4 2, bench-v5 1, golden20 5
- built in: `action_cards.rs`, `factions/deepwrought.rs`, `game.rs`

#### `ingress_system`

- offered 48 times in 20 decisions; per decision 1..4 options in the whole choice
- ids: `int 19..76` x48
  examples: `22` / "place a BIOTIC ingress in 22 (tarmann)"; `76` / "place a BIOTIC ingress in 76 (rigeliii)"; `27` / "place a BIOTIC ingress in 27 (newalbion)"
- payload: `planet`: str 48, `system`: str 48, `technology_colour`: str 48; key sets: {planet,system,technology_colour} x48
- label: 12 template(s): "place a BIOTIC ingress in 22 (tarmann)" -> `place a BIOTIC ingress in # (tarmann)` x10; "place a WARFARE ingress in 37 (meer)" -> `place a WARFARE ingress in # (meer)` x7; "place a BIOTIC ingress in 76 (rigeliii)" -> `place a BIOTIC ingress in # (rigeliii)` x5
- prompt: 1 template(s): "The Fracture: choose a BIOTIC technology-specialty ingress system" x20
- order: contiguous x16, insertion order x7, ids ascending (string) x5, ids descending x4
- subtypes: `fracture_choose_ingress_system` x20
- sources: te 6, templates 14
- built in: `fracture.rs`

#### `custodians`

- offered 47 times in 47 decisions; per decision 2..2 options in the whole choice
- ids: `{yes}` x47
  examples: `yes` / "remove it for a victory point"
- payload: none
- label: 1 template(s): "remove it for a victory point" -> `remove it for a victory point` x47
- prompt: 1 template(s): "spend 6 influence to remove the custodians token" x47
- order: never more than one of this kind in a decision
- subtypes: `remove_custodians` x47
- sources: bench 4, bench-v1 4, bench-v2 3, bench-v3 4, bench-v4 4, bench-v5 4, golden20 20, te 4
- built in: `invasion.rs`

#### `unit`

- offered 47 times in 34 decisions; per decision 1..2 options in the whole choice
- ids: `{pds,spacedock}`|`int 18..75`|`14 distinct (name) e.g. accoen, bereg, corneeq` x30; `{deploy,use}` x17
  examples: `use` / "end their turn"; `deploy` / "deploy the mech"; `pds|60|perimeter` / "Reclaimer: place a PDS on perimeter"
- payload: `planet`: str 30, `system`: str 30; key sets: {planet,system} x30; {-} x17
- label: 4 template(s): "Reclaimer: place a PDS on perimeter" -> `Reclaimer: place a PDS on <id>` x17; "Reclaimer: place a space dock on perimeter" -> `Reclaimer: place a space dock on <id>` x13; "end their turn" -> `end their turn` x11
- prompt: 9 template(s): "Reclaimer: place a structure" x17; "Starlancer: spend c's command token to end their turn" x5; "Starlancer: spend a's command token to end their turn" x4
- order: ids ascending (string) x13, contiguous x13
- subtypes: `reclaimer` x17, `starlancer_end_turn` x11, `deploy_mech` x6
- sources: bench 3, bench-v2 3, bench-v3 1, bench-v4 1, bench-v5 1, golden20 22, te 3
- built in: `combat.rs`, `factions/arborec.rs`, `factions/bastion_units.rs`, `factions/cabal.rs`, `factions/crimson_cards.rs`, `factions/empyrean_units.rs` (+15)

#### `follower`

- offered 46 times in 23 decisions; per decision 2..2 options in the whole choice
- ids: `{no,yes}`|`{a,b,c,d,e,f}` x46
  examples: `yes|c` / "let c follow"; `no|c` / "not c"; `yes|d` / "let d follow"
- payload: none
- label: 12 template(s): "let c follow" -> `let c follow` x5; "not c" -> `not c` x5; "let d follow" -> `let d follow` x5
- prompt: 6 template(s): "Mathis Mathinus: may c follow the secondary" x5; "Mathis Mathinus: may d follow the secondary" x5; "Mathis Mathinus: may a follow the secondary" x4
- order: ids descending x23, contiguous x23
- subtypes: `hero_follower` x23
- sources: golden20 18, te 5
- built in: `factions/winnu.rs`

#### `breakthrough`

- offered 45 times in 45 decisions; per decision 2..2 options in the whole choice
- ids: `{allow}` x45
  examples: `allow` / "allow Scheming"
- payload: none
- label: 6 template(s): "allow Scheming" -> `<id> Scheming` x32; "allow b" -> `<id> b` x4; "allow c" -> `<id> c` x3
- prompt: 10 template(s): "Deepgloom Executable: allow c to use Scheming" x9; "Deepgloom Executable: allow e to use Scheming" x7; "Deepgloom Executable: allow f to use Scheming" x6
- order: never more than one of this kind in a decision
- subtypes: `deepgloom_allow_scheming` x32, `deepgloom_allow_stall_tactics` x13
- sources: te 45
- built in: `factions/cabal.rs`, `factions/firmament.rs`, `factions/sardakk.rs`, `factions/yin.rs`, `factions/yssaril.rs`, `timing.rs`

#### `leader_xxchaagent_infantry`

- offered 44 times in 22 decisions; per decision 2..2 options in the whole choice
- ids: `{keep,remove}` x44
  examples: `remove` / "remove 1 infantry"; `keep` / "keep it"
- payload: none
- label: 2 template(s): "remove 1 infantry" -> `<id> # infantry` x22; "keep it" -> `<id> it` x22
- prompt: 1 template(s): "Leader: remove 1 infantry from that planet?" x22
- order: ids descending x22, contiguous x22
- subtypes: `leader_xxchaagent_remove_infantry` x22
- sources: bench 5, bench-v1 4, bench-v2 1, bench-v4 2, bench-v5 2, golden20 5, te 3
- built in: `leaders.rs`

#### `elect`

- offered 37 times in 15 decisions; per decision 2..3 options in the whole choice
- ids: `{a,b,c,d,e,f}` x37
  examples: `c` / "c"; `d` / "d"; `b` / "b"
- payload: none
- label: 10 template(s): "b" -> `b` x7; "d" -> `d` x6; "elect b" -> `elect b` x5
- prompt: 3 template(s): "which tied player does the agenda name" x7; "Confusing Legal Text: who is the elected player instead" x5; "which of the trailing players may settle the planet" x3
- order: ids ascending (string) x15, contiguous x15
- subtypes: `agenda_elect_tiebreak` x7, `confusing_legal_text_elect` x5, `redistribution_choose_settler` x3
- sources: bench 1, bench-v1 1, bench-v2 1, bench-v3 1, bench-v4 2, bench-v5 3, golden20 5, te 1
- built in: `action_cards.rs`, `agenda_effects.rs`

#### `reparations_ready`

- offered 36 times in 7 decisions; per decision 2..9 options in the whole choice
- ids: `28 distinct (name) e.g. abyz, alioprima, archonren` x36
  examples: `fria` / "fria"; `jord` / "jord"; `lisis` / "lisis"
- payload: `planet`: str 36, `system`: str 36; key sets: {planet,system} x36
- label: 1 template(s): "fria" -> `<id>` x36
- prompt: 1 template(s): "ready a planet (Reparations)" x7
- order: ids ascending (string) x7, contiguous x7
- subtypes: `reparations_ready` x7
- sources: bench 1, bench-v1 1, bench-v2 1, bench-v3 2, bench-v5 1, golden20 1
- built in: `action_cards.rs`

#### `mercer_place`

- offered 34 times in 16 decisions; per decision 2..3 options in the whole choice
- ids: `15 distinct (name) e.g. abyz, arinam, arnor` x34
  examples: `abyz` / "place infantry on abyz"; `fria` / "place infantry on fria"; `arinam` / "place infantry on arinam"
- payload: `system`: str 34, `unit`: str 34; key sets: {system,unit} x34
- label: 2 template(s): "place infantry on abyz" -> `place infantry on <id>` x26; "place nomad_mech on rigeli" -> `place nomad_mech on <id>` x8
- prompt: 2 template(s): "Field Marshal Mercer: place infantry on a planet" x13; "Field Marshal Mercer: place nomad_mech on a planet" x3
- order: ids ascending (string) x16, contiguous x16
- subtypes: `mercer_place` x16
- sources: golden20 14, te 2
- built in: `factions/nomad_agents.rs`

#### `tether`

- offered 34 times in 4 decisions; per decision 6..12 options in the whole choice
- ids: `{move}`|`int 18..54`|`int 56..65`|`int 18..60`|`int 30..72` x22; `{place}`|`int 18..60`|`int 60..65` x12
  examples: `place|23|65` / "place a Void Tether on the border of 23 and 65"; `place|28|65` / "place a Void Tether on the border of 28 and 65"; `place|33|65` / "place a Void Tether on the border of 33 and 65"
- payload: none
- label: 2 template(s): "move the Void Tether on 54 and 65 to the border of 18 and 60" -> `<id> the Void Tether on # and # to the border of # and #` x22; "place a Void Tether on the border of 23 and 65" -> `<id> a Void Tether on the border of # and #` x12
- prompt: 1 template(s): "Void Tether: which border" x4
- order: contiguous x4, ids ascending (string) x3, insertion order x1
- subtypes: `tether_border` x4
- sources: te 4
- built in: `factions/empyrean.rs`

#### `scuttle`

- offered 26 times in 10 decisions; per decision 2..4 options in the whole choice
- ids: `int 0..3` x26
  examples: `0` / "destroy the one on mr"; `1` / "destroy the one on tarmann"; `2` / "destroy the one on semlore"
- payload: `planet`: str 26, `system`: str 26; key sets: {planet,system} x26
- label: 17 template(s): "destroy the one on 0.0.0" -> `destroy the one on #.#` x5; "destroy the one on tarmann" -> `destroy the one on tarmann` x2; "destroy the one on xxehan" -> `destroy the one on xxehan` x2
- prompt: 1 template(s): "destroy one of your structures" x10
- order: ids ascending (string) x10, contiguous x10
- subtypes: `defense_act_choose_pds` x10
- sources: bench-v1 3, bench-v2 2, bench-v3 3, bench-v4 2
- built in: `agenda_effects.rs`

#### `vortex`

- offered 24 times in 8 decisions; per decision 2..6 options in the whole choice
- ids: `{a,b,c,d,f}`|`9 distinct (name) e.g. argent_destroyer, carrier, cruiser` x24
  examples: `a|dreadnought` / "take a dreadnought from a's reinforcements"; `a|infantry` / "take a infantry from a's reinforcements"; `c|fighter` / "take a fighter from c's reinforcements"
- payload: none
- label: 5 template(s): "take a dreadnought from a's reinforcements" -> `take a <id> from a's reinforcements` x11; "take a fighter from c's reinforcements" -> `take a <id> from c's reinforcements` x7; "take a carrier from f's reinforcements" -> `take a <id> from f's reinforcements` x4
- prompt: 1 template(s): "Vortex: which unit type do you take" x8
- order: ids ascending (string) x8, contiguous x8
- subtypes: `vortex_target` x8
- sources: bench-v2 2, bench-v3 1, golden20 3, te 2
- built in: `factions/cabal.rs`

#### `ingress_move`

- offered 24 times in 4 decisions; per decision 6..8 options in the whole choice
- ids: `int 18..76`>`int 54..54` x24
  examples: `18>54` / "move the ingress token in 18 into 54"; `19>54` / "move the ingress token in 19 into 54"; `22>54` / "move the ingress token in 22 into 54"
- payload: none
- label: 1 template(s): "move the ingress token in 18 into 54" -> `move the ingress token in # into #` x24
- prompt: 1 template(s): "Al'Raith Ix Ianovar: move an ingress token into a gravity rift" x4
- order: ids ascending (string) x4, contiguous x4
- subtypes: `ingress_move` x4
- sources: te 1, templates 3
- built in: `factions/cabal.rs`, `factions/obsidian.rs`

#### `vote_predictive`

- offered 23 times in 23 decisions; per decision 2..2 options in the whole choice
- ids: `{predictive}`|`{cast}` x23
  examples: `predictive|cast` / "cast 3 additional votes for b"
- payload: `votes`: int 23; key sets: {votes} x23
- label: 8 template(s): "cast 3 additional votes for d" -> `<id> # additional votes for d` x6; "cast 3 additional votes for for" -> `<id> # additional votes for for` x6; "cast 3 additional votes for b" -> `<id> # additional votes for b` x5
- prompt: 6 template(s): "Predictive Intelligence: 3 more votes for b" x12; "Predictive Intelligence: 3 more votes for for" x6; "Predictive Intelligence: 3 more votes for against" x2
- order: never more than one of this kind in a decision
- subtypes: `vote_predictive_intelligence` x23
- sources: bench 5, bench-v5 7, golden20 8, te 3
- built in: not found as a literal (built from a variable or in host/bot code)

#### `flock_migration_to`

- offered 22 times in 11 decisions; per decision 2..4 options in the whole choice
- ids: `{to}`|`int 14..73` x22
  examples: `to|27` / "move it to 27"; `to|62` / "move it to 62"; `to|71` / "move it to 71"
- payload: none
- label: 1 template(s): "move it to 27" -> `move it <id> #` x22
- prompt: 1 template(s): "Flock Migration: choose the destination" x11
- order: ids ascending (string) x8, contiguous x8
- subtypes: `flock_migration_to` x11
- sources: golden20 9, te 2
- built in: `factions/argent.rs`

#### `fragment`

- offered 21 times in 9 decisions; per decision 2..3 options in the whole choice
- ids: `{CULTURAL,HAZARDOUS,INDUSTRIAL}` x21
  examples: `CULTURAL` / "CULTURAL"; `INDUSTRIAL` / "INDUSTRIAL"; `HAZARDOUS` / "HAZARDOUS"
- payload: none
- label: 2 template(s): "CULTURAL" -> `<id>` x11; "purge 1 CULTURAL" -> `purge # <id>` x10
- prompt: 2 template(s): "Seize Artifact: which fragment to take" x5; "purge 1 fragment of which type" x4
- order: ids ascending (string) x9, contiguous x9
- subtypes: `seize_pick_fragment` x5, `fragment_type` x4
- sources: bench-v2 1, bench-v3 1, bench-v4 1, golden20 6
- built in: `factions/naaz.rs`, `transactions.rs`

#### `production_agent`

- offered 21 times in 21 decisions; per decision 8..9 options in the whole choice
- ids: `{exhaust_agent}`|`{winnuagent}` x21
  examples: `exhaust_agent|winnuagent` / "Exhaust Berekar Berekon: -2 cost"
- payload: `discount_offered`: int 21, `leader`: str 21; key sets: {discount_offered,leader} x21
- label: 1 template(s): "Exhaust Berekar Berekon: -2 cost" -> `Exhaust Berekar Berekon: # cost` x21
- prompt: 1 template(s): "produce in 07 (5 left)" x21
- order: never more than one of this kind in a decision
- subtypes: `produce_unit` x21
- sources: te 6, templates 15
- built in: not found as a literal (built from a variable or in host/bot code)

#### `hero`

- offered 20 times in 10 decisions; per decision 3..3 options in the whole choice
- ids: `{discard,take}` x20
  examples: `take` / "take Sanction from a"; `discard` / "force a to discard 3 random action cards"
- payload: none
- label: 16 template(s): "force b to discard 3 random action cards" -> `force b to <id> # random action cards` x2; "force c to discard 3 random action cards" -> `force c to <id> # random action cards` x2; "force d to discard 3 random action cards" -> `force d to <id> # random action cards` x2
- prompt: 10 template(s): "Kyver: a showed Sanction" x1; "Kyver: b showed Deadly Plot" x1; "Kyver: c showed Imperial Rider" x1
- order: ids descending x10, contiguous x10
- subtypes: `kyver_take_or_discard` x10
- sources: golden20 10
- built in: `factions/yssaril.rs`, `leaders.rs`

#### `vote_trade_goods`

- offered 19 times in 6 decisions; per decision 2..6 options in the whole choice
- ids: `{spend}`|`int 1..5` x19
  examples: `spend|1` / "spend 1 trade goods for 2 votes"; `spend|2` / "spend 2 trade goods for 4 votes"; `spend|3` / "spend 3 trade goods for 6 votes"
- payload: `trade_goods`: int 19; key sets: {trade_goods} x19
- label: 1 template(s): "spend 1 trade goods for 2 votes" -> `<id> # trade goods for # votes` x19
- prompt: 3 template(s): "spend trade goods for votes on for" x3; "spend trade goods for votes on against" x2; "spend trade goods for votes on a" x1
- order: ids ascending (string) x5, contiguous x5
- subtypes: `vote_spend_trade_goods` x6
- sources: golden20 6
- built in: `vote.rs`

#### `research_waiver_payment`

- offered 18 times in 4 decisions; per decision 2..8 options in the whole choice
- ids: `{ground}`|`int 3..76`|`13 distinct (name) e.g. centauri, darien, hopesend` x17; `{space}`|`int 76..76` x1
  examples: `ground|03|darien` / "return 1 infantry from darien in 03"; `ground|34|centauri` / "return 1 infantry from centauri in 34"; `ground|76|rigeli` / "return 1 infantry from rigeli in 76"
- payload: none
- label: 1 template(s): "return 1 infantry from darien in 03" -> `return # infantry from <id> in #` x18
- prompt: 1 template(s): "return 1 infantry to reinforcements to ignore its prerequisites: choose the payment" x4
- order: ids ascending (string) x3, contiguous x3
- subtypes: `research_waiver_payment` x4
- sources: bench-v1 1, bench-v2 1, bench-v3 1, bench-v5 1
- built in: `strategy_cards.rs`

#### `take_revealed_card`

- offered 17 times in 6 decisions; per decision 2..4 options in the whole choice
- ids: `16 distinct (name) e.g. assassin, confusing, cripple` x17
  examples: `dh2` / "Direct Hit"; `plague` / "Plague"; `assassin` / "Assassinate Representative"
- payload: none
- label: 15 template(s): "Maneuvering Jets" -> `Maneuvering Jets` x3; "Direct Hit" -> `Direct Hit` x1; "Plague" -> `Plague` x1
- prompt: 4 template(s): "take 1 of e's action cards" x2; "take 1 of d's action cards" x2; "take 1 of b's action cards" x1
- order: contiguous x6, ids ascending (string) x5, insertion order x1
- subtypes: `take_revealed_action_card` x6
- sources: golden20 4, te 2
- built in: `action_cards.rs`

#### `flock_migration_cargo`

- offered 16 times in 6 decisions; per decision 2..6 options in the whole choice
- ids: `{cargo}`|`int 0..4` x16
  examples: `cargo|0` / "carry fighter"; `cargo|1` / "carry infantry"; `cargo|2` / "carry infantry"
- payload: none
- label: 3 template(s): "carry infantry" -> `carry infantry` x8; "carry fighter" -> `carry fighter` x7; "carry argent_mech" -> `carry argent_mech` x1
- prompt: 1 template(s): "Flock Migration: carry a unit aboard (it cannot be landed)" x6
- order: ids ascending (string) x4, contiguous x4
- subtypes: `flock_migration_cargo` x6
- sources: golden20 6
- built in: `factions/argent.rs`

#### `nekro_assimilate`

- offered 14 times in 7 decisions; per decision 1..4 options in the whole choice
- ids: `{gain,z}`|`{c,d,e,f}`|`{amd,gd,naalu,nm,sr,st}` x14
  examples: `gain|c|sr` / "gain Sling Relay from c"; `gain|e|amd` / "gain Antimass Deflectors from e"; `gain|e|st` / "gain Sarween Tools from e"
- payload: none
- label: 8 template(s): "gain Neural Motivator from d" -> `<id> Neural Motivator from d` x3; "place the Z token on the faction sheet of d" -> `place the Z token on the faction sheet of d` x3; "gain Sling Relay from c" -> `<id> Sling Relay from c` x2
- prompt: 1 template(s): "gain a technology, or assimilate instead" x7
- order: ids ascending (string) x4, contiguous x4
- subtypes: `assimilate` x7
- sources: golden20 3, te 4
- built in: `factions/nekro.rs`

#### `letnev_commander_trade_good`

- offered 11 times in 11 decisions; per decision 2..2 options in the whole choice
- ids: `{gain}` x11
  examples: `gain` / "Gain 1 trade good"
- payload: none
- label: 1 template(s): "Gain 1 trade good" -> `Gain # trade good` x11
- prompt: 1 template(s): "Rear Admiral Farran: gain 1 trade good for the sustained damage" x11
- order: never more than one of this kind in a decision
- subtypes: `letnev_commander_trade_good` x11
- sources: golden20 11
- built in: `combat.rs`

#### `commander`

- offered 11 times in 4 decisions; per decision 2..3 options in the whole choice
- ids: `{action_cards,promissory_notes,secret_objectives}` x11
  examples: `action_cards` / "look at a's action cards"; `promissory_notes` / "look at a's promissory notes"; `secret_objectives` / "look at a's secret objectives"
- payload: none
- label: 11 template(s): "look at a's action cards" -> `look at a's action cards` x1; "look at a's promissory notes" -> `look at a's promissory notes` x1; "look at a's secret objectives" -> `look at a's secret objectives` x1
- prompt: 4 template(s): "So Ata: what to look at on a" x1; "So Ata: what to look at on b" x1; "So Ata: what to look at on c" x1
- order: ids ascending (string) x4, contiguous x4
- subtypes: `so_ata_look` x4
- sources: golden20 4
- built in: `factions/mentak.rs`, `factions/yin.rs`, `factions/yssaril.rs`, `leaders.rs`, `promissory.rs`

#### `research_waiver`

- offered 10 times in 10 decisions; per decision 2..2 options in the whole choice
- ids: `{waiver}`|`int 0..0` x10
  examples: `waiver|0` / "return 1 infantry to reinforcements to ignore its prerequisites"
- payload: none
- label: 1 template(s): "return 1 infantry to reinforcements to ignore its prerequisites" -> `return # infantry to reinforcements to ignore its prerequisites` x10
- prompt: 1 template(s): "research gd: choose a prerequisite waiver" x10
- order: never more than one of this kind in a decision
- subtypes: `research_waiver` x10
- sources: bench 2, bench-v1 1, bench-v2 2, bench-v3 1, bench-v4 1, bench-v5 2, golden20 1
- built in: `strategy_cards.rs`

#### `scepter`

- offered 9 times in 2 decisions; per decision 4..5 options in the whole choice
- ids: `int 19..76` x9
  examples: `19` / "sol, l1z1x place a command token in 19"; `36` / "sol, l1z1x place a command token in 36"; `46` / "sol, l1z1x place a command token in 46"
- payload: none
- label: 2 template(s): "mentak places a command token in 28" -> `mentak places a command token in #` x5; "sol, l1z1x place a command token in 19" -> `sol, l#z#x place a command token in #` x4
- prompt: 1 template(s): "Scepter of Dominion: choose a system" x2
- order: ids ascending (string) x2, contiguous x2
- subtypes: `scepter_system` x2
- sources: bench-v3 2
- built in: `factions/mahact.rs`

#### `relic`

- offered 9 times in 9 decisions; per decision 2..2 options in the whole choice
- ids: `{neuraloop,purge}` x9
  examples: `neuraloop` / "purge neuraloop"; `purge` / "purge the Dominus Orb"
- payload: none
- label: 2 template(s): "purge neuraloop" -> `purge <id>` x8; "purge the Dominus Orb" -> `<id> the Dominus Orb` x1
- prompt: 6 template(s): "The Neuraloop: purge a relic to replace massive_cities" x4; "The Dominus Orb: purge to move out of your own command tokens" x1; "The Neuraloop: purge a relic to replace lead" x1
- order: never more than one of this kind in a decision
- subtypes: `neuraloop_choose_relic_to_purge` x8, `dominus_orb_purge_to_move` x1
- sources: golden20 6, te 3
- built in: `relics.rs`

#### `repeal`

- offered 8 times in 3 decisions; per decision 2..3 options in the whole choice
- ids: `{censure,conscription,minister_sciences,regulations,revolution,warrant}` x8
  examples: `censure` / "censure"; `conscription` / "conscription"; `minister_sciences` / "minister_sciences"
- payload: none
- label: 1 template(s): "censure" -> `<id>` x8
- prompt: 1 template(s): "repeal which law" x3
- order: ids ascending (string) x3, contiguous x3
- subtypes: `repeal_pick_repeal` x3
- sources: bench-v1 1, bench-v3 1, te 1
- built in: not found as a literal (built from a variable or in host/bot code)

#### `cavalry_ship`

- offered 8 times in 3 decisions; per decision 2..3 options in the whole choice
- ids: `{carrier,carrier2,cruiser,dreadnought,ghost_flagship}` x8
  examples: `carrier` / "treat a carrier as the Nomad flagship"; `dreadnought` / "treat a dreadnought as the Nomad flagship"; `cruiser` / "treat a cruiser as the Nomad flagship"
- payload: `unit`: str 8; key sets: {unit} x8
- label: 1 template(s): "treat a carrier as the Nomad flagship" -> `treat a <id> as the Nomad flagship` x8
- prompt: 1 template(s): "The Cavalry: which non-fighter ship takes the Nomad flagship's abilities" x3
- order: ids ascending (string) x3, contiguous x3
- subtypes: `cavalry_ship` x3
- sources: golden20 3
- built in: `factions/nomad_agents.rs`

#### `produce_hit`

- offered 7 times in 7 decisions; per decision 2..2 options in the whole choice
- ids: `{produce_hit}` x7
  examples: `produce_hit` / "produce 1 hit and assign it to 1 of your opponent's ships"
- payload: none
- label: 1 template(s): "produce 1 hit and assign it to 1 of your opponent's ships" -> `produce # hit and assign it to # of your opponent's ships` x7
- prompt: 1 template(s): "Dimensional Splicer: produce 1 hit against your opponent's ships" x7
- order: never more than one of this kind in a decision
- subtypes: `dimensional_splicer` x7
- sources: bench-v1 1, bench-v3 4, bench-v4 1, te 1
- built in: `factions/ghost.rs`

#### `mahact_hero`

- offered 7 times in 3 decisions; per decision 2..3 options in the whole choice
- ids: `int 21..74`|`int 6..76` x7
  examples: `21|06` / "move the fleet in 21 to 06"; `52|26` / "move the fleet in 52 to 26"; `35|76` / "move the fleet in 35 to 76"
- payload: none
- label: 1 template(s): "move the fleet in 21 to 06" -> `move the fleet in # to #` x7
- prompt: 1 template(s): "Airo Shir Aur: which fleet moves where" x3
- order: ids ascending (string) x3, contiguous x3
- subtypes: `mahact_hero_move` x3
- sources: bench 1, golden20 1, te 1
- built in: `factions/mahact_units.rs`

#### `watcher`

- offered 7 times in 3 decisions; per decision 2..3 options in the whole choice
- ids: `{watcher}`|`int 22..74`|`{atlas,lodor,newalbion,tarmann,thedark,vegamajor}` x7
  examples: `watcher|26|lodor` / "remove the Watcher in system 26"; `watcher|74|vegamajor` / "remove the Watcher in system 74"; `watcher|22|tarmann` / "remove the Watcher in system 22"
- payload: `system`: str 7; key sets: {system} x7
- label: 1 template(s): "remove the Watcher in system 26" -> `remove the Watcher in system #` x7
- prompt: 3 template(s): "Watcher: which mech cancels d's action card" x1; "Watcher: which mech cancels c's action card" x1; "Watcher: which mech cancels f's action card" x1
- order: ids ascending (string) x3, contiguous x3
- subtypes: `watcher_remove` x3
- sources: golden20 3
- built in: `factions/empyrean_units.rs`

#### `awaken`

- offered 6 times in 3 decisions; per decision 3..3 options in the whole choice
- ids: `{mech,pds}` x6
  examples: `pds` / "replace the sleeper token on lirtaiv with 1 PDS"; `mech` / "replace the sleeper token on lirtaiv with 1 mech and 1 infantry (Hecatoncheires)"
- payload: none
- label: 4 template(s): "replace the sleeper token on lirtaiv with 1 PDS" -> `replace the sleeper token on lirtaiv with # PDS` x2; "replace the sleeper token on lirtaiv with 1 mech and 1 infantry (Hecatoncheires)" -> `replace the sleeper token on lirtaiv with # <id> and # infantry (Hecatoncheires)` x2; "replace the sleeper token on torkan with 1 PDS" -> `replace the sleeper token on torkan with # PDS` x1
- prompt: 2 template(s): "Awaken: the sleeper token on lirtaiv" x2; "Awaken: the sleeper token on torkan" x1
- order: ids descending x3, contiguous x3
- subtypes: `awaken_replace` x3
- sources: golden20 2, te 1
- built in: `factions/titans.rs`

#### `salvage`

- offered 4 times in 4 decisions; per decision 2..2 options in the whole choice
- ids: `{carrier,destroyer}` x4
  examples: `destroyer` / "produce 1 destroyer for 1 resources"; `carrier` / "produce 1 carrier for 3 resources"
- payload: `cost`: int 4; key sets: {cost} x4
- label: 1 template(s): "produce 1 destroyer for 1 resources" -> `produce # <id> for # resources` x4
- prompt: 1 template(s): "Salvage Operations: produce 1 ship of a type destroyed in the combat" x4
- order: never more than one of this kind in a decision
- subtypes: `salvage_produce` x4
- sources: bench-v4 1, golden20 3
- built in: `factions/mentak.rs`

#### `extra_die`

- offered 4 times in 1 decisions; per decision 4..4 options in the whole choice
- ids: `{unit}`|`int 0..3` x4
  examples: `unit|0` / "cabal_flagship rolls 1 additional die"; `unit|1` / "dreadnought rolls 1 additional die"; `unit|2` / "cabal_flagship rolls 1 additional die"
- payload: none
- label: 2 template(s): "cabal_flagship rolls 1 additional die" -> `cabal_flagship rolls # additional die` x2; "dreadnought rolls 1 additional die" -> `dreadnought rolls # additional die` x2
- prompt: 1 template(s): "ambuscade: choose a unit to roll 1 additional die" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `extra_die_unit` x1
- sources: golden20 1
- built in: `factions/argent.rs`

#### `star_forge`

- offered 4 times in 3 decisions; per decision 2..3 options in the whole choice
- ids: `int 20..69`|`{destroyer,fighters}` x4
  examples: `69|fighters` / "place 2 fighters in 69"; `69|destroyer` / "place 1 destroyer in 69"; `27|fighters` / "place 2 fighters in 27"
- payload: `system`: str 4; key sets: {system} x4
- label: 1 template(s): "place 2 fighters in 69" -> `place # <id> in #` x4
- prompt: 1 template(s): "Star Forge: place which units, and where" x3
- order: ids descending x1, contiguous x1
- subtypes: `star_forge` x3
- sources: golden20 2, te 1
- built in: `factions/muaat.rs`

#### `production_discount`

- offered 3 times in 3 decisions; per decision 2..2 options in the whole choice
- ids: `{exhaust}` x3
  examples: `exhaust` / "exhaust to reduce the combined cost by 2"
- payload: `discount_offered`: int 3, `technology`: str 3; key sets: {discount_offered,technology} x3
- label: 1 template(s): "exhaust to reduce the combined cost by 2" -> `<id> to reduce the combined cost by #` x3
- prompt: 1 template(s): "AI Development Algorithm: exhaust to reduce this use's combined cost by 2" x3
- order: never more than one of this kind in a decision
- subtypes: `exhaust_for_production_discount` x3
- sources: golden20 3
- built in: `technology.rs`

#### `cruiser`

- offered 3 times in 1 decisions; per decision 3..3 options in the whole choice
- ids: `int 0..2` x3
  examples: `0` / "cruiser"; `1` / "cruiser"; `2` / "cruiser"
- payload: `system`: str 3; key sets: {system} x3
- label: 1 template(s): "cruiser" -> `cruiser` x3
- prompt: 1 template(s): "Upgrade: which cruiser becomes a dreadnought" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `upgrade_pick_cruiser` x1
- sources: golden20 1
- built in: `action_cards.rs`, `faction_effects_tests/d1.rs`, `faction_effects_tests/d2.rs`, `faction_effects_tests/d3.rs`, `faction_effects_tests/d4.rs`, `faction_effects_tests/d5.rs` (+10)

#### `strategy card`

- offered 3 times in 1 decisions; per decision 3..3 options in the whole choice
- ids: `{pok3politics,pok7technology,pok8imperial}` x3
  examples: `pok7technology` / "d's Technology"; `pok3politics` / "Politics"; `pok8imperial` / "Imperial"
- payload: none
- label: 3 template(s): "d's Technology" -> `d's Technology` x1; "Politics" -> `Politics` x1; "Imperial" -> `Imperial` x1
- prompt: 1 template(s): "Strategize: which card's secondary ability to perform" x1
- order: insertion order x1, contiguous x1
- subtypes: `strategize2_pick_strategy card` x1
- sources: te 1
- built in: not found as a literal (built from a variable or in host/bot code)

#### `rescue_from`

- offered 3 times in 1 decisions; per decision 3..3 options in the whole choice
- ids: `int 37..69` x3
  examples: `37` / "a ship from 37"; `53` / "a ship from 53"; `69` / "a ship from 69"
- payload: `system`: str 3; key sets: {system} x3
- label: 1 template(s): "a ship from 37" -> `a ship from #` x3
- prompt: 1 template(s): "Rescue: bring a ship from where" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `rescue_pick_rescue_from` x1
- sources: te 1
- built in: `action_cards.rs`

#### `nomadbt`

- offered 3 times in 1 decisions; per decision 4..4 options in the whole choice
- ids: `{pair}`|`int 0..2` x3
  examples: `pair|0` / "exhaust nomadagentartuno to ready e's hacanagent"; `pair|1` / "exhaust nomadagentmercer to ready e's hacanagent"; `pair|2` / "exhaust nomadagentthundarian to ready e's hacanagent"
- payload: none
- label: 3 template(s): "exhaust nomadagentartuno to ready e's hacanagent" -> `exhaust nomadagentartuno to ready e's hacanagent` x1; "exhaust nomadagentmercer to ready e's hacanagent" -> `exhaust nomadagentmercer to ready e's hacanagent` x1; "exhaust nomadagentthundarian to ready e's hacanagent" -> `exhaust nomadagentthundarian to ready e's hacanagent` x1
- prompt: 1 template(s): "Thunder's Paradox: exhaust which agent to ready which" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `thunders_paradox` x1
- sources: templates 1
- built in: `faction_effects_tests/d4.rs`

#### `produce_unit`

- offered 2 times in 1 decisions; per decision 3..3 options in the whole choice
- ids: `{opt_carrier,opt_infantry}` x2
  examples: `opt_carrier` / "Produce Carrier (Cost: 3)"; `opt_infantry` / "Produce 2 Infantry (Cost: 1)"
- payload: `cost`: int 2, `unit`: str 2; key sets: {cost,unit} x2
- label: 2 template(s): "Produce Carrier (Cost: 3)" -> `Produce Carrier (Cost: #)` x1; "Produce 2 Infantry (Cost: 1)" -> `Produce # Infantry (Cost: #)` x1
- prompt: 1 template(s): "Choose units to produce in Mecatol Rex" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `produce_units` x1
- sources: fixtures 1
- built in: `production.rs`

#### `leader_hacanhero_free_production`

- offered 2 times in 2 decisions; per decision 2..2 options in the whole choice
- ids: `{use}` x2
  examples: `use` / "purge Harrugh Gefhara: every unit costs 0"
- payload: none
- label: 1 template(s): "purge Harrugh Gefhara: every unit costs 0" -> `purge Harrugh Gefhara: every unit costs #` x2
- prompt: 1 template(s): "Harrugh Gefhara: make this use of PRODUCTION free" x2
- order: never more than one of this kind in a decision
- subtypes: `leader_hacanhero_free_production` x2
- sources: golden20 1, te 1
- built in: `leaders.rs`

#### `leader_l1z1xagent_copy_planet`

- offered 2 times in 1 decisions; per decision 2..2 options in the whole choice
- ids: `{retillion,shalloq}` x2
  examples: `retillion` / "replace infantry on Retillion with a mech"; `shalloq` / "replace infantry on Shalloq with a mech"
- payload: `planet`: str 2, `system`: str 2; key sets: {planet,system} x2
- label: 2 template(s): "replace infantry on Retillion with a mech" -> `replace infantry on Retillion with a mech` x1; "replace infantry on Shalloq with a mech" -> `replace infantry on Shalloq with a mech` x1
- prompt: 1 template(s): "I48S (Ssruu): replace which infantry in the active system?" x1
- order: ids ascending (string) x1, contiguous x1
- subtypes: `leader_l1z1xagent_copy_planet` x1
- sources: golden20 1
- built in: `leaders.rs`

#### `answer`

- offered 2 times in 1 decisions; per decision 2..2 options in the whole choice
- ids: `{no,yes}` x2
  examples: `yes` / "accept"; `no` / "refuse"
- payload: none
- label: 2 template(s): "accept" -> `accept` x1; "refuse" -> `refuse` x1
- prompt: 1 template(s): "Exchange Program: accept the exchange?" x1
- order: ids descending x1, contiguous x1
- subtypes: `exchange_program_answer` x1
- sources: te 1
- built in: `action_cards.rs`

#### `destroy_own_ship`

- offered 1 times in 1 decisions; per decision 2..2 options in the whole choice
- ids: `{destroyer}` x1
  examples: `destroyer` / "destroy 1 destroyer to produce 1 hit"
- payload: none
- label: 1 template(s): "destroy 1 destroyer to produce 1 hit" -> `destroy # <id> to produce # hit` x1
- prompt: 1 template(s): "Devotion: destroy a cruiser or destroyer to produce 1 hit" x1
- order: never more than one of this kind in a decision
- subtypes: `devotion_sacrifice` x1
- sources: bench-v2 1
- built in: `factions/yin.rs`

#### `production`

- offered 1 times in 1 decisions; per decision 2..2 options in the whole choice
- ids: `int 11..11` x1
  examples: `11` / "produce one unit in 11"
- payload: `system`: str 1, `technology`: str 1; key sets: {system,technology} x1
- label: 1 template(s): "produce one unit in 11" -> `produce one unit in #` x1
- prompt: 1 template(s): "Chaos Mapping" x1
- order: never more than one of this kind in a decision
- subtypes: `chaos_mapping_choose_system` x1
- sources: bench-v4 1
- built in: `technology.rs`

#### `give_note`

- offered 1 times in 1 decisions; per decision 1..1 options in the whole choice
- ids: `{ta}`:`{mentak}` x1
  examples: `ta:mentak` / "ta:mentak"
- payload: none
- label: 1 template(s): "ta:mentak" -> `<id>:<id>` x1
- prompt: 1 template(s): "Give d 1 promissory note from your hand" x1
- order: never more than one of this kind in a decision
- subtypes: `commander_give_note` x1
- sources: golden20 1
- built in: `factions/mentak.rs`

### Payload keys by type (all kinds)

A key whose values are `float` anywhere, or are `int` in one kind and `float`/`str` in another, needs its number type pinned by the family.

| Key | Types (kinds) |
|---|---|
| `action_card` | str: `offer` |
| `alias` | str: `offer` |
| `available_resources` | int: `produce` |
| `bio_stims` | bool: `ready_technology` |
| `capacity` | int: `move` |
| `capacity_excess_after` | int: `place`, `produce` |
| `capacity_free_after` | int: `place`, `produce` |
| `capacity_needed` | int: `produce` |
| `capacity_provided` | int: `produce` |
| `capacity_remaining` | int: `load` |
| `capacity_role` | str: `produce` |
| `capacity_used` | int: `place`, `produce` |
| `card` | str: `ability`, `action_card` |
| `card_name` | str: `ability`, `action_card` |
| `card_names` | array: `ability` |
| `colour` | str: `starting_technology` |
| `cost` | int: `indoctrination`, `produce`, `produce_unit`, `research`, `salvage` |
| `cost_tokens` | int: `research` |
| `count` | int: `move`, `place`, `place_unit`, `produce` |
| `credit` | int: `produce` |
| `credit_used` | int: `produce` |
| `current_votes` | int: `vote` |
| `damaged` | bool: `casualty`, `commit`, `load`, `move` |
| `destination` | str: `place`, `produce` |
| `destination_system` | str: `transit` |
| `discount` | int: `produce` |
| `discount_offered` | int: `production_agent`, `production_discount` |
| `event` | str: `ability` |
| `exchange` | bool: `produce` |
| `face` | int: `reroll_die` |
| `fleet_excess_after` | int: `place`, `produce` |
| `fleet_headroom_after` | int: `place`, `produce` |
| `fleet_ships` | int: `produce` |
| `free_this_use` | bool: `produce` |
| `from` | str: `move` |
| `from_planet` | str: `commit` |
| `from_system` | str: `commit` |
| `galvanized` | bool: `load` |
| `gift` | bool: `offer` |
| `grants_breakthrough` | bool: `component` |
| `gravity_drive` | bool: `move` |
| `ground_available` | int: `decline` |
| `hits_on` | int: `reroll_die` |
| `integrated_economy` | bool: `produce` |
| `kind` | str: `pay` |
| `leader` | str: `production_agent` |
| `loaded_fighters` | int: `decline`, `load` |
| `loaded_ground` | int: `decline`, `load` |
| `moves_to` | str: `offer` |
| `net` | float: `offer`, `transaction` |
| `non_fighters_first` | bool: `decline` |
| `note` | str: `offer` |
| `opportunity_cost` | float: `component` |
| `orbital_drop_deploy` | bool: `produce` |
| `origin` | str: `move` |
| `owed` | int: `pay`, `produce` |
| `owners` | array: `starting_technology` |
| `pickup_system` | str: `load` |
| `placed` | int: `place`, `produce` |
| `placement_pending` | int: `produce` |
| `planet` | str: `ability`, `annex`, `build`, `commit`, `ingress_system`, `leader_l1z1xagent_copy_planet`, `legendary`, `offer` (+11) |
| `prerequisites` | int: `starting_technology` |
| `printed_cost` | int: `produce` |
| `production_spent` | int: `produce` |
| `promissory` | str: `offer` |
| `received_planet` | str: `offer` |
| `received_promissory` | str: `offer` |
| `slice` | str: `component` |
| `sling_relay` | bool: `place`, `produce` |
| `source` | null: `load`; str: `load`, `pay`, `transit` |
| `source_system` | str: `transit` |
| `space_placement` | str: `produce` |
| `system` | str: `ability`, `annex`, `build`, `creuss_token`, `cruiser`, `decline`, `infantry`, `ingress_system` (+24) |
| `technology` | str: `ability`, `production`, `production_discount`, `ready`, `ready_technology` |
| `technology_colour` | str: `ingress_system` |
| `their_net` | float: `offer` |
| `to` | str: `move` |
| `trade_goods` | int: `vote_trade_goods` |
| `unit` | str: `build`, `casualty`, `cavalry_ship`, `commit`, `decline`, `legendary`, `load`, `mercer_place` (+8) |
| `votes` | int: `vote_predictive` |
| `worth` | int: `pay` |
| `yield` | int: `produce` |

Prompt templates seen at least 10 times: 274, covering 98.0% of the decisions; the long tail (1177 templates) is card, planet and faction names inside prose.

### Proposed typed areas (each kind assigned to one `Ask` area)

A kind never changes area, so one `choice/<area>.rs` owns all of its options. `strategy card` and `promissory note` (kinds with a space) are separate, older kinds from `strategy_card` / `promissory_note`: byte-identical output keeps both spellings.

| Area | Kinds (options offered) |
|---|---|
| generic (decline, done, yes/no, numbers) | `decline` (53315), `end_turn` (8184), `count` (48), `answer` (2) |
| strategy phase and action menu | `redistribute` (48128), `pool` (20157), `action` (17909), `strategy` (16039), `strategy_card` (14564), `strategy card` (3) |
| tactical action (activate, move, load, transit) | `activate` (166872), `load` (20748), `move` (14863), `system` (2292), `transit` (1161), `ship` (994) |
| combat and invasion | `commit` (12757), `retreat` (2661), `remove` (1370), `commit_infantry` (1359), `ground_casualty` (953), `casualty` (687), `sustain` (671), `retreat_to` (541), `infantry` (248), `ambush_roll` (73), `reroll_die` (53), `unit` (47), `produce_hit` (7), `extra_die` (4), `cruiser` (3), `rescue_from` (3), `destroy_own_ship` (1) |
| production, payment and research | `pay` (40967), `produce` (30187), `build` (11753), `research` (9446), `ready` (5139), `technology` (672), `place_unit` (407), `starting_technology` (364), `place` (267), `ready_technology` (82), `production_agent` (21), `research_waiver_payment` (18), `research_waiver` (10), `salvage` (4), `production_discount` (3), `produce_unit` (2), `production` (1) |
| transactions, trade and economy | `open_transaction` (41224), `offer` (29723), `transaction` (2842), `replenish` (1088), `promissory_note` (476), `pillage` (218), `promissory note` (134), `economy` (100), `give_note` (1) |
| agenda and voting | `vote_planet` (20943), `vote` (8253), `agenda` (856), `speaker` (793), `player` (702), `prediction` (668), `tiebreak` (320), `recombine` (207), `tribute` (68), `law` (53), `follower` (46), `elect` (37), `vote_predictive` (23), `vote_trade_goods` (19), `repeal` (8) |
| objectives, relics, exploration, cards | `component` (11470), `ability` (9153), `planet` (3452), `legendary` (3110), `discard` (1862), `explore` (1604), `score` (1481), `own_hand_card` (485), `objective` (232), `action_card` (213), `component_action` (133), `custodians` (47), `fragment` (21), `take_revealed_card` (17), `relic` (9) |
| leaders | `leader_ssruu_round_agent_unit` (351), `leader_hacanagent_branch` (194), `leader` (83), `agent` (75), `leader_jolnarhero_swap` (75), `breakthrough` (45), `leader_xxchaagent_infantry` (44), `hero` (20), `commander` (11), `leader_hacanhero_free_production` (2), `leader_l1z1xagent_copy_planet` (2) |
| faction mechanics (one family per faction ability) | `creuss_token` (1867), `return` (1517), `stillness` (421), `mercer_take` (317), `sleeper` (268), `skilled_retreat` (196), `mitosis` (139), `reparations_exhaust` (137), `nekro_malleon` (95), `indoctrination` (93), `silence` (89), `self_assembly` (70), `flock_migration_ship` (68), `stymie` (62), `recall` (54), `annex` (50), `ingress_system` (48), `reparations_ready` (36), `mercer_place` (34), `tether` (34), `scuttle` (26), `vortex` (24), `ingress_move` (24), `flock_migration_to` (22), `flock_migration_cargo` (16), `nekro_assimilate` (14), `letnev_commander_trade_good` (11), `scepter` (9), `cavalry_ship` (8), `mahact_hero` (7), `watcher` (7), `awaken` (6), `star_forge` (4), `nomadbt` (3) |

### Families by module (kinds whose literal is built in a file)

| Module (ti4-engine/src) | Option-builder sites | Kinds built there |
|---|---:|---|
| `production.rs` | 96 | `ability`, `decline`, `infantry`, `pay`, `place`, `place_unit`, `player`, `produce`, `produce_unit`, `system` |
| `strategy_cards.rs` | 61 | `activate`, `agenda`, `build`, `economy`, `leader`, `objective`, `planet`, `pool`, `ready`, `recall`, `replenish`, `research`, `research_waiver`, `research_waiver_payment`, `speaker`, `system`, `unit` |
| `invasion.rs` | 48 | `commit`, `custodians`, `decline`, `ground_casualty`, `planet`, `player`, `sustain`, `system`, `unit` |
| `action_cards.rs` | 37 | `answer`, `component`, `count`, `cruiser`, `discard`, `elect`, `infantry`, `move`, `own_hand_card`, `place_unit`, `planet`, `player`, `prediction`, `reparations_exhaust`, `reparations_ready`, `rescue_from`, `ship`, `silence`, `skilled_retreat`, `system`, `take_revealed_card` |
| `technology.rs` | 27 | `ability`, `component`, `decline`, `player`, `production`, `production_discount`, `ready`, `ready_technology`, `redistribute`, `self_assembly` |
| `legendary.rs` | 21 | `legendary`, `technology` |
| `faction_abilities.rs` | 20 | `ability`, `annex`, `component`, `planet`, `player`, `system` |
| `leaders.rs` | 20 | `agent`, `commander`, `component`, `hero`, `infantry`, `leader_hacanagent_branch`, `leader_hacanhero_free_production`, `leader_jolnarhero_swap`, `leader_l1z1xagent_copy_planet`, `leader_xxchaagent_infantry`, `planet`, `remove`, `system` |
| `combat.rs` | 18 | `ability`, `casualty`, `letnev_commander_trade_good`, `player`, `reroll_die`, `sustain`, `system`, `unit` |
| `factions/naaz.rs` | 16 | `fragment`, `planet`, `player`, `pool`, `strategy_card`, `system`, `technology` |
| `choice.rs` | 15 | `decline`, `offer` |
| `factions/winnu.rs` | 13 | `follower`, `planet`, `player`, `ship`, `strategy_card`, `system`, `unit` |
| `factions/nekro_units.rs` | 13 | `component`, `discard`, `nekro_malleon`, `player`, `system`, `unit` |
| `vote.rs` | 13 | `law`, `tiebreak`, `vote`, `vote_planet`, `vote_trade_goods` |
| `relics.rs` | 12 | `action_card`, `component`, `planet`, `player`, `relic`, `technology` |
| `exploration.rs` | 11 | `explore`, `infantry`, `planet`, `player`, `technology` |
| `transit.rs` | 10 | `load`, `player` |
| `transactions.rs` | 10 | `action_card`, `fragment`, `offer`, `planet`, `transaction` |
| `thunders_edge.rs` | 10 | `component`, `discard`, `return`, `system` |
| `strategy.rs` | 9 | `action`, `decline`, `strategy` |
| `tactical.rs` | 9 | `activate`, `move`, `unit` |
| `factions/arborec.rs` | 9 | `infantry`, `mitosis`, `ship`, `stymie`, `system`, `unit` |
| `factions/mentak.rs` | 9 | `agent`, `ambush_roll`, `commander`, `component_action`, `cruiser`, `give_note`, `pillage`, `player`, `salvage`, `system`, `unit` |
| `factions/ralnel_cards.rs` | 9 | `planet`, `player`, `system` |
| `factions/yssaril.rs` | 9 | `breakthrough`, `commander`, `discard`, `hero`, `player`, `system` |
| `factions/crimson_cards.rs` | 8 | `player`, `ship`, `system`, `unit` |
| `factions/nomad_agents.rs` | 8 | `cavalry_ship`, `mercer_place`, `mercer_take`, `player`, `system`, `unit` |
| `factions/saar.rs` | 8 | `infantry`, `planet`, `player`, `ship`, `system` |
| `factions/deepwrought_cards.rs` | 8 | `technology` |
| `fracture.rs` | 8 | `ingress_system` |
| `factions/argent.rs` | 7 | `extra_die`, `flock_migration_cargo`, `flock_migration_ship`, `flock_migration_to`, `infantry`, `leader`, `player`, `system` |
| `factions/ghost.rs` | 7 | `creuss_token`, `player`, `produce_hit`, `system` |
| `factions/muaat.rs` | 7 | `place_unit`, `player`, `star_forge`, `system` |
| `factions/titans.rs` | 6 | `awaken`, `build`, `place`, `planet`, `sleeper`, `system` |
| `factions/firmament.rs` | 6 | `breakthrough`, `infantry`, `place`, `planet`, `player`, `system`, `technology`, `unit` |
| `factions/sardakk.rs` | 6 | `breakthrough`, `infantry`, `leader`, `planet`, `player`, `ship`, `system`, `unit` |
| `factions/mahact_units.rs` | 6 | `economy`, `mahact_hero`, `player`, `unit` |
| `factions/keleres.rs` | 6 | `component_action` |
| `factions/cabal.rs` | 5 | `breakthrough`, `ingress_move`, `stillness`, `system`, `unit`, `vortex` |
| `factions/obsidian.rs` | 5 | `infantry`, `ingress_move`, `player`, `ship`, `system`, `unit` |
| `factions/yin.rs` | 5 | `breakthrough`, `commander`, `commit_infantry`, `destroy_own_ship`, `indoctrination`, `infantry`, `objective`, `planet`, `player`, `system`, `unit` |
| `reactions.rs` | 5 | `action_card`, `infantry`, `leader`, `player`, `technology` |
| `laws.rs` | 5 | `discard`, `law` |
| `factions/borrowed_commanders.rs` | 4 | `component`, `economy`, `infantry`, `planet`, `player`, `retreat`, `system` |
| `factions/crimson.rs` | 4 | `player`, `system` |
| `factions/deepwrought.rs` | 4 | `count`, `infantry`, `planet`, `player`, `system`, `technology` |
| `factions/ralnel.rs` | 4 | `planet`, `player`, `ship`, `system` |
| `magen_defense.rs` | 4 | `system` |
| `agenda_effects.rs` | 4 | `elect`, `infantry`, `scuttle` |
| `technology_effects_tests/g2.rs` | 4 | `cruiser`, `infantry`, `player` |
| `factions/empyrean_units.rs` | 3 | `component_action`, `planet`, `player`, `system`, `unit`, `watcher` |
| `game.rs` | 3 | `count`, `player`, `system` |
| `technology_effects_tests/g4.rs` | 3 | `component`, `cruiser`, `infantry`, `player`, `system`, `unit` |
| `objectives.rs` | 3 | `score` |
| `factions/firmament_plots.rs` | 3 | `infantry`, `planet`, `player`, `technology` |
| `factions/mahact.rs` | 3 | `player`, `scepter` |
| `factions/nekro.rs` | 3 | `nekro_assimilate`, `player` |
| `factions/titans_leaders.rs` | 3 | `player`, `sleeper` |
| `entropic_scars.rs` | 3 | `technology` |
| `starting_tech.rs` | 3 | `starting_technology` |
| `tokens.rs` | 2 | `pool`, `redistribute` |
| `factions/borrowed_round_agents.rs` | 2 | `leader_ssruu_round_agent_unit`, `planet`, `system` |
| `factions/naalu.rs` | 2 | `player`, `promissory_note`, `system` |
| `factions/deepwrought_research.rs` | 2 | `leader`, `planet` |
| `factions/empyrean.rs` | 2 | `tether` |
| `draft.rs` | 1 | `strategy_card` |
| `secondary_preview_tests.rs` | 1 | `action` |
| `fleet.rs` | 1 | `remove` |
| `factions/bastion.rs` | 1 | `infantry`, `system` |
| `factions/keleres_units.rs` | 1 | `player`, `system` |
| `technology_effects_tests/g1.rs` | 1 | `cruiser`, `player`, `system` |
| `supply.rs` | 1 | `player` |
| `secrets.rs` | 1 | `return` |
| `faction_effects_tests/d1.rs` | 1 | `cruiser`, `infantry` |
| `faction_effects_tests/d4.rs` | 0 | `cruiser`, `decline`, `infantry`, `nomadbt` |
| `faction_effects_tests/f3.rs` | 0 | `cruiser`, `decline` |
| `timing.rs` | 0 | `ability`, `breakthrough`, `leader`, `technology` |
| `factions/bastion_units.rs` | 0 | `infantry`, `planet`, `player`, `system`, `unit` |
| `factions/borrowed_commanders_b.rs` | 0 | `system` |
| `technology_effects_tests/g3.rs` | 0 | `cruiser`, `player`, `system`, `unit` |
| `reaction_preview.rs` | 0 | `agenda` |
| `trigger.rs` | 0 | `agenda`, `player` |
| `breakthroughs.rs` | 0 | `infantry` |
| `faction_effects_tests/d2.rs` | 0 | `cruiser`, `infantry` |
| `faction_effects_tests/d3.rs` | 0 | `cruiser`, `infantry` |
| `faction_effects_tests/d5.rs` | 0 | `cruiser`, `infantry` |
| `faction_effects_tests/d6.rs` | 0 | `infantry` |
| `faction_effects_tests/f1.rs` | 0 | `cruiser`, `infantry` |
| `opening.rs` | 0 | `infantry` |
| `wiring.rs` | 0 | `infantry` |
| `infantry_ii.rs` | 0 | `unit` |
| `promissory.rs` | 0 | `commander` |
| `faction_effects_tests/f4.rs` | 0 | `cruiser` |
| `faction_effects_tests/v16.rs` | 0 | `cruiser` |
| `nexus_reach_tests.rs` | 0 | `cruiser` |

### Prompt templates by frequency (top 60)

| Decisions | Template (digits folded) |
|---:|---|
| 9433 | action phase |
| 9394 | movement |
| 8167 | end your turn |
| 7538 | after <EVENT> |
| 6706 | gain a command token into which pool |
| 5601 | commit ground forces in # |
| 5498 | load carrier (# free) |
| 4923 | pay # more resources |
| 4732 | produce in # (# left) |
| 4412 | activate a system |
| 4180 | pay # more influence |
| 2673 | spend # influence for a command token |
| 2644 | exhaust a planet to vote <seat> |
| 2611 | choose a strategy card |
| 2611 | load <id> (# free) |
| 1518 | vote for which outcome |
| 1384 | redistribute your command tokens |
| 1331 | announce a retreat from # |
| 1184 | when <EVENT> |
| 1066 | spend a strategy token to produce at home |
| 1039 | score an objective |
| 1031 | ready which planet |
| 979 | exhaust a planet to vote for |
| 920 | place a structure |
| 834 | use a legendary planet ability |
| 793 | spend a strategy token to place a structure |
| 783 | spend a strategy token to draw a secret objective |
| 750 | remove a unit: over capacity in # |
| 666 | spend a strategy token to replenish commodities |
| 637 | research a technology |
| 606 | cancel a hit at # |
| 571 | spend a strategy token to ready two planets |
| 566 | load dreadnought (# free) |
| 556 | Predictive Intelligence: redistribute command tokens |
| 544 | exhaust a planet to vote against |
| 523 | spend a strategy token to draw two action cards |
| 489 | return a secret objective to the deck |
| 461 | spend # more influence for a command token |
| 357 | assign a hit |
| 347 | Warfare: redistribute your command tokens before the tactical action |
| 347 | Warfare: a tactical action without a command token |
| 336 | Warfare: redistribute your command tokens after the tactical action |
| 323 | spend a strategy token and # resources to research |
| 311 | let another player replenish commodities |
| 309 | remove # infantry from this planet to resolve the exploration card |
| 302 | Construction: a structure or a production |
| 247 | choose a system for Diplomacy |
| 237 | over the hand limit — discard one of # |
| 232 | Local Fabricators |
| 219 | retreat to which system |
| 218 | Imperial Arms Vault |
| 218 | transaction with sol |
| 214 | who becomes speaker |
| 190 | remove a unit: over fleet supply in # |
| 185 | transaction with <id> |
| 183 | Bio-Stims |
| 174 | score a public objective with Imperial |
| 173 | transaction with yin |
| 163 | Functioning Base |
| 161 | place <id> where |

### Option-building site inventory (ti4-engine, outside `#[cfg(test)]`)

Sites = `ChoiceOption::new|labelled` calls + `.with("..")` payload setters + `offer_*` detail builders + `Choice` constructors (`Choice::new`/`Choice {`). Counted per file.

| File | option ctors | payload setters | offer_* | Choice ctors |
|---|---:|---:|---:|---:|
| `production.rs` | 15 | 76 | 5 | 10 |
| `strategy_cards.rs` | 29 | 4 | 28 | 26 |
| `invasion.rs` | 18 | 11 | 19 | 12 |
| `action_cards.rs` | 19 | 12 | 6 | 17 |
| `technology.rs` | 11 | 16 | 0 | 7 |
| `legendary.rs` | 17 | 4 | 0 | 11 |
| `faction_abilities.rs` | 10 | 10 | 0 | 6 |
| `leaders.rs` | 16 | 3 | 1 | 9 |
| `combat.rs` | 9 | 7 | 2 | 5 |
| `factions/naaz.rs` | 13 | 0 | 3 | 4 |
| `choice.rs` | 4 | 1 | 10 | 3 |
| `vote.rs` | 8 | 5 | 0 | 7 |
| `factions/nekro_units.rs` | 10 | 3 | 0 | 1 |
| `factions/winnu.rs` | 10 | 3 | 0 | 7 |
| `diplomacy/window.rs` | 9 | 4 | 0 | 2 |
| `relics.rs` | 9 | 2 | 1 | 7 |
| `exploration.rs` | 4 | 1 | 6 | 2 |
| `transit.rs` | 2 | 8 | 0 | 1 |
| `thunders_edge.rs` | 6 | 4 | 0 | 5 |
| `transactions.rs` | 5 | 0 | 5 | 2 |
| `strategy.rs` | 9 | 0 | 0 | 7 |
| `tactical.rs` | 3 | 6 | 0 | 5 |
| `factions/mentak.rs` | 8 | 1 | 0 | 1 |
| `factions/ralnel_cards.rs` | 9 | 0 | 0 | 0 |
| `factions/arborec.rs` | 9 | 0 | 0 | 8 |
| `factions/yssaril.rs` | 9 | 0 | 0 | 5 |
| `fracture.rs` | 2 | 6 | 0 | 2 |
| `factions/deepwrought_cards.rs` | 8 | 0 | 0 | 0 |
| `factions/saar.rs` | 8 | 0 | 0 | 1 |
| `factions/nomad_agents.rs` | 3 | 5 | 0 | 1 |
| `factions/crimson_cards.rs` | 7 | 0 | 1 | 0 |
| `diplomacy/builder.rs` | 7 | 1 | 0 | 0 |
| `factions/ghost.rs` | 5 | 2 | 0 | 1 |
| `factions/muaat.rs` | 6 | 1 | 0 | 1 |
| `factions/argent.rs` | 7 | 0 | 0 | 4 |
| `factions/firmament.rs` | 6 | 0 | 0 | 1 |
| `factions/keleres.rs` | 5 | 0 | 1 | 2 |
| `factions/titans.rs` | 6 | 0 | 0 | 1 |
| `factions/mahact_units.rs` | 5 | 0 | 1 | 4 |
| `factions/sardakk.rs` | 5 | 1 | 0 | 4 |
| `laws.rs` | 1 | 0 | 4 | 1 |
| `reactions.rs` | 3 | 2 | 0 | 3 |
| `factions/cabal.rs` | 5 | 0 | 0 | 3 |
| `factions/yin.rs` | 4 | 1 | 0 | 1 |
| `factions/obsidian.rs` | 5 | 0 | 0 | 1 |
| `agenda_effects.rs` | 3 | 1 | 0 | 3 |
| `magen_defense.rs` | 1 | 3 | 0 | 1 |
| `factions/deepwrought.rs` | 4 | 0 | 0 | 2 |
| `factions/borrowed_commanders.rs` | 4 | 0 | 0 | 2 |
| `factions/ralnel.rs` | 4 | 0 | 0 | 1 |
| `factions/crimson.rs` | 4 | 0 | 0 | 2 |
| `diplomacy/candidates.rs` | 2 | 0 | 2 | 0 |
| `technology_effects_tests/g2.rs` | 0 | 0 | 4 | 0 |
| `objectives.rs` | 3 | 0 | 0 | 2 |
| `entropic_scars.rs` | 1 | 2 | 0 | 1 |
| `starting_tech.rs` | 1 | 2 | 0 | 1 |
| `game.rs` | 1 | 0 | 2 | 1 |
| `factions/mahact.rs` | 3 | 0 | 0 | 1 |
| `factions/empyrean_units.rs` | 2 | 1 | 0 | 1 |
| `factions/firmament_plots.rs` | 3 | 0 | 0 | 0 |
| `factions/titans_leaders.rs` | 3 | 0 | 0 | 1 |
| `factions/nekro.rs` | 3 | 0 | 0 | 1 |
| `technology_effects_tests/g4.rs` | 1 | 0 | 2 | 0 |
| `tokens.rs` | 2 | 0 | 0 | 3 |
| `factions/deepwrought_research.rs` | 2 | 0 | 0 | 2 |
| `factions/naalu.rs` | 2 | 0 | 0 | 2 |
| `factions/borrowed_round_agents.rs` | 1 | 0 | 1 | 1 |
| `factions/empyrean.rs` | 2 | 0 | 0 | 2 |
| `supply.rs` | 0 | 0 | 1 | 0 |
| `draft.rs` | 1 | 0 | 0 | 1 |
| `fleet.rs` | 1 | 0 | 0 | 1 |
| `secrets.rs` | 1 | 0 | 0 | 1 |
| `secondary_preview_tests.rs` | 1 | 0 | 0 | 0 |
| `factions/bastion.rs` | 1 | 0 | 0 | 1 |
| `factions/firmament_flip.rs` | 1 | 0 | 0 | 0 |
| `factions/keleres_units.rs` | 1 | 0 | 0 | 1 |
| `technology_effects_tests/g1.rs` | 0 | 0 | 1 | 0 |
| `faction_effects_tests/d1.rs` | 0 | 0 | 1 | 1 |
| **total** | **428** | **209** | **107** | **237** |

Option-builder sites (ctors + setters + offer_*): **744**.
