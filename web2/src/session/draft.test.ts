import { expect, it } from "vitest";
import {
  activate,
  answer,
  canRedo,
  canUndo,
  entryOf,
  fits,
  frontier,
  isBoundary,
  newDraft,
  problemOf,
  readDraft,
  redo,
  scriptOf,
  setLandings,
  setMovement,
  setProduction,
  stageAfter,
  stageAt,
  startOver,
  storeDraft,
  undo,
} from "./draft";
import type { LandingStep, MovementStep, ProductionStep } from "./movementPlan";
import type { DraftOutcome, DraftStop, SessionUpdate } from "./wire";

const MOVE: MovementStep[] = [
  {
    kind: "move",
    origin: "01",
    unit: "destroyer",
    damaged: false,
    gravity_drive: false,
    ionian: false,
  },
  { kind: "done_moving" },
];
const outcome = (stop: DraftStop, consumed: number): DraftOutcome => ({
  update: {} as SessionUpdate,
  consumed,
  stop,
});

it("is a script: the tactical action, its system, its movement", () => {
  let draft = newDraft();
  expect(scriptOf(entryOf(draft))).toEqual([{ kind: "choose", option_id: "tactical" }]);
  draft = activate(draft, "23");
  expect(scriptOf(entryOf(draft))).toEqual([
    { kind: "choose", option_id: "tactical" },
    { kind: "choose", option_id: "23" },
  ]);
  draft = setMovement(draft, MOVE);
  expect(scriptOf(entryOf(draft))).toEqual([
    { kind: "choose", option_id: "tactical" },
    { kind: "choose", option_id: "23" },
    ...MOVE,
  ]);
});

it("undoes and redoes each change, and a change drops what redo had", () => {
  let draft = setMovement(activate(newDraft(), "23"), MOVE);
  expect(canUndo(draft)).toBe(true);
  expect(canRedo(draft)).toBe(false);
  draft = undo(draft);
  expect(entryOf(draft)).toEqual({ system: "23", steps: null });
  draft = undo(draft);
  expect(entryOf(draft)).toEqual({ system: null, steps: null });
  expect(canUndo(draft)).toBe(false);
  expect(undo(draft)).toBe(draft);
  draft = redo(redo(draft));
  expect(entryOf(draft)).toEqual({ system: "23", steps: MOVE });
  // Another system: the movement of the first one is gone, also for redo.
  draft = activate(undo(draft), "24");
  expect(canRedo(draft)).toBe(false);
  expect(entryOf(draft)).toEqual({ system: "24", steps: null });
  // Start over is a change too: undo brings the draft back.
  draft = startOver(draft);
  expect(entryOf(draft).system).toBeNull();
  expect(entryOf(undo(draft)).system).toBe("24");
  expect(startOver(newDraft())).toEqual(newDraft());
});

it("says whether the game played the whole script, and why not", () => {
  const entry = entryOf(setMovement(activate(newDraft(), "23"), MOVE));
  expect(fits(entry, outcome({ kind: "complete" }, 4))).toBe(true);
  expect(problemOf(entry, outcome({ kind: "complete" }, 4))).toBeNull();
  expect(fits(entry, outcome({ kind: "open" }, 3))).toBe(false);
  const refused = (step: number) =>
    outcome({ kind: "refused", step, reason: "The game does not offer this." }, step);
  expect(fits(entry, refused(2))).toBe(false);
  expect(problemOf(entry, refused(0))).toContain("cannot be drafted from here");
  expect(problemOf(entry, refused(1))).toBe("System 23 cannot be activated now.");
  expect(problemOf(entry, refused(2))).toBe(
    "A part of the movement no longer fits. The game does not offer this.",
  );
  expect(problemOf(entry, outcome({ kind: "stopped", reason: "Uncertainty" }, 3))).toContain(
    "a roll or a draw",
  );
});

it("is kept between two visits, with its history", () => {
  const stored = new Map<string, string>();
  const storage = {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => void stored.set(key, value),
    removeItem: (key: string) => void stored.delete(key),
  };
  expect(readDraft(storage, "draft")).toBeNull();
  const draft = undo(setMovement(activate(newDraft(), "23"), MOVE));
  storeDraft(storage, "draft", draft);
  expect(readDraft(storage, "draft")).toEqual(draft);
  stored.set("draft", '{"history":[],"cursor":0}');
  expect(readDraft(storage, "draft")).toBeNull();
  storeDraft(storage, "draft", null);
  expect(stored.has("draft")).toBe(false);
});

const LAND: LandingStep[] = [
  {
    kind: "land",
    planet: "saudor",
    unit: "infantry",
    damaged: false,
    from_system: null,
    from_planet: null,
  },
  { kind: "done_committing" },
];
const BUILD: ProductionStep[] = [
  { kind: "produce", unit: "cruiser" },
  { kind: "pay", source: "exhaust|jord" },
  { kind: "done_producing" },
];
const choose = (option_id: string) => ({ kind: "choose", option_id });

it("goes on to the landings and the production, with what the game asks before them", () => {
  let draft = setMovement(activate(newDraft(), "23"), MOVE);
  draft = setLandings(draft, LAND);
  draft = answer(draft, "production", "decline");
  draft = setProduction(draft, BUILD, true);
  const entry = entryOf(draft);
  expect(scriptOf(entry)).toEqual([
    choose("tactical"),
    choose("23"),
    ...MOVE,
    ...LAND,
    choose("decline"),
    ...BUILD,
  ]);
  expect([1, 2, 3, 4, 5, 6, 7, 9].map((index) => stageAt(entry, index))).toEqual([
    null,
    "movement",
    "movement",
    "invasion",
    "invasion",
    "production",
    "production",
    "production",
  ]);
  expect(problemOf(entry, outcome({ kind: "refused", step: 4, reason: "Gone." }, 4))).toBe(
    "A part of the landings no longer fits. Gone.",
  );
  // A change of the movement keeps what comes after it; another system does not.
  expect(entryOf(setMovement(draft, [{ kind: "done_moving" }])).landings).toEqual(LAND);
  expect(entryOf(activate(draft, "24"))).toEqual({ system: "24", steps: null });
  // A step that is not drafted adds nothing: the run is open at its first question.
  const skipped = entryOf(
    setProduction(setMovement(activate(newDraft(), "23"), MOVE), BUILD, true),
  );
  expect(scriptOf(skipped)).toEqual([choose("tactical"), choose("23"), ...MOVE, ...BUILD]);
});

it("keeps a production that is not paid, and does not ask the game for it", () => {
  let draft = setMovement(activate(newDraft(), "23"), MOVE);
  draft = setProduction(draft, BUILD.slice(0, 1), false);
  expect(entryOf(draft).production).toEqual(BUILD.slice(0, 1));
  expect(scriptOf(entryOf(draft))).toEqual([choose("tactical"), choose("23"), ...MOVE]);
  draft = setProduction(draft, BUILD, true);
  expect(scriptOf(entryOf(draft))).toHaveLength(2 + MOVE.length + BUILD.length);
  expect(entryOf(undo(draft)).unpaid).toBe(true);
});

it("shows the step where the run is open, or the last one that it reached", () => {
  const asks = (subtype: string) =>
    ({ pending_choice: { nonce: "draft", choice: { context: { subtype } } } }) as SessionUpdate;
  const at = (stop: DraftStop, update: SessionUpdate, reached: string[]): DraftOutcome => ({
    update,
    consumed: 0,
    stop,
    ...Object.fromEntries(reached.map((stage) => [stage, asks(stage)])),
  });
  const open: DraftStop = { kind: "open" };
  expect(frontier(at(open, asks("commit_ground_forces"), ["movement", "invasion"]))).toBe(
    "invasion",
  );
  expect(frontier(at(open, asks("produce_unit"), ["movement", "production"]))).toBe("production");
  // A question of an ability is no step of the action.
  expect(frontier(at(open, asks("exhaust_for_production_discount"), ["movement"]))).toBeNull();
  const rolled = at({ kind: "stopped", reason: "Uncertainty" }, {} as SessionUpdate, [
    "movement",
    "invasion",
  ]);
  expect(frontier(rolled)).toBe("invasion");
  expect(isBoundary(rolled)).toBe(true);
  expect(stageAfter(rolled, "movement")).toBe("invasion");
  expect(stageAfter(rolled, "invasion")).toBeNull();
  const done = at({ kind: "complete" }, {} as SessionUpdate, ["movement", "production"]);
  expect(frontier(done)).toBe("production");
  expect(stageAfter(done, "movement")).toBe("production");
  expect(isBoundary(done)).toBe(false);
});
