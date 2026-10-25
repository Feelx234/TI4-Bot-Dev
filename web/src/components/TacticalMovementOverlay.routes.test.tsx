import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TacticalMovementOverlay } from "./TacticalMovementOverlay.tsx";
import type {
  BoardView,
  MovementFacts,
  PendingChoiceDto,
  RouteFact,
} from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";

// Rules version 29: a ship with more than one way to the active system gets a route choice.
//   24 (origin) - 30 (gravity rift) - 18 (active)      the default: rolls for the rift
//   24 - 31 - 32 - 18                                  the way round: spends Gravity Drive
const choice: PendingChoiceDto = {
  prompt: "movement",
  actor: "p1",
  nonce: "n1",
  context: partialContext({ subtype: "movement_step", target: { System: "18" } }),
  options: [
    {
      id: "move|24|0",
      label: "Cruiser",
      kind: "move",
      payload: { origin: "24", unit: "cruiser", capacity: 0 },
    },
    { id: "done_moving", label: "Finish Movement", kind: "decline", payload: {} },
  ],
};

const tile = (system_id: string, q: number, r: number, anomalies: string[] = []) => ({
  system_id,
  label: `#${system_id}`,
  q,
  r,
  anomalies,
});
const sys = (id: string, units: BoardView["systems"][string]["units"]) => ({
  system_id: id,
  command_tokens: [],
  planets: {},
  units,
});

const board: BoardView = {
  map_tiles: [
    tile("24", 0, 0),
    tile("30", 1, 0, ["gravity rift"]),
    tile("18", 2, 0),
    tile("31", 0, 1),
    tile("32", 1, 1),
  ],
  systems: {
    "24": sys("24", [
      { owner: "p1", unit_type: "cruiser", damaged: false, planet: null, galvanized: false },
    ]),
    "30": sys("30", []),
    "18": sys("18", []),
    "31": sys("31", []),
    "32": sys("32", []),
  },
};

const through: RouteFact = {
  path: ["24", "30", "18"],
  default: true,
  gravity_drive: false,
  ionian: false,
  rifts: ["30"],
  wormholes: 0,
  anomalies: ["30"],
};
const round: RouteFact = {
  path: ["24", "31", "32", "18"],
  default: false,
  gravity_drive: true,
  ionian: false,
  rifts: [],
  wormholes: 0,
  anomalies: [],
};

const facts = (routes: RouteFact[]): MovementFacts => ({
  active: "18",
  ships: [
    {
      origin: "24",
      index: 0,
      unit: "cruiser",
      damaged: false,
      capacity: 0,
      fleet: true,
      move_value: 2,
      move: {
        gravity_drive: false,
        ionian: false,
        path: ["24", "30", "18"],
        rifts: ["30"],
        ...(routes.length > 1 ? { routes } : {}),
      },
    },
  ],
  cargo: [],
  gravity_drive: true,
  ionian: false,
  fleet_supply: { limit: 4, charged: 0 },
  moved: 0,
});

const renderOverlay = (f: MovementFacts) => {
  const onSubmitBatch = vi.fn().mockResolvedValue(undefined);
  render(
    <TacticalMovementOverlay
      choice={choice}
      board={board}
      facts={f}
      activeSystemId="18"
      onSubmit={vi.fn().mockResolvedValue(undefined)}
      onSubmitBatch={onSubmitBatch}
      isOpen
      onClose={vi.fn()}
    />,
  );
  return onSubmitBatch;
};

describe("TacticalMovementOverlay routes (rules version 29)", () => {
  it("shows no route choice for a ship with one way", () => {
    renderOverlay(facts([through]));
    fireEvent.click(screen.getByTestId("rally-inc-24-cruiser"));
    expect(screen.queryByTestId("route-picker-24-cruiser")).toBeNull();
  });

  it("offers the routes once the ship is staged, the default preselected", () => {
    renderOverlay(facts([through, round]));
    expect(screen.queryByTestId("route-picker-24-cruiser")).toBeNull();
    fireEvent.click(screen.getByTestId("rally-inc-24-cruiser"));
    expect(screen.getByTestId("route-picker-24-cruiser")).toBeInTheDocument();
    expect(screen.getByTestId("route-24-cruiser-0")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByTestId("route-24-cruiser-0")).toHaveTextContent("Default");
    expect(screen.getByTestId("route-24-cruiser-0")).toHaveTextContent("roll for destruction");
    expect(screen.getByTestId("route-24-cruiser-1")).toHaveTextContent("Spends Gravity Drive");
  });

  it("sends a move without a route for the default", async () => {
    const onSubmitBatch = renderOverlay(facts([through, round]));
    fireEvent.click(screen.getByTestId("rally-inc-24-cruiser"));
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    await waitFor(() =>
      expect(onSubmitBatch).toHaveBeenCalledExactlyOnceWith("18", [
        { kind: "move", origin: "24", unit: "cruiser", damaged: false },
        { kind: "done_moving" },
      ]),
    );
  });

  it("sends the chosen route in the move step", async () => {
    const onSubmitBatch = renderOverlay(facts([through, round]));
    fireEvent.click(screen.getByTestId("rally-inc-24-cruiser"));
    fireEvent.click(screen.getByTestId("route-24-cruiser-1"));
    expect(screen.getByTestId("route-24-cruiser-1")).toHaveAttribute("aria-checked", "true");
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    await waitFor(() =>
      expect(onSubmitBatch).toHaveBeenCalledExactlyOnceWith("18", [
        { kind: "move", origin: "24", unit: "cruiser", damaged: false, route: 1 },
        { kind: "done_moving" },
      ]),
    );
  });
});
