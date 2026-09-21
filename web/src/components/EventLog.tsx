import React, { useState } from 'react';
import { GameLogEntry } from '../hooks/useGameSession.ts';

export interface EventLogProps {
  events: GameLogEntry[];
}

function getCategoryColor(category?: GameLogEntry['category']): string {
  switch (category) {
    case 'action':
      return '#38bdf8'; // Sky blue
    case 'decision':
      return '#f59e0b'; // Amber
    case 'phase':
      return '#c084fc'; // Purple
    case 'status':
      return '#34d399'; // Emerald
    case 'error':
      return '#f87171'; // Red
    case 'system':
    default:
      return '#e2e8f0'; // Light slate
  }
}

export const EventLog: React.FC<EventLogProps> = ({ events }) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div
      data-testid="event-log-container"
      className="event-log"
      style={{
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 330, // leave space for player sheet
        borderTop: '1px solid var(--color-border)',
        color: '#cbd5e1',
        fontSize: 12,
        zIndex: 35,
      }}
    >
      <button
        type="button"
        data-testid="event-log-toggle"
        onClick={() => setIsOpen((open) => !open)}
        aria-expanded={isOpen}
        className="button"
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '6px 16px',
          width: '100%',
          textAlign: 'left',
          background: 'var(--color-surface)',
          fontWeight: 600,
          color: 'var(--color-text-muted)',
          userSelect: 'none',
        }}
      >
        <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span>Event Log</span>
          <span
            style={{
              background: '#1e293b',
              color: '#38bdf8',
              padding: '1px 6px',
              borderRadius: 10,
              fontSize: 11,
            }}
          >
            {events.length}
          </span>
        </span>
        <span style={{ fontSize: 11 }}>{isOpen ? '▼ Hide' : '▲ Show'}</span>
      </button>

      {isOpen && (
        <div
          data-testid="event-log-list"
          style={{
            maxHeight: 200,
            overflowY: 'auto',
            padding: '10px 16px',
            display: 'flex',
            flexDirection: 'column',
            gap: 6,
          }}
        >
          {events.length === 0 ? (
            <div style={{ color: '#64748b' }}>No events recorded yet.</div>
          ) : (
            events.map((ev, i) => {
              return (
                <div
                  key={ev.id}
                  data-testid="event-log-entry"
                  style={{
                    fontFamily: 'ui-monospace, monospace',
                    fontSize: 12,
                    display: 'flex',
                    gap: 8,
                    alignItems: 'baseline',
                    lineHeight: 1.4,
                  }}
                >
                  <span style={{ color: '#475569', fontSize: 11, flexShrink: 0 }}>
                    [{i + 1}]
                  </span>
                  {ev.timestamp && (
                    <span style={{ color: '#64748b', fontSize: 11, flexShrink: 0 }}>
                      {ev.timestamp}
                    </span>
                  )}
                  {ev.version !== undefined && (
                    <span
                      style={{
                        color: '#0284c7',
                        fontSize: 10,
                        background: '#0c4a6e',
                        padding: '1px 4px',
                        borderRadius: 3,
                        flexShrink: 0,
                      }}
                    >
                      v{ev.version}
                    </span>
                  )}
                  <span style={{ color: getCategoryColor(ev.category), wordBreak: 'break-word' }}>
                    {ev.text}
                  </span>
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
};
