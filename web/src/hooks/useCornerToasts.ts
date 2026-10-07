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
import { buildTurnRecap, trackTurns, type OpenTurn, type TurnRecap } from "../presentation/turnRecap.ts";
import { isTurnMenuChoice } from "../presentation/turnBar.ts";
import { useTurnRecapSetting } from "./useTurnRecapSetting.ts";
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
  /** The history generation of the session; a new one replaces the timeline (undo/redo, rewind). */
  historyGeneration?: number;
  /** False until the first snapshot arrived: what is already in the log is history, not news. */
  ready: boolean;
}

const MAX_RECAPS = 2;

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
  historyGeneration,
  ready,
}: CornerToastsInput) {
  const { muted } = useToastMute();
  const { enabled: recapOn } = useTurnRecapSetting();
  const recapRef = useRef(recapOn);
  recapRef.current = recapOn;
  const openTurn = useRef<OpenTurn | null>(null);
  const [recaps, setRecaps] = useState<TurnRecap[]>([]);
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

  // One recap per call, and only the latest when several turns closed at once (a reconnect that
  // catches up on missed entries must not replay the game).
  const emitRecap = useCallback((closed: readonly OpenTurn[]) => {
    if (!recapRef.current || mutedRef.current) return;
    const recap = closed.length ? buildTurnRecap(closed[closed.length - 1]) : null;
    if (!recap) return;
    setRecaps((prev) => [...prev.filter((r) => r.id !== recap.id), recap].slice(-MAX_RECAPS));
  }, []);

  // A new history generation replaced the timeline: nothing open belongs to it.
  const lastGeneration = useRef(historyGeneration);
  useEffect(() => {
    if (lastGeneration.current !== historyGeneration) {
      lastGeneration.current = historyGeneration;
      openTurn.current = null;
    }
  }, [historyGeneration]);

  useEffect(() => {
    if (!ready) return;
    if (seen.current === null) {
      seen.current = new Set(events.map((e) => e.id));
      return;
    }
    // Undo removed entries of the open turn: forget it.
    if (openTurn.current) {
      const present = new Set(events.map((e) => e.id));
      if (openTurn.current.entries.some((e) => !present.has(e.id))) openTurn.current = null;
    }
    const fresh = events.filter((e) => !seen.current!.has(e.id));
    if (fresh.length === 0) return;
    for (const e of fresh) seen.current.add(e.id);
    const turns = trackTurns(openTurn.current, fresh, viewerSeat);
    openTurn.current = turns.open;
    emitRecap(turns.closed);
    if (mutedRef.current) return;
    // With the recap on, the acting player's own steps are summed up at the end of their turn.
    const live = recapRef.current ? fresh.filter((e) => !turns.tracked.has(e.id)) : fresh;
    push(live.flatMap((e) => actionToastFromEvent(e, viewerSeat) ?? []));
  }, [events, ready, viewerSeat, push, emitRecap]);

  // The viewer's own action menu means the previous player's turn is over.
  useEffect(() => {
    const open = openTurn.current;
    if (!open || !viewerSeat || !pendingChoice) return;
    if (pendingChoice.actor !== viewerSeat || !isTurnMenuChoice(pendingChoice)) return;
    openTurn.current = null;
    emitRecap([open]);
  }, [pendingChoice, viewerSeat, emitRecap]);

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

  // Muting also clears what is on screen.
  useEffect(() => {
    if (muted) setActions([]);
  }, [muted]);
  useEffect(() => {
    if (muted || !recapOn) setRecaps([]);
  }, [muted, recapOn]);

  const dismissAction = useCallback(
    (id: string) => setActions((prev) => prev.filter((t) => t.id !== id)),
    [],
  );
  const { dismissToast } = auto;
  const dismiss = useCallback(
    (id: string) => {
      setRecaps((prev) => prev.filter((r) => r.id !== id));
      dismissAction(id);
      dismissToast(id);
    },
    [dismissAction, dismissToast],
  );

  const notifications = useMemo<AutoResolveNotification[]>(
    () => [
      ...auto.toasts,
      ...recaps.map(
        (r): AutoResolveNotification => ({
          id: r.id,
          kind: "recap",
          decisionType: "",
          selectedValue: "",
          actor: r.actor,
          text: r.text,
        }),
      ),
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
    [auto.toasts, actions, recaps],
  );

  return { notifications, dismiss, muted };
}
