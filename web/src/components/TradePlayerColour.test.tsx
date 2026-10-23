import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { TradeDeskModal } from "./TradeDeskModal.tsx";
import { PlayerChip, PlayerIdentityProvider } from "../presentation/PlayerIdentity.tsx";
import { SEAT_COLORS } from "../presentation/playerDisplay.ts";
import type { LobbyDto, PendingChoiceDto } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";

const lobby: LobbyDto = {
  game_id: "g",
  phase: "running",
  lobby_version: 1,
  host_player_id: "seat_1",
  slots: [
    { slot_id: "s1", position: 1, occupant: "seat_1", nickname: "Ana", connected: true, can_take_over: false },
    { slot_id: "s2", position: 4, occupant: "seat_2", nickname: "Bo", connected: true, can_take_over: false },
  ],
};

const wrap = (ui: React.ReactElement) =>
  render(
    <PlayerIdentityProvider lobby={lobby} seatingOrder={["seat_1", "seat_2"]}>
      {ui}
    </PlayerIdentityProvider>,
  );

const propose: PendingChoiceDto = {
  actor: "seat_1",
  nonce: "30",
  prompt: "Propose a transaction",
  context: partialContext({ subtype: "propose_transaction", target: { Player: "seat_2" } }),
  options: [
    { id: "cc1", label: "swap 1 commodity each", kind: "offer", payload: {} },
    { id: "decline", label: "Offer nothing", kind: "decline", payload: {} },
  ],
};

const answer: PendingChoiceDto = {
  actor: "seat_2",
  nonce: "31",
  prompt: "seat_1 offers 2 commodities for 2 trade goods -- accept?",
  context: partialContext({ subtype: "answer_transaction", target: { Player: "seat_1" } }),
  options: [
    { id: "accept", label: "Accept", kind: "answer", payload: { net: 0 } },
    { id: "refuse", label: "Refuse", kind: "decline", payload: {} },
  ],
};

describe("trade dialog shows the partner's seat colour", () => {
  it("PlayerChip pairs the seat badge with the name, never colour alone", () => {
    wrap(<PlayerChip id="seat_2" testId="chip" />);
    const chip = screen.getByTestId("chip");
    expect(chip).toHaveTextContent("Bo");
    expect(chip.getAttribute("data-seat-color")).toBe(SEAT_COLORS[3]);
    expect(chip.getAttribute("title")).toContain("red");
    expect(within(chip).getByLabelText("Position 4")).toBeInTheDocument();
  });

  it("PlayerChip falls back to the bare name for an unseated id", () => {
    wrap(<PlayerChip id="stranger" testId="chip" />);
    const chip = screen.getByTestId("chip");
    expect(chip.querySelector(".seat-badge")).toBeNull();
    expect(chip).toHaveTextContent("Unknown participant");
  });

  it("propose mode: header and the receive column name the partner with their colour", () => {
    wrap(
      <TradeDeskModal isOpen choice={propose} viewerSeat="seat_1" onSubmit={vi.fn()} onClose={vi.fn()} />,
    );
    const header = screen.getByTestId("trade-header-partner");
    expect(header).toHaveTextContent("Bo");
    expect(header.getAttribute("data-seat-color")).toBe(SEAT_COLORS[3]);
    const column = screen.getByTestId("stage-receive");
    expect(within(column).getByTestId("stage-partner")).toHaveTextContent("Bo");
    expect(within(column).getByLabelText("Position 4")).toBeInTheDocument();
  });

  it("answer mode: the proposer keeps their colour in the header", () => {
    wrap(
      <TradeDeskModal isOpen choice={answer} viewerSeat="seat_2" onSubmit={vi.fn()} onClose={vi.fn()} />,
    );
    const header = screen.getByTestId("trade-header-partner");
    expect(header).toHaveTextContent("Ana");
    expect(header.getAttribute("data-seat-color")).toBe(SEAT_COLORS[0]);
  });

  it("without a roster the dialog still renders with plain text", () => {
    render(<TradeDeskModal isOpen choice={propose} viewerSeat="seat_1" onSubmit={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByTestId("trade-staging-desk")).toBeInTheDocument();
    expect(document.querySelector(".seat-badge")).toBeNull();
  });
});
