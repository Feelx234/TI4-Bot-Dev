export interface PolicyCandidate {
  desc: string;
  /** A checked checkbox or radio; clicking it again would take the selection back. */
  checked?: boolean;
}

/**
 * Narrows the clickable controls while a payment drawer is open. Random clicking toggles planets
 * on and off, which never settles a bill that needs most sources (for example 3 owed with exactly
 * 3 available). So: once the confirm button is enabled, press it, and never un-toggle a planet that
 * is already staged while something else can still be clicked.
 */
export function preferPayment<T extends PolicyCandidate>(candidates: T[]): T[] {
  const confirm = candidates.find((c) => /^confirm-payment-btn\b/.test(c.desc));
  if (confirm) return [confirm];
  // "Pick on map" would only minimise the list; the harness pays from the list.
  const kept = candidates.filter(
    (c) =>
      !(c.checked && /^planet-card-/.test(c.desc)) &&
      !/^pick-on-map-btn\b/.test(c.desc),
  );
  return kept.length ? kept : candidates;
}

/**
 * Hit assignment: once every hit is staged ("Confirm hits" is enabled), confirm. Otherwise any
 * staging control, so random play still spreads hits over different ships.
 */
export function preferHitConfirm<T extends PolicyCandidate>(
  candidates: T[],
): T[] {
  const confirm = candidates.find((c) => /^hit-confirm\b/.test(c.desc));
  return confirm ? [confirm] : candidates;
}

/**
 * Command tokens: Confirm unlocks only once every token is assigned, so confirm as soon as it is
 * enabled; until then only "+" can make progress (a "-" or Reset would loop forever).
 *
 * Leadership's panel also has a purchase stepper. Buying is optional, so now and then (when `rng`
 * is given) press "buy one more" instead: it only raises the number of tokens to assign, is bounded
 * by what is affordable, and Confirm still waits for every token, so the staging still ends.
 */
export function preferTokenConfirm<T extends PolicyCandidate>(
  candidates: T[],
  rng?: () => number,
): T[] {
  // Leadership's "Pay on the map" bar (minimized dialog): confirm as soon as it is enabled, else
  // fall back to the Auto-pay suggestion, never the trade-goods stepper or the map planets, whose
  // toggles can leave the bill unpaid. With neither enabled only the way back to the panel is left.
  if (candidates.some((c) => /^token-bar-/.test(c.desc))) {
    const barConfirm = candidates.find((c) => /^token-bar-confirm\b/.test(c.desc));
    if (barConfirm) return [barConfirm];
    const barAuto = candidates.find((c) => /^token-bar-auto\b/.test(c.desc));
    if (barAuto) return [barAuto];
    const back = candidates.filter((c) => /^resume-/.test(c.desc));
    return back.length ? back : candidates.filter((c) => !/^token-bar-goods/.test(c.desc));
  }
  // With the token panel up the map's payable planets are clickable too; toggling one writes a
  // payment override that can leave the bill unpaid. The panel's own controls make all the progress.
  if (candidates.some((c) => /^token-/.test(c.desc))) {
    candidates = candidates.filter((c) => !/^(planet|system-hex)-/.test(c.desc));
  }
  // The payment override is optional and its toggles can leave the bill unpaid: never click them,
  // only "Use Auto-pay" (to recover) when nothing else can make progress.
  const autoPay = candidates.find((c) => /^token-payment-auto\b/.test(c.desc));
  const payOnMap = candidates.find((c) => /^token-pay-on-map\b/.test(c.desc));
  candidates = candidates.filter((c) => !/^token-payment-|^token-pay-on-map\b/.test(c.desc));
  const buy = candidates.find((c) => /^token-buy-plus\b/.test(c.desc));
  if (buy && rng && rng() < TOKEN_BUY_CHANCE) return [buy];
  const confirm = candidates.find((c) => /^token-confirm\b/.test(c.desc));
  // Once everything is assigned, now and then pay from the map bar instead of the panel.
  if (confirm && payOnMap && rng && rng() < PAY_ON_MAP_CHANCE) return [payOnMap];
  if (confirm) return [confirm];
  const plus = candidates.filter((c) => /^token-plus-/.test(c.desc));
  if (plus.length) return plus;
  return autoPay && !candidates.length ? [autoPay] : candidates;
}

/** How often a settled Leadership purchase is paid from the map bar ("Pay on the map"). */
export const PAY_ON_MAP_CHANCE = 0.3;

/**
 * The trade desk (propose screen). Propose is enabled only when the staged combination is a deal
 * the server lists, so random staging rarely got there. Once Propose is enabled press it; while the
 * desk shows "Nearest available deals" press one of those (it stages a listed deal). Now and then
 * decline instead ("Offer Nothing"), so the desk is not always pushed through. Anything else (an
 * empty desk, the answer screen with Accept/Refuse/Counter) is left to the general choice.
 */
export function preferTradeDesk<T extends PolicyCandidate>(candidates: T[], rng?: () => number): T[] {
  const propose = candidates.find((c) => /^propose-trade-btn\b/.test(c.desc));
  const decline = candidates.find((c) => /^decline-trade-btn\b/.test(c.desc));
  const suggestions = candidates.filter((c) => /^stage-suggest-/.test(c.desc));
  if (!propose && !suggestions.length) return candidates;
  if (decline && rng && rng() < TRADE_DECLINE_CHANCE) return [decline];
  return propose ? [propose] : suggestions;
}

/** How often a desk that could be pushed through is declined instead. */
export const TRADE_DECLINE_CHANCE = 0.15;

/** How often the harness buys one more Leadership token while the panel offers it. */
export const TOKEN_BUY_CHANCE = 0.35;

/**
 * The persistent turn bar. Every button there submits (or opens a menu) in one click, so none of
 * the staging-then-confirm logic applies, and the button text (strategy card effects mention
 * "remove", "pass", "done") must never be read as a take-back or a submit control.
 */
export const isBarControl = (desc: string): boolean => /^turn-bar-/.test(desc);

/** Bar controls that send an engine option; the three menu toggles only open a list. */
export function isBarSubmit(desc: string): boolean {
  return /^turn-bar-(tactical|strategic-|pass|end|item-|open-)/.test(desc);
}

/** Rows inside an open bar menu (components, action cards, trade partners). */
const isBarRow = (desc: string): boolean => /^turn-bar-(item|open)-/.test(desc);

/**
 * The controls to choose among while the turn bar is up, or `null` when it is not. An open menu
 * must be answered first (its rows, never a second toggle), otherwise every enabled bar button.
 */
export function turnBarPool<T extends PolicyCandidate>(candidates: T[]): T[] | null {
  const bar = candidates.filter((c) => isBarControl(c.desc));
  if (!bar.length) return null;
  const rows = bar.filter((c) => isBarRow(c.desc));
  return rows.length ? rows : bar;
}

// Steering weights, first match wins; anything unmatched weighs 1. They push random play toward
// moving fleets into contested systems and Mecatol Rex instead of passing and trading.
const STEER_WEIGHTS: [RegExp, number][] = [
  // The turn bar (matched on the test id, which comes first in the description).
  [/^turn-bar-tactical\b/, 30],
  [/^turn-bar-strategic-/, 3],
  [/^turn-bar-trade\b/, 0.1],
  [/^turn-bar-open-/, 1],
  // The action card and component menus: their rows are where action cards, relics, leaders and
  // technologies get played, which random play rarely reached at weight 1 among ~10 buttons.
  [/^turn-bar-(cards|components)\b/, 6],
  [/^turn-bar-pass\b/, 0.3],
  // Ending the turn is the closing question; when it is enabled it should usually be taken.
  [/^turn-bar-end\b/, 20],
  [/^turn-bar-/, 1],
  // A reaction window offers "play" and "pass"; at equal weight most of the cards stay in hand.
  [/^play-reaction-btn/, 4],
  [/take a tactical action/i, 30],
  [/strategic action/i, 3],
  [/open a transaction|propose-trade-btn|trade-opt-/i, 0.1],
  [/decline-trade-btn/i, 5],
  [/\| pass$/i, 0.3],
  // 27.2: lifting the custodians opens the agenda phase, which random play otherwise never sees.
  [/remove it for a victory point/i, 60],
  // Munitions Reserves (Letnev) spends 2 trade goods per reroll, which a raider needs for the
  // custodians.
  [/reroll this round's misses/i, 0.1],
  [/(^|\| )leave it\b/i, 0.05],
  // Carry ground forces along: a carrier that moves empty can never invade, and the batch
  // declines an unplanned cargo hold, so the infantry stays behind (27.2a then forbids the
  // custodians and Mecatol stays unlanded).
  [/^rally-inc-cargo-/, 25],
  // A raider in Mecatol leaving it gives up the custodians it was placed there to lift.
  [/^rally-inc-18-/, 0.2],
  [/^rally-inc-/, 10],
  [/ in space/i, 10],
  // Finishing while moves are staged throws them away, so a populated commit wins.
  [/^commit-moves-btn \| Commit Moves/, 50],
  [/finish-movement-btn|done committing/i, 0.05],
];

const UNSTAGE = /reset|remove|rally-dec|decrement/i;
// The invasion overlay renders the custodians options as bare buttons (no test id), so the
// description is just the label.
const CUSTODIANS_YES = /remove it for a victory point/i;

/**
 * Controls that take back a staged selection. A choice-dialog option never does, and neither does
 * the custodians option, even though its text says "remove": "remove it for a victory point" was
 * treated as one, so it was almost never clicked and "no" was submitted instead.
 */
export function isUnstage(desc: string): boolean {
  if (isBarControl(desc) || /^choice-option\b/.test(desc) || CUSTODIANS_YES.test(desc)) return false;
  return UNSTAGE.test(desc);
}

// Controls that hide the decision or rewrite history; clicking them never advances the game. The
// reaction dialog's card-text, compact and map controls are all `reaction-inspect-*`.
// The info buttons (unit and faction cards, hover popovers) only show text; "prepare" and the
// secondary-prep chip/banner record a local plan instead of answering the real question.
export const EXCLUDED_CONTROLS =
  /minimi[sz]e|close|cancel|undo|redo|history|search-input|trade-tab|pin-reaction|inspect|paused-plan-dismiss|ask[- ]each[- ](payment|build)|^(unit|faction)-info|: (unit|faction) details|^planet-effect-info|^(secondary-prep|prepare-|prep-)/i;

/** Whether a decision subtype is a reaction window, its inner card pick or a reaction ability. */
export function isReactionSubtype(subtype: string): boolean {
  return /^(play_)?reaction_(when|after)_|^instinct_training_cancel$|^l1z1x_agent_swap$/.test(
    subtype,
  );
}

/** What is wrong with a reaction dialog's visible text, if anything: raw engine ids, doubled verbs. */
export function reactionTextProblem(text: string): string | null {
  const raw = /\b[A-Z]{3,}_[A-Z_]{3,}\b/.exec(text);
  if (raw) return `raw engine id ${raw[0]}`;
  if (/Play play/i.test(text)) return "doubled verb 'Play play'";
  return null;
}

// Controls that submit something to the server.
const COMMIT =
  /submit|confirm|commit|done|finish|pass|paused-plan-continue|decline|abstain|propose|play-reaction|answer-opt|tiebreak-opt|sustain-opt|casualty-opt|retreat-opt|follow-up|vote-outcome|end turn/i;

/**
 * Whether a control takes back staging and whether it submits. A turn bar button submits in one
 * click, and its text must never be read for submit or take-back words.
 */
export function classifyControl(desc: string): { unstage: boolean; commit: boolean } {
  if (isBarControl(desc)) return { unstage: false, commit: isBarSubmit(desc) };
  const unstage = isUnstage(desc);
  return { unstage, commit: !unstage && COMMIT.test(desc) };
}

/** Weight at which an option is worth choosing before anything is submitted. */
const STRONG = 50;

/**
 * A strongly preferred option that is not selected yet. Choice dialogs preselect their first
 * option, so submitting before choosing takes it: the custodians dialog lists "no" first, and a
 * 35% chance of an early submit kept the custodians on Mecatol.
 */
export function strongUnselected<T extends PolicyCandidate>(
  stages: T[],
): T | undefined {
  return stages.find((c) => !c.checked && steerWeight(c.desc) >= STRONG);
}

/** Steering weight of one control, by its description (`<testid> | <label>`). */
export function steerWeight(desc: string): number {
  return STEER_WEIGHTS.find(([pattern]) => pattern.test(desc))?.[1] ?? 1;
}

/**
 * Steering weight for activating one system. Unreachable systems are rarely worth it; Mecatol Rex
 * and systems holding other players' units are favoured. A seat whose ground forces wait in
 * Mecatol's space area (the combat start preset) can activate it in place and lift the custodians
 * without moving, so that is weighted far above everything else.
 */
export function activationWeight(
  id: string,
  reachable: boolean,
  hasEnemies: boolean,
  groundForcesInPlaceOnMecatol: boolean, // ground forces in the system's own space area
  defendedPlanet = false,
): number {
  // Dominant: among ~35 other activations a weight of 40 was rarely picked, and the raider then
  // moved its ships out of Mecatol instead.
  // The same holds for a defended colony (the invasion preset) with the invader's ground forces
  // waiting in its space area.
  if (groundForcesInPlaceOnMecatol && (id === "18" || defendedPlanet)) return 2000;
  if (!reachable) return 0.2;
  // Another player's ground forces on a planet: the way to an invasion with space cannon,
  // bombardment and ground casualties, which fleet-versus-fleet space fights never reach.
  if (defendedPlanet && id !== "18") return 60;
  return id === "18" ? 40 : hasEnemies ? 30 : 5;
}

/**
 * What a successful batch response says about a plan the server paused at a reaction window, or
 * `null` when the plan was applied whole (or the body is not a batch result). A pause is progress:
 * the steps before it are committed and the reaction is the next decision, so the harness must
 * neither count it as a rejection nor re-send anything itself.
 */
export function pausedBatch(
  body: string,
): { applied: number; remaining: number; waiting: string | null } | null {
  try {
    const stopped = (JSON.parse(body) as {
      interrupted?: {
        applied_steps?: number;
        remaining_steps?: unknown[];
        offered?: { subtype?: string | null };
      };
    }).interrupted;
    if (!stopped) return null;
    return {
      applied: stopped.applied_steps ?? 0,
      remaining: stopped.remaining_steps?.length ?? 0,
      waiting: stopped.offered?.subtype ?? null,
    };
  } catch {
    return null;
  }
}
