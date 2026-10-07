import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AutoResolveNotification } from "../components/AutoResolveToast.tsx";
import type { AutoResolvedNote, GameEvent, PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import {
  ActionToast,
  actionToastFromEvent,
  actionToastText,
  autoResolvedToast,
  mergeActionToast,
  victoryPointToasts,
} from "../presentation/actionToasts.ts";
import { useAutoResolveToasts } from "./useAutoResolveToasts.ts";
import { useToastMute } from "./useToastMute.ts";

export interface CornerToastsInput {
  /** The public event log as the session delivers it. */
  events: readonly GameEvent[];
  players?: readonly Pick<PlayerView, "id" | "victory_points">[];
  /** The viewer's seat; absent for spectators, who get every player's actions. */
  viewerSeat?: string | null;
  pendingChoice?: PendingChoiceDto | null;
  /** The server's notes for decisions it settled for this seat (state updates only). */
  autoResolved?: readonly AutoResolvedNote[];
  /** Decisions this client answered for the viewer (lone strategic action / activation). */
  localNotes?: readonly { id: string; text: string }[];
  /** False until the first snapshot arrived: what is already in the log is history, not news. */
  ready: boolean;
}

type Shown = ActionToast & { revision: number };

/**
 * Corner notifications: other players' public actions (from newly arrived log entries) and the
 * viewer's own auto-resolved single-choice decisions. Entries present when the game loads, and
 * entries that merely reappear after an undo or redo, never toast.
 */
export function useCornerToasts({
  events,
  players,
  viewerSeat,
  pendingChoice,
  autoResolved,
  localNotes,
  ready,
}: CornerToastsInput) {
  const { muted } = useToastMute();
  const auto = useAutoResolveToasts();
  const [actions, setActions] = useState<Shown[]>([]);
  const seen = useRef<Set<string> | null>(null);
  const lastVp = useRef<Map<string, number> | null>(null);
  const lastAutoNonce = useRef<string | null>(null);
  const shownNotes = useRef<Set<string>>(new Set());
  const mutedRef = useRef(muted);
  mutedRef.current = muted;

  const push = useCallback((incoming: ActionToast[]) => {
    if (incoming.length === 0) return;
    setActions((prev) => {
      let list: ActionToast[] = prev;
      for (const toast of incoming) list = mergeActionToast(list, toast);
      return list.map((t) => {
        const before = prev.find((p) => p.id === t.id);
        const grew = before && before.parts.length !== t.parts.length;
        return { ...t, revision: before ? before.revision + (grew ? 1 : 0) : 0 };
      });
    });
  }, []);

  useEffect(() => {
    if (!ready) return;
    if (seen.current === null) {
      seen.current = new Set(events.map((e) => e.id));
      return;
    }
    const fresh = events.filter((e) => !seen.current!.has(e.id));
    if (fresh.length === 0) return;
    for (const e of fresh) seen.current.add(e.id);
    if (mutedRef.current) return;
    push(fresh.flatMap((e) => actionToastFromEvent(e, viewerSeat) ?? []));
  }, [events, ready, viewerSeat, push]);

  useEffect(() => {
    if (!ready || !players) return;
    const now = new Map(players.map((p) => [p.id, p.victory_points]));
    const before = lastVp.current;
    lastVp.current = now;
    if (!before || mutedRef.current) return;
    push(
      victoryPointToasts(
        [...before].map(([id, victory_points]) => ({ id, victory_points })),
        players,
        viewerSeat,
      ),
    );
  }, [players, ready, viewerSeat, push]);

  const { showToast } = auto;
  useEffect(() => {
    const auto1 = autoResolvedToast(pendingChoice, viewerSeat);
    if (!auto1 || !pendingChoice || lastAutoNonce.current === pendingChoice.nonce) return;
    lastAutoNonce.current = pendingChoice.nonce;
    showToast(auto1.decisionType, auto1.selectedValue);
  }, [pendingChoice, viewerSeat, showToast]);

  useEffect(() => {
    if (!autoResolved) return;
    for (const note of autoResolved) {
      if (shownNotes.current.has(note.id)) continue;
      shownNotes.current.add(note.id);
      const times = note.count && note.count > 1 ? ` \u00d7${note.count}` : "";
      showToast(note.prompt, `${note.selected}${times}`, note.reason || undefined);
    }
  }, [autoResolved, showToast]);

  useEffect(() => {
    if (!localNotes) return;
    for (const note of localNotes) {
      if (shownNotes.current.has(note.id)) continue;
      shownNotes.current.add(note.id);
      showToast("action phase", note.text);
    }
  }, [localNotes, showToast]);

  // Muting also clears what is on screen.
  useEffect(() => {
    if (muted) setActions([]);
  }, [muted]);

  const dismissAction = useCallback(
    (id: string) => setActions((prev) => prev.filter((t) => t.id !== id)),
    [],
  );
  const { dismissToast } = auto;
  const dismiss = useCallback(
    (id: string) => {
      dismissAction(id);
      dismissToast(id);
    },
    [dismissAction, dismissToast],
  );

  const notifications = useMemo<AutoResolveNotification[]>(
    () => [
      ...auto.toasts,
      ...actions.map(
        (t): AutoResolveNotification => ({
          id: t.id,
          kind: "action",
          decisionType: "",
          selectedValue: "",
          actor: t.actor,
          text: actionToastText(t),
          revision: t.revision,
        }),
      ),
    ],
    [auto.toasts, actions],
  );

  return { notifications, dismiss, muted };
}
