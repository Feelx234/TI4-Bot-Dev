import { playerWithHand, opponent } from "../_shared/players";
import type { MockGameOptions } from "../_shared/mockGame";
import { productionChoice } from "../Y-produce-units-inline-stats/fixtures";

const single = (unit: string, cost: number) => ({
  id: `build|${unit}|1`,
  kind: "produce",
  label: `produce 1x ${unit} for ${cost}`,
  payload: { unit, cost, count: 1 },
});

/** Before (rules < 25): the single-unit questions Minister of Industry used to ask, with no decision context. */
export const beforeGame = (): MockGameOptions => ({
  players: [playerWithHand({ faction: "sol", technologies: ["dn2", "ac2"] }), opponent],
  choice: {
    prompt: "produce one unit in 18",
    options: [
      single("carrier", 3),
      single("cruiser", 2),
      single("destroyer", 1),
      single("dreadnought", 4),
      single("fighter", 1),
      single("infantry", 1),
      single("mech", 2),
      { id: "decline", kind: "decline", label: "decline", payload: {} },
    ],
  },
});

/** After (rules >= 25): the ordinary production window the law opens in the system of the placed dock. */
export const afterGame = (): MockGameOptions => ({
  players: [playerWithHand({ faction: "sol", technologies: ["dn2", "ac2"] }), opponent],
  choice: { ...productionChoice(9, 4), prompt: "produce in 18 (4 left)" },
});
