import { render, screen, within } from "@testing-library/react";
import { expect, it } from "vitest";
import { GameProvider } from "../game/context";
import { ReferenceDrawer } from "../game/shell/Reference";
import menuUpdate from "../session/fixtures/update-menu.json";
import { selectShell } from "../session/select/shell";
import type { SessionUpdate } from "../session/wire";

// The Objectives sheet over an update of the real engine with eight seats, viewed by seat a
// (Sol). The fixture is older than the `score` fact, so the points are written here. The test is
// here because `dev` is the layer that may import both the session and the components.
const update: SessionUpdate = {
  ...(menuUpdate as unknown as SessionUpdate),
  score: {
    target: 10,
    public: [
      {
        id: "expand_borders",
        name: "Expand Borders",
        points: 1,
        text: "Control 6 planets in non-home systems.",
        scored: ["a", "b"],
      },
      {
        id: "master_science",
        name: "Master the Sciences",
        points: 2,
        text: "Own 2 technologies in each of 4 colors.",
        scored: ["d"],
      },
    ],
    sources: [
      { kind: "secrets", name: "", points: { a: 2, d: 1 } },
      { kind: "custodians", name: "", points: { b: 1 } },
      { kind: "imperial", name: "", points: {} },
      { kind: "support", name: "", points: { a: 1 }, from: { a: ["b"] } },
      { kind: "agenda", name: "Mutiny", points: { d: -1 } },
    ],
  },
};
const local = {
  inspected: null,
  staged: null,
  sent: null,
  error: null,
  replaying: null,
  canUndo: false,
  movement: {},
  landing: {},
  landingKind: null,
  production: { builds: [], planets: [], goods: 0 },
  productionStage: "build" as const,
  handled: [],
  step: null,
  remaining: null,
  planNote: null,
  stepping: null,
  draftLocked: null,
  draft: null,
};

function open(shown = update) {
  const view = selectShell(shown, local, null);
  render(
    <GameProvider seats={view.seats} dispatch={() => {}}>
      <ReferenceDrawer
        id="objectives"
        view={view.reference}
        players={view.players.rows}
        onClose={() => {}}
      />
    </GameProvider>,
  );
}
const row = (name: string) => within(screen.getByRole("article", { name }));
const names = (items: HTMLElement[]) => items.map((item) => item.getAttribute("aria-label"));
/** The cells of a row by the faction of their seat: the columns are in the order of the points. */
const cells = (name: string) =>
  Object.fromEntries(
    row(name)
      .getAllByRole("listitem")
      .map((item) => [item.getAttribute("aria-label")?.split(":")[0], item]),
  );
const cellName = (name: string, faction: string) =>
  cells(name)[faction]?.getAttribute("aria-label");

it("has the viewer first in the heading, then the seats by their points", () => {
  open();
  expect(screen.getByText("10 VP to win")).toBeInTheDocument();
  const seats = within(screen.getByRole("list", { name: "Victory points" })).getAllByRole(
    "listitem",
  );
  expect(seats).toHaveLength(8);
  expect(names(seats)[0]).toBe("Sol (you): 0 points");
  const points = names(seats)
    .slice(1)
    .map((name) => Number(name?.match(/: (\d+) point/)?.[1]));
  expect(points).toEqual([...points].sort((a, b) => b - a));
  expect(names(seats)).toContain("Hacan: 1 point");
  expect(names(seats)).toContain("Xxcha: 1 point");
});

it("lists the public objectives by stage, as printed, with who scored each", () => {
  open();
  const first = within(screen.getByRole("region", { name: "Public objectives · stage I" }));
  expect(first.getAllByRole("article")).toHaveLength(1);
  expect(row("Expand Borders").getByText("1 VP")).toBeInTheDocument();
  expect(
    row("Expand Borders").getByText("Control 6 planets in non-home systems."),
  ).toBeInTheDocument();
  expect(cellName("Expand Borders", "Sol")).toBe("Sol: scored");
  expect(cellName("Expand Borders", "Hacan")).toBe("Hacan: scored");
  expect(cellName("Expand Borders", "Letnev")).toBe("Letnev: nothing");
  expect(cellName("Expand Borders", "Xxcha")).toBe("Xxcha: nothing");
  expect(row("Expand Borders").getAllByText("✓")).toHaveLength(2);
  const second = within(screen.getByRole("region", { name: "Public objectives · stage II" }));
  expect(second.getByRole("article", { name: "Master the Sciences" })).toBeInTheDocument();
  expect(row("Master the Sciences").getByText("2 VP")).toBeInTheDocument();
});

it("has every other source of a point, with one cell for each seat", () => {
  open();
  const other = within(screen.getByRole("region", { name: "Other points" }));
  expect(other.getAllByRole("article").map((item) => item.getAttribute("aria-label"))).toEqual([
    "Secret objectives",
    "Custodians of Mecatol Rex",
    "Imperial · Mecatol Rex",
    "Support for the Throne",
    "Agenda · Mutiny",
  ]);
  for (const item of screen.getAllByRole("article")) {
    expect(within(item).getAllByRole("listitem")).toHaveLength(8);
  }
  expect(cellName("Secret objectives", "Sol")).toBe("Sol: 2 points");
  expect(cellName("Secret objectives", "Hacan")).toBe("Hacan: nothing");
  expect(cellName("Secret objectives", "Letnev")).toBe("Letnev: nothing");
  expect(cellName("Secret objectives", "Xxcha")).toBe("Xxcha: 1 point");
  // A source that nobody has a point from keeps its row.
  expect(row("Imperial · Mecatol Rex").queryByText(/\d/)).not.toBeInTheDocument();
  // Support for the Throne shows whose note it is.
  expect(cellName("Support for the Throne", "Sol")).toBe("Sol: 1 point from Hacan");
  expect(
    within(cells("Support for the Throne").Sol).getByRole("img", { name: "Hacan" }),
  ).toBeInTheDocument();
  // A loss has the minus sign.
  expect(cellName("Agenda · Mutiny", "Xxcha")).toBe("Xxcha: −1 point");
  expect(row("Agenda · Mutiny").getByText("−1")).toBeInTheDocument();
});

it("says when no objective of a stage is revealed, and shows nothing without the fact", () => {
  open({ ...update, score: { ...update.score!, public: [] } });
  expect(screen.getAllByText("None revealed")).toHaveLength(2);
});
