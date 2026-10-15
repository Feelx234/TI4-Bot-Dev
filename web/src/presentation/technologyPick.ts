import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import { TECH_TRACKS, getTechnologyTrack, hydrateTech, type PrereqSymbol } from "./technologyData.ts";

const TECH_PICK = /_pick_technology$/;

/** A card asking which technology: Divert Funding returns one, then researches another. */
export const isTechnologyPick = (choice: Pick<PendingChoiceDto, "context">): boolean =>
  TECH_PICK.test(choice.context?.subtype ?? "");

export interface TechnologyPickOption {
  name: string;
  /** "Biotic", or null when the technology has no colour. */
  track: string | null;
  accentColor: string | null;
  /** The prerequisites it needs (what researching it takes), as coloured symbols. */
  prereqs: PrereqSymbol[];
  /** The printed text, one paragraph per entry. */
  text: string[];
  /** The seat the technology is taken from (Plagiarize), or null when the pick has no owner. */
  owner: string | null;
}

/** Plagiarize offers `<seat>|<technology>`: the owner and the technology the option stands for. */
export function splitTechnologyOptionId(id: string): { owner: string | null; technology: string } {
  const at = id.indexOf("|");
  return at < 0 ? { owner: null, technology: id } : { owner: id.slice(0, at), technology: id.slice(at + 1) };
}

/** One technology option as the card prints it. `null` for options that are not technologies. */
export function describeTechnologyPickOption(
  choice: Pick<PendingChoiceDto, "context">,
  option: Pick<ChoiceOption, "id" | "kind">,
): TechnologyPickOption | null {
  if (!isTechnologyPick(choice) || option.kind !== "technology") return null;
  const { owner, technology } = splitTechnologyOptionId(option.id);
  const tech = hydrateTech(technology);
  const trackId = getTechnologyTrack(technology);
  const track = TECH_TRACKS.find((entry) => entry.id === trackId) ?? null;
  return {
    name: tech.meta.name,
    track: track?.name ?? null,
    accentColor: track?.accentColor ?? null,
    prereqs: tech.prereqs,
    text: (tech.meta.description ?? "")
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0),
    owner,
  };
}

/** What this question of the card does, by its prompt (the same subtype serves both halves). */
export function technologyPickStage(
  choice: Pick<PendingChoiceDto, "context" | "prompt">,
): { title: string; text: string } | null {
  if (!isTechnologyPick(choice)) return null;
  if (/which technology to return/i.test(choice.prompt)) {
    return {
      title: "Return a technology",
      text: "The technology leaves your play area and goes back to the technology deck. Only a non-unit-upgrade, non-faction technology can be returned. You then research another one.",
    };
  }
  if (/what to research/i.test(choice.prompt)) {
    return {
      title: "Research another technology",
      text: "Prerequisites still apply. These are the technologies you can research now.",
    };
  }
  if (/which technology to steal/i.test(choice.prompt)) {
    return {
      title: "Steal a technology",
      text: "You have paid 5 influence. Choose a non-faction technology one of your neighbours owns; you gain it and they keep theirs.",
    };
  }
  return null;
}
