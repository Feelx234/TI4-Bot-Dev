import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { setLoad, setShip } from "./movementDraft";
import { entryOf, activate, fits, newDraft, scriptOf, setMovement } from "./draft";
import { type MovementStep, draftOf, stepsOf } from "./movementPlan";
import type { TransportEvent } from "./transport";
import type { LocalGame } from "./savedGame";
import { type Engine, createWasmTransport, scenariosOf } from "./wasmTransport";
import type { MovementFacts, SessionUpdate } from "./wire";

// The real engine, built by `scripts/build-wasm.sh` (it is not in the repository).
const WASM = fileURLToPath(new URL("./ti4.wasm", import.meta.url));
const built = existsSync(WASM);

type Update = Extract<TransportEvent, { kind: "update" }>;

/** A game of seed 3 with seat a on this side, and the updates of it, one after the other. */
async function play(stepping = false, game: LocalGame = { seed: 3, players: 8, humans: 1 }) {
  const engine: Engine = {
    module: await WebAssembly.compile(readFileSync(WASM)),
    hash: "test",
    scenarios: [],
  };
  let answers: readonly string[] = [];
  const transport = createWasmTransport(engine, game, {
    onAnswers: (all) => (answers = [...all]),
    stepping,
  });
  const seen: Update[] = [];
  let wake: (() => void) | null = null;
  transport.subscribe((event) => {
    if (event.kind === "error") {
      throw new Error(event.message);
    }
    if (event.kind === "update") {
      seen.push(event);
      wake?.();
    }
  });
  let read = 0;
  const next = async (): Promise<Update> => {
    while (seen.length <= read) {
      await new Promise<void>((resume) => (wake = resume));
    }
    return seen[read++];
  };
  const nonce = (event: Update) => event.update.pending_choice!.nonce;
  return { transport, next, nonce, answers: () => answers };
}

const shipsIn = (update: SessionUpdate, system: string) =>
  (update.view.board.systems[system]?.units ?? [])
    .filter((unit) => unit.owner === "a")
    .map((unit) => unit.unit_type)
    .sort();
const subtype = (event: Update) => event.update.pending_choice?.choice.context?.subtype;

it.skipIf(!built)(
  "sends a movement as one plan, takes a refused plan back, and undoes a movement whole",
  async () => {
    const { transport, next, nonce, answers } = await play();
    // Seat a takes a tactical action and activates the system next to its home.
    for (const answer of ["pok8imperial", "no", "tactical", "23"]) {
      transport.submitChoice(nonce(await next()), answer);
    }
    const open = await next();
    expect(subtype(open)).toBe("movement_step");
    const facts = open.update.tactical as MovementFacts;
    expect(facts.moved).toBe(0);
    expect(shipsIn(open.update, "23")).toEqual([]);
    const home = shipsIn(open.update, "01");

    // A plan with a step that the game does not offer, after a ship that it does offer.
    let draft = setShip(facts, {}, "01|0", true);
    draft = setLoad(
      facts,
      draft,
      "01|0",
      facts.cargo.findIndex((pool) => pool.unit === "sol_infantry"),
      2,
    );
    draft = setLoad(
      facts,
      draft,
      "01|0",
      facts.cargo.findIndex((pool) => pool.unit === "fighter"),
      1,
    );
    const good = stepsOf(facts, setShip(facts, draft, "01|2", true));
    const dreadnought: MovementStep = {
      kind: "move",
      origin: "01",
      unit: "dreadnought",
      damaged: false,
      gravity_drive: false,
      ionian: false,
    };
    const bad = [...good.slice(0, -1), dreadnought, good.at(-1)!];
    transport.submitPlan(nonce(open), { destination: "23", steps: bad });
    const refused = await next();
    // Nothing of the plan is in the game: the same choice is open, on the same board.
    expect(refused.plan).toEqual({
      destination: "23",
      applied: 0,
      remaining: bad,
      interrupted: false,
      reason: "The game does not offer to move dreadnought from 01.",
    });
    expect(nonce(refused)).toBe(nonce(open));
    expect(answers()).toHaveLength(4);
    expect(shipsIn(refused.update, "23")).toEqual([]);
    expect(shipsIn(refused.update, "01")).toEqual(home);
    expect((refused.update.tactical as MovementFacts).moved).toBe(0);

    // The plan that fits: a carrier with two infantry and a fighter, and the destroyer.
    transport.submitPlan(nonce(refused), { destination: "23", steps: good });
    const after = await next();
    expect(after.plan).toBeUndefined();
    expect(subtype(after)).not.toBe("movement_step");
    expect(subtype(after)).not.toBe("load_cargo");
    expect(shipsIn(after.update, "23")).toEqual([
      "destroyer",
      "fighter",
      "sol_carrier",
      "sol_infantry",
      "sol_infantry",
    ]);
    // move, three loads, the closed hold, move, done: each is an answer of the saved game.
    expect(answers().slice(4)).toEqual([
      "move|01|0",
      "load|0",
      "load|3",
      "load|4",
      "done_loading",
      "move|01|1",
      "done_moving",
    ]);

    // One undo takes the movement back whole.
    expect(after.canUndo).toBe(true);
    transport.undo();
    const again = await next();
    expect(subtype(again)).toBe("movement_step");
    expect(nonce(again)).toBe(nonce(open));
    expect(answers()).toHaveLength(4);
    expect(shipsIn(again.update, "23")).toEqual([]);
    expect(shipsIn(again.update, "01")).toEqual(home);
    // The update tells what was taken back: the draft of the movement is staged from it.
    expect(again.undone).toEqual(good);
    expect(draftOf(facts, again.undone!)).toEqual(setShip(facts, draft, "01|2", true));

    // One more undo is one answer: the activation is open again.
    transport.undo();
    const activation = await next();
    expect(subtype(activation)).toBe("activate_system");
    expect(activation.undone).toBeUndefined();
    expect(answers()).toHaveLength(3);
    transport.close();
  },
  60_000,
);

it.skipIf(!built)(
  "plays a saved game again, and still undoes its movement whole",
  async () => {
    const first = await play();
    for (const answer of ["pok8imperial", "no", "tactical", "23"]) {
      first.transport.submitChoice(first.nonce(await first.next()), answer);
    }
    const open = await first.next();
    const facts = open.update.tactical as MovementFacts;
    const steps = stepsOf(facts, setShip(facts, setShip(facts, {}, "01|0", true), "01|2", true));
    first.transport.submitPlan(first.nonce(open), { destination: "23", steps });
    await first.next();
    const saved = [...first.answers()];
    first.transport.close();
    // The hold of the carrier was declined, though the plan loads nothing.
    expect(saved.slice(4)).toEqual(["move|01|0", "done_loading", "move|01|1", "done_moving"]);

    // A new visit: the page does not know which answers were one plan.
    const engine: Engine = {
      module: await WebAssembly.compile(readFileSync(WASM)),
      hash: "test",
      scenarios: [],
    };
    let answers: readonly string[] = saved;
    const transport = createWasmTransport(
      engine,
      { seed: 3, players: 8, humans: 1 },
      { answers: saved, onAnswers: (all) => (answers = [...all]) },
    );
    const updates: Update[] = [];
    let wake: (() => void) | null = null;
    transport.subscribe((event) => {
      if (event.kind === "update") {
        updates.push(event);
        wake?.();
      }
    });
    const wait = async (count: number) => {
      while (updates.length < count) {
        await new Promise<void>((resume) => (wake = resume));
      }
      return updates[count - 1];
    };
    const resumed = await wait(1);
    expect(shipsIn(resumed.update, "23")).toEqual(["destroyer", "sol_carrier"]);
    transport.undo();
    const back = await wait(2);
    expect(subtype(back)).toBe("movement_step");
    expect((back.update.tactical as MovementFacts).moved).toBe(0);
    // Also in a new visit: the game that was played again says what its answers chose.
    expect(back.undone).toEqual(steps);
    expect(answers).toHaveLength(4);
    transport.close();
  },
  60_000,
);

it.skipIf(!built)(
  "runs a draft on a copy of the game, and applies it as one request",
  async () => {
    const { transport, next, nonce, answers } = await play();
    for (const answer of ["pok8imperial", "no"]) {
      transport.submitChoice(nonce(await next()), answer);
    }
    const menu = await next();
    expect(subtype(menu)).toBe("action_menu");

    // The draft starts at the choice of a system: the tactical action is its first step.
    let draft = newDraft();
    const start = transport.runDraft(scriptOf(entryOf(draft)))!;
    expect(start.stop).toEqual({ kind: "open" });
    expect(start.update.pending_choice?.nonce).toBe("draft");
    expect(start.update.tactical?.kind).toBe("activation");

    draft = activate(draft, "23");
    const activated = transport.runDraft(scriptOf(entryOf(draft)))!;
    expect(fits(entryOf(draft), activated)).toBe(true);
    const facts = activated.movement?.tactical as MovementFacts;
    expect(facts).toMatchObject({ kind: "movement", active: "23", moved: 0 });
    // The token of the draft is spent in the draft only.
    const tokens = (update: SessionUpdate) =>
      update.view.players.find((player) => player.id === "a")!.tactic_tokens;
    expect(tokens(activated.update)).toBe(tokens(menu.update) - 1);

    const steps = stepsOf(facts, setShip(facts, setShip(facts, {}, "01|0", true), "01|2", true));
    draft = setMovement(draft, steps);
    const moved = transport.runDraft(scriptOf(entryOf(draft)))!;
    expect(moved.stop).toEqual({ kind: "complete" });
    expect(fits(entryOf(draft), moved)).toBe(true);
    expect(shipsIn(moved.update, "23")).toEqual(["destroyer", "sol_carrier"]);
    // What the movement is staged against is still the fleet before it.
    expect(moved.movement?.tactical).toEqual(facts);

    // A step that the game does not offer is named, with how far the script fits.
    const bad = transport.runDraft([
      ...scriptOf(entryOf(draft)).slice(0, 2),
      {
        kind: "move",
        origin: "01",
        unit: "dreadnought",
        damaged: false,
        gravity_drive: false,
        ionian: false,
      },
      { kind: "done_moving" },
    ])!;
    expect(bad.stop).toMatchObject({ kind: "refused", step: 2 });
    expect(bad.consumed).toBe(2);

    // Nothing of this is in the game.
    expect(answers()).toHaveLength(2);

    transport.applyDraft(nonce(menu), {
      choices: ["tactical", "23"],
      plan: { destination: "23", steps },
    });
    const after = await next();
    expect(after.plan).toBeUndefined();
    expect(shipsIn(after.update, "23")).toEqual(["destroyer", "sol_carrier"]);
    expect(answers().slice(2)).toEqual([
      "tactical",
      "23",
      "move|01|0",
      "done_loading",
      "move|01|1",
      "done_moving",
    ]);
    transport.close();
  },
  60_000,
);

it.skipIf(!built)(
  "takes a draft back whole when the game does not offer one of its choices",
  async () => {
    const { transport, next, nonce, answers } = await play();
    for (const answer of ["pok8imperial", "no"]) {
      transport.submitChoice(nonce(await next()), answer);
    }
    const menu = await next();
    transport.applyDraft(nonce(menu), { choices: ["tactical", "no such system"], plan: null });
    const refused = await next();
    expect(refused.plan).toMatchObject({
      applied: 0,
      interrupted: false,
      reason: 'The game does not offer "no such system" here.',
    });
    expect(nonce(refused)).toBe(nonce(menu));
    expect(answers()).toHaveLength(2);
    transport.close();
  },
  60_000,
);

it.skipIf(!built)(
  "waits for a step before each decision of another seat, and plays the same game",
  async () => {
    const run = await play();
    const stepped = await play(true);
    const answersOf = ["pok8imperial", "no", "tactical", "23"];
    for (const answer of answersOf) {
      run.transport.submitChoice(run.nonce(await run.next()), answer);
    }
    const reached = await run.next();

    let pauses = 0;
    let given = 0;
    let event = await stepped.next();
    while (given < answersOf.length || event.stepping) {
      if (event.stepping) {
        // No choice of this side is open; the update names the seat that decides.
        pauses += 1;
        expect(event.update.pending_choice).toBeUndefined();
        expect(event.update.viewer).toEqual({ role: "player", seat: "a" });
        expect(event.stepping.seat).not.toBe("a");
        if (pauses === 3) {
          // A draft can be run while another seat decides, also while it chooses its card.
          expect(event.update.view.phase).toBe("strategy");
          const drafted = stepped.transport.runDraft([{ kind: "choose", option_id: "tactical" }]);
          expect(drafted?.stop).toEqual({ kind: "open" });
          expect(drafted?.update.tactical?.kind).toBe("activation");
          // From here on the seats play on.
          stepped.transport.setStepping(false);
        } else {
          stepped.transport.step();
        }
      } else {
        stepped.transport.submitChoice(stepped.nonce(event), answersOf[given]);
        given += 1;
      }
      event = await stepped.next();
    }
    expect(pauses).toBe(3);
    expect(event.update).toEqual(reached.update);
    run.transport.close();
    stepped.transport.close();
  },
  60_000,
);

it.skipIf(!built)("starts a scenario at the turn menu of seat a, on its board", async () => {
  const scenarios = await scenariosOf(await WebAssembly.compile(readFileSync(WASM)));
  const late = scenarios.find((scenario) => scenario.id === "late-game")!;
  expect(late).toMatchObject({ seed: 42, players: 8 });
  const { transport, next, nonce } = await play(false, {
    seed: late.seed,
    players: late.players,
    humans: 1,
    scenario: late.id,
  });
  const menu = await next();
  expect(nonce(menu)).toBe("1");
  expect(menu.update.view.round).toBe(4);
  // The fleet in the gravity rift, with its upgrades and the ground forces in its space area.
  expect(shipsIn(menu.update, "41")).toEqual([
    "cruiser2",
    "fighter2",
    "fighter2",
    "sol_carrier2",
    "sol_infantry",
    "sol_infantry",
  ]);
  transport.submitChoice(nonce(menu), "tactical");
  const activation = await next();
  const facts = activation.update.tactical;
  expect(facts?.kind).toBe("activation");
  transport.close();
});
