import type { GalleryCase } from "./decisionGalleryCases.ts";

const actor = "gallery_seat";
const option = (id: string, label: string, kind?: string, payload?: Record<string, unknown>) => ({
  id,
  label,
  kind,
  payload,
});
const decline = (label = "decline") => option("decline", label, "decline");

/** Decisions the Porkchop911 merge added, shown as engine-built offer cards. */
export function offerCases(): GalleryCase[] {
  return [
    {
      workflow: "generic_selection",
      title: "Ground hit: sustain damage",
      fallback: "'use SUSTAIN DAMAGE' / decline -> the unit, the planet and what each answer does",
      note: "An effect (Deorbit Barrage and similar) assigned a hit to a named ground force; its owner may sustain it.",
      choice: {
        actor,
        nonce: "gallery-ground-sustain",
        prompt: "sustain the hit on mech on Jord (system 14)",
        context: { subtype: "ground_effect_sustain", source: { Rule: "87" } },
        options: [option("sustain", "use SUSTAIN DAMAGE", "ground_effect_sustain"), decline()],
        details: {
          kind: "offer",
          card: {
            title: "Sustain damage",
            tag: "ground hit",
            window: "An effect has assigned a hit to this ground force",
            text: "SUSTAIN DAMAGE cancels the hit and the unit stays, damaged. Without it the unit is destroyed.",
          },
          facts: [
            { label: "Unit", unit: "mech" },
            { label: "Where", planet: "jord", system: "14" },
          ],
          captions: {
            sustain: { label: "Sustain damage", hint: "The unit stays on the planet, damaged" },
            decline: { label: "Let it be destroyed", hint: "The unit is removed" },
          },
        },
      },
    },
  ];
}
