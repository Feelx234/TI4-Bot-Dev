import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  BoardView,
  GameEvent,
  HistoryStatus,
  PendingChoiceDto,
  PlayerView,
} from "../protocol/types.ts";
import type { BasketPlan } from "../protocol/client.ts";
import { TOKEN_POOLS, type TokenStep } from "../presentation/commandTokens.ts";
import {
  detectStrategicAction,
  prepareEligibility,
} from "../presentation/strategicAction.ts";
import {
  isSecondaryQuestion,
  resolveStep,
  type SecondaryPlan,
  type StructureUnit,
  type StepResolution,
} from "../presentation/secondaryPlan.ts";
import {
  buildDryChoice,
  nextDryStep,
  type DryChoice,
  type DryStep,
} from "../presentation/dryChoice.ts";
import { usePreparedPlan } from "./useSecondaryPlan.ts";
import { useSecondaryAutoPlay } from "./useSecondaryAutoPlay.ts";

export interface UseSecondaryPrepareInput {
  gameId?: string;
  viewerSeat?: string | null;
  players: readonly PlayerView[];
  events: readonly GameEvent[];
  board?: BoardView;
  phase?: string;
  activePlayer?: string | null;
  history?: HistoryStatus;
  /** The engine's own pending decision (never the dry one). */
  realChoice: PendingChoiceDto | null;
  /** A pipeline or a history change is busy: nothing is sent on the player's behalf. */
  busy?: boolean;
  submitChoice: (optionId: string) => Promise<void>;
  submitBatch?: (plan: BasketPlan) => Promise<void>;
}

/**
 * Preparing a strategy-card secondary with the usual UI.
 *
 * While another seat resolves a card the viewer has not been asked about, the viewer can open
 * "preparation mode": the app then shows a synthesized stand-in (a dry choice) for each question of
 * the card in place of a pending decision, so the real components render it and the map highlights
 * its planets. Answering one calls `submitChoice`/`submitBatch` of THIS hook, which record the
 * semantic plan on the device instead of sending anything.
 *
 * When the real question opens the plan is validated against its options and either offered for
 * one click (review), played in the background (auto) or flagged "Needs review".
 */
export function useSecondaryPrepare({
  gameId,
  viewerSeat,
  players,
  events,
  board,
  phase,
  activePlayer,
  history,
  realChoice,
  busy,
  submitChoice,
  submitBatch,
}: UseSecondaryPrepareInput) {
  const generation = history?.generation ?? 0;
  const action = useMemo(
    () => detectStrategicAction({ events, players, activePlayer, phase }),
    [events, players, activePlayer, phase],
  );
  const eligibility = prepareEligibility(action, viewerSeat, players, events);
  const ready = events.length > 0 && players.length > 0;
  const { plan, set, clear } = usePreparedPlan({
    gameId,
    viewerSeat,
    actionKey: action?.key ?? null,
    generation,
    ready,
  });
  const viewer = players.find((player) => player.id === viewerSeat);

  const mine = Boolean(realChoice && viewerSeat && realChoice.actor === viewerSeat);
  const canPrepare = Boolean(action && eligibility.canPrepare && !mine && viewer);

  // ---- preparation mode: which dry question is showing -----------------------------------------
  const [step, setStep] = useState<DryStep | null>(null);
  const [firstPlanet, setFirstPlanet] = useState<string | null>(null);
  const preparing = step !== null && canPrepare;
  // The mode ends by itself when its action ends, or the viewer's real question arrives.
  useEffect(() => {
    if (step !== null && !canPrepare) setStep(null);
  }, [step, canPrepare]);

  const dry: DryChoice | null = useMemo(() => {
    if (!preparing || !action || !viewer || !step) return null;
    return buildDryChoice({
      action,
      viewer,
      board,
      step,
      taken: step === "planet2" && firstPlanet ? [firstPlanet] : [],
    });
  }, [preparing, action, viewer, board, step, firstPlanet]);

  const open = useCallback(() => {
    setFirstPlanet(null);
    setStep("secondary");
  }, []);
  /** Leaves preparation mode; the plan stays saved. */
  const close = useCallback(() => {
    setStep(null);
    setFirstPlanet(null);
  }, []);

  const base: SecondaryPlan | null = action ? (plan ?? { card: action.card, follow: true }) : null;

  /** The preparation-mode replacement for `onSubmitChoice`: records a plan, sends nothing. */
  const prepareSubmit = useCallback(
    async (optionId: string) => {
      if (!action || !step) return;
      const card = action.card;
      const finish = () => close();
      if (step === "secondary") {
        if (optionId === "no" || optionId === "decline") {
          set({ card, follow: false });
          return finish();
        }
        set({ card, follow: true });
        const next = nextDryStep(action.family, "secondary", { planets: 0 });
        if (!next) return finish();
        const probe = viewer && buildDryChoice({ action, viewer, board, step: next });
        // A follow-up with nothing to pick (no exhausted planet, nothing researchable) ends the plan.
        if (!probe) return finish();
        return setStep(next);
      }
      const current = base ?? { card, follow: true };
      if (step === "tech") {
        if (optionId === "decline") {
          set({ ...current, follow: true, tech: undefined });
          return finish();
        }
        set({ ...current, follow: true, tech: optionId });
        return finish();
      }
      if (step === "planet1") {
        set({ ...current, follow: true, planets: [optionId] });
        setFirstPlanet(optionId);
        const probe =
          viewer && buildDryChoice({ action, viewer, board, step: "planet2", taken: [optionId] });
        if (!probe) return finish();
        return setStep("planet2");
      }
      if (step === "planet2") {
        const first = current.planets?.[0];
        set({ ...current, follow: true, planets: first ? [first, optionId] : [optionId] });
        return finish();
      }
      if (step === "site") {
        if (optionId === "decline") {
          set({ ...current, follow: true, structure: undefined });
          return finish();
        }
        const [unit, , planet] = optionId.split("|");
        if ((unit === "pds" || unit === "spacedock") && planet) {
          set({ ...current, follow: true, structure: { unit: unit as StructureUnit, planet } });
        }
        return finish();
      }
    },
    [action, step, base, set, close, viewer, board],
  );

  /** The preparation-mode replacement for the token batch (Leadership's purchase panel). */
  const prepareBatch = useCallback(
    async (batch: BasketPlan) => {
      if (!action || batch.kind !== "tokens") return;
      const pools = { tactic: 0, fleet: 0, strategic: 0 };
      for (const tokenStep of batch.steps) {
        if (tokenStep.kind !== "pool") continue;
        const pool = TOKEN_POOLS.find((candidate) => tokenStep.pool === `${candidate}_tokens`);
        if (pool) pools[pool] += 1;
      }
      const total = pools.tactic + pools.fleet + pools.strategic;
      if (total === 0) set({ card: action.card, follow: false });
      else set({ card: action.card, follow: true, leadership: { pools } });
      close();
    },
    [action, set, close],
  );

  // ---- when the real question opens ------------------------------------------------------------
  // The follow-up prompts (technology, planets, site) belong to the plan only once its own window
  // opened in this action: a decision seen after a page load, or any other research prompt, is not
  // answered by it.
  const [openedFor, setOpenedFor] = useState<string | null>(null);
  const secondaryOpen = Boolean(
    mine &&
      realChoice &&
      action &&
      isSecondaryQuestion(realChoice) &&
      realChoice.details?.card === action.card,
  );
  useEffect(() => {
    if (secondaryOpen && action) setOpenedFor(action.key);
  }, [secondaryOpen, action]);
  const applicable = secondaryOpen || (action !== null && openedFor === action.key);
  const resolution: StepResolution = useMemo(
    () =>
      plan && applicable ? resolveStep(plan, realChoice, viewerSeat) : { kind: "none" as const },
    [plan, applicable, realChoice, viewerSeat],
  );

  const submitTokens = submitBatch
    ? (steps: TokenStep[]) => submitBatch({ kind: "tokens", steps })
    : undefined;
  const { pending, played, cancel, holding } = useSecondaryAutoPlay({
    choice: realChoice,
    viewerSeat,
    history,
    busy,
    resolution,
    submitOption: submitChoice,
    submitTokens,
  });

  /** What the app shows as the pending decision: the dry one while preparing, nothing while held. */
  const shownChoice = dry ? dry.choice : holding ? null : realChoice;

  return {
    action,
    canPrepare,
    plan,
    clear,
    preparing,
    open,
    close,
    dry,
    shownChoice,
    prepareSubmit,
    prepareBatch,
    realChoice,
    mine,
    resolution,
    pending,
    played,
    cancel,
    holding,
  };
}

export type SecondaryPrepare = ReturnType<typeof useSecondaryPrepare>;
