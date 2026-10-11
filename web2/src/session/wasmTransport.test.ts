import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";
import { setLoad, setShip } from "./movementDraft";
import { entryOf, activate, fits, newDraft, scriptOf, setMovement } from "./draft";
import { draftOf as landingOf, stepsOf as landingSteps, setLanding } from "./landingDraft";
import { type MovementStep, type PlanStep, draftOf, stepsOf } from "./movementPlan";
import { selectPlayers } from "./select/players";
import type { TransportEvent } from "./transport";
import type { LocalGame, SeatSwitch } from "./savedGame";
import { type Engine, catalogOf, createWasmTransport, scenariosOf } from "./wasmTransport";
import type { InvasionFacts, MovementFacts, SessionUpdate } from "./wire";

// The real engine, built by `scripts/build-wasm.sh` (it is not in the repository).
const WASM = fileURLToPath(new URL("./ti4.wasm", import.meta.url));
const built = existsSync(WASM);

type Update = Extract<TransportEvent, { kind: "update" }>;

/** A game of seed 3 with seat a on this side, and the updates of it, one after the other. */
async function play(
  stepping = false,
  game: LocalGame = { seed: 3, players: 8, humans: 1 },
  saved: { answers?: readonly string[]; switches?: readonly SeatSwitch[] } = {},
) {
  const engine: Engine = {
    module: await WebAssembly.compile(readFileSync(WASM)),
    hash: "test",
    scenarios: [],
    catalog: null,
  };
  let answers: readonly string[] = saved.answers ?? [];
  let switches: readonly SeatSwitch[] = saved.switches ?? [];
  const transport = createWasmTransport(engine, game, {
    ...saved,
    onAnswers: (all, changes) => {
      answers = [...all];
      switches = [...changes];
    },
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
  return { transport, next, nonce, answers: () => answers, switches: () => switches };
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
  "sends the landings as one plan, takes a refused plan back, and undoes the landings whole",
  async () => {
    const { transport, next, nonce, answers } = await play();
    for (const answer of ["pok8imperial", "no", "tactical", "23"]) {
      transport.submitChoice(nonce(await next()), answer);
    }
    // A carrier moves with two infantry to the system next to the home, which has one planet.
    const moving = await next();
    const ships = moving.update.tactical as MovementFacts;
    const infantry = ships.cargo.findIndex((pool) => pool.unit === "sol_infantry");
    transport.submitPlan(nonce(moving), {
      destination: "23",
      steps: stepsOf(ships, setLoad(ships, setShip(ships, {}, "01|0", true), "01|0", infantry, 2)),
    });
    const open = await next();
    expect(subtype(open)).toBe("commit_ground_forces");
    const facts = open.update.tactical as InvasionFacts;
    expect(facts.forces).toEqual([{ unit: "sol_infantry", damaged: false, count: 2 }]);
    const before = answers().length;
    const onPlanet = (update: SessionUpdate) =>
      (update.view.board.systems["23"]?.units ?? []).filter((unit) => unit.planet === "saudor");

    // A plan with a unit that the game does not offer, after one that it does offer.
    const good = landingSteps(facts, setLanding(facts, {}, "saudor", 0, 2));
    const mech: PlanStep = { ...(good[0] as PlanStep & { kind: "land" }), unit: "sol_mech" };
    const bad = [good[0], mech, good.at(-1)!];
    transport.submitPlan(nonce(open), { destination: "23", steps: bad });
    const refused = await next();
    expect(refused.plan).toEqual({
      destination: "23",
      applied: 0,
      remaining: bad,
      interrupted: false,
      reason: "The game does not offer to land sol_mech on saudor.",
    });
    expect(nonce(refused)).toBe(nonce(open));
    expect(answers()).toHaveLength(before);
    expect(onPlanet(refused.update)).toEqual([]);
    expect(refused.update.tactical).toEqual(facts);

    // The plan that fits. After the last unit the game does not ask for the end of the landings.
    transport.submitPlan(nonce(refused), { destination: "23", steps: good });
    const after = await next();
    expect(after.plan).toBeUndefined();
    expect(subtype(after)).not.toBe("commit_ground_forces");
    expect(answers().slice(before)).toEqual(["commit|0|saudor", "commit|0|saudor"]);
    expect(onPlanet(after.update).map((unit) => unit.unit_type)).toEqual([
      "sol_infantry",
      "sol_infantry",
    ]);

    // One undo takes the landings back whole, and tells them: they are staged again.
    transport.undo();
    const again = await next();
    expect(subtype(again)).toBe("commit_ground_forces");
    expect(nonce(again)).toBe(nonce(open));
    expect(answers()).toHaveLength(before);
    expect(again.undone).toEqual(good.slice(0, 2));
    expect(landingOf(again.update.tactical as InvasionFacts, again.undone!)).toEqual({
      saudor: { 0: 2 },
    });

    // One unit lands and one stays: the game is told the end of the landings.
    const one = landingSteps(facts, setLanding(facts, {}, "saudor", 0, 1));
    transport.submitPlan(nonce(again), { destination: "23", steps: one });
    const done = await next();
    expect(done.plan).toBeUndefined();
    expect(answers().slice(before)).toEqual(["commit|0|saudor", "done_committing"]);

    // One more undo after the landings takes the movement back whole.
    transport.undo();
    expect(subtype(await next())).toBe("commit_ground_forces");
    transport.undo();
    const movement = await next();
    expect(subtype(movement)).toBe("movement_step");
    expect(nonce(movement)).toBe(nonce(moving));
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
      catalog: null,
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

it.skipIf(!built)(
  "has the printed exploration cards, and an update says what the decks hold",
  async () => {
    const printed = await catalogOf(await WebAssembly.compile(readFileSync(WASM)));
    const catalog = printed.exploration;
    expect(Object.keys(catalog.decks).sort()).toEqual([
      "CULTURAL",
      "FRONTIER",
      "HAZARDOUS",
      "INDUSTRIAL",
    ]);
    expect(catalog.cards.find((card) => card.id === "ds")).toMatchObject({
      name: "Dyson Sphere",
      deck: "CULTURAL",
      resolution: "Attach",
    });
    const { transport, next } = await play(false, {
      seed: 42,
      players: 8,
      humans: 1,
      scenario: "late-game",
    });
    const { update } = await next();
    transport.close();
    // The scenario drew nine cards. A deck names the cards that are left by id.
    const facts = update.exploration!;
    expect(facts.drawn).toHaveLength(9);
    expect(facts.decks.CULTURAL).toHaveLength(catalog.decks.CULTURAL.length - 5);
    expect(facts.decks.CULTURAL).not.toContain("ds");
    expect(facts.explores.abyz).toEqual(["HAZARDOUS"]);
    expect(facts.explores.vegamajor).toBeUndefined();
    expect(update.view.board.systems["74"].planets.vegamajor.attachments).toEqual(["dysonsphere"]);
    expect(update.view.players.find((player) => player.id === "a")!.relic_fragments).toEqual({
      CULTURAL: 3,
      HAZARDOUS: 1,
    });
    for (const id of Object.values(facts.decks).flat()) {
      expect(catalog.cards.some((card) => card.id === id)).toBe(true);
    }
  },
);

it.skipIf(!built)(
  "has the printed cards of a play area, and an update says which are used",
  async () => {
    const catalog = await catalogOf(await WebAssembly.compile(readFileSync(WASM)));
    const { transport, next } = await play(false, {
      seed: 42,
      players: 8,
      humans: 1,
      scenario: "late-game",
    });
    const { update } = await next();
    transport.close();
    const sol = selectPlayers(update, catalog).rows.find((row) => row.seat === "a")!;
    expect(sol.abilities.map((card) => [card.name, card.window, card.state])).toEqual([
      ["Orbital Drop", "ACTION:", null],
      ["Versatile", "When you gain command tokens during the status phase:", null],
    ]);
    expect(sol.abilities[0].text).toContain("2 infantry");
    expect(sol.leaders.map((card) => [card.name, card.kind, card.state])).toEqual([
      ["Evelyn DeLouis", "Agent · Master Tactician", "exhausted"],
      ["Claire Gibson", "Commander · Voice of the People", "ready"],
      ["Jace X, 4th Air Legion", "Hero · Federation Hero", "locked"],
    ]);
    expect(sol.leaders[0].unlock).toBe("");
    expect(sol.leaders[2].unlock).toBe("Have 3 scored objectives.");
    // Gravity Drive and the unit upgrades always hold: they are not cards of the sheet.
    expect(sol.components.map((card) => [card.name, card.kind, card.state])).toEqual([
      ["Bio-Stims", "Technology", "exhausted"],
      ["Dynamis Core", "Relic", "ready"],
      ["The Prophet's Tears", "Relic", "exhausted"],
    ]);
    expect(sol.technologies).toContain("GD");
    // Without the catalogue the leaders keep their kind and their state.
    const bare = selectPlayers(update).rows.find((row) => row.seat === "a")!;
    expect(bare.leaders.map((card) => [card.name, card.state])).toEqual([
      ["Agent", "exhausted"],
      ["Commander", "ready"],
      ["Hero", "locked"],
    ]);
    expect(bare.abilities).toEqual([]);
  },
);

it.skipIf(!built)(
  "plays another seat from a decision on, several times, and a replay is the same game",
  async () => {
    const { transport, next, nonce, answers, switches } = await play();
    const seatOf = (event: Update) => {
      const { viewer, pending_choice } = event.update;
      expect(pending_choice?.choice.player).toBe(viewer.role === "player" ? viewer.seat : null);
      return pending_choice?.choice.player;
    };
    /** Answers the open choice by a fixed rule, and gives the update after it. */
    const answer = async (event: Update) => {
      const { options } = event.update.pending_choice!.choice;
      transport.submitChoice(nonce(event), options[(answers().length * 7) % options.length].id);
      return next();
    };
    let open = await next();
    for (let count = 0; count < 3; count += 1) {
      expect(seatOf(open)).toBe("a");
      open = await answer(open);
    }

    // Seat c is played here. The open decision of seat a is not an answer of the page.
    transport.switchSeat("c");
    const first = await next();
    expect(seatOf(first)).toBe("c");
    expect(answers()).toHaveLength(3);
    // An undo does not give seat a back.
    expect(first.canUndo).toBe(false);
    transport.undo();
    expect(answers()).toHaveLength(3);
    open = await answer(first);
    expect(seatOf(open)).toBe("c");
    expect(open.canUndo).toBe(true);
    transport.undo();
    const again = await next();
    expect(again.update).toEqual(first.update);
    expect(again.canUndo).toBe(false);
    open = await answer(again);

    // Two changes with no answer between them, then back to the seat of the start.
    transport.switchSeat("e");
    expect(seatOf(await next())).toBe("e");
    transport.switchSeat("b");
    open = await next();
    expect(seatOf(open)).toBe("b");
    expect(open.canUndo).toBe(false);
    open = await answer(open);
    expect(seatOf(open)).toBe("b");
    // A seat that is played here already is no change.
    transport.switchSeat("b");
    expect(switches()).toHaveLength(3);
    transport.switchSeat("a");
    open = await next();
    expect(seatOf(open)).toBe("a");
    const reached = await answer(open);
    expect(seatOf(reached)).toBe("a");
    expect(switches().map((item) => [item.humans, item.answers])).toEqual([
      [0b100, 3],
      [0b10000, 4],
      [0b10, 4],
      [0b1, 5],
    ]);
    transport.close();

    // A new visit: the answers and the seat changes play the same game.
    const visit = await play(false, undefined, { answers: answers(), switches: switches() });
    const resumed = await visit.next();
    expect(resumed.update).toEqual(reached.update);
    expect(resumed.canUndo).toBe(true);
    // The undo of a new visit plays the game again, with its seat changes.
    visit.transport.undo();
    const back = await visit.next();
    expect(back.update).toEqual(open.update);
    expect(back.canUndo).toBe(false);
    visit.transport.close();
  },
);

it.skipIf(!built)(
  "takes the seat that the game waits for, while the seats are stepped",
  async () => {
    const { transport, next, nonce, answers, switches } = await play(true);
    const menu = await next();
    transport.submitChoice(nonce(menu), menu.update.pending_choice!.choice.options[0].id);
    const paused = await next();
    const seat = paused.stepping!.seat;
    expect(seat).not.toBe("a");
    expect(paused.update.pending_choice).toBeUndefined();
    // The decision that the game waited to draw is asked here.
    transport.switchSeat(seat);
    const asked = await next();
    expect(asked.stepping).toBeUndefined();
    expect(asked.update.viewer).toEqual({ role: "player", seat });
    expect(asked.update.pending_choice?.choice.player).toBe(seat);
    expect(asked.canUndo).toBe(false);
    transport.close();

    // Played again without steps, the change is at the same decision.
    const visit = await play(false, undefined, { answers: answers(), switches: switches() });
    expect((await visit.next()).update).toEqual(asked.update);
    visit.transport.close();
  },
);
