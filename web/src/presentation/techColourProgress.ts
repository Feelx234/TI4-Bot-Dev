import type { ObjectiveProgressView } from "../protocol/types.ts";
import { TECH_TRACKS, getTechnologyTrack } from "./technologyData.ts";

/** The objectives that count technologies per colour, with the number each colour needs. */
const COLOUR_OBJECTIVES: Record<string, number> = {
  diversify: 2, // Diversify Research: 2 technologies in each of 2 colours
  master_science: 2, // Master the Sciences: 2 technologies in each of 4 colours
};

export interface TechColourCount {
  track: (typeof TECH_TRACKS)[number]["id"];
  /** "Blue": the colour the card names. */
  colour: string;
  /** "Propulsion": the track the colour belongs to. */
  trackName: string;
  accent: string;
  count: number;
  /** How many the objective needs of this colour. */
  needed: number;
  met: boolean;
}

export interface TechColourDisplay {
  chips: TechColourCount[];
  /** Colours that reach the count: the engine's number when it sent one, else the chips'. */
  have: number;
  /** Colours the objective needs. */
  threshold: number;
  /** "2 of 2 colors · need 2". */
  summary: string;
  /** "Blue 2 of 2, Green 1 of 2, ...", for screen readers. */
  aria: string;
}

export const isColourObjective = (objectiveId: string): boolean => objectiveId in COLOUR_OBJECTIVES;

/** Owned technologies per colour (unit upgrades have none). */
export function techColourCounts(technologies: readonly string[]): Record<string, number> {
  const counts: Record<string, number> = Object.fromEntries(TECH_TRACKS.map((t) => [t.id, 0]));
  for (const id of new Set(technologies)) {
    const track = getTechnologyTrack(id);
    if (track) counts[track] += 1;
  }
  return counts;
}

/**
 * Diversify Research and Master the Sciences, per colour: how many technologies of each colour a
 * seat owns against the number the card needs ("Blue 2/2, Green 1/2, ..."). Owned technologies are
 * public. The engine's own progress still decides what counts as met; this only says where it comes
 * from. Null for any other objective.
 */
export function techColourDisplay(
  objectiveId: string,
  technologies: readonly string[] | undefined,
  progress: ObjectiveProgressView | undefined,
): TechColourDisplay | null {
  const needed = COLOUR_OBJECTIVES[objectiveId];
  if (needed === undefined || !technologies) return null;
  const counts = techColourCounts(technologies);
  const chips = TECH_TRACKS.map((track): TechColourCount => ({
    track: track.id,
    colour: track.colorName,
    trackName: track.name,
    accent: track.accentColor,
    count: counts[track.id],
    needed,
    met: counts[track.id] >= needed,
  }));
  const have = progress?.have ?? chips.filter((chip) => chip.met).length;
  const threshold = progress?.threshold ?? (objectiveId === "master_science" ? 4 : 2);
  return {
    chips,
    have,
    threshold,
    summary: `${have} of ${threshold} colors with ${needed}+ · need ${threshold}`,
    aria: chips
      .map((chip) => `${chip.colour} (${chip.trackName.toLowerCase()}) ${chip.count} of ${chip.needed}`)
      .join(", "),
  };
}
