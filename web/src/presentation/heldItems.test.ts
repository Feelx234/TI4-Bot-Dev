import { describe, expect, it } from "vitest";
import type { BoardView, PlayerView } from "../protocol/types.ts";
import { ATTACHMENTS } from "../protocol/contentCatalog.ts";
import { attachmentInfo } from "./attachmentInfo.ts";
import { buildHeldItems, readHeldState, type HeldItemsModel, type HeldState } from "./heldItems.ts";

const base = (id: string, faction: string, extra: Partial<PlayerView> = {}): PlayerView => ({
  id,
  faction,
  victory_points: 0,
  trade_goods: 0,
  commodities: 0,
  tactic_tokens: 3,
  fleet_tokens: 3,
  strategic_tokens: 2,
  passed: false,
  strategy_cards: [],
  exhausted_strategy_cards: [],
  technologies: [],
  exhausted_technologies: [],
  relics: [],
  exhausted_relics: [],
  action_cards_count: 0,
  secret_objectives_count: 0,
  leaders: {},
  ...extra,
});

const me = base("p1", "hacan", {
  technologies: ["aida", "qdn", "amd"],
  exhausted_technologies: ["aida"],
  relics: ["heartofixth", "obsidian"],
  exhausted_relics: ["heartofixth"],
  relic_fragments: { cultural: 3, hazardous: 1 },
  exhausted_legendary: ["primor"],
  exploration_cards: [],
  leaders: { hacanagent: "readied", hacancommander: "locked", hacanhero: "locked" },
});
const rival = base("p2", "sol", { technologies: ["pi"], relics: ["thetriad"] });
const players = [me, rival];

const board = {
  systems: {
    s1: {
      system_id: "s1",
      command_tokens: [],
      units: [],
      planets: {
        primor: { planet_id: "primor", controlled_by: "p1", exhausted: false, attachments: ["dysonsphere"] },
        jord: { planet_id: "jord", controlled_by: "p2", exhausted: false, attachments: ["biotic"] },
      },
    },
  },
} as unknown as BoardView;

const held: HeldState = {
  notes: {
    "cf:sol": "p1", // received, in hand
    "ps:hacan": "p1", // own copy at home
    "an:sol": "p1", // faceup alliance
    "ta:hacan": "p2", // own note faceup elsewhere
    "ps:sol": "p2", // sol's own, in sol's hand
    "ta:sol": "p2",
  },
  faceup: ["an:sol", "ta:hacan"],
  support: { hacan: "p2" },
  marks: { "hacan:arbiters:used": "1", "sol:orbital:x": "secret-mark" },
};

const names = (m: HeldItemsModel, id: string) => m.sections.find((s) => s.id === id)?.items.map((i) => i.name) ?? [];
const label = (id: string) => (id === "p1" ? "Red" : "Blue");

describe("buildHeldItems", () => {
  const own = buildHeldItems({ players, subject: "p1", viewer: "p1", board, held, label });

  it("groups into named sections with counts", () => {
    expect(own.sections.map((s) => s.id)).toEqual([
      "alliance",
      "promissory",
      "relics",
      "techs",
      "upgrades",
      "leaders",
      "faction",
      "legendary",
      "attachments",
    ]);
    for (const s of own.sections) expect(s.count).toBe(s.items.length);
  });

  it("shows relics and fragments with ready and exhausted state", () => {
    const relics = own.sections.find((s) => s.id === "relics")!.items;
    const heart = relics.find((i) => i.key === "relic-heartofixth")!;
    expect(heart.chips.map((c) => c.label)).toEqual(["exhausted"]);
    expect(relics.find((i) => i.key === "relic-obsidian")!.chips[0].label).toBe("in play");
    const cultural = relics.find((i) => i.key === "fragments-cultural")!;
    expect(cultural.chips[0]).toEqual({ label: "3 of 3", tone: "unlocked" });
    expect(relics.find((i) => i.key === "fragments-hazardous")!.chips[0].label).toBe("1 of 3");
    expect(relics.some((i) => i.key === "fragments-industrial")).toBe(false);
  });

  it("splits exhaustable technologies from unit upgrades and faction techs", () => {
    const techs = own.sections.find((s) => s.id === "techs")!.items;
    expect(techs.map((i) => i.name)).toEqual(["AI Development Algorithm"]);
    expect(techs[0].chips[0]).toEqual({ label: "exhausted", tone: "exhausted" });
    expect(names(own, "upgrades")).toContain("Quantum Datahub Node");
    // A plain passive technology is not a held item.
    expect(JSON.stringify(own)).not.toContain("Antimass");
  });

  it("lists leaders with their lifecycle chips, agent first", () => {
    const leaders = own.sections.find((s) => s.id === "leaders")!.items;
    expect(leaders.map((i) => i.chips[0].label)).toEqual(["readied", "locked", "locked"]);
    expect(leaders.map((i) => i.chips[0].tone)).toEqual(["ready", "locked", "locked"]);
    expect(leaders[1].text).toMatch(/^Unlock:/);
  });

  it("shows a legendary ability with its own exhausted state and attachments only of own planets", () => {
    const legendary = own.sections.find((s) => s.id === "legendary")!.items;
    expect(legendary).toHaveLength(1);
    expect(legendary[0].name).toBe("The Atrament");
    expect(legendary[0].chips[0]).toEqual({ label: "ability exhausted", tone: "exhausted" });
    expect(names(own, "attachments")).toEqual(["Dyson Sphere"]);
  });

  it("holds the viewer's notes: received, faceup, given, support and the alliance partner", () => {
    const notes = own.sections.find((s) => s.id === "promissory")!.items;
    const byChip = (needle: string) => notes.filter((i) => i.chips.some((c) => c.label.includes(needle)));
    expect(byChip("in hand").map((i) => i.name)).toEqual(["Ceasefire"]);
    expect(byChip("in hand")[0].owner).toBe("Blue");
    expect(byChip("faceup with Blue").map((i) => i.name)).toEqual(["Trade Agreement"]);
    expect(byChip("given to Blue").map((i) => i.name)).toEqual(["Support for the Throne"]);
    expect(byChip("own copy").map((i) => i.name)).toEqual(["Political Secret"]);
    const alliance = own.sections.find((s) => s.id === "alliance")!.items;
    expect(alliance).toHaveLength(1);
    expect(alliance[0].text).toContain("Blue");
  });

  it("names the issuer where a printed note says <color>", () => {
    const cf = own.sections.find((s) => s.id === "promissory")!.items.find((i) => i.name === "Ceasefire" && i.owner === "Blue")!;
    expect(cf.text).not.toContain("<color>");
    expect(cf.text).toContain("Blue");
  });

  it("marks components usable now from the pending choice, read-only", () => {
    const model = buildHeldItems({
      players,
      subject: "p1",
      viewer: "p1",
      board,
      held,
      label,
      options: [
        { id: "component|tech|aida", label: "AI Development Algorithm", kind: "action" },
        { id: "component|leader|hacanagent", label: "Hacan agent", kind: "action" },
        { id: "primor", label: "The Atrament", kind: "legendary" },
        { id: "relic|heartofixth", label: "Heart of Ixth", kind: "action" },
        { id: "purge|cultural", label: "Purge", kind: "action" },
      ],
    });
    const usable = model.sections.flatMap((s) => s.items).filter((i) => i.usableNow).map((i) => i.key);
    expect(usable.sort()).toEqual(
      ["fragments-cultural", "leader-hacanagent", "legendary-primor", "relic-heartofixth", "tech-aida"].sort(),
    );
  });

  it("counts the badge without own notes at home, static faction abilities and locked cards", () => {
    const items = own.sections.flatMap((s) => s.items);
    const skipped = items.filter(
      (i) => i.kind === "Faction ability" || i.chips.some((c) => c.label.startsWith("own copy") || c.tone === "locked"),
    );
    expect(skipped.length).toBeGreaterThan(4);
    expect(own.total).toBe(items.length - skipped.length);
  });
});

describe("hidden information", () => {
  it("never lists another seat's hand or markers", () => {
    const view = buildHeldItems({ players, subject: "p2", viewer: "p1", board, held, label });
    expect(view.own).toBe(false);
    const text = JSON.stringify(view);
    // Sol's Political Secret and Trade Agreement copies sit in Sol's hand: not shown, not counted.
    expect(text).not.toContain("Political Secret");
    expect(text).not.toContain("in hand");
    expect(text).not.toContain("secret-mark");
    expect(text).not.toContain("Faction marker");
    // Public: the faceup Trade Agreement lying in Sol's play area, Support received, Sol's Alliance out.
    const notes = view.sections.find((s) => s.id === "promissory")!.items;
    expect(notes.map((i) => i.name)).toEqual(["Trade Agreement", "Support for the Throne"]);
    expect(view.sections.find((s) => s.id === "alliance")!.items).toHaveLength(1);
  });

  it("shows nothing private to a spectator", () => {
    const view = buildHeldItems({ players, subject: "p1", viewer: undefined, board, held, label });
    expect(view.own).toBe(false);
    const text = JSON.stringify(view);
    expect(text).not.toContain("in hand");
    expect(text).not.toContain("own copy");
    expect(text).not.toContain("Faction marker");
    expect(view.sections.flatMap((s) => s.items).some((i) => i.usableNow)).toBe(false);
  });

  it("shows no usable chips on another seat even when the viewer is offered the same ids", () => {
    const view = buildHeldItems({
      players,
      subject: "p2",
      viewer: "p1",
      board,
      held,
      options: [{ id: "component|tech|pi", label: "x", kind: "action" }],
    });
    expect(view.sections.flatMap((s) => s.items).some((i) => i.usableNow)).toBe(false);
  });

  it("is empty for an early seat", () => {
    const view = buildHeldItems({ players: [base("p9", "sol")], subject: "p9", viewer: "p9" });
    expect(view.sections.map((s) => s.id)).not.toContain("relics");
    expect(view.total).toBe(0);
  });
});

describe("readHeldState", () => {
  it("reads the promissory fields and ignores malformed ones", () => {
    expect(readHeldState(null).notes).toEqual({});
    const s = readHeldState({
      promissory_notes: { "cf:a": "p1", bad: 3 },
      promissory_faceup: ["cf:a", 4],
      support_holders: { a: "p1" },
      faction_marks: { "a:b:c": "1" },
    });
    expect(s).toEqual({ notes: { "cf:a": "p1" }, faceup: ["cf:a"], support: { a: "p1" }, marks: { "a:b:c": "1" } });
  });
});

describe("breakthrough section follows the table's content", () => {
  const keys = (on?: boolean) =>
    buildHeldItems({ players: [base("p1", "sol")], subject: "p1", viewer: "p1", held: readHeldState({}), breakthroughsOn: on })
      .sections.flatMap((s) => s.items.map((i) => i.key));
  it("is there by default and when on, gone when off", () => {
    expect(keys()).toContain("breakthrough");
    expect(keys(true)).toContain("breakthrough");
    expect(keys(false)).not.toContain("breakthrough");
  });
});

describe("attachments in the held items", () => {
  it("every attachment the content has shows its name and a non-empty effect text", () => {
    const ids = Object.keys(ATTACHMENTS);
    expect(ids.length).toBeGreaterThan(15);
    const planets = Object.fromEntries(
      ids.map((id) => [`pl_${id}`, { planet_id: `pl_${id}`, controlled_by: "p1", exhausted: false, attachments: [id] }]),
    );
    const everything = { systems: { s1: { system_id: "s1", command_tokens: [], units: [], planets } } } as unknown as BoardView;
    const model = buildHeldItems({ players, subject: "p1", viewer: "p1", board: everything, held, label });
    const items = model.sections.find((s) => s.id === "attachments")!.items;
    expect(items).toHaveLength(ids.length);
    for (const item of items) {
      expect(item.name.length, item.key).toBeGreaterThan(0);
      expect((item.text ?? "").trim().length, `${item.key} has no text`).toBeGreaterThan(10);
    }
  });

  it("Demilitarized Zone and the Cybernetic Research Facility say what they do", () => {
    const text = (id: string) => attachmentInfo(id).text;
    expect(text("dmz")).toMatch(/cannot be committed to, produced on, or placed on this planet/);
    expect(text("cybernetic")).toMatch(/yellow technology specialty/);
    expect(text("cyberneticstat")).toMatch(/increased by 1/);
    expect(text("warfare")).toMatch(/red technology specialty/);
  });
});
