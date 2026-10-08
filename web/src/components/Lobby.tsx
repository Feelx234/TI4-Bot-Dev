import React, { useEffect, useRef, useState } from "react";
import {
  CreateGameResponse,
  LobbyDto,
  LobbySlot,
  MapChoice,
  StrategyCardSetId,
} from "../protocol/types.ts";
import { MapPicker } from "./MapPicker.tsx";
import { mapChangeNotice, mapName } from "../presentation/mapPicker.ts";
import {
  DEFAULT_STRATEGY_CARD_SET,
  STRATEGY_CARD_SETS,
  isStrategyCardSetId,
  strategyCardSetLabel,
  strategyCardSetName,
} from "../presentation/strategyCardSet.ts";
import { decodeCreateGameResponse } from "../protocol/decode.ts";
import { preferredNickname, rememberNickname, validNickname } from "../protocol/nickname.ts";
import { SeatBadge } from "../presentation/PlayerIdentity.tsx";
import { seatStyle } from "../presentation/playerDisplay.ts";

export const CreateLobby: React.FC<{
  onCreated: (created: CreateGameResponse) => void;
  onError: (message: string) => void;
}> = ({ onCreated, onError }) => {
  const [count, setCount] = useState(3);
  const [cardSet, setCardSet] = useState<StrategyCardSetId>(DEFAULT_STRATEGY_CARD_SET);
  // The seed lets whoever knows it predict dice and deck order, so players never see the field;
  // it exists only in dev builds (the API field itself is unchanged).
  const showSeed = import.meta.env.DEV;
  const [seed, setSeed] = useState("");
  const [nickname, setNickname] = useState(preferredNickname);
  const [creating, setCreating] = useState(false);
  const inFlight = useRef(false);
  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if (inFlight.current) return;
    const parsedSeed = !showSeed || seed === "" ? undefined : Number(seed);
    if (parsedSeed !== undefined && (!Number.isSafeInteger(parsedSeed) || parsedSeed < 0))
      return onError("Seed must be a non-negative whole number.");
    if (!validNickname(nickname))
      return onError(
        "Nickname must be 1–64 UTF-8 bytes, trimmed, without control or format characters.",
      );
    inFlight.current = true;
    setCreating(true);
    try {
      const response = await fetch("/api/games", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          player_count: count,
          seed: parsedSeed,
          nickname,
          strategy_card_set: cardSet,
        }),
      });
      if (!response.ok)
        throw new Error(`Create game failed (${response.status}): ${await response.text()}`);
      const created = decodeCreateGameResponse(await response.json());
      rememberNickname(nickname);
      onCreated(created);
    } catch (cause) {
      onError(`${String(cause)} Check your details and try again.`);
    } finally {
      inFlight.current = false;
      setCreating(false);
    }
  };
  return (
    <main data-testid="lobby-container" className="lobby-page">
      <section className="panel lobby-panel">
        <h1>Twilight Imperium 4</h1>
        <p className="text-muted">Create a table and share its game URL.</p>
        <form className="lobby-form" onSubmit={create}>
          <label className="field-label">
            Players
            <select
              className="input"
              disabled={creating}
              value={count}
              onChange={(event) => setCount(Number(event.target.value))}
            >
              {[2, 3, 4, 5, 6, 7, 8].map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </select>
          </label>
          <label className="field-label">
            Strategy cards
            <select
              className="input"
              data-testid="strategy-card-set-select"
              disabled={creating}
              value={cardSet}
              onChange={(event) => {
                if (isStrategyCardSetId(event.target.value)) setCardSet(event.target.value);
              }}
            >
              {STRATEGY_CARD_SETS.map((set) => (
                <option key={set.id} value={set.id}>
                  {strategyCardSetLabel(set)}
                </option>
              ))}
            </select>
          </label>
          <p className="text-faint" data-testid="strategy-card-set-description">
            {STRATEGY_CARD_SETS.find((set) => set.id === cardSet)?.description}
          </p>
          <label className="field-label">
            Nickname
            <input
              className="input"
              disabled={creating}
              value={nickname}
              onChange={(event) => setNickname(event.target.value)}
            />
          </label>
          {showSeed && (
            <label className="field-label">
              Seed (dev, optional)
              <input
                className="input"
                disabled={creating}
                type="number"
                min="0"
                step="1"
                value={seed}
                onChange={(event) => setSeed(event.target.value)}
              />
            </label>
          )}
          <p className="text-faint">
            You join as host. Other players and bots join from the shared URL.
          </p>
          <button
            data-testid="create-game-button"
            className="button button--success"
            disabled={creating}
            type="submit"
          >
            {creating ? "Creating lobby…" : "Create lobby"}
          </button>
        </form>
      </section>
    </main>
  );
};

interface LobbyStatusProps {
  lobby: LobbyDto;
  playerId: string | null;
  onReady: (ready: boolean) => void;
  onStart: () => void;
  onLeave: () => void;
  onJoin: (nickname: string) => void;
  onTakeover: (playerId: string, nickname: string) => void;
  onReorder: (slotIds: string[]) => void;
  onWatch: () => void;
  onAddBot?: (password: string, nickname?: string) => Promise<boolean | void>;
  onRemoveBot?: (playerId: string) => Promise<boolean | void>;
  /** Host only: seat in-process random bots (one, or every open seat with `fill`). */
  onAddRandomBots?: (options: { fill?: boolean; count?: number }) => Promise<boolean | void>;
  /** Host only: save a map choice (resolves once the lobby has been updated). */
  onChooseMap?: (choice: MapChoice, startPreset?: string) => Promise<boolean | void>;
  watching?: boolean;
  pendingAction?: string | null;
}

function rememberedSeen(key: string): boolean {
  try {
    return sessionStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function markSeen(key: string): void {
  try {
    sessionStorage.setItem(key, "1");
  } catch {
    // Without storage the picker may open again after a reload; that is harmless.
  }
}

/** Nicknames are public display data; the position distinguishes duplicate names. */
function playerLabel(slot: LobbySlot): string {
  return slot.nickname ?? `Player at position ${slot.position}`;
}

export const LobbyStatus: React.FC<LobbyStatusProps> = ({
  lobby,
  playerId,
  onReady,
  onStart,
  onLeave,
  onJoin,
  onTakeover,
  onReorder,
  onWatch,
  onAddBot,
  onRemoveBot,
  onAddRandomBots,
  onChooseMap,
  watching,
  pendingAction,
}) => {
  const [nickname, setNickname] = useState(preferredNickname);
  const [copyState, setCopyState] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  const copyingRef = useRef(false);
  const [addBotPosition, setAddBotPosition] = useState<number | null>(null);
  const [botPassword, setBotPassword] = useState(
    () => localStorage.getItem("ti4_bot_password") || "",
  );
  const [botNickname, setBotNickname] = useState("");
  const [rememberBotPassword, setRememberBotPassword] = useState(true);
  const [botError, setBotError] = useState<string | null>(null);
  const [addingBot, setAddingBot] = useState(false);
  const viewer =
    playerId === null ? undefined : lobby.slots.find((slot) => slot.occupant === playerId);
  const isHost = playerId !== null && playerId === lobby.host_player_id;
  const canEditMap = isHost && lobby.phase === "lobby" && !!onChooseMap;
  // The picker opens by itself for a host who has not chosen yet (a fresh lobby), once per tab.
  const seenKey = `ti4.map-picker-seen:${lobby.game_id}`;
  const [pickerOpen, setPickerOpen] = useState(
    () => canEditMap && !!lobby.map && !lobby.map_revision && !rememberedSeen(seenKey),
  );
  const closePicker = () => {
    setPickerOpen(false);
    markSeen(seenKey);
  };
  const [mapNotice, setMapNotice] = useState<string | null>(null);
  const previousMap = useRef<{ revision: number | undefined; map: LobbyDto["map"] } | null>(null);
  useEffect(() => {
    const now = { revision: lobby.map_revision, map: lobby.map };
    if (!isHost) setMapNotice((old) => mapChangeNotice(previousMap.current, now) ?? old);
    previousMap.current = now;
  }, [lobby.map_revision, lobby.map, isHost]);
  // An older server lists no bot kinds: it has no random bots, so offer none.
  const canAddRandomBots =
    isHost &&
    lobby.phase === "lobby" &&
    !!onAddRandomBots &&
    !!lobby.bot_kinds?.includes("random") &&
    lobby.slots.some((slot) => !slot.occupant);
  const canStart =
    lobby.phase === "lobby" && lobby.slots.every((slot) => slot.occupant && slot.ready);
  const handleAddBot = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!botPassword.trim()) {
      setBotError("Password is required.");
      return;
    }
    if (rememberBotPassword) {
      localStorage.setItem("ti4_bot_password", botPassword);
    } else {
      localStorage.removeItem("ti4_bot_password");
    }
    setAddingBot(true);
    setBotError(null);
    try {
      if (onAddBot) {
        await onAddBot(botPassword, botNickname.trim() || undefined);
      }
      setAddBotPosition(null);
    } catch (err) {
      setBotError(String(err));
    } finally {
      setAddingBot(false);
    }
  };
  const copyUrl = async () => {
    if (copyingRef.current || pendingAction) return;
    copyingRef.current = true;
    setCopying(true);
    setCopyState(null);
    try {
      if (!navigator.clipboard?.writeText)
        throw new Error("Clipboard is unavailable. Copy the address from your browser instead.");
      await navigator.clipboard.writeText(`${window.location.origin}${window.location.pathname}`);
      setCopyState("Game URL copied.");
    } catch {
      setCopyState("Could not copy the game URL. Copy the address from your browser instead.");
    } finally {
      copyingRef.current = false;
      setCopying(false);
    }
  };
  const openCount = lobby.slots.filter((slot) => !slot.occupant).length;
  const unreadyCount = lobby.slots.filter((slot) => slot.occupant && !slot.ready).length;
  const startReason = openCount
    ? `Waiting for ${openCount} open position${openCount === 1 ? "" : "s"} to be filled.`
    : unreadyCount
      ? `Waiting for ${unreadyCount} player${unreadyCount === 1 ? "" : "s"} to be ready.`
      : null;
  const move = (index: number, delta: number) => {
    const slots = lobby.slots.map((slot) => slot.slot_id);
    [slots[index], slots[index + delta]] = [slots[index + delta], slots[index]];
    onReorder(slots);
  };
  return (
    <main data-testid="lobby-container" className="lobby-page">
      <section className="panel lobby-panel">
        <h1>{lobby.phase === "running" ? "Game in progress" : "Game lobby"}</h1>
        <button
          type="button"
          className="button button--outline"
          disabled={!!pendingAction || copying}
          onClick={() => void copyUrl()}
        >
          {copying ? "Copying…" : "Copy game URL"}
        </button>
        {copyState && (
          <p role="status" className="text-muted">
            {copyState}
          </p>
        )}
        {lobby.map && (
          <div className="lobby-map-row" data-testid="lobby-map-row">
            <span className="lobby-map-row__name">
              Map: <strong>{mapName(lobby.map)}</strong>
              {lobby.map.recommended ? " (recommended)" : ""}
            </span>
            <button
              type="button"
              className="button button--outline"
              data-testid="lobby-map-button"
              disabled={!!pendingAction && pendingAction !== "map"}
              onClick={() => setPickerOpen(true)}
            >
              {canEditMap ? "Choose map" : "View map"}
            </button>
          </div>
        )}
        {lobby.strategy_card_set !== undefined && (
          <p className="text-muted" data-testid="lobby-card-set-row">
            Strategy cards: <strong>{strategyCardSetName(lobby.strategy_card_set)}</strong>
          </p>
        )}
        {mapNotice && (
          <p role="status" className="text-muted" data-testid="map-change-notice">
            {mapNotice}{" "}
            <button
              type="button"
              className="button button--outline"
              onClick={() => setMapNotice(null)}
            >
              Dismiss
            </button>
          </p>
        )}
        {pickerOpen && lobby.map && (
          <MapPicker
            lobby={lobby}
            editable={canEditMap}
            viewerPosition={viewer?.position ?? null}
            saving={pendingAction === "map"}
            onChoose={(choice, startPreset) => void onChooseMap?.(choice, startPreset)}
            onClose={closePicker}
          />
        )}
        {canAddRandomBots && (
          <div className="lobby-bot-actions" data-testid="random-bot-actions">
            <button
              type="button"
              className="button button--outline"
              data-testid="add-random-bot-button"
              disabled={!!pendingAction}
              onClick={() => void onAddRandomBots?.({ count: 1 })}
            >
              {pendingAction === "add_random_bots" ? "Adding…" : "Add random bot"}
            </button>
            <button
              type="button"
              className="button button--outline"
              data-testid="fill-random-bots-button"
              disabled={!!pendingAction}
              onClick={() => void onAddRandomBots?.({ fill: true })}
            >
              Fill empty seats with random bots
            </button>
            <p className="text-faint lobby-bot-actions__hint">
              Random bots play legal moves at random. They need no password and cannot be taken
              over.
            </p>
          </div>
        )}
        <div className="lobby-list">
          {lobby.slots.map((slot, index) => (
            <div
              className="lobby-list__item"
              key={slot.slot_id}
              style={{ borderLeft: `3px solid ${seatStyle(slot.position).color}`, paddingLeft: 8 }}
            >
              <strong>
                <SeatBadge position={slot.position} /> Position {slot.position}:{" "}
                {slot.occupant ? playerLabel(slot) : "Open"}
                {slot.occupant === lobby.host_player_id ? " (Host)" : ""}
                {slot.bot === "random" && (
                  <span className="lobby-bot-badge" data-testid={`bot-badge-${slot.position}`}>
                    Random bot
                  </span>
                )}
              </strong>
              <span className="lobby-list__status">
                {slot.occupant ? (
                  <>
                    <span
                      className={`lobby-readiness ${slot.ready ? "lobby-readiness--ready" : "lobby-readiness--waiting"}`}
                    >
                      {slot.ready ? "✔ Ready" : "○ Not ready"}
                    </span>
                    {slot.bot ? (
                      <span className="lobby-presence lobby-presence--connected">● Plays itself</span>
                    ) : (
                      <span
                        className={`lobby-presence ${slot.connected ? "lobby-presence--connected" : "lobby-presence--disconnected"}`}
                      >
                        {slot.connected ? "● Connected" : "◇ Disconnected"}
                      </span>
                    )}
                  </>
                ) : isHost && lobby.phase === "lobby" && lobby.bot_service_enabled ? (
                  <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                    Available
                    <button
                      type="button"
                      className="button button--outline add-bot-button"
                      data-testid={`add-bot-button-${slot.position}`}
                      style={{ padding: "2px 8px", fontSize: "11px" }}
                      disabled={!!pendingAction}
                      onClick={() => {
                        setBotNickname(`Bot ${slot.position}`);
                        setAddBotPosition(slot.position);
                        setBotError(null);
                      }}
                    >
                      Add Bot 🤖
                    </button>
                  </span>
                ) : (
                  "Available"
                )}
              </span>
              {isHost && lobby.phase === "lobby" && (
                <span className="lobby-reorder">
                  {slot.occupant && slot.occupant !== lobby.host_player_id && onRemoveBot && (
                    <button
                      type="button"
                      className="button button--outline button--icon remove-bot-button"
                      data-testid={`remove-bot-button-${slot.position}`}
                      aria-label={
                        slot.bot
                          ? `Remove bot at position ${slot.position}`
                          : `Remove player at position ${slot.position}`
                      }
                      title={
                        slot.bot
                          ? `Remove bot at position ${slot.position}`
                          : `Remove player at position ${slot.position}`
                      }
                      disabled={!!pendingAction}
                      onClick={() => void onRemoveBot(slot.occupant!)}
                    >
                      ✕
                    </button>
                  )}
                  <button
                    type="button"
                    className="button button--outline button--icon"
                    aria-label={`Move position ${slot.position} up`}
                    disabled={!!pendingAction || index === 0}
                    onClick={() => move(index, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className="button button--outline button--icon"
                    aria-label={`Move position ${slot.position} down`}
                    disabled={!!pendingAction || index === lobby.slots.length - 1}
                    onClick={() => move(index, 1)}
                  >
                    ↓
                  </button>
                </span>
              )}
            </div>
          ))}
        </div>
        {addBotPosition !== null && (
          <div
            className="decision-modal"
            data-testid="add-bot-modal"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "rgba(0, 0, 0, 0.75)",
            }}
          >
            <div className="panel decision-modal__panel" style={{ width: "min(440px, 95vw)" }}>
              <div className="decision-modal__header">
                <h2>Add MLP Bot (Position {addBotPosition})</h2>
                <button
                  type="button"
                  className="button button--outline button--icon"
                  onClick={() => setAddBotPosition(null)}
                  disabled={addingBot}
                >
                  ✕
                </button>
              </div>
              <form onSubmit={handleAddBot} className="lobby-form">
                <label className="field-label">
                  Bot Nickname
                  <input
                    className="input"
                    value={botNickname}
                    disabled={addingBot}
                    onChange={(e) => setBotNickname(e.target.value)}
                  />
                </label>
                <label className="field-label">
                  Server Bot Password
                  <input
                    className="input"
                    type="password"
                    placeholder="Enter server bot password"
                    value={botPassword}
                    disabled={addingBot}
                    onChange={(e) => setBotPassword(e.target.value)}
                  />
                </label>
                <label
                  className="field-label"
                  style={{ flexDirection: "row", alignItems: "center", gap: 8, cursor: "pointer" }}
                >
                  <input
                    type="checkbox"
                    checked={rememberBotPassword}
                    disabled={addingBot}
                    onChange={(e) => setRememberBotPassword(e.target.checked)}
                  />
                  Remember password on this device
                </label>
                {botError && (
                  <p
                    role="alert"
                    style={{ color: "var(--color-danger, #ef4444)", margin: "4px 0" }}
                  >
                    {botError}
                  </p>
                )}
                <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                  <button type="submit" className="button button--success" disabled={addingBot}>
                    {addingBot ? "Adding bot…" : "Add Bot"}
                  </button>
                  <button
                    type="button"
                    className="button button--outline"
                    disabled={addingBot}
                    onClick={() => setAddBotPosition(null)}
                  >
                    Cancel
                  </button>
                </div>
              </form>
            </div>
          </div>
        )}
        {viewer && lobby.phase === "lobby" && (
          <button
            type="button"
            data-testid="ready-button"
            className={`button ${viewer.ready ? "button--outline lobby-unready-button" : "button--success"}`}
            disabled={!!pendingAction}
            onClick={() => onReady(!viewer.ready)}
          >
            {pendingAction === "ready"
              ? "Updating readiness…"
              : viewer.ready
                ? "Mark not ready"
                : "Mark ready"}
          </button>
        )}
        {isHost && lobby.phase === "lobby" && (
          <div>
            <button
              type="button"
              data-testid="start-game-button"
              className="button button--primary"
              disabled={!canStart || !!pendingAction}
              aria-describedby={startReason ? "start-reason" : undefined}
              onClick={onStart}
            >
              {pendingAction === "start" ? "Starting game…" : "Start game"}
            </button>
            {startReason && (
              <p id="start-reason" className="text-muted">
                {startReason}
              </p>
            )}
          </div>
        )}
        {!playerId && (
          <section aria-label="Join or watch" className="lobby-actions">
            <h2>{watching ? "Watching as spectator" : "Join or watch"}</h2>
            <label className="field-label">
              Nickname
              <input
                className="input"
                disabled={!!pendingAction}
                value={nickname}
                onChange={(event) => setNickname(event.target.value)}
              />
            </label>
            {lobby.phase === "lobby" && openCount > 0 && (
              <button
                type="button"
                className="button button--success"
                disabled={!!pendingAction}
                onClick={() => onJoin(nickname)}
              >
                {pendingAction === "join" ? "Joining…" : "Join game"}
              </button>
            )}
            {!watching && (
              <button
                type="button"
                className="button button--secondary"
                disabled={!!pendingAction}
                onClick={onWatch}
              >
                Watch
              </button>
            )}
            {lobby.slots
              .filter((slot) => slot.occupant && slot.can_take_over)
              .map((slot) => (
                <button
                  type="button"
                  className="button button--outline"
                  disabled={!!pendingAction}
                  key={slot.slot_id}
                  onClick={() => onTakeover(slot.occupant!, nickname)}
                >
                  {pendingAction === "takeover"
                    ? "Rejoining…"
                    : `Rejoin as ${playerLabel(slot)} (position ${slot.position})`}
                </button>
              ))}
          </section>
        )}
        {viewer && !isHost && lobby.phase === "lobby" && (
          <button
            type="button"
            className="button button--secondary"
            disabled={!!pendingAction}
            onClick={onLeave}
          >
            {pendingAction === "leave" ? "Leaving…" : "Leave lobby"}
          </button>
        )}
      </section>
    </main>
  );
};
