#!/usr/bin/env python3
"""The differential-test corpus of the soak (docs/ENGINE_SOAK.md): selfplay matrices for `dual_run`.

  soak_corpus.py lanes                                  the lanes, their weights and what each varies
  soak_corpus.py gen LANE GAMES FIRST_SEED [--out F]    one matrix (stdout or F) of GAMES games, seeds FIRST_SEED..
  soak_corpus.py profiles [DIR]                         write the committed tier files (100/300/1000 games per lane)
  soak_corpus.py coverage SPECS.jsonl...                faction / map / player-count histogram of `dual_run --specs` output

A lane is a selfplay matrix (docs/SELFPLAY.md format; `dual_run CONFIG GAMES AREAS` plays it) with one bot and a
mix of setups: player counts 3-8, template maps (hyperlane maps included) and the seeded random board, the lobby's
faction draw (LOBBY_FACTIONS: Last Bastion and Crimson are not seated), rules version, Thunder's Edge on or off.
`gen` splits GAMES over the lane's setups by weight and gives every setup its own seed range, so the same
(lane, games, first seed) always makes the same games. Never replaces tools/engine_profile/profile-20.json.
"""
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
MAPS_FILE = os.path.join(ROOT, "tools", "engine_profile", "soak", "maps.json")

LIMITS = {"max_rounds": 50, "max_steps": 400000, "max_seconds": 300, "loop_steps": 5000}

# Share of games per player count (%). Eight seats are slow games, so fewer.
PLAYERS = {3: 10, 4: 25, 5: 15, 6: 25, 7: 10, 8: 15}

# The 24 factions a lobby seats (ti4_engine::seating::LOBBY_FACTIONS).
LOBBY = ["sol", "hacan", "letnev", "xxcha", "jolnar", "l1z1x", "arborec", "mentak", "muaat", "naalu", "nekro", "saar",
         "sardakk", "winnu", "yin", "yssaril", "ghost", "argent", "cabal", "empyrean", "mahact", "naaz", "nomad", "titans"]

# Rules versions older than the model's (35). The boundary gates 31-34 (Crisis Pact, Classified, Covert, Diplomacy)
# are in, plus a spread down to 0.
OLD_VERSIONS = [0, 1, 3, 5, 8, 12, 16, 19, 21, 22, 23, 26, 29, 30, 31, 32, 33, 34]


def maps_for(players):
    """Template aliases for a player count, then the seeded random board; hyperlane maps are in the list."""
    maps = json.load(open(MAPS_FILE))
    return list(maps.get(str(players), [])) + ["random"]


def setup(name, players, games, first_seed, **kw):
    s = {"name": name, "players": players, "maps": maps_for(players), "map_policy": "random",
         "starting_tech": True, "seeds": f"{first_seed}..{first_seed + games}"}
    s.update(kw)
    return s


def split(total, weights, seed=0):
    """total split by integer weights; largest remainder, ties broken by a shuffle of `seed` (so a small slice does not
    always pick the same keys and the seed ranges of consecutive slices cover all of them)."""
    s = sum(weights.values())
    raw = {k: total * w / s for k, w in weights.items()}
    out = {k: int(v) for k, v in raw.items()}
    rest = total - sum(out.values())
    order = list(weights)
    random.Random(seed).shuffle(order)
    for k in sorted(order, key=lambda k: -(raw[k] - out[k]))[:rest]:
        out[k] += 1
    return out


def players_lane(games, first, **kw):
    counts = split(games, PLAYERS, first)
    setups, seed = [], first
    for p, n in counts.items():
        if n:
            setups.append(setup(f"{p}p", p, n, seed, **kw))
            seed += n
    return setups


def lane_std(games, first):
    return players_lane(games, first)


def lane_te(games, first):
    return players_lane(games, first, thunders_edge=True)


def lane_old(games, first):
    versions = OLD_VERSIONS
    per = split(games, {v: 1 for v in versions}, first)
    setups, seed = [], first
    for i, v in enumerate(versions):
        n = per[v]
        if n:
            players = (3, 4, 5, 6, 4, 6)[i % 6]
            # every other old version also with Thunder's Edge (it is a switch, not a rules version)
            setups.append(setup(f"v{v}-{players}p", players, n, seed, rules_version=v, thunders_edge=i % 2 == 1))
            seed += n
    return setups


def lane_factions(games, first):
    """Every lobby faction on seat a (the others drawn by the lobby), 4-6 players, half with Thunder's Edge."""
    per = split(games, {f: 1 for f in LOBBY}, first)
    setups, seed = [], first
    for i, f in enumerate(LOBBY):
        n = per[f]
        if n:
            players = (4, 5, 6, 3, 8, 7)[i % 6]
            setups.append(setup(f"f-{f}", players, n, seed, factions=[f], thunders_edge=i % 2 == 0))
            seed += n
    return setups


def bot_lane(version):
    def make(games, first):
        return players_lane(games, first)
    make.bot = {"bot_version": str(version)}
    return make


# name -> (generator, weight in the soak's round, what it varies)
LANES = {
    "std": (lane_std, 38, "rules version 35 (newest), Thunder's Edge off, players 3-8, all maps incl. hyperlanes, lobby faction draw"),
    "te": (lane_te, 14, "rules version 35, Thunder's Edge on, players 3-8, all maps"),
    "old": (lane_old, 14, "older rules versions 0..34 (every gate boundary 31-34), alternating Thunder's Edge"),
    "factions": (lane_factions, 18, "every lobby faction pinned on seat a, 3-8 players, alternating Thunder's Edge"),
    "bot-v1": (bot_lane(1), 5, "bot version 1 (the plainest bot: different choices, different rare paths), players 3-8"),
    "bot-v3": (bot_lane(3), 5, "bot version 3"),
    "bot-v5": (bot_lane(5), 6, "bot version 5"),
}


def gen(lane, games, first):
    make, _, _ = LANES[lane]
    bot = getattr(make, "bot", {})
    return {"limits": LIMITS, "setups": make(games, first), "bot": bot} if bot else {"limits": LIMITS, "setups": make(games, first)}


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd, rest = argv[0], argv[1:]
    if cmd == "lanes":
        for name, (_, w, doc) in LANES.items():
            print(f"{name:10} weight {w:3}  {doc}")
        return 0
    if cmd == "gen":
        out = None
        if "--out" in rest:
            i = rest.index("--out")
            out = rest[i + 1]
            del rest[i:i + 2]
        lane, games, first = rest[0], int(rest[1]), int(rest[2])
        text = json.dumps(gen(lane, games, first), indent=2) + "\n"
        if out:
            open(out, "w").write(text)
        else:
            sys.stdout.write(text)
        return 0
    if cmd == "profiles":
        d = rest[0] if rest else os.path.join(ROOT, "tools", "engine_profile", "soak")
        os.makedirs(d, exist_ok=True)
        for lane in LANES:
            for n in (100, 300, 1000):
                # first seed 1: the tier files are the reference corpus; the soak itself rotates the seed ranges
                open(os.path.join(d, f"{lane}-{n}.json"), "w").write(json.dumps(gen(lane, n, 1), indent=2) + "\n")
        return 0
    if cmd == "coverage":
        fac, maps, players = {}, {}, {}
        for path in rest:
            for line in open(path):
                line = line.strip()
                if not line.startswith("{"):
                    continue
                g = json.loads(line)
                players[g["players"]] = players.get(g["players"], 0) + 1
                maps[g["map"]] = maps.get(g["map"], 0) + 1
                for f in g["factions"]:
                    fac[f] = fac.get(f, 0) + 1
        print("players:", dict(sorted(players.items())))
        print("maps:", dict(sorted(maps.items())))
        print("factions:", dict(sorted(fac.items(), key=lambda kv: kv[1])))
        print("lobby factions never seated:", [f for f in LOBBY if f not in fac])
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
