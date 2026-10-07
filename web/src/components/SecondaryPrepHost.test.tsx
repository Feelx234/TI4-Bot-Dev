import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { GameEvent, HistoryStatus, PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import { SecondaryPrepHost, type SecondaryPrepHostProps } from "./SecondaryPrepHost.tsx";
import { PlayerSheet } from "./PlayerSheet.tsx";
import { SECONDARY_PREP_MODE_KEY } from "../hooks/useSecondaryPrepMode.ts";
import { AUTO_PLAY_DELAY_MS } from "../hooks/useSecondaryAutoPlay.ts";
import {
  actionEvent,
  boardWith,
  option,
  playedLog,
  player,
  secondaryChoice,
  stepChoice,
} from "../test/secondaryPrepFixtures.ts";

const hist = (cursor: number, generation = 0, redo_count = 0): HistoryStatus => ({ cursor, redo_count, generation });
const players = (card = "pok7technology", extra: Partial<PlayerView> = {}): PlayerView[] => [
  player("a", { strategy_cards: [card, "pok1leadership"] }),
  player("b", extra),
  player("c"),
];

const base = (over: Partial<SecondaryPrepHostProps> = {}): SecondaryPrepHostProps => ({
  gameId: "g1",
  viewerSeat: "b",
  players: players(),
  events: playedLog("Technology"),
  board: boardWith([{ system: "18", planet: "jord", owner: "b", exhausted: true }]),
  phase: "action",
  activePlayer: "a",
  history: hist(5),
  choice: null,
  busy: false,
  onSubmitChoice: vi.fn().mockResolvedValue(undefined),
  onSubmitBasketBatch: vi.fn().mockResolvedValue(undefined),
  ...over,
});

const open = () => fireEvent.click(screen.getByTestId("secondary-prep-chip"));

describe("waiting: the prepare panel", () => {
  beforeEach(() => localStorage.clear());

  it("offers 'Prepare your secondary' to an eligible follower who has not been asked", () => {
    render(<SecondaryPrepHost {...base()} />);
    expect(screen.getByTestId("secondary-prep-chip").textContent).toMatch(/Prepare your secondary/);
    expect(screen.queryByTestId("secondary-prepared-badge")).toBeNull();
  });

  it("shows nothing to spectators, the primary player, tokenless followers, or once asked", () => {
    const hidden: Partial<SecondaryPrepHostProps>[] = [
      { viewerSeat: null },
      { viewerSeat: "a" },
      { players: players("pok7technology", { strategic_tokens: 0 }) },
      { events: [...playedLog("Technology"), actionEvent("action_5", "b", { detail: "b followed" })] },
      { events: [] },
      { activePlayer: "b" },
      { phase: "status" },
    ];
    for (const over of hidden) {
      const { container, unmount } = render(<SecondaryPrepHost {...base(over)} />);
      expect(container.querySelector("[data-testid=secondary-prep]")).toBeNull();
      unmount();
    }
  });

  it("hides the panel while the viewer has a decision pending", () => {
    render(<SecondaryPrepHost {...base({ choice: stepChoice("reaction_when_x", [option("a")]) })} />);
    expect(screen.queryByTestId("secondary-prep-chip")).toBeNull();
  });

  it("sets a 'Prepared' badge, keeps it on this device, and clears it", () => {
    render(<SecondaryPrepHost {...base()} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    expect(screen.getByTestId("secondary-prepared-badge").textContent).toBe("Prepared");
    fireEvent.click(screen.getByTestId("prep-tech-amd"));
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toContain("amd");
    fireEvent.click(screen.getByTestId("prep-clear"));
    expect(screen.queryByTestId("secondary-prepared-badge")).toBeNull();
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toBeNull();
  });

  it("is not shown to other seats as a ready indicator: nothing is sent anywhere", () => {
    const props = base();
    render(<SecondaryPrepHost {...props} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    expect(props.onSubmitChoice).not.toHaveBeenCalled();
    expect(props.onSubmitBasketBatch).not.toHaveBeenCalled();
  });

  it("warns on Trade that a replenish removes the window", () => {
    render(<SecondaryPrepHost {...base({ players: players("pok5trade"), events: playedLog("Trade") })} />);
    open();
    expect(screen.getByTestId("prep-trade-warning").textContent).toMatch(/replenishes you/);
  });

  it("offers per-card detail pickers: planets for Diplomacy, sites for Construction", () => {
    const { unmount } = render(
      <SecondaryPrepHost {...base({ players: players("pok2diplomacy"), events: playedLog("Diplomacy") })} />,
    );
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    fireEvent.click(screen.getByTestId("prep-planet-jord"));
    expect(screen.getByTestId("prep-planet-jord").getAttribute("aria-pressed")).toBe("true");
    unmount();
    localStorage.clear();
    render(
      <SecondaryPrepHost {...base({ players: players("pok4construction"), events: playedLog("Construction") })} />,
    );
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    fireEvent.click(screen.getByTestId("prep-site-jord"));
    fireEvent.click(screen.getByTestId("prep-unit-spacedock"));
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toContain("spacedock");
  });
});

describe("invalidation", () => {
  beforeEach(() => localStorage.clear());

  const prepare = () => {
    const view = render(<SecondaryPrepHost {...base()} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    return view;
  };

  it("drops the plan on a new strategic action", () => {
    const view = prepare();
    view.rerender(
      <SecondaryPrepHost
        {...base({
          events: [...playedLog("Technology"), actionEvent("action_9", "c", { action_actor: "c", detail: "c played Politics" })],
          activePlayer: "c",
          players: [player("a"), player("b"), player("c", { strategy_cards: ["pok3politics"] })],
        })}
      />,
    );
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toBeNull();
  });

  it("drops the plan when the action is cancelled (the turn moves on)", () => {
    const view = prepare();
    view.rerender(<SecondaryPrepHost {...base({ activePlayer: "c" })} />);
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toBeNull();
  });

  it("drops the plan when history changes generation (undo past the action)", () => {
    const view = prepare();
    view.rerender(<SecondaryPrepHost {...base({ history: hist(3, 1) })} />);
    expect(localStorage.getItem("ti4_secondary_prepared:g1:b")).toBeNull();
  });
});

describe("review mode (the default): the prepared answer in the real decision", () => {
  beforeEach(() => localStorage.clear());

  /** Prepares a plan, then lets the real question arrive after the game moved forward. */
  function arrive(choice: PendingChoiceDto, props: Partial<SecondaryPrepHostProps> = {}) {
    const shared = base(props);
    const view = render(<SecondaryPrepHost {...shared} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    fireEvent.click(screen.getByTestId("prep-tech-amd"));
    const events: GameEvent[] = [...(shared.events ?? [])];
    view.rerender(<SecondaryPrepHost {...shared} events={events} history={hist(6)} choice={choice} />);
    return { shared, view };
  }

  it("pre-fills the window and confirms with one click, sending nothing by itself", async () => {
    const { shared } = arrive(secondaryChoice("pok7technology"));
    expect(screen.getByTestId("secondary-prepared-bar").textContent).toMatch(/Follow Technology/);
    expect(shared.onSubmitChoice).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByTestId("secondary-prepared-confirm"));
    });
    expect(shared.onSubmitChoice).toHaveBeenCalledTimes(1);
    expect(shared.onSubmitChoice).toHaveBeenCalledWith("yes");
  });

  it("uses the faction waiver option when that is what the engine offers", async () => {
    const waiver = secondaryChoice("pok7technology", {}, [option("no"), option("follow|waived|0|w", "strategy", "free")]);
    const { shared } = arrive(waiver);
    await act(async () => {
      fireEvent.click(screen.getByTestId("secondary-prepared-confirm"));
    });
    expect(shared.onSubmitChoice).toHaveBeenCalledWith("follow|waived|0|w");
  });

  it("lets the player choose differently without sending anything", () => {
    const { shared } = arrive(secondaryChoice("pok7technology"));
    fireEvent.click(screen.getByTestId("secondary-prepared-dismiss"));
    expect(screen.queryByTestId("secondary-prepared-bar")).toBeNull();
    expect(shared.onSubmitChoice).not.toHaveBeenCalled();
  });

  it("shows 'Needs review' instead of pre-filling when the prepared intent no longer validates", () => {
    arrive(secondaryChoice("pok7technology", {}, [option("no")]));
    expect(screen.getByTestId("secondary-needs-review").textContent).toMatch(/Needs review/);
    expect(screen.queryByTestId("secondary-prepared-confirm")).toBeNull();
  });

  it("prefills the technology pick after the window was answered, and flags an unavailable one", async () => {
    const { shared, view } = arrive(secondaryChoice("pok7technology"));
    const asked = [...(shared.events ?? []), actionEvent("action_5", "b", { detail: "b followed" })];
    view.rerender(
      <SecondaryPrepHost
        {...shared}
        events={asked}
        history={hist(7)}
        choice={stepChoice("research_technology", [option("amd", "research"), option("decline", "decline")])}
      />,
    );
    expect(screen.getByTestId("secondary-prepared-bar").textContent).toMatch(/Research/);
    await act(async () => {
      fireEvent.click(screen.getByTestId("secondary-prepared-confirm"));
    });
    expect(shared.onSubmitChoice).toHaveBeenCalledWith("amd");
    view.rerender(
      <SecondaryPrepHost
        {...shared}
        events={asked}
        history={hist(8)}
        choice={stepChoice("research_technology", [option("nm", "research")], "other")}
      />,
    );
    expect(screen.getByTestId("secondary-needs-review")).toBeTruthy();
  });

  it("does not apply the plan to an unrelated research prompt after a reload", () => {
    const shared = base();
    const first = render(<SecondaryPrepHost {...shared} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    fireEvent.click(screen.getByTestId("prep-tech-amd"));
    first.unmount();
    render(
      <SecondaryPrepHost
        {...shared}
        choice={stepChoice("research_technology", [option("amd", "research")])}
      />,
    );
    expect(screen.queryByTestId("secondary-prepared-bar")).toBeNull();
    expect(screen.queryByTestId("secondary-needs-review")).toBeNull();
  });

  it("never auto-plays in Review mode, even when the game moved forward", () => {
    vi.useFakeTimers();
    try {
      const { shared } = arrive(secondaryChoice("pok7technology"));
      act(() => {
        vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
      });
      expect(shared.onSubmitChoice).not.toHaveBeenCalled();
      expect(screen.queryByTestId("secondary-autoplay-toast")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("auto mode", () => {
  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem(SECONDARY_PREP_MODE_KEY, "auto");
  });
  afterEach(() => vi.useRealTimers());

  it("shows the toast, then submits the prepared answer once", () => {
    vi.useFakeTimers();
    const shared = base();
    const view = render(<SecondaryPrepHost {...shared} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    view.rerender(<SecondaryPrepHost {...shared} history={hist(6)} choice={secondaryChoice("pok7technology")} />);
    expect(screen.getByTestId("secondary-autoplay-toast").textContent).toMatch(/Follow Technology/);
    act(() => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(shared.onSubmitChoice).toHaveBeenCalledTimes(1);
    expect(shared.onSubmitChoice).toHaveBeenCalledWith("yes");
  });

  it("Cancel keeps the decision open for a click", () => {
    vi.useFakeTimers();
    const shared = base();
    const view = render(<SecondaryPrepHost {...shared} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    view.rerender(<SecondaryPrepHost {...shared} history={hist(6)} choice={secondaryChoice("pok7technology")} />);
    fireEvent.click(screen.getByTestId("secondary-autoplay-cancel"));
    act(() => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(shared.onSubmitChoice).not.toHaveBeenCalled();
    expect(screen.queryByTestId("secondary-autoplay-toast")).toBeNull();
  });

  it("does not auto-play after an undo even with a plan", () => {
    vi.useFakeTimers();
    const shared = base({ history: hist(8) });
    const view = render(<SecondaryPrepHost {...shared} />);
    open();
    fireEvent.click(screen.getByTestId("prep-follow"));
    // The undo changes the generation, which voids the plan as well.
    view.rerender(<SecondaryPrepHost {...shared} history={hist(6, 1, 2)} choice={secondaryChoice("pok7technology")} />);
    act(() => {
      vi.advanceTimersByTime(AUTO_PLAY_DELAY_MS + 10);
    });
    expect(shared.onSubmitChoice).not.toHaveBeenCalled();
  });
});

describe("the setting in the player sheet", () => {
  beforeEach(() => localStorage.clear());
  const sheet = (userSeat?: string) =>
    render(
      <PlayerSheet
        players={players()}
        userSeat={userSeat}
        seatingOrder={["a", "b", "c"]}
        revealedObjectives={[]}
        board={boardWith([])}
        table={{ revealed_objectives: [], scored_objectives: {}, unclaimed_strategy_cards: [], strategy_card_goods: {}, laws: {} }}
      />,
    );

  it("defaults to Review, switches to Auto and persists", () => {
    sheet("b");
    const mode = screen.getByTestId("secondary-prep-mode");
    expect(mode.getAttribute("data-mode")).toBe("review");
    expect(screen.getByTestId("secondary-prep-mode-review").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByTestId("secondary-prep-mode-auto"));
    expect(localStorage.getItem(SECONDARY_PREP_MODE_KEY)).toBe("auto");
    expect(screen.getByTestId("secondary-prep-mode").getAttribute("data-mode")).toBe("auto");
    fireEvent.click(screen.getByTestId("secondary-prep-mode-review"));
    expect(localStorage.getItem(SECONDARY_PREP_MODE_KEY)).toBe("review");
  });

  it("is not shown to spectators", () => {
    sheet(undefined);
    expect(screen.queryByTestId("secondary-prep-mode")).toBeNull();
  });
});
