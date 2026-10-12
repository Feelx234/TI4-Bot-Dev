import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import {
  BOARD_PROMPT_INSET,
  BOARD_IDLE_INSET,
  boardStageStyle,
  BOARD_TOOLBAR_INSET,
  COMMAND_TOKEN_RADIUS,
  COMMAND_TOKEN_STEP,
  CONTROL_SYMBOL_FONT_SIZE,
} from "./boardLayout.ts";
import { Board } from "../Board.tsx";
import { SEAT_BADGE_SIZE, SeatBadge } from "../../presentation/PlayerIdentity.tsx";

/** Where the SVG's viewBox lands (meet) inside a stage of the given size. */
function fittedMap(viewBox: string, stage: { width: number; height: number }, hasPrompt: boolean) {
  const [, , w, h] = viewBox.split(" ").map(Number);
  const padding = Number(boardStageStyle(hasPrompt).paddingBottom);
  const scale = Math.min(stage.width / w, (stage.height - padding) / h);
  return { scale, freeBelow: stage.height - h * scale };
}

describe("map fit insets", () => {
  it("reserves room under the map for the prompt pill, a smaller one otherwise, and a 56px top row", () => {
    expect(BOARD_PROMPT_INSET).toBeGreaterThanOrEqual(100);
    expect(BOARD_IDLE_INSET).toBeLessThan(BOARD_PROMPT_INSET);
    expect(BOARD_TOOLBAR_INSET).toBe(56);
    expect(boardStageStyle(true).paddingBottom).toBe(BOARD_PROMPT_INSET);
    expect(boardStageStyle(false).paddingBottom).toBe(BOARD_IDLE_INSET);
  });

  it("always leaves the inset free below the fitted map", () => {
    for (const stage of [
      { width: 1110, height: 800 },
      { width: 950, height: 617 },
      { width: 694, height: 607 },
    ]) {
      expect(fittedMap("-550 -490 1100 980", stage, true).freeBelow).toBeGreaterThanOrEqual(BOARD_PROMPT_INSET);
      expect(fittedMap("-550 -490 1100 980", stage, false).freeBelow).toBeGreaterThanOrEqual(BOARD_IDLE_INSET);
    }
  });

  it("renders the toolbar row and the stage as siblings, so the toolbar never overlays the map", () => {
    const { getByTestId } = render(<Board board={{ systems: {}, map: [] } as never} seatingOrder={["a"]} />);
    const chrome = getByTestId("board-chrome");
    const stage = getByTestId("board-stage");
    expect(chrome.contains(stage)).toBe(false);
    expect(chrome.nextElementSibling).toBe(stage);
    expect(stage.querySelector('[data-testid="ti4-board-svg"]')).not.toBeNull();
    expect(chrome.querySelector(".board-seat-legend")).not.toBeNull();
    expect(stage.style.paddingBottom).toBe(`${BOARD_IDLE_INSET}px`);
  });
});

describe("seat marker sizes", () => {
  it("draws the badge glyph 40% over the old 20px and never under 18px", () => {
    expect(SEAT_BADGE_SIZE).toBeGreaterThanOrEqual(28);
    const { container } = render(<SeatBadge position={3} />);
    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("width")).toBe(String(SEAT_BADGE_SIZE));
    expect(svg.getAttribute("height")).toBe(String(SEAT_BADGE_SIZE));
    expect(container.querySelector(".seat-badge")!.getAttribute("aria-label")).toBe("Position 3");
  });

  it("keeps each seat's shape and colour on the larger badge", () => {
    const seen = new Set<string>();
    for (let position = 1; position <= 8; position++) {
      const { container } = render(<SeatBadge position={position} />);
      seen.add(container.querySelector("text")!.textContent! + container.querySelector("circle")!.getAttribute("fill"));
    }
    expect(seen.size).toBe(8);
  });

  it("draws command tokens and control glyphs on the board at 18px or more", () => {
    expect(COMMAND_TOKEN_RADIUS * 2).toBeGreaterThanOrEqual(18);
    expect(COMMAND_TOKEN_STEP).toBeGreaterThanOrEqual(COMMAND_TOKEN_RADIUS * 2);
    expect(CONTROL_SYMBOL_FONT_SIZE).toBeGreaterThanOrEqual(17);
  });
});
