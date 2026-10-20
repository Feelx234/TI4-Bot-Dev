/**
 * Promissory note ids in prose (option labels, prompts, event log lines, toasts): replaced by the
 * note's title ("Nekro Virus's Antivirus") so no raw `antivirus:nekro`, `support:hacan` or concealed
 * `?3` id reaches the player. Only ids that name a catalogued note are touched.
 */
import { promissoryNoteMeta } from "../protocol/contentCatalog.ts";
import { CONTENT } from "../protocol/catalogStore.ts";

const norm = (s: string): string => s.toLowerCase().replace(/^the\s+/, "").replace(/[^a-z0-9]/g, "");

let known: { content: unknown; aliases: Set<string>; factions: Set<string> } | undefined;
function tables() {
  if (known?.content !== CONTENT.promissoryNotes) {
    const aliases = new Set<string>(["support", "support_for_throne"]);
    for (const key of Object.keys(CONTENT.promissoryNotes)) aliases.add(key.replace(/^<color>_/, ""));
    const factions = new Set<string>(["generic"]);
    for (const f of Object.values(CONTENT.factions as unknown as Record<string, { id: string }>)) factions.add(f.id);
    known = { content: CONTENT.promissoryNotes, aliases, factions };
  }
  return known;
}

// `alias:owner` as a standalone token: not part of a longer `a:b:c` id, a path or a word.
const NOTE_TOKEN = /(?<![\w:|/.>-])([a-z][a-z_0-9]*):([a-z][a-z_0-9]*)(?![\w:|/-])/g;
// An ability id of a note reaction: `promissory:<holder faction>:<alias>:<EVENT>:<when|after>`.
const NOTE_ABILITY = /(?<![\w:|/.-])promissory:([a-z_0-9]+):([a-z_0-9]+):([A-Z][A-Z0-9_]*):(when|after)(?![\w:|/-])/g;
// A concealed id (`?3`) only when it is plainly a note ("the note ?3", "promissory note ?3").
const CONCEALED_NOTE = /\b(?:the |a )?(?:promissory )?note:?\s+\?\d*(?![\w])/gi;

/** Replace note ids and note ability ids in `text` with their titles. */
export function humanizeNoteIds(text: string): string {
  if (!text || (!text.includes(":") && !text.includes("?"))) return text;
  const { aliases, factions } = tables();
  return text
    .replace(NOTE_ABILITY, (m, _holder: string, alias: string, event: string, relation: string) => {
      if (!aliases.has(alias)) return m;
      const meta = promissoryNoteMeta(alias);
      const issuer = meta.issuerName ? `${meta.issuerName}'s ` : "";
      return `${issuer}${meta.name} (${relation} ${event.toLowerCase().replaceAll("_", " ")})`;
    })
    .replace(NOTE_TOKEN, (m, alias: string, owner: string) => {
      if (!aliases.has(alias)) return m;
      if (!factions.has(owner) && ![...factions].some((f) => norm(f) === norm(owner))) return m;
      return promissoryNoteMeta(m).title;
    })
    .replace(CONCEALED_NOTE, "a promissory note");
}
