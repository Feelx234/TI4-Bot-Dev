import React, { createContext, useContext, useState } from "react";
import { createPortal } from "react-dom";
import { Dialog } from "../primitives/index.ts";
import { useBoardPrepSlot } from "../presentation/BoardPrepSlot.tsx";
import type { PlayerView } from "../protocol/types.ts";
import { POOL_LABEL, TOKEN_POOLS, type Pools, type TokenPool } from "../presentation/commandTokens.ts";
import {
  describeStatusPart,
  describeStatusPlan,
  planTotal,
  poolsAfterGain,
  redistributionProblem,
  type StatusTokenPlan,
} from "../presentation/statusTokenPlan.ts";
import type { StatusTokenPlanApi } from "../hooks/useStatusTokenPlan.ts";
import "./SecondaryPrep.css";

/** The plan and what it says about the open question, for the token panel's chips. */
export const StatusTokenPlanContext = createContext<StatusTokenPlanApi | null>(null);
export const useStatusTokenPlanContext = () => useContext(StatusTokenPlanContext);

const ZERO: Pools = { tactic: 0, fleet: 0, strategic: 0 };
const poolsOf = (player: PlayerView | undefined): Pools =>
  player
    ? { tactic: player.tactic_tokens, fleet: player.fleet_tokens, strategic: player.strategic_tokens }
    : ZERO;

const PoolSteppers: React.FC<{ id: string; pools: Pools; onChange: (pools: Pools) => void }> = ({
  id,
  pools,
  onChange,
}) => (
  <div className="status-plan__pools" data-testid={`${id}-pools`}>
    {TOKEN_POOLS.map((pool: TokenPool) => (
      <span key={pool} className="status-plan__pool">
        <span className="status-plan__pool-name">{POOL_LABEL[pool]}</span>
        <button
          type="button"
          className="button button--secondary button--sm"
          data-testid={`${id}-${pool}-minus`}
          aria-label={`One ${POOL_LABEL[pool]} token less`}
          disabled={pools[pool] === 0}
          onClick={() => onChange({ ...pools, [pool]: pools[pool] - 1 })}
        >
          −
        </button>
        <span className="token-panel__count" data-testid={`${id}-${pool}-count`}>
          {pools[pool]}
        </span>
        <button
          type="button"
          className="button button--secondary button--sm"
          data-testid={`${id}-${pool}-plus`}
          aria-label={`One ${POOL_LABEL[pool]} token more`}
          disabled={planTotal(pools) >= 16}
          onClick={() => onChange({ ...pools, [pool]: pools[pool] + 1 })}
        >
          +
        </button>
      </span>
    ))}
  </div>
);

/** The default gain of the status phase (81.5): two tokens. */
const DEFAULT_GAIN: Pools = { tactic: 2, fleet: 0, strategic: 0 };

export interface StatusTokenPlanDialogProps {
  viewer: PlayerView | undefined;
  initial: StatusTokenPlan;
  onSave: (plan: StatusTokenPlan) => void;
  onClear: () => void;
  onClose: () => void;
}

/**
 * Plans the status phase's two token decisions ahead of time: where the gained tokens go, and the
 * arrangement after redistributing. Nothing is sent; the real questions are answered from it only
 * when they open and still fit exactly.
 */
export const StatusTokenPlanDialog: React.FC<StatusTokenPlanDialogProps> = ({
  viewer,
  initial,
  onSave,
  onClear,
  onClose,
}) => {
  const held = poolsOf(viewer);
  const [gainOn, setGainOn] = useState(Boolean(initial.gain));
  const [gain, setGain] = useState<Pools>(initial.gain ?? DEFAULT_GAIN);
  const [redisOn, setRedisOn] = useState(Boolean(initial.redistribute));
  const [target, setTarget] = useState<Pools>(
    initial.redistribute ?? poolsAfterGain(held, initial.gain ?? DEFAULT_GAIN),
  );
  // The redistribution starts from what is held after the gain: planned, else the usual two.
  const expected = planTotal(poolsAfterGain(held, gainOn ? gain : DEFAULT_GAIN));
  const problem = redisOn ? redistributionProblem(target, expected, viewer?.faction) : null;
  const gainEmpty = gainOn && planTotal(gain) === 0;
  const blocked = problem !== null || gainEmpty || (!gainOn && !redisOn);
  const draft: StatusTokenPlan = {
    ...(gainOn ? { gain } : {}),
    ...(redisOn ? { redistribute: target } : {}),
  };
  return (
    <Dialog.Root open onOpenChange={(open) => !open && onClose()}>
      <Dialog.Content data-testid="status-plan-dialog" className="choice-workflow-dialog">
        <div className="panel choice-workflow-modal status-plan">
          <Dialog.Title as="h2" className="choice-workflow-title">
            Plan your status phase tokens
          </Dialog.Title>
          <Dialog.Description>
            Nothing is sent or spent now. When the game asks, the question is answered from this plan only
            if it still fits exactly; otherwise it stays open marked Needs review.
          </Dialog.Description>
          <p className="status-plan__held" data-testid="status-plan-held">
            You hold {held.tactic} tactic / {held.fleet} fleet / {held.strategic} strategy now.
          </p>
          <section className="status-plan__section" data-testid="status-plan-gain">
            <label className="status-plan__toggle">
              <input
                type="checkbox"
                data-testid="status-plan-gain-toggle"
                checked={gainOn}
                onChange={(event) => setGainOn(event.target.checked)}
              />
              Gain command tokens (81.5)
            </label>
            {gainOn && (
              <>
                <PoolSteppers id="status-plan-gain" pools={gain} onChange={setGain} />
                <p className="secondary-prep__note" data-testid="status-plan-gain-note">
                  {planTotal(gain)} token{planTotal(gain) === 1 ? "" : "s"}. Normally you gain 2; if the game
                  gives you a different number, the plan is not applied.
                </p>
              </>
            )}
          </section>
          <section className="status-plan__section" data-testid="status-plan-redistribute">
            <label className="status-plan__toggle">
              <input
                type="checkbox"
                data-testid="status-plan-redistribute-toggle"
                checked={redisOn}
                onChange={(event) => {
                  setRedisOn(event.target.checked);
                  if (event.target.checked && !initial.redistribute) {
                    setTarget(poolsAfterGain(held, gainOn ? gain : DEFAULT_GAIN));
                  }
                }}
              />
              Redistribute command tokens (81.5)
            </label>
            {redisOn && (
              <>
                <PoolSteppers id="status-plan-redis" pools={target} onChange={setTarget} />
                <p
                  className={`secondary-prep__note${problem ? " secondary-prep__note--attention" : ""}`}
                  data-testid="status-plan-problem"
                >
                  {problem ?? `Arranges all ${expected} tokens you expect to hold after the gain.`}
                </p>
              </>
            )}
          </section>
          <div className="status-plan__actions">
            <button
              type="button"
              className="button button--secondary"
              data-testid="status-plan-clear"
              onClick={() => {
                onClear();
                onClose();
              }}
            >
              Clear plan
            </button>
            <button type="button" className="button button--secondary" data-testid="status-plan-cancel" onClick={onClose}>
              Cancel
            </button>
            <button
              type="button"
              className="button button--primary"
              data-testid="status-plan-save"
              disabled={blocked}
              onClick={() => {
                onSave(draft);
                onClose();
              }}
            >
              Save plan
            </button>
          </div>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};

export interface StatusTokenPlanHostProps {
  api: StatusTokenPlanApi;
  viewer: PlayerView | undefined;
  /** The viewer may plan: a seated player in a running game, not already asked for these tokens. */
  canPlan: boolean;
  busy?: boolean;
}

/**
 * The chrome around a prepared status phase: the chip that opens the planner (next to the prepared
 * secondary chip), "Clear plan", and what the plan did when the question arrived: a one-click bar
 * (review mode), "Needs review", and the auto-play toasts ("Answered from your plan").
 */
export const StatusTokenPlanHost: React.FC<StatusTokenPlanHostProps> = ({ api, viewer, canPlan, busy }) => {
  const slot = useBoardPrepSlot()?.slot ?? null;
  const [planning, setPlanning] = useState(false);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { plan, hasPlan, resolution, pending, played, holding, part } = api;
  const showChip = canPlan && !planning;
  const actionable = resolution.kind === "option" || resolution.kind === "tokens";
  const showBar = part !== null && !pending && !holding && dismissed !== part && (actionable || resolution.kind === "review");

  const confirm = async () => {
    setSending(true);
    setError(null);
    try {
      await api.play();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setSending(false);
    }
  };

  const chips = showChip ? (
    <>
      <button type="button" className="secondary-prep__chip" data-testid="status-plan-chip" onClick={() => setPlanning(true)}>
        {hasPlan ? "Status plan" : "Plan status tokens"}
        <span className="secondary-prep__note" style={{ margin: 0 }}>
          {hasPlan ? describeStatusPlan(plan) : "gain and redistribute"}
        </span>
      </button>
      {hasPlan && (
        <button type="button" className="button button--secondary button--sm" data-testid="status-plan-clear-chip" onClick={api.clear}>
          Clear plan
        </button>
      )}
    </>
  ) : null;

  const hasFloating = showBar || !!pending || !!played;
  const floatsChips = !slot && !!chips;
  return (
    <>
      {slot && chips ? createPortal(<div className="secondary-prep-chips" data-testid="status-plan-chips">{chips}</div>, slot) : null}
      {planning && (
        <StatusTokenPlanDialog
          viewer={viewer}
          initial={plan}
          onSave={api.save}
          onClear={api.clear}
          onClose={() => setPlanning(false)}
        />
      )}
      {(hasFloating || floatsChips) && (
        <div className="secondary-prep" data-testid="status-plan-host">
          {floatsChips && chips}
          {pending && (
            <div className="secondary-prep__bar" role="status" data-testid="status-plan-autoplay-toast">
              <span className="secondary-prep__bar-text">
                Answering from your plan: <strong>{pending.text}</strong>
              </span>
              <span className="secondary-prep__bar-actions">
                <button type="button" className="button button--secondary button--sm" data-testid="status-plan-autoplay-cancel" onClick={api.cancel}>
                  Cancel
                </button>
              </span>
            </div>
          )}
          {!pending && played && (
            <div className="secondary-prep__bar" role="status" data-testid="status-plan-answered-toast">
              <span className="secondary-prep__bar-text">
                <span className="secondary-prep__badge" style={{ marginRight: 8 }}>Answered from your plan</span>
                <strong>{played}</strong>
              </span>
            </div>
          )}
          {showBar && actionable && (
            <div className="secondary-prep__bar" data-testid="status-plan-bar">
              <span className="secondary-prep__bar-text">
                <span className="secondary-prep__badge" style={{ marginRight: 8 }}>From your plan</span>
                <strong>{resolution.kind === "option" || resolution.kind === "tokens" ? resolution.text : ""}</strong>
                {error && <span role="alert"> {error}</span>}
              </span>
              <span className="secondary-prep__bar-actions">
                <button type="button" className="button button--primary button--sm" data-testid="status-plan-confirm" disabled={sending || busy} onClick={() => void confirm()}>
                  Confirm
                </button>
                <button type="button" className="button button--secondary button--sm" data-testid="status-plan-dismiss" onClick={() => setDismissed(part)}>
                  Choose myself
                </button>
              </span>
            </div>
          )}
          {showBar && resolution.kind === "review" && part && (
            <div className="secondary-prep__bar secondary-prep__bar--review" data-testid="status-plan-needs-review">
              <span className="secondary-prep__bar-text">
                <span className="secondary-prep__badge secondary-prep__badge--review" style={{ marginRight: 8 }}>
                  Needs review
                </span>
                {resolution.reason}
                {plan[part] && (
                  <span className="secondary-prep__note"> (You planned: {describeStatusPart(part, plan[part]!)}.)</span>
                )}
              </span>
              <span className="secondary-prep__bar-actions">
                <button type="button" className="button button--secondary button--sm" data-testid="status-plan-review-clear" onClick={api.clear}>
                  Clear plan
                </button>
              </span>
            </div>
          )}
        </div>
      )}
    </>
  );
};
