import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import { readReadyHints, type ReadyHints } from "./readyHints.ts";
import {
  findStrategyCardMeta,
  humanizeId,
  promissoryNoteMeta,
} from "../protocol/contentCatalog.ts";
import {
  describeCommodityStanding,
  type CommodityStanding,
} from "./commodityStanding.ts";

export interface StrategySecondaryView {
  cardName: string;
  initiative: number | null;
  /** The seat that played the card (a raw seat id; resolve it with the player identity). */
  playedBy: string | null;
  /** What the secondary does, in the card's printed words. */
  secondaryText: string | null;
  tokensLeft: number | null;
  costsToken: boolean;
  /** Follow by spending the strategy token; `null` when the engine offers only a free way (no token left). */
  yes: ChoiceOption | null;
  no: ChoiceOption;
  /** Token-free ways to follow (a held Acquiescence), each shown beside Follow and Skip. */
  waivers: SecondaryWaiverView[];
  /** A button caption that says what happens, e.g. "Spend 1 strategy token to draw two action cards". */
  yesLabel: string;
  noLabel: string;
  /** Diplomacy: the planets that could be readied and which spending cards were played. */
  readyHints: ReadyHints | null;
  /** Trade: the follower's commodities now and the limit a replenish fills to. */
  commodities: CommodityStanding | null;
}

/** One token-free way to follow, e.g. the Winnu promissory note Acquiescence. */
export interface SecondaryWaiverView {
  option: ChoiceOption;
  /** The button caption ("Use Acquiescence"). */
  label: string;
  /** The promissory note behind it, with its printed text; `null` for a non-note waiver. */
  note: { name: string; text: string | null } | null;
}

export const WAIVED_PREFIX = "follow|waived|";

const isYes = (option: ChoiceOption) =>
  option.id === "yes" || option.id === "follow";

function describeWaiver(option: ChoiceOption): SecondaryWaiverView {
  // `follow|waived|<module index>|<waiver id>`; the waiver id is a note alias for Acquiescence.
  const key = option.id.slice(WAIVED_PREFIX.length).split("|")[1] ?? "";
  const meta = key ? promissoryNoteMeta(key) : null;
  if (meta && meta.known && !meta.concealed) {
    return {
      option,
      label: `Use ${meta.name}`,
      note: { name: meta.name, text: meta.text },
    };
  }
  return {
    option,
    label: option.label || "Follow without a token",
    note: null,
  };
}

const isNo = (option: ChoiceOption) =>
  option.id === "no" || option.id === "decline" || option.kind === "decline";

/** "spend a strategy token and 4 resources to research" -> "Spend 1 strategy token + 4 resources to research". */
export function secondaryButtonLabel(prompt: string, fallback: string): string {
  const strategy = /^spend a strategy token(?: and (.+?))? to (.+)$/i.exec(
    prompt.trim(),
  );
  if (strategy) {
    const extra = strategy[1] ? ` + ${strategy[1]}` : "";
    return `Spend 1 strategy token${extra} to ${strategy[2]}`;
  }
  const influence = /^spend (\d+) influence for a command token$/i.exec(
    prompt.trim(),
  );
  if (influence) return `Spend ${influence[1]} influence for a command token`;
  return fallback;
}

/**
 * A strategy card secondary offer as a named action: which card, who played it, what it does and
 * what it costs. `null` for any other decision, or when the options are not a yes/no pair.
 */
export function describeStrategySecondary(
  choice: Pick<PendingChoiceDto, "prompt" | "options" | "details">,
): StrategySecondaryView | null {
  const details = choice.details;
  if (
    !details ||
    details.kind !== "strategy_secondary" ||
    typeof details.card !== "string"
  ) {
    return null;
  }
  const no = choice.options.find(isNo);
  const waiverOptions = choice.options.filter((option) =>
    option.id.startsWith(WAIVED_PREFIX),
  );
  const yes = waiverOptions.length
    ? (choice.options.find(isYes) ?? null)
    : (choice.options.find((option) => option !== no) ?? null);
  if (!no) return null;
  if (waiverOptions.length) {
    if (choice.options.length !== 1 + waiverOptions.length + (yes ? 1 : 0))
      return null;
  } else if (!yes || choice.options.length !== 2) return null;
  const meta = findStrategyCardMeta(details.card);
  const tokensLeft =
    typeof details.tokens_left === "number" ? details.tokens_left : null;
  return {
    cardName: meta?.name ?? humanizeId(details.card),
    initiative: meta?.initiative ?? null,
    playedBy: typeof details.played_by === "string" ? details.played_by : null,
    secondaryText: meta?.secondaryText || null,
    tokensLeft,
    costsToken: details.costs_token !== false,
    yes,
    no,
    waivers: waiverOptions.map(describeWaiver),
    yesLabel: yes ? secondaryButtonLabel(choice.prompt, yes.label) : "",
    noLabel: "Skip",
    readyHints: readReadyHints(details),
    commodities: describeCommodityStanding(choice),
  };
}
