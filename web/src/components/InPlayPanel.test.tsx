import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { InPlayPanel } from "./InPlayPanel.tsx";
import { MobileTopMenu } from "./MobileTopMenu.tsx";
import { buildInPlayModel } from "../presentation/inPlay.ts";

const model = buildInPlayModel({
  view: {
    round: 3,
    phase: "agenda",
    speaker: "p1",
    seating_order: ["p1", "p2"],
    players: [],
    board: { systems: {}, map_tiles: [] },
    table: { revealed_objectives: [], scored_objectives: {}, unclaimed_strategy_cards: [], strategy_card_goods: {}, laws: { rep_govt: "for" } },
  } as never,
  state: {
    discarded_action_cards: ["sabotage1"],
    agenda_log: [{ round: 2, agenda: "rep_govt", outcome: "for", law: true, enacted: true }],
  },
  seatName: (id) => ({ p1: "Ada", p2: "Bo" })[id] ?? null,
});

describe("InPlayPanel", () => {
  it("shows the law, filters by chip and search, and closes with Escape and Close", () => {
    const onClose = vi.fn();
    render(<InPlayPanel open wide model={model} onClose={onClose} />);
    const panel = screen.getByTestId("in-play-panel");
    expect(within(panel).getByTestId("in-play-count-laws")).toHaveTextContent("1");
    expect(within(panel).getByText("Representative Government")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("in-play-filter-discard"));
    expect(screen.queryByTestId("in-play-section-laws")).toBeNull();
    expect(screen.getByTestId("in-play-section-discard-action")).toBeInTheDocument();
    expect(screen.getByTestId("in-play-filter-discard")).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(screen.getByTestId("in-play-filter-all"));
    fireEvent.change(screen.getByTestId("in-play-search"), { target: { value: "zzzz" } });
    expect(screen.getByText("Nothing matches.")).toBeInTheDocument();

    fireEvent.keyDown(screen.getByTestId("in-play-side"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("in-play-close"));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("clamps long text and expands it with a keyboard-operable toggle", () => {
    render(<InPlayPanel open wide model={model} onClose={() => undefined} />);
    const more = screen.getAllByRole("button", { name: "More" })[0];
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    expect(screen.getAllByRole("button", { name: "Less" })[0]).toHaveAttribute("aria-expanded", "true");
  });

  it("is a sheet on a phone and notes what it leaves out", () => {
    render(<InPlayPanel open wide={false} model={model} onClose={() => undefined} />);
    expect(screen.getByTestId("in-play-sheet")).toBeInTheDocument();
    expect(screen.getByTestId("in-play-note")).toHaveTextContent(/draw piles/);
  });

  it("renders nothing while closed", () => {
    const { container } = render(<InPlayPanel open={false} wide model={model} onClose={() => undefined} />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("the phone menu entry", () => {
  it("has an In play button that closes the menu and opens the panel", () => {
    const onOpenInPlay = vi.fn();
    render(
      <MobileTopMenu attention={null} status="s" onOpenTechnologies={() => undefined} onOpenObjectives={() => undefined} onOpenInPlay={onOpenInPlay} />,
    );
    fireEvent.click(screen.getByTestId("top-menu-button"));
    fireEvent.click(screen.getByTestId("in-play-menu-button"));
    expect(onOpenInPlay).toHaveBeenCalledTimes(1);
  });
});
