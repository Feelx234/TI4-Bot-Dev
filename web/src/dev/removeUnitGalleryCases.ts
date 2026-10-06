import type { GalleryCase } from "./decisionGalleryCases.ts";

const actor = "gallery_seat";
const remove = (id: string, label: string) => ({ id, label, kind: "remove" });

/** "Remove a unit" over the fleet supply or capacity: system 18 holds the shared gallery fleet. */
export function removeUnitCases(): GalleryCase[] {
  const make = (
    nonce: string,
    title: string,
    reason: string,
    details: Record<string, unknown>,
    note: string,
  ): GalleryCase => ({
    workflow: "generic_selection",
    title,
    fallback: "Unit names and icons, where each is, how far over, what leaves with it",
    note,
    choice: {
      actor,
      nonce,
      prompt: `remove a unit: over ${reason} in 18`,
      details: { reason, system: "18", ...details },
      options: [remove("remove|0", "remove dreadnought"), remove("remove|2", "remove fighter")],
    },
  });
  return [
    make(
      "gallery-remove-supply",
      "Remove a unit: over fleet supply",
      "fleet supply",
      { fleet_limit: 1, fleet_charged: 3 },
      "Engine details give the supply and the limit; the board gives the fleet and its cargo.",
    ),
    make(
      "gallery-remove-capacity",
      "Remove a unit: over capacity",
      "capacity",
      { capacity_consumed: 3, capacity_transport: 2 },
      "Capacity: fighters and ground forces against the places the ships offer.",
    ),
  ];
}
