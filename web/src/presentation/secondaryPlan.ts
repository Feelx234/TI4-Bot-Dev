import type {
  BoardView,
  ChoiceOptionDto,
  PendingChoiceDto,
  PlayerView,
} from "../protocol/types.ts";
import { getTechnologyMeta, findPlanetMeta } from "../protocol/contentCatalog.ts";
import {
  describeCommandTokens,
  maxPurchases,
  tokenOutcome,
  TOKEN_POOLS,
  type TokenPool,
  type TokenStep,
  type TokenStaging,
} from "./commandTokens.ts";
import {
  TECH_TIERS,
  UNIT_UPGRADE_TECH_IDS,
  TECH_TRACKS,
  checkTechPrerequisites,
  getFactionTechIds,
  hasResearchedTech,
  hydrateTech,
  isTechAllowedForFaction,
  techMatches,
} from "./technologyData.ts";
import { cardFamily, cardName, type CardFamily } from "./strategicAction.ts";

/**
 * A prepared strategy-card secondary: the follower's private, revocable suggestion.
 *
 * It is stored by meaning (card plus intent), never by option id: the ids and labels of the real
 * question differ per situation (a faction waiver replaces the plain "yes" for a seat without a
 * token). Nothing here spends or exhausts anything; the real decision is still answered by the
 * player (or, in auto mode, by a click-equivalent submission) when the engine asks.
 */
export interface SecondaryPlan {
  /** Strategy card id, e.g. `pok7technology`. */
  card: string;
  /** Follow the secondary (`true`) or skip it (`false`). */
  follow: boolean;
  /** Technology: the technology to research. */
  tech?: string;
  /** Diplomacy: up to two exhausted planets to ready, in order. */
  planets?: string[];
  /** Construction: what to place and where. */
  structure?: { unit: StructureUnit; planet: string };
  /** Leadership: how many tokens to buy with influence and the pool they go to. */
  leadership?: { tokens: number; pool: TokenPool };
}

export type StructureUnit = "pds" | "spacedock";
export const STRUCTURE_UNITS: readonly StructureUnit[] = ["pds", "spacedock"];

/** A plan as kept on the device, with what it is valid for. */
export interface StoredPlan {
  v: 1;
  /** `StrategicAction.key` of the action it was made for. */
  actionKey: string;
  /** History generation it was made in; any other generation (undo, redo, restore) voids it. */
  generation: number;
  plan: SecondaryPlan;
}

const isString = (value: unknown): value is string => typeof value === "string" && value.length > 0;

/** Parses a stored plan strictly; anything unexpected is `null` (the plan is then simply gone). */
export function parseStoredPlan(raw: string | null | undefined): StoredPlan | null {
  if (!raw) return null;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!data || typeof data !== "object") return null;
  const record = data as Record<string, unknown>;
  const plan = record.plan as Record<string, unknown> | undefined;
  if (record.v !== 1 || !isString(record.actionKey) || typeof record.generation !== "number") {
    return null;
  }
  if (!plan || typeof plan !== "object" || !isString(plan.card) || typeof plan.follow !== "boolean") {
    return null;
  }
  const clean: SecondaryPlan = { card: plan.card, follow: plan.follow };
  if (isString(plan.tech)) clean.tech = plan.tech;
  if (Array.isArray(plan.planets)) {
    const planets = plan.planets.filter(isString).slice(0, 2);
    if (planets.length) clean.planets = planets;
  }
  const structure = plan.structure as Record<string, unknown> | undefined;
  if (
    structure &&
    STRUCTURE_UNITS.includes(structure.unit as StructureUnit) &&
    isString(structure.planet)
  ) {
    clean.structure = { unit: structure.unit as StructureUnit, planet: structure.planet };
  }
  const leadership = plan.leadership as Record<string, unknown> | undefined;
  if (
    leadership &&
    typeof leadership.tokens === "number" &&
    Number.isInteger(leadership.tokens) &&
    leadership.tokens >= 1 &&
    leadership.tokens <= 10 &&
    TOKEN_POOLS.includes(leadership.pool as TokenPool)
  ) {
    clean.leadership = { tokens: leadership.tokens, pool: leadership.pool as TokenPool };
  }
  return { v: 1, actionKey: record.actionKey, generation: record.generation, plan: clean };
}

export const serializeStoredPlan = (stored: StoredPlan): string => JSON.stringify(stored);

/** Drops the parts of a plan that do not belong to its card, so a card switch never leaks intent. */
export function normalizePlan(plan: SecondaryPlan): SecondaryPlan {
  const family = cardFamily(plan.card);
  const next: SecondaryPlan = { card: plan.card, follow: plan.follow };
  if (!plan.follow) return next;
  if (family === "technology" && plan.tech) next.tech = plan.tech;
  if (family === "diplomacy" && plan.planets?.length) next.planets = plan.planets.slice(0, 2);
  if (family === "construction" && plan.structure) next.structure = plan.structure;
  if (family === "leadership" && plan.leadership) next.leadership = plan.leadership;
  return next;
}

/** Trade's owner may replenish a follower, which removes the follower's secondary entirely. */
export const TRADE_WARNING =
  "If the Trade player replenishes you, you are not offered a secondary at all and this is dropped.";

export const hasTradeWarning = (card: string): boolean => cardFamily(card) === "trade";

/** Whether the card has a second-level choice the panel can prepare (beyond follow or skip). */
export function hasDetailStep(family: CardFamily | null): boolean {
  return (
    family === "technology" ||
    family === "diplomacy" ||
    family === "construction" ||
    family === "leadership"
  );
}

// ---------------------------------------------------------------------------------------------
// What the panel can offer, from the viewer's own public data. Estimates: the engine re-checks.
// ---------------------------------------------------------------------------------------------

/** Technologies the seat can probably research now: not owned, allowed, prerequisites met. */
export function researchableEstimate(player: PlayerView | undefined): string[] {
  if (!player) return [];
  const counts = { PROPULSION: 0, BIOTIC: 0, CYBERNETIC: 0, WARFARE: 0 };
  for (const track of TECH_TRACKS) {
    counts[track.id] = (player.technologies ?? []).filter((owned) =>
      track.techIds.some((id) => techMatches(owned, id)),
    ).length;
  }
  const none = { PROPULSION: 0, BIOTIC: 0, CYBERNETIC: 0, WARFARE: 0 };
  const candidates = new Set<string>([
    ...TECH_TIERS.flatMap((tier) => tier.rows.flat()),
    ...UNIT_UPGRADE_TECH_IDS,
    ...getFactionTechIds(player.faction),
  ]);
  return [...candidates].filter((id) => {
    if (hasResearchedTech(player, id)) return false;
    const tech = hydrateTech(id);
    if (!isTechAllowedForFaction(tech, player.faction)) return false;
    return checkTechPrerequisites(tech, counts, none, player.faction);
  });
}

export interface OwnPlanet {
  planet: string;
  system: string;
  exhausted: boolean;
}

/** Planets the seat controls, from the public board. */
export function ownPlanets(board: BoardView | null | undefined, seat: string | null | undefined): OwnPlanet[] {
  if (!board || !seat) return [];
  const seen = new Set<string>();
  const planets: OwnPlanet[] = [];
  for (const [systemId, system] of Object.entries(board.systems ?? {})) {
    for (const planet of Object.values(system.planets ?? {})) {
      if (planet.controlled_by !== seat || seen.has(planet.planet_id)) continue;
      seen.add(planet.planet_id);
      planets.push({ planet: planet.planet_id, system: systemId, exhausted: Boolean(planet.exhausted) });
    }
  }
  return planets;
}

export const planetName = (planet: string): string => findPlanetMeta(planet)?.name ?? planet;
export const techName = (tech: string): string => getTechnologyMeta(tech).name;
export const structureName = (unit: StructureUnit): string => (unit === "pds" ? "PDS" : "space dock");

// ---------------------------------------------------------------------------------------------
// Resolving a plan against the real question
// ---------------------------------------------------------------------------------------------

export type StepResolution =
  /** The plan has nothing to say about this decision (for example a faction prompt in between). */
  | { kind: "none" }
  | { kind: "option"; optionId: string; text: string }
  | { kind: "tokens"; steps: TokenStep[]; text: string }
  /** The plan no longer validates: show "Needs review" and answer nothing. */
  | { kind: "review"; reason: string };

const isNoOption = (option: ChoiceOptionDto) =>
  option.id === "no" || option.id === "decline" || option.kind === "decline";
const isYesOption = (option: ChoiceOptionDto) => option.id === "yes" || option.id === "follow";
const isWaiver = (option: ChoiceOptionDto) => option.id.startsWith("follow|waived|");

/** True for the strategy-card secondary question itself. */
export const isSecondaryQuestion = (choice: Pick<PendingChoiceDto, "details">): boolean =>
  choice.details?.kind === "strategy_secondary";

/**
 * The prepared answer for one decision of the viewer's, validated against the options the engine
 * really offers. Pure; the caller decides whether to show, prefill or (auto mode) submit it.
 */
export function resolveStep(
  plan: SecondaryPlan | null | undefined,
  choice: PendingChoiceDto | null | undefined,
  viewerSeat: string | null | undefined,
): StepResolution {
  if (!plan || !choice || !viewerSeat || choice.actor !== viewerSeat) return { kind: "none" };
  const family = cardFamily(plan.card);
  const name = cardName(plan.card);
  if (isSecondaryQuestion(choice)) {
    if (choice.details?.card !== plan.card) {
      return { kind: "review", reason: `A different card's secondary is open (you prepared ${name}).` };
    }
    if (!plan.follow) {
      const no = choice.options.find(isNoOption);
      return no
        ? { kind: "option", optionId: no.id, text: `Skip the ${name} secondary` }
        : { kind: "review", reason: "Skipping is not offered any more." };
    }
    if (family === "leadership" && plan.leadership) return resolveLeadership(plan, choice, name);
    const yes = choice.options.find(isYesOption);
    if (yes) {
      const spends = choice.details?.costs_token !== false;
      return {
        kind: "option",
        optionId: yes.id,
        text: `Follow ${name}${spends ? " (spends 1 strategy token)" : ""}`,
      };
    }
    const waivers = choice.options.filter(isWaiver);
    if (waivers.length === 1) {
      return {
        kind: "option",
        optionId: waivers[0].id,
        text: `Follow ${name} for free: ${waivers[0].label}`,
      };
    }
    if (waivers.length > 1) {
      return { kind: "review", reason: "Several free ways to follow are offered; choose one." };
    }
    return { kind: "review", reason: `You can no longer follow ${name}.` };
  }
  if (!plan.follow) return { kind: "none" };
  const subtype = choice.context?.subtype;
  if (subtype === "research_technology" && plan.tech) {
    const option = choice.options.find((o) => o.kind === "research" && techMatches(o.id, plan.tech!));
    return option
      ? { kind: "option", optionId: option.id, text: `Research ${techName(plan.tech)}` }
      : { kind: "review", reason: `${techName(plan.tech)} can no longer be researched.` };
  }
  if (subtype === "ready_planet" && plan.planets?.length) {
    const offered = new Set(choice.options.filter((o) => o.kind === "ready").map((o) => o.id));
    const next = plan.planets.find((planet) => offered.has(planet));
    return next
      ? { kind: "option", optionId: next, text: `Ready ${planetName(next)}` }
      : { kind: "review", reason: "No prepared planet is offered (readied already, or no longer exhausted); choose by hand." };
  }
  if (subtype === "place_structure" && plan.structure) {
    const { unit, planet } = plan.structure;
    const option = choice.options.find((o) => {
      const [kind, , target, extra] = o.id.split("|");
      return kind === unit && target === planet && extra === undefined;
    });
    return option
      ? {
          kind: "option",
          optionId: option.id,
          text: `Place a ${structureName(unit)} on ${planetName(planet)}`,
        }
      : {
          kind: "review",
          reason: `A ${structureName(unit)} can no longer be placed on ${planetName(planet)}.`,
        };
  }
  return { kind: "none" };
}

function resolveLeadership(
  plan: SecondaryPlan,
  choice: PendingChoiceDto,
  name: string,
): StepResolution {
  const wanted = plan.leadership!;
  const view = describeCommandTokens(choice, true);
  if (!view?.purchase) {
    // No purchase details: fall back to the plain first purchase; later prompts are answered by hand.
    const yes = choice.options.find(isYesOption);
    return yes
      ? { kind: "option", optionId: yes.id, text: `Follow ${name}` }
      : { kind: "review", reason: "You can no longer buy a command token." };
  }
  const affordable = maxPurchases(view);
  if (affordable < wanted.tokens) {
    return {
      kind: "review",
      reason:
        affordable === 0
          ? "You can no longer afford a command token."
          : `You can now afford only ${affordable} of the ${wanted.tokens} tokens you prepared.`,
    };
  }
  const staging: TokenStaging = { tactic: 0, fleet: 0, strategic: 0 };
  staging[wanted.pool] = wanted.tokens;
  const outcome = tokenOutcome(view, staging, wanted.tokens);
  if (!outcome || outcome.kind !== "plan") {
    return { kind: "review", reason: "The influence payment can no longer be planned." };
  }
  const tokens = `${wanted.tokens} command token${wanted.tokens === 1 ? "" : "s"}`;
  return {
    kind: "tokens",
    steps: outcome.steps,
    text: `Buy ${tokens} (${wanted.pool} pool) for ${view.purchase.cost * wanted.tokens} influence`,
  };
}

/** One-line description of a plan for the badge and the toast. */
export function describePlan(plan: SecondaryPlan): string {
  const name = cardName(plan.card);
  if (!plan.follow) return `Skip ${name}`;
  const family = cardFamily(plan.card);
  if (family === "technology" && plan.tech) return `Follow ${name}: research ${techName(plan.tech)}`;
  if (family === "diplomacy" && plan.planets?.length) {
    return `Follow ${name}: ready ${plan.planets.map(planetName).join(" and ")}`;
  }
  if (family === "construction" && plan.structure) {
    return `Follow ${name}: ${structureName(plan.structure.unit)} on ${planetName(plan.structure.planet)}`;
  }
  if (family === "leadership" && plan.leadership) {
    return `Buy ${plan.leadership.tokens} command token${plan.leadership.tokens === 1 ? "" : "s"}`;
  }
  return `Follow ${name}`;
}
