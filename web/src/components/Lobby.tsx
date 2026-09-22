import React, { useState } from 'react';
import { CreateGameResponse, LobbyDto } from '../protocol/types.ts';
import { decodeCreateGameResponse } from '../protocol/decode.ts';

export const CreateLobby: React.FC<{ onCreated: (created: CreateGameResponse) => void; onError: (message: string) => void }> = ({ onCreated, onError }) => {
  const [count, setCount] = useState(3);
  const [seed, setSeed] = useState('');
  const [bots, setBots] = useState<string[]>([]);
  const seats = Array.from({ length: count }, (_, index) => `p${index + 1}`);

  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    const parsedSeed = seed === '' ? undefined : Number(seed);
    if (parsedSeed !== undefined && (!Number.isSafeInteger(parsedSeed) || parsedSeed < 0)) return onError('Seed must be a non-negative whole number.');
    try {
      const response = await fetch('/api/games', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ players: seats, seed: parsedSeed, bot_seats: bots }) });
      if (!response.ok) throw new Error(`Create game failed (${response.status}): ${await response.text()}`);
      onCreated(decodeCreateGameResponse(await response.json()));
    } catch (cause) { onError(String(cause)); }
  };

  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>Twilight Imperium 4</h1><p className="text-muted">Create a table and share its game URL.</p>
    <form className="lobby-form" onSubmit={create}>
      <label className="field-label">Players<select className="input" value={count} onChange={(event) => { const next = Number(event.target.value); setCount(next); setBots((current) => current.filter((seat) => Number(seat.slice(1)) <= next)); }}>{[2, 3, 4, 5, 6, 7, 8].map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
      <fieldset className="lobby-roster"><legend>Roster</legend>{seats.map((seat) => <label key={seat} className="lobby-roster__slot"><span>{seat}{seat === 'p1' ? ' (Host)' : ''}</span><select className="input" disabled={seat === 'p1'} value={bots.includes(seat) ? 'bot' : 'human'} onChange={(event) => setBots((current) => event.target.value === 'bot' ? [...current, seat] : current.filter((item) => item !== seat))}><option value="human">Human</option><option value="bot">Bot</option></select></label>)}</fieldset>
      <label className="field-label">Seed (advanced, optional)<input className="input" type="number" min="0" step="1" value={seed} onChange={(event) => setSeed(event.target.value)} /></label>
      <p className="text-faint">You claim p1 as the host. Other players choose an available human seat from the shared URL.</p><button data-testid="create-game-button" className="button button--success" type="submit">Create lobby</button>
    </form>
  </section></main>;
};

export const LobbyStatus: React.FC<{ lobby: LobbyDto; onReady: (ready: boolean) => void; onStart: () => void; onForget: () => void; onClaim: (seat: string) => void }> = ({ lobby, onReady, onStart, onForget, onClaim }) => {
  const viewerSeat = lobby.viewer?.seat;
  const viewer = lobby.roster.find((entry) => entry.seat === viewerSeat);
  const isHost = viewerSeat === lobby.host_seat;
  const copyUrl = async () => { await navigator.clipboard?.writeText(`${window.location.origin}/games/${encodeURIComponent(lobby.game_id)}`); };
  return <main data-testid="lobby-container" className="lobby-page"><section className="panel lobby-panel"><h1>{lobby.phase === 'running' ? 'Game in progress' : 'Game lobby'}</h1><p className="text-muted">Game: {lobby.game_id}</p><button className="button button--outline" onClick={() => void copyUrl()}>Copy game URL</button><div className="lobby-list">{lobby.roster.map((entry) => <div className="lobby-list__item" key={entry.seat}><strong>{entry.seat}{entry.seat === lobby.host_seat ? ' (Host)' : ''}</strong><span>{entry.controller === 'bot' ? 'Bot' : entry.available ? 'Available' : entry.ready ? 'Ready' : 'Occupied'}</span></div>)}</div>
    {viewer?.controller === 'human' && <button data-testid="ready-button" className="button button--success" onClick={() => onReady(!viewer.ready)}>{viewer.ready ? 'Not Ready' : 'Ready'}</button>}
    {isHost && <button data-testid="start-game-button" className="button button--primary" disabled={!lobby.can_start} onClick={onStart}>Start game</button>}
    {!lobby.can_start && <p className="text-muted">Waiting for every human player to be ready.</p>}
    {!viewerSeat && <section aria-label="Choose a seat"><h2>Choose an available seat</h2>{lobby.roster.filter((entry) => entry.controller === 'human' && entry.available).map((entry) => <button data-testid={`claim-seat-${entry.seat}`} className="button button--primary" key={entry.seat} onClick={() => onClaim(entry.seat)}>Claim {entry.seat}</button>)}</section>}
    {viewerSeat && <button className="button button--secondary" onClick={onForget}>Forget this seat</button>}
  </section></main>;
};
