import { InitialSnapshotMsg, PROTOCOL_VERSION, ServerMessage } from './types.ts';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function fail(message: string): never {
  throw new Error(`Invalid server message: ${message}`);
}

/** Validates the protocol envelope before React consumes any network payload. */
export function decodeServerMessage(value: unknown, expectedGameId: string): ServerMessage {
  if (!isRecord(value) || typeof value.type !== 'string') fail('missing message type');
  if (value.protocol_version !== PROTOCOL_VERSION) fail('unsupported protocol version');

  const gameScoped = value.type !== 'error' && value.type !== 'pong';
  if (gameScoped && value.game_id !== expectedGameId) fail('unexpected game id');

  switch (value.type) {
    case 'initial_snapshot':
    case 'state_update':
      if (!isNonNegativeInteger(value.game_version) || !isRecord(value.view) || !isRecord(value.viewer) || !isRecord(value.turn_status)) fail('invalid snapshot');
      return value as unknown as ServerMessage;
    case 'pending_choice':
      if (!isNonNegativeInteger(value.game_version) || !isRecord(value.choice)) fail('invalid pending choice');
      return value as unknown as ServerMessage;
    case 'turn_status':
      if (!isNonNegativeInteger(value.game_version) || !isRecord(value.status)) fail('invalid turn status');
      return value as unknown as ServerMessage;
    case 'action_accepted':
      if (!isNonNegativeInteger(value.game_version) || typeof value.option_id !== 'string') fail('invalid accepted action');
      return value as unknown as ServerMessage;
    case 'action_rejected':
      if (!isNonNegativeInteger(value.game_version) || !isRecord(value.reason)) fail('invalid rejected action');
      return value as unknown as ServerMessage;
    case 'event':
      if (!isRecord(value.entry) || typeof value.entry.id !== 'string') fail('invalid event');
      return value as unknown as ServerMessage;
    case 'game_over':
      if (!isNonNegativeInteger(value.game_version) || !isRecord(value.final_scores)) fail('invalid game-over message');
      return value as unknown as ServerMessage;
    case 'error':
      if (typeof value.kind !== 'string' || typeof value.message !== 'string') fail('invalid error');
      return value as unknown as ServerMessage;
    case 'pong':
      if (!isNonNegativeInteger(value.sequence)) fail('invalid pong');
      return value as unknown as ServerMessage;
    default:
      return fail('unknown message type');
  }
}

export function decodeInitialSnapshot(value: unknown, expectedGameId: string): InitialSnapshotMsg {
  const message = decodeServerMessage(value, expectedGameId);
  if (message.type !== 'initial_snapshot') fail('expected initial snapshot');
  return message;
}

export function isStaleServerMessage(message: ServerMessage, currentVersion: number): boolean {
  const version = 'game_version' in message
    ? message.game_version
    : message.type === 'event'
      ? message.entry.version
      : undefined;
  return version !== undefined && version < currentVersion;
}
