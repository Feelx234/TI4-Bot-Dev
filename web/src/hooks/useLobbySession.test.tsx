import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useLobbySession } from "./useLobbySession.ts";

const lobby = {
  game_id: "game",
  phase: "lobby",
  lobby_version: 1,
  host_player_id: "player_a",
  slots: [
    {
      slot_id: "slot_1",
      position: 1,
      occupant: "player_a",
      nickname: "Host",
      ready: false,
      connected: true,
      can_take_over: false,
    },
    {
      slot_id: "slot_2",
      position: 2,
      occupant: null,
      nickname: null,
      ready: false,
      connected: false,
      can_take_over: false,
    },
  ],
};
const json = (value: unknown) => ({ ok: true, json: async () => value });

afterEach(() => {
  vi.unstubAllGlobals();
});

it("serializes lobby mutations, keeps failed actions actionable, and permits a retry", async () => {
  let finish!: (response: unknown) => void;
  const fetchMock = vi.fn().mockImplementation((url: string) => {
    if (url.endsWith("/ready"))
      return new Promise((resolve) => {
        finish = resolve;
      });
    return Promise.resolve(json({ player: { id: "player_a" }, lobby }));
  });
  vi.stubGlobal("fetch", fetchMock);
  const { result } = renderHook(() => useLobbySession("game", "session_secret"));
  await waitFor(() => expect(result.current.playerId).toBe("player_a"));
  let first!: Promise<void>;
  let blocked!: Promise<void>;
  act(() => {
    first = result.current.setReady(true);
    blocked = result.current.start();
  });
  expect(result.current.pendingAction).toBe("ready");
  expect(fetchMock.mock.calls.filter(([url]) => String(url).endsWith("/start"))).toHaveLength(0);
  await act(async () => {
    finish({ ok: false, status: 409, text: async () => "Player already ready" });
    await Promise.all([first, blocked]);
  });
  expect(result.current.pendingAction).toBeNull();
  expect(result.current.error).toContain("409");
  act(() => {
    first = result.current.setReady(true);
  });
  await act(async () => {
    finish(json({ ...lobby, slots: [{ ...lobby.slots[0], ready: true }, lobby.slots[1]] }));
    await first;
  });
  expect(result.current.error).toBeNull();
  expect(result.current.lobby?.slots[0].ready).toBe(true);
});

it("posts the host's map choice with the credential and adopts the lobby it returns", async () => {
  const chosen = {
    ...lobby,
    lobby_version: 2,
    map: { kind: "random", systems: 36, hyperlanes: false, recommended: false },
    map_revision: 1,
  };
  const fetchMock = vi
    .fn()
    .mockImplementation((url: string) =>
      Promise.resolve(
        String(url).endsWith("/map") ? json(chosen) : json({ player: { id: "player_a" }, lobby }),
      ),
    );
  vi.stubGlobal("fetch", fetchMock);
  const { result } = renderHook(() => useLobbySession("game", "session_secret"));
  await waitFor(() => expect(result.current.playerId).toBe("player_a"));
  await act(async () => {
    await result.current.chooseMap({ kind: "random" });
  });
  const call = fetchMock.mock.calls.find(([url]) => String(url).endsWith("/lobby/map"))!;
  expect(call[1].method).toBe("POST");
  expect(call[1].headers["x-ti4-player-session"]).toBe("session_secret");
  expect(JSON.parse(call[1].body)).toEqual({ map: { kind: "random" } });
  expect(result.current.lobby?.map?.kind).toBe("random");
  expect(result.current.lobby?.map_revision).toBe(1);
});

it("does not stack lobby polls while one is still in flight", async () => {
  vi.useFakeTimers();
  try {
    let finish!: (response: unknown) => void;
    const fetchMock = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const { result } = renderHook(() => useLobbySession("game", "session_secret"));
    expect(fetchMock).toHaveBeenCalledTimes(1);
    // A slow server: several poll ticks pass while the first join is unanswered.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await act(async () => {
      finish(json({ player: { id: "player_a" }, lobby }));
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(result.current.playerId).toBe("player_a");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });
    expect(fetchMock).toHaveBeenCalledTimes(2);
  } finally {
    vi.useRealTimers();
  }
});
