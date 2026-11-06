# Lane core-jolnar-mech-1 (2026-11-06): the plain `mech` on the Jol-Nar home planet

Branch `core-jolnar-mech-1`, cut from `core-heavyfix-1` (`b7ac383a9`). Nothing pushed, nothing deployed, no heavy gate run. The six benches were run
once locally with 2 workers to list the churn; the baselines and the golden set are NOT re-recorded.

## 1. Origin

bench-v2 seed 69 (4p 4pHyperlanes arborec,yin,jolnar,xxcha, bot version 2, hash `ffc9db427f69bbc9`). A throw-away hook in `ti4-selfplay` (not committed)
printed every step at which the set of plain `mech` units on planets changed. The first such step is 1295 (decision 1481), round 5, agenda phase:
`agenda_log` = Rearmament Agreement, outcome For, `speaker_decided`. Plain mechs appear on `c:12:jol` (Jol-Nar), `a:05:nestphar` (Arborec) and
`d:14:archonren` (Xxcha). Rin (decision 1533, round 6) re-forms the Jol-Nar one; that is the `jolnar_mech` of the two Transit Diodes offers the heavyfix
lane saw.

The placement is `rules/agenda/effects/placements.rs::mechs`: `Unit::FactionlessMech` for every seat but the Naaz, no supply check. It was copied ON
PURPOSE from the string (`agenda_effects::effect_rearmament` pushes `UnitTypeId("mech")`) by lane core-lent-gaps (`LENT_GAPS_RESUME.md` section 3,
`ENGINE_KNOWN_ISSUES.md` W0-open-01) so the dual streams stayed identical. Not setup, not a deploy, not a faction technology, not a unit swap.

## 2. Verdict: a core bug (copied from the string)

Card text (`agendas.json`, `rearmament`): "For: Each player places 1 mech **from their reinforcements** on a planet they control in their home system." A
faction with its own mech has that mech in its reinforcements (Jol-Nar: J.N.S. Hylarim, core `Unit::JNSHylarim`, string `jolnar_mech`); there is no plain
mech to place. Every other placing effect of the core already uses the seat's form (`fx_strategy::place_for` / `placed_unit`, for example the infantry of
Swords to Plowshares Against in the same file). The string is wrong too; it stays as it is (user rule) and is now known string bug S9.

## 3. Fix

* Core (`placements.rs::mechs`): non-Naaz seats place through `fx_strategy::place_for(.., UnitBaseType::Mech)`: the seat's own form, refused when the
  reinforcements hold none (31.4) or a faction effect forbids it. The Naaz path is unchanged. No roster in the facts: `unsupported` (like `place_infantry`).
  Natively the roster always comes from `close.rs::effect_facts`.
* `docs/STRING_ENGINE_BUGS.md` S9.
* Dual (`dual_window::agenda_effect`): for Rearmament For the core gets a roster whose mechs are plain except the Naaz's
  (`technology_core::string_rearmament_roster_for`). The window is counted as `agenda_effects::resolve_with [known string bug S9: ...]`: compared, no bail,
  no difference. If a non-Naaz seat has no mech left in its reinforcements, the window bails with the S9 reason (new `adapter` row in
  `tools/engine_core/zero_bail_classes.tsv`).
* Shadow (`native_play::effect_bug`): a difference in a step whose string events hold `AGENDA_EFFECT_RESOLVED:rearmament` is `known string bug S9`.
* Live lockstep sweep (`agenda_live_tests::sweep`): Rearmament For is skipped (like Ixthian Artifact For / S6); new `rearmament_for_is_live_the_seats_own_mech`.

Tests:
* `ti4-engine-core-tests/tests/agenda_table_effects2.rs::rearmament_for_places_each_seats_own_mech_from_its_reinforcements` (red without the fix: the
  Jol-Nar seat gets `FactionlessMech`). It also covers empty reinforcements and the missing roster. The Naaz test now passes a roster.
* `agenda_effects::lockstep::rearmament_for_is_compared_with_the_core_cut_to_the_strings_plain_mech_s9` (dual: 1 compared, 0 bailed, 0 differences).
* `game::agenda_live_tests::rearmament_for_is_live_the_seats_own_mech`. The `native_play` test `s6_and_s7_are_recognised_by_the_step_that_ran_the_effect`
  is extended to S9.

## 4. Checks

| check | result |
|---|---|
| `native_play --quick --shadow --twice` | fingerprint `538dc68f23dddcc9` (unchanged), deterministic, ZERO GAP |
| `perf_kernel suite 1` | `suite_hash 775f4136e60a99d5` (unchanged) |
| `ti4-engine-core --lib` | 485 passed |
| `ti4-engine-core-tests` (all targets) / `ti4-engine-core-factions --lib` | ok / 160 passed |
| `ti4-engine-legacy --lib` `agenda_effects`, `agenda_live`, `rearmament`, `every_agenda` (`TI4_AREAS=all=dual`) | ok |
| `ti4-engine-core-bridge --lib` | 144 passed, **1 failed: `golden::tests::the_smoke_subset_reproduces`** (section 6) |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `check_layers.py core / thin / setup` | 576 / 192 / 14 files, 0 violations |
| `test_zero_bail.py` | 18 ok |

## 5. Bench impact (77 of 1,200 games; all still `victory`; not re-recorded)

Proof of cause: the same tree with only `placements.rs` reverted reproduces the baseline hash of all 77 games
(`selfplay replay --spec .. --expect-hash <old>`: 77 of 77). Most are not Jol-Nar games, because every faction with its own mech (Arborec, Xxcha, Sol,
...) now places that mech for Rearmament For.

Counts: bench 19, bench-v1 17, bench-v2 9, bench-v3 7, bench-v4 8, bench-v5 17.

| bench | seed / table | old hash | new hash | decisions | vp | winner | termination |
|---|---|---|---|---|---|---|---|
| bench | 7, 4p 4pHyperlanes winnu,titans,mentak,ghost | `da98e7757316cbf0` | `b2896e69b63e558a` | 2469 -> 2487 | vp [10, 8, 8, 7] -> [10, 8, 8, 7] | winner 0 -> 0 | victory |
| bench | 17, 4p 4pHyperlanes winnu,saar,nomad,nekro | `26f96990637d502c` | `d8f41b87543054b9` | 2547 -> 1847 | vp [6, 10, 10, 10] -> [6, 10, 6, 7] | winner 3 -> 1 | victory |
| bench | 27, 4p 4pHyperlanes nomad,cabal,mahact,sardakk | `932b2be169efc05e` | `86dd78f4f37fcc73` | 1807 -> 1807 | vp [10, 5, 4, 4] -> [10, 5, 4, 4] | winner 0 -> 0 | victory |
| bench | 37, 4p 4pHyperlanes naalu,sardakk,ghost,nomad | `7502a9551f279c98` | `30eb2cff0846fee4` | 2236 -> 2242 | vp [8, 8, 7, 10] -> [10, 9, 6, 9] | winner 3 -> 0 | victory |
| bench | 52, 4p 4pOuterRim jolnar,sardakk,mentak,l1z1x | `3812bf4ef94dd063` | `6c3528e74f9851a2` | 2144 -> 2167 | vp [6, 8, 10, 8] -> [6, 6, 10, 6] | winner 2 -> 2 | victory |
| bench | 61, 4p 4pHyperlanes winnu,muaat,xxcha,letnev | `f7a341a8278dd7bb` | `3c8d0413ed3fb7a4` | 2163 -> 2106 | vp [5, 10, 10, 6] -> [4, 10, 10, 6] | winner 2 -> 2 | victory |
| bench | 74, 4p 4pOuterRim empyrean,naalu,arborec,sol | `835cb209737b97fa` | `57d9b05e592451f2` | 2462 -> 2500 | vp [8, 6, 10, 9] -> [7, 6, 10, 9] | winner 2 -> 2 | victory |
| bench | 80, 4p 4pOuterRim nomad,winnu,letnev,yssaril | `c29f2c6be6dbff51` | `653e323a0fdcc103` | 1863 -> 1863 | vp [7, 7, 10, 6] -> [7, 7, 10, 6] | winner 2 -> 2 | victory |
| bench | 89, 4p 4pHyperlanes ghost,mahact,xxcha,jolnar | `ce4757e4a2743d73` | `3523a5b7977910fd` | 2356 -> 2427 | vp [10, 8, 9, 7] -> [10, 8, 6, 7] | winner 0 -> 0 | victory |
| bench | 96, 4p 4pOuterRim xxcha,naaz,arborec,empyrean | `b543ec7c09da4368` | `77f1cad9d9dfc9cf` | 2405 -> 1844 | vp [3, 8, 9, 10] -> [4, 6, 7, 10] | winner 3 -> 3 | victory |
| bench | 1, 6p 6pStandard sardakk,cabal,nomad,nekro,winnu,ghost | `1c14bdcaf103d944` | `7bbbc6e32f6049cf` | 3424 -> 3343 | vp [5, 7, 6, 10, 8, 7] -> [4, 8, 4, 10, 7, 7] | winner 3 -> 3 | victory |
| bench | 17, 6p 6pStandard winnu,saar,nomad,nekro,naaz,titans | `82ead84eddce3396` | `2250dc726cca4768` | 2939 -> 3196 | vp [4, 6, 7, 7, 7, 10] -> [6, 10, 7, 4, 8, 10] | winner 5 -> 5 | victory |
| bench | 27, 6p 6pStandard nomad,cabal,mahact,sardakk,l1z1x,naalu | `5456d22ab91c101a` | `12ab3ceadc4bb50d` | 3094 -> 3030 | vp [7, 8, 4, 8, 7, 10] -> [7, 10, 5, 8, 5, 9] | winner 5 -> 1 | victory |
| bench | 40, 6p 6pOuterRim naaz,l1z1x,sardakk,mentak,titans,cabal | `7159d8fbd7e42b43` | `b8738d134f7ad32b` | 2675 -> 2675 | vp [6, 10, 6, 8, 8, 7] -> [6, 10, 6, 8, 8, 7] | winner 1 -> 1 | victory |
| bench | 41, 6p 6pStandard jolnar,mahact,hacan,naaz,ghost,yssaril | `12da841ef4897a0f` | `ce00814aaec68be8` | 2546 -> 2546 | vp [5, 6, 10, 2, 5, 8] -> [5, 6, 10, 2, 5, 8] | winner 2 -> 2 | victory |
| bench | 58, 6p 6pOuterRim cabal,mentak,l1z1x,ghost,sardakk,jolnar | `33e21d378beb3585` | `c7eb1dc9244b54c7` | 2618 -> 2484 | vp [8, 2, 6, 6, 3, 10] -> [10, 1, 8, 6, 5, 8] | winner 5 -> 0 | victory |
| bench | 59, 6p 6pStandard nomad,mentak,winnu,argent,arborec,empyrean | `653b51b797cdf1b9` | `ea9b076131c0dbf4` | 3167 -> 3014 | vp [5, 10, 7, 5, 7, 1] -> [7, 9, 10, 6, 8, 3] | winner 1 -> 2 | victory |
| bench | 78, 6p 6pOuterRim winnu,ghost,nekro,empyrean,xxcha,saar | `80c86e2f2e07c951` | `dffb73daf17215ff` | 3745 -> 3087 | vp [9, 8, 8, 6, 7, 10] -> [5, 10, 5, 6, 9, 9] | winner 5 -> 1 | victory |
| bench | 83, 6p 6pStandard nekro,sol,naaz,ghost,muaat,titans | `202b67e395fa9ccd` | `b9170969cb54401e` | 3167 -> 3193 | vp [8, 10, 3, 9, 8, 7] -> [8, 9, 3, 10, 8, 5] | winner 1 -> 3 | victory |
| bench-v1 | 8, 4p 4pOuterRim yssaril,naalu,muaat,nomad | `03296d417fce72e2` | `5d9c028c21076d65` | 1460 -> 1460 | vp [4, 7, 4, 10] -> [4, 7, 4, 10] | winner 3 -> 3 | victory |
| bench-v1 | 17, 4p 4pHyperlanes winnu,saar,nomad,nekro | `b3df98fdfce85792` | `b728553e9041b0aa` | 2628 -> 1758 | vp [9, 10, 8, 5] -> [6, 10, 8, 4] | winner 1 -> 1 | victory |
| bench-v1 | 26, 4p 4pOuterRim argent,nekro,empyrean,xxcha | `ca4816d5c0c037ae` | `c109c817086376bc` | 1847 -> 1847 | vp [8, 10, 6, 6] -> [8, 10, 6, 6] | winner 1 -> 1 | victory |
| bench-v1 | 41, 4p 4pHyperlanes jolnar,mahact,hacan,naaz | `391c91ddb09c5a88` | `546e676634383d78` | 2338 -> 1840 | vp [9, 8, 7, 10] -> [8, 4, 3, 10] | winner 3 -> 3 | victory |
| bench-v1 | 52, 4p 4pOuterRim jolnar,sardakk,mentak,l1z1x | `d0b498f03a9e8a62` | `8a45ce92982631e7` | 2305 -> 2042 | vp [8, 8, 10, 10] -> [9, 5, 9, 10] | winner 2 -> 3 | victory |
| bench-v1 | 62, 4p 4pOuterRim sol,cabal,mahact,hacan | `2ea5f17bf86a4039` | `c83a978a2cb488b0` | 1355 -> 1354 | vp [2, 4, 3, 10] -> [2, 4, 3, 10] | winner 3 -> 3 | victory |
| bench-v1 | 76, 4p 4pOuterRim argent,mentak,ghost,jolnar | `cd8104c15a0fd6de` | `976eebf36fb60885` | 1620 -> 1620 | vp [3, 4, 7, 10] -> [3, 4, 7, 10] | winner 3 -> 3 | victory |
| bench-v1 | 78, 4p 4pOuterRim winnu,ghost,nekro,empyrean | `7d41632e7ee3b619` | `40f72e9186c0a79f` | 3283 -> 3317 | vp [7, 8, 9, 5] -> [10, 6, 8, 6] | winner 2 -> 0 | victory |
| bench-v1 | 82, 4p 4pOuterRim sardakk,cabal,hacan,mahact | `b0ab7c39163bfcf3` | `dd966dff78304ffa` | 2077 -> 2077 | vp [5, 10, 6, 7] -> [5, 10, 6, 7] | winner 1 -> 1 | victory |
| bench-v1 | 83, 4p 4pHyperlanes nekro,sol,naaz,ghost | `60185f44bef0e9fc` | `1c8ae777d98ec8ac` | 1698 -> 1698 | vp [4, 10, 6, 5] -> [4, 10, 6, 5] | winner 1 -> 1 | victory |
| bench-v1 | 17, 6p 6pStandard winnu,saar,nomad,nekro,naaz,titans | `b8e75d7affd9d44d` | `a6b29601aea815f8` | 2758 -> 3123 | vp [4, 10, 8, 7, 5, 9] -> [4, 9, 9, 10, 6, 9] | winner 1 -> 3 | victory |
| bench-v1 | 27, 6p 6pStandard nomad,cabal,mahact,sardakk,l1z1x,naalu | `f5a8380a3eaccd07` | `9e0e307f3edec68a` | 2935 -> 2943 | vp [6, 9, 4, 9, 5, 10] -> [6, 9, 4, 9, 5, 10] | winner 5 -> 5 | victory |
| bench-v1 | 40, 6p 6pOuterRim naaz,l1z1x,sardakk,mentak,titans,cabal | `61ab2ea9910dfc1d` | `603d27a6c119aa32` | 1757 -> 1757 | vp [3, 3, 6, 5, 3, 10] -> [3, 3, 6, 5, 3, 10] | winner 5 -> 5 | victory |
| bench-v1 | 41, 6p 6pStandard jolnar,mahact,hacan,naaz,ghost,yssaril | `fc0fcf66faeaa7eb` | `c143f8b2e79e0f73` | 3260 -> 3279 | vp [8, 6, 6, 9, 10, 9] -> [7, 7, 6, 9, 8, 10] | winner 4 -> 5 | victory |
| bench-v1 | 74, 6p 6pOuterRim empyrean,naalu,arborec,sol,yssaril,mahact | `ab95ec341158830a` | `d4520487ea7e240f` | 2413 -> 2404 | vp [2, 9, 1, 10, 2, 3] -> [3, 9, 1, 10, 2, 2] | winner 3 -> 3 | victory |
| bench-v1 | 82, 6p 6pOuterRim sardakk,cabal,hacan,mahact,letnev,l1z1x | `a42ec8ca55c1259d` | `b832b21d82789857` | 3513 -> 3507 | vp [7, 6, 9, 3, 9, 9] -> [7, 6, 8, 3, 9, 9] | winner 5 -> 5 | victory |
| bench-v1 | 88, 6p 6pOuterRim l1z1x,titans,mentak,argent,empyrean,sol | `642a8f9c1df688e3` | `de71b47f21bd16a9` | 2403 -> 2371 | vp [10, 3, 8, 2, 6, 6] -> [10, 6, 8, 2, 3, 4] | winner 0 -> 0 | victory |
| bench-v2 | 8, 4p 4pOuterRim yssaril,naalu,muaat,nomad | `d78f231d65916952` | `ea44dc1e27e10ead` | 1722 -> 1722 | vp [4, 9, 5, 10] -> [4, 9, 5, 10] | winner 3 -> 3 | victory |
| bench-v2 | 69, 4p 4pHyperlanes arborec,yin,jolnar,xxcha | `ffc9db427f69bbc9` | `0dabf57b49305693` | 1830 -> 1843 | vp [7, 5, 6, 10] -> [7, 5, 6, 10] | winner 3 -> 3 | victory |
| bench-v2 | 36, 6p 6pOuterRim yssaril,titans,argent,l1z1x,arborec,nekro | `1d37e240ea40b3de` | `bc3395b60acdfc22` | 1998 -> 1998 | vp [5, 9, 10, 0, 9, 4] -> [5, 9, 10, 0, 9, 4] | winner 2 -> 2 | victory |
| bench-v2 | 58, 6p 6pOuterRim cabal,mentak,l1z1x,ghost,sardakk,jolnar | `a273800eb3a86cb0` | `8ddf2a2df9a705b2` | 2153 -> 2222 | vp [10, 5, 4, 6, 7, 4] -> [10, 5, 4, 6, 5, 6] | winner 0 -> 0 | victory |
| bench-v2 | 59, 6p 6pStandard nomad,mentak,winnu,argent,arborec,empyrean | `8a4bc50554025f49` | `542aa5f456a32cc5` | 3448 -> 3479 | vp [3, 8, 9, 5, 10, 6] -> [4, 8, 9, 5, 10, 7] | winner 4 -> 4 | victory |
| bench-v2 | 69, 6p 6pStandard arborec,yin,jolnar,xxcha,winnu,nomad | `bdd4fa0f32ec77e4` | `fe88439d660cc8b3` | 2608 -> 2438 | vp [4, 6, 8, 8, 6, 10] -> [7, 8, 5, 10, 5, 5] | winner 5 -> 3 | victory |
| bench-v2 | 74, 6p 6pOuterRim empyrean,naalu,arborec,sol,yssaril,mahact | `f2a4bf9f9d6b54a9` | `3e7b01f174988ef4` | 1850 -> 1842 | vp [2, 3, 2, 10, 2, 3] -> [2, 3, 2, 10, 2, 3] | winner 3 -> 3 | victory |
| bench-v2 | 75, 6p 6pStandard l1z1x,xxcha,muaat,mahact,hacan,yin | `84444aec87423659` | `a4af2b69ad341596` | 3088 -> 2615 | vp [9, 10, 6, 2, 9, 5] -> [7, 10, 7, 4, 5, 5] | winner 1 -> 1 | victory |
| bench-v2 | 86, 6p 6pOuterRim xxcha,titans,ghost,mahact,empyrean,letnev | `8d48e908ff36b1ab` | `30aa6246b16e6127` | 2905 -> 3406 | vp [6, 9, 3, 10, 2, 4] -> [9, 8, 4, 10, 3, 7] | winner 3 -> 3 | victory |
| bench-v3 | 17, 4p 4pHyperlanes winnu,saar,nomad,nekro | `0c6ab4a04ecedad5` | `0ed74564e1b00282` | 2163 -> 1753 | vp [4, 5, 10, 6] -> [5, 7, 10, 5] | winner 2 -> 2 | victory |
| bench-v3 | 26, 4p 4pOuterRim argent,nekro,empyrean,xxcha | `243aab20087af59c` | `99c7d69e7ffb5fe5` | 1575 -> 1408 | vp [10, 9, 10, 10] -> [8, 4, 10, 6] | winner 0 -> 2 | victory |
| bench-v3 | 27, 6p 6pStandard nomad,cabal,mahact,sardakk,l1z1x,naalu | `43895c37bb2a3c31` | `2ad9a652d1c5d926` | 2944 -> 2497 | vp [8, 5, 5, 2, 10, 7] -> [10, 4, 5, 2, 7, 7] | winner 4 -> 0 | victory |
| bench-v3 | 36, 6p 6pOuterRim yssaril,titans,argent,l1z1x,arborec,nekro | `fdf63d04d36337ad` | `db25d3193af3927c` | 2446 -> 2666 | vp [9, 4, 2, 9, 10, 6] -> [6, 5, 4, 9, 10, 3] | winner 4 -> 4 | victory |
| bench-v3 | 59, 6p 6pStandard nomad,mentak,winnu,argent,arborec,empyrean | `396b111c324893ba` | `b1c797f79a160954` | 3612 -> 3327 | vp [6, 5, 9, 9, 10, 5] -> [8, 4, 8, 8, 10, 5] | winner 4 -> 4 | victory |
| bench-v3 | 62, 6p 6pOuterRim sol,cabal,mahact,hacan,nekro,saar | `0b922e9c0ebb5e37` | `5c034c1a44465796` | 2481 -> 1997 | vp [5, 10, 6, 9, 4, 8] -> [4, 7, 4, 10, 4, 7] | winner 1 -> 3 | victory |
| bench-v3 | 74, 6p 6pOuterRim empyrean,naalu,arborec,sol,yssaril,mahact | `ac48a94178780d06` | `dc5fcad487f8bde1` | 3657 -> 2915 | vp [7, 5, 6, 8, 4, 7] -> [9, 5, 5, 10, 3, 3] | winner 3 -> 3 | victory |
| bench-v4 | 40, 4p 4pOuterRim naaz,l1z1x,sardakk,mentak | `11509a05844e51bd` | `1fe2247df7fd5e23` | 1916 -> 1916 | vp [5, 7, 7, 10] -> [5, 7, 7, 10] | winner 3 -> 3 | victory |
| bench-v4 | 96, 4p 4pOuterRim xxcha,naaz,arborec,empyrean | `352019a6fe1fb341` | `9b2716715478f7ad` | 1904 -> 1721 | vp [6, 5, 4, 10] -> [6, 6, 5, 10] | winner 3 -> 3 | victory |
| bench-v4 | 1, 6p 6pStandard sardakk,cabal,nomad,nekro,winnu,ghost | `9a511f2a26429fc6` | `d6ce5904a4be8b4f` | 2566 -> 2566 | vp [6, 10, 5, 7, 7, 5] -> [6, 10, 5, 7, 7, 5] | winner 1 -> 1 | victory |
| bench-v4 | 27, 6p 6pStandard nomad,cabal,mahact,sardakk,l1z1x,naalu | `3cf6d596cd6cfc60` | `4f0127be377af22e` | 3013 -> 2936 | vp [8, 5, 10, 2, 7, 7] -> [8, 7, 9, 4, 9, 10] | winner 2 -> 5 | victory |
| bench-v4 | 30, 6p 6pOuterRim mentak,mahact,letnev,sardakk,yin,jolnar | `d6a13683bce797d5` | `446988e74e8b5ecd` | 2442 -> 2442 | vp [7, 8, 3, 7, 10, 8] -> [7, 8, 3, 7, 10, 8] | winner 4 -> 4 | victory |
| bench-v4 | 69, 6p 6pStandard arborec,yin,jolnar,xxcha,winnu,nomad | `f71395d4f0d078e1` | `aa95545feb6b2219` | 2606 -> 2607 | vp [2, 8, 7, 8, 8, 10] -> [2, 8, 7, 8, 8, 10] | winner 5 -> 5 | victory |
| bench-v4 | 80, 6p 6pOuterRim nomad,winnu,letnev,yssaril,xxcha,argent | `94849e3099b12d7c` | `2ae171999ef21a2c` | 2379 -> 2379 | vp [7, 3, 4, 6, 6, 10] -> [7, 3, 4, 6, 6, 10] | winner 5 -> 5 | victory |
| bench-v4 | 82, 6p 6pOuterRim sardakk,cabal,hacan,mahact,letnev,l1z1x | `bcdbfc8b87135f1f` | `936881e68ca4bd48` | 3210 -> 3210 | vp [7, 9, 1, 5, 8, 10] -> [7, 9, 1, 5, 8, 10] | winner 5 -> 5 | victory |
| bench-v5 | 1, 4p 4pHyperlanes sardakk,cabal,nomad,nekro | `f5f644263d75b83b` | `9c4607c6b69d37ee` | 2394 -> 2394 | vp [6, 10, 8, 10] -> [6, 10, 8, 10] | winner 3 -> 3 | victory |
| bench-v5 | 17, 4p 4pHyperlanes winnu,saar,nomad,nekro | `411d86d660e014b7` | `d2439e78a7625bfa` | 2038 -> 1822 | vp [5, 7, 6, 10] -> [6, 10, 5, 7] | winner 3 -> 1 | victory |
| bench-v5 | 27, 4p 4pHyperlanes nomad,cabal,mahact,sardakk | `381e75d209da0eff` | `aa959e9d4cf60ade` | 1865 -> 1865 | vp [8, 10, 3, 5] -> [8, 10, 3, 5] | winner 1 -> 1 | victory |
| bench-v5 | 36, 4p 4pOuterRim yssaril,titans,argent,l1z1x | `ed8268755e8dded4` | `944ead0d234b2657` | 2016 -> 2056 | vp [6, 7, 5, 10] -> [6, 7, 5, 10] | winner 3 -> 3 | victory |
| bench-v5 | 40, 4p 4pOuterRim naaz,l1z1x,sardakk,mentak | `94e262eeed077d58` | `8b51876ece99cdfb` | 1984 -> 1984 | vp [10, 10, 8, 6] -> [10, 10, 8, 6] | winner 0 -> 0 | victory |
| bench-v5 | 52, 4p 4pOuterRim jolnar,sardakk,mentak,l1z1x | `2b3d5205f02a14a4` | `37942976de9008b1` | 1474 -> 1474 | vp [5, 10, 4, 6] -> [5, 10, 4, 6] | winner 1 -> 1 | victory |
| bench-v5 | 74, 4p 4pOuterRim empyrean,naalu,arborec,sol | `be078849257bbbb8` | `62fdcef2fe5568cb` | 2398 -> 2425 | vp [9, 5, 5, 10] -> [8, 6, 6, 10] | winner 3 -> 3 | victory |
| bench-v5 | 96, 4p 4pOuterRim xxcha,naaz,arborec,empyrean | `4fb0b789303fe053` | `59d39ae725d97b1f` | 2887 -> 1944 | vp [4, 9, 8, 8] -> [5, 8, 6, 10] | winner 1 -> 3 | victory |
| bench-v5 | 17, 6p 6pStandard winnu,saar,nomad,nekro,naaz,titans | `b75da787da6b5419` | `c57a9ecd3a7baae2` | 3213 -> 3199 | vp [5, 9, 5, 8, 7, 10] -> [5, 10, 8, 8, 7, 9] | winner 5 -> 1 | victory |
| bench-v5 | 48, 6p 6pOuterRim naaz,mentak,naalu,ghost,nomad,muaat | `256e01f747eb88db` | `b26b1ccb78c05b4a` | 3240 -> 3391 | vp [4, 5, 8, 9, 4, 10] -> [3, 5, 9, 6, 5, 10] | winner 5 -> 5 | victory |
| bench-v5 | 52, 6p 6pOuterRim jolnar,sardakk,mentak,l1z1x,naalu,muaat | `676b66a5d317a2c5` | `7c70726063ba718e` | 2964 -> 2964 | vp [6, 9, 3, 10, 9, 5] -> [6, 9, 3, 10, 9, 5] | winner 3 -> 3 | victory |
| bench-v5 | 58, 6p 6pOuterRim cabal,mentak,l1z1x,ghost,sardakk,jolnar | `062862af28af1658` | `2f099109ba116fa7` | 2136 -> 2295 | vp [10, 1, 7, 5, 4, 1] -> [9, 5, 10, 6, 5, 5] | winner 0 -> 2 | victory |
| bench-v5 | 59, 6p 6pStandard nomad,mentak,winnu,argent,arborec,empyrean | `b1176379bc806c25` | `c71ff59158d81a7a` | 3117 -> 3098 | vp [6, 10, 5, 5, 8, 3] -> [6, 10, 5, 5, 8, 3] | winner 1 -> 1 | victory |
| bench-v5 | 75, 6p 6pStandard l1z1x,xxcha,muaat,mahact,hacan,yin | `03e4d46743a59b92` | `0fb06238cee02451` | 3133 -> 3133 | vp [8, 10, 6, 2, 7, 8] -> [8, 10, 6, 2, 7, 8] | winner 1 -> 1 | victory |
| bench-v5 | 86, 6p 6pOuterRim xxcha,titans,ghost,mahact,empyrean,letnev | `d69719a60ce92508` | `5fdafc0d828b6a98` | 2521 -> 2646 | vp [5, 9, 6, 10, 3, 6] -> [5, 9, 6, 9, 4, 10] | winner 3 -> 5 | victory |
| bench-v5 | 89, 6p 6pStandard ghost,mahact,xxcha,jolnar,saar,empyrean | `444bd98cfda4ddd5` | `f1eb5b9efafc2959` | 3726 -> 3562 | vp [8, 6, 10, 8, 8, 9] -> [8, 6, 10, 9, 8, 9] | winner 2 -> 2 | victory |
| bench-v5 | 96, 6p 6pOuterRim xxcha,naaz,arborec,empyrean,sol,titans | `6bdd469c804cc6d2` | `9e3ad62b01cc525b` | 3010 -> 3091 | vp [4, 3, 8, 7, 10, 9] -> [5, 4, 7, 6, 10, 8] | winner 4 -> 4 | victory |

## 6. Needs a decision

* **Golden set `v36-std`, game `g040`** (5p 5pLearnToPlayPoK seed 41, jolnar,mahact,hacan,naaz,ghost) no longer reproduces: the state differs at step 1512,
  the strategy phase right after that round's agenda votes. This is the intended change, but it is not proven per game the way the bench games are.
  It needs `golden bless v36-std --reason "Rearmament For places the seat's own mech (S9)" --expect ... --games g040`. Not blessed here.
* **Saved v36 games**: the change alters what rules version 36 does, and `MIN_SUPPORTED_RULES_VERSION` is 36. A saved v36 game (alpha since the flip,
  offline saves) in which Rearmament For resolved for a seat with a faction mech will not replay the same way. S6/S7 set the precedent of changing v36 in
  place. The alternative is a `RULES_VERSION` bump, but with the minimum equal to the newest version that refuses every v36 save. User call.
* **Bench baselines**: re-record the 77 entries above (as the heavyfix lane did for Rin) once the change is accepted.
