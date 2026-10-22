import type { PublicRoll } from "../../../src/protocol/types";

/** One gun's Space Cannon Offense dice, as the server publishes them. */
export const cannonRoll = (
  seat: string,
  unit: string,
  planet: string | undefined,
  faces: number[],
  hitsOn: number,
  extra: Partial<PublicRoll> = {},
): Omit<PublicRoll, "index"> => {
  const hits = faces.filter((f) => f >= hitsOn).length;
  return {
    roller: seat,
    reason: "space cannon",
    label: "Space cannon",
    faces,
    hits_on: hitsOn,
    hits,
    system: "18",
    outcome: hits === 0 ? "no hits" : hits === 1 ? "1 hit" : `${hits} hits`,
    unit,
    planet,
    ...extra,
  };
};
