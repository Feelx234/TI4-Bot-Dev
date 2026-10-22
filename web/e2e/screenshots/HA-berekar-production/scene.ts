import type { MockChoice } from "../_shared/mockGame";

const build = (unit: string, cost: number, count = 1, available = 3) => ({
  id: `build|${unit}|${count}`,
  kind: "produce",
  label: `produce ${count}x ${unit} for ${cost}`,
  payload: {
    unit,
    count,
    placed: count,
    cost,
    printed_cost: cost,
    discount: 0,
    production_spent: count,
    available_resources: available,
  },
});

const agent = {
  id: "exhaust_agent|winnuagent",
  kind: "production_agent",
  label: "Exhaust Berekar Berekon: -2 cost",
  payload: { leader: "winnuagent", discount_offered: 2 },
};

const done = { id: "done_producing", kind: "decline", label: "produce nothing further", payload: {} };

const context = {
  subtype: "produce_unit",
  target: { System: "07" },
  outstanding: [{ kind: "production_capacity", amount: 5, paid: 0 }],
};

/** The Winnu player producing at home with 3 resources to spend and Berekar Berekon ready. */
export const withAgent: MockChoice = {
  prompt: "produce in 07 (5 left)",
  context,
  options: [
    build("fighter", 1, 2),
    build("infantry", 1, 2),
    build("carrier", 3),
    build("destroyer", 1),
    agent,
    done,
  ],
};

/** Not one build is affordable without the card: it is the only thing to take. */
export const nothingInReach: MockChoice = {
  prompt: "produce in 07 (5 left)",
  context,
  options: [agent, done],
};
