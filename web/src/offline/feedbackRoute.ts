import { activeOfflineGame, getOfflineLauncher } from "./registry.ts";
import type { FeedbackCategory } from "./types.ts";

export type RoutedFeedbackResult =
  | { ok: true; id: string; queued?: boolean }
  | { ok: false; message: string };

/**
 * Sends a report like the online sheet does, except that during an offline game it is put into
 * the launcher's queue (resolves at once, `queued: true`) and goes out in the background. The
 * game never waits for the network.
 */
export async function routeFeedback(
  body: { category: FeedbackCategory; text: string; context: Record<string, unknown> },
  online: (body: {
    category: FeedbackCategory;
    text: string;
    context: Record<string, unknown>;
  }) => Promise<RoutedFeedbackResult>,
): Promise<RoutedFeedbackResult> {
  const game = activeOfflineGame();
  const launcher = getOfflineLauncher();
  if (!game || !launcher) return online(body);
  try {
    const item = await launcher.feedback.enqueue({ ...body, local: game.local() });
    return { ok: true, id: item.localId, queued: true };
  } catch {
    return { ok: false, message: "Could not save the report on this device. Try again." };
  }
}
