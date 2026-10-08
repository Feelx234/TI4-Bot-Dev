import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CreateLobby, LobbyStatus } from "./Lobby.tsx";
import { LobbyDto } from "../protocol/types.ts";

const lobby: LobbyDto = {
  game_id: "game",
  phase: "lobby",
  lobby_version: 2,
  host_player_id: "player_a",
  slots: [
    {
      slot_id: "slot_1",
      position: 1,
      occupant: "player_a",
      nickname: "Same name",
      ready: true,
      connected: true,
      can_take_over: false,
    },
    {
      slot_id: "slot_2",
      position: 2,
      occupant: "player_b",
      nickname: "Same name",
      ready: false,
      connected: false,
      can_take_over: true,
    },
    {
      slot_id: "slot_3",
      position: 3,
      occupant: null,
      nickname: null,
      ready: false,
      connected: false,
      can_take_over: false,
    },
  ],
};
const props = {
  onReady: vi.fn(),
  onStart: vi.fn(),
  onLeave: vi.fn(),
  onJoin: vi.fn(),
  onWatch: vi.fn(),
  onTakeover: vi.fn(),
  onReorder: vi.fn(),
};
afterEach(() => {
  localStorage.clear();
  vi.unstubAllGlobals();
});

describe("lobby UI", () => {
  it("creates empty positions without choosing bot or player identities", async () => {
    const onCreated = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({
        game_id: "game",
        player_session: "secret",
        player: { id: "player_a" },
        lobby,
      }),
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={onCreated} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Host 🪐" } });
    fireEvent.change(screen.getByLabelText("Players"), { target: { value: "3" } });
    fireEvent.click(screen.getByTestId("create-game-button"));
    await waitFor(() => expect(onCreated).toHaveBeenCalledOnce());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      player_count: 3,
      nickname: "Host 🪐",
      strategy_card_set: "te",
    });
    expect(localStorage.getItem("ti4.nickname")).toBe("Host 🪐");
  });

  it("offers the strategy card sets with Thunder's Edge preselected and sends the choice", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 500, text: async () => "x" });
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={vi.fn()} />);
    const select = screen.getByLabelText("Strategy cards");
    expect(select).toHaveValue("te");
    expect(screen.getByRole("option", { name: "Thunder's Edge (default)" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Prophecy of Kings" })).toBeInTheDocument();
    expect(screen.getByTestId("strategy-card-set-description")).toHaveTextContent(/Thunder's Edge/);
    fireEvent.change(select, { target: { value: "pok" } });
    expect(screen.getByTestId("strategy-card-set-description")).toHaveTextContent(/Prophecy of Kings/);
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Host" } });
    fireEvent.click(screen.getByTestId("create-game-button"));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body).strategy_card_set).toBe("pok");
  });

  it("names the table's strategy card set in the lobby, and nothing for an older server", () => {
    const { rerender } = render(
      <LobbyStatus lobby={{ ...lobby, strategy_card_set: "te" }} playerId={null} {...props} />,
    );
    expect(screen.getByTestId("lobby-card-set-row")).toHaveTextContent("Strategy cards: Thunder's Edge");
    rerender(<LobbyStatus lobby={{ ...lobby, strategy_card_set: "pok" }} playerId={null} {...props} />);
    expect(screen.getByTestId("lobby-card-set-row")).toHaveTextContent("Prophecy of Kings");
    rerender(<LobbyStatus lobby={lobby} playerId={null} {...props} />);
    expect(screen.queryByTestId("lobby-card-set-row")).toBeNull();
  });

  it("hides the seed input outside dev builds and never sends a seed", async () => {
    vi.stubEnv("DEV", false);
    const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 500, text: async () => "x" });
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={vi.fn()} />);
    expect(screen.queryByLabelText(/Seed/)).toBeNull();
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Host" } });
    fireEvent.click(screen.getByTestId("create-game-button"));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).not.toHaveProperty("seed");
    vi.unstubAllEnvs();
  });

  it("shows the seed input behind the dev flag and sends the typed seed", async () => {
    vi.stubEnv("DEV", true);
    const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 500, text: async () => "x" });
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Host" } });
    fireEvent.change(screen.getByLabelText(/Seed/), { target: { value: "42" } });
    fireEvent.click(screen.getByTestId("create-game-button"));
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(JSON.parse(fetchMock.mock.calls[0][1].body).seed).toBe(42);
    vi.unstubAllEnvs();
  });

  it("prefills the saved preference and refuses invalid creation before any request", async () => {
    localStorage.setItem("ti4.nickname", "Returning visitor");
    const fetchMock = vi.fn();
    const onError = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={onError} />);
    expect(screen.getByLabelText("Nickname")).toHaveValue("Returning visitor");
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: " bad " } });
    fireEvent.click(screen.getByText("Create lobby"));
    expect(onError).toHaveBeenCalledWith(expect.stringContaining("Nickname"));
    expect(fetchMock).not.toHaveBeenCalled();
    expect(localStorage.getItem("ti4.nickname")).toBe("Returning visitor");
  });

  it("shows watch, first-open join, and eligible explicit rejoin without admitting on render", () => {
    render(<LobbyStatus lobby={lobby} playerId={null} {...props} />);
    fireEvent.click(screen.getByText("Watch"));
    fireEvent.click(screen.getByText("Join game"));
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Replacement" } });
    fireEvent.click(screen.getByText(/Rejoin as Same name/));
    expect(props.onWatch).toHaveBeenCalledOnce();
    expect(props.onJoin).toHaveBeenCalledWith("");
    expect(props.onTakeover).toHaveBeenCalledWith("player_b", "Replacement");
    expect(screen.getByText(/Position 3: Open/)).toBeInTheDocument();
    expect(screen.getByLabelText("Position 3").querySelector("svg")).toBeInTheDocument();
    expect(screen.getByText(/Position 1: Same name \(Host\)/)).toBeInTheDocument();
    expect(screen.getByTestId("lobby-container").textContent).not.toContain("player_a");
    expect(screen.getByTestId("lobby-container").textContent).not.toContain("player_b");
    expect(screen.getByTestId("lobby-container").textContent).not.toContain("Game: game");
  });

  it("permits only the stable host identity to reorder every slot, including open slots", () => {
    const { rerender } = render(<LobbyStatus lobby={lobby} playerId="player_a" {...props} />);
    fireEvent.click(screen.getByLabelText("Move position 2 down"));
    expect(props.onReorder).toHaveBeenCalledWith(["slot_1", "slot_3", "slot_2"]);
    expect(screen.getByTestId("start-game-button")).toBeDisabled();
    expect(screen.queryByText("Leave lobby")).toBeNull();
    rerender(<LobbyStatus lobby={lobby} playerId="player_b" {...props} />);
    expect(screen.queryByLabelText("Move position 2 down")).toBeNull();
    fireEvent.click(screen.getByTestId("ready-button"));
    fireEvent.click(screen.getByText("Leave lobby"));
    expect(props.onReady).toHaveBeenCalledWith(true);
    expect(props.onLeave).toHaveBeenCalledOnce();
    const moved = {
      ...lobby,
      slots: [
        { ...lobby.slots[1], position: 1 },
        { ...lobby.slots[0], position: 2 },
        lobby.slots[2],
      ],
    };
    rerender(<LobbyStatus lobby={moved} playerId="player_b" {...props} />);
    expect(screen.getByText(/Position 1: Same name/)).toBeInTheDocument();
    expect(screen.getByText(/Position 2: Same name \(Host\)/)).toBeInTheDocument();
    expect(screen.getByText(/Position 1: Same name/).closest(".lobby-list__item")).toHaveStyle({
      borderLeftColor: "rgb(230, 159, 0)",
    });
  });

  it("keeps unjoined watchers and running spectators away from readiness and host controls", () => {
    const { rerender } = render(<LobbyStatus lobby={lobby} playerId={null} watching {...props} />);
    expect(screen.queryByTestId("ready-button")).not.toBeInTheDocument();
    expect(screen.queryByTestId("start-game-button")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Join game" })).toHaveClass("button");
    expect(screen.getByRole("button", { name: /Rejoin as/ })).toHaveClass("button");
    expect(screen.getByText("○ Not ready")).toHaveClass("lobby-readiness--waiting");
    expect(screen.getByText("◇ Disconnected")).toHaveClass("lobby-presence--disconnected");
    rerender(
      <LobbyStatus lobby={{ ...lobby, phase: "running" }} playerId={null} watching {...props} />,
    );
    expect(screen.queryByRole("button", { name: "Join game" })).not.toBeInTheDocument();
    expect(screen.queryByTestId("ready-button")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Move position/)).not.toBeInTheDocument();
  });

  it("explains disabled start and distinguishes readiness and presence without color", () => {
    const { rerender } = render(<LobbyStatus lobby={lobby} playerId="player_a" {...props} />);
    expect(screen.getByTestId("ready-button")).toHaveTextContent("Mark not ready");
    expect(screen.getByTestId("ready-button")).toHaveClass("lobby-unready-button");
    expect(screen.getByTestId("start-game-button")).toHaveAttribute(
      "aria-describedby",
      "start-reason",
    );
    expect(screen.getByText("Waiting for 1 open position to be filled.")).toBeInTheDocument();
    expect(screen.getByText("✔ Ready")).toBeInTheDocument();
    expect(screen.getByText("● Connected")).toBeInTheDocument();
    const filled = {
      ...lobby,
      slots: [
        lobby.slots[0],
        lobby.slots[1],
        { ...lobby.slots[2], occupant: "player_c", nickname: "C" },
      ],
    };
    rerender(<LobbyStatus lobby={filled} playerId="player_a" {...props} />);
    expect(screen.getByText("Waiting for 2 players to be ready.")).toBeInTheDocument();
    rerender(
      <LobbyStatus
        lobby={{ ...filled, slots: filled.slots.map((slot) => ({ ...slot, ready: true })) }}
        playerId="player_a"
        {...props}
      />,
    );
    expect(screen.getByTestId("start-game-button")).toBeEnabled();
  });

  it("locks conflicting actions during a request and leaves reorder keyboard labels readable", () => {
    render(<LobbyStatus lobby={lobby} playerId="player_a" pendingAction="reorder" {...props} />);
    expect(screen.getByTestId("ready-button")).toBeDisabled();
    expect(screen.getByTestId("start-game-button")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Copy game URL" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move position 2 down" })).toBeDisabled();
  });

  it("reports clipboard failure with a usable fallback", async () => {
    vi.stubGlobal("navigator", {
      clipboard: { writeText: vi.fn().mockRejectedValue(new Error("denied")) },
    });
    render(<LobbyStatus lobby={lobby} playerId={null} {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Copy game URL" }));
    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent(
        "Copy the address from your browser instead.",
      ),
    );
  });

  it("disables create controls and ignores repeated submits while the request is pending", async () => {
    let resolve!: (value: unknown) => void;
    const fetchMock = vi.fn().mockImplementation(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    vi.stubGlobal("fetch", fetchMock);
    render(<CreateLobby onCreated={vi.fn()} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Nickname"), { target: { value: "Host" } });
    fireEvent.click(screen.getByTestId("create-game-button"));
    expect(screen.getByTestId("create-game-button")).toBeDisabled();
    expect(screen.getByLabelText("Nickname")).toBeDisabled();
    fireEvent.submit(screen.getByTestId("create-game-button").closest("form")!);
    expect(fetchMock).toHaveBeenCalledOnce();
    resolve({ ok: false, status: 503, text: async () => "Unavailable" });
    await waitFor(() => expect(screen.getByTestId("create-game-button")).toBeEnabled());
  });

  it("shows Add Bot button only for host when bot service is enabled", async () => {
    const onAddBot = vi.fn().mockResolvedValue(true);
    const lobbyWithBots: LobbyDto = {
      ...lobby,
      bot_service_enabled: true,
    };

    // Non-host viewer: no Add Bot button
    const { rerender } = render(
      <LobbyStatus lobby={lobbyWithBots} playerId={null} onAddBot={onAddBot} {...props} />,
    );
    expect(screen.queryByTestId("add-bot-button-3")).not.toBeInTheDocument();

    // Host viewer: Add Bot button is visible on open slot (position 3)
    rerender(
      <LobbyStatus lobby={lobbyWithBots} playerId="player_a" onAddBot={onAddBot} {...props} />,
    );
    const addBtn = screen.getByTestId("add-bot-button-3");
    expect(addBtn).toBeInTheDocument();

    // Clicking opens modal
    fireEvent.click(addBtn);
    expect(screen.getByTestId("add-bot-modal")).toBeInTheDocument();

    // Fill password and submit
    fireEvent.change(screen.getByPlaceholderText("Enter server bot password"), {
      target: { value: "secret123" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add Bot" }));

    await waitFor(() => {
      expect(onAddBot).toHaveBeenCalledWith("secret123", "Bot 3");
    });
    expect(localStorage.getItem("ti4_bot_password")).toBe("secret123");
  });

  it("allows host to remove an occupied non-host seat", () => {
    const onRemoveBot = vi.fn().mockResolvedValue(true);
    render(<LobbyStatus lobby={lobby} playerId="player_a" onRemoveBot={onRemoveBot} {...props} />);

    const removeBtn = screen.getByTestId("remove-bot-button-2");
    expect(removeBtn).toBeInTheDocument();
    fireEvent.click(removeBtn);
    expect(onRemoveBot).toHaveBeenCalledWith("player_b");
  });
  describe("random bots", () => {
    const withBots: LobbyDto = { ...lobby, bot_kinds: ["random"] };

    it("gives the host add and fill buttons that call the handler, and nobody else", () => {
      const onAddRandomBots = vi.fn().mockResolvedValue(true);
      const { rerender } = render(
        <LobbyStatus lobby={withBots} playerId={null} onAddRandomBots={onAddRandomBots} {...props} />,
      );
      expect(screen.queryByTestId("add-random-bot-button")).not.toBeInTheDocument();
      rerender(
        <LobbyStatus lobby={withBots} playerId="player_b" onAddRandomBots={onAddRandomBots} {...props} />,
      );
      expect(screen.queryByTestId("add-random-bot-button")).not.toBeInTheDocument();
      rerender(
        <LobbyStatus lobby={withBots} playerId="player_a" onAddRandomBots={onAddRandomBots} {...props} />,
      );
      fireEvent.click(screen.getByTestId("add-random-bot-button"));
      expect(onAddRandomBots).toHaveBeenLastCalledWith({ count: 1 });
      fireEvent.click(screen.getByTestId("fill-random-bots-button"));
      expect(onAddRandomBots).toHaveBeenLastCalledWith({ fill: true });
      // No password prompt: random bots need none.
      expect(screen.queryByTestId("add-bot-modal")).not.toBeInTheDocument();
    });

    it("offers nothing on a server that lists no random bots, or when no seat is open, or after start", () => {
      const onAddRandomBots = vi.fn();
      const { rerender } = render(
        <LobbyStatus lobby={lobby} playerId="player_a" onAddRandomBots={onAddRandomBots} {...props} />,
      );
      expect(screen.queryByTestId("add-random-bot-button")).not.toBeInTheDocument();
      const full: LobbyDto = {
        ...withBots,
        slots: withBots.slots.map((slot, i) =>
          slot.occupant ? slot : { ...slot, occupant: `player_${i}`, nickname: "X", ready: true },
        ),
      };
      rerender(
        <LobbyStatus lobby={full} playerId="player_a" onAddRandomBots={onAddRandomBots} {...props} />,
      );
      expect(screen.queryByTestId("fill-random-bots-button")).not.toBeInTheDocument();
      rerender(
        <LobbyStatus
          lobby={{ ...withBots, phase: "running" }}
          playerId="player_a"
          onAddRandomBots={onAddRandomBots}
          {...props}
        />,
      );
      expect(screen.queryByTestId("add-random-bot-button")).not.toBeInTheDocument();
    });

    it("labels a bot seat, says it plays itself, and lets the host remove it", () => {
      const onRemoveBot = vi.fn().mockResolvedValue(true);
      const botLobby: LobbyDto = {
        ...withBots,
        slots: withBots.slots.map((slot) =>
          slot.position === 2
            ? { ...slot, nickname: "Random bot 1", ready: true, connected: true, can_take_over: false, bot: "random" }
            : slot,
        ),
      };
      render(
        <LobbyStatus lobby={botLobby} playerId="player_a" onRemoveBot={onRemoveBot} {...props} />,
      );
      expect(screen.getByTestId("bot-badge-2")).toHaveTextContent("Random bot");
      expect(screen.queryByTestId("bot-badge-1")).not.toBeInTheDocument();
      expect(screen.getByText("● Plays itself")).toBeInTheDocument();
      const remove = screen.getByRole("button", { name: "Remove bot at position 2" });
      fireEvent.click(remove);
      expect(onRemoveBot).toHaveBeenCalledWith("player_b");
    });
  });
});
