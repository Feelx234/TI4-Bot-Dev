# B8 status (strategy cards)

Branch `core-b8-strategy`. Final gate PACKAGE OK on the commit before this note (verify_package.sh strategy, SKIP_AB=1 SKIP_WASM=1): 41 suites ok, dual suite ok, golden dual 0 differences, board_equivalence 0 mismatches, bench + bench-v1..v5 200/200, bench on String 200/200, clippy clean, layers 0 violations. Private target dirs deleted.
Ported: Warfare primary, Diplomacy, Politics, Leadership (over PayStep), Construction, Technology (plain path), Imperial primary (scoring stays string) and secret draw, Trade secondary, follower token spend. Not ported: the secondary sequencing window (no DecisionContext in its question, see ENGINE_KNOWN_ISSUES B8), TE variants. Nothing pushed or merged.
