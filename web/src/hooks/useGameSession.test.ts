import { describe, expect, it } from 'vitest';
import { GameLogEntry, serverEventLog } from './useGameSession.ts';

const entry = (id: string): GameLogEntry => ({
  id,
  timestamp: '12:00:00',
  version: 1,
  text: `Authoritative event ${id}`,
  category: 'action',
});

describe('serverEventLog', () => {
  it('renders only server-authored entries without synthesizing an initialization record', () => {
    expect(serverEventLog(undefined)).toEqual([]);
    expect(serverEventLog([entry('server-1')])).toEqual([entry('server-1')]);
  });

  it('keeps the latest bounded window of the authoritative event stream', () => {
    const events = Array.from({ length: 501 }, (_, index) => entry(`server-${index}`));
    const bounded = serverEventLog(events);

    expect(bounded).toHaveLength(500);
    expect(bounded[0]?.id).toBe('server-1');
    expect(bounded.at(-1)?.id).toBe('server-500');
  });
});
