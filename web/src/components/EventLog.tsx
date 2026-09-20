import React, { useState } from 'react';
import { GameLogEntry } from '../hooks/useGameSession.ts';

export interface EventLogProps {
  events: (GameLogEntry | string)[];
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
      style={{
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 330, // leave space for player sheet
        background: '#090d16',
        borderTop: '1px solid #1e293b',
        color: '#cbd5e1',
        fontSize: 12,
        zIndex: 35,
      }}
    >
      <div
        data-testid="event-log-toggle"
        onClick={() => setIsOpen(!isOpen)}
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '6px 16px',
          cursor: 'pointer',
          background: '#0f172a',
          fontWeight: 600,
          color: '#94a3b8',
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
      </div>

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
            background: '#090d16',
          }}
        >
          {events.length === 0 ? (
            <div style={{ color: '#64748b' }}>No events recorded yet.</div>
          ) : (
            events.map((ev, i) => {
              const isObj = typeof ev !== 'string';
              const text = isObj ? ev.text : ev;
              const timestamp = isObj ? ev.timestamp : undefined;
              const version = isObj ? ev.version : undefined;
              const category = isObj ? ev.category : undefined;

              return (
                <div
                  key={isObj ? ev.id : i}
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
                  {timestamp && (
                    <span style={{ color: '#64748b', fontSize: 11, flexShrink: 0 }}>
                      {timestamp}
                    </span>
                  )}
                  {version !== undefined && (
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
                      v{version}
                    </span>
                  )}
                  <span style={{ color: getCategoryColor(category), wordBreak: 'break-word' }}>
                    {text}
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
