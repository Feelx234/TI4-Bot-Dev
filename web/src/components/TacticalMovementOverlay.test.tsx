import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act, waitFor } from "@testing-library/react";
import { TacticalMovementOverlay } from "./TacticalMovementOverlay.tsx";
import { PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import { deriveChoiceRendererModel } from "../presentation/choiceModel.ts";

const mockMoveChoice: PendingChoiceDto = {
  prompt: "movement",
  actor: "p1",
  nonce: "nonce_move_step",
  context: {
    subtype: "movement_step",
    target: { System: "18" },
  },
  options: [
    {
      id: "move|24|0",
      label: "Cruiser",
      kind: "move",
      payload: { origin: "24", unit: "cruiser" },
    },
    {
      id: "move|24|1",
      label: "Carrier",
      kind: "move",
      payload: { origin: "24", unit: "carrier", capacity: 4 },
    },
    {
      id: "move|24|2",
      label: "Fighter",
      kind: "move",
      payload: { origin: "24", unit: "fighter" },
    },
    {
      id: "done_moving",
      label: "Finish Movement",
      kind: "decline",
    },
  ],
};

const mockPlayer: PlayerView = {
  id: "p1",
  faction: "sol",
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
};

describe("TacticalMovementOverlay Component", () => {
  it("offers and submits the engine decline-only movement choice", async () => {
    const choice: PendingChoiceDto = {
      actor: "p1",
      nonce: "0123456789abcdef",
      prompt: "movement",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [{ id: "done_moving", kind: "decline", label: "finish movement" }],
    };
    const model = deriveChoiceRendererModel(choice, "p1");
    expect(model?.workflow).toBe("tactical_movement");
    expect(model?.declineOption?.id).toBe("done_moving");
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={choice}
        model={model}
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText("No ships eligible to move into the active system.")).toBeVisible();
    expect(screen.getByTestId("fleet-supply-gauge")).toHaveTextContent("0 / 3 Ships");
    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("0 / 0 Loaded");
    const done = screen.getByTestId("commit-moves-btn");
    expect(done).toBeEnabled();
    expect(done).toHaveTextContent("Done Moving");
    fireEvent.click(done);
    await waitFor(() => expect(onSubmit).toHaveBeenCalledExactlyOnceWith("done_moving"));
  });

  it("reports an unresolvable finish action instead of submitting an arbitrary sole option", async () => {
    const choice: PendingChoiceDto = {
      actor: "p1",
      nonce: "0123456789abcdef",
      prompt: "movement",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [{ id: "unknown", kind: "mystery", label: "unrecognized action" }],
    };
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={choice}
        model={deriveChoiceRendererModel(choice, "p1")}
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen
        onClose={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(await screen.findByRole("alert")).toHaveTextContent(/finish movement/i);
  });

  it("reports a rejected direct submission and permits a retry", async () => {
    const choice: PendingChoiceDto = {
      actor: "p1",
      nonce: "0123456789abcdef",
      prompt: "movement",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [{ id: "done_moving", kind: "decline", label: "finish movement" }],
    };
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new Error("connection lost"))
      .mockResolvedValueOnce(undefined);
    render(
      <TacticalMovementOverlay
        choice={choice}
        model={deriveChoiceRendererModel(choice, "p1")}
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen
        onClose={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    expect(await screen.findByRole("alert")).toHaveTextContent("connection lost");
    await waitFor(() => expect(screen.getByTestId("commit-moves-btn")).toBeEnabled());
    fireEvent.click(screen.getByTestId("commit-moves-btn"));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not render when isOpen is false or choice is null", () => {
    const { container: c1 } = render(
      <TacticalMovementOverlay
        choice={null}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );
    expect(c1.firstChild).toBeNull();

    const { container: c2 } = render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={false}
        onClose={vi.fn()}
      />,
    );
    expect(c2.firstChild).toBeNull();
  });

  it("renders ship groups by origin with initial zero staged", () => {
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByTestId("tactical-movement-tray")).toBeInTheDocument();
    expect(screen.getByText("Destination: system 18")).toBeInTheDocument();
    expect(screen.getByTestId("rally-row-24-cruiser")).toBeInTheDocument();
    expect(screen.getByTestId("rally-row-24-carrier")).toBeInTheDocument();
    expect(screen.getByTestId("rally-row-24-fighter")).toBeInTheDocument();

    expect(screen.getByTestId("fleet-supply-gauge")).toHaveTextContent("0 / 3 Ships");
    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("0 / 0 Loaded");
  });

  it("updates fleet supply and cargo capacity when staging ships", () => {
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    // Stage 1 Carrier (+1 fleet, +4 capacity)
    const carrierInc = screen.getByTestId("rally-inc-24-carrier");
    fireEvent.click(carrierInc);

    expect(screen.getByTestId("fleet-supply-gauge")).toHaveTextContent("1 / 3 Ships");
    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("0 / 4 Loaded");

    // Stage 1 Fighter (+1 cargo)
    const fighterInc = screen.getByTestId("rally-inc-24-fighter");
    fireEvent.click(fighterInc);

    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("1 / 4 Loaded");
    expect(screen.getByTestId("commit-moves-btn")).toHaveTextContent("Commit Moves (2)");
  });

  it("submits done_moving when finish movement button is clicked", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    const finishBtn = screen.getByTestId("finish-movement-btn");
    await act(async () => {
      fireEvent.click(finishBtn);
    });
    expect(onSubmit).toHaveBeenCalledWith("done_moving");
  });

  it("submits staged moves sequentially using semantic intent matching", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <TacticalMovementOverlay
        choice={mockMoveChoice}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    // Stage Cruiser
    fireEvent.click(screen.getByTestId("rally-inc-24-cruiser"));

    const commitBtn = screen.getByTestId("commit-moves-btn");
    await act(async () => {
      fireEvent.click(commitBtn);
    });

    // Submits the first matched option for cruiser from 24
    expect(onSubmit).toHaveBeenCalledWith("move|24|0");
  });

  it("reads true unit count from boardView, formats unit names, and shows UnitIcons", () => {
    const choiceWithSolCarrier: PendingChoiceDto = {
      prompt: "movement",
      actor: "p1",
      nonce: "nonce_sol",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [
        {
          id: "move|24|0",
          label: "Sol Carrier",
          kind: "move",
          payload: { origin: "24", unit: "sol_carrier", capacity: 4 },
        },
        {
          id: "move|24|1",
          label: "Cruiser",
          kind: "move",
          payload: { origin: "24", unit: "cruiser" },
        },
        { id: "done_moving", kind: "decline", label: "finish movement" },
      ],
    };

    const mockBoard = {
      systems: {
        "24": {
          system_id: "24",
          command_tokens: [],
          planets: {},
          units: [
            { unit_type: "sol_carrier", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
          ],
        },
      },
    };

    render(
      <TacticalMovementOverlay
        choice={choiceWithSolCarrier}
        board={mockBoard as any}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    // Title Case header and clear instruction
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Move Units");
    expect(
      screen.getByText("Select ships and cargo to rally into the active system"),
    ).toBeInTheDocument();
    expect(screen.queryByText("movement")).not.toBeInTheDocument();

    // Friendly display name for sol_carrier
    expect(screen.getByText("Carrier")).toBeInTheDocument();
    // UnitIcon rendered
    const icons = screen.getAllByRole("img");
    expect(icons.length).toBeGreaterThan(0);

    // Cruisers count derived from boardView: 3 available instead of 1
    expect(screen.getByText(/Origin: #24 • Available: 3/)).toBeInTheDocument();

    // Can increment cruiser up to 3
    const cruiserInc = screen.getByTestId("rally-inc-24-cruiser");
    fireEvent.click(cruiserInc);
    fireEvent.click(cruiserInc);
    fireEvent.click(cruiserInc);
    expect(screen.getByTestId("rally-count-24-cruiser")).toHaveTextContent("3");
    expect(cruiserInc).toBeDisabled();
  });

  it("permits exceeding fleet supply with an advisory warning while keeping commit enabled", () => {
    const choiceWithManyShips: PendingChoiceDto = {
      prompt: "movement",
      actor: "p1",
      nonce: "nonce_fleet",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [
        {
          id: "move|24|0",
          label: "Cruiser",
          kind: "move",
          payload: { origin: "24", unit: "cruiser" },
        },
        { id: "done_moving", kind: "decline", label: "finish movement" },
      ],
    };

    const mockBoard = {
      systems: {
        "24": {
          system_id: "24",
          command_tokens: [],
          planets: {},
          units: [
            { unit_type: "cruiser", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
            { unit_type: "cruiser", owner: "p1", damaged: false },
          ],
        },
      },
    };

    // Fleet tokens = 2 (so staging 4 cruisers exceeds the fleet limit)
    const constrainedPlayer: PlayerView = {
      ...mockPlayer,
      fleet_tokens: 2,
    };

    render(
      <TacticalMovementOverlay
        choice={choiceWithManyShips}
        board={mockBoard as any}
        activeSystemId="18"
        player={constrainedPlayer}
        onSubmit={vi.fn()}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    const cruiserInc = screen.getByTestId("rally-inc-24-cruiser");
    fireEvent.click(cruiserInc);
    fireEvent.click(cruiserInc);
    fireEvent.click(cruiserInc);

    // 3 cruisers staged against fleet supply of 2
    expect(screen.getByTestId("fleet-supply-gauge")).toHaveTextContent("3 / 2 Ships");
    expect(
      screen.getByText(/Exceeds fleet limit \(3\/2\) — excess ships must be lost in combat or destroyed/i),
    ).toBeInTheDocument();

    // Commit button remains ENABLED (attacking over fleet supply is valid TI4 rules)
    const commitBtn = screen.getByTestId("commit-moves-btn");
    expect(commitBtn).toBeEnabled();
    expect(commitBtn).toHaveTextContent("Commit Moves (3)");
  });

  it("stages pooled cargo from origin and enqueues load intents in the pipeline", async () => {
    const choiceWithCarrier: PendingChoiceDto = {
      prompt: "movement",
      actor: "p1",
      nonce: "nonce_cargo",
      context: { subtype: "movement_step", target: { System: "18" } },
      options: [
        {
          id: "move|24|0",
          label: "Carrier",
          kind: "move",
          payload: { origin: "24", unit: "carrier", capacity: 4 },
        },
        { id: "done_moving", kind: "decline", label: "finish movement" },
      ],
    };

    const mockBoard = {
      systems: {
        "24": {
          system_id: "24",
          command_tokens: [],
          planets: {
            jord: { planet_id: "jord", exhausted: false },
          },
          units: [
            { unit_type: "carrier", owner: "p1", damaged: false },
            { unit_type: "infantry", owner: "p1", planet: "jord", damaged: false },
            { unit_type: "infantry", owner: "p1", planet: "jord", damaged: false },
          ],
        },
      },
    };

    const onSubmit = vi.fn().mockResolvedValue(undefined);

    render(
      <TacticalMovementOverlay
        choice={choiceWithCarrier}
        board={mockBoard as any}
        activeSystemId="18"
        player={mockPlayer}
        onSubmit={onSubmit}
        isOpen={true}
        onClose={vi.fn()}
      />,
    );

    // Carrier is staged
    fireEvent.click(screen.getByTestId("rally-inc-24-carrier"));
    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("0 / 4 Loaded");

    // Carryable cargo section is visible
    expect(screen.getByText("Carryable Cargo in Origin Systems")).toBeInTheDocument();

    // Stage 2 infantry from Jord
    const infInc = screen.getByTestId("rally-inc-24-infantry");
    fireEvent.click(infInc);
    fireEvent.click(infInc);

    expect(screen.getByTestId("cargo-capacity-gauge")).toHaveTextContent("2 / 4 Loaded");

    // Commit moves
    const commitBtn = screen.getByTestId("commit-moves-btn");
    await act(async () => {
      fireEvent.click(commitBtn);
    });

    // Submits the carrier move first
    expect(onSubmit).toHaveBeenCalledWith("move|24|0");
  });
});

