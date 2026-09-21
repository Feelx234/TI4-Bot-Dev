import React, { useState } from 'react';
import { ViewerRole } from '../protocol/types.ts';

export interface LobbyProps {
  onJoin: (gameId: string, role: ViewerRole) => void;
}

export const Lobby: React.FC<LobbyProps> = ({ onJoin }) => {
  const [gameId, setGameId] = useState('demo');
  const [roleType, setRoleType] = useState<'player' | 'spectator'>('player');
  const [seat, setSeat] = useState('p1');
  const [seatToken, setSeatToken] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [newGameId, setNewGameId] = useState('');
  const [seed, setSeed] = useState('42');
  const [botSeats, setBotSeats] = useState<string[]>(['p3']);
  const [createError, setCreateError] = useState<string | null>(null);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsedSeed = Number(seed);
    if (!Number.isSafeInteger(parsedSeed) || parsedSeed < 0) {
      setCreateError('Seed must be a non-negative whole number.');
      return;
    }
    try {
      const res = await fetch('/api/games', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          game_id: newGameId.trim() || undefined,
          players: ['p1', 'p2', 'p3'],
          seed: parsedSeed,
          bot_seats: botSeats,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        onJoin(data.game_id, { role: 'player', seat: 'p1', seatToken: data.seat_tokens.p1 });
      } else {
        alert('Failed to create game: ' + (await res.text()));
      }
    } catch (err) {
      alert('Network error creating game: ' + String(err));
    }
  };

  return (
    <div
      data-testid="lobby-container"
      className="app-shell"
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        padding: 20,
      }}
    >
      <div
        className="panel"
        style={{
          padding: 32,
          maxWidth: 440,
          width: '100%',
        }}
      >
        <h1 style={{ fontSize: 24, fontWeight: 'bold', color: '#38bdf8', marginTop: 0, marginBottom: 8 }}>
          Twilight Imperium 4
        </h1>
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24 }}>
          Authoritative Online Multiplayer Vertical Slice
        </p>

        {!isCreating ? (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              onJoin(
                gameId,
                roleType === 'player' ? { role: 'player', seat, seatToken } : { role: 'spectator' }
              );
            }}
            style={{ display: 'flex', flexDirection: 'column', gap: 16 }}
          >
            <div>
              <label className="field-label">
                Game Session ID:
              </label>
              <input
                data-testid="input-game-id"
                type="text"
                value={gameId}
                onChange={(e) => setGameId(e.target.value)}
                className="input"
              />
            </div>

            <div>
              <label className="field-label">
                Role:
              </label>
              <div style={{ display: 'flex', gap: 16 }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="role"
                    checked={roleType === 'player'}
                    onChange={() => setRoleType('player')}
                  />
                  Player Seat
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                  <input
                    type="radio"
                    name="role"
                    checked={roleType === 'spectator'}
                    onChange={() => setRoleType('spectator')}
                  />
                  Spectator
                </label>
              </div>
            </div>

            {roleType === 'player' && (
              <>
                <div>
                <label className="field-label">
                  Select Seat:
                </label>
                <input
                  data-testid="select-seat"
                  type="text"
                  value={seat}
                  onChange={(e) => setSeat(e.target.value)}
                  className="input"
                />
                </div>
                <div>
                  <label className="field-label">
                    Seat Capability:
                  </label>
                  <input
                    data-testid="input-seat-token"
                    type="password"
                    required
                    value={seatToken}
                    onChange={(e) => setSeatToken(e.target.value)}
                    className="input"
                  />
                </div>
              </>
            )}

            <button
              type="submit"
              data-testid="join-game-button"
              className="button button--primary"
              style={{
                padding: '12px',
                marginTop: 8,
              }}
            >
              Join Game Session
            </button>

            <button
              type="button"
              onClick={() => setIsCreating(true)}
              className="button button--outline"
              style={{
                padding: '10px',
                fontSize: 13,
              }}
            >
              + Create Three-Seat Demo
            </button>
          </form>
        ) : (
          <form onSubmit={handleCreate} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <p style={{ color: '#94a3b8', fontSize: 13, margin: 0 }}>
              Demo setup: you join as p1; p2 and p3 can be bots.
            </p>
            <div>
              <label className="field-label" style={{ marginBottom: 4 }}>
                Custom Game ID (optional):
              </label>
              <input
                type="text"
                value={newGameId}
                placeholder="e.g. game_my_session"
                onChange={(e) => setNewGameId(e.target.value)}
                className="input"
                style={{ padding: '8px 12px' }}
              />
            </div>

            <div>
              <label className="field-label" style={{ marginBottom: 4 }}>
                RNG Seed:
              </label>
              <input
                type="number"
                step="1"
                value={seed}
                onChange={(e) => setSeed(e.target.value)}
                className="input"
                style={{ padding: '8px 12px' }}
              />
            </div>

            <div>
              <label className="field-label" style={{ marginBottom: 4 }}>
                Bot Seat Assignment:
              </label>
              <div style={{ display: 'flex', gap: 12 }}>
                {['p2', 'p3'].map((p) => (
                  <label key={p} style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 13 }}>
                    <input
                      type="checkbox"
                      checked={botSeats.includes(p)}
                      onChange={(e) => {
                        if (e.target.checked) setBotSeats([...botSeats, p]);
                        else setBotSeats(botSeats.filter((b) => b !== p));
                      }}
                    />
                    {p} (Bot)
                  </label>
                ))}
              </div>
            </div>

            {createError && <div role="alert" style={{ color: '#fca5a5', fontSize: 13 }}>{createError}</div>}

            <div style={{ display: 'flex', gap: 10, marginTop: 8 }}>
              <button
                type="submit"
                className="button button--success"
                style={{
                  flex: 1,
                  padding: '10px',
                }}
              >
                Create & Join
              </button>
              <button
                type="button"
                onClick={() => setIsCreating(false)}
                className="button button--secondary"
                style={{
                  padding: '10px',
                  fontSize: 13,
                }}
              >
                Cancel
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
};
