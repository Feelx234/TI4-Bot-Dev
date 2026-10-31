# The lent census (package W0 of `docs/todo/lent-bodies.md`)

Branch `core-w`, from `core-integration-5` @ `670963aa`. What the dual run lends to the string engine, and why a window did not run in
the core: every bail and every gate of the dual windows names its reason, and this file counts them.

## How to run

```
# once: the dual runner (gate profile)
CARGO_BUILD_JOBS=2 cargo build --profile gate -p ti4-engine-core-bridge --bins
# the 20 golden games (profile-20) and the 19 games of the lent set, every area on `dual`, under the heavy lock
flock /root/.ti4-heavy.lock env TI4_NESTED_LOG=1 SHARDS=4 tools/engine_core/dual_sharded.sh \
    $CARGO_TARGET_DIR/gate/dual_run tools/engine_profile/profile-20.json 20 all=dual OUT/golden-shards > OUT/golden.txt
flock /root/.ti4-heavy.lock env TI4_NESTED_LOG=1 SHARDS=4 tools/engine_core/dual_sharded.sh \
    $CARGO_TARGET_DIR/gate/dual_run tools/rulebot/golden-lent.json 20 all=dual OUT/lent-shards > OUT/lent.txt
tools/engine_core/lent_census.py OUT/golden.txt     # window x reason x runs; exit 1 on a bail with no reason
tools/engine_core/lent_census.py OUT/lent.txt
```

`golden-lent.json` (`tools/rulebot/`) fixes the seats to the factions with most guards (Nomad, Naaz, Letnev, Yin, Mentak, Yssaril;
Argent, Jol-Nar, Sardakk, Mahact, Hacan, Titans; a 4-player table Nomad, Naaz, Letnev, Mentak). It is not a baseline file.
A reason prefixed `gate:` is a position the call site runs bare (no window opened, so it is not part of `bailed`).

## Silent bails named by W0

`dual::reasoned` (called by `run_dual_window`, `run_dual_ask`, the vote replay) counts `NO REASON RECORDED` when a core half returns
`None` without having counted a reason; the census tool fails on that line. The adapters name their reasons with `dual::Why`
(`x.why("...")?`) and `note_bail_here`. Silent before W0, named now: the production windows (`ProductionWindow::to_core`), the
begin/settle/answer/announce windows of the combat machine, the write adapters (exploration, leaders, relics, action cards, strategy
cards), the agenda effect window (no window for the agenda; the dice streams), the offer of a law discard. Tests:
`dual::tests::a_core_half_that_bails_always_leaves_a_reason`, `agenda_effects::tests::an_agenda_without_a_core_window_bails_with_a_reason`.
The last census that still showed `NO REASON RECORDED` (2 runs of `agenda_effects::resolve_with`, in both the golden and the lent games) was taken before the agenda
effect window named its two bails; a census at a later commit must show none (`lent_census.py` exits 0).

## Numbers

Before: the dual runner of the first W0 commit (reasons only, no port yet). After: commit `3de90cbe` of `core-w`.

* golden (20 games, profile-20): before window calls 542406, bailed 5565 (1.03%), bails with no reason 878; after window calls 541822, bailed 4931 (0.91%), bails with no reason 2.
* lent set (19 games): before window calls 634252, bailed 5953 (0.94%), bails with no reason 1066; after window calls 634212, bailed 5335 (0.84%), bails with no reason 2.

### golden games, window by window

| window | calls before | bailed before | calls after | bailed after | reasons after (runs) |
|---|---:|---:|---:|---:|---|
| `ProductionWindow::step` | 3336 | 479 | 3336 | 53 | 53 a production by an ability (the window of a card or an ability, not a production use) |
| `action_cards::announce` | 261 | 5 | 261 | 5 | 3 the typed state could not be lent (a value the codecs refuse)<br>2 a card the core announce window does not play<br>2 a technology gained at a table the research window does not carry<br>1 the research hooks of the string and the core disagree (a hook the core does not carry) |
| `agenda_effects::resolve_with` | 128 | 3 | 128 | 3 | 2 NO REASON RECORDED<br>1 an agenda effect the core window does not carry (a rider or draw it does not pay) |
| `combat::announce_end` | 259 | 8 | 195 | 0 | - |
| `combat::answer` | 894 | 37 | 894 | 8 | 3 the machine: a position it does not carry (cancelled hits, Duranium, staged destructions, Devotion, a retreat the core does not move)<br>2 nested (an action card the core does not play)<br>2 nested (unported ability reaction:*:SUSTAIN_DAMAGE_USED:when (card reflective))<br>2 the core window refused the recorded answer or stage<br>1 nested (unported ability reaction:*:SHIP_DESTROYED:after (card courageous)) |
| `combat::barrage_step` | 191 | 4 | 127 | 3 | 45 gate: an Argent side (the excess hits of the barrage)<br>5 gate: the Jol-Nar commander (a reroll question in the barrage)<br>2 Waylay played in the barrage (the hits go to every ship)<br>1 nested (unported ability reaction:*:UNIT_ABILITY_ROLLED:after (card scramble)) |
| `combat::begin_round` | 366 | 116 | 268 | 64 | 55 a Letnev side (Munitions, the self-repairing flagship)<br>6 nested (a card cost the core does not judge)<br>6 the core window refused the recorded answer or stage<br>3 nested (unported ability reaction:*:COMBAT_ROUND_STARTED:after (card reveal_prototype)) |
| `combat::offer_sustain` | 90 | 1 | 90 | 1 | 1 nested (unported ability reaction:*:SUSTAIN_DAMAGE_USED:when (card reflective)) |
| `combat::roll_round` | 283 | 0 | 187 | 0 | 62 gate: The Thundarian watches the Roll Dice step (Nomad agent, or a Yssaril copy) |
| `combat::settle` | 457 | 34 | 709 | 38 | 184 gate: a Mentak side (Ambush at the start of the combat)<br>138 gate: The Thundarian watches the Roll Dice step (Nomad agent, or a Yssaril copy)<br>129 gate: an Argent side (the excess hits of the barrage)<br>109 gate: a Letnev side (Munitions, the self-repairing flagship)<br>18 notes of the end announcement (Cabal marks, a staged event) written by the string<br>18 gate: Assault Cannon, Dimensional Splicer or Impulse Core (start-of-combat hits) |
| `game::close_vote` | 141 | 20 | 141 | 20 | 12 nested (unported ability ability:*:future_sight:AGENDA_RESOLVED:after)<br>8 a rider or draw the core does not pay |
| `game::reveal_agenda` | 141 | 10 | 141 | 10 | 10 nested (unported ability ability:*:galactic_threat:AGENDA_REVEALED:after) |
| `invasion::answer_custodians` | 20 | 14 | 20 | 13 | 8 paid removal at a table with a faction the production window does not carry<br>5 naalu seat |
| `invasion::begin_ground_round` | 317 | 0 | 352 | 0 | 10 Dunlain Reaper |
| `invasion::bombard_step` | 1376 | 1 | 1376 | 1 | 1 nested (dice rolled by an ability in a window lent no stream)<br>1 the core window refused the recorded answer or stage |
| `invasion::commit_one` | 2030 | 2 | 2030 | 2 | 2 nested (unported ability reaction:*:UNITS_COMMITTED:after (card ghost_squad)) |
| `invasion::finish_committing` | 0 | 0 | 0 | 0 | 1080 gate: the Research Team offer and the forced combats of the commitment are the string's (5b) |
| `invasion::finish_control_gain` | 0 | 0 | 0 | 0 | 1348 gate: the control-gain chain (Assimilate, PLANET_CONTROL_GAINED and its windows, technology AFTER windows, legendary and Fracture, exploration) is the string's (5a) |
| `invasion::roll_ground_tail` | 320 | 0 | 367 | 0 | - |
| `laws::offer_discard` | 81 | 73 | 81 | 0 | - |
| `leaders::hacanagent` | 11 | 8 | 11 | 8 | 8 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |
| `production::pay_step` | 42 | 4 | 42 | 0 | - |
| `production::sling_relay` | 17 | 9 | 17 | 9 | 9 a table whose seats the triggered production does not carry (Mentak, Naaz, Nomad, Yssaril, Keleres, Deepwrought) |
| `relics::ask_technology` | 9 | 7 | 9 | 7 | 7 a source that is no relic (Research Team, an agenda: the string question alone) |
| `strategy::secondary` | 2401 | 6 | 2401 | 6 | 6 a preview window or a card nobody holds |
| `strategy_cards::construction_secondary` | 126 | 68 | 126 | 56 | 56 a seat the production windows do not carry, or Minister of Industry (a production after the dock) |
| `strategy_cards::imperial_primary` | 116 | 37 | 116 | 37 | 36 the hand would pass its limit (the string asks which secret to return)<br>1 a scoreable objective the tables do not know |
| `strategy_cards::imperial_secret` | 290 | 146 | 290 | 146 | 146 the hand would pass its limit (the string asks which secret to return) |
| `strategy_cards::leadership_primary` | 115 | 78 | 115 | 62 | 59 a table with a seat the production windows do not carry (Naaz, Nomad, Yssaril, Keleres, Deepwrought)<br>3 a command token in a Mahact fleet pool (the reinforcements differ) |
| `strategy_cards::leadership_secondary` | 299 | 179 | 299 | 164 | 156 a table with a seat the production windows do not carry (Naaz, Nomad, Yssaril, Keleres, Deepwrought)<br>8 a command token in a Mahact fleet pool (the reinforcements differ) |
| `strategy_cards::politics_primary` | 91 | 38 | 91 | 38 | 38 a draw that is not plain (a hand over its limit, a draw bonus, a hooked draw) |
| `strategy_cards::politics_secondary` | 71 | 32 | 71 | 32 | 32 a draw that is not plain (a hand over its limit, a draw bonus, a hooked draw) |
| `strategy_cards::te4_construction` | 116 | 80 | 116 | 79 | 67 a seat the production windows do not carry, or Minister of Industry (a production after the dock)<br>7 the production of the Thunder's Edge Construction (a production window nested in the card)<br>5 a structure the seat may place instead of a PDS (Titans' Hecatoncheires) |
| `strategy_cards::technology_primary` | 131 | 54 | 131 | 54 | 46 a seat whose own effects on a research are questions or forms (Deepwrought, Keleres, Mentak, Last Bastion, Crimson) or a purged technology<br>8 the research hooks of the string and the core disagree (a hook the core does not carry) |
| `strategy_cards::technology_secondary` | 108 | 25 | 108 | 25 | 21 a seat whose own effects on a research are questions or forms (Deepwrought, Keleres, Mentak, Last Bastion, Crimson) or a purged technology<br>2 the research hooks of the string and the core disagree (a hook the core does not carry)<br>2 the research window left the path the core carries (a research only a faction waiver opens) |
| `strategy_cards::trade_primary` | 100 | 58 | 100 | 58 | 58 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |
| `strategy_cards::trade_secondary` | 116 | 70 | 116 | 70 | 70 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |

### lent set, window by window

| window | calls before | bailed before | calls after | bailed after | reasons after (runs) |
|---|---:|---:|---:|---:|---|
| `ProductionWindow::step` | 3532 | 476 | 3532 | 0 | - |
| `action_cards::announce` | 332 | 13 | 329 | 13 | 11 a technology gained at a table the research window does not carry<br>2 a card the core announce window does not play<br>2 the typed state could not be lent (a value the codecs refuse) |
| `agenda_effects::resolve_with` | 143 | 3 | 143 | 3 | 2 NO REASON RECORDED<br>1 an agenda effect the core window does not carry (a rider or draw it does not pay) |
| `combat::announce_end` | 321 | 12 | 294 | 0 | - |
| `combat::answer` | 968 | 27 | 968 | 25 | 12 the core window refused the recorded answer or stage<br>10 nested (Nomad Temporal Command Suite (agent exhaustion))<br>5 the machine: a position it does not carry (cancelled hits, Duranium, staged destructions, Devotion, a retreat the core does not move)<br>4 nested (unported ability reaction:*:SUSTAIN_DAMAGE_USED:when (card reflective))<br>3 nested (unported ability reaction:*:SHIP_DESTROYED:after (card courageous))<br>2 nested (an action card the core does not play) |
| `combat::barrage_step` | 255 | 10 | 232 | 10 | 42 gate: an Argent side (the excess hits of the barrage)<br>13 gate: the Jol-Nar commander (a reroll question in the barrage)<br>7 Waylay played in the barrage (the hits go to every ship)<br>3 nested (unported ability reaction:*:UNIT_ABILITY_ROLLED:after (card scramble)) |
| `combat::begin_round` | 389 | 210 | 362 | 158 | 133 a Letnev side (Munitions, the self-repairing flagship)<br>20 the core window refused the recorded answer or stage<br>14 nested (Nomad Temporal Command Suite (agent exhaustion))<br>6 nested (a card cost the core does not judge)<br>5 nested (unported ability reaction:*:COMBAT_ROUND_STARTED:after (card reveal_prototype)) |
| `combat::offer_sustain` | 117 | 3 | 117 | 3 | 2 the Letnev commander's payment held another way than as an unlocked leader<br>1 nested (unported ability reaction:*:SUSTAIN_DAMAGE_USED:when (card reflective)) |
| `combat::roll_round` | 205 | 0 | 177 | 0 | 160 gate: The Thundarian watches the Roll Dice step (Nomad agent, or a Yssaril copy) |
| `combat::settle` | 428 | 20 | 505 | 23 | 385 gate: a Letnev side (Munitions, the self-repairing flagship)<br>260 gate: a Mentak side (Ambush at the start of the combat)<br>130 gate: The Thundarian watches the Roll Dice step (Nomad agent, or a Yssaril copy)<br>115 gate: an Argent side (the excess hits of the barrage)<br>40 gate: the Jol-Nar commander (a reroll question in the barrage)<br>13 the machine: a position it does not carry (cancelled hits, Duranium, staged destructions, Devotion, a retreat the core does not move) |
| `game::close_vote` | 161 | 50 | 161 | 50 | 39 nested (unported ability ability:*:future_sight:AGENDA_RESOLVED:after)<br>9 a rider or draw the core does not pay<br>2 a Mahact fleet token under Leadership Rider |
| `invasion::answer_custodians` | 19 | 12 | 19 | 12 | 12 paid removal at a table with a faction the production window does not carry |
| `invasion::begin_ground_round` | 311 | 0 | 364 | 0 | 58 Dunlain Reaper |
| `invasion::commit_one` | 2303 | 4 | 2303 | 4 | 4 nested (unported ability reaction:*:UNITS_COMMITTED:after (card ghost_squad)) |
| `invasion::finish_committing` | 0 | 0 | 0 | 0 | 1233 gate: the Research Team offer and the forced combats of the commitment are the string's (5b) |
| `invasion::finish_control_gain` | 0 | 0 | 0 | 0 | 1517 gate: the control-gain chain (Assimilate, PLANET_CONTROL_GAINED and its windows, technology AFTER windows, legendary and Fracture, exploration) is the string's (5a) |
| `invasion::ground_round_hits` | 421 | 14 | 421 | 14 | 14 nested (Nomad Temporal Command Suite (agent exhaustion))<br>14 the core window refused the recorded answer or stage<br>1 hits added to a roll |
| `invasion::roll_ground_tail` | 320 | 0 | 442 | 0 | - |
| `laws::offer_discard` | 100 | 86 | 100 | 0 | - |
| `leaders::hacanagent` | 47 | 29 | 47 | 29 | 29 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |
| `production::pay_step` | 61 | 1 | 61 | 0 | - |
| `production::sling_relay` | 24 | 22 | 24 | 22 | 22 a table whose seats the triggered production does not carry (Mentak, Naaz, Nomad, Yssaril, Keleres, Deepwrought) |
| `relics::ask_technology` | 12 | 11 | 12 | 11 | 11 a source that is no relic (Research Team, an agenda: the string question alone) |
| `strategy_cards::construction_secondary` | 163 | 106 | 163 | 106 | 101 a seat the production windows do not carry, or Minister of Industry (a production after the dock)<br>5 a structure the seat may place instead of a PDS (Titans' Hecatoncheires) |
| `strategy_cards::imperial_primary` | 112 | 42 | 112 | 42 | 38 the hand would pass its limit (the string asks which secret to return)<br>3 a point that would pass the victory target<br>1 a scoreable objective the tables do not know |
| `strategy_cards::imperial_secret` | 275 | 128 | 275 | 128 | 128 the hand would pass its limit (the string asks which secret to return) |
| `strategy_cards::leadership_primary` | 109 | 77 | 109 | 77 | 74 a table with a seat the production windows do not carry (Naaz, Nomad, Yssaril, Keleres, Deepwrought)<br>3 a command token in a Mahact fleet pool (the reinforcements differ) |
| `strategy_cards::leadership_secondary` | 324 | 241 | 324 | 241 | 204 a table with a seat the production windows do not carry (Naaz, Nomad, Yssaril, Keleres, Deepwrought)<br>37 a command token in a Mahact fleet pool (the reinforcements differ) |
| `strategy_cards::politics_primary` | 78 | 31 | 78 | 31 | 31 a draw that is not plain (a hand over its limit, a draw bonus, a hooked draw) |
| `strategy_cards::politics_secondary` | 86 | 33 | 86 | 33 | 33 a draw that is not plain (a hand over its limit, a draw bonus, a hooked draw) |
| `strategy_cards::te4_construction` | 119 | 86 | 119 | 86 | 76 a seat the production windows do not carry, or Minister of Industry (a production after the dock)<br>7 a structure the seat may place instead of a PDS (Titans' Hecatoncheires)<br>3 the production of the Thunder's Edge Construction (a production window nested in the card) |
| `strategy_cards::te6_warfare` | 106 | 0 | 106 | 8 | 8 a command token in a Mahact fleet pool (the reinforcements differ) |
| `strategy_cards::technology_primary` | 126 | 77 | 126 | 77 | 75 a seat whose own effects on a research are questions or forms (Deepwrought, Keleres, Mentak, Last Bastion, Crimson) or a purged technology<br>2 the research window left the path the core carries (a research only a faction waiver opens) |
| `strategy_cards::technology_secondary` | 117 | 58 | 117 | 58 | 53 a seat whose own effects on a research are questions or forms (Deepwrought, Keleres, Mentak, Last Bastion, Crimson) or a purged technology<br>5 the research window left the path the core carries (a research only a faction waiver opens) |
| `strategy_cards::trade_primary` | 101 | 69 | 101 | 69 | 69 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |
| `strategy_cards::trade_secondary` | 135 | 77 | 135 | 77 | 77 a replenishment that causes events the core does not hold (Cabal, a siphoning plot, a Trade Agreement) |

## Differences the lent set found

The 19 games of the lent set reported 11 differences at the first W0 binary (the golden games none). What happened to each:

| difference | cause | state |
|---|---|---|
| 8 x `strategy_cards::te6_warfare` `details.reinforcements` string = n, core = n + 1 | the TE Warfare window counted the reinforcements without the command tokens in a Mahact fleet pool | FIXED: a counted bail (`a command token in a Mahact fleet pool`), test `the_thunders_edge_warfare_*` in `strategy_cards.rs` (fails without the bail) |
| `invasion::begin_ground_round` `context.invasion_seq` / `canonical()` / hash (found when the Naaz rounds un-gated) | the core Supercharge question of a GROUND round did not name the invasion it is asked in | FIXED: `naaz::offer_supercharge` reads `transient.invasion.active.seq`; the lockstep test now runs inside an active invasion (fails without it) |
| `combat::begin_round` `options[..]` of a Skilled Retreat (a destination "42" missing) | the string notes the destinations of every play of the card in a window (`note_withdraw`), the core kept one note per (player, system): the LAST, so the first play read the destinations of a later round | FIXED: notes are keyed by (player, system, combat round sequence); test `skilled_retreat_reads_the_destinations_of_its_own_round` |
| `combat::settle` `combat_presentation.barrage_dice[i].unit` string = `naaz_mech_space`, core = `extra barrage` | the roll texts of the composed machine were interned from the units on the board before the fight; the ship form a Naaz mech takes at the start of the fight was missing | FIXED: `with_twin_forms`; test `a_dual_form_unit_brings_its_other_form_into_the_roll_texts` |
| composed Naaz space round, round 2: the core machine kept the +1 of Supercharge | the combat kernel's string bridge carried the Supercharge term through a snapshot of the string state taken when the window started, so a multi-round window kept round 1's +1 | FIXED: native `naaz::query::supercharge` in `combat_kernel`, the bridge subtracts the string term (`naaz::supercharge_text`); test `a_naaz_side_opens_its_space_rounds_in_the_core_machine_in_lockstep` (seed 4, composed) |
| `leaders::jolnarhero` `board.12.planet_units.jol[1].type_id` string = `jolnar_mech`, core = `mech` | the core window of Rin (B7, `leaders_core::jolnar_hero`) re-derives the unit forms after the technology swap with the generic mech where the string keeps the faction mech | OPEN, not a lent-bodies piece (B7). Repro: `dual_run tools/rulebot/golden-lent.json 20 all=dual --game 12` |
| `nested.events` string = `[ACTION_CARD_DISCARDED, ACTION_CARD_PLAYED]`, core = `[]` | an action card played and discarded inside a nested ability the driver window does not record on the core side | OPEN (driver, D1b). Repro: game 3 of `golden-lent.json` |
| `combat::announce_end` `board.02.planet_units.mollprimus[7].type_id` string = `mentak_mech`, core = `mech` (game 0, first W0 binary only) | not reproduced in the later runs; the Mentak form work of W3 changed the path | UNEXPLAINED, absent after W3: keep an eye on game 0 |

## What stays a bail or a gate, and why

Every line of the tables above is one of these; each has a test that pins the reason (a bail test counts the reason, a gate test
the label), or is owned by another package:

* `a Letnev side (Munitions, the self-repairing flagship)` (`round_opening_why`): Letnev's round opening is a question plus a
  payment and a repair; todo item W2b. Tests `the_round_opening_of_naaz_letnev_and_diplomacy_games_stays_string`.
* `a Mentak side (Ambush at the start of the combat)`, `Assault Cannon, Dimensional Splicer or Impulse Core`: hits at the start of the
  fight with their own questions (`opening_why`).
* `an Argent side (the excess hits of the barrage)`, `the Jol-Nar commander`, `Waylay`: the barrage step (`barrage_why`).
* `The Thundarian watches the Roll Dice step`: the step-ended event is inside the compared window of the SPACE roll (`roll_why`);
  the ground tail runs since W1 (`the_thundarian_watching_does_not_stop_the_tail_window_in_lockstep`).
* `Dunlain Reaper` (`begin_ground_unported`): a question and a payment inside the opening of a ground round.
* `invasion::finish_control_gain` (5a) and `invasion::finish_committing` (5b): the control-gain chain and the commitment's Research
  Team and forced combats; counted as gates, ported by W5/W8 later.
* `a seat the production windows do not carry` (`unported_window`: a Naaz own seat, Keleres, Deepwrought; a Nomad or Yssaril seat
  anywhere): W6.
* Last Bastion retreat (`retreat_unported`), Dark Energy Tap with a retreat that explores, `a retreat the core does not move`
  (the composed machine): documented bails.
* Dropped by decision, no port planned: deck reserve and turn redo, structured diplomacy, Last Bastion, Crimson, Keleres, Deepwrought,
  Firmament, Obsidian, Ral Nel.

## Package W10 (lane 10): what the lines above no longer say

Not re-counted (no census before the end-of-D1 run). Gates of the sections above that are now core halves: the Jol-Nar commander in the barrage (`gate: the Jol-Nar commander`), Waylay in the barrage, `a Letnev side (Munitions ...)` below rules version 8 (the payment at the opening), the machine bails "cancelled hits" and "a roll that ends the fight" and the stranded ships of a retreat, X-89 in the ground roll / tail / bombardment, a command token in a Mahact fleet pool (Leadership, Warfare, TE Warfare, Fabrication, Leadership Rider), the end-of-use effects of a production by an ability (Hacan breakthrough, Prophecy of Ixth, swapped value), a galvanized ground force that falls (`|g`), and the Valefar-carried Production Biomes / Vortex / Chaos Mapping / C4R. See `docs/W_L10_RESUME.md` for the gates that stay and why.

## Package W13 (lane 13): what the lines above no longer say

Not re-counted (no census before the end-of-D1 run). Now core halves: the Thunder's Edge Warfare and Construction primaries inside the Winnu hero (and the Construction
primary played by its holder, with the use of PRODUCTION as a child window), the Mahact-pool gates of the Leadership / Warfare abilities performed by a hero or an
action card and of the Empyrean command token, the Imperial primary's point past the victory target, the Prophecy of Ixth in a home production, and for the Yin hero
X-89, Dacxive Animators, the Letnev Dunlain Reaper (read in each round), the hits a faction adds to a roll, the Minister of Exploration and the Crown of Emphidia on the
control gained, the Thundarian and Indoctrination (nested activity, followed on the core table). The Naaz / Winnu / Empyrean / Mahact / Yin / TE Construction windows follow
nested reaction windows (`outside_following`). See `docs/W_L13_RESUME.md` for what stays.

## Package W12 (lane 12): what the lines above no longer say

Not re-counted (no census). Bails and gates of the sections above that are now core halves: `combat::offer_sustain` / `combat::space_cannon_offense` / the Experimental Battlestation hit flow in a game below `RULES_FACTION_EFFECTS` ("a game below the faction effects"), the Technology research below `RULES_TECH_PAYMENT_CHOICE` (`strategy_cards::technology_*`, the Technology Rider, Strategize / Overrule, Reveal Prototype), `nested (Nomad Temporal Command Suite (agent exhaustion))` at Tellurian, the L1Z1X agent, Brother Milor, Trilossa, the Sol / Letnev round agents, Ssruu copies and The Thundarian (a staged `AGENT_EXHAUSTED` as `leaders::exhaust` stages it), `factions::muaat::agent` for a copy through Ssruu, `factions::muaat::inferno` for a Nekro Z-token flagship, `factions::muaat::fires` for a Nekro holder, `a structure the seat may place instead of a PDS (Titans' Hecatoncheires)` (`strategy_cards::construction_*`, `te4_construction`), and the gate "a combat a faction rule forces (Titans' Coalescence)" of `invasion::finish_committing` (the window `invasion::forced_combats`). See `docs/W_L12_RESUME.md` for what stays.

## Package X (lane X, `core-x-explore`): what the lines above no longer say

Not re-counted (no census). The exploration is one core window now (`rules::explore_windows::Exploring`, lockstep behind `exploration::explore_with` / `explore_frontier`, labels
`exploration::explore` / `exploration::explore_frontier`): gone from the gates above are `invasion::control_gain_exploration` (the `note_gate` was removed; the first-claim exploration of
the string invasion is a compared window), the string-run explorations of the Crown of Emphidia and Dark Energy Tap tail, of the Naaz / Titans / Cabal / legendary / action-card callers, and the
card arms of the Empyrean hero (it explores through the same window). Gates of the native invasion that are now core: the exploration of a first claim (`IStage::Exploring`), the Minister of Exploration and the
Crown of Emphidia on the control gain, a ship-form unit flipping on landing, and in the space cannon defense an entropic scar, L4 Disruptors and an attachment that gives SPACE CANNON
(`dual_window::defense_unported` keeps only irreproducible dice and a neutral invader). What stays (counted bails, label `exploration::explore [bail: ..]`): Freelancers, Titans' Terragenesis,
a replenishment of two seats of one faction, a draw a faction hook changes; in the native first claim also a card whose child window of another area asks a question inside the exploration (`LinkedAsks`).
See `docs/X_EXPLORE_RESUME.md`.
## Package W11 (lane 11): what the lines above no longer say

Not re-counted (no census before the end-of-D1 run). Gates of the sections above that are now core halves: every `a neutral side` gate (`dual::reason::NEUTRAL`: the barrage, roll, flow, round-opening, retreat, announce-end, Assault Cannon and ground windows, `Assault Cannon at a neutral side` and `a side that is not a seated player` of `opening_why`, `neutral units in the system` and `a neutral side` of the space cannon offense and the end of the fight in `cannon_dual`), and the nested bails `unported ability reaction:*:SUSTAIN_DAMAGE_USED:when (card reflective)`, `reaction:*:SHIP_DESTROYED:after (card courageous)` and `reaction:*:UNITS_COMMITTED:after (card ghost_squad)`. The bails that stay for the cards played inside a window, with the reason, are in `docs/W_L11_RESUME.md`.
