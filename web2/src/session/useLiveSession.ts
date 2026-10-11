import { useEffect, useMemo, useState } from "react";
import type { GameSession, Intent } from "../model";
import {
  type Draft,
  activate,
  canRedo,
  canUndo,
  entryOf,
  fits,
  newDraft,
  problemOf,
  readDraft as readPlan,
  redo,
  scriptOf,
  setMovement,
  startOver,
  storeDraft as storePlan,
  undo,
} from "./draft";
import {
  type LandingDraft,
  chosenForce,
  forceOfId,
  draftOf as landingOf,
  stepsOf as landingSteps,
  setCount as setLandingCount,
  setLanding,
} from "./landingDraft";
import { type MovementDraft, fill, poolOfSource, resetOrigin, setCount } from "./movementDraft";
import { type MovementStep, type PlanStep, draftOf, stepsOf } from "./movementPlan";
import {
  GOODS,
  EMPTY as NO_PRODUCTION,
  type ProductionDraft,
  autoPay,
  clearPayment,
  isEmpty,
  overrunOf,
  draftOf as productionOf,
  stepsOf as productionSteps,
  setGoods,
  setCount as setProductionCount,
  togglePlanet,
  totals,
} from "./productionDraft";
import type { SaveStorage } from "./savedGame";
import type { DraftInfo, LocalState } from "./select/action";
import { systemLabel } from "./select/movement";
import { selectShell } from "./select/shell";
import { tacticalFacts } from "./select/tactical";
import type { Transport } from "./transport";
import type { DraftOutcome, MovementFacts, SessionUpdate } from "./wire";

const NOTHING: LocalState = {
  inspected: null,
  staged: null,
  sent: null,
  error: null,
  replaying: null,
  canUndo: false,
  movement: {},
  landing: {},
  landingKind: null,
  production: NO_PRODUCTION,
  productionStage: "build",
  handled: [],
  step: null,
  remaining: null,
  planNote: null,
  stepping: null,
  draftLocked: null,
  draft: null,
};

/** The label of a system as the activation step lists it, for the search field. */
const sameSystem = (query: string, id: string) =>
  query.trim() === id || query.trim().endsWith(`#${id}`);

/** Where the staged movement is kept between two visits. */
export interface DraftStore {
  storage: SaveStorage;
  key: string;
  /** Where the private draft of a tactical action is kept. Without it a reload loses the draft. */
  planKey?: string;
}

/** The private draft of the page, and whether it is in view. */
interface Plan {
  draft: Draft | null;
  open: boolean;
}

const readPlanOf = (drafts: DraftStore | undefined): Plan => ({
  draft: drafts?.planKey ? readPlan(drafts.storage, drafts.planKey) : null,
  open: false,
});

/** Why the viewer cannot draft a tactical action now. Null when the Draft tab opens. */
function draftLock(update: SessionUpdate, local: LocalState): string | null {
  const seat = update.viewer.role === "player" ? update.viewer.seat : null;
  if (local.error || local.replaying || update.view.finished) {
    return "The game does not wait for you";
  }
  if (seat === null) {
    return "A draft is private to a player";
  }
  // In the strategy phase the draft is of the turn that comes: the board is the same.
  if (update.view.phase !== "action" && update.view.phase !== "strategy") {
    return "A draft is of the action phase";
  }
  if (update.view.players.find((player) => player.id === seat)?.passed) {
    return "You have passed";
  }
  return null;
}

/** The turn menu of the viewer, with the tactical action in it: where a draft is sent. */
const menuNonce = (update: SessionUpdate, local: LocalState): string | null => {
  const pending = update.pending_choice;
  return pending &&
    pending.nonce !== local.sent &&
    pending.choice.context?.subtype === "action_menu" &&
    pending.choice.options.some((option) => option.id === "tactical")
    ? pending.nonce
    : null;
};

/** What "Apply to Live" sends, in words. */
function applyItems(update: SessionUpdate, system: string, steps: MovementStep[] | null) {
  const tiles = new Map((update.view.board.map_tiles ?? []).map((tile) => [tile.system_id, tile]));
  const items = [`Activate ${systemLabel(tiles, system)}`];
  if (steps === null) {
    return [...items, "The movement is decided in Live"];
  }
  const origins = new Map<string, { ships: number; units: number }>();
  let last: { ships: number; units: number } | null = null;
  for (const step of steps) {
    if (step.kind === "move") {
      last = origins.get(step.origin) ?? { ships: 0, units: 0 };
      last.ships += 1;
      origins.set(step.origin, last);
    } else if (step.kind === "load" && last) {
      last.units += 1;
    }
  }
  for (const [origin, { ships, units }] of origins) {
    const cargo = units ? ` with ${units} ${units === 1 ? "unit" : "units"}` : "";
    items.push(
      `${ships} ${ships === 1 ? "ship" : "ships"} from ${systemLabel(tiles, origin)}${cargo}`,
    );
  }
  return origins.size ? items : [...items, "No ship moves"];
}

/** The draft in view: the update that its screens are made from, and what its toolbar says. */
function draftView(
  live: SessionUpdate,
  local: LocalState,
  draft: Draft,
  outcome: DraftOutcome,
): { update: SessionUpdate; local: LocalState; facts: MovementFacts | null } {
  const entry = entryOf(draft);
  const problem = problemOf(entry, outcome);
  const staged = entry.system !== null ? outcome.movement : undefined;
  const update = staged ?? outcome.update;
  const facts = staged?.tactical?.kind === "movement" ? staged.tactical : null;
  // A roll ends the check, not the draft: the game goes on from there in Live.
  const sound =
    fits(entry, outcome) ||
    (outcome.stop.kind === "stopped" && outcome.stop.reason === "Uncertainty");
  const atMenu = menuNonce(live, local) !== null;
  const canApply = entry.system !== null && sound && atMenu;
  const info: DraftInfo = {
    canUndo: canUndo(draft),
    canRedo: canRedo(draft),
    canApply,
    applyHint:
      entry.system === null
        ? "Choose a system first"
        : !sound
          ? (problem ?? "The draft does not fit the game")
          : !atMenu
            ? "Apply when you choose your action"
            : "Apply the recorded choices to the live game",
    apply:
      canApply && entry.system !== null
        ? {
            items: applyItems(update, entry.system, entry.steps),
            cost: "Costs 1 tactic token.",
            note: "Dice and later decisions continue in Live.",
          }
        : null,
    problem,
  };
  return {
    update,
    facts,
    local: {
      ...local,
      sent: null,
      canUndo: false,
      movement: facts ? draftOf(facts, entry.steps ?? []) : {},
      remaining: null,
      planNote: facts && problem ? { text: problem, error: outcome.stop.kind === "refused" } : null,
      stepping: null,
      draft: info,
    },
  };
}

/**
 * The staged movement of one movement step, as the steps that "Move fleet" would send, or the
 * staged landings of one invasion step.
 */
interface SavedDraft {
  nonce: string;
  destination: string;
  steps: PlanStep[];
}

function readDraft(store: DraftStore | undefined): SavedDraft | null {
  try {
    const value = JSON.parse(store?.storage.getItem(store.key) ?? "null");
    return value && Array.isArray(value.steps) ? value : null;
  } catch {
    return null;
  }
}

export interface LiveSession {
  /** Null until the first update. */
  session: GameSession | null;
  /** Why the game stopped, or cannot start. */
  error: string | null;
  /** A saved game is played again: how many of its answers are done. */
  replaying: { done: number; total: number } | null;
}

/**
 * A `GameSession` on a transport: the game comes from the updates, and what the player stages
 * between two updates is kept here. Only the main button sends something.
 * With `drafts`, a staged movement is still staged after a reload.
 */
export function useLiveSession(transport: Transport | null, drafts?: DraftStore): LiveSession {
  const [update, setUpdate] = useState<SessionUpdate | null>(null);
  const [local, setLocal] = useState<LocalState>(NOTHING);
  const [plan, setPlan] = useState<Plan>(() => readPlanOf(drafts));

  useEffect(() => {
    setUpdate(null);
    setLocal(NOTHING);
    setPlan(readPlanOf(drafts));
    /** The step and the system whose sheet (the landings, the build) opened with the last update. */
    let sheetAt: { key: string; system: string } | null = null;
    /** What the inspector showed before the landings opened. */
    let inspectedBefore: string | null = null;
    return transport?.subscribe((event) => {
      if (event.kind === "error") {
        setLocal((now) => ({ ...now, staged: null, replaying: null, error: event.message }));
        return;
      }
      if (event.kind === "replaying") {
        const { done, total } = event;
        setLocal((now) => ({
          ...now,
          staged: null,
          sent: null,
          error: null,
          replaying: { done, total },
        }));
        return;
      }
      setUpdate(event.update);
      // The landings open at the active system when their step opens, and so does the build of
      // a production.
      const tactical = event.update.tactical;
      const sheet =
        tactical?.kind === "invasion"
          ? { key: `invasion:${tactical.active}`, system: tactical.active }
          : tactical?.kind === "production"
            ? { key: `production:${tactical.system}`, system: tactical.system }
            : null;
      const opens = sheet !== null && sheetAt?.key !== sheet.key;
      // When their step is over (sent, or taken back), the board is where the player left it.
      const closes = sheet === null ? (sheetAt?.system ?? null) : null;
      sheetAt = sheet;
      // A new choice starts with nothing staged. The inspector stays where the player left it.
      setLocal((now) => {
        const facts = event.update.tactical;
        const stopped = event.plan;
        // A movement that another decision stopped is staged again when the movement goes on.
        let remaining =
          stopped?.interrupted && stopped.remaining.length
            ? { destination: stopped.destination, steps: stopped.remaining }
            : now.remaining;
        // A movement that an undo took back is staged again, in front of what was still to move.
        const undone = event.undone;
        const active =
          facts?.kind === "movement" ? facts.active : event.update.view.board.active_system;
        if (undone && active) {
          const rest = remaining?.destination === active ? remaining.steps : [];
          remaining = { destination: active, steps: [...undone, ...rest] };
        }
        let movement = {};
        let landing: LandingDraft = {};
        let production: ProductionDraft = NO_PRODUCTION;
        let planNote =
          stopped && !stopped.interrupted ? { text: stopped.reason, error: true } : null;
        if (facts && facts.kind !== "activation") {
          const place = facts.kind === "production" ? facts.system : facts.active;
          const again = stopped && !stopped.interrupted ? stopped.remaining : remaining?.steps;
          // The steps that were staged for this choice before a reload.
          const saved = again ? null : readDraft(drafts);
          const steps =
            again && (stopped?.destination ?? remaining?.destination) === place
              ? again
              : saved?.nonce === event.update.pending_choice?.nonce && saved?.destination === place
                ? saved.steps
                : null;
          if (steps && facts.kind === "movement") {
            movement = draftOf(facts, steps);
          } else if (steps && facts.kind === "invasion") {
            landing = landingOf(facts, steps);
          } else if (steps && facts.kind === "production") {
            production = productionOf(facts, steps);
          }
          if (steps && steps === again && !undone) {
            planNote ??= {
              text:
                facts.kind === "movement"
                  ? "Another decision stopped the movement. What did not move is staged again."
                  : facts.kind === "invasion"
                    ? "Another decision stopped the landings. What did not land is staged again."
                    : "Another decision stopped the production. What was not produced is staged again.",
              error: false,
            };
          }
          remaining = null;
        } else if (facts?.kind === "activation") {
          remaining = null;
        }
        if (opens) {
          inspectedBefore = now.inspected;
        }
        return {
          ...now,
          inspected: opens
            ? (sheet?.system ?? null)
            : closes !== null && now.inspected === closes
              ? inspectedBefore
              : now.inspected,
          staged: null,
          sent: null,
          error: null,
          replaying: null,
          canUndo: event.canUndo,
          movement,
          landing,
          landingKind: null,
          production,
          productionStage: "build",
          handled: [],
          step: null,
          remaining,
          planNote,
          stepping: event.stepping?.seat ?? null,
        };
      });
    });
  }, [transport, drafts]);

  // The staged movement is kept for a reload, with the choice that it belongs to. So are the
  // staged landings.
  useEffect(() => {
    const facts = update?.tactical;
    const nonce = update?.pending_choice?.nonce;
    if (!update || !drafts) {
      return;
    }
    const steps =
      facts?.kind === "movement" && Object.keys(local.movement).length
        ? stepsOf(facts, local.movement)
        : facts?.kind === "invasion" && Object.keys(local.landing).length
          ? landingSteps(facts, local.landing)
          : facts?.kind === "production" && !isEmpty(local.production)
            ? productionSteps(facts, local.production)
            : null;
    if (facts && facts.kind !== "activation" && nonce && steps) {
      const destination = facts.kind === "production" ? facts.system : facts.active;
      const saved: SavedDraft = { nonce, destination, steps };
      drafts.storage.setItem(drafts.key, JSON.stringify(saved));
    } else {
      drafts.storage.removeItem(drafts.key);
    }
  }, [update, local.movement, local.landing, local.production, drafts]);

  // The private draft is kept for a reload, with its undo history.
  useEffect(() => {
    if (update && drafts?.planKey) {
      storePlan(drafts.storage, drafts.planKey, plan.draft);
    }
  }, [update, plan.draft, drafts]);

  // The draft is played again on a copy of the game, for each change of it and of the game.
  const entry = plan.draft ? entryOf(plan.draft) : null;
  const locked = update ? draftLock(update, local) : "The game has not started";
  const outcome = useMemo((): DraftOutcome | null => {
    if (!transport || !update || !entry || !plan.open || locked) {
      return null;
    }
    try {
      return transport.runDraft(scriptOf(entry));
    } catch (error) {
      return {
        update,
        consumed: 0,
        stop: { kind: "failed", reason: error instanceof Error ? error.message : String(error) },
      };
    }
  }, [transport, update, entry, plan.open, locked]);

  const session = useMemo((): GameSession | null => {
    if (!update || !transport) {
      return null;
    }
    const live: LocalState = { ...local, draftLocked: locked, draft: null };
    const drafted = plan.draft && outcome ? draftView(update, live, plan.draft, outcome) : null;
    const shown = drafted?.update ?? update;
    const state = drafted?.local ?? live;
    const pending = state.sent === shown.pending_choice?.nonce ? null : shown.pending_choice;
    /** Opens or closes the draft. What was staged for the other workspace is dropped. */
    const workspace = (open: boolean, draft: Draft | null) => {
      setPlan({ open, draft });
      setLocal((now) => ({ ...now, staged: null, inspected: null, handled: [], step: null }));
    };
    /** A change of the draft: one step of its undo. */
    const redraft = (change: (now: Draft) => Draft) =>
      setPlan((now) => (now.draft ? { ...now, draft: change(now.draft) } : now));
    const stage = (option: string) => {
      if (pending?.choice.options.some((item) => item.id === option)) {
        setLocal((now) => ({ ...now, staged: now.staged === option ? null : option }));
      }
    };
    const facts = tacticalFacts(shown, state);
    const moving = facts?.kind === "movement" ? facts : null;
    const landing = facts?.kind === "invasion" && !drafted ? facts : null;
    const producing = facts?.kind === "production" && !drafted ? facts : null;
    /** A change of the staged production. It clears the note of the last plan. */
    const produce = (change: (now: ProductionDraft) => ProductionDraft) =>
      setLocal((now) => ({ ...now, production: change(now.production), planNote: null }));
    /** The build is chosen: the payment is next, when there is something to pay. */
    const toPayment = () => {
      const { units, cost } = producing
        ? totals(producing, local.production)
        : { units: 0, cost: 0 };
      // A build over a limit stays open: the gauges and the footer say what is over.
      if (producing && overrunOf(producing, local.production) > 0) {
        return true;
      }
      const pays = units > 0 && cost > 0;
      setLocal((now) => ({ ...now, inspected: null, productionStage: pays ? "pay" : "build" }));
      return pays;
    };
    /** Back to the build of the system that produces: its sheet opens. */
    const toBuild = (system: string) =>
      setLocal((now) => ({ ...now, inspected: system, productionStage: "build" }));
    /** A change of the staged landings. It clears the note of the last plan. */
    const land = (change: (now: LandingDraft) => LandingDraft) =>
      setLocal((now) => ({ ...now, landing: change(now.landing), planNote: null }));
    /**
     * A change of the staged movement. Live, it clears the note of the last plan. A draft
     * records it at once, as the steps that the game is asked for.
     */
    const draft = (change: (now: MovementDraft) => MovementDraft) => {
      if (!drafted) {
        return setLocal((now) => ({ ...now, movement: change(now.movement), planNote: null }));
      }
      if (moving) {
        const steps = stepsOf(moving, change(state.movement));
        if (JSON.stringify(steps) !== JSON.stringify(stepsOf(moving, state.movement))) {
          redraft((now) => setMovement(now, steps));
        }
      }
    };
    const unmark = (keep: (system: string) => boolean) =>
      setLocal((now) => ({ ...now, handled: now.handled.filter(keep) }));
    const send = () => {
      if (!pending) {
        return;
      }
      if (drafted) {
        // The activation of a draft is recorded. Its movement has nothing to send.
        const system = facts?.kind === "activation" ? state.staged : null;
        if (system) {
          redraft((now) => activate(now, system));
          setLocal((now) => ({ ...now, staged: null, inspected: null }));
        }
        return;
      }
      if (producing) {
        const { units, cost, paid } = totals(producing, local.production);
        if (local.productionStage === "build" && toPayment()) {
          return;
        }
        if ((units > 0 && paid < cost) || overrunOf(producing, local.production) > 0) {
          return;
        }
        // Nothing is paid for nothing: a payment that is still staged is not sent.
        const staged =
          units === 0
            ? NO_PRODUCTION
            : cost === 0
              ? clearPayment(local.production)
              : local.production;
        transport.submitPlan(pending.nonce, {
          destination: producing.system,
          steps: productionSteps(producing, staged),
        });
        return setLocal((now) => ({ ...now, sent: pending.nonce, planNote: null }));
      }
      if (moving || landing) {
        transport.submitPlan(
          pending.nonce,
          moving
            ? { destination: moving.active, steps: stepsOf(moving, local.movement) }
            : { destination: landing!.active, steps: landingSteps(landing!, local.landing) },
        );
        return setLocal((now) => ({ ...now, sent: pending.nonce, planNote: null }));
      }
      if (local.staged) {
        transport.submitChoice(pending.nonce, local.staged);
        setLocal((now) => ({ ...now, staged: null, sent: pending.nonce }));
      }
    };
    const apply = () => {
      const nonce = menuNonce(update, live);
      const system = plan.draft ? entryOf(plan.draft).system : null;
      if (!drafted?.local.draft?.canApply || !plan.draft || nonce === null || system === null) {
        return;
      }
      const { steps } = entryOf(plan.draft);
      transport.applyDraft(nonce, {
        choices: ["tactical", system],
        plan: steps ? { destination: system, steps } : null,
      });
      workspace(false, null);
      setLocal((now) => ({ ...now, sent: nonce, planNote: null }));
    };
    const dispatch = (intent: Intent) => {
      switch (intent.type) {
        case "workspace":
          if (intent.mode === "live") {
            return workspace(false, plan.draft);
          }
          return intent.mode === "draft" && !locked
            ? workspace(true, plan.draft ?? newDraft())
            : undefined;
        case "redo":
          return redraft(redo);
        case "startOver":
          return redraft(startOver);
        case "discardDraft":
          return workspace(false, null);
        case "confirmApply":
          return apply();
        case "simulate":
          return transport.step();
        case "inspectSystem":
          // The system that produces opens its build, also from the payment.
          return producing?.system === intent.system
            ? toBuild(intent.system)
            : setLocal((now) => ({ ...now, inspected: intent.system }));
        case "closeInspector":
          return setLocal((now) => ({ ...now, inspected: null }));
        case "chooseOption": {
          // The kind of ground force that a tap lands is a choice of the hand.
          const force = intent.group === "landing" ? forceOfId(intent.option) : null;
          return landing && force !== null
            ? setLocal((now) => ({ ...now, landingKind: force }))
            : stage(intent.option);
        }
        case "togglePlanet": {
          // A planet of the active system: one more unit of the chosen kind lands there.
          const { planet } = intent;
          if (producing) {
            // A planet pays, once the build is chosen.
            return (
              local.productionStage === "pay" &&
              produce((now) => togglePlanet(producing, now, planet))
            );
          }
          return (
            landing &&
            land((now) => {
              const force = chosenForce(landing, now, local.landingKind, planet);
              return force === null
                ? now
                : setLanding(landing, now, planet, force, (now[planet]?.[force] ?? 0) + 1);
            })
          );
        }
        case "setPayment":
          return (
            producing &&
            intent.source === GOODS &&
            produce((now) => setGoods(producing, now, intent.value))
          );
        case "payment":
          return (
            producing &&
            produce((now) =>
              intent.action === "auto" ? autoPay(producing, now) : clearPayment(now),
            )
          );
        case "toggleSystem":
          // In a movement the fleet of a system opens at the system, and closes there. So does
          // the build of a production.
          return moving || producing
            ? setLocal((now) => ({
                ...now,
                inspected: now.inspected === intent.system ? null : intent.system,
              }))
            : stage(intent.system);
        case "findSystem": {
          const found = pending?.choice.options.find((item) => sameSystem(intent.query, item.id));
          if (found && facts?.kind === "activation") {
            setLocal((now) => ({ ...now, staged: found.id }));
          }
          return;
        }
        case "clearChoice":
          // Esc in the payment of a production goes back to its build.
          return producing && local.productionStage === "pay"
            ? toBuild(producing.system)
            : setLocal((now) => ({ ...now, staged: null }));
        case "undo":
          return drafted ? redraft(undo) : transport.undo();
        case "flow":
          return intent.action === "resolve" && !facts && !drafted ? send() : undefined;
        // The steps of a tactical action.
        case "commitEdit":
          return facts ? send() : undefined;
        case "selectStep":
          return setLocal((now) => ({ ...now, step: moving && intent.step === 0 ? 0 : null }));
        case "returnToCurrent":
          return setLocal((now) => ({ ...now, step: null }));
        case "setCount":
          if (producing) {
            return produce((now) => setProductionCount(producing, now, intent.key, intent.value));
          }
          if (landing) {
            return land((now) => setLandingCount(landing, now, intent.key, intent.value));
          }
          return moving && draft((now) => setCount(moving, now, intent.key, intent.value));
        case "fillHold": {
          const pool = poolOfSource(intent.source);
          const origin = intent.key.startsWith("origin:") ? intent.key.slice(7) : null;
          return (
            moving &&
            pool !== null &&
            origin !== null &&
            draft((now) => fill(moving, now, origin, pool))
          );
        }
        case "resetOrigin":
          if (producing) {
            return produce(() => NO_PRODUCTION);
          }
          if (landing) {
            return land(() => ({}));
          }
          unmark((system) => system !== intent.system);
          return moving && draft((now) => resetOrigin(now, intent.system));
        case "markOrigin":
          if (producing) {
            return void toPayment();
          }
          if (landing) {
            return setLocal((now) => ({ ...now, inspected: null }));
          }
          return (
            moving &&
            setLocal((now) => ({
              ...now,
              inspected: null,
              handled: [...new Set([...now.handled, intent.system])],
            }))
          );
        case "cancelEdit":
          if (producing) {
            produce(() => NO_PRODUCTION);
            return setLocal((now) => ({ ...now, productionStage: "build" }));
          }
          if (landing) {
            return land(() => ({}));
          }
          unmark(() => false);
          return moving && draft(() => ({}));
        default:
          // The other intents belong to screens that local play does not have yet.
          return;
      }
    };
    return { view: selectShell(shown, state, transport?.catalog ?? null), dispatch };
  }, [update, local, transport, plan, outcome, locked]);

  return { session, error: local.error, replaying: local.replaying };
}
