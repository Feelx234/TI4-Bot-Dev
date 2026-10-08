import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { TacticalMovementOverlay } from "./TacticalMovementOverlay.tsx";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";

// 95.1: a carrier also picks up from the systems it moves through. 24 (origin) - 30 - 18 (active).
const choice: PendingChoiceDto = {
  prompt: "movement",
  actor: "p1",
  nonce: "n1",
  context: { subtype: "movement_step", target: { System: "18" } },
  options: [
    {
      id: "move|24|0",
      label: "Carrier",
      kind: "move",
      payload: { origin: "24", unit: "carrier", capacity: 4 },
    },
    { id: "done_moving", label: "Finish Movement", kind: "decline" },
  ],
};

const tile = (system_id: string, q: number, r: number) => ({
  system_id,
  label: `#${system_id}`,
  q,
  r,
});
const sys = (id: string, units: BoardView["systems"][string]["units"], tokens: string[] = []) => ({
  system_id: id,
  command_tokens: tokens,
  planets: {},
  units,
});

const board = (mid: BoardView["systems"][string]): BoardView => ({
  map_tiles: [tile("24", 0, 0), tile("30", 1, 0), tile("18", 2, 0)],
  systems: {
    "24": sys("24", [{ owner: "p1", unit_type: "carrier", damaged: false }]),
    "30": mid,
    "18": sys("18", []),
  },
});

const infantryOnPlanet = { owner: "p1", unit_type: "infantry", planet: "mid", damaged: false };
const fighterInSpace = { owner: "p1", unit_type: "fighter", damaged: false };

const renderOverlay = (b: BoardView, onSubmitBatch = vi.fn().mockResolvedValue(undefined)) => {
  render(
    <TacticalMovementOverlay
      choice={choice}
      board={b}
      activeSystemId="18"
      onSubmit={vi.fn().mockResolvedValue(undefined)}
      onSubmitBatch={onSubmitBatch}
      isOpen
      onClose={vi.fn()}
    />,
  );
  return onSubmitBatch;
};

describe("TacticalMovementOverlay en-route pickups (95.1)", () => {
  it("stages infantry on a planet of a system the carrier passes and plans the load", async () => {
    const onSubmitBatch = renderOverlay(
      board(sys("30", [infantryOnPlanet, infantryOnPlanet, fighterInSpace])),
    );
    expect(screen.getByTestId("rally-row-cargo-24-infantry-mid-via-30")).toHaveTextContent(
      "On the way: #30",
    );
    fireEvent.click(screen.getByTestId("rally-inc-24-carrier"));
    fireEvent.click(screen.getByTestId("rally-inc-cargo-24-infantry-mid-via-30"));
    fireEvent.click(screen.getByTestId("rally-inc-cargo-24-infantry-mid-via-30"));
    // only two infantry are there
    expect(screen.getByTestId("rally-inc-cargo-24-infantry-mid-via-30")).toBeDisabled();
    fireEvent.click(screen.getByTestId("rally-inc-cargo-24-fighter-space-via-30"));
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    await waitFor(() =>
      expect(onSubmitBatch).toHaveBeenCalledExactlyOnceWith("18", [
        { kind: "move", origin: "24", unit: "carrier", damaged: false },
        { kind: "load", origin: "24", unit: "infantry", source: "mid", damaged: false },
        { kind: "load", origin: "24", unit: "infantry", source: "mid", damaged: false },
        { kind: "load", origin: "24", unit: "fighter", source: null, damaged: false },
        { kind: "done_loading" },
        { kind: "done_moving" },
      ]),
    );
  });

  it("offers nothing from a system with the player's command token (95.5)", () => {
    renderOverlay(board(sys("30", [infantryOnPlanet], ["p1"])));
    expect(screen.queryByTestId("rally-row-cargo-24-infantry-mid-via-30")).toBeNull();
  });

  it("offers nothing when the passed system has no own units", () => {
    renderOverlay(board(sys("30", [{ ...infantryOnPlanet, owner: "p2" }])));
    expect(screen.queryByTestId("rally-row-cargo-24-infantry-mid-via-30")).toBeNull();
  });

  it("needs a carrier staged: capacity gates the en-route cargo too", () => {
    renderOverlay(board(sys("30", [infantryOnPlanet])));
    expect(screen.getByTestId("rally-inc-cargo-24-infantry-mid-via-30")).toBeDisabled();
  });

  it("refuses a space pickup the engine would fill from another system's unit of the same kind", () => {
    const b = board(sys("30", [fighterInSpace]));
    b.systems["24"].units.push(fighterInSpace);
    renderOverlay(b);
    fireEvent.click(screen.getByTestId("rally-inc-24-carrier"));
    expect(screen.getByTestId("rally-inc-cargo-24-fighter-space-via-30")).toBeDisabled();
  });
});
