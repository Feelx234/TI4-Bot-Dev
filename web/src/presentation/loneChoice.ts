import type { PendingChoiceDto } from "../protocol/types.ts";

/** The two decisions the client answers for a seat when the engine offers exactly one option. */
export interface LoneAction {
  kind: "strategic" | "activate";
  optionId: string;
  /** Toast wording after "Only one choice: ". */
  text: string;
}

/**
 * Derive a lone case strictly from the pending choice the client received:
 *  - the opening action-phase turn menu (not the end-of-turn menu) whose only option is the bare
 *    `strategic` entry: the seat cannot pass or do anything else, so the card is the one move;
 *  - the tactical action's activation step (subtype `activate_system`, prompt "activate a system")
 *    with exactly one `activate` option. Free tactical actions (Warfare) use another prompt and
 *    subtype and are never matched.
 * Anything else, including a seat that can pass, returns null.
 */
export function loneAction(choice: PendingChoiceDto | null | undefined): LoneAction | null {
  if (!choice || choice.options.length !== 1) return null;
  const [only] = choice.options;
  if (
    choice.details?.kind === "turn_menu" &&
    choice.details.closing !== true &&
    choice.prompt === "action phase" &&
    only.id === "strategic"
  )
    return { kind: "strategic", optionId: only.id, text: "take your strategic action" };
  if (
    choice.context?.subtype === "activate_system" &&
    choice.prompt === "activate a system" &&
    only.kind === "activate"
  )
    return { kind: "activate", optionId: only.id, text: `activate system ${only.id}` };
  return null;
}
