import React, { useRef, useState } from 'react';
import { CreateGameResponse, LobbyDto, LobbySlot } from '../protocol/types.ts';
import { decodeCreateGameResponse } from '../protocol/decode.ts';
import { preferredNickname, rememberNickname, validNickname } from '../protocol/nickname.ts';
import { SeatBadge } from '../presentation/PlayerIdentity.tsx';
import { seatStyle } from '../presentation/playerDisplay.ts';

export const CreateLobby: React.FC<{ onCreated: (created: CreateGameResponse) => void; onError: (message: string) => void }> = ({ onCreated, onError }) => {
  const [count, setCount] = useState(3);
  const [seed, setSeed] = useState('');
  const [nickname, setNickname] = useState(preferredNickname);
  const [creating, setCreating] = useState(false);
  const inFlight = useRef(false);
  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if (inFlight.current) return;
    const parsedSeed = seed === '' ? undefined : Number(seed);
    if (parsedSeed !== undefined && (!Number.isSafeInteger(parsedSeed) || parsedSeed < 0)) return onError('Seed must be a non-negative whole number.');
    if (!validNickname(nickname)) return onError('Nickname must be 1–64 UTF-8 bytes, trimmed, without control or format characters.');
    inFlight.current = true;
    setCreating(true);
    try {
      const response = await fetch('/api/games', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ player_count: count, seed: parsedSeed, nickname }) });
      if (!response.ok) throw new Error(`Create game failed (${response.status}): ${await response.text()}`);
      const created = decodeCreateGameResponse(await response.json());
      rememberNickname(nickname);
      onCreated(created);
    } catch (cause) { onError(`${String(cause)} Check your details and try again.`); }
    finally { inFlight.current = false; setCreating(false); }
  };
  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>Twilight Imperium 4</h1><p className="text-muted">Create a table and share its game URL.</p>
    <form className="lobby-form" onSubmit={create}>
      <label className="field-label">Players<select className="input" disabled={creating} value={count} onChange={(event) => setCount(Number(event.target.value))}>{[2, 3, 4, 5, 6, 7, 8].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
      <label className="field-label">Nickname<input className="input" disabled={creating} value={nickname} onChange={(event) => setNickname(event.target.value)} /></label>
      <label className="field-label">Seed (advanced, optional)<input className="input" disabled={creating} type="number" min="0" step="1" value={seed} onChange={(event) => setSeed(event.target.value)} /></label>
      <p className="text-faint">You join as host. Other players and bots join from the shared URL.</p><button data-testid="create-game-button" className="button button--success" disabled={creating} type="submit">{creating ? 'Creating lobby…' : 'Create lobby'}</button>
    </form>
  </section></main>;
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
  watching?: boolean;
  pendingAction?: string | null;
}

/** Nicknames are public display data; the position distinguishes duplicate names. */
function playerLabel(slot: LobbySlot): string {
  return slot.nickname ?? `Player at position ${slot.position}`;
}

export const LobbyStatus: React.FC<LobbyStatusProps> = ({ lobby, playerId, onReady, onStart, onLeave, onJoin, onTakeover, onReorder, onWatch, watching, pendingAction }) => {
  const [nickname, setNickname] = useState(preferredNickname);
  const [copyState, setCopyState] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  const copyingRef = useRef(false);
  const viewer = playerId === null ? undefined : lobby.slots.find((slot) => slot.occupant === playerId);
  const isHost = playerId !== null && playerId === lobby.host_player_id;
  const canStart = lobby.phase === 'lobby' && lobby.slots.every((slot) => slot.occupant && slot.ready);
  const copyUrl = async () => {
    if (copyingRef.current || pendingAction) return;
    copyingRef.current = true;
    setCopying(true);
    setCopyState(null);
    try {
      if (!navigator.clipboard?.writeText) throw new Error('Clipboard is unavailable. Copy the address from your browser instead.');
      await navigator.clipboard.writeText(`${window.location.origin}${window.location.pathname}`);
      setCopyState('Game URL copied.');
    } catch { setCopyState('Could not copy the game URL. Copy the address from your browser instead.'); }
    finally { copyingRef.current = false; setCopying(false); }
  };
  const openCount = lobby.slots.filter((slot) => !slot.occupant).length;
  const unreadyCount = lobby.slots.filter((slot) => slot.occupant && !slot.ready).length;
  const startReason = openCount ? `Waiting for ${openCount} open position${openCount === 1 ? '' : 's'} to be filled.` : unreadyCount ? `Waiting for ${unreadyCount} player${unreadyCount === 1 ? '' : 's'} to be ready.` : null;
  const move = (index: number, delta: number) => {
    const slots = lobby.slots.map((slot) => slot.slot_id);
    [slots[index], slots[index + delta]] = [slots[index + delta], slots[index]];
    onReorder(slots);
  };
  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>{lobby.phase === 'running' ? 'Game in progress' : 'Game lobby'}</h1><button type="button" className="button button--outline" disabled={!!pendingAction || copying} onClick={() => void copyUrl()}>{copying ? 'Copying…' : 'Copy game URL'}</button>{copyState && <p role="status" className="text-muted">{copyState}</p>}
    <div className="lobby-list">{lobby.slots.map((slot, index) => <div className="lobby-list__item" key={slot.slot_id} style={{ borderLeft: `3px solid ${seatStyle(slot.position).color}`, paddingLeft: 8 }}>
      <strong><SeatBadge position={slot.position} /> Position {slot.position}: {slot.occupant ? playerLabel(slot) : 'Open'}{slot.occupant === lobby.host_player_id ? ' (Host)' : ''}</strong>
      <span className="lobby-list__status">{slot.occupant ? <><span className={`lobby-readiness ${slot.ready ? 'lobby-readiness--ready' : 'lobby-readiness--waiting'}`}>{slot.ready ? '✔ Ready' : '○ Not ready'}</span><span className={`lobby-presence ${slot.connected ? 'lobby-presence--connected' : 'lobby-presence--disconnected'}`}>{slot.connected ? '● Connected' : '◇ Disconnected'}</span></> : 'Available'}</span>
      {isHost && lobby.phase === 'lobby' && <span className="lobby-reorder"><button type="button" className="button button--outline button--icon" aria-label={`Move position ${slot.position} up`} disabled={!!pendingAction || index === 0} onClick={() => move(index, -1)}>↑</button><button type="button" className="button button--outline button--icon" aria-label={`Move position ${slot.position} down`} disabled={!!pendingAction || index === lobby.slots.length - 1} onClick={() => move(index, 1)}>↓</button></span>}
    </div>)}</div>
    {viewer && lobby.phase === 'lobby' && <button type="button" data-testid="ready-button" className={`button ${viewer.ready ? 'button--outline lobby-unready-button' : 'button--success'}`} disabled={!!pendingAction} onClick={() => onReady(!viewer.ready)}>{pendingAction === 'ready' ? 'Updating readiness…' : viewer.ready ? 'Mark not ready' : 'Mark ready'}</button>}
    {isHost && lobby.phase === 'lobby' && <div><button type="button" data-testid="start-game-button" className="button button--primary" disabled={!canStart || !!pendingAction} aria-describedby={startReason ? 'start-reason' : undefined} onClick={onStart}>{pendingAction === 'start' ? 'Starting game…' : 'Start game'}</button>{startReason && <p id="start-reason" className="text-muted">{startReason}</p>}</div>}
    {!playerId && <section aria-label="Join or watch" className="lobby-actions"><h2>{watching ? 'Watching as spectator' : 'Join or watch'}</h2><label className="field-label">Nickname<input className="input" disabled={!!pendingAction} value={nickname} onChange={(event) => setNickname(event.target.value)} /></label>{lobby.phase === 'lobby' && openCount > 0 && <button type="button" className="button button--success" disabled={!!pendingAction} onClick={() => onJoin(nickname)}>{pendingAction === 'join' ? 'Joining…' : 'Join game'}</button>}{!watching && <button type="button" className="button button--secondary" disabled={!!pendingAction} onClick={onWatch}>Watch</button>}
      {lobby.slots.filter((slot) => slot.occupant && slot.can_take_over).map((slot) => <button type="button" className="button button--outline" disabled={!!pendingAction} key={slot.slot_id} onClick={() => onTakeover(slot.occupant!, nickname)}>{pendingAction === 'takeover' ? 'Rejoining…' : `Rejoin as ${playerLabel(slot)} (position ${slot.position})`}</button>)}
    </section>}
    {viewer && !isHost && lobby.phase === 'lobby' && <button type="button" className="button button--secondary" disabled={!!pendingAction} onClick={onLeave}>{pendingAction === 'leave' ? 'Leaving…' : 'Leave lobby'}</button>}
  </section></main>;
};
