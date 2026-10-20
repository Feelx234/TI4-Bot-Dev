import { describe, expect, it } from "vitest";
import { CONTENT } from "../protocol/catalogStore.ts";
import { promissoryNoteMeta } from "../protocol/contentCatalog.ts";
import { humanizeNoteIds } from "./noteText.ts";
import { participantText } from "./participantText.ts";
import { describeCardOption } from "./cardOptions.ts";
import { dealLabel, decodeTradeOption } from "./tradeDecoder.ts";

const NOTES = CONTENT.promissoryNotes as unknown as Record<string, { name: string; faction?: string }>;
const FACTIONS = Object.values(CONTENT.factions as unknown as Record<string, { id: string }>).map((f) => f.id);

/** `alias:faction` or `?3` left in shown text: a raw id. */
const LEAK = /\b[a-z][a-z_0-9]*:(?:generic|[a-z][a-z_0-9]*)\b|\?\d+/;

describe("promissoryNoteMeta", () => {
  it("reads the Antivirus with its issuer and text", () => {
    const meta = promissoryNoteMeta("antivirus:nekro");
    expect(meta.name).toBe("Antivirus");
    expect(meta.issuerName).toMatch(/Nekro/);
    expect(meta.title).toBe(`${meta.issuerName}'s Antivirus`);
    expect(meta.text).toContain("Technological Singularity");
  });

  it("names whose Support for the Throne it is", () => {
    for (const id of ["support:hacan", "sftt:hacan", "support:Hacan"]) {
      const meta = promissoryNoteMeta(id);
      expect(meta.support).toBe(true);
      expect(meta.title).toBe("Hacan's Support for the Throne");
      expect(meta.text).toContain("gain 1 victory point");
      expect(meta.text).toContain("Hacan's units");
      expect(meta.text).not.toContain("<color>");
    }
    expect(promissoryNoteMeta("support_for_throne").title).toBe("Support for the Throne");
  });

  it("calls a concealed id a promissory note", () => {
    for (const id of ["?3", "?", "?12"]) {
      const meta = promissoryNoteMeta(id);
      expect(meta.concealed).toBe(true);
      expect(meta.title).toBe("a promissory note");
    }
  });

  it("never shows an id for any catalogued note, issued by any faction", () => {
    const problems: string[] = [];
    for (const [key, note] of Object.entries(NOTES)) {
      const alias = key.replace(/^<color>_/, "");
      for (const owner of note.faction ? [note.faction] : ["generic", ...FACTIONS.slice(0, 4)]) {
        const meta = promissoryNoteMeta(`${alias}:${owner}`);
        if (!meta.known || LEAK.test(meta.title) || meta.title.includes(":") || (meta.text ?? "").includes("<color>"))
          problems.push(`${alias}:${owner} -> ${meta.title}`);
      }
    }
    expect(problems).toEqual([]);
  });
});

describe("humanizeNoteIds: option and label shapes the engine produces", () => {
  const shapes = [
    "antivirus:nekro",
    "Play promissory:yssaril:antivirus:SPACE_COMBAT_STARTED:after",
    "Play promissory:xxcha:cf:SYSTEM_ACTIVATED:after",
    "give support:hacan to the Empyrean",
    "give ra:jolnar to the Naalu player",
    "resolve the secondary by giving player_x the antivirus:nekro promissory note",
    "Give Hacan 1 promissory note from your hand",
    "give promissory note ra:jolnar",
    "request promissory note support:hacan",
    "sell antivirus:nekro for 2 trade goods",
    "Visionaria Select: which promissory note to give Red",
    "Red gave the note ?3 to Blue",
    "Blue received promissory note ?12",
  ];
  it("leaves no raw id in any shape", () => {
    const leaks = shapes.map(humanizeNoteIds).filter((text) => LEAK.test(text));
    expect(leaks).toEqual([]);
  });

  it("writes the note as Faction's Note", () => {
    expect(humanizeNoteIds("sell antivirus:nekro for 2 trade goods")).toMatch(/^sell \S.*'s Antivirus for 2 trade goods$/);
    expect(humanizeNoteIds("give support:hacan to the Empyrean")).toBe("give Hacan's Support for the Throne to the Empyrean");
    expect(humanizeNoteIds("Play promissory:yssaril:antivirus:SPACE_COMBAT_STARTED:after")).toMatch(
      /^Play .*Antivirus \(after space combat started\)$/,
    );
    expect(humanizeNoteIds("Red gave the note ?3 to Blue")).toBe("Red gave a promissory note to Blue");
  });

  it("leaves other prose, ids of other kinds and paths alone", () => {
    for (const text of [
      "component|tech|sc2",
      "play_area:ed2:gd",
      "Pay 3 trade goods",
      "relic:shard_of_the_throne",
      "note|antivirus:nekro",
      "Is this ok? (why not)",
    ])
      expect(humanizeNoteIds(text)).toBe(text);
  });

  it("is applied to every prose line the game shows", () => {
    expect(participantText("give support:hacan to the Empyrean", null, [])).toBe(
      "give Hacan's Support for the Throne to the Empyrean",
    );
  });
});

describe("other note displays", () => {
  it("note picks show name, issuer and text (no raw id)", () => {
    for (const id of ["antivirus:nekro", "support:hacan", "cf:generic", "?4"]) {
      const info = describeCardOption("dp1_pick_promissory note", { id, label: id });
      expect(info?.title, id).not.toMatch(LEAK);
      expect(info?.title, id).not.toContain(":");
    }
    expect(describeCardOption("dp1_pick_promissory note", { id: "support:hacan", label: "x" })?.title).toBe(
      "Hacan's Support for the Throne",
    );
  });

  it("trade labels of Support for the Throne and the Antivirus carry printed names", () => {
    for (const [id, label] of [
      ["pnsupport:hacan:4", "sell support:hacan for 4 trade goods"],
      ["pnantivirus:nekro:2", "sell antivirus:nekro for 2 trade goods"],
      ["nnantivirus:nekro>support:hacan", "give the note antivirus:nekro for the note support:hacan"],
    ]) {
      const shown = decodeTradeOption({ id, kind: "offer", label, payload: {} } as never).label;
      expect(shown, id).not.toMatch(LEAK);
      expect(shown, id).toMatch(/Antivirus|Support for the Throne/);
    }
    expect(typeof dealLabel).toBe("function");
  });
});
