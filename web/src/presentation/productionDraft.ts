import type { ChoiceOptionDto, PendingChoiceDto } from "../protocol/types.ts";

/**
 * The arithmetic of the production builder, shared by the drawer (what a staged draft costs) and by
 * the prepared Warfare secondary (whether a planned build still fits the real question).
 * Option payloads are the engine's (`production.rs::build_options`); nothing here is estimated.
 */

/** Resources a staged draft costs: printed costs less the single one-time discount of the use. */
export function draftResourceCost(options: ChoiceOptionDto[], draft: Record<string, number>): number {
  let printedTotal = 0;
  let discountOnce = 0;
  for (const option of options) {
    const batches = draft[option.id] ?? 0;
    if (!batches) continue;
    if (option.payload?.free_this_use === true) continue;
    const cost = option.payload?.cost;
    if (typeof cost !== "number" || !Number.isSafeInteger(cost) || cost < 0) return Infinity;
    const printed = option.payload?.printed_cost;
    const discount = option.payload?.discount;
    if (
      typeof printed === "number" &&
      Number.isSafeInteger(printed) &&
      printed >= cost &&
      typeof discount === "number" &&
      Number.isSafeInteger(discount) &&
      discount >= 0
    ) {
      printedTotal += batches * printed;
      // Every offered option previews the same one-time discount. Count it once, not per batch.
      discountOnce = Math.max(discountOnce, Math.min(discount, printed));
    } else {
      printedTotal += batches * cost;
    }
  }
  return Math.max(0, printedTotal - discountOnce);
}

/** Production capacity one batch of this option uses. */
export const optionCapacity = (option: ChoiceOptionDto): number =>
  typeof option.payload?.production_spent === "number"
    ? option.payload.production_spent
    : typeof option.payload?.placed === "number"
      ? option.payload.placed
      : typeof option.payload?.count === "number"
        ? option.payload.count
        : 1;

/** The unit an option builds (its payload, else its id). */
export const optionUnit = (option: ChoiceOptionDto): string =>
  String(option.payload?.unit ?? option.id);

const isBuild = (option: ChoiceOptionDto) => option.id !== "decline" && option.kind !== "decline";

/** The production room the question states: capacity left and resources that can be spent. */
export function productionRoom(choice: PendingChoiceDto): {
  capacityLeft: number;
  resources: number | null;
} {
  const constraint = choice.context?.outstanding?.[0];
  const capacityLeft = Math.max(0, (constraint?.amount ?? 0) - (constraint?.paid ?? 0));
  const options = choice.options.filter(isBuild);
  const available = options.find((o) => typeof o.payload?.available_resources === "number")?.payload
    ?.available_resources;
  const credit = options.find((o) => typeof o.payload?.credit === "number")?.payload?.credit;
  const resources =
    typeof available === "number" && Number.isSafeInteger(available) && available >= 0
      ? available +
        (typeof credit === "number" && Number.isSafeInteger(credit) && credit > 0 ? credit : 0)
      : null;
  return { capacityLeft, resources };
}

/** The system the production question is about. */
export function productionSystem(choice: PendingChoiceDto): string {
  const target = choice.context?.target;
  return target && "System" in target ? String(target.System) : "";
}

export type BuildPlanResult =
  | { ok: true; destination: string; draft: Record<string, number>; steps: { unit: string; count: number }[] }
  | { ok: false; reason: string };

/**
 * Checks planned builds (unit ids, one batch each, in order) against the real production question
 * the way the drawer's own limits do: every unit must be offered, the batches must fit the capacity
 * left, and the staged cost must fit the resources. Returns the drawer draft (batches per option id)
 * and the production steps to send.
 */
export function planBuilds(choice: PendingChoiceDto, builds: readonly string[]): BuildPlanResult {
  const options = choice.options.filter(isBuild);
  const destination = productionSystem(choice);
  if (!destination) return { ok: false, reason: "The production system is not named by the question." };
  if (!builds.length) return { ok: false, reason: "No units were prepared." };
  const draft: Record<string, number> = {};
  const steps: { unit: string; count: number }[] = [];
  let capacity = 0;
  const { capacityLeft, resources } = productionRoom(choice);
  for (const unit of builds) {
    const matching = options.filter((option) => optionUnit(option) === unit);
    if (matching.length !== 1) {
      return {
        ok: false,
        reason:
          matching.length === 0
            ? `${unit} can no longer be built here.`
            : `${unit} is offered in several ways; choose by hand.`,
      };
    }
    const option = matching[0];
    draft[option.id] = (draft[option.id] ?? 0) + 1;
    capacity += optionCapacity(option);
    steps.push({ unit, count: Number(option.payload?.count ?? 1) });
    if (capacity > capacityLeft) {
      return { ok: false, reason: "The prepared builds no longer fit the production capacity." };
    }
    if (resources !== null && draftResourceCost(options, draft) > resources) {
      return { ok: false, reason: "The prepared builds cost more than you can spend now." };
    }
  }
  return { ok: true, destination, draft, steps };
}
