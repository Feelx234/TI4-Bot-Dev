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
    manyReactionsCase(),
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

/** A reaction window with more than four cards keeps the reaction dialog (it once became a bare list). */
function manyReactionsCase(): GalleryCase {
  const cards: Array<[string, string]> = [
    ["sabo1", "Sabotage"],
    ["direct_hit", "Direct Hit"],
    ["skilled_retreat", "Skilled Retreat"],
    ["counterstroke", "Counterstroke"],
    ["reflective_shielding", "Reflective Shielding"],
  ];
  return {
    workflow: "action_card_reaction",
    title: "Reaction window with five cards",
    fallback: "Used to fall into the generic list; now the reaction dialog with every card and Pass",
    note: "More than four options no longer leave the reaction dialog; the bar scrolls.",
    choice: {
      actor,
      nonce: "gallery-reaction-many",
      prompt: "reaction_after_SYSTEM_ACTIVATED",
      context: {
        subtype: "reaction_after_SYSTEM_ACTIVATED",
        optional: true,
        source: { Reaction: "SYSTEM_ACTIVATED" },
      },
      options: [
        ...cards.map(([id, name]) => ({
          id,
          label: `Play ${name}`,
          kind: "ability",
          payload: { card: id, card_name: name },
        })),
        { id: "decline", label: "Pass", kind: "decline" },
      ],
    },
  };
}
