import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { ViewerRole } from "./protocol/types.ts";
import { useGameSession } from "./hooks/useGameSession.ts";
import { useLobbySession } from "./hooks/useLobbySession.ts";
import { useTurnSound } from "./hooks/useTurnSound.ts";
import { useTurnRedo } from "./hooks/useTurnRedo.ts";
import { playerDisplay } from "./presentation/playerDisplay.ts";
import { Board } from "./components/Board.tsx";
import { TurnStatusBar } from "./components/TurnStatusBar.tsx";
import { PlayerSheet } from "./components/PlayerSheet.tsx";
import { CreateLobby, LobbyStatus } from "./components/Lobby.tsx";
import { GameShell } from "./components/GameShell.tsx";
import { usePresence } from "./hooks/usePresence.ts";
import { logHistoryKey as logHistoryKeyFor } from "./components/eventLogScroll.ts";
import { PlayerIdentityProvider } from "./presentation/PlayerIdentity.tsx";
import { SeatInfoProvider } from "./presentation/SeatInfoContext.tsx";
import { DecisionTableProvider } from "./components/PoliticsDecisionParts.tsx";
import { participantText } from "./presentation/participantText.ts";
import { CardDetails, CardSubject } from "./components/CardDetails.tsx";
import { TechnologyModal } from "./components/TechnologyModal.tsx";
import { ObjectivesModal } from "./components/ObjectivesModal.tsx";
import { resolveMapTargetSelection } from "./presentation/planetSelection.ts";
import { isPlanetSelectionChoice } from "./presentation/choiceModel.ts";
import {
  derivePaymentOffer,
  isPaymentChoice,
  paymentOptionForPlanet,
  paymentPlanetKey,
} from "./presentation/paymentDraft.ts";
import { UndoConfirmDialog } from "./components/UndoConfirmDialog.tsx";
import { useSecondaryPrepare, useTokenPrefill } from "./hooks/useSecondaryPrepare.ts";
import { SecondaryPrepHost } from "./components/SecondaryPrepHost.tsx";
import { BluffHoldBar, BluffProvider } from "./components/BluffSelector.tsx";
import { PreparedHintProvider } from "./presentation/PreparedHint.tsx";
import { CornerToastLayer } from "./components/CornerToastLayer.tsx";
import { PaymentDraftProvider, usePaymentDraftState } from "./presentation/PaymentDraftContext.tsx";
import {
  CommandTokenDraftProvider,
  useCommandTokenDraft,
} from "./presentation/CommandTokenDraftContext.tsx";

const DevDecisionGallery = import.meta.env.DEV
  ? React.lazy(() =>
      import("./dev/DecisionGallery.tsx").then(({ DecisionGallery }) => ({
        default: DecisionGallery,
      })),
    )
  : null;

const DevScenarioLauncher = import.meta.env.DEV
  ? React.lazy(() =>
      import("./dev/ScenarioLauncher.tsx").then(({ ScenarioLauncher }) => ({
        default: ScenarioLauncher,
      })),
    )
  : null;

const DevMapPickerGallery = import.meta.env.DEV
  ? React.lazy(() =>
      import("./dev/MapPickerGallery.tsx").then(({ MapPickerGallery }) => ({
        default: MapPickerGallery,
      })),
    )
  : null;
const DevToastGallery = import.meta.env.DEV
  ? React.lazy(() =>
      import("./dev/ToastGallery.tsx").then(({ ToastGallery }) => ({
        default: ToastGallery,
      })),
    )
  : null;

const storageKey = (gameId: string) => `ti4.player-session:${gameId}`;
const pathGameId = () =>
  /^\/games\/([^/]+)$/.exec(window.location.pathname)?.[1]
    ? decodeURIComponent(/^\/games\/([^/]+)$/.exec(window.location.pathname)![1])
    : null;

export const App: React.FC = () => {
  const [gameId, setGameId] = useState(pathGameId);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState(() =>
    gameId ? (sessionStorage.getItem(storageKey(gameId)) ?? undefined) : undefined,
  );
  const navigate = (id: string | null, nextToken?: string) => {
    if (id) {
      if (nextToken) sessionStorage.setItem(storageKey(id), nextToken);
      history.pushState({}, "", `/games/${encodeURIComponent(id)}`);
    } else history.pushState({}, "", "/");
    setGameId(id);
    setToken(nextToken);
  };
  useLayoutEffect(() => {
    const receive = () => {
      const id = pathGameId();
      setGameId(id);
      setToken(id ? (sessionStorage.getItem(storageKey(id)) ?? undefined) : undefined);
    };
    window.addEventListener("popstate", receive);
    return () => window.removeEventListener("popstate", receive);
  }, []);
  if (DevDecisionGallery && window.location.pathname === "/dev/decisions")
    return (
      <React.Suspense fallback={<main>Loading decision gallery…</main>}>
        <DevDecisionGallery />
      </React.Suspense>
    );
  if (DevMapPickerGallery && window.location.pathname === "/dev/map-picker")
    return (
      <React.Suspense fallback={<main>Loading map picker…</main>}>
        <DevMapPickerGallery />
      </React.Suspense>
    );
  if (DevToastGallery && window.location.pathname === "/dev/toasts")
    return (
      <React.Suspense fallback={<main>Loading toasts…</main>}>
        <DevToastGallery />
      </React.Suspense>
    );
  if (DevScenarioLauncher && window.location.pathname === "/dev/scenarios")
    return (
      <React.Suspense fallback={<main>Loading dev scenarios…</main>}>
        <DevScenarioLauncher />
      </React.Suspense>
    );
  if (!gameId)
    return (
      <>
        <CreateLobby
          onError={setError}
          onCreated={(created) => navigate(created.game_id, created.player_session)}
        />
        {error && (
          <div className="session-error" role="alert">
            {error}
          </div>
        )}
      </>
    );
  return (
    <GameRoute
      key={gameId}
      gameId={gameId}
      token={token}
      onCredential={(credential) => {
        sessionStorage.setItem(storageKey(gameId), credential);
        setToken(credential);
      }}
      onCredentialInvalid={() => {
        sessionStorage.removeItem(storageKey(gameId));
        setToken(undefined);
      }}
      onForget={() => {
        sessionStorage.removeItem(storageKey(gameId));
        navigate(null);
      }}
    />
  );
};

const GameRoute: React.FC<{
  gameId: string;
  token?: string;
  onCredential: (credential: string) => void;
  onCredentialInvalid: () => void;
  onForget: () => void;
}> = ({ gameId, token, onCredential, onCredentialInvalid, onForget }) => {
  const {
    lobby,
    playerId,
    error,
    loading,
    invalidCredential,
    pendingAction,
    setReady,
    start,
    reorder,
    chooseMap,
    join,
    leave,
    addBot,
    removeBot,
  } = useLobbySession(gameId, token);
  const [watching, setWatching] = useState(false);
  const invalidate = useCallback(() => onCredentialInvalid(), [onCredentialInvalid]);
  usePresence(gameId, token, invalidate);
  useEffect(() => {
    if (invalidCredential) onCredentialInvalid();
  }, [invalidCredential, onCredentialInvalid]);
  if (!lobby)
    return (
      <main className="lobby-page">
        <div className="panel lobby-panel">
          {loading ? "Loading lobby..." : "Unable to load lobby."}
        </div>
      </main>
    );
  const viewer: ViewerRole =
    token && playerId
      ? { role: "player", seat: playerId, playerSession: token }
      : { role: "spectator" };
  const enter = async (nickname: string, id?: string) => {
    const credential = await join(nickname, id);
    if (credential) {
      setWatching(false);
      onCredential(credential);
    }
  };
  const leaveLobby = async () => {
    if (await leave()) onForget();
  };
  return (
    <>
      {error && (
        <div className="session-error" role="alert">
          {participantText(error, lobby, [])} Check the lobby and try again.
        </div>
      )}
      {lobby.phase === "running" && (playerId || watching) ? (
        <GameViewContainer
          key={`${gameId}:${token ?? "watch"}`}
          gameId={gameId}
          lobby={lobby}
          viewer={viewer}
          onLeave={onForget}
        />
      ) : (
        <LobbyStatus
          lobby={lobby}
          playerId={playerId}
          watching={watching}
          pendingAction={pendingAction}
          onReady={(ready) => void setReady(ready)}
          onStart={() => void start()}
          onLeave={() => void leaveLobby()}
          onJoin={(name) => void enter(name)}
          onTakeover={(id, name) => void enter(name, id)}
          onReorder={(ids) => void reorder(ids)}
          onChooseMap={chooseMap}
          onWatch={() => setWatching(true)}
          onAddBot={(password, name) => addBot(password, name)}
          onRemoveBot={(targetId) => removeBot(targetId)}
        />
      )}
    </>
  );
};

/** Gives a seated player the bluff selector on the player mat; a spectator gets none. */
const BluffScope: React.FC<{
  gameId: string;
  seat: string | undefined;
  intent: import("./protocol/types.ts").ReactionIntentStateMsg | null;
  round: number;
  onChange: (triggers: string[]) => void;
  children: React.ReactNode;
}> = ({ gameId, seat, intent, round, onChange, children }) =>
  seat ? (
    <BluffProvider gameId={gameId} seat={seat} intent={intent} round={round} onChange={onChange}>
      {children}
    </BluffProvider>
  ) : (
    <>{children}</>
  );

const GameViewContainer: React.FC<{
  gameId: string;
  lobby: import("./protocol/types.ts").LobbyDto;
  viewer: ViewerRole;
  onLeave?: () => void;
}> = ({ gameId, lobby, viewer }) => {
  const {
    status,
    gameVersion,
    snapshot,
    pendingChoice: realPendingChoice,
    turnStatus,
    lastError,
    events,
    history: gameHistory,
    submitChoice,
    setReactionMode,
    reactionIntent,
    setReactionIntent,
    passReactionHold,
    changeHistory,
    fetchReplay,
    fetchTurnRedoStatus,
    turnRedoCommand,
    submitMovementBatch,
    submitBatch,
    previewSecondary,
    batchResume,
    resumeBatch,
    dismissBatchResume,
  } = useGameSession({ gameId, viewer });
  const { playTurnNotification } = useTurnSound();
  // The event log drops the reader's manual expansion only for a new history generation (undo,
  // redo, restore). A reconnect re-sends the same generation with a fresh events array and keeps
  // the reader's place; servers without a generation fall back to the array identity.
  const logHistoryKey = useRef<unknown>(null);
  const logEventsIdentity = useRef<unknown>(null);
  if (
    snapshot?.type === "initial_snapshot" &&
    snapshot.events &&
    logEventsIdentity.current !== snapshot.events
  )
    logEventsIdentity.current = snapshot.events;
  logHistoryKey.current = logHistoryKeyFor(gameHistory?.generation, logEventsIdentity.current);
  const [localNotes, setLocalNotes] = useState<{ id: string; text: string }[]>([]);
  const [historyBusy, setHistoryBusy] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [undoRequest, setUndoRequest] = useState<{
    action: import("./protocol/client.ts").HistoryChange;
    steps: number;
  } | null>(null);
  const onChangeHistory = (action: import("./protocol/client.ts").HistoryChange, steps = 1) => {
    if (historyBusy) return;
    // Undo rewinds shared history, so ask once in a styled dialog before running it.
    if (action === "undo_pipeline" || steps > 1) {
      setUndoRequest({ action, steps });
      return;
    }
    runHistoryChange(action);
  };
  const runHistoryChange = (action: import("./protocol/client.ts").HistoryChange) => {
    setHistoryBusy(true);
    setHistoryError(null);
    void changeHistory(action)
      .then(() => {
        setSelectedOptionId(undefined);
        setSelectedPlanetId(null);
        setSelectedSystemId(null);
        setCardSubject(null);
      })
      .catch((error: unknown) =>
        setHistoryError(error instanceof Error ? error.message : String(error)),
      )
      .finally(() => setHistoryBusy(false));
  };
  const userSeat = viewer.role === "player" ? viewer.seat : undefined;
  const turnRedo = useTurnRedo({
    enabled: viewer.role === "player",
    fetchStatus: fetchTurnRedoStatus,
    command: turnRedoCommand,
    gameVersion,
    generation: gameHistory.generation ?? 0,
    viewerSeat: userSeat,
    onTimelineChanged: () => {
      setSelectedOptionId(undefined);
      setSelectedPlanetId(null);
      setSelectedSystemId(null);
      setCardSubject(null);
    },
  });
  // Strategy-card secondary preparation: while another seat resolves a card the viewer may prepare
  // an answer with the usual UI. `pendingChoice` is then a client-made stand-in for the question
  // (answered into the local plan, never submitted), or nothing while an auto-played answer is
  // being sent. The engine's own decision is `realPendingChoice`.
  const productionSubmitRef = useRef<
    | ((units: string[], destination: string, real: import("./protocol/types.ts").PendingChoiceDto) => Promise<void>)
    | null
  >(null);
  const prep = useSecondaryPrepare({
    gameId,
    viewerSeat: userSeat,
    players: snapshot?.view.players ?? [],
    events,
    board: snapshot?.view.board,
    phase: snapshot?.view.phase,
    history: gameHistory,
    realChoice: realPendingChoice,
    busy: historyBusy,
    submitChoice,
    submitBatch,
    previewSecondary: userSeat ? previewSecondary : undefined,
    refreshKey: `${events.length}:${events[events.length - 1]?.id ?? ""}:${gameHistory.generation ?? 0}`,
    submitProduction: (units, destination, real) =>
      productionSubmitRef.current
        ? productionSubmitRef.current(units, destination, real)
        : Promise.reject(new Error("The production builder is not available")),
  });
  const pendingChoice = prep.shownChoice;
  const paymentDraft = usePaymentDraftState(pendingChoice?.nonce);
  const tokenDraft = useCommandTokenDraft(pendingChoice);
  const [selectedOptionId, setSelectedOptionId] = useState<string>();
  const [selectedPlanetId, setSelectedPlanetId] = useState<string | null>(null);
  const [selectedSystemId, setSelectedSystemId] = useState<string | null>(null);
  const [cardSubject, setCardSubject] = useState<CardSubject | null>(null);
  const [isTechModalOpen, setIsTechModalOpen] = useState(false);
  const [isObjectivesModalOpen, setIsObjectivesModalOpen] = useState(false);

  // Play sound notification when it becomes the player's turn
  const previousPendingChoiceRef = useRef<string | null>(null);
  useEffect(() => {
    const isPendingChoiceForViewer =
      realPendingChoice && userSeat && realPendingChoice.actor === userSeat;

    if (isPendingChoiceForViewer && previousPendingChoiceRef.current !== realPendingChoice.nonce) {
      playTurnNotification();
    }

    previousPendingChoiceRef.current = realPendingChoice?.nonce ?? null;
  }, [realPendingChoice?.nonce, userSeat, realPendingChoice?.actor, playTurnNotification]);

  useEffect(() => {
    setSelectedOptionId(undefined);
    setSelectedPlanetId(null);
  }, [pendingChoice?.nonce]);
  // The usual UI shows the prepared answer already selected (the confirm stays the player's).
  const prefillId = prep.resolution.kind === "option" ? prep.resolution.optionId : null;
  const prefillNonce = prep.realChoice?.nonce;
  useEffect(() => {
    if (prefillId && prefillNonce && !prep.holding) setSelectedOptionId(prefillId);
  }, [prefillId, prefillNonce, pendingChoice?.nonce]); // eslint-disable-line react-hooks/exhaustive-deps
  // Leadership: the usual token panel opens with the prepared purchase already staged (tokens, their
  // pools and the payment on the map). Once per question, so the player's own changes stay.
  useTokenPrefill(prep, pendingChoice?.nonce, tokenDraft);
  const cardIsVisible =
    cardSubject &&
    snapshot &&
    (cardSubject.kind === "publicObjective"
      ? snapshot.view.table.revealed_objectives.includes(cardSubject.id)
      : cardSubject.kind === "strategy"
        ? snapshot.view.players.some((player) => player.strategy_cards.includes(cardSubject.id))
        : cardSubject.kind === "action"
          ? snapshot.view.players.some(
              (player) =>
                player.id === userSeat && player.held_action_cards?.includes(cardSubject.id),
            )
          : snapshot.view.players.some(
              (player) =>
                player.scored_secret_objectives?.includes(cardSubject.id) ||
                (player.id === userSeat && player.held_secret_objectives?.includes(cardSubject.id)),
            ));
  const handleSelectTarget = (systemId: string, planetId?: string) => {
    if (!pendingChoice || pendingChoice.actor !== userSeat) return;
    // Leadership purchase: a click on a planet that can pay flips it in the staged payment.
    if (tokenDraft.mapPayment) {
      if (planetId && tokenDraft.mapPayment.offer.planets.some((p) => p.planetId === planetId)) {
        tokenDraft.togglePlanet(planetId);
      }
      return;
    }
    // Paying: a click on a payable planet stages or unstages it (shared with the payment list).
    if (isPaymentChoice(pendingChoice)) {
      if (!planetId) return;
      const option = paymentOptionForPlanet(derivePaymentOffer(pendingChoice), planetId);
      if (!option) return;
      const staged = paymentDraft.draft.planetIds.find((id) => paymentPlanetKey(id) === planetId);
      paymentDraft.togglePlanet(staged ?? option.id);
      return;
    }
    const selection = resolveMapTargetSelection(
      pendingChoice,
      systemId,
      planetId,
      snapshot?.view.board,
    );
    if (selection.kind === "ignore") return;
    setSelectedOptionId(selection.optionId);
    setSelectedPlanetId(selection.planetId);
  };
  return (
    <PlayerIdentityProvider lobby={lobby} seatingOrder={snapshot?.view.seating_order ?? []}>
    <SeatInfoProvider players={snapshot?.view.players} viewerSeat={userSeat}>
    <DecisionTableProvider table={snapshot?.view ?? null}>
    <BluffScope
      gameId={gameId}
      seat={userSeat}
      intent={reactionIntent}
      round={snapshot?.view.round ?? 0}
      onChange={setReactionIntent}
    >
    <PreparedHintProvider
      value={
        !prep.preparing && prep.realChoice
          ? prep.resolution.kind === "option"
            ? { optionId: prep.resolution.optionId, text: prep.resolution.text }
            : prep.resolution.kind === "production"
              ? { optionId: "", text: prep.resolution.text, builds: prep.resolution.units }
              : null
          : null
      }
    >
      {/* The provider wraps the rest unindented to keep this diff small. */}
      <PaymentDraftProvider value={paymentDraft}>
      <CommandTokenDraftProvider value={tokenDraft}>
      {undoRequest && (
        <UndoConfirmDialog
          steps={undoRequest.steps}
          onCancel={() => setUndoRequest(null)}
          onConfirm={() => {
            const { action } = undoRequest;
            setUndoRequest(null);
            runHistoryChange(action);
          }}
        />
      )}
      {historyError && (
        <div className="session-error" role="alert">
          {historyError}
        </div>
      )}
      <CornerToastLayer
        events={events}
        players={snapshot?.view.players}
        viewerSeat={userSeat}
        pendingChoice={realPendingChoice}
        historyGeneration={gameHistory.generation}
        autoResolved={snapshot?.type === "state_update" ? snapshot.auto_resolved : undefined}
        ready={Boolean(snapshot)}
        localNotes={localNotes}
      />
      <GameShell
        header={
          <div className="game-header">
            <TurnStatusBar
              status={turnStatus}
              view={snapshot?.view ?? null}
              gameVersion={gameVersion}
              connectionStatus={status}
              userSeat={userSeat}
            />
            <BluffHoldBar holding={Boolean(reactionIntent?.holding)} onPass={passReactionHold} />
            <div className="game-header__actions">
              <button
                type="button"
                data-testid="technology-modal-button"
                onClick={() => setIsTechModalOpen(true)}
                className="button button--secondary"
              >
                Technologies
              </button>
              <button
                type="button"
                data-testid="objectives-modal-button"
                onClick={() => setIsObjectivesModalOpen(true)}
                className="button button--secondary"
              >
                Objectives
              </button>
            </div>
          </div>
        }
        board={
          snapshot ? (
            <Board
              board={snapshot.view.board}
              seatingOrder={snapshot.view.seating_order}
              players={snapshot.view.players}
              pendingChoice={pendingChoice}
              viewerSeat={userSeat}
              selectedSystemId={selectedSystemId}
              onSelectSystem={(id) => {
                setSelectedSystemId(id);
                setCardSubject(null);
                // In a planet selection a hex click only inspects; it never drops the pick.
                if (
                  id &&
                  pendingChoice &&
                  pendingChoice.actor === userSeat &&
                  !isPlanetSelectionChoice(pendingChoice)
                ) {
                  const match = pendingChoice.options.find(
                    (option) =>
                      String(option.payload?.system ?? option.payload?.to ?? option.id) === id,
                  );
                  if (!match) {
                    setSelectedOptionId(undefined);
                  }
                }
              }}
              onSelectOptionId={(id) => {
                if (pendingChoice?.options.some((option) => option.id === id))
                  setSelectedOptionId(id);
              }}
              onSelectTarget={handleSelectTarget}
            />
          ) : (
            <div className="game-loading">Loading game state...</div>
          )
        }
        boardView={snapshot?.view.board}
        activeSystemId={
          typeof snapshot?.state.active_system === "string" ? snapshot.state.active_system : null
        }
        playerSheet={
          snapshot ? (
            <PlayerSheet
              players={snapshot.view.players}
              userSeat={userSeat}
              seatingOrder={snapshot.view.seating_order}
              revealedObjectives={snapshot.view.table.revealed_objectives}
              board={snapshot.view.board}
              table={snapshot.view.table}
              reactionModes={snapshot.reaction_modes}
              onSetReactionMode={userSeat ? setReactionMode : undefined}
              onInspectCard={(subject) => {
                setSelectedSystemId(null);
                setCardSubject(subject);
              }}
            />
          ) : null
        }
        detail={
          cardSubject &&
          cardIsVisible && (
            <CardDetails subject={cardSubject} onClose={() => setCardSubject(null)} />
          )
        }
        events={events}
        currentPath={snapshot?.current_path}
        logHistoryKey={logHistoryKey.current}
        history={gameHistory}
        historyBusy={historyBusy}
        onAutoSubmitNotice={(note) => setLocalNotes((prev) => [...prev, note])}
        onChangeHistory={userSeat === lobby.host_player_id ? onChangeHistory : undefined}
        onFetchReplay={userSeat ? fetchReplay : undefined}
        turnRedo={
          userSeat
            ? {
                status: turnRedo.status,
                busy: turnRedo.busy,
                error: turnRedo.error,
                onRequest: turnRedo.request,
                onAutoplay: turnRedo.autoplay,
                onRestore: turnRedo.restore,
                onKeep: turnRedo.keep,
                viewerSeat: userSeat,
                seats:
                  userSeat === lobby.host_player_id && snapshot
                    ? [userSeat, ...snapshot.view.seating_order.filter((id) => id !== userSeat)].map(
                        (id) => ({
                          id,
                          label: playerDisplay(lobby, snapshot.view.seating_order, id).label,
                        }),
                      )
                    : undefined,
              }
            : undefined
        }
        choice={pendingChoice}
        viewerSeat={userSeat}
        players={snapshot?.view.players}
        turn={
          snapshot
            ? {
                phase: snapshot.view.phase,
                activePlayer: snapshot.view.active_player ?? null,
              }
            : undefined
        }
        revealedObjectives={snapshot?.view.table.revealed_objectives}
        scoredObjectives={snapshot?.view.table.scored_objectives}
        objectiveProgress={snapshot?.view.table.objective_progress}
        onSubmitChoice={prep.preparing ? prep.prepareSubmit : submitChoice}
        prepOverlay={
          <SecondaryPrepHost prep={prep} busy={historyBusy} />
        }
        reactionModes={snapshot?.reaction_modes}
        onSetReactionMode={userSeat ? setReactionMode : undefined}
        onSubmitMovementBatch={submitMovementBatch}
        onSubmitBasketBatch={prep.preparing ? prep.prepareBatch : submitBatch}
        productionSubmitRef={productionSubmitRef}
        batchResume={batchResume}
        onResumeBatch={resumeBatch}
        onDismissBatchResume={dismissBatchResume}
        lastError={lastError}
        selectedOptionId={selectedOptionId}
        onSelectOption={setSelectedOptionId}
        selectedPlanetId={selectedPlanetId}
        onSelectPlanet={setSelectedPlanetId}
        onShowSystem={setSelectedSystemId}
      />
      <TechnologyModal
        isOpen={isTechModalOpen}
        onClose={() => setIsTechModalOpen(false)}
        players={snapshot?.view.players}
        viewerSeat={userSeat}
      />
      <ObjectivesModal
        isOpen={
          isObjectivesModalOpen &&
          pendingChoice?.context?.subtype !== "score_objective" &&
          pendingChoice?.context?.subtype !== "imperial_score_objective"
        }
        onClose={() => setIsObjectivesModalOpen(false)}
        revealedObjectives={snapshot?.view.table.revealed_objectives}
        scoredObjectives={snapshot?.view.table.scored_objectives}
        objectiveProgress={snapshot?.view.table.objective_progress}
        players={snapshot?.view.players}
        viewerSeat={userSeat}
        onInspectCard={(subject) => {
          setSelectedSystemId(null);
          setCardSubject(subject);
        }}
      />
      </CommandTokenDraftProvider>
      </PaymentDraftProvider>
    </PreparedHintProvider>
    </BluffScope>
    </DecisionTableProvider>
    </SeatInfoProvider>
    </PlayerIdentityProvider>
  );
};
