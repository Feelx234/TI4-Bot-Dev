import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SystemInspector } from "./SystemInspector.tsx";
import { SelectedSystemDetails } from "../presentation/boardPresentation.ts";

const mockSystem: SelectedSystemDetails = {
  systemId: "18",
  label: "Mecatol Rex",
  anomalies: ["supernova"],
  wormholes: ["alpha"],
  specialArea: null,
  isActiveSystem: true,
  planets: [
    {
      id: "mecatol_rex",
      label: "Mecatol Rex",
      resources: 1,
      influence: 6,
      traits: ["cultural"],
      techSpecialties: [],
      legendary: true,
      controlledBy: "seat_a",
      controllerColor: "#ef4444",
      exhausted: false,
      attachments: ["custodians"],
      isCandidateTarget: true,
      isContextSubject: false,
      associatedOptionIds: ["opt_produce"],
    },
  ],
  spaceUnits: [{ unitType: "dreadnought", owner: "seat_a", ownerColor: "#ef4444", damaged: true }],
  planetUnits: {
    mecatol_rex: [{ unitType: "infantry", owner: "seat_a", ownerColor: "#ef4444", damaged: false }],
  },
  commandTokens: [{ owner: "seat_a", color: "#ef4444" }],
  availableActions: [
    { optionId: "opt_produce", label: "Produce 2 Fighters", kind: "produce_unit" },
  ],
};

describe("SystemInspector Component", () => {
  it("renders system details, anomalies, wormholes, and planets", () => {
    const onClose = vi.fn();
    render(<SystemInspector system={mockSystem} onClose={onClose} />);

    expect(screen.getByTestId("system-inspector")).toBeInTheDocument();
    expect(screen.getByTestId("inspector-system-title")).toHaveTextContent("Mecatol Rex");
    expect(screen.getByText("ACTIVE SYSTEM")).toBeInTheDocument();
    expect(screen.getByTestId("inspector-anomaly")).toHaveTextContent("SUPERNOVA");
    expect(screen.getByTestId("inspector-wormhole")).toHaveTextContent("WORMHOLE: ALPHA");

    // Planet
    expect(screen.getByTestId("inspector-planet-mecatol_rex")).toBeInTheDocument();
    expect(screen.getByText("1 Res / 6 Inf")).toBeInTheDocument();
    expect(screen.getAllByText(/Unknown participant/).length).toBeGreaterThan(0);
    expect(screen.getByTestId("system-inspector").textContent).not.toContain("seat_a");
    expect(screen.getByText("Attachments: custodians")).toBeInTheDocument();

    // Units
    expect(screen.getByTestId("inspector-space-unit")).toHaveTextContent(
      "dreadnought (Damaged) [ Unknown participant]",
    );
    expect(screen.getByTestId("inspector-ground-unit")).toHaveTextContent(
      "infantry [ Unknown participant]",
    );

    // Command Token
    expect(screen.getByTestId("inspector-command-token")).toHaveTextContent("Unknown participant");

    // Close button
    fireEvent.click(screen.getByTestId("close-inspector-button"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("triggers action selection callback when an available action is clicked", () => {
    const onSelectAction = vi.fn();
    render(
      <SystemInspector system={mockSystem} onClose={() => {}} onSelectAction={onSelectAction} />,
    );

    const actionBtn = screen.getByTestId("inspector-action-opt_produce");
    expect(actionBtn).toHaveTextContent("Produce 2 Fighters");
    fireEvent.click(actionBtn);
    expect(onSelectAction).toHaveBeenCalledWith("opt_produce");
  });

  it("renders nothing when system is null", () => {
    const { container } = render(<SystemInspector system={null} onClose={() => {}} />);
    expect(container.firstChild).toBeNull();
  });

  it("dismisses on Escape key", () => {
    const onClose = vi.fn();
    render(<SystemInspector system={mockSystem} onClose={onClose} />);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
