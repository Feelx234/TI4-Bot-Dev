import { visibilityKind } from "../protocol/eventVisibility.ts";
import type { GameEvent, PendingChoiceDto, ViewerRole } from "../protocol/types.ts";
import type { SnapshotState } from "../protocol/client.ts";

/**
 * The context attached to a feedback report.
 *
 * It is built only from what the viewer's own client already holds: the viewer-projected snapshot
 * (`view`, never the raw `state`), the viewer's own pending decision and the PUBLIC events. Every
 * field is read explicitly; nothing is spread from a source object, so new fields on the wire
 * cannot leak into a report by accident.
 */

export interface FeedbackSourceData {
  gameId?: string;
  viewer?: ViewerRole;
  snapshot?: SnapshotState | null;
  pendingChoice?: PendingChoiceDto | null;
  events?: GameEvent[];
}

export interface FeedbackEnvironment {
  commit: string;
  serverProtocol: number | null;
  clientProtocol: number;
  viewport: { width: number; height: number; pixelRatio: number };
  userAgent: string;
  now: Date;
  path: string;
}

export const MAX_RECENT_EVENTS = 20;
const MAX_OPTION_IDS = 60;
const MAX_TEXT = 300;

const clip = (value: unknown): string | undefined =>
  typeof value === "string" ? value.slice(0, MAX_TEXT) : undefined;

export function recentPublicEvents(events: GameEvent[] | undefined, limit = MAX_RECENT_EVENTS) {
  return (events ?? [])
    .filter((event) => visibilityKind(event.visibility) === "public")
    .slice(-limit)
    .map((event) => ({
      id: clip(event.id),
      round: event.round,
      phase: clip(event.phase),
      actor: clip(event.actor),
      action_type: clip(event.action_type),
      stage: clip(event.stage),
      detail: clip(event.detail),
    }));
}

export function buildFeedbackContext(
  source: FeedbackSourceData,
  env: FeedbackEnvironment,
  includeGameInfo: boolean,
): Record<string, unknown> {
  const context: Record<string, unknown> = {
    build_commit: env.commit,
    server_protocol_version: env.serverProtocol,
    client_protocol_version: env.clientProtocol,
    viewport: env.viewport,
    user_agent: env.userAgent.slice(0, 400),
    timestamp: env.now.toISOString(),
    path: env.path.replace(/^\/games\/[^/]*/, "/games/:id"),
  };
  if (!includeGameInfo) return context;
  if (source.gameId) context.game_id = source.gameId;
  const viewer = source.viewer;
  if (viewer) {
    context.viewer_role = viewer.role;
    if (viewer.role === "player") context.viewer_seat = viewer.seat;
  }
  const view = source.snapshot?.view;
  if (view) {
    context.round = view.round;
    context.phase = view.phase;
    context.active_player = view.active_player ?? null;
    if (viewer?.role === "player") {
      const me = view.players.find((player) => player.id === viewer.seat);
      if (me) context.faction = me.faction;
    }
  }
  const pending = source.pendingChoice;
  if (pending) {
    context.pending_decision = {
      subtype: clip(pending.context?.subtype) ?? null,
      prompt: clip(pending.prompt) ?? null,
      option_ids: pending.options.slice(0, MAX_OPTION_IDS).map((option) => option.id),
    };
  }
  context.recent_public_events = recentPublicEvents(source.events);
  return context;
}

export function currentEnvironment(commit: string, serverProtocol: number | null, clientProtocol: number): FeedbackEnvironment {
  return {
    commit,
    serverProtocol,
    clientProtocol,
    viewport: {
      width: window.innerWidth,
      height: window.innerHeight,
      pixelRatio: window.devicePixelRatio || 1,
    },
    userAgent: navigator.userAgent,
    now: new Date(),
    path: window.location.pathname,
  };
}
