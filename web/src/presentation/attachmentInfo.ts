import { findAttachmentMeta, humanizeId } from "../protocol/contentCatalog.ts";

const COLOUR_OF: Record<string, string> = { warfare: "red", propulsion: "blue", cybernetic: "yellow", biotic: "green" };

/** An attachment as a hover, tap or focus card shows it: its name and what it does to the planet. */
export interface AttachmentInfo {
  id: string;
  name: string;
  /** Always set: the printed card text, or what the catalog's numbers say. */
  text: string;
}

const signed = (n: number, what: string) => (n ? `${n > 0 ? "+" : ""}${n} ${what}` : "");

/**
 * The name and effect of attachment `id`. The printed card text comes from the engine's catalog;
 * an attachment whose card the catalog does not carry reads its numbers (values, specialty) back.
 */
export function attachmentInfo(id: string): AttachmentInfo {
  const meta = findAttachmentMeta(id);
  const name = meta?.name ?? humanizeId(id);
  if (meta?.text) return { id, name, text: meta.text };
  const bits: string[] = [];
  for (const specialty of meta?.techSpecialties ?? []) {
    bits.push(`This planet has a ${COLOUR_OF[specialty] ?? specialty} technology specialty.`);
  }
  const values = [signed(meta?.resourcesModifier ?? 0, "resources"), signed(meta?.influenceModifier ?? 0, "influence")]
    .filter(Boolean)
    .join(", ");
  if (values) bits.push(`${values} on this planet.`);
  return { id, name, text: bits.join(" ") || `${name} is attached to this planet.` };
}

/** "Name: effect text", for a native title and for the lines of a list. */
export function attachmentTitle(id: string): string {
  const info = attachmentInfo(id);
  return `${info.name}: ${info.text}`;
}
