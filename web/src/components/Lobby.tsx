import React, { useState } from 'react';
import { CreateGameResponse, LobbyDto, LobbySlot } from '../protocol/types.ts';
import { decodeCreateGameResponse } from '../protocol/decode.ts';
import { preferredNickname, rememberNickname, validNickname } from '../protocol/nickname.ts';

export const CreateLobby: React.FC<{ onCreated: (created: CreateGameResponse) => void; onError: (message: string) => void }> = ({ onCreated, onError }) => {
  const [count, setCount] = useState(3);
  const [seed, setSeed] = useState('');
  const [nickname, setNickname] = useState(preferredNickname);
  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    const parsedSeed = seed === '' ? undefined : Number(seed);
    if (parsedSeed !== undefined && (!Number.isSafeInteger(parsedSeed) || parsedSeed < 0)) return onError('Seed must be a non-negative whole number.');
    if (!validNickname(nickname)) return onError('Nickname must be 1–64 UTF-8 bytes, trimmed, without control or format characters.');
    try {
      const response = await fetch('/api/games', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ player_count: count, seed: parsedSeed, nickname }) });
      if (!response.ok) throw new Error(`Create game failed (${response.status}): ${await response.text()}`);
      const created = decodeCreateGameResponse(await response.json());
      rememberNickname(nickname);
      onCreated(created);
    } catch (cause) { onError(String(cause)); }
  };
  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>Twilight Imperium 4</h1><p className="text-muted">Create a table and share its game URL.</p>
    <form className="lobby-form" onSubmit={create}>
      <label className="field-label">Players<select className="input" value={count} onChange={(event) => setCount(Number(event.target.value))}>{[2, 3, 4, 5, 6, 7, 8].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
      <label className="field-label">Nickname<input className="input" value={nickname} onChange={(event) => setNickname(event.target.value)} /></label>
      <label className="field-label">Seed (advanced, optional)<input className="input" type="number" min="0" step="1" value={seed} onChange={(event) => setSeed(event.target.value)} /></label>
      <p className="text-faint">You join as host. Other players and bots join from the shared URL.</p><button data-testid="create-game-button" className="button button--success" type="submit">Create lobby</button>
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
}

/** Nicknames are public display data; the position distinguishes duplicate names. */
function playerLabel(slot: LobbySlot): string {
  return slot.nickname ?? `Player at position ${slot.position}`;
}

export const LobbyStatus: React.FC<LobbyStatusProps> = ({ lobby, playerId, onReady, onStart, onLeave, onJoin, onTakeover, onReorder, onWatch, watching }) => {
  const [nickname, setNickname] = useState(preferredNickname);
  const viewer = lobby.slots.find((slot) => slot.occupant === playerId);
  const isHost = playerId !== null && playerId === lobby.host_player_id;
  const canStart = lobby.phase === 'lobby' && lobby.slots.every((slot) => slot.occupant && slot.ready);
  const copyUrl = async () => { await navigator.clipboard?.writeText(`${window.location.origin}${window.location.pathname}`); };
  const move = (index: number, delta: number) => {
    const slots = lobby.slots.map((slot) => slot.slot_id);
    [slots[index], slots[index + delta]] = [slots[index + delta], slots[index]];
    onReorder(slots);
  };
  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>{lobby.phase === 'running' ? 'Game in progress' : 'Game lobby'}</h1><button className="button button--outline" onClick={() => void copyUrl()}>Copy game URL</button>
    <div className="lobby-list">{lobby.slots.map((slot, index) => <div className="lobby-list__item" key={slot.slot_id}>
      <strong>Position {slot.position}: {slot.occupant ? playerLabel(slot) : 'Open'}{slot.occupant === lobby.host_player_id ? ' (Host)' : ''}</strong>
      <span>{slot.occupant ? `${slot.ready ? 'Ready' : 'Not ready'} · ${slot.connected ? 'Connected' : 'Disconnected'}` : 'Available'}</span>
      {isHost && lobby.phase === 'lobby' && <span><button aria-label={`Move position ${slot.position} up`} disabled={index === 0} onClick={() => move(index, -1)}>↑</button><button aria-label={`Move position ${slot.position} down`} disabled={index === lobby.slots.length - 1} onClick={() => move(index, 1)}>↓</button></span>}
    </div>)}</div>
    {viewer && lobby.phase === 'lobby' && <button data-testid="ready-button" className="button button--success" onClick={() => onReady(!viewer.ready)}>{viewer.ready ? 'Not Ready' : 'Ready'}</button>}
    {isHost && lobby.phase === 'lobby' && <button data-testid="start-game-button" className="button button--primary" disabled={!canStart} onClick={onStart}>Start game</button>}
    {lobby.phase === 'lobby' && !canStart && <p className="text-muted">Waiting for all positions to be filled and ready.</p>}
    {!playerId && <section aria-label="Join or watch"><h2>{watching ? 'Watching as spectator' : 'Join or watch'}</h2><label className="field-label">Nickname<input className="input" value={nickname} onChange={(event) => setNickname(event.target.value)} /></label>{lobby.phase === 'lobby' && lobby.slots.some((slot) => !slot.occupant) && <button onClick={() => onJoin(nickname)}>Join game</button>}{!watching && <button onClick={onWatch}>Watch</button>}
      {lobby.slots.filter((slot) => slot.occupant && slot.can_take_over).map((slot) => <button key={slot.slot_id} onClick={() => onTakeover(slot.occupant!, nickname)}>Rejoin as {playerLabel(slot)} (position {slot.position})</button>)}
    </section>}
    {viewer && !isHost && lobby.phase === 'lobby' && <button className="button button--secondary" onClick={onLeave}>Leave lobby</button>}
  </section></main>;
};
