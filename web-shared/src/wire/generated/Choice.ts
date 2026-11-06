// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { ChoiceOption } from "./ChoiceOption.ts";
import type { DecisionContext } from "./DecisionContext.ts";
import type { MultiPick } from "./MultiPick.ts";
import type { PlayerId } from "./PlayerId.ts";

/**
 * A decision point handed to exactly one actor.
 */
export type Choice = { player: PlayerId, prompt: string, options: Array<ChoiceOption>, 
/**
 * Why this question exists and what remains outstanding in its transaction.
 */
context?: DecisionContext, 
/**
 * Facts a client needs to present the question (current pools, who played a card, tokens
 * left). Display only: never read by the engine, never copied into a [`DecisionRecord`],
 * and skipped when empty, so records, replays and fingerprints are unaffected.
 */
details?: Record<string, unknown>, 
/**
 * An id prefix that names answers built by the asker rather than listed in `options`
 * (a transaction's free-form deal). Unlisted ids starting with it are accepted as answers;
 * whoever resolves the answer parses the rest and rejects what it cannot read.
 */
free_form?: string, 
/**
 * A MULTI-PICK (`docs/MULTIASK1_RESUME.md`): the answer is a set of the listed options, their ids joined by
 * [`MULTI_PICK_SEPARATOR`] (an option named in [`MultiPick::repeat`] once per unit spent; a purchase adds `buy:N`), which the
 * engine checks. `None`: the answer is one listed option.
 */
multi_pick?: MultiPick, };
