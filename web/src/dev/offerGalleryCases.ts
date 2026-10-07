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
    paymentCase({
      nonce: "gallery-crimson-pay",
      title: "Crimson commander: gain or convert",
      prompt: "Crimson commander: gain 1 commodity or convert 1 commodity to a trade good",
      subtype: "crimson_payment",
      source: "crimsoncommander",
      card: {
        title: "Ahk Siever",
        window: "At the end of a combat between any players:",
        text: "Gain 1 commodity or convert 1 of your commodities to a trade good.",
      },
      note: "The Crimson commander pays its holder after every combat; with room for a commodity and one to convert, the holder chooses.",
    }),
    paymentCase({
      nonce: "gallery-deepwrought-pay",
      title: "Deepwrought commander: gain or convert",
      prompt: "Deepwrought commander: gain 1 commodity or convert 1 to a trade good",
      subtype: "deepwrought_payment",
      source: "deepwroughtcommander",
      card: {
        title: "Aello",
        window: "When another player spends resources to research a technology",
        text: "That player may reduce the cost by 1; if they do, gain 1 commodity or convert 1 of your commodities to a trade good.",
      },
      note: "The holder is paid after another seat took 1 off its research cost with the Deepwrought commander.",
    }),
  ];
}

/** The commanders that pay "gain 1 commodity or convert 1 to a trade good" (Crimson, Deepwrought). */
function paymentCase(spec: {
  nonce: string;
  title: string;
  prompt: string;
  subtype: string;
  source: string;
  card: { title: string; window: string; text: string };
  note: string;
}): GalleryCase {
  return {
    workflow: "generic_selection",
    title: spec.title,
    fallback: "Two plain labels -> the commander as printed, commodities and trade goods now, and before/after on each answer",
    note: spec.note,
    choice: {
      actor,
      nonce: spec.nonce,
      prompt: spec.prompt,
      context: { subtype: spec.subtype, source: { Content: spec.source } },
      options: [
        option("gain", "gain 1 commodity", "economy"),
        option("convert", "convert 1 commodity to a trade good", "economy"),
      ],
      details: {
        kind: "offer",
        card: { ...spec.card, tag: "commander" },
        facts: [
          { label: "Commodities", value: "1 of 3" },
          { label: "Trade goods", value: 4 },
        ],
        captions: {
          gain: { label: "Gain 1 commodity", hint: "Commodities 1 → 2" },
          convert: {
            label: "Convert 1 commodity to a trade good",
            hint: "Commodities 1 → 0, trade goods 4 → 5",
          },
        },
      },
    },
  };
}
