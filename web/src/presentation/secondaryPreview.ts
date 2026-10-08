import type {
  EngineChoice,
  PendingChoiceDto,
  PreviewPayment,
  SecondaryBlocker,
} from "../protocol/types.ts";
import type { SecondaryPreviewReply } from "../protocol/client.ts";
import { decodeDecisionTrigger } from "../protocol/decode.ts";
import { planetName } from "./secondaryPlan.ts";

/**
 * The engine's exact answer to "what would my secondary ask if its window opened now", reduced to
 * what the preparation panel needs. Produced by the server's read-only `preview_secondary`
 * (`ti4_engine::secondary_preview`); everything here is "as of now" and is re-checked against the
 * real question when it opens (`resolveStep` stays authoritative).
 */
export type ExactResult =
  /** The question the engine would ask, ready to render with the real components. */
  | { kind: "question"; choice: EngineChoice; payment?: PreviewPayment; skipped: string[] }
  /** The window would not open for this seat right now. */
  | { kind: "not_asked"; blocker: SecondaryBlocker }
  /** The answers given end the secondary: nothing further is asked. */
  | { kind: "complete" }
  /** No exact answer (old server, refusal, timeout, unexpected question): use the estimate. */
  | { kind: "none"; why: string };

export function toExactResult(reply: SecondaryPreviewReply): ExactResult {
  if (reply.kind === "refused") return { kind: "none", why: `refused: ${reply.reason}` };
  const body = reply.body;
  switch (body.status) {
    case "question":
      return {
        kind: "question",
        choice: body.choice,
        payment: body.payment,
        skipped: (body.skipped ?? []).map((prompt) => prompt.prompt),
      };
    case "would_not_be_asked":
      return { kind: "not_asked", blocker: body.blocker };
    case "complete":
      return { kind: "complete" };
    case "unavailable":
      return { kind: "none", why: body.detail };
  }
}

/** The engine's choice as the shell's pending decision, under a client-made (dry) nonce. */
export function pendingFromEngine(choice: EngineChoice, nonce: string): PendingChoiceDto {
  const context = choice.context;
  return {
    nonce,
    actor: choice.player,
    prompt: choice.prompt,
    options: choice.options,
    context:
      context && "trigger" in context
        ? { ...context, trigger: decodeDecisionTrigger(context.trigger) }
        : context,
    ...(choice.details ? { details: choice.details } : {}),
  };
}

const BLOCKERS: Record<SecondaryBlocker, string> = {
  no_strategy_token: "you have no strategy token left to spend",
  cannot_afford_influence: "you cannot afford a command token with your influence",
  cannot_pay_resources: "you cannot pay the 4 resources right now",
  nothing_to_research: "you have nothing to research right now",
  no_exhausted_planet: "none of your planets is exhausted right now",
  commodities_full: "your commodities are already full",
  no_production: "your home system has no production right now",
  other: "it is not available to you right now",
};

/** One sentence for the preparation banner when the engine says the window would not open. */
export const describeBlocker = (blocker: SecondaryBlocker): string =>
  `As of now you would not be asked this secondary: ${BLOCKERS[blocker]}. Your plan is kept and applies if that changes before your turn.`;

/** "Jord (2 resources), 1 trade good" for the plan the engine would pay a research with. */
export function describePayment(payment: PreviewPayment): string {
  const parts = [
    ...payment.planets.map((planet) => planetName(planet)),
    ...(payment.trade_goods
      ? [`${payment.trade_goods} trade good${payment.trade_goods === 1 ? "" : "s"}`]
      : []),
  ];
  return parts.join(", ");
}
