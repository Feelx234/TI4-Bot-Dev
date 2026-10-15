import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MapPreviewBoard } from "../MapPreviewBoard.tsx";
import { EDGE_DIRS } from "./hyperlaneNetwork.ts";
import { parseHyperlaneId } from "./hyperlaneGeometry.ts";
import type { MapPreviewDto } from "../../protocol/types.ts";

/**
 * A second, differently built oracle: nodes are the physical edges between two cells (a sorted
 * pair of cell coordinates), a printed lane joins the two physical edges of its tile, and a
 * system touches the physical edges that face a hyperlane tile with a lane end there.
 */
function expectedPairs(tiles: { system_id: string; q: number; r: number }[]): Set<string> {
  const cells = new Map(tiles.map((t) => [`${t.q},${t.r}`, t]));
  const edgeNode = (q: number, r: number, e: number) => {
    const [dq, dr] = EDGE_DIRS[e];
    return [`${q},${r}`, `${q + dq},${r + dr}`].sort().join("~");
  };
  const parent = new Map<string, string>();
  const find = (x: string): string => {
    if (!parent.has(x)) parent.set(x, x);
    let p = parent.get(x)!;
    while (p !== parent.get(p)) p = parent.get(p)!;
    parent.set(x, p);
    return p;
  };
  const laneEnds = new Set<string>();
  for (const t of tiles) {
    const lane = parseHyperlaneId(t.system_id);
    if (!lane) continue;
    for (const [a, b] of lane.lanes) {
      parent.set(find(edgeNode(t.q, t.r, a)), find(edgeNode(t.q, t.r, b)));
      laneEnds.add(`${t.q},${t.r}:${a}`);
      laneEnds.add(`${t.q},${t.r}:${b}`);
    }
  }
  const touching: { cell: string; node: string }[] = [];
  for (const t of tiles) {
    if (parseHyperlaneId(t.system_id)) continue;
    for (let e = 0; e < 6; e++) {
      const [dq, dr] = EDGE_DIRS[e];
      const n = cells.get(`${t.q + dq},${t.r + dr}`);
      if (n && parseHyperlaneId(n.system_id) && laneEnds.has(`${n.q},${n.r}:${(e + 3) % 6}`)) {
        touching.push({ cell: `${t.q},${t.r}`, node: edgeNode(t.q, t.r, e) });
      }
    }
  }
  const out = new Set<string>();
  for (const a of touching)
    for (const b of touching)
      if (a.cell !== b.cell && find(a.node) === find(b.node)) out.add([a.cell, b.cell].sort().join("|"));
  return out;
}

const dir = resolve(process.cwd(), "e2e/screenshots/CO-community-maps/server-snapshot");
const rows: string[] = [];

describe("every linked pair is drawn and individually identifiable", () => {
  for (const file of readdirSync(dir).filter((f) => f.endsWith("p.json")).sort()) {
    const { previews } = JSON.parse(readFileSync(resolve(dir, file), "utf8")) as {
      previews: Record<string, MapPreviewDto>;
    };
    for (const [alias, preview] of Object.entries(previews)) {
      const tiles = preview.tiles.filter((t) => !t.special_area);
      if (!tiles.some((t) => parseHyperlaneId(t.system_id))) continue;
      it(alias, () => {
        const expected = expectedPairs(tiles);
        const { container, unmount } = render(<MapPreviewBoard preview={preview} />);
        const key = (id: string) => id.split("|").map((k) => k).sort().join("|");
        const arcs = [...container.querySelectorAll('[data-testid="hyperlane-link"]')].map((n) => key(n.getAttribute("data-pair")!));
        const buttons = [...container.querySelectorAll('[data-testid="hyperlane-pair"]')].map((n) => key(n.getAttribute("data-pair")!));
        expect(new Set(arcs).size, "arcs unique").toBe(arcs.length);
        expect(new Set(arcs)).toEqual(expected);
        expect(new Set(buttons)).toEqual(expected);
        // each arc has its own geometry
        const ds = [...container.querySelectorAll('[data-testid="hyperlane-link"] path:first-of-type')].map((n) => n.getAttribute("d"));
        expect(new Set(ds).size).toBe(ds.length);
        // each linked system names its partners in its tooltip
        const linked = container.querySelectorAll('[data-linked="true"]').length;
        expect(linked).toBe(new Set([...expected].flatMap((p) => p.split("|"))).size);
        rows.push(`${alias}\t${expected.size}\t${arcs.length}\t${buttons.length}`);
        unmount();
      });
    }
  }
  it("writes the table", () => {
    if (process.env.PAIR_TABLE) writeFileSync(process.env.PAIR_TABLE, rows.join("\n"));
  });
});
