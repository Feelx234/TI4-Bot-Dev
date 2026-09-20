import React, { useState } from 'react';
import { ViewerRole } from '../protocol/types.ts';

export interface LobbyProps {
  onJoin: (gameId: string, role: ViewerRole) => void;
}

export const Lobby: React.FC<LobbyProps> = ({ onJoin }) => {
  const [gameId, setGameId] = useState('demo');
  const [roleType, setRoleType] = useState<'player' | 'spectator'>('player');
  const [seat, setSeat] = useState('p1');
  const [isCreating, setIsCreating] = useState(false);
  const [newGameId, setNewGameId] = useState('');
  const [seed, setSeed] = useState('42');
  const [botSeats, setBotSeats] = useState<string[]>(['p3']);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const res = await fetch('/api/games', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          game_id: newGameId.trim() || undefined,
          players: ['p1', 'p2', 'p3'],
          seed: parseInt(seed, 10) || 42,
          bot_seats: botSeats,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        onJoin(data.game_id, { role: 'player', seat: 'p1' });
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
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100vh',
        background: '#090d16',
        color: '#f8fafc',
        padding: 20,
      }}
    >
      <div
        style={{
          background: '#0f172a',
          border: '1px solid #1e293b',
          borderRadius: 12,
          padding: 32,
          maxWidth: 440,
          width: '100%',
          boxShadow: '0 12px 40px rgba(0, 0, 0, 0.7)',
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
                roleType === 'player' ? { role: 'player', seat } : { role: 'spectator' }
              );
            }}
            style={{ display: 'flex', flexDirection: 'column', gap: 16 }}
          >
            <div>
              <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 6 }}>
                Game Session ID:
              </label>
              <input
                data-testid="input-game-id"
                type="text"
                value={gameId}
                onChange={(e) => setGameId(e.target.value)}
                style={{
                  width: '100%',
                  boxSizing: 'border-box',
                  background: '#1e293b',
                  border: '1px solid #334155',
                  borderRadius: 6,
                  color: '#f8fafc',
                  padding: '10px 12px',
                  fontSize: 14,
                }}
              />
            </div>

            <div>
              <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 6 }}>
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
              <div>
                <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 6 }}>
                  Select Seat:
                </label>
                <select
                  data-testid="select-seat"
                  value={seat}
                  onChange={(e) => setSeat(e.target.value)}
                  style={{
                    width: '100%',
                    background: '#1e293b',
                    border: '1px solid #334155',
                    borderRadius: 6,
                    color: '#f8fafc',
                    padding: '10px 12px',
                    fontSize: 14,
                  }}
                >
                  <option value="p1">Seat P1</option>
                  <option value="p2">Seat P2</option>
                  <option value="p3">Seat P3</option>
                </select>
              </div>
            )}

            <button
              type="submit"
              data-testid="join-game-button"
              style={{
                background: '#0284c7',
                color: '#ffffff',
                border: 'none',
                borderRadius: 6,
                padding: '12px',
                fontWeight: 'bold',
                fontSize: 14,
                cursor: 'pointer',
                marginTop: 8,
              }}
            >
              Join Game Session
            </button>

            <button
              type="button"
              onClick={() => setIsCreating(true)}
              style={{
                background: 'transparent',
                color: '#38bdf8',
                border: '1px solid #334155',
                borderRadius: 6,
                padding: '10px',
                fontSize: 13,
                cursor: 'pointer',
              }}
            >
              + Create New Game
            </button>
          </form>
        ) : (
          <form onSubmit={handleCreate} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <div>
              <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 4 }}>
                Custom Game ID (optional):
              </label>
              <input
                type="text"
                value={newGameId}
                placeholder="e.g. game_my_session"
                onChange={(e) => setNewGameId(e.target.value)}
                style={{
                  width: '100%',
                  boxSizing: 'border-box',
                  background: '#1e293b',
                  border: '1px solid #334155',
                  borderRadius: 6,
                  color: '#f8fafc',
                  padding: '8px 12px',
                  fontSize: 14,
                }}
              />
            </div>

            <div>
              <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 4 }}>
                RNG Seed:
              </label>
              <input
                type="number"
                value={seed}
                onChange={(e) => setSeed(e.target.value)}
                style={{
                  width: '100%',
                  boxSizing: 'border-box',
                  background: '#1e293b',
                  border: '1px solid #334155',
                  borderRadius: 6,
                  color: '#f8fafc',
                  padding: '8px 12px',
                  fontSize: 14,
                }}
              />
            </div>

            <div>
              <label style={{ display: 'block', fontSize: 13, color: '#cbd5e1', marginBottom: 4 }}>
                Bot Seat Assignment:
              </label>
              <div style={{ display: 'flex', gap: 12 }}>
                {['p1', 'p2', 'p3'].map((p) => (
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

            <div style={{ display: 'flex', gap: 10, marginTop: 8 }}>
              <button
                type="submit"
                style={{
                  flex: 1,
                  background: '#10b981',
                  color: '#ffffff',
                  border: 'none',
                  borderRadius: 6,
                  padding: '10px',
                  fontWeight: 'bold',
                  fontSize: 14,
                  cursor: 'pointer',
                }}
              >
                Create & Join
              </button>
              <button
                type="button"
                onClick={() => setIsCreating(false)}
                style={{
                  background: '#334155',
                  color: '#f8fafc',
                  border: 'none',
                  borderRadius: 6,
                  padding: '10px',
                  fontSize: 13,
                  cursor: 'pointer',
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
